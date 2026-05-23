#![allow(dead_code)] // Reserved: dlcs, game_name fields for future features
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SearchResult {
    pub game_id: Option<Value>,
    pub game_name: Option<String>,
    pub app_id: Option<Value>,
    pub name: Option<String>,
    #[serde(default)]
    pub is_free: bool,
    // FIX v1.7.3: Cover URL from search used as fallback
    #[serde(default)]
    pub tiny_image: Option<String>, 
    #[serde(default)]
    pub logo: Option<String>,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct GameHierarchy {
    pub root_id: String,
    pub root_name: String,
    pub base_depots: Vec<DepotNode>,
    pub dlcs: Vec<DlcNode>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DlcNode {
    pub app_id: String,
    pub name: String,
    pub depots: Vec<DepotNode>,
    pub selected: bool, // Default selection state
}

#[derive(Serialize, Debug, Clone)]
pub struct DepotNode {
    pub depot_id: String,
    pub gid: Option<String>,
    pub size: Option<u64>,  // max_size in bytes
    pub language: Option<String>,
    pub is_common: bool,    // true if no language config
    pub is_dlc_depot: bool, // true if triggered by a DLC
}
#[allow(dead_code)]
pub struct CoveredGame {
    pub id: String,
    pub name: String,
    pub cover_url: String,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct GameDetails {
    pub app_id: String,
    pub name: String,
    pub short_description: String,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub genres: Vec<String>,
    pub release_date: String,
    pub metacritic_score: Option<u32>,
    pub recommendations: Option<u64>,
    pub platforms: (bool, bool, bool), // (windows, mac, linux)
    pub required_age: u8,
}

pub fn val_to_string(v: &Option<Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => "".to_string(),
    }
}

#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    pub api_key: String,
}

impl ApiClient {
    pub fn new(api_key: String) -> Self {
        let mut headers = HeaderMap::new();
        // Docs recommend Authorization: Bearer for production
        let auth_value = format!("Bearer {}", api_key);
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_value).unwrap_or(HeaderValue::from_static("")),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .danger_accept_invalid_certs(true) 
            .timeout(std::time::Duration::from_secs(25))
            .build()
            .unwrap_or_default();

        Self { client, api_key }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
        // 1. Primary: Steam Store Search (Free & Public)
        // This prevents token usage for "Unknown" game resolution on startup.
        let fallback_url = "https://store.steampowered.com/api/storesearch/";
        let params = [
            ("term", query),
            ("l", "english"),
            ("cc", "US")
        ];
        
        let mut results = Vec::new();

        // Try Steam Store first (free)
        if let Ok(resp) = self.client.get(fallback_url)
            .query(&params)
            .send().await 
        {
            if let Ok(root) = resp.json::<Value>().await {
                if let Some(items) = root.get("items").and_then(|v| v.as_array()) {
                    for item in items {
                        let id_val = item.get("id").cloned();
                        let name_str = item.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                        
                        // Price Parsing Loophole:
                        // If "price" object is MISSING -> Free (e.g. CS2)
                        // If "price" object exists but "final" is 0 -> Free
                        let mut is_free = true;
                        
                        if let Some(price_obj) = item.get("price") {
                            if let Some(final_price) = price_obj.get("final").and_then(|v| v.as_i64()) {
                                if final_price > 0 {
                                    is_free = false;
                                }
                            }
                        }

                        let tiny_image = item.get("tiny_image").and_then(|v| v.as_str()).map(|s| s.to_string());

                        results.push(SearchResult {
                            game_id: id_val.clone(), 
                            game_name: name_str.clone(), 
                            app_id: id_val,
                            name: name_str,
                            is_free, 
                            tiny_image,
                            logo: None,
                        });
                    }
                }
            }
        }

        if !results.is_empty() {
             return Ok(results);
        }

        // 2. Fallback: Morrenus API (Paid)
        // Only hits this if Steam Store returns nothing (e.g. Unlisted games)
        if !self.api_key.is_empty() {
             let url = "https://manifest.morrenus.xyz/api/v1/search";
             if let Ok(resp) = self.client.get(url)
                .query(&[("q", query)])
                .send().await 
             {
                 if let Ok(text) = resp.text().await {
                     if let Ok(list) = serde_json::from_str::<Vec<SearchResult>>(&text) {
                         return Ok(list);
                     }
                     #[derive(Deserialize)]
                     struct Wrapper { results: Vec<SearchResult> }
                     if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&text) {
                         return Ok(wrapper.results);
                     }
                 }
             }
        }
        
        Ok(results) // Return empty if both failed
    }

    pub async fn download_manifest(&self, appid: &str) -> Result<Bytes, Box<dyn Error + Send + Sync>> {
        // 1. Get Metadata (Solus: ManifestApiService.GetManifestAsync)
        let meta_url = format!("https://manifest.morrenus.xyz/api/v1/manifest/{}?api_key={}", appid, self.api_key);
        let resp = self.client.get(&meta_url).send().await?;
        
        if !resp.status().is_success() {
            return Err(format!("Manifest Metadata Error: {}", resp.status()).into());
        }

        // Check Content-Type to see if we got JSON Metadata or Direct Binary
        let content_type = resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        // If it's NOT JSON, assume it's the file itself (Direct Download)
        // User reports Morrenus API might just return the ZIP directly.
        if !content_type.contains("json") {
             // CONSUME RESP HERE - Return Bytes immediately
             let bytes = resp.bytes().await?;
             return Ok(bytes);
        }

        // Parse JSON to get download_url
        #[derive(Deserialize)]
        struct ManifestMeta {
            download_url: String,
        }
        
        // CONSUME RESP HERE - Read Text for JSON parsing
        let text = resp.text().await?;
        let meta: ManifestMeta = match serde_json::from_str(&text) {
             Ok(m) => m,
             Err(_) => {
                 // Fallback: If JSON parsing fails, maybe it IS the zip but with wrong header?
                 // But we already consumed text...
                 // Let's assume error for now, but logged.
                 return Err(format!("API Error: Failed to parse JSON. Raw: '{}'", text).into());
             }
        };
        
        // 2. Download File (Solus: DownloadService.cs)
        // Ensure we append API key if not present (Solus does append it).
        let download_url = if meta.download_url.contains('?') {
            format!("{}&api_key={}", meta.download_url, self.api_key)
        } else {
            format!("{}?api_key={}", meta.download_url, self.api_key)
        };

        let file_resp = self.client.get(&download_url).send().await?;
        if !file_resp.status().is_success() {
             return Err(format!("Manifest Download Error: {}", file_resp.status()).into());
        }
        
        let bytes = file_resp.bytes().await?;
        Ok(bytes)
    }

    /// Generic download helper
    pub async fn download_file(&self, url: &str) -> Result<Bytes, Box<dyn Error + Send + Sync>> {
        let resp = self.client.get(url).send().await?;
        if !resp.status().is_success() {
             return Err(format!("Download Error: {}", resp.status()).into());
        }
        let bytes = resp.bytes().await?;
        Ok(bytes)
    }

    pub async fn get_dlc_list(&self, appid: &str) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://store.steampowered.com/api/appdetails?appids={}&filters=dlc",
            appid
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let root: Value = resp.json().await?;

        let mut dlc_ids = Vec::new();
        if let Some(app_data) = root.get(appid) {
            if let Some(true) = app_data.get("success").and_then(|v| v.as_bool()) {
                if let Some(data) = app_data.get("data") {
                    if let Some(dlc_array) = data.get("dlc").and_then(|v| v.as_array()) {
                        for item in dlc_array {
                            if let Some(id) = item.as_u64() {
                                dlc_ids.push(id.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(dlc_ids)
    }

    pub async fn get_details_parent(&self, appid: &str) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://store.steampowered.com/api/appdetails?appids={}&filters=basic,fullgame",
            appid
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }

        let root: Value = resp.json().await?;
        if let Some(app_data) = root.get(appid) {
             if let Some(true) = app_data.get("success").and_then(|v| v.as_bool()) {
                 if let Some(data) = app_data.get("data") {
                     // Check if type is DLC
                     if let Some("dlc") = data.get("type").and_then(|s| s.as_str()) {
                         if let Some(fullgame) = data.get("fullgame") {
                             if let Some(pid) = fullgame.get("appid").and_then(|v| v.as_str()) {
                                 return Ok(Some(pid.to_string())); 
                             }
                         }
                     }
                 }
             }
        }
        Ok(None)
    }

    pub async fn get_status(&self, appid: &str) -> Result<GameStatus, Box<dyn Error + Send + Sync>> {
        if self.api_key.is_empty() {
             return Err("No API Key".into());
        }
        let url = format!("https://manifest.morrenus.xyz/api/v1/status/{}", appid);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
             return Err(format!("API Error {}", resp.status()).into());
        }
        let status: GameStatus = resp.json().await?;
        Ok(status)
    }

    pub async fn get_user_stats(&self) -> Result<UserStats, Box<dyn Error + Send + Sync>> {
        if self.api_key.is_empty() { return Err("No API Key Provided".into()); }
        
        let url = "https://manifest.morrenus.xyz/api/v1/user/stats";
        // No query param needed if header is set
        let resp = self.client.get(url).send().await?;
        
        if !resp.status().is_success() { 
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            // Try to parse {"detail": "..."}
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                if let Some(detail) = json.get("detail").and_then(|v| v.as_str()) {
                    return Err(format!("API Error {}: {}", status.as_u16(), detail).into());
                }
            }
            return Err(format!("API Error {}: {}", status, text).into()); 
        }
        
        let stats: UserStats = resp.json().await?;
        Ok(stats)
    }

    /// Fetch detailed game info from Steam Store API (FREE - no Morrenus token used)
    pub async fn get_game_details(&self, appid: &str) -> Result<GameDetails, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://store.steampowered.com/api/appdetails?appids={}&l=english",
            appid
        );
        
        let resp = self.client.get(&url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        
        let data = json
            .get(appid)
            .and_then(|v| v.get("data"))
            .ok_or("No data found")?;
        
        Ok(GameDetails {
            app_id: appid.to_string(),
            name: data.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            short_description: data.get("short_description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            developers: data.get("developers")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            publishers: data.get("publishers")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            genres: data.get("genres")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter()
                    .filter_map(|v| v.get("description").and_then(|d| d.as_str()).map(String::from))
                    .collect())
                .unwrap_or_default(),
            release_date: data.get("release_date")
                .and_then(|v| v.get("date"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            metacritic_score: data.get("metacritic")
                .and_then(|v| v.get("score"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            recommendations: data.get("recommendations")
                .and_then(|v| v.get("total"))
                .and_then(|v| v.as_u64()),
            platforms: (
                data.get("platforms").and_then(|v| v.get("windows")).and_then(|v| v.as_bool()).unwrap_or(false),
                data.get("platforms").and_then(|v| v.get("mac")).and_then(|v| v.as_bool()).unwrap_or(false),
                data.get("platforms").and_then(|v| v.get("linux")).and_then(|v| v.as_bool()).unwrap_or(false),
            ),
            required_age: data.get("required_age")
                .and_then(|v| v.as_u64())
                .map(|v| v as u8)
                .unwrap_or(0),
        })
    }

    // =========================================================================
    // VAULT AUDIT v1.7.2 - MANIFEST GID EXTRACTION
    // =========================================================================

    /// Fetches current manifest GIDs from Morrenus for an appid.
    /// Returns HashMap<depot_id, manifest_gid> for version verification.
    /// 
    /// NOTE: This downloads the full ZIP which costs 1 API token.
    /// Use sparingly and cache the result for reinstallations.
    pub async fn get_manifest_gids(
        &self,
        appid: &str,
    ) -> Result<std::collections::HashMap<String, String>, Box<dyn Error + Send + Sync>> {
        // Download ZIP (unfortunately full download is required)
        let bytes = self.download_manifest(appid).await
            .map_err(|e| -> Box<dyn Error + Send + Sync> { e.to_string().into() })?;
        Self::extract_gids_from_zip(&bytes)
    }

    /// Extracts manifest GIDs from already-downloaded ZIP bytes.
    /// Used when ZIP is already available (e.g., from cache or direct download).
    pub fn extract_gids_from_zip(
        bytes: &[u8],
    ) -> Result<std::collections::HashMap<String, String>, Box<dyn Error + Send + Sync>> {
        let mut gids = std::collections::HashMap::new();

        if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
            for i in 0..archive.len() {
                if let Ok(f) = archive.by_index(i) {
                    let name = f.name();
                    if name.ends_with(".manifest") {
                        // Filename format: depotcache/{depot_id}_{gid}.manifest
                        // or: {depot_id}_{gid}.manifest
                        if let Some(stem) = std::path::Path::new(name).file_stem() {
                            let stem_str = stem.to_string_lossy();
                            if let Some(idx) = stem_str.rfind('_') {
                                let depot_id = &stem_str[..idx];
                                let gid = &stem_str[idx + 1..];
                                gids.insert(depot_id.to_string(), gid.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(gids)
    }

    /// Lightweight GID check using SteamCMD API (FREE, no token cost).
    /// Returns HashMap<depot_id, manifest_gid> from public Steam API.
    /// 
    /// Prefer this over get_manifest_gids() when possible.
    pub async fn get_public_gids(
        &self,
        appid: &str,
    ) -> Result<std::collections::HashMap<String, String>, Box<dyn Error + Send + Sync>> {
        let info = self.get_app_info(appid).await
            .map_err(|e| -> Box<dyn Error + Send + Sync> { e.to_string().into() })?;
        let mut gids = std::collections::HashMap::new();
        for (depot_id, depot_info) in info.depots {
            if let Some(gid) = depot_info.gid {
                gids.insert(depot_id, gid);
            }
        }
        Ok(gids)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[allow(dead_code)]
pub struct GameStatus {
    pub app_id: String,
    pub status: String,
    pub needs_update: Option<bool>,
    pub file_modified: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[allow(dead_code)]
pub struct UserStats {
    pub api_key_usage_count: i32,
    pub daily_usage: i32,
    pub daily_limit: i32,
    pub can_make_requests: bool,
    pub role: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct SteamCmdInfo {
    pub name: Option<String>,
    pub buildid: Option<u64>,
    pub depots: std::collections::HashMap<String, DepotInfo>,
    pub dlcs: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DepotInfo {
    pub gid: Option<String>,
    pub language: Option<String>,  // None = common/all, Some("italian") = language-specific
}

impl ApiClient {
    /// Fetch public info from api.steamcmd.net
    pub async fn get_app_info(&self, appid: &str) -> Result<SteamCmdInfo, Box<dyn Error + Send + Sync>> {
        let url = format!("https://api.steamcmd.net/v1/info/{}", appid);
        let resp = self.client.get(&url).send().await?;
        
        if !resp.status().is_success() {
             return Err(format!("SteamCMD API Error {}", resp.status()).into());
        }

        let root: Value = resp.json().await?;
        
        // Navigate: data -> {appid} -> depots -> branches -> public -> buildid
        let data = root.get("data").and_then(|d| d.get(appid));
        if data.is_none() { return Err("AppID not found in response".into()); }
        let data = data.unwrap();

        // Extract common.name (works for games, DLCs, depots — unlike Store API)
        let name = data.get("common")
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut buildid = None;
        if let Some(depots) = data.get("depots") {
            if let Some(branches) = depots.get("branches") {
                if let Some(public) = branches.get("public") {
                    if let Some(bid) = public.get("buildid").and_then(|v| v.as_str()) {
                         buildid = bid.parse::<u64>().ok();
                    }
                }
            }
        }

        // Depots GIDs (data -> {appid} -> depots -> {depot_id} -> manifests -> public -> gid)
        // Also extract language config for filtering
        let mut depots_map = std::collections::HashMap::new();
        if let Some(depots) = data.get("depots").and_then(|d| d.as_object()) {
            for (key, val) in depots {
                 if let Ok(_) = key.parse::<u64>() { // key is a depot id number
                     let mut gid = None;
                     let mut language = None;
                     
                     // Extract manifest GID
                     if let Some(manifests) = val.get("manifests") {
                         if let Some(public) = manifests.get("public") {
                             if let Some(g) = public.get("gid").and_then(|v| v.as_str()) {
                                 gid = Some(g.to_string());
                             }
                         }
                     }
                     
                     // Extract language config (depot -> config -> language)
                     if let Some(config) = val.get("config") {
                         if let Some(lang) = config.get("language").and_then(|v| v.as_str()) {
                             language = Some(lang.to_lowercase());
                         }
                     }
                     
                     if gid.is_some() {
                         depots_map.insert(key.clone(), DepotInfo { gid, language });
                     }
                 }
            }
        }

        // DLCs (data -> {appid} -> extended -> listofdlc)
        let mut dlc_list = Vec::new();
        if let Some(extended) = data.get("extended") {
            if let Some(list_str) = extended.get("listofdlc").and_then(|v| v.as_str()) {
                 dlc_list = list_str.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }

        Ok(SteamCmdInfo { name, buildid, depots: depots_map, dlcs: dlc_list })
    }

    /// Lightweight name lookup via SteamCMD API.
    /// Works for ALL types: games, DLCs, depots.
    /// Unlike Steam Store API (appdetails), this never fails for depots.
    pub async fn get_app_name(&self, appid: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let info = self.get_app_info(appid).await?;
        info.name.ok_or_else(|| format!("No name found for {}", appid).into())
    }
}
#[derive(Deserialize, Debug, Clone)]
pub struct SchemaResponse {
    pub game: GameSchema,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GameSchema {
    #[serde(rename = "gameName")]
    pub game_name: Option<String>,
    #[serde(rename = "availableGameStats")]
    pub available_game_stats: Option<AvailableGameStats>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AvailableGameStats {
    pub achievements: Option<Vec<SchemaAchievement>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SchemaAchievement {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub hidden: Option<i32>,
    pub description: Option<String>,
    pub icon: Option<String>,
    #[serde(rename = "icongray")]
    pub icon_gray: Option<String>,
}

impl ApiClient {
    pub async fn get_schema_for_game(&self, appid: &str) -> Result<SchemaResponse, Box<dyn Error + Send + Sync>> {
        if self.api_key.is_empty() {
             return Err("No Web API Key provided".into());
        }
        
        let url = format!(
            "https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/?key={}&appid={}",
            self.api_key, appid
        );

        let resp = self.client.get(&url).send().await?;
        
        if !resp.status().is_success() {
             let status = resp.status();
             // Try to read error body
             let text = resp.text().await.unwrap_or_default();
             return Err(format!("Steam Web API Error {}: {}", status, text).into());
        }

        let schema: SchemaResponse = resp.json().await?;
        Ok(schema)
    }

    /// Fetch list of ALL available Depot IDs from Morrenus (Low bandwidth, huge cache value).
    /// Used to verify which DLCs are actually playable/downloadable.
    pub async fn get_available_depots(&self) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        if self.api_key.is_empty() {
             return Err("No Morrenus API Key provided".into());
        }

        let url = "https://manifest.morrenus.xyz/api/v1/depot-keys";
        let resp = self.client.get(url).send().await?;
        
        if !resp.status().is_success() {
             return Err(format!("Morrenus API Error {}: Failed to fetch depot keys", resp.status()).into());
        }

        #[derive(Deserialize)]
        struct DepotKeysResponse {
            depot_ids: Vec<String>,
        }

        let data: DepotKeysResponse = resp.json().await?;
        Ok(data.depot_ids)
    }
}

/// Filter depots by language priority:
/// 1. Core/Common depots (no language config) → ALWAYS KEEP
/// 2. Target language depots → KEEP (PRIORITY)
/// 3. English depots → KEEP ONLY IF target language variant does NOT exist
/// 4. Other languages → DROP
pub fn filter_depots_by_language(
    depots: &std::collections::HashMap<String, DepotInfo>,
    target_language: &str,
) -> std::collections::HashMap<String, DepotInfo> {
    let target_lang = target_language.to_lowercase();
    
    // First pass: identify what languages exist for each depot "type"
    let has_target_lang = depots.values().any(|d| {
        d.language.as_ref().map(|l| l == &target_lang).unwrap_or(false)
    });
    
    let mut filtered = std::collections::HashMap::new();
    
    for (depot_id, depot_info) in depots {
        let should_include = match &depot_info.language {
            // Core/Common depot (no language) → ALWAYS KEEP
            None => true,
            
            // Target language → KEEP (PRIORITY)
            Some(lang) if lang == &target_lang => true,
            
            // English → KEEP ONLY IF target language variant does NOT exist
            Some(lang) if lang == "english" => !has_target_lang,
            
            // Other languages → DROP
            Some(_) => false,
        };
        
        if should_include {
            filtered.insert(depot_id.clone(), depot_info.clone());
        }
    }
    
    filtered
}

impl ApiClient {
    /// RECURSIVE HIERARCHY FETCH "THE MANIFESTOR"
    /// Fetches Base Game + All DLCs + All Depots
    pub async fn fetch_full_hierarchy(&self, appid: &str, target_lang: &str) -> Result<GameHierarchy, Box<dyn Error + Send + Sync>> {
        let lang_lower = target_lang.to_lowercase();
        
        // 1. Fetch Root Info
        let root_info = self.get_app_info_raw(appid).await?;
        
        let root_name = root_info.get_name().unwrap_or_else(|| format!("App {}", appid));
        let root_depots = root_info.extract_depots(&lang_lower);
        
        // 2. Identify DLCs
        let dlc_ids = root_info.extract_dlc_ids();
        
        // 3. Parallel Fetch of DLCs (Batched/Semaphores could be added if needed)
        // For now, we spawn tasks for each DLC to speed it up
        let mut dlc_nodes = Vec::new();
        
        if !dlc_ids.is_empty() {
            // Limited concurrency could be better, but let's try strict join for now
            let mut handles = Vec::new();
            
            for dlc_id in dlc_ids {
                let client = self.clone();
                let lang = lang_lower.clone();
                // Spawn a tokio task
                handles.push(tokio::spawn(async move {
                    match client.get_app_info_raw(&dlc_id).await {
                        Ok(info) => {
                            let name = info.get_name().unwrap_or_else(|| format!("DLC {}", dlc_id));
                            let depots = info.extract_depots(&lang);
                            
                            // Smart Pre-selection Logic
                            let name_lower = name.to_lowercase();
                            let is_media = name_lower.contains("soundtrack") || name_lower.contains(" ost") || name_lower.contains("artbook") || name_lower.contains("score");
                            let is_beta = name_lower.contains("beta") || name_lower.contains("test") || name_lower.contains("server");
                            let selected = !is_media && !is_beta;

                            Some(DlcNode {
                                app_id: dlc_id,
                                name,
                                depots,
                                selected,
                            })
                        },
                        Err(_) => {
                        // Fallback: Try fetching name from Steam Store API (GET_DETAILS)
                        // This handles cases where SteamCMD has no public info (e.g. License-only DLCs)
                        if let Ok(details) = client.get_game_details(&dlc_id).await {
                             Some(DlcNode {
                                app_id: dlc_id,
                                name: details.name,
                                depots: Vec::new(),
                                selected: true,
                            })
                        } else {
                            // Ultimate Fallback: Add as Generic Node so it appears in the list
                            Some(DlcNode {
                                app_id: dlc_id.clone(),
                                name: format!("DLC {} (Info Unavailable)", dlc_id),
                                depots: Vec::new(),
                                selected: true,
                            })
                        }
                    },
                    }
                }));
            }
            
            // Await all
            for handle in handles {
                if let Ok(Some(node)) = handle.await {
                    dlc_nodes.push(node);
                }
            }
        }
        
        // Sort DLCs by ID for consistency
        dlc_nodes.sort_by(|a, b| a.app_id.cmp(&b.app_id));

        Ok(GameHierarchy {
            root_id: appid.to_string(),
            root_name,
            base_depots: root_depots,
            dlcs: dlc_nodes,
        })
    }
    
    // Helper to get raw Value from SteamCMD API
    async fn get_app_info_raw(&self, appid: &str) -> Result<RawSteamInfo, Box<dyn Error + Send + Sync>> {
        let url = format!("https://api.steamcmd.net/v1/info/{}", appid);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(format!("Error fetching {}: {}", appid, resp.status()).into());
        }
        let root: Value = resp.json().await?;
        Ok(RawSteamInfo { appid: appid.to_string(), data: root })
    }
}

// Helper wrapper for parsing JSON
struct RawSteamInfo {
    appid: String,
    data: Value,
}

impl RawSteamInfo {
    fn get_name(&self) -> Option<String> {
        self.data.get("data")
            .and_then(|d| d.get(&self.appid))
            .and_then(|a| a.get("common"))
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
    
    fn extract_dlc_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();

        // 1. Standard Method: extended -> listofdlc
        if let Some(extended) = self.data.get("data")
            .and_then(|d| d.get(&self.appid))
            .and_then(|a| a.get("extended")) 
        {
            if let Some(list) = extended.get("listofdlc").and_then(|v| v.as_str()) {
                ids = list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
        }

        // 2. "Solus" Method: Reverse Lookup via Depots (dlcappid)
        // Scan all depots to see if they claim to belong to a DLC
        if let Some(depots) = self.data.get("data")
            .and_then(|d| d.get(&self.appid))
            .and_then(|a| a.get("depots"))
            .and_then(|d| d.as_object()) 
        {
            for val in depots.values() {
                if let Some(dlc_id) = val.get("dlcappid").and_then(|v| v.as_str()) {
                    let dlc_str = dlc_id.to_string();
                    if !ids.contains(&dlc_str) {
                         ids.push(dlc_str);
                    }
                }
            }
        }
        
        // Sort for consistency
        ids.sort();
        ids
    }
    
    pub fn extract_depots(&self, target_lang: &str) -> Vec<DepotNode> {
        let mut nodes = Vec::new();
        if let Some(depots) = self.data.get("data").and_then(|d| d.get(&self.appid)).and_then(|a| a.get("depots")).and_then(|d| d.as_object()) {
            for (key, val) in depots {
                if let Ok(_) = key.parse::<u64>() {
                    // 1. OS Filtering (Windows Only)
                    // If config.oslist exists, it MUST contain "windows".
                    // If missing, assume all platforms.
                    if let Some(config) = val.get("config") {
                        if let Some(oslist) = config.get("oslist").and_then(|v| v.as_str()) {
                            if !oslist.contains("windows") {
                                continue; // Skip non-windows depots
                            }
                        }
                    }

                    // 2. DLC Depot Detection
                    // If depot has "dlcappid", it belongs to a DLC.
                    let is_dlc_depot = val.get("dlcappid").is_some();

                    // 3. Language Filtering
                    let mut language = None;
                    if let Some(config) = val.get("config") {
                        if let Some(l) = config.get("language").and_then(|v| v.as_str()) {
                            language = Some(l.to_lowercase());
                        }
                    }
                    
                    let is_common = language.is_none();
                    let is_target = language.as_ref().map(|l| l == target_lang).unwrap_or(false);
                    
                    if is_common || is_target {
                        // Extract GID
                        let mut gid = None;
                        
                        // Try "public" branch first
                        if let Some(manifests) = val.get("manifests").and_then(|m| m.get("public")) {
                             // GID can be string or number (stupid JSON generic)
                             if let Some(g) = manifests.get("gid").and_then(|v| {
                                 if let Some(s) = v.as_str() { Some(s.to_string()) }
                                 else if let Some(n) = v.as_u64() { Some(n.to_string()) }
                                 else { None }
                             }) {
                                 gid = Some(g);
                             }
                        }
                        
                        // Extract Size
                         let size = val.get("maxsize").and_then(|v| {
                             if let Some(s_val) = v.as_str() {
                                 s_val.parse::<u64>().ok()
                             } else if let Some(f_val) = v.as_f64() {
                                 Some(f_val as u64)
                             } else if let Some(u_val) = v.as_u64() {
                                 Some(u_val)
                             } else {
                                 None
                             }
                         });

                         nodes.push(DepotNode {
                             depot_id: key.clone(),
                             gid,
                             size,
                             language,
                             is_common,
                             is_dlc_depot,
                         });
                    }
                }
            }
        }
        nodes
    }
}
