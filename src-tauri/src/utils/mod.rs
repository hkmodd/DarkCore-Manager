pub mod crypto;
pub mod gl_config;
pub mod lua_parser;
pub mod vdf;

use std::collections::HashMap;

pub struct AcfDepotInfo {
    pub gid: u64,
    pub size: u64,
}

pub fn generate_ghost_acf(
    acf_path: &std::path::Path,
    appid: &str,
    install_dir: &str,
    _game_name: &str,
    depots: &HashMap<u32, AcfDepotInfo>, // Changed to include size
    build_id: u64,
    total_size: u64,
) -> std::io::Result<()> {
    if let Some(parent) = acf_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // InstalledDepots: DepotID -> { "manifest" "gid", "size" "bytes" }
    let mut installed_depots_str = String::new();
    // MountedDepots: DepotID -> "gid"
    let mut mounted_depots_str = String::new();

    for (depot_id, info) in depots {
        // InstalledDepots Block
        installed_depots_str.push_str(&format!(
            "\t\t\"{}\"\n\t\t{{\n\t\t\t\"manifest\"\t\t\"{}\"\n\t\t\t\"size\"\t\t\"{}\"\n\t\t}}\n",
            depot_id, info.gid, info.size
        ));

        // MountedDepots Line
        mounted_depots_str.push_str(&format!("\t\t\"{}\"\t\t\"{}\"\n", depot_id, info.gid));
    }

    let depots_section = if depots.is_empty() {
        String::new()
    } else {
        format!(
            "\t\"InstalledDepots\"\n\t{{\n{}\t}}\n\t\"MountedDepots\"\n\t{{\n{}\t}}\n",
            installed_depots_str, mounted_depots_str
        )
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let content = format!(
        r#""AppState"
{{
	"appid"		"{}"
	"Universe"		"1"
	"name"		"{}"
	"StateFlags"		"4"
	"installdir"		"{}"
	"LastUpdated"		"{}"
	"UpdateResult"		"0"
	"SizeOnDisk"		"{}"
	"buildid"		"{}"
	"LastOwner"		"76561198000000000"
	"BytesToDownload"		"0"
	"BytesDownloaded"		"0"
	"BytesToStage"		"0"
	"BytesStaged"		"0"
	"AutoUpdateBehavior"		"0"
{}
}}"#,
        appid, _game_name, install_dir, timestamp, total_size, build_id, depots_section
    );

    std::fs::write(acf_path, content)?;
    Ok(())
}
