use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct VaultManager {
    base_path: PathBuf,
}

impl VaultManager {
    pub fn new(base_dir: &str) -> Self {
        let path = Path::new(base_dir).join("Vault");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        Self { base_path: path }
    }

    fn get_path(&self, appid: &str) -> PathBuf {
        self.base_path.join(format!("{}.lua", appid))
    }

    pub fn exists(&self, appid: &str) -> bool {
        self.get_path(appid).exists()
    }

    pub fn save(&self, appid: &str, data: &[u8]) -> std::io::Result<()> {
        fs::write(self.get_path(appid), data)
    }

    pub fn get(&self, appid: &str) -> std::io::Result<Vec<u8>> {
        fs::read(self.get_path(appid))
    }

    pub fn get_timestamp(&self, appid: &str) -> Option<SystemTime> {
        if let Ok(meta) = fs::metadata(self.get_path(appid)) {
            meta.modified().ok()
        } else {
            None
        }
    }
}
