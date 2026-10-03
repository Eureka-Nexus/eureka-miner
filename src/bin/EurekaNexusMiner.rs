#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("Eureka Nexus Miner Desktop 1.1.0 is for Windows.");
}

#[cfg(target_os = "windows")]
mod app {
    use anyhow::{anyhow, Context, Result};
    use std::{
        io::Write,
        net::{TcpListener, TcpStream},
        os::windows::io::AsRawHandle,
        os::windows::process::CommandExt,
        path::PathBuf,
        process::{Child, Command, Stdio},
        thread,
        time::Duration,
    };

    use tao::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder},
        window::{Icon, WindowBuilder},
    };

    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
    };
    use wry::{WebContext, WebViewBuilder};

    struct MiningJob(HANDLE);

    impl MiningJob {
        fn new() -> Result<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(std::io::Error::last_os_error().into());
                }
                let job = Self(handle);
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    std::mem::size_of_val(&limits) as u32,
                ) == 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(job)
            }
        }

        fn attach(&self, child: &Child) -> Result<()> {
            if unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle()) } == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(())
        }
    }

    impl Drop for MiningJob {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    const DASHBOARD_ADDR: &str = "127.0.0.1:8077";
    const DASHBOARD_URL: &str = "http://127.0.0.1:8077";
    const INSTANCE_ADDR: &str = "127.0.0.1:48077";
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    fn webview_data_dir() -> Result<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|profile| PathBuf::from(profile).join("AppData").join("Local"))
            })
            .ok_or_else(|| anyhow!("Cannot locate Windows LocalAppData directory"))?;

        let dir = base.join("EurekaNexus").join("WebView2");

        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Cannot create WebView2 data directory {}", dir.display()))?;

        Ok(dir)
    }

    #[derive(Debug, Clone, Copy)]
    enum UserEvent {
        Show,
    }

    struct Backend {
        child: Child,
        job: MiningJob,
        stopped: bool,
    }

    impl Backend {
        fn shutdown(&mut self) {
            if self.stopped {
                return;
            }

            // The job owns the backend and every mining child, even if HTTP is stuck.
            unsafe {
                TerminateJobObject(self.job.0, 0);
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.stopped = true;
        }
    }

    impl Drop for Backend {
        fn drop(&mut self) {
            self.shutdown();
        }
    }

    fn app_icon() -> Option<Icon> {
        let img = image::load_from_memory(include_bytes!("../../web/branding/eureka_app_icon.png"))
            .ok()?
            .into_rgba8();

        let (width, height) = img.dimensions();

        Icon::from_rgba(img.into_raw(), width, height).ok()
    }

    fn signal_existing_instance() -> bool {
        if let Ok(mut stream) = TcpStream::connect(INSTANCE_ADDR) {
            let _ = stream.write_all(b"SHOW\n");
            return true;
        }

        false
    }

    fn wait_for_backend(child: &mut Child) -> Result<()> {
        for _ in 0..100 {
            if let Some(status) = child.try_wait()? {
                return Err(anyhow!("Mining backend stopped during startup: {status}"));
            }

            if TcpStream::connect(DASHBOARD_ADDR).is_ok() {
                return Ok(());
            }

            thread::sleep(Duration::from_millis(100));
        }

        Err(anyhow!("Timeout starting mining backend"))
    }

    fn backend_paths() -> Result<(PathBuf, PathBuf)> {
        let desktop_exe = std::env::current_exe().context("Cannot find application path")?;

        let app_dir = desktop_exe
            .parent()
            .context("Cannot find application directory")?
            .to_path_buf();

        let backend = app_dir.join("eureka-nexus-miner-official.exe");

        if !backend.is_file() {
            return Err(anyhow!("Mining backend not found: {}", backend.display()));
        }

        Ok((app_dir, backend))
    }

    pub fn run() -> Result<()> {
        let instance_listener = match TcpListener::bind(INSTANCE_ADDR) {
            Ok(listener) => listener,
            Err(error) => {
                if signal_existing_instance() {
                    return Ok(());
                }

                return Err(anyhow!("Cannot create Eureka instance lock: {error}"));
            }
        };

        let dashboard_probe = TcpListener::bind(DASHBOARD_ADDR)
            .context("Port 8077 is already occupied. Close the old Eureka Miner first.")?;

        drop(dashboard_probe);

        let (app_dir, backend_exe) = backend_paths()?;

        let job = MiningJob::new().context("Cannot create mining process job")?;
        let mut child = Command::new(&backend_exe)
            .current_dir(&app_dir)
            .env("EUREKA_DESKTOP_MODE", "1")
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Cannot start backend {}", backend_exe.display()))?;

        if let Err(error) = job.attach(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.context("Cannot attach backend to mining process job"));
        }

        let mut backend = Backend {
            child,
            job,
            stopped: false,
        };

        wait_for_backend(&mut backend.child)?;

        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

        let window = WindowBuilder::new()
            .with_title("Eureka Nexus Miner 1.1.0")
            .with_inner_size(LogicalSize::new(1400.0, 900.0))
            .with_min_inner_size(LogicalSize::new(1000.0, 650.0))
            .with_window_icon(app_icon())
            .build(&event_loop)
            .context("Cannot create Eureka Nexus Miner window")?;

        let webview_data_dir = webview_data_dir()?;
        let mut web_context = WebContext::new(Some(webview_data_dir));

        let webview = WebViewBuilder::new_with_web_context(&mut web_context)
            .with_url(DASHBOARD_URL)
            .build(&window)
            .context("Cannot create Eureka WebView2 window")?;

        let proxy = event_loop.create_proxy();

        thread::spawn(move || {
            for connection in instance_listener.incoming() {
                if connection.is_ok() {
                    let _ = proxy.send_event(UserEvent::Show);
                }
            }
        });

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            let _keep_webview_alive = &webview;
            let _keep_web_context_alive = &web_context;

            match event {
                Event::UserEvent(UserEvent::Show) => {
                    window.set_visible(true);
                    window.set_minimized(false);
                    window.set_focus();
                }

                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    backend.shutdown();
                    *control_flow = ControlFlow::Exit;
                }

                Event::LoopDestroyed => {
                    backend.shutdown();
                }

                _ => {}
            }
        });
    }
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = app::run() {
        let msg = format!("Eureka Nexus Miner error:\n{error:#}\n");

        let log_path = std::env::temp_dir().join("EurekaNexusMiner-startup.log");

        let _ = std::fs::write(&log_path, &msg);

        eprintln!("{msg}");
    }
}
