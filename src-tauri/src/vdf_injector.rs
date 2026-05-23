use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct VdfInjector {
    steam_path: PathBuf,
}

impl VdfInjector {
    pub fn new(steam_path: &Path) -> Self {
        Self {
            steam_path: steam_path.to_path_buf(),
        }
    }

    pub fn set_paths(&mut self, steam_path: &Path) {
        self.steam_path = steam_path.to_path_buf();
    }

    /// Inject decryption keys into config.vdf using SURGICAL TEXT INSERTION.
    ///
    /// This does NOT parse-then-serialize the whole file (which destroys data).
    /// Instead it:
    /// 1. Finds the "depots" section
    /// 2. For existing depot IDs: replaces the DecryptionKey value in-place
    /// 3. For new depot IDs: inserts a block right after the "depots" opening brace
    /// 4. Writes back the modified text without touching anything else
    pub fn inject_vdf(
        &self,
        vdf_keys: &HashMap<String, String>,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        if vdf_keys.is_empty() {
            return Ok(0);
        }

        let cfg_path = self.steam_path.join("config").join("config.vdf");
        if !cfg_path.exists() {
            return Err(format!("config.vdf not found at {:?}", cfg_path).into());
        }

        // Backup BEFORE any modification
        let _ = fs::copy(&cfg_path, cfg_path.with_extension("vdf.bak"));

        let content = fs::read_to_string(&cfg_path)?;
        let mut modified = content.clone();
        let mut injected = 0;

        // Find the "depots" section opening brace position
        // Pattern: "depots" followed by optional whitespace and {
        let depots_re = Regex::new(r#""depots"\s*\{[\t ]*\r?\n"#)?;

        let depots_match = match depots_re.find(&modified) {
            Some(m) => m,
            None => {
                // No depots section exists — we need to find the Steam section and create one
                // Look for "Steam" section
                let steam_re = Regex::new(r#""Steam"\s*\{[\t ]*\r?\n"#)?;
                if let Some(sm) = steam_re.find(&modified) {
                    let insert_pos = sm.end();
                    let depots_block = "\t\t\t\t\"depots\"\n\t\t\t\t{\n\t\t\t\t}\n";
                    modified.insert_str(insert_pos, depots_block);
                    // Now retry finding depots
                    match depots_re.find(&modified) {
                        Some(m) => m,
                        None => return Err("Failed to create depots section".into()),
                    }
                } else {
                    return Err("Cannot find Steam section in config.vdf".into());
                }
            }
        };

        let _depots_insert_pos = depots_match.end();

        for (depot_id, key) in vdf_keys {
            // Check if this depot ID already exists in the depots section
            let existing_re = Regex::new(&format!(
                r#""{}"\s*\{{\s*\r?\n\s*"DecryptionKey"\s*"[a-fA-F0-9]*""#,
                regex::escape(depot_id)
            ))?;

            if let Some(_existing_match) = existing_re.find(&modified) {
                // Depot exists — replace the DecryptionKey value in-place
                let key_value_re = Regex::new(&format!(
                    r#"("{}"\s*\{{\s*\r?\n\s*"DecryptionKey"\s*")[a-fA-F0-9]*""#,
                    regex::escape(depot_id)
                ))?;

                modified = key_value_re
                    .replace(&modified, |caps: &regex::Captures| {
                        format!("{}{}\"", &caps[1], key)
                    })
                    .to_string();
            } else {
                // Depot doesn't exist — insert a new block right after "depots" {
                // Use the same indentation as existing depot entries
                let new_block = format!(
                    "\t\t\t\t\t\"{}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"DecryptionKey\"\t\t\"{}\"\n\t\t\t\t\t}}\n",
                    depot_id, key
                );

                // Find current depots position again (it may have shifted from previous inserts)
                if let Some(dm) = depots_re.find(&modified) {
                    modified.insert_str(dm.end(), &new_block);
                }
            }

            injected += 1;
        }

        // Only write if we actually changed something
        if modified != content {
            fs::write(&cfg_path, &modified)?;
        }

        Ok(injected)
    }

    /// Remove decryption keys for the given depot IDs from config.vdf.
    /// Uses surgical text removal — never re-serializes the whole file.
    pub fn remove_vdf_keys(
        &self,
        depot_ids: &[String],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let cfg_path = self.steam_path.join("config").join("config.vdf");
        if !cfg_path.exists() {
            return Ok(0);
        }

        let _ = fs::copy(&cfg_path, cfg_path.with_extension("vdf.bak"));

        let mut content = fs::read_to_string(&cfg_path)?;
        let mut removed = 0;

        for depot_id in depot_ids {
            // Match the entire depot block: "DEPOTID" { "DecryptionKey" "..." }
            // Account for varying whitespace and indentation
            let block_re = Regex::new(&format!(
                r#"[ \t]*"{}"\s*\{{\s*\r?\n\s*"DecryptionKey"\s*"[a-fA-F0-9]*"\s*\r?\n\s*\}}\s*\r?\n"#,
                regex::escape(depot_id)
            ))?;

            if block_re.is_match(&content) {
                content = block_re.replace(&content, "").to_string();
                removed += 1;
            }
        }

        if removed > 0 {
            fs::write(&cfg_path, &content)?;
        }

        Ok(removed)
    }
}

/// Parse LUA file for AppIDs and Depot Keys
///
/// Returns:
/// - `applist_ids`: IDs that should go in GreenLuma AppList
///   - All IDs WITHOUT keys (these are AppIDs for game/DLCs)
///   - The FIRST ID WITH a key (essential base depot for download)
/// - `keys`: ALL depot decryption keys (for config.vdf injection)
pub fn parse_lua_for_keys(lua_content: &str) -> (Vec<String>, HashMap<String, String>) {
    let mut applist_ids = Vec::new(); // IDs for GreenLuma AppList
    let mut keys = HashMap::new(); // ALL keys for config.vdf

    // Primary Regex: addappid(depot_id, flag, "key") - 3 argument format (SMD Compatible)
    let re_3arg =
        Regex::new(r#"addappid\s*\(\s*(\d+)\s*,\s*\d+\s*,\s*["']([a-fA-F0-9]{64})["']"#).unwrap();

    // Fallback Regex: addappid(depot_id, "key") - 2 argument format
    let re_2arg = Regex::new(r#"addappid\s*\(\s*(\d+)\s*,\s*["']([a-fA-F0-9]{64})["']"#).unwrap();

    // Simple ID-only Regex: addappid(ID) - ONLY the ID with closing paren
    let re_id_only = Regex::new(r#"addappid\s*\(\s*(\d+)\s*\)"#).unwrap();

    // Process LINE BY LINE to respect Lua comments
    for line in lua_content.lines() {
        let trimmed = line.trim();

        // Skip commented lines (Lua comment: --)
        if trimmed.starts_with("--") {
            continue;
        }

        // First: Try 3-arg match (has decryption key = depot)
        if let Some(cap) = re_3arg.captures(trimmed) {
            if let (Some(id_match), Some(key_match)) = (cap.get(1), cap.get(2)) {
                let id = id_match.as_str().to_string();
                let key = key_match.as_str().to_string();

                // Add key to config.vdf (always)
                keys.insert(id.clone(), key);

                // Add ALL depots to AppList (User requested all depot IDs be present)
                if !applist_ids.contains(&id) {
                    applist_ids.push(id);
                }

                continue;
            }
        }

        // Second: Try 2-arg match (fallback format with key)
        if let Some(cap) = re_2arg.captures(trimmed) {
            if let (Some(id_match), Some(key_match)) = (cap.get(1), cap.get(2)) {
                let id = id_match.as_str().to_string();
                let key = key_match.as_str().to_string();

                if !keys.contains_key(&id) {
                    keys.insert(id.clone(), key);
                }

                // Add ALL depots to AppList
                if !applist_ids.contains(&id) {
                    applist_ids.push(id);
                }

                continue;
            }
        }

        // Third: ID-only = AppID or DLC (no key = ALWAYS add to AppList)
        if let Some(cap) = re_id_only.captures(trimmed) {
            if let Some(id_match) = cap.get(1) {
                let id = id_match.as_str().to_string();
                if !applist_ids.contains(&id) {
                    applist_ids.push(id);
                }
            }
        }
    }

    (applist_ids, keys)
}
