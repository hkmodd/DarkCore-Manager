use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, REFERER};
use serde::Deserialize;
use std::error::Error;
use std::path::Path;

pub struct ManifestDownloader {
    client: reqwest::Client,
}

#[derive(Deserialize, Debug)]
struct ServerResponse {
    response: ServerResponseInternal,
}

#[derive(Deserialize, Debug)]
struct ServerResponseInternal {
    servers: Vec<SteamServer>,
}

#[derive(Deserialize, Debug)]
struct SteamServer {
    vhost: String,
    // other fields ignored
}

impl ManifestDownloader {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// Step 1: Get Valve content servers
    async fn get_cdn_host(&self) -> Result<String, Box<dyn Error>> {
        let url = "https://api.steampowered.com/IContentServerDirectoryService/GetServersForSteamPipe/v1/?cell_id=0";
        let resp = self.client.get(url).send().await?;
        
        if !resp.status().is_success() {
            return Err(format!("Failed to get CDN list: {}", resp.status()).into());
        }

        let data: ServerResponse = resp.json().await?;
        if let Some(first) = data.response.servers.first() {
             return Ok(first.vhost.clone());
        }

        Err("No CDN servers found".into())
    }

    /// Step 2: Get GMRC Code from wudrm.com
    async fn get_gmrc(&self, manifest_gid: &str) -> Result<String, Box<dyn Error>> {
        let url = format!("http://gmrc.wudrm.com/manifest/{}", manifest_gid);
        
        let mut headers = HeaderMap::new();
        headers.insert(REFERER, HeaderValue::from_static("http://gmrc.wudrm.com"));
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"));

        let resp = self.client.get(&url)
            .headers(headers)
            .send().await?;

        if !resp.status().is_success() {
             return Err(format!("wudrm error: {}", resp.status()).into());
        }

        let text = resp.text().await?;
        let text = text.trim().to_string();
        
        if text.is_empty() || text.len() > 100 { // GMRC is usually short
             return Err("Invalid GMRC response".into());
        }

        Ok(text)
    }

    /// Step 3 & 4: Download Manifest
    pub async fn download_manifest(
        &self, 
        depot_id: &str, 
        manifest_gid: &str, 
        output_dir: &Path
    ) -> Result<std::path::PathBuf, Box<dyn Error>> {
        // 1. Resolve CDN
        let cdn_host = self.get_cdn_host().await?;
        
        // 2. Resolve Code
        let gmrc = self.get_gmrc(manifest_gid).await?;
        
        // 3. Construct URL
        let url = format!(
            "https://{}/depot/{}/manifest/{}/5/{}", 
            cdn_host, depot_id, manifest_gid, gmrc
        );

        // 4. Download
        let resp = self.client.get(&url)
            .header(USER_AGENT, "Valve/Steam HTTP Client 1.0")
            .send().await?;

        if !resp.status().is_success() {
            return Err(format!("Manifest download failed ({}): {}", resp.status(), url).into());
        }

        let bytes = resp.bytes().await?;
        
        if bytes.len() < 100 {
             return Err("Manifest file too small, likely garbage.".into());
        }

        let filename = format!("{}_{}.manifest", depot_id, manifest_gid);
        let out_path = output_dir.join(&filename);
        
        // Ensure dir
        if let Some(p) = output_dir.parent() {
             if !p.exists() {
                 let _ = std::fs::create_dir_all(p);
             }
        }
        
        // Create dir if output_dir doesn't exist (it's the directory itself)
        if !output_dir.exists() {
             let _ = std::fs::create_dir_all(output_dir);
        }

        tokio::fs::write(&out_path, &bytes).await?;
        
        Ok(out_path)
    }
}
