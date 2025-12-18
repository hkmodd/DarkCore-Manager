use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_steamless(target_exe: &str, steamless_path: &str) -> Result<String, String> {
    let target = Path::new(target_exe);
    let tool = Path::new(steamless_path);

    if !target.exists() || !tool.exists() {
        return Err("Paths invalid".to_string());
    }

    // Run Steamless
    // Steamless CLI usually takes just the path.
    let output = Command::new(tool)
        .arg(target)
        .output()
        .map_err(|e| format!("Failed to run Steamless: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Steamless failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Check for unpacked file
    // Steamless creates "filename.unpacked.exe" usually
    let file_stem = target.file_stem().unwrap().to_string_lossy();
    let ext = target.extension().unwrap_or_default().to_string_lossy();
    let unpacked_name = format!("{}.unpacked.{}", file_stem, ext);
    let unpacked_path = target.parent().unwrap().join(unpacked_name);

    if unpacked_path.exists() {
        // Backup original
        let backup_name = format!("{}.orig", target.file_name().unwrap().to_string_lossy());
        let backup_path = target.parent().unwrap().join(backup_name);

        if let Err(e) = fs::rename(target, &backup_path) {
            return Err(format!("Failed to backup original: {}", e));
        }

        if let Err(e) = fs::rename(&unpacked_path, target) {
            // Try to restore backup
            let _ = fs::rename(&backup_path, target);
            return Err(format!("Failed to move unpacked file: {}", e));
        }

        Ok("Patch successful. Original backed up.".to_string())
    } else {
        Err("Unpacked file not found after execution.".to_string())
    }
}
