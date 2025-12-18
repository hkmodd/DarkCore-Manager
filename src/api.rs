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
        headers.insert(
            "X-API-Key",
            HeaderValue::from_str(&api_key).unwrap_or(HeaderValue::from_static("")),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("DarkCore/10.4-Rust"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .danger_accept_invalid_certs(true) // Matches verify=False in Python
            .timeout(std::time::Duration::from_secs(25))
            .build()
            .unwrap_or_default();

        Self { client, api_key }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>, Box<dyn Error>> {
        let url = format!("https://manifest.morrenus.xyz/api/v1/search?q={}", query);
        let resp = self.client.get(&url).send().await?;

        // Handle both list and object w/ results
        let text = resp.text().await?;
        // Try parsing as list first
        if let Ok(list) = serde_json::from_str::<Vec<SearchResult>>(&text) {
            return Ok(list);
        }
        // Try as object
        #[derive(Deserialize)]
        struct Wrapper {
            results: Vec<SearchResult>,
        }
        if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&text) {
            return Ok(wrapper.results);
        }

        Ok(vec![])
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

        // Parse: {"12345": {"success": true, "data": {"dlc": [1, 2, 3]}}}
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
}
