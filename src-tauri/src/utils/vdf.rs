use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
// VdfValue Enums and Impls
pub enum VdfValue {
    Str(String),
    Obj(Vec<(String, VdfValue)>),
}

impl VdfValue {
    pub fn get_str(&self) -> Option<&str> {
        if let VdfValue::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn get_obj(&self) -> Option<&Vec<(String, VdfValue)>> {
        if let VdfValue::Obj(entries) = self {
            Some(entries)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut VdfValue> {
        if let VdfValue::Obj(entries) = self {
            for (k, v) in entries {
                if k.eq_ignore_ascii_case(key) {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn has_key(&self, key: &str) -> bool {
        if let VdfValue::Obj(entries) = self {
            entries.iter().any(|(k, _)| k.eq_ignore_ascii_case(key))
        } else {
            false
        }
    }

    pub fn insert_or_update(&mut self, key: String, value: VdfValue) {
        if let VdfValue::Obj(entries) = self {
            for (k, v) in entries.iter_mut() {
                if k.eq_ignore_ascii_case(&key) {
                    *v = value;
                    return;
                }
            }
            entries.push((key, value));
        }
    }

    pub fn ensure_path(&mut self, path: &[&str]) -> Option<&mut VdfValue> {
        let mut current = self;
        for &segment in path {
            if !current.has_key(segment) {
                current.insert_or_update(segment.to_string(), VdfValue::Obj(Vec::new()));
            }
            match current.get_mut(segment) {
                Some(next) => current = next,
                None => return None,
            }
        }
        Some(current)
    }

    pub fn find_key(&self, key: &str) -> Option<&VdfValue> {
        if let VdfValue::Obj(entries) = self {
            for (k, v) in entries {
                if k.eq_ignore_ascii_case(key) {
                    return Some(v);
                }
            }
        }
        None
    }
}

pub struct VdfParser;

impl VdfParser {
    pub fn parse(input: &str) -> Option<VdfValue> {
        let mut tokens = VecDeque::new();
        let mut chars = input.chars().peekable();
        while let Some(c) = chars.next() {
            if c.is_whitespace() {
                continue;
            }
            match c {
                '{' | '}' => tokens.push_back(c.to_string()),
                '"' => {
                    let mut s = String::new();
                    while let Some(&next) = chars.peek() {
                        if next == '"' {
                            chars.next();
                            break;
                        }
                        if next == '\\' {
                            chars.next();
                            if let Some(escaped) = chars.next() {
                                s.push(escaped);
                            }
                        } else {
                            s.push(chars.next().unwrap());
                        }
                    }
                    tokens.push_back(s);
                }
                _ => {
                    let mut s = c.to_string();
                    while let Some(&next) = chars.peek() {
                        if next.is_whitespace() || next == '{' || next == '}' || next == '"' {
                            break;
                        }
                        s.push(chars.next().unwrap());
                    }
                    tokens.push_back(s);
                }
            }
        }
        Self::parse_obj(&mut tokens)
    }

    fn parse_obj(tokens: &mut VecDeque<String>) -> Option<VdfValue> {
        let mut entries = Vec::new();

        while let Some(key) = tokens.pop_front() {
            if key == "}" {
                return Some(VdfValue::Obj(entries));
            }

            if let Some(val_token) = tokens.pop_front() {
                if val_token == "{" {
                    if let Some(nested) = Self::parse_obj(tokens) {
                        entries.push((key, nested));
                    }
                } else {
                    entries.push((key, VdfValue::Str(val_token)));
                }
            }
        }
        Some(VdfValue::Obj(entries))
    }

    pub fn serialize(root: &VdfValue) -> String {
        let mut buf = String::new();
        if let VdfValue::Obj(entries) = root {
            for (k, v) in entries {
                Self::write_entry(&mut buf, k, v, 0);
            }
        }
        buf
    }

    fn write_entry(buf: &mut String, key: &str, val: &VdfValue, depth: usize) {
        let indent = "\t".repeat(depth);
        match val {
            VdfValue::Str(s) => {
                buf.push_str(&format!("{}\"{}\"\t\t\"{}\"\n", indent, key, s));
            }
            VdfValue::Obj(children) => {
                buf.push_str(&format!("{}\"{}\"\n", indent, key));
                buf.push_str(&format!("{}{{\n", indent));
                for (k, v) in children {
                    Self::write_entry(buf, k, v, depth + 1);
                }
                buf.push_str(&format!("{}}}\n", indent));
            }
        }
    }
}

/// [LEGACY PORT] GamePathFinder::get_library_folders
/// Scans libraryfolders.vdf
pub fn get_all_library_folders(steam_path: &str) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let main_steam = PathBuf::from(steam_path);
    folders.push(main_steam.clone());

    let vdf_path = main_steam.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(vdf_path) {
        if let Some(parsed) = VdfParser::parse(&content) {
            let root = if let Some(lf) = parsed.find_key("libraryfolders") {
                lf
            } else {
                &parsed
            };

            if let Some(entries) = root.get_obj() {
                for (_, data) in entries {
                    if let Some(path_val) = data.find_key("path") {
                        if let Some(s) = path_val.get_str() {
                            // Clean logic
                            let mut cleaned = s.replace("\\\\", "\\");
                            if cleaned.starts_with(r"\\?\") {
                                cleaned = cleaned[4..].to_string();
                            }
                            let p = PathBuf::from(&cleaned);
                            if p != main_steam {
                                folders.push(p);
                            }
                        }
                    }
                }
            }
        }
    }
    folders
}

/// [LEGACY PORT] Inject decryption keys into config.vdf
pub fn inject_keys_into_config(
    steam_path: &Path,
    keys: &std::collections::HashMap<u32, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = steam_path.join("config").join("config.vdf");
    if !cfg_path.exists() {
        return Ok(());
    }

    // Backup
    let _ = fs::copy(&cfg_path, cfg_path.with_extension("vdf.bak"));

    let content_bytes = fs::read(&cfg_path)?;
    // Use lossy utf8
    let content = String::from_utf8_lossy(&content_bytes).to_string();

    let mut root = match VdfParser::parse(&content) {
        Some(r) => r,
        None => return Err("Failed to parse config.vdf".into()),
    };

    // Locate "InstallConfigStore" -> "Software" -> "Valve" -> "Steam" -> "depots"
    let base = if let Some(ics) = root.get_mut("InstallConfigStore") {
        ics
    } else {
        // Fallback: search top level?
        return Err("Invalid config.vdf structure (missing InstallConfigStore)".into());
    };

    if let Some(steam_node) = base.ensure_path(&["Software", "Valve", "Steam"]) {
        if let Some(depots) = steam_node.ensure_path(&["depots"]) {
            for (appid, key) in keys {
                let appid_str = appid.to_string();

                // Check if APPID block exists
                if !depots.has_key(&appid_str) {
                    // Create new: "AppID" { "DecryptionKey" "abc" }
                    let mut new_obj = Vec::new();
                    new_obj.push(("DecryptionKey".to_string(), VdfValue::Str(key.clone())));
                    depots.insert_or_update(appid_str.clone(), VdfValue::Obj(new_obj));
                } else {
                    // Exists, update DecryptionKey inside it
                    if let Some(app_node) = depots.get_mut(&appid_str) {
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

    let new_content = VdfParser::serialize(&root);
    fs::write(cfg_path, new_content)?;

    Ok(())
}
