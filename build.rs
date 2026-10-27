fn main() {
    if std::path::Path::new("ui/main.slint").exists() {
        slint_build::compile("ui/main.slint").unwrap();
    }

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("wix/Product.ico");
        let _ = res.compile();
    }
}
