use glob::glob;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct GameProfile {
    pub app_id: String,
    pub name: String,
    pub filename: String,
}

pub fn refresh_active_games_list(
    gl_path: &str,
    steam_path: &str,
    cache: &std::collections::HashMap<String, String>,
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
    _steam_path: &str, // Kept unused to match signature
    target_id_to_remove: Option<&str>,
) -> Result<(), std::io::Error> {
    let al_path = Path::new(gl_path).join("AppList");
    if !al_path.exists() {
        return Ok(());
    }

    // Safety: We do NOT auto-clean depots anymore (user reported it breaks updates).
    // We only reorder existing files 0..N and remove specific targets if requested.

    let mut existing_ids = Vec::new();

    // Read all existing IDs
    let pattern = al_path.join("*.txt");
    let pattern_str = pattern.to_string_lossy();

    if let Ok(paths) = glob(&pattern_str) {
        for path in paths.flatten() {
            if let Ok(content) = fs::read_to_string(&path) {
                let aid = content.trim().to_string();

                // Only remove if specifically targeted
                if let Some(target) = target_id_to_remove {
                    if aid == target {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                }

                existing_ids.push(aid);
            }
            // Delete old file to prepare for re-write
            let _ = fs::remove_file(&path);
        }
    }

    // Sort IDs
    existing_ids.sort_by(|a, b| {
        let na = a.parse::<u64>().unwrap_or(u64::MAX);
        let nb = b.parse::<u64>().unwrap_or(u64::MAX);
        if na != u64::MAX && nb != u64::MAX {
            na.cmp(&nb)
        } else {
            a.cmp(b)
        }
    });
    existing_ids.dedup();

    // Write back
    for (i, aid) in existing_ids.iter().enumerate() {
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
