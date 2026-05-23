use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_steamless(target_exe: &str, steamless_path: &str) -> Result<String, String> {
    let target = Path::new(target_exe);
    let tool = Path::new(steamless_path);

    if !target.exists() || !tool.exists() {
        return Err("Paths invalid (Target or Steamless not found)".to_string());
    }

    let parent_dir = target.parent().unwrap_or(Path::new("."));

    // Run Steamless
    // Steamless CLI: "Steamless.CLI.exe process_file.exe"
    let output = Command::new(tool)
        .arg(target)
        .output()
        .map_err(|e| format!("Failed to run Steamless: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Steamless CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Determine unpacked name
    // Steamless typically produces: "Game.exe" -> "Game.unpacked.exe"
    // But some versions might do "Game.exe.unpacked.exe"
    // We check both.
    let file_stem = target.file_stem().unwrap().to_string_lossy();
    let ext = target.extension().unwrap_or_default().to_string_lossy();

    // Possibility 1: Game.unpacked.exe
    let unpacked_1_name = format!("{}.unpacked.{}", file_stem, ext);
    let unpacked_1_path = parent_dir.join(&unpacked_1_name);

    // Possibility 2: Game.exe.unpacked.exe
    let target_filename = target.file_name().unwrap().to_string_lossy();
    let unpacked_2_name = format!("{}.unpacked.exe", target_filename); // Approximate
    let unpacked_2_path = parent_dir.join(&unpacked_2_name);

    let unpacked_final = if unpacked_1_path.exists() {
        unpacked_1_path
    } else if unpacked_2_path.exists() {
        unpacked_2_path
    } else {
        return Err(
            "Steamless ran but unpacked file was not found. (Check Steamless output manualy)"
                .to_string(),
        );
    };

    // Backup Original -> .bak
    let backup_name = format!("{}.bak", target_filename);
    let backup_path = parent_dir.join(backup_name);

    // If backup exists, delete it first (overwrite)
    if backup_path.exists() {
        let _ = fs::remove_file(&backup_path);
    }

    if let Err(e) = fs::rename(target, &backup_path) {
        return Err(format!("Failed to create backup (rename error): {}", e));
    }

    // Rename Unpacked -> Original
    if let Err(e) = fs::rename(&unpacked_final, target) {
        // Try to restore backup
        let _ = fs::rename(&backup_path, target);
        return Err(format!("Failed to apply unpacked file: {}", e));
    }

    Ok("Patch successful. Original backed up as .bak".to_string())
}

/// List of EXE patterns to SKIP (non-game executables)
const SKIP_PATTERNS: &[&str] = &[
    "unins",
    "uninst",
    "setup",
    "install",
    "vcredist",
    "dxsetup",
    "dotnet",
    "crash",
    "ue4prereq",
    "easyanticheat",
    "battleye",
    "launcher", // Often not the main game
    "updater",
    "patcher",
];

/// Find all .exe files in a game folder recursively, filtering out non-game executables
pub fn find_game_executables(game_folder: &Path) -> Vec<std::path::PathBuf> {
    let mut exes = Vec::new();

    if !game_folder.exists() {
        return exes;
    }

    // Use walkdir to recursively scan
    fn scan_dir(dir: &Path, exes: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Skip some common non-game directories
                    let dir_name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();
                    if !dir_name.contains("redist")
                        && !dir_name.contains("support")
                        && !dir_name.contains("_commonredist")
                    {
                        scan_dir(&path, exes);
                    }
                } else if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("exe") {
                        exes.push(path);
                    }
                }
            }
        }
    }

    scan_dir(game_folder, &mut exes);

    // Filter out non-game executables
    exes.into_iter()
        .filter(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();

            // Skip if matches any skip pattern
            !SKIP_PATTERNS.iter().any(|pattern| name.contains(pattern))
        })
        .collect()
}

/// Generate steam_appid.txt in the game folder
pub fn generate_steam_appid(game_folder: &Path, app_id: &str) -> Result<(), String> {
    let appid_path = game_folder.join("steam_appid.txt");
    fs::write(&appid_path, app_id)
        .map_err(|e| format!("Failed to create steam_appid.txt: {}", e))?;
    Ok(())
}

/// Result of patching an individual EXE
pub struct PatchResult {
    pub exe_path: String,
    pub success: bool,
    pub message: String,
}

/// Run Steamless on all game executables in a folder
/// Returns: (successful_count, total_count, detailed_results)
pub fn run_steamless_folder(
    game_folder: &Path,
    steamless_path: &str,
    app_id: &str,
) -> (usize, usize, Vec<PatchResult>) {
    let exes = find_game_executables(game_folder);
    let total = exes.len();
    let mut success_count = 0;
    let mut results = Vec::new();

    // Generate steam_appid.txt first
    if let Err(e) = generate_steam_appid(game_folder, app_id) {
        results.push(PatchResult {
            exe_path: "steam_appid.txt".to_string(),
            success: false,
            message: e,
        });
    } else {
        results.push(PatchResult {
            exe_path: "steam_appid.txt".to_string(),
            success: true,
            message: format!("Created with AppID {}", app_id),
        });
    }

    // Patch each EXE
    for exe in exes {
        let exe_str = exe.to_string_lossy().to_string();
        let exe_name = exe
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        match run_steamless(&exe_str, steamless_path) {
            Ok(msg) => {
                success_count += 1;
                results.push(PatchResult {
                    exe_path: exe_name,
                    success: true,
                    message: msg,
                });
            }
            Err(e) => {
                // Not necessarily an error - some EXEs may not have DRM
                results.push(PatchResult {
                    exe_path: exe_name,
                    success: false,
                    message: e,
                });
            }
        }
    }

    (success_count, total, results)
}
