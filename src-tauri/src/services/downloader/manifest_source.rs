use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, REFERER};
use serde::Deserialize;
use std::error::Error;
use std::path::Path;



pub struct ManifestSource {
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

/// Result of a manifest download attempt, with metadata for logging
#[derive(Debug)]
pub struct ManifestResult {
    pub path: std::path::PathBuf,
    pub format_detected: ManifestFormat,
    pub original_size: usize,
    pub final_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum ManifestFormat {
    /// Raw Steam protobuf manifest (native depotcache format)
    RawProtobuf,
    /// ZIP archive containing .manifest file(s)
    ZipArchive,
    /// Zlib-compressed data (needs inflate)
    ZlibCompressed,
    /// Unknown format (saved as-is with warning)
    Unknown,
}

impl std::fmt::Display for ManifestFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestFormat::RawProtobuf => write!(f, "Steam Protobuf"),
            ManifestFormat::ZipArchive => write!(f, "ZIP Archive"),
            ManifestFormat::ZlibCompressed => write!(f, "Zlib Compressed"),
            ManifestFormat::Unknown => write!(f, "Unknown"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// STEAM MANIFEST FORMAT CONSTANTS
// ═══════════════════════════════════════════════════════════════════════
const STEAM_MANIFEST_MAGIC: u32 = 0x71F617D0;
const ZIP_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
const ZLIB_CMF: u8 = 0x78;

impl ManifestSource {
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

    // ═══════════════════════════════════════════════════════════════════
    // FORMAT DETECTION
    // ═══════════════════════════════════════════════════════════════════

    fn detect_format(bytes: &[u8]) -> ManifestFormat {
        if bytes.len() < 4 {
            return ManifestFormat::Unknown;
        }

        if bytes[..4] == ZIP_MAGIC {
            return ManifestFormat::ZipArchive;
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic == STEAM_MANIFEST_MAGIC {
            return ManifestFormat::RawProtobuf;
        }

        if bytes[0] == ZLIB_CMF {
            return ManifestFormat::ZlibCompressed;
        }

        ManifestFormat::Unknown
    }

    fn extract_manifest_bytes(
        raw_bytes: &[u8],
        depot_id: &str,
        manifest_gid: &str,
    ) -> Result<(Vec<u8>, ManifestFormat), Box<dyn Error>> {
        let format = Self::detect_format(raw_bytes);

        match format {
            ManifestFormat::RawProtobuf => {
                Ok((raw_bytes.to_vec(), format))
            }

            ManifestFormat::ZipArchive => {
                let cursor = std::io::Cursor::new(raw_bytes);
                let mut archive = zip::ZipArchive::new(cursor)
                    .map_err(|e| format!("Failed to open ZIP archive: {}", e))?;
                
                let _expected_name = format!("{}_{}.manifest", depot_id, manifest_gid);
                
                let mut manifest_bytes: Option<Vec<u8>> = None;

                for i in 0..archive.len() {
                    if let Ok(mut entry) = archive.by_index(i) {
                        if entry.is_dir() { continue; }

                        let entry_name = entry.name().to_string();
                        let mut buf = Vec::new();
                        use std::io::Read;
                        if entry.read_to_end(&mut buf).is_ok() {
                            if buf.len() >= 4 {
                                let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                                if magic == STEAM_MANIFEST_MAGIC {
                                    manifest_bytes = Some(buf);
                                    break; 
                                }
                            }
                            if manifest_bytes.is_none() && entry_name.ends_with(".manifest") {
                                manifest_bytes = Some(buf);
                            }
                        }
                    }
                }

                match manifest_bytes {
                    Some(bytes) => Ok((bytes, format)),
                    None => Err("ZIP archive contains no valid manifest files".into())
                }
            }

            ManifestFormat::ZlibCompressed => {
                use std::io::Read;
                let mut decoder = flate2::read::ZlibDecoder::new(raw_bytes);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)
                    .map_err(|e| format!("Zlib decompression failed: {}", e))?;
                
                Ok((decompressed, format))
            }

            ManifestFormat::Unknown => {
                // Try zlib fallback
                {
                    use std::io::Read;
                    let mut decoder = flate2::read::ZlibDecoder::new(raw_bytes);
                    let mut decompressed = Vec::new();
                    if decoder.read_to_end(&mut decompressed).is_ok() && decompressed.len() > raw_bytes.len() / 2 {
                        return Ok((decompressed, ManifestFormat::ZlibCompressed));
                    }
                }
                Ok((raw_bytes.to_vec(), ManifestFormat::Unknown))
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // DOWNLOAD
    // ═══════════════════════════════════════════════════════════════════

    pub async fn download_manifest(
        &self, 
        depot_id: &str, 
        manifest_gid: &str, 
        output_dir: &Path
    ) -> Result<std::path::PathBuf, Box<dyn Error>> {
        // 1. Resolve CDN
        let cdn_host = self.get_cdn_host().await?;
        
        // 2. Resolve GMRC Code
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

        let raw_bytes = resp.bytes().await?;
        
        if raw_bytes.len() < 100 {
             return Err("Manifest response too small, likely garbage.".into());
        }

        // 5. Extract
        let (manifest_bytes, format) = Self::extract_manifest_bytes(
            &raw_bytes, depot_id, manifest_gid
        )?;

        if manifest_bytes.len() < 50 {
            return Err(format!("Extracted manifest too small ({} bytes).", manifest_bytes.len()).into());
        }

        // 6. Write
        let filename = format!("{}_{}.manifest", depot_id, manifest_gid);
        let out_path = output_dir.join(&filename);
        
        if !output_dir.exists() {
             std::fs::create_dir_all(output_dir)?;
        }

        tokio::fs::write(&out_path, &manifest_bytes).await?;
        
        if !matches!(format, ManifestFormat::RawProtobuf) {
            println!("[ManifestSource] Converted {} to RawProtobuf.", format);
        }

        Ok(out_path)
    }
}
