fn main() {
    // Embed the app icon into the .exe (only when building for Windows)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("resources/windows/clockode.ico");
        res.compile().expect("failed to embed Windows resources");
    }
}
