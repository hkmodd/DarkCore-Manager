use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub app_ids: Vec<String>,
}

pub struct ProfileManager {
    pub profiles_dir: String,
}

impl ProfileManager {
    pub fn new(base_path: &str) -> Self {
        let path = Path::new(base_path).join("Profiles");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        ProfileManager {
            profiles_dir: path.to_string_lossy().to_string(),
        }
    }

    pub fn list_profiles(&self) -> Vec<String> {
        let mut names = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.profiles_dir) {
            for entry in entries.flatten() {
                if let Ok(fname) = entry.file_name().into_string() {
                    if fname.ends_with(".json") {
                        names.push(fname.replace(".json", ""));
                    }
                }
            }
        }
        names
    }

    pub fn load_profile(&self, name: &str) -> Result<Profile, io::Error> {
        let path = Path::new(&self.profiles_dir).join(format!("{}.json", name));
        let content = fs::read_to_string(path)?;
        let profile: Profile = serde_json::from_str(&content)?;
        Ok(profile)
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<(), io::Error> {
        let path = Path::new(&self.profiles_dir).join(format!("{}.json", profile.name));
        let json = serde_json::to_string_pretty(profile)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn delete_profile(&self, name: &str) -> Result<(), io::Error> {
        let path = Path::new(&self.profiles_dir).join(format!("{}.json", name));
        fs::remove_file(path)
    }
}
