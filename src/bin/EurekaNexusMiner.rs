#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(not(target_os = "windows"))]
fn main() {
    println!(
        "Eureka 2026 1.0 (technical version {}) is for Windows.",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(target_os = "windows")]
mod app {
    use anyhow::{anyhow, Context, Result};
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        os::windows::io::AsRawHandle,
        os::windows::process::CommandExt,
        path::{Path, PathBuf},
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

    fn updater_dir() -> Result<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("Cannot locate Windows LocalAppData directory")?;

        Ok(base.join("EurekaNexus").join("Updater"))
    }

    fn pending_update_path() -> Result<PathBuf> {
        Ok(updater_dir()?.join("pending-update.json"))
    }

    fn verified_pending_update() -> Result<Option<PathBuf>> {
        let pending = pending_update_path()?;

        if !pending.exists() {
            return Ok(None);
        }

        let raw = fs::read(&pending)
            .context("Cannot read pending update manifest")?;

        let manifest: serde_json::Value =
            serde_json::from_slice(&raw)
                .context("Invalid pending update manifest")?;

        let latest = manifest["latest"]
            .as_str()
            .context("Pending update version is missing")?;

        let latest_version =
            semver::Version::parse(latest)
                .context("Invalid pending update version")?;

        let current_version =
            semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

        if latest_version <= current_version {
            return Err(anyhow!(
                "Pending update {latest} is not newer than {}",
                env!("CARGO_PKG_VERSION")
            ));
        }

        let setup_raw = manifest["setup_path"]
            .as_str()
            .context("Pending installer path is missing")?;

        let setup = PathBuf::from(setup_raw);

        if !setup.is_file() {
            return Err(anyhow!(
                "Pending installer does not exist: {}",
                setup.display()
            ));
        }

        let expected_name =
            format!("Eureka-Nexus-Miner-Setup-{latest}.exe");

        if setup.file_name().and_then(|v| v.to_str())
            != Some(expected_name.as_str())
        {
            return Err(anyhow!(
                "Pending installer filename is not trusted"
            ));
        }

        let update_root =
            fs::canonicalize(updater_dir()?)
                .context("Cannot canonicalize updater directory")?;

        let setup_real =
            fs::canonicalize(&setup)
                .context("Cannot canonicalize pending installer")?;

        if !setup_real.starts_with(&update_root) {
            return Err(anyhow!(
                "Pending installer is outside the Eureka updater directory"
            ));
        }

        let expected_hash = manifest["sha256"]
            .as_str()
            .context("Pending installer SHA256 is missing")?
            .to_ascii_lowercase();

        if expected_hash.len() != 64
            || !expected_hash
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            return Err(anyhow!(
                "Invalid SHA256 in pending update manifest"
            ));
        }

        let mut file =
            fs::File::open(&setup_real)
                .context("Cannot open pending installer")?;

        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];

        loop {
            let count = file.read(&mut buffer)?;

            if count == 0 {
                break;
            }

            hasher.update(&buffer[..count]);
        }

        let actual_hash = hex::encode(hasher.finalize());

        if actual_hash != expected_hash {
            return Err(anyhow!(
                "Pending installer SHA256 verification failed"
            ));
        }

        Ok(Some(setup_real))
    }

    fn launch_verified_installer(setup: &Path) -> Result<()> {
        Command::new(setup)
            .arg("/SILENT")
            .arg("/SUPPRESSMSGBOXES")
            .arg("/NORESTART")
            .arg("/CLOSEAPPLICATIONS")
            .current_dir(
                setup.parent()
                    .context("Installer directory is unavailable")?
            )
            .spawn()
            .with_context(|| {
                format!(
                    "Cannot start verified installer {}",
                    setup.display()
                )
            })?;

        Ok(())
    }


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
            .with_title("Eureka 2026 1.0")
            .with_inner_size(LogicalSize::new(1180.0, 760.0))
            .with_min_inner_size(LogicalSize::new(820.0, 560.0))
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
            // Wake periodically so the launcher can detect an unexpected
            // backend termination. If the backend disappears, every mining
            // engine in the Windows Job Object is stopped immediately.
            *control_flow =
                ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(500));

            let _keep_webview_alive = &webview;
            let _keep_web_context_alive = &web_context;

            if !backend.stopped {
                match verified_pending_update() {
                    Ok(Some(setup)) => {
                        match launch_verified_installer(&setup) {
                            Ok(()) => {
                                if let Ok(pending) = pending_update_path() {
                                    let _ = fs::remove_file(pending);
                                }

                                backend.shutdown();
                                *control_flow = ControlFlow::Exit;
                                return;
                            }

                            Err(error) => {
                                if let Ok(pending) = pending_update_path() {
                                    let _ = fs::remove_file(pending);
                                }

                                let message = format!(
                                    "UPDATE FAILED\n\n\
                                     The verified installer could not be started:\n\
                                     {error:#}"
                                );

                                let script = format!(
                                    "alert({});",
                                    serde_json::to_string(&message)
                                        .unwrap_or_else(|_| "\"Update failed.\"".to_string())
                                );

                                let _ = webview.evaluate_script(&script);
                            }
                        }
                    }

                    Ok(None) => {}

                    Err(error) => {
                        if let Ok(pending) = pending_update_path() {
                            let _ = fs::remove_file(pending);
                        }

                        let message = format!(
                            "UPDATE VERIFICATION FAILED\n\n\
                             The pending update was rejected:\n\
                             {error:#}"
                        );

                        let script = format!(
                            "alert({});",
                            serde_json::to_string(&message)
                                .unwrap_or_else(|_| "\"Update verification failed.\"".to_string())
                        );

                        let _ = webview.evaluate_script(&script);
                    }
                }
            }

            if !backend.stopped {
                match backend.child.try_wait() {
                    Ok(Some(status)) => {
                        backend.shutdown();

                        window.set_title("Eureka 2026 1.0 - BACKEND STOPPED");

                        let message = format!(
                            "BACKEND INTERROMPIDO\n\n\
                             O processo principal terminou inesperadamente ({status}).\n\n\
                             A mineração GPU/CPU foi parada automaticamente por segurança.\n\
                             Feche e volte a abrir o Eureka 2026 1.0."
                        );

                        let script = format!(
                            "alert({});",
                            serde_json::to_string(&message)
                                .unwrap_or_else(|_| "\"Backend interrompido.\"".to_string())
                        );

                        let _ = webview.evaluate_script(&script);
                    }

                    Ok(None) => {}

                    Err(error) => {
                        backend.shutdown();

                        window.set_title("Eureka 2026 1.0 - BACKEND ERROR");

                        let message = format!(
                            "ERRO NO BACKEND\n\n\
                             Não foi possível verificar o processo principal: {error}\n\n\
                             A mineração GPU/CPU foi parada automaticamente por segurança.\n\
                             Feche e volte a abrir o Eureka 2026 1.0."
                        );

                        let script = format!(
                            "alert({});",
                            serde_json::to_string(&message)
                                .unwrap_or_else(|_| "\"Erro no backend.\"".to_string())
                        );

                        let _ = webview.evaluate_script(&script);
                    }
                }
            }

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
        let msg = format!("Eureka 2026 1.0 error:\n{error:#}\n");

        let log_path = std::env::temp_dir().join("EurekaNexusMiner-startup.log");

        let _ = std::fs::write(&log_path, &msg);

        eprintln!("{msg}");
    }
}
