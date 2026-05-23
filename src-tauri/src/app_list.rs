use glob::glob;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

// NOTE: GreenLuma 2025 supports thousands of entries. No artificial limit.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameProfile {
    pub app_id: String,
    pub name: String,
    pub filename: String,
    pub parent_id: Option<String>,
    pub item_type: String, // "game", "dlc", "depot"
    pub is_installed: bool,
    pub injection_status: String, // "injected", "family_godmode", "applist_only"
    pub pending_update: Option<String>,
}

pub type RelationshipMap = HashMap<String, String>; // Child -> Parent

pub struct AppListManager {
    gl_path: PathBuf,
    steam_path: PathBuf,
}

impl AppListManager {
    pub fn new(gl_path: &Path, steam_path: &Path) -> Self {
        Self {
            gl_path: gl_path.to_path_buf(),
            steam_path: steam_path.to_path_buf(),
        }
    }

    pub fn set_paths(&mut self, gl_path: &Path, steam_path: &Path) {
        self.gl_path = gl_path.to_path_buf();
        self.steam_path = steam_path.to_path_buf();
    }

    pub fn load_relationships(&self) -> RelationshipMap {
        // v1.7.2 compat: check ./relationships.json first (next to exe), then gl_path/
        let local_path = Path::new(".").join("relationships.json");
        let gl_path = self.gl_path.join("relationships.json");

        for path in [&local_path, &gl_path] {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(map) = serde_json::from_str(&content) {
                        // Auto-migrate: if found in local but not in gl_path, copy over
                        if path == &local_path && !gl_path.exists() {
                            let _ = fs::copy(&local_path, &gl_path);
                        }
                        return map;
                    }
                }
            }
        }
        HashMap::new()
    }

    pub fn save_relationships(&self, map: &RelationshipMap) {
        let path = self.gl_path.join("relationships.json");
        if let Ok(content) = serde_json::to_string_pretty(map) {
            let _ = fs::write(path, content);
        }
    }

    pub fn load_types(&self) -> HashMap<String, String> {
        let local_path = Path::new(".").join("types.json");
        let gl_path = self.gl_path.join("types.json");

        for path in [&local_path, &gl_path] {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(map) = serde_json::from_str(&content) {
                        return map;
                    }
                }
            }
        }
        HashMap::new()
    }

    pub fn save_types(&self, map: &HashMap<String, String>) {
        let path = self.gl_path.join("types.json");
        if let Ok(content) = serde_json::to_string_pretty(map) {
            let _ = fs::write(path, content);
        }
    }

    pub async fn resolve_name_fallback(&self, appid: &str, cache: &HashMap<String, String>) -> String {
        // Layer 1: Cache
        if let Some(name) = cache.get(appid) {
            if name != "Unknown" && !name.starts_with("AppID") {
                return name.clone();
            }
        }

        // Layer 2: Relationship Parent
        let relationships = self.load_relationships();
        if let Some(parent_id) = relationships.get(appid) {
            if let Some(parent_name) = cache.get(parent_id) {
                return format!("{} (Content)", parent_name);
            }
        }

        // Layer 3: Steam API (Online)
        // We can't easily call the async API client here without passing it down.
        // For now, we skip this layer in this synchronous method, or we rely on the command layer to update cache.
        // The command `update_name_cache` should handle the heavy lifting.

        // Layer 4: VDF / Depot Fallback
        // Check if it matches a known depot
        let depot_path = self.steam_path.join("depotcache");
        let pattern = depot_path.join(format!("{}_*.manifest", appid));
        if glob::glob(&pattern.to_string_lossy()).map(|mut p| p.next().is_some()).unwrap_or(false) {
             return format!("Depot ({})", appid);
        }

        format!("AppID {}", appid)
    }

    pub fn refresh_active_games_list(
        &self,
        cache: &HashMap<String, String>,
        family_godmode_ids: &[String],
        pending_updates: &HashMap<String, (String, u64, u64)>
    ) -> Vec<GameProfile> {
        let mut profiles = Vec::new();
        let al_path = self.gl_path.join("AppList");
        let relationships = self.load_relationships();
        let types_map = self.load_types(); // Load strict types

        if !al_path.exists() {
            return profiles;
        }

        // 0. Pre-load AppList IDs for fast lookup
        // We need to know efficiently if a child/depot is in the AppList
        let mut applist_ids = HashSet::new();
        let pattern = al_path.join("*.txt");
        if let Ok(paths) = glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                 if let Ok(content) = fs::read_to_string(&path) {
                     applist_ids.insert(content.trim().to_string());
                 }
            }
        }

        // 0b. Build Reverse Relationship Map (Parent -> Children)
        let mut parent_to_children: HashMap<String, Vec<String>> = HashMap::new();
        for (child, parent) in &relationships {
            parent_to_children.entry(parent.clone()).or_default().push(child.clone());
        }

        // 1. Scan for Depots (Manifests in Steam)
        let mut depot_ids = HashSet::new();
        let depot_path = self.steam_path.join("depotcache");
        let pattern = depot_path.join("*.manifest");
        
        // NEW: Get depot IDs that have decrypt keys in config.vdf (= truly injected by GreenLuma)
        // let injected_depot_ids = Self::get_injected_depot_ids(&self.steam_path);

        if let Ok(paths) = glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                 if let Some(stem) = path.file_stem() {
                     let stem_str = stem.to_string_lossy().to_string();
                     if let Some(idx) = stem_str.find('_') {
                         depot_ids.insert(stem_str[..idx].to_string());
                     }
                 }
            }
        }

        // 2. Scan AppList
        let pattern = al_path.join("*.txt");
        let mut paths_result = glob(&pattern.to_string_lossy());
        
        // Retry logic for flakey FS operations (User "Disappearing" Bug)
        if paths_result.is_err() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            paths_result = glob(&pattern.to_string_lossy());
        }

        if let Ok(paths) = paths_result {
            let mut entries: Vec<_> = paths.filter_map(|x| x.ok()).collect();

            // Sort by numeric filename (0.txt, 1.txt...)
            entries.sort_by_key(|path| {
                path.file_stem()
                    .and_then(|s| s.to_string_lossy().parse::<u32>().ok())
                    .unwrap_or(9999)
            });

            for path in entries {
                // Robust Read with Retry (Fixes "Disappearing Game" Bug)
                let mut content_result = fs::read_to_string(&path);
                if content_result.is_err() {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    content_result = fs::read_to_string(&path);
                }

                if let Ok(content) = content_result {
                    let app_id = content.trim().to_string();
                    
                    // 5-Layer Resolution strategy (Applied here)
                    let mut name = cache.get(&app_id).cloned().unwrap_or_else(|| "Unknown".to_string());

                    // Layer 2: Parent Relation
                    if name == "Unknown" || name.starts_with("AppID") {
                        if let Some(parent_id) = relationships.get(&app_id) {
                            if let Some(parent_name) = cache.get(parent_id) {
                                name = format!("{} (Content)", parent_name);
                            }
                        }
                    }

                    // Layer 5: Depot Fallback
                    if (name == "Unknown" || name.starts_with("AppID")) && depot_ids.contains(&app_id) {
                        name = format!("Depot ({})", app_id);
                    }

                     // If still unknown, keep as "Unknown" or "AppID X"
                     if name == "Unknown" {
                         name = format!("AppID {}", app_id);
                     }

                    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();

                // Determine item type: v1.7.2 parity — only use relationships map
                let has_parent = relationships.contains_key(&app_id);
                let item_type = if let Some(t) = types_map.get(&app_id) {
                    t.clone()
                } else if has_parent {
                    "dlc".to_string() // default assumption if unknown child
                } else {
                    "game".to_string()
                };

                // Check if installed (ACF exists)
                let is_installed = if let Some(steam_str) = self.steam_path.to_str() {
                    crate::game_path::GamePathFinder::find_manifest_path(steam_str, &app_id).is_some()
                } else {
                    false
                };

                // INJECTION STATUS — THE SIMPLE RULE:
                // If game's base depot (AppID+1) is in AppList → INJECTED
                // If not → FAMILY SHARED
                // family_godmode overrides everything
                
                let mut status = "family_shared".to_string();

                if family_godmode_ids.contains(&app_id) {
                    status = "family_godmode".to_string();
                } else if !has_parent {
                    // ROOT GAME: Check if base depot (AppID+1) is in AppList
                    if let Ok(app_num) = app_id.parse::<u64>() {
                        let base_depot_id = (app_num + 1).to_string();
                        if applist_ids.contains(&base_depot_id) {
                            status = "injected".to_string();
                        }
                    }
                } else {
                    // CHILD (depot/DLC): inherits from parent, mark as injected by default
                    status = "injected".to_string();
                }

                let injection_status = status;

                // Determine Pending Update Status
                let pending_update = pending_updates.get(&app_id).map(|(msg, _, _)| msg.clone());

                profiles.push(GameProfile {
                    parent_id: relationships.get(&app_id).cloned(),
                    app_id,
                    name,
                    filename,
                    item_type,
                    is_installed,
                    injection_status,
                    pending_update,
                });
                }
            }
        }
        profiles
    }




    pub fn nuke_reorder(
        &self,
        target_id_to_remove: Option<&str>,
        cache: Option<&HashMap<String, String>>,
    ) -> Result<(), std::io::Error> {
        let al_path = self.gl_path.join("AppList");
        if !al_path.exists() {
            return Ok(());
        }

        let mut entries = Vec::new();

        // 1. Read all existing IDs
        let pattern = al_path.join("*.txt");
        if let Ok(paths) = glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let aid = content.trim().to_string();

                    // Remove target
                    if let Some(target) = target_id_to_remove {
                        if aid == target {
                            let _ = fs::remove_file(&path);
                            continue;
                        }
                    }
                    entries.push(aid);
                }
                // Delete file to prepare for rewrite
                let _ = fs::remove_file(&path);
            }
        }

        // 2. Sort Logic
        if let Some(game_map) = cache {
            entries.sort_by(|a, b| {
                let name_a = game_map.get(a).map(|s| s.as_str()).unwrap_or("zzz_unknown");
                let name_b = game_map.get(b).map(|s| s.as_str()).unwrap_or("zzz_unknown");

                // Primary: Name
                let name_cmp = name_a.to_lowercase().cmp(&name_b.to_lowercase());
                if name_cmp != std::cmp::Ordering::Equal {
                    return name_cmp;
                }

                // Secondary: ID Length
                let len_cmp = a.len().cmp(&b.len());
                if len_cmp != std::cmp::Ordering::Equal {
                    return len_cmp;
                }

                // Tertiary: ID Value
                a.cmp(b)
            });
        } else {
            // Fallback Numeric
            entries.sort_by(|a, b| {
                let na = a.parse::<u64>().unwrap_or(u64::MAX);
                let nb = b.parse::<u64>().unwrap_or(u64::MAX);
                na.cmp(&nb)
            });
        }

        entries.dedup();

        // 3. Write back
        for (i, aid) in entries.iter().enumerate() {
            let text_path = al_path.join(format!("{}.txt", i));
            fs::write(text_path, aid)?;
        }

        Ok(())
    }

    pub fn add_games_to_list(&self, new_ids: Vec<String>) -> Result<(), std::io::Error> {
        let al_path = self.gl_path.join("AppList");
        if !al_path.exists() {
            fs::create_dir_all(&al_path)?;
        }

        // 1. Read existing and delete files
        let mut current_ids = HashSet::new();
        let pattern = al_path.join("*.txt");
        if let Ok(paths) = glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                if let Ok(content) = fs::read_to_string(&path) {
                    current_ids.insert(content.trim().to_string());
                }
                let _ = fs::remove_file(path);
            }
        }

        // 2. Add new
        for id in new_ids {
            current_ids.insert(id);
        }

        // 3. Sort (Numeric default) and Write
        let mut final_list: Vec<_> = current_ids.into_iter().collect();
        final_list.sort_by(|a, b| {
            let na = a.parse::<u64>().unwrap_or(u64::MAX);
            let nb = b.parse::<u64>().unwrap_or(u64::MAX);
            na.cmp(&nb)
        });

        for (i, aid) in final_list.iter().enumerate() {
            let text_path = al_path.join(format!("{}.txt", i));
            fs::write(text_path, aid)?;
        }
        Ok(())
    }

    /// Remove specific IDs from AppList (v1.7.2 port — needed for bulk delete, godmode disable)
    pub fn remove_games_from_list(&self, ids_to_remove: Vec<String>) -> Result<(), std::io::Error> {
        let al_path = self.gl_path.join("AppList");
        if !al_path.exists() {
            return Ok(());
        }

        let remove_set: HashSet<_> = ids_to_remove.into_iter().collect();
        let mut current_ids = HashSet::new();

        let pattern = al_path.join("*.txt");
        if let Ok(paths) = glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let id = content.trim().to_string();
                    if !remove_set.contains(&id) {
                        current_ids.insert(id);
                    }
                }
                let _ = fs::remove_file(path);
            }
        }

        let mut final_list: Vec<_> = current_ids.into_iter().collect();
        final_list.sort_by(|a, b| {
            let na = a.parse::<u64>().unwrap_or(u64::MAX);
            let nb = b.parse::<u64>().unwrap_or(u64::MAX);
            na.cmp(&nb)
        });

        for (i, aid) in final_list.iter().enumerate() {
            let text_path = al_path.join(format!("{}.txt", i));
            fs::write(text_path, aid)?;
        }
        Ok(())
    }

    /// Overwrite entire AppList with new IDs (used by profile loading)
    pub fn overwrite_app_list(&self, new_ids: Vec<String>) -> Result<(), std::io::Error> {
        let al_path = self.gl_path.join("AppList");
        if !al_path.exists() {
            fs::create_dir_all(&al_path)?;
        }

        // 1. Delete ALL existing
        let pattern = al_path.join("*.txt");
        if let Ok(paths) = glob(&pattern.to_string_lossy()) {
            for path in paths.flatten() {
                let _ = fs::remove_file(path);
            }
        }

        // 2. Write New
        for (i, aid) in new_ids.iter().enumerate() {
            let text_path = al_path.join(format!("{}.txt", i));
            fs::write(text_path, aid)?;
        }
        Ok(())
    }
}

