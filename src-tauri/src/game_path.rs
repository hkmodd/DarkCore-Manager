#![allow(clippy::collapsible_match)]

use regex::Regex;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum VdfValue {
    Str(String),
    Obj(Vec<(String, VdfValue)>), // Preserves order
}

impl VdfValue {
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

    pub fn insert_or_update(&mut self, key: String, value: VdfValue) {
        if let VdfValue::Obj(entries) = self {
            for (k, v) in entries.iter_mut() {
                if k.eq_ignore_ascii_case(&key) {
                    *v = value;
                    return;
                }
            }
            // Not found, append
            entries.push((key, value));
        }
    }

    // Helper to ensure path exists and get mutable ref to it
    pub fn ensure_path(&mut self, path: &[&str]) -> Option<&mut VdfValue> {
        if path.is_empty() {
            return Some(self);
        }

        let mut current = self;
        for &key in path {
            if !current.has_key(key) {
                current.insert_or_update(key.to_string(), VdfValue::Obj(Vec::new()));
            }
            current = current.get_mut(key).unwrap();
        }
        Some(current)
    }

    pub fn has_key(&self, key: &str) -> bool {
        if let VdfValue::Obj(entries) = self {
            entries.iter().any(|(k, _)| k.eq_ignore_ascii_case(key))
        } else {
            false
        }
    }
}

pub struct GamePathFinder;

impl GamePathFinder {
    #[allow(dead_code)] // Utility function kept for future use
    pub fn find_manifest_path(steam_path: &str, app_id: &str) -> Option<PathBuf> {
        let library_folders = Self::get_library_folders(steam_path);
        for lib in library_folders {
            let manifest_path = lib
                .join("steamapps")
                .join(format!("appmanifest_{}.acf", app_id));
            if manifest_path.exists() {
                return Some(manifest_path);
            }
        }
        None
    }

    pub fn find_game_path(steam_path: &str, app_id: &str) -> Option<PathBuf> {
        let library_folders = Self::get_library_folders(steam_path);
        for lib in library_folders {
            let manifest_path = lib
                .join("steamapps")
                .join(format!("appmanifest_{}.acf", app_id));
            if manifest_path.exists() {
                if let Ok(content) = fs::read_to_string(&manifest_path) {
                    if let Some(install_dir) = Self::extract_install_dir(&content) {
                        let full_path = lib.join("steamapps").join("common").join(install_dir);
                        if full_path.exists() {
                            return Some(full_path);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn get_library_folders(steam_path: &str) -> Vec<PathBuf> {
        let mut folders = Vec::new();
        let main_steam = PathBuf::from(steam_path);
        folders.push(main_steam.clone());

        let vdf_path = main_steam.join("steamapps").join("libraryfolders.vdf");
        if let Ok(content) = fs::read_to_string(vdf_path) {
            if let Some(parsed) = Self::parse_vdf(&content) {
                // Navigate to "libraryfolders"
                // Navigate to "libraryfolders"
                let root = if let VdfValue::Obj(entries) = parsed {
                    // Check if "libraryfolders" exists at root level
                    if let Some((_, v)) = entries
                        .clone()
                        .into_iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("libraryfolders"))
                    {
                        Some(v)
                    } else {
                        // Fallback: Assume the whole file is the content of libraryfolders (legacy/weird format)
                        // But we consumed entries. Reconstruct or just use the clone?
                        // Better: logic.
                        Some(VdfValue::Obj(entries))
                    }
                } else {
                    Some(parsed)
                };

                if let Some(VdfValue::Obj(libs)) = root {
                    for (_, data) in libs {
                        if let VdfValue::Obj(props) = data {
                            for (key, val) in props {
                                if key.eq_ignore_ascii_case("path") {
                                    if let VdfValue::Str(s) = val {
                                        // Clean the path string
                                        let mut cleaned = s.replace("\\\\", "\\");
                                        // FIX 3: REMOVE \\?\ prefix if present (don't skip!)
                                        if cleaned.starts_with(r"\\?\") {
                                            cleaned = cleaned[4..].to_string(); // Remove first 4 chars: \\?\
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
            }
        }
        folders
    }

    fn extract_install_dir(manifest_content: &str) -> Option<String> {
        let re = Regex::new(r#""installdir"\s+"(.+?)""#).unwrap();
        if let Some(cap) = re.captures(manifest_content) {
            return cap.get(1).map(|m| m.as_str().to_string());
        }
        None
    }

    pub fn find_parent_for_depot(steam_path: &str, depot_id: &str) -> Option<String> {
        let config_path = PathBuf::from(steam_path).join("config").join("config.vdf");
        if let Ok(content) = fs::read_to_string(config_path) {
            if let Some(mut root) = Self::parse_vdf(&content) {
                // Traverse to Apps
                let mut current = &mut root;
                let path = ["InstallConfigStore", "Software", "Valve", "Steam", "Apps"];

                for &key in &path {
                    if let Some(next) = current.get_mut(key) {
                        current = next;
                    } else {
                        return None;
                    }
                }

                // Now iterate all Apps
                if let VdfValue::Obj(apps) = current {
                    for (app_id, data) in apps {
                        if let VdfValue::Obj(fields) = data {
                            for (k, v) in fields {
                                if k.eq_ignore_ascii_case("depots") && v.has_key(depot_id) {
                                    return Some(app_id.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn find_parent_by_scanning_manifests(steam_path: &str, depot_id: &str) -> Option<String> {
        let lib_folders = Self::get_library_folders(steam_path);

        for lib in lib_folders {
            let apps_dir = lib.join("steamapps");
            if let Ok(paths) = fs::read_dir(apps_dir) {
                for entry in paths.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "acf") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if !content.contains(depot_id) {
                                continue;
                            }

                            if let Some(root) = Self::parse_vdf(&content) {
                                if let VdfValue::Obj(entries) = root {
                                    for (_, v) in entries {
                                        if let VdfValue::Obj(fields) = v {
                                            let mut current_appid = None;
                                            let mut found_depot = false;

                                            for (key, val) in fields {
                                                if key.eq_ignore_ascii_case("appid") {
                                                    if let VdfValue::Str(s) = &val {
                                                        current_appid = Some(s.clone());
                                                    }
                                                }
                                                if key.eq_ignore_ascii_case("MountedDepots") {
                                                    if let VdfValue::Obj(depots) = &val {
                                                        if depots
                                                            .iter()
                                                            .any(|(d_id, _)| d_id == depot_id)
                                                        {
                                                            found_depot = true;
                                                        }
                                                    }
                                                }
                                            }

                                            if found_depot {
                                                return current_appid;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    // --- Minimal VDF Parser Public ---
    pub fn parse_vdf(input: &str) -> Option<VdfValue> {
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

    pub fn serialize_vdf(val: &VdfValue) -> String {
        let mut buf = String::new();
        Self::serialize_recursive(val, &mut buf, 0);
        buf
    }

    fn serialize_recursive(val: &VdfValue, buf: &mut String, depth: usize) {
        let indent = "\t".repeat(depth);
        if let VdfValue::Obj(entries) = val {
            for (k, v) in entries {
                buf.push_str(&format!("{}\"{}\"", indent, k));
                match v {
                    VdfValue::Str(s) => {
                        buf.push_str(&format!("\t\t\"{}\"\n", s));
                    }
                    VdfValue::Obj(_) => {
                        buf.push('\n');
                        buf.push_str(&format!("{}{{\n", indent));
                        Self::serialize_recursive(v, buf, depth + 1);
                        buf.push_str(&format!("{}}}\n", indent));
                    }
                }
            }
        }
    }
}
