use self_update::cargo_crate_version;
use std::error::Error;

pub fn check_for_updates() -> Result<Option<self_update::update::Release>, Box<dyn Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("hkmodd")
        .repo_name("DarkCore-Manager")
        .bin_name("DarkCore-Manager") // Match release asset name
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?;

    let latest = status.get_latest_release()?;

    // Compare versions
    if self_update::version::bump_is_greater(cargo_crate_version!(), &latest.version)? {
        Ok(Some(latest))
    } else {
        Ok(None)
    }
}

pub fn perform_update() -> Result<(), Box<dyn Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("hkmodd")
        .repo_name("DarkCore-Manager")
        .bin_name("DarkCore-Manager")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?;

    // This performs download, backup, and swap
    status.update()?;

    Ok(())
}

pub fn restart_application() {
    let _ = std::process::Command::new(std::env::current_exe().unwrap()).spawn();
    std::process::exit(0);
}
