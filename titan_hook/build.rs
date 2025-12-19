fn main() {
    // Only on Windows, link exports to system version.dll
    #[cfg(target_os = "windows")]
    {
        // List of functions to forward to real version.dll
        let exports = vec![
            "GetFileVersionInfoA",
            "GetFileVersionInfoExA",
            "GetFileVersionInfoExW",
            "GetFileVersionInfoSizeA",
            "GetFileVersionInfoSizeExA",
            "GetFileVersionInfoSizeExW",
            "GetFileVersionInfoSizeW",
            "GetFileVersionInfoW",
            "VerFindFileA",
            "VerFindFileW",
            "VerInstallFileA",
            "VerInstallFileW",
            "VerLanguageNameA",
            "VerLanguageNameW",
            "VerQueryValueA",
            "VerQueryValueW",
        ];

        let system_dir = "C:\\Windows\\System32";

        for export in exports {
            // Syntax: /EXPORT:Name=Path.Name
            println!(
                "cargo:rustc-link-arg=/EXPORT:{}={}\\version.{}",
                export, system_dir, export
            );
        }
    }
}
