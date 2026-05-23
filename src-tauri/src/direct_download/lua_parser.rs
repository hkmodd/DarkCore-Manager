use flate2::read::ZlibDecoder;
use regex::Regex;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum DepotCategory {
    Unknown,
    MainApp,
    MainDepot,
    SharedDepot,
    DlcDepot,
}

/// Represents a downloadable Depot
#[derive(Debug, Clone)]
pub struct DepotInfo {
    pub depot_id: u32,
    pub depot_key: String,
    pub manifest_id: Option<u64>,
    pub name: Option<String>,
    pub category: DepotCategory,
}

#[derive(Debug, Clone)]
pub struct DlcInfo {
    pub app_id: u32,
    pub name: String,
}

/// Data extracted from Morrenus .lua/.st files
#[derive(Debug, Default, Clone)]
pub struct ScriptData {
    pub app_id: Option<u32>,
    pub app_name: Option<String>,
    pub depots: Vec<DepotInfo>,
    pub dlcs: Vec<DlcInfo>,

    // Legacy mapping (kept for internal use/merging)
    pub depot_keys: HashMap<u32, String>,
    pub manifests: HashMap<u32, u64>,
}

impl ScriptData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_primary_depot(&self) -> Option<&DepotInfo> {
        self.depots.first()
    }

    pub fn get_downloadable_depots(&self) -> Vec<&DepotInfo> {
        self.depots
            .iter()
            .filter(|d| d.manifest_id.is_some())
            .collect()
    }

    pub fn merge(&mut self, other: ScriptData) {
        if self.app_id.is_none() {
            self.app_id = other.app_id;
        }
        if self.app_name.is_none() {
            self.app_name = other.app_name;
        }

        for depot in other.depots {
            if !self.depots.iter().any(|d| d.depot_id == depot.depot_id) {
                self.depots.push(depot);
            }
        }

        for dlc in other.dlcs {
            if !self.dlcs.iter().any(|d| d.app_id == dlc.app_id) {
                self.dlcs.push(dlc);
            }
        }

        self.depot_keys.extend(other.depot_keys);
        self.manifests.extend(other.manifests);
    }
}

pub fn parse_file(path: &Path) -> Result<ScriptData, Box<dyn Error>> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if extension == "st" {
        parse_st(path)
    } else {
        parse_lua(path)
    }
}

fn parse_lua(path: &Path) -> Result<ScriptData, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    parse_content(&content)
}

fn parse_st(path: &Path) -> Result<ScriptData, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    if buffer.len() < 12 {
        return Err("ST file too short".into());
    }

    let xor_key_raw = u32::from_le_bytes(buffer[0..4].try_into()?);
    let size = u32::from_le_bytes(buffer[4..8].try_into()?) as usize;
    let xor_byte = ((xor_key_raw ^ 0xFFFEA4C8) & 0xFF) as u8;

    if buffer.len() < 12 + size {
        return Err("ST file truncated or size mismatch".into());
    }

    let mut payload = buffer[12..12 + size].to_vec();
    for byte in payload.iter_mut() {
        *byte ^= xor_byte;
    }

    let mut decoder = ZlibDecoder::new(Cursor::new(payload));
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;

    if decompressed.len() <= 512 {
        return Err("Decompressed ST data too short".into());
    }

    // Skip first 512 bytes (garbage header in older formats? or padding?)
    // Reference implementation skips 512.
    let content_str = String::from_utf8_lossy(&decompressed[512..]).to_string();
    parse_content(&content_str)
}

pub fn parse_content(content: &str) -> Result<ScriptData, Box<dyn Error>> {
    let mut data = ScriptData::new();
    let lines: Vec<&str> = content.lines().collect();

    // Regex compilation
    let re_addappid_with_key =
        Regex::new(r#"addappid\(\s*(\d+)\s*,\s*\d+\s*,\s*"([0-9a-fA-F]+)"\s*\)"#)?;

    let re_addappid_only = Regex::new(r#"addappid\(\s*(\d+)\s*\)"#)?;

    let re_setmanifestid =
        Regex::new(r#"setManifestid\(\s*(\d+)\s*,\s*"(\d+)"\s*(?:,\s*\d+\s*)?\)"#)?;

    // Step 1: Extract App Name
    if lines.len() >= 2 {
        let second_line = lines[1].trim();
        if second_line.starts_with("--") {
            let name = second_line.trim_start_matches('-').trim();
            if !name.is_empty()
                && !name.contains("Lua and Manifest")
                && !name.contains("Created:")
                && !name.contains("Website:")
                && !name.contains("Total Depots")
                && !name.contains("Total DLCs")
            {
                data.app_name = Some(name.to_string());
            }
        }
    }

    // Step 2: Parse Lines
    // Section mapping now uses the public enum
    let mut current_section = DepotCategory::Unknown;

    // Helper to map section (because we can't store DepotCategory with associated data in hashmap easily
    // without more complex logic, we'll store section in a parallel map or just use current loop state)
    // Actually we need to store the category per depot ID.
    let mut temp_depot_cats: HashMap<u32, DepotCategory> = HashMap::new();

    let mut temp_keys: HashMap<u32, (String, Option<String>)> = HashMap::new();
    let mut manifest_ids: HashMap<u32, u64> = HashMap::new();
    let mut main_app_id: Option<u32> = None;

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.contains("MAIN APPLICATION") {
            current_section = DepotCategory::MainApp;
            continue;
        } else if trimmed.contains("MAIN APP DEPOTS") || trimmed.contains("APP DEPOTS") {
            current_section = DepotCategory::MainDepot;
            continue;
        } else if trimmed.contains("DLCS") && trimmed.starts_with("--") {
            current_section = DepotCategory::DlcDepot;
            continue;
        } else if trimmed.contains("SHARED DEPOTS") {
            current_section = DepotCategory::SharedDepot; // CRITICAL FIX: Detect shared depots
            continue;
        }

        if trimmed.starts_with("--") && !trimmed.contains("addappid") {
            continue;
        }

        let comment_name = if let Some(idx) = trimmed.find("--") {
            let comment = trimmed[idx + 2..].trim();
            if !comment.is_empty() {
                Some(comment.to_string())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(cap) = re_addappid_with_key.captures(trimmed) {
            if let (Some(id_match), Some(key_match)) = (cap.get(1), cap.get(2)) {
                let id = id_match.as_str().parse::<u32>()?;
                let key = key_match.as_str().to_string();

                if current_section == DepotCategory::MainApp && main_app_id.is_none() {
                    main_app_id = Some(id);
                }

                temp_keys.insert(id, (key.clone(), comment_name.clone()));
                temp_depot_cats.insert(id, current_section.clone());
                data.depot_keys.insert(id, key);
            }
        } else if let Some(cap) = re_addappid_only.captures(trimmed) {
            if let Some(id_match) = cap.get(1) {
                let id = id_match.as_str().parse::<u32>()?;
                // Check if already present in dlcs or main app
                if !data.dlcs.iter().any(|d| d.app_id == id) && Some(id) != main_app_id {
                    let dlc_name = comment_name
                        .clone()
                        .unwrap_or_else(|| format!("DLC {}", id));
                    data.dlcs.push(DlcInfo {
                        app_id: id,
                        name: dlc_name,
                    });
                }
            }
        }

        if let Some(cap) = re_setmanifestid.captures(trimmed) {
            if let (Some(depot_match), Some(manifest_match)) = (cap.get(1), cap.get(2)) {
                let depot_id = depot_match.as_str().parse::<u32>()?;
                let manifest_id = manifest_match.as_str().parse::<u64>()?;
                manifest_ids.insert(depot_id, manifest_id);
                data.manifests.insert(depot_id, manifest_id);
            }
        }
    }

    // Step 3: Build Depots
    for (depot_id, manifest_id) in &manifest_ids {
        if let Some((key, name)) = temp_keys.get(depot_id) {
            let cat = temp_depot_cats
                .get(depot_id)
                .cloned()
                .unwrap_or(DepotCategory::Unknown);

            data.depots.push(DepotInfo {
                depot_id: *depot_id,
                depot_key: key.clone(),
                manifest_id: Some(*manifest_id),
                name: name.clone(),
                category: cat,
            });
        }
    }

    data.depots.sort_by_key(|d| d.depot_id);

    // Step 4: Finalize App ID
    data.app_id = main_app_id;
    if data.app_id.is_none() {
        for (id, _) in &temp_keys {
            if !manifest_ids.contains_key(id) {
                data.app_id = Some(*id);
                break;
            }
        }
    }

    // Header Fallback
    if data.app_id.is_none() && !lines.is_empty() {
        let first_line = lines[0];
        if let Ok(re_header_id) = Regex::new(r"--\s*(\d+)'s") {
            if let Some(cap) = re_header_id.captures(first_line) {
                if let Some(id_match) = cap.get(1) {
                    data.app_id = Some(id_match.as_str().parse::<u32>()?);
                }
            }
        }
    }

    // Name Fallback
    if data.app_name.is_none() {
        if let Some(app_id) = data.app_id {
            if let Some((_, name)) = temp_keys.get(&app_id) {
                if name.is_some() {
                    data.app_name = name.clone();
                }
            }
        }
    }

    Ok(data)
}
