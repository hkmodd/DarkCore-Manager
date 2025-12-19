use crate::game_path::{GamePathFinder, VdfValue};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn inject_vdf(
    steam_path: &str,
    vdf_keys: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = Path::new(steam_path).join("config").join("config.vdf");
    if !cfg_path.exists() {
        return Ok(());
    }

    // Backup
    let _ = fs::copy(&cfg_path, cfg_path.with_extension("vdf.bak"));

    let content_bytes = fs::read(&cfg_path)?;
    let content = String::from_utf8_lossy(&content_bytes).to_string();

    let mut root = match GamePathFinder::parse_vdf(&content) {
        Some(r) => r,
        None => return Err("Failed to parse config.vdf".into()),
    };

    // Traverse to "depots". config.vdf structure is typically:
    // "InstallConfigStore" -> "Software" -> "Valve" -> "Steam" -> "depots"
    // But sometimes it starts differently? We usually look for "InstallConfigStore".

    let base = if root.has_key("InstallConfigStore") {
        root.get_mut("InstallConfigStore").unwrap()
    } else {
        // Fallback: maybe it's cleaner? Search for keys at top level?
        // If parsing didn't find the root key, we fail safer than corrupting.
        return Err("Invalid config.vdf structure (missing InstallConfigStore)".into());
    };

    if let Some(steam_node) = base.ensure_path(&["Software", "Valve", "Steam"]) {
        if let Some(depots) = steam_node.ensure_path(&["depots"]) {
            for (appid, key) in vdf_keys {
                // Check if APPID block exists
                if !depots.has_key(appid) {
                    // Create new: "AppID" { "DecryptionKey" "abc" }
                    let mut new_obj = Vec::new();
                    new_obj.push(("DecryptionKey".to_string(), VdfValue::Str(key.clone())));
                    depots.insert_or_update(appid.clone(), VdfValue::Obj(new_obj));
                } else {
                    // Exists, update DecryptionKey inside it
                    if let Some(app_node) = depots.get_mut(appid) {
                        if let VdfValue::Obj(fields) = app_node {
                            // Check if DecryptionKey is there
                            let mut found = false;
                            for (k, v) in fields.iter_mut() {
                                if k.eq_ignore_ascii_case("DecryptionKey") {
                                    *v = VdfValue::Str(key.clone());
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                fields.push((
                                    "DecryptionKey".to_string(),
                                    VdfValue::Str(key.clone()),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    let new_content = GamePathFinder::serialize_vdf(&root);
    fs::write(cfg_path, new_content)?;

    Ok(())
}

pub fn parse_lua_for_keys(lua_content: &str) -> (Vec<String>, HashMap<String, String>) {
    let mut ids = Vec::new();
    let mut keys = HashMap::new();

    // Regex: addappid\s*\(\s*(\d+)(?:[^)]*?"([a-fA-F0-9]{64})")?
    let re = Regex::new(r#"addappid\s*\(\s*(\d+)(?:[^)]*?"([a-fA-F0-9]{64})")?"#).unwrap();

    for cap in re.captures_iter(lua_content) {
        if let Some(id_match) = cap.get(1) {
            let id = id_match.as_str().to_string();
            ids.push(id.clone());

            if let Some(key_match) = cap.get(2) {
                keys.insert(id, key_match.as_str().to_string());
            }
        }
    }

    (ids, keys)
}
