use std::fs;
use std::path::Path;

pub fn setup_greenluma_config(gl_path: &Path, enable_stealth: bool) -> Result<(), String> {
    if !gl_path.exists() {
        return Err("GreenLuma path does not exist".to_string());
    }

    // 1. NoQuestion.bin (Always create)
    let no_question = gl_path.join("NoQuestion.bin");
    if !no_question.exists() {
        fs::write(&no_question, "").map_err(|e| e.to_string())?;
    }

    // 2. StealthMode.bin (Toggle)
    let stealth_bin = gl_path.join("StealthMode.bin");
    if enable_stealth {
        if !stealth_bin.exists() {
            fs::write(&stealth_bin, "").map_err(|e| e.to_string())?;
        }
    } else {
        if stealth_bin.exists() {
            fs::remove_file(&stealth_bin).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
