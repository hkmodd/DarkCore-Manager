fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        // Point to icon.ico in the project root
        // If it doesn't exist yet, this might fail build, or winres handles missing gracefully?
        // Winres usually errors if file missing.
        // Instructions for user: MUST provide icon.ico.
        res.set_icon("icon.ico");
        res.compile().unwrap();
    }
}
