fn main() {
    if std::path::Path::new("ui/main.slint").exists() {
        slint_build::compile("ui/main.slint").unwrap();
    }
}
