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

    // Read file (lossy to avoid encoding issues similar to Python's errors='ignore')
    let content_bytes = fs::read(&cfg_path)?;
    let mut content = String::from_utf8_lossy(&content_bytes).to_string();

    // Ensure "depots" block exists
    if !content.contains("\"depots\"") {
        let re_steam = Regex::new(r#"("Steam"\s*\{)"#).unwrap();
        content = re_steam
            .replace(&content, "$1\n\t\t\"depots\"\n\t\t{\n\t\t}")
            .to_string();
    }

    let re_depots = Regex::new(r#""depots"\s*\{"#).unwrap();

    if let Some(m) = re_depots.find(&content) {
        let insert_pos = m.end();
        let mut block_to_insert = String::new();

        for (appid, key) in vdf_keys {
            let app_check = format!("\"{}\"", appid);
            if !content.contains(&app_check) {
                block_to_insert.push_str(&format!(
                    "\n\t\t\t\"{}\"\n\t\t\t{{\n\t\t\t\t\"DecryptionKey\"\t\t\"{}\"\n\t\t\t}}",
                    appid, key
                ));
            }
        }

        if !block_to_insert.is_empty() {
            content.insert_str(insert_pos, &block_to_insert);
            fs::write(cfg_path, content)?;
        }
    }

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
