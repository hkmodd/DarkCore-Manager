use reqwest;
use serde_json::Value;
use std::error::Error;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SteamCmdInfo {
    pub buildid: Option<u64>,
    pub depots: HashMap<String, DepotInfo>,
    pub dlcs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DepotInfo {
    pub gid: Option<String>,
    pub language: Option<String>,
}

pub struct SteamApiClient {
    client: reqwest::Client,
}

impl SteamApiClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// Fetch public info from api.steamcmd.net
    pub async fn get_app_info(&self, appid: &str) -> Result<SteamCmdInfo, Box<dyn Error + Send + Sync>> {
        let url = format!("https://api.steamcmd.net/v1/info/{}", appid);
        let resp = self.client.get(&url).send().await?;
        
        if !resp.status().is_success() {
             return Err(format!("SteamCMD API Error {}", resp.status()).into());
        }

        let root: Value = resp.json().await?;
        
        // Navigate: data -> {appid}
        let data = root.get("data").and_then(|d| d.get(appid));
        if data.is_none() { 
            return Err("AppID not found in response".into()); 
        }
        let data = data.unwrap();

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

        // Depots GIDs
        let mut depots_map = HashMap::new();
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
                     
                     // Extract language config
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

        // DLCs
        let mut dlc_list = Vec::new();
        if let Some(extended) = data.get("extended") {
            if let Some(list_str) = extended.get("listofdlc").and_then(|v| v.as_str()) {
                 dlc_list = list_str.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }

        Ok(SteamCmdInfo { buildid, depots: depots_map, dlcs: dlc_list })
    }

    /// Download Manifest ZIP from Morrenus Repository
    pub async fn download_manifest_zip(&self, appid: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let url = format!("https://manifest.morrenus.xyz/api/v1/manifest/{}", appid);
        println!("[SteamApi] Downloading Manifest ZIP from: {}", url);
        
        let resp = self.client.get(&url).send().await?;
        
        if !resp.status().is_success() {
             return Err(format!("Morrenus API Error: {}", resp.status()).into());
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }
}
