use glob::glob;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GameProfile {
    pub app_id: String,
    pub name: String,
    pub filename: String,
    #[allow(dead_code)]
    pub parent_id: Option<String>,
}

pub type RelationshipMap = std::collections::HashMap<String, String>; // Child -> Parent

pub fn load_relationships(data_dir: &str) -> RelationshipMap {
    let path = Path::new(data_dir).join("relationships.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(map) = serde_json::from_str(&content) {
                return map;
            }
        }
    }
    RelationshipMap::new()
}

pub fn save_relationships(data_dir: &str, map: &RelationshipMap) {
    let path = Path::new(data_dir).join("relationships.json");
    if let Ok(content) = serde_json::to_string_pretty(map) {
        let _ = fs::write(path, content);
    }
}

pub fn refresh_active_games_list(
    gl_path: &str,
    steam_path: &str,
    cache: &std::collections::HashMap<String, String>,
    relationships: &RelationshipMap,
) -> Vec<GameProfile> {
    let mut profiles = Vec::new();
    let al_path = Path::new(gl_path).join("AppList");

    if !al_path.exists() {
        return profiles;
    }

    // 1. Scan for Depots
    let mut depot_ids = HashSet::new();
    let depot_path = Path::new(steam_path).join("depotcache");
    let pattern = depot_path.join("*.manifest");
    if let Ok(paths) = glob(&pattern.to_string_lossy()) {
        for path in paths.flatten() {
            if let Some(stem) = path.file_stem() {
                depot_ids.insert(stem.to_string_lossy().to_string());
            }
        }
    }

    // Using glob to find text files
    let pattern = al_path.join("*.txt");
    let pattern_str = pattern.to_string_lossy();

    if let Ok(paths) = glob(&pattern_str) {
        let mut entries: Vec<_> = paths.filter_map(|x| x.ok()).collect();

        // Sort by numeric filename explicitly (0.txt, 1.txt...)
        entries.sort_by(|a, b| {
            let a_stem = a
                .file_stem()
                .and_then(|s| s.to_string_lossy().parse::<u32>().ok())
                .unwrap_or(9999);
            let b_stem = b
                .file_stem()
                .and_then(|s| s.to_string_lossy().parse::<u32>().ok())
                .unwrap_or(9999);
            a_stem.cmp(&b_stem)
        });

        for path in entries {
            if let Ok(content) = fs::read_to_string(&path) {
                let app_id = content.trim().to_string();
                let mut name = cache
                    .get(&app_id)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string());

                // Fix Unknown Label if it's a Depot
                if name == "Unknown" && depot_ids.contains(&app_id) {
                    name = format!("Depot ({})", app_id);
                }

                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                profiles.push(GameProfile {
                    parent_id: relationships.get(&app_id).cloned(),
                    app_id,
                    name,
                    filename,
                });
            }
        }
    }
    profiles
}

pub fn nuke_reorder(
    gl_path: &str,
    _steam_path: &str,
    target_id_to_remove: Option<&str>,
    cache: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), std::io::Error> {
    let al_path = Path::new(gl_path).join("AppList");
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
            // Delete file
            let _ = fs::remove_file(&path);
        }
    }

    // 2. Sort Logic
    // If cache is provided, sort by Name. Else, sort by ID numeric.
    if let Some(game_map) = cache {
        entries.sort_by(|a, b| {
            let name_a = game_map.get(a).map(|s| s.as_str()).unwrap_or("zzz_unknown");
            let name_b = game_map.get(b).map(|s| s.as_str()).unwrap_or("zzz_unknown");

            // Primary: Name
            let name_cmp = name_a.to_lowercase().cmp(&name_b.to_lowercase());
            if name_cmp != std::cmp::Ordering::Equal {
                return name_cmp;
            }

            // Secondary: ID Length (Main game usually shorter ID than DLC, sometimes)
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

pub fn add_games_to_list(gl_path: &str, new_ids: Vec<String>) -> Result<(), std::io::Error> {
    let al_path = Path::new(gl_path).join("AppList");
    if !al_path.exists() {
        fs::create_dir_all(&al_path)?;
    }

    // 1. Read existing
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

    // 3. Sort and Write
    let mut final_list: Vec<_> = current_ids.into_iter().collect();
    // Sort logic to match others if needed, but simple sort is fine for now
    final_list.sort();

    for (i, aid) in final_list.iter().enumerate() {
        let text_path = al_path.join(format!("{}.txt", i));
        fs::write(text_path, aid)?;
    }
    Ok(())
}

pub fn overwrite_app_list(gl_path: &str, new_ids: Vec<String>) -> Result<(), std::io::Error> {
    let al_path = Path::new(gl_path).join("AppList");
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

pub fn nuke_unknowns(
    gl_path: &str,
    cache: &std::collections::HashMap<String, String>,
    relationships: &RelationshipMap,
) -> Result<usize, std::io::Error> {
    let al_path = Path::new(gl_path).join("AppList");
    if !al_path.exists() {
        return Ok(0);
    }

    let mut entries = Vec::new();
    let mut nuked_count = 0;

    // 1. Read Valid IDs
    let pattern = al_path.join("*.txt");
    if let Ok(paths) = glob(&pattern.to_string_lossy()) {
        for path in paths.flatten() {
            if let Ok(content) = fs::read_to_string(&path) {
                let aid = content.trim().to_string();

                // INTELLIGENT FILTER
                let name = cache.get(&aid).map(|s| s.as_str()).unwrap_or("Unknown");
                let is_unknown = name == "Unknown" || name.starts_with("Depot (");
                let is_linked_dlc = relationships.contains_key(&aid);

                // RULE: If it's Unknown/Depot AND NOT LINKED -> NUKE IT
                if is_unknown && !is_linked_dlc {
                    nuked_count += 1;
                    let _ = fs::remove_file(&path);
                    continue;
                }

                entries.push(aid);
            }
            let _ = fs::remove_file(&path);
        }
    }

    // 2. Re-sort (Alphabetical)
    entries.sort_by(|a, b| {
        let name_a = cache.get(a).map(|s| s.as_str()).unwrap_or("zzz_unknown");
        let name_b = cache.get(b).map(|s| s.as_str()).unwrap_or("zzz_unknown");
        let name_cmp = name_a.to_lowercase().cmp(&name_b.to_lowercase());
        if name_cmp != std::cmp::Ordering::Equal {
            return name_cmp;
        }
        a.cmp(b)
    });

    entries.dedup();

    // 3. Write Back
    for (i, aid) in entries.iter().enumerate() {
        let text_path = al_path.join(format!("{}.txt", i));
        fs::write(text_path, aid)?;
    }

    Ok(nuked_count)
}
