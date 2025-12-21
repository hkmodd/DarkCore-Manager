use glob;
use regex;
use std::fs;
use std::path::{Path, PathBuf};

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

    /// Backs up the AppManifest (ACF) and identified Depot Manifests to the Vault.
    /// Vital for Offline Manual Fixes.
    pub fn backup_manifests(&self, steam_path: &str, appid: &str) -> std::io::Result<usize> {
        let mut count = 0;
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }

        // 1. Find and Copy Main ACF
        let acf_name = format!("appmanifest_{}.acf", appid);
        let steam_apps = Path::new(steam_path).join("steamapps");
        let acf_path = steam_apps.join(&acf_name);

        let mut mounted_depots = Vec::new();

        if acf_path.exists() {
            if let Ok(content) = fs::read_to_string(&acf_path) {
                // Parse MountedDepots via Regex
                // Pattern: "MountedDepots"\s*\{([^\}]+)\}
                let re_block = regex::Regex::new(r#""MountedDepots"\s*\{([^}]+)\}"#).unwrap();
                if let Some(caps) = re_block.captures(&content) {
                    if let Some(block) = caps.get(1) {
                        let re_val = regex::Regex::new(r#""(\d+)""#).unwrap();
                        for cap in re_val.captures_iter(block.as_str()) {
                            if let Some(id) = cap.get(1) {
                                mounted_depots.push(id.as_str().to_string());
                            }
                        }
                    }
                }

                // Copy ACF
                fs::copy(&acf_path, storage_dir.join(&acf_name))?;
                count += 1;
            }
        }

        // 2. DepotCache Manifests
        let depot_cache = Path::new(steam_path).join("depotcache");
        if depot_cache.exists() {
            for depot_id in mounted_depots {
                let pattern = format!("{}*.manifest", depot_id); // e.g. 12345*.manifest
                let glob_pat = depot_cache.join(&pattern);

                if let Ok(paths) = glob::glob(&glob_pat.to_string_lossy()) {
                    for path in paths.flatten() {
                        if let Some(fname) = path.file_name() {
                            let dest = storage_dir.join(fname);
                            if fs::copy(&path, dest).is_ok() {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    /// Restores AppManifest and Depot Manifests from the Vault to Steam.
    /// Returns: (restored_acf, restored_depots_count)
    pub fn restore_manifests(
        &self,
        steam_path: &str,
        appid: &str,
    ) -> std::io::Result<(bool, usize)> {
        let storage_dir = self.base_path.join(appid);
        if !storage_dir.exists() {
            return Ok((false, 0));
        }

        let mut restored_acf = false;
        let mut restored_count = 0;

        // 1. Restore ACF
        let acf_name = format!("appmanifest_{}.acf", appid);
        let vault_acf = storage_dir.join(&acf_name);
        // We restore ACF to the MAIN steamapps for simplicity.
        // Logic could be improved to restore to original library if tracked, but default is SteamPath.
        let steam_apps = Path::new(steam_path).join("steamapps");
        let target_acf = steam_apps.join(&acf_name);

        if vault_acf.exists() {
            // Only restore if target doesn't exist? Or overwrite?
            // "Repair" implies overwrite corruption.
            // "Install" implies target doesn't exist.
            // Overwriting is generally safer for "Restore".
            fs::copy(&vault_acf, &target_acf)?;
            restored_acf = true;
        }

        // 2. Restore Depot Manifests
        let depot_cache = Path::new(steam_path).join("depotcache");
        if !depot_cache.exists() {
            let _ = fs::create_dir_all(&depot_cache);
        }

        // Glob all .manifest files in vault
        let pattern = storage_dir.join("*.manifest");
        if let Ok(paths) = glob::glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                if let Some(fname) = path.file_name() {
                    let dest = depot_cache.join(fname);
                    if !dest.exists() {
                        // Avoid redundant writes if already there
                        if fs::copy(&path, dest).is_ok() {
                            restored_count += 1;
                        }
                    }
                }
            }
        }

        Ok((restored_acf, restored_count))
    }
}
