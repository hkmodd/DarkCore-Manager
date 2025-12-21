use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use serde_json::Value;
use std::error::Error;

#[derive(Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub game_id: Option<Value>,
    pub game_name: Option<String>,
    pub app_id: Option<Value>,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CoveredGame {
    pub id: String,
    pub name: String,
    pub cover_url: String,
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
    api_key: String,
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

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error>> {
        // 1. Try Morrenus API (if key exists)
        if !self.api_key.is_empty() {
             let url = "https://manifest.morrenus.xyz/api/v1/search";
             // Use query params for proper encoding
             if let Ok(resp) = self.client.get(url)
                .query(&[("q", query)])
                .send().await 
             {
                 if let Ok(text) = resp.text().await {
                     // Try parsing as list
                     if let Ok(list) = serde_json::from_str::<Vec<SearchResult>>(&text) {
                         return Ok(list);
                     }
                     // Try as object
                     #[derive(Deserialize)]
                     struct Wrapper { results: Vec<SearchResult> }
                     if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&text) {
                         return Ok(wrapper.results);
                     }
                 }
             }
        }

        // 2. Fallback: Steam Store Search (Public)
        let fallback_url = "https://store.steampowered.com/api/storesearch/";
        let params = [
            ("term", query),
            ("l", "english"),
            ("cc", "US")
        ];
        
        let resp = self.client.get(fallback_url)
            .query(&params)
            .send().await?;
            
        let root: Value = resp.json().await?;
        
        let mut results = Vec::new();
        if let Some(items) = root.get("items").and_then(|v| v.as_array()) {
            for item in items {
                let id_val = item.get("id").cloned();
                let name_str = item.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                
                results.push(SearchResult {
                    game_id: id_val.clone(), 
                    game_name: name_str.clone(), 
                    app_id: id_val,
                    name: name_str,
                });
            }
        }
        
        Ok(results)
    }

    pub async fn download_manifest(&self, appid: &str) -> Result<Bytes, Box<dyn Error>> {
        let url = format!("https://manifest.morrenus.xyz/api/v1/manifest/{}", appid);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err("Download failed".into());
        }
        let bytes = resp.bytes().await?;
        Ok(bytes)
    }

    pub async fn get_dlc_list(&self, appid: &str) -> Result<Vec<String>, Box<dyn Error>> {
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

    pub async fn get_status(&self, appid: &str) -> Result<GameStatus, Box<dyn Error>> {
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

    pub async fn get_user_stats(&self) -> Result<UserStats, Box<dyn Error>> {
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
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct GameStatus {
    pub app_id: String,
    pub status: String,
    pub needs_update: Option<bool>,
    pub file_modified: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct UserStats {
    pub api_key_usage_count: i32,
    pub daily_usage: i32,
    pub daily_limit: i32,
    pub can_make_requests: bool,
    pub role: Option<String>,
}
