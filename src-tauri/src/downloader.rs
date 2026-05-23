use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, REFERER};
use std::error::Error;
use std::path::Path;

#[derive(serde::Deserialize, Debug)]
struct ServerResponse {
    response: ServerResponseInternal,
}

#[derive(serde::Deserialize, Debug)]
struct ServerResponseInternal {
    servers: Vec<SteamServer>,
}

#[derive(serde::Deserialize, Debug)]
struct SteamServer {
    vhost: String,
    // other fields ignored
}

/// Result of a manifest download attempt, with metadata for logging
#[derive(Debug, serde::Serialize)]
pub struct ManifestResult {
    pub path: std::path::PathBuf,
    pub format_detected: String,
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

pub struct ManifestDownloader {
    client: reqwest::Client,
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
    pub async fn get_cdn_host(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
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
    async fn get_gmrc(&self, manifest_gid: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
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

        // Check for ZIP archive (PK\x03\x04)
        if bytes[..4] == ZIP_MAGIC {
            return ManifestFormat::ZipArchive;
        }

        // Check for Steam protobuf manifest (little-endian magic)
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic == STEAM_MANIFEST_MAGIC {
            return ManifestFormat::RawProtobuf;
        }

        // Check for zlib-compressed data (CMF byte = 0x78)
        if bytes[0] == ZLIB_CMF {
            return ManifestFormat::ZlibCompressed;
        }

        ManifestFormat::Unknown
    }

    fn extract_manifest_bytes(
        raw_bytes: &[u8],
        depot_id: &str,
        manifest_gid: &str,
    ) -> Result<(Vec<u8>, ManifestFormat), Box<dyn Error + Send + Sync>> {
        let format = Self::detect_format(raw_bytes);

        match format {
            ManifestFormat::RawProtobuf => {
                Ok((raw_bytes.to_vec(), format))
            }

            ManifestFormat::ZipArchive => {
                let cursor = std::io::Cursor::new(raw_bytes);
                let mut archive = zip::ZipArchive::new(cursor)
                    .map_err(|e| format!("Failed to open ZIP archive: {}", e))?;
                
                let expected_gid = manifest_gid.to_string();
                let expected_depot = depot_id.to_string();
                
                let mut best_candidate: Option<(Vec<u8>, i32)> = None; // (bytes, score)

                for i in 0..archive.len() {
                     if let Ok(mut entry) = archive.by_index(i) {
                        if entry.is_dir() { continue; }

                        let entry_name = entry.name().to_string();
                        let mut buf = Vec::new();
                        use std::io::Read;
                        
                        if entry.read_to_end(&mut buf).is_ok() {
                            // Must have Magic
                            if buf.len() < 8 { continue; }
                            let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                            if magic != STEAM_MANIFEST_MAGIC { continue; }
                            
                            // Safety: Do NOT trim payload. Trust the ZIP content.
                            // Payload Trimming was corrupting valid manifests.
                            // let payload_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
                            // if payload_len + 8 <= buf.len() {
                            //    buf.truncate(payload_len + 8);
                            // }

                            // Scoring
                            let mut score = 10; // Base score for having magic
                            if entry_name.contains(&expected_gid) { score += 100; }
                            if entry_name.contains(&expected_depot) { score += 50; }
                            if entry_name.ends_with(".manifest") { score += 5; }

                            // Check if current is better
                            match best_candidate {
                                Some((_, best_score)) => {
                                    if score > best_score {
                                        best_candidate = Some((buf, score));
                                    }
                                },
                                None => {
                                    best_candidate = Some((buf, score));
                                }
                            }
                        }
                     }
                }

                match best_candidate {
                    Some((bytes, _)) => Ok((bytes, format)),
                    None => Err(format!("ZIP archive contains no valid manifest files").into())
                }
            }

            ManifestFormat::ZlibCompressed => {
                use std::io::Read;
                let mut decoder = flate2::read::ZlibDecoder::new(raw_bytes);
                let mut decompressed = Vec::new();
                if let Err(e) = decoder.read_to_end(&mut decompressed) {
                     return Err(format!("Zlib decompression failed: {}", e).into());
                }
                Ok((decompressed, format))
            }

            ManifestFormat::Unknown => {
                // Try crude ZIP scan
                 if raw_bytes.len() > 10 {
                    let scan_limit = raw_bytes.len().min(1024);
                    for offset in 0..scan_limit.saturating_sub(4) {
                        if raw_bytes[offset..offset + 4] == ZIP_MAGIC {
                            let cursor = std::io::Cursor::new(&raw_bytes[offset..]);
                            if let Ok(mut archive) = zip::ZipArchive::new(cursor) {
                                for i in 0..archive.len() {
                                    if let Ok(mut entry) = archive.by_index(i) {
                                        if entry.name().ends_with(".manifest") {
                                            let mut buf = Vec::new();
                                            use std::io::Read;
                                            if entry.read_to_end(&mut buf).is_ok() {
                                                return Ok((buf, ManifestFormat::ZipArchive));
                                            }
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
                
                // Try crude Zlib
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

    /// Main logic: Download -> Extract -> Save
    pub async fn download_manifest(
        &self, 
        depot_id: &str, 
        manifest_gid: &str, 
        output_dir: &Path
    ) -> Result<ManifestResult, Box<dyn Error + Send + Sync>> {
        // 1. Resolve CDN
        let cdn_host = self.get_cdn_host().await?;
        eprintln!("[WUDRM] CDN Host: {}", cdn_host);
        
        // 2. Resolve GMRC Code
        let gmrc = self.get_gmrc(manifest_gid).await?;
        eprintln!("[WUDRM] GMRC Code for GID {}: {}", manifest_gid, gmrc);
        
        // 3. Construct URL
        let url = format!(
            "https://{}/depot/{}/manifest/{}/5/{}", 
            cdn_host, depot_id, manifest_gid, gmrc
        );
        eprintln!("[WUDRM] Full URL: {}", url);

        // 4. Download raw response
        let resp = self.client.get(&url)
            .header(USER_AGENT, "Valve/Steam HTTP Client 1.0")
            .send().await?;

        if !resp.status().is_success() {
            return Err(format!("Manifest download failed ({}): {}", resp.status(), url).into());
        }

        let raw_bytes = resp.bytes().await?;
        let original_size = raw_bytes.len();
        eprintln!("[WUDRM] Response size: {} bytes", original_size);
        
        // Log first 16 bytes for format debug
        if raw_bytes.len() >= 16 {
            eprintln!("[WUDRM] First 16 bytes: {:02X?}", &raw_bytes[..16]);
        }
        
        if raw_bytes.len() < 100 {
             return Err("Manifest response too small, likely garbage.".into());
        }

        // 5. INTELLIGENT EXTRACTION
        let (manifest_bytes, format) = Self::extract_manifest_bytes(
            &raw_bytes, depot_id, manifest_gid
        )?;
        
        eprintln!("[WUDRM] Detected format: {:?}, extracted size: {} bytes", format, manifest_bytes.len());

        if manifest_bytes.len() < 50 {
             return Err(format!("Extracted manifest too small. Likely corrupt.").into());
        }

        // 6. Write to output
        let filename = format!("{}_{}.manifest", depot_id, manifest_gid);
        let out_path = output_dir.join(&filename);
        
        if !output_dir.exists() {
             std::fs::create_dir_all(output_dir)?;
        }

        tokio::fs::write(&out_path, &manifest_bytes).await?;
        eprintln!("[WUDRM] Saved manifest to: {:?} ({} bytes)", out_path, manifest_bytes.len());
        
        Ok(ManifestResult {
            path: out_path,
            format_detected: format.to_string(),
            original_size,
            final_size: manifest_bytes.len()
        })
    }
}
