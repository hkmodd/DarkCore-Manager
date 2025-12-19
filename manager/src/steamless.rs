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
