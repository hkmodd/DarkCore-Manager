use self_update::cargo_crate_version;
use std::error::Error;

/// Checks GitHub Releases for a newer version.
/// Returns Some(release) if update available, None if already up-to-date.
pub fn check_for_updates() -> Result<Option<self_update::update::Release>, Box<dyn Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("hkmodd")
        .repo_name("DarkCore-Manager")
        .bin_name("darkcore-manager") // Matches the .exe filename in GitHub release
        .target("x86_64-pc-windows-msvc") // Windows target
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .no_confirm(true) // Silent operation
        .build()?;

    let latest = status.get_latest_release()?;

    // Compare versions
    if self_update::version::bump_is_greater(cargo_crate_version!(), &latest.version)? {
        Ok(Some(latest))
    } else {
        Ok(None)
    }
}

/// Downloads and installs the latest release.
/// Performs backup of current exe and swaps in the new one.
pub fn perform_update() -> Result<(), Box<dyn Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("hkmodd")
        .repo_name("DarkCore-Manager")
        .bin_name("darkcore-manager") // Must match GitHub asset: darkcore-manager.exe
        .target("x86_64-pc-windows-msvc")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .no_confirm(true)
        .build()?;

    // Download, backup current, and swap
    status.update()?;

    Ok(())
}

/// Restarts the application after update.
/// Spawns new process and exits current one.
pub fn restart_application() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}
