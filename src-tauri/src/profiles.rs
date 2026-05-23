use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub app_ids: Vec<String>,
}

pub struct ProfileManager {
    base_path: PathBuf, // Usually the root folder where "Profiles" dir lives
}

impl ProfileManager {
    pub fn new() -> Self {
        // Determine base path (portable)
        let base_path = if let Ok(exe_path) = std::env::current_exe() {
            exe_path.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            PathBuf::from(".")
        };

        let profiles_dir = base_path.join("Profiles");
        if !profiles_dir.exists() {
            let _ = fs::create_dir_all(&profiles_dir);
        }

        Self { base_path }
    }

    fn get_profiles_dir(&self) -> PathBuf {
        self.base_path.join("Profiles")
    }

    pub fn list_profiles(&self) -> Vec<String> {
        let mut names = Vec::new();
        if let Ok(entries) = fs::read_dir(self.get_profiles_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
        names
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<(), String> {
        let dir = self.get_profiles_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        }

        let path = dir.join(format!("{}.json", profile.name));
        let content = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())
    }

    pub fn load_profile(&self, name: &str) -> Result<Profile, String> {
        let path = self.get_profiles_dir().join(format!("{}.json", name));
        if !path.exists() {
            // Return empty profile if not found
            return Ok(Profile {
                name: name.to_string(),
                app_ids: Vec::new(),
            });
        }
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    pub fn delete_profile(&self, name: &str) -> Result<(), String> {
        let path = self.get_profiles_dir().join(format!("{}.json", name));
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
