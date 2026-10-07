fn main() {
    println!("cargo:rerun-if-changed=web/branding/eureka_app_icon.ico");

    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();

        res.set_icon("web/branding/eureka_app_icon.ico");
        res.set("CompanyName", "Eureka Nexus");
        res.set("ProductName", "Eureka 2026 1.0");
        res.set("FileDescription", "Eureka 2026 1.0");
        res.set("LegalCopyright", "Copyright (c) 2026 Eureka Nexus");

        res.compile()
            .expect("failed to compile Eureka Nexus Windows resources");
    }
}
