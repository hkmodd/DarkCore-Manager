use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
enum VdfValue {
    Str(String),
    Obj(Vec<(String, VdfValue)>), // Preserves order
}

impl VdfValue {
    fn get_mut(&mut self, key: &str) -> Option<&mut VdfValue> {
        if let VdfValue::Obj(entries) = self {
            for (k, v) in entries {
                if k.eq_ignore_ascii_case(key) {
                    return Some(v);
                }
            }
        }
        None
    }

    fn insert_or_update(&mut self, key: String, value: VdfValue) {
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
    fn ensure_path(&mut self, path: &[&str]) -> Option<&mut VdfValue> {
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

    fn has_key(&self, key: &str) -> bool {
        if let VdfValue::Obj(entries) = self {
            entries.iter().any(|(k, _)| k.eq_ignore_ascii_case(key))
        } else {
            false
        }
    }
}

pub struct GamePathFinder;

impl GamePathFinder {
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

    fn get_library_folders(steam_path: &str) -> Vec<PathBuf> {
        let mut folders = Vec::new();
        let main_steam = PathBuf::from(steam_path);
        folders.push(main_steam.clone());

        let vdf_path = main_steam.join("steamapps").join("libraryfolders.vdf");
        if let Ok(content) = fs::read_to_string(vdf_path) {
            let re = Regex::new(r#""path"\s+"(.+?)""#).unwrap();
            for cap in re.captures_iter(&content) {
                if let Some(m) = cap.get(1) {
                    let path_str = m.as_str().replace("\\\\", "\\");
                    let p = PathBuf::from(path_str);
                    if p != main_steam {
                        folders.push(p);
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

    pub fn is_titan_active(steam_path: &str, app_id: &str) -> bool {
        if let Some(path) = Self::find_game_path(steam_path, app_id) {
            return path.join("version.dll").exists();
        }
        false
    }

    pub fn deploy_titan_hook(steam_path: &str, app_id: &str) -> Result<PathBuf, String> {
        let game_path = Self::find_game_path(steam_path, app_id)
            .ok_or_else(|| "Game installation directory not found.".to_string())?;

        let source_dll = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("titan_hook.dll")))
            .unwrap_or_else(|| PathBuf::from("titan_hook.dll"));

        if !source_dll.exists() {
            return Err("Titan Hook DLL source not found. Please rebuild.".to_string());
        }

        let target_dll = game_path.join("version.dll");
        std::fs::copy(&source_dll, &target_dll)
            .map_err(|e| format!("Failed to copy DLL: {}", e))?;

        let appid_txt = game_path.join("steam_appid.txt");
        std::fs::write(&appid_txt, app_id)
            .map_err(|e| format!("Failed to write steam_appid.txt: {}", e))?;

        Ok(game_path)
    }

    pub fn suppress_cloud_sync(steam_path: &str, app_id: &str) -> Result<(), String> {
        let userdata = PathBuf::from(steam_path).join("userdata");
        if !userdata.exists() {
            return Err("Userdata directory not found.".to_string());
        }

        if let Ok(entries) = fs::read_dir(userdata) {
            for entry in entries.flatten() {
                let user_path = entry.path();

                // 1. Patch localconfig.vdf
                let local_config = user_path.join("config").join("localconfig.vdf");
                if local_config.exists() {
                    match Self::patch_localconfig_vdf(&local_config, app_id) {
                        Ok(_) => {
                            println!("Patched localconfig for user {:?}", user_path.file_name())
                        }
                        Err(e) => println!("Failed to patch {:?}: {}", local_config, e),
                    }
                }

                // 2. Delete remotecache.vdf (The "93KB" ghost file)
                // Path: userdata/{User}/ {AppID} / remotecache.vdf
                let app_remotecache = user_path.join(app_id).join("remotecache.vdf");
                if app_remotecache.exists() {
                    match fs::remove_file(&app_remotecache) {
                        Ok(_) => println!("Deleted ghost remotecache: {:?}", app_remotecache),
                        Err(e) => println!("Failed to delete {:?}: {}", app_remotecache, e),
                    }
                }
            }
        }
        Ok(())
    }

    fn patch_localconfig_vdf(path: &PathBuf, app_id: &str) -> Result<(), String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let content = String::from_utf8_lossy(&bytes).to_string();

        let mut root = Self::parse_vdf(&content).ok_or("Failed to parse VDF")?;

        let store = if root.has_key("UserLocalConfigStore") {
            root.get_mut("UserLocalConfigStore").unwrap()
        } else {
            &mut root
        };

        if let Some(apps) = store.ensure_path(&["Software", "Valve", "Steam", "Apps", app_id]) {
            apps.insert_or_update("Cloud".to_string(), VdfValue::Str("0".to_string()));
        } else {
            return Err("Could not navigate to Apps key".to_string());
        }

        let new_content = Self::serialize_vdf(&root);
        fs::write(path, new_content).map_err(|e| e.to_string())?;

        Ok(())
    }

    // --- Minimal VDF Parser ---
    fn parse_vdf(input: &str) -> Option<VdfValue> {
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

    fn serialize_vdf(val: &VdfValue) -> String {
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
                        buf.push_str("\n");
                        buf.push_str(&format!("{}{{\n", indent));
                        Self::serialize_recursive(v, buf, depth + 1);
                        buf.push_str(&format!("{}}}\n", indent));
                    }
                }
            }
        }
    }
}
