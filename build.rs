fn main() {
    if std::path::Path::new("ui/main.slint").exists() {
        slint_build::compile("ui/main.slint").unwrap();
    }

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("wix/Product.ico");
        res.set("FileDescription", "urpwallet - Local-first Personal Finance Tracker");
        res.set("ProductName", "urpwallet");
        res.set("CompanyName", "urpwallet team");
        res.set("LegalCopyright", "Copyright (c) 2026 Asraful");
        let _ = res.compile();
    }
}
