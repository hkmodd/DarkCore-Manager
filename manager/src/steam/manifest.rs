/// Generates a detailed ACF file with `InstalledDepots`.
/// This is used by Direct Download to make the game fully visible to Steam immediately.
pub fn generate_full_acf(
    acf_path: &std::path::Path,
    appid: &str,
    name: &str,
    installed_depots: &Vec<(String, u64, String)>, // (DepotID, Size, ManifestGID)
) -> std::io::Result<()> {
    // Ensure parent dir exists
    if let Some(parent) = acf_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Sanitize installdir (Matches SteamDB convention: Remove non-alphanumeric, keep spaces)
    let install_dir_sanitized: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .to_string();

    // Create the game directory in steamapps/common (Important for Direct Download if not already valid)
    if let Some(parent) = acf_path.parent() {
        let common_dir = parent.join("common");
        let game_dir = common_dir.join(&install_dir_sanitized);
        if !game_dir.exists() {
            let _ = std::fs::create_dir_all(&game_dir);
        }
    }

    // Build InstalledDepots Section
    let mut depots_section = String::from("\n\t\"InstalledDepots\"\n\t{");
    for (d_id, d_size, d_manifest) in installed_depots {
        depots_section.push_str(&format!(
            r#"
		"{}"
		{{
			"manifest"		"{}"
			"size"		"{}"
		}}"#,
            d_id, d_manifest, d_size
        ));
    }
    depots_section.push_str("\n\t}");

    // Calculate total size
    let total_size: u64 = installed_depots.iter().map(|(_, s, _)| s).sum();

    // StateFlags 4 = Fully Installed.
    let content = format!(
        r#""AppState"
{{
	"appid"		"{}"
	"Universe"		"1"
	"LauncherPath"		"{}"
	"name"		"{}"
	"StateFlags"		"4"
	"installdir"		"{}"
	"LastUpdated"		"{}"
	"SizeOnDisk"		"{}"
	"StagingSize"		"0"
	"buildid"		"0"
	"LastOwner"		"0"
	"UpdateResult"		"0"
	"BytesToDownload"		"{}"
	"BytesDownloaded"		"{}"
	"BytesToStage"		"0"
	"BytesStaged"		"0"
	"TargetBuildID"		"0"
	"AutoUpdateBehavior"		"0"
	"AllowOtherDownloadsWhileRunning"		"0"
	"ScheduledAutoUpdate"		"0"{}{}
	"UserConfig"
	{{
		"language"		"english"
	}}
	"MountedConfig"
	{{
		"language"		"english"
	}}
}}
"#,
        appid,
        "C:\\Program Files (x86)\\Steam\\steam.exe", // Default dummy
        name,
        install_dir_sanitized,
        "0",        // LastUpdated (TODO: Current timestamp?)
        total_size, // SizeOnDisk
        total_size, // BytesToDownload
        total_size, // BytesDownloaded
        depots_section,
        "" // Extra
    );

    std::fs::write(acf_path, content)?;
    Ok(())
}

/// Generates a minimal, "SMD-style" ACF file.
/// This is preferred as it relies on Steam to fill in the details.
pub fn generate_smd_style_acf(
    acf_path: &std::path::Path,
    appid: &str,
    game_name: &str,
) -> std::io::Result<()> {
    // Ensure parent dir exists
    if let Some(parent) = acf_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Sanitize installdir (Remove non-alphanumeric except spaces, similar to pathvalidate)
    let install_dir_sanitized: String = game_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .to_string();

    // NOTE: We do NOT create the game directory here.
    // SMD doesn't create it either. Steam will create it during download.
    // Creating an empty folder causes Steam to think the game is "installed but corrupt".

    // MINIMAL ACF - Exactly 5 fields like SMD
    // StateFlags "4" = Fully Installed (tells Steam game is ready but needs update)
    let content = format!(
        r#""AppState"
{{
	"appid"		"{}"
	"Universe"		"1"
	"name"		"{}"
	"installdir"		"{}"
	"StateFlags"		"4"
}}
"#,
        appid, game_name, install_dir_sanitized,
    );

    std::fs::write(acf_path, content)?;
    Ok(())
}
