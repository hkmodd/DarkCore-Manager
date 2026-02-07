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
// Steam depot manifests use a protobuf-based format with specific magic bytes.
// The header starts with a 4-byte little-endian magic value.
// Reference: SteamKit2/Types/Manifest.cs - PROTOBUF_PAYLOAD_MAGIC
const STEAM_MANIFEST_MAGIC: u32 = 0x71F617D0;

// ZIP files always start with PK\x03\x04 (local file header)
const ZIP_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

// Zlib default compression starts with 0x78 (CMF byte)
// 0x78 0x01 = no compression
// 0x78 0x5E = fast compression
// 0x78 0x9C = default compression
// 0x78 0xDA = best compression
const ZLIB_CMF: u8 = 0x78;

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

    // ═══════════════════════════════════════════════════════════════════
    // FORMAT DETECTION — Identify what the CDN actually gave us
    // ═══════════════════════════════════════════════════════════════════

    /// Detect the format of downloaded manifest data by inspecting magic bytes.
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

    /// Extract the actual .manifest bytes from whatever format the CDN returned.
    /// Returns (final_bytes, format_detected).
    fn extract_manifest_bytes(
        raw_bytes: &[u8],
        depot_id: &str,
        manifest_gid: &str,
    ) -> Result<(Vec<u8>, ManifestFormat), Box<dyn Error>> {
        let format = Self::detect_format(raw_bytes);

        match format {
            ManifestFormat::RawProtobuf => {
                // Already in the correct format — pass through directly
                Ok((raw_bytes.to_vec(), format))
            }

            ManifestFormat::ZipArchive => {
                // ZIP archive: extract the .manifest file from inside
                let cursor = std::io::Cursor::new(raw_bytes);
                let mut archive = zip::ZipArchive::new(cursor)
                    .map_err(|e| format!("Failed to open ZIP archive: {}", e))?;
                
                let expected_name = format!("{}_{}.manifest", depot_id, manifest_gid);
                
                // Strategy: Scan ALL files for Steam Protobuf Magic Bytes
                // This handles the "file named 'z'" case reported by devs.
                let mut manifest_bytes: Option<Vec<u8>> = None;
                let mut found_name = String::new();

                for i in 0..archive.len() {
                    if let Ok(mut entry) = archive.by_index(i) {
                        // Skip directories
                        if entry.is_dir() { continue; }

                        let entry_name = entry.name().to_string();
                        
                        // Read content
                        let mut buf = Vec::new();
                        use std::io::Read;
                        if entry.read_to_end(&mut buf).is_ok() {
                            // CHECK 1: Magic Bytes (0x71F617D0)
                            if buf.len() >= 4 {
                                let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                                if magic == STEAM_MANIFEST_MAGIC {
                                    manifest_bytes = Some(buf);
                                    found_name = entry_name;
                                    break; // Found it!
                                }
                            }

                            // CHECK 2: Fallback to extension if magic check is inconclusive?
                            // Actually, if it's a valid manifest, it MUST have the magic.
                            // But for legacy support, if we haven't found a magic match yet,
                            // we can tentatively keep a .manifest file.
                            if manifest_bytes.is_none() && entry_name.ends_with(".manifest") {
                                manifest_bytes = Some(buf);
                                found_name = entry_name;
                            }
                        }
                    }
                }

                match manifest_bytes {
                    Some(bytes) => {
                        if !found_name.is_empty() && found_name != expected_name {
                            // Only log if it's NOT the expected name (e.g. "z")
                            println!(
                                "[ManifestDownloader] Extracted manifest from ZIP entry: '{}' (Magic Match)",
                                found_name
                            );
                        }
                        Ok((bytes, format))
                    }
                    None => {
                        // ZIP exists but contains no valid manifest files
                        let mut contents = Vec::new();
                        if let Ok(mut archive2) = zip::ZipArchive::new(std::io::Cursor::new(raw_bytes)) {
                            for i in 0..archive2.len() {
                                if let Ok(entry) = archive2.by_index_raw(i) {
                                    contents.push(entry.name().to_string());
                                }
                            }
                        }
                        Err(format!(
                            "ZIP archive contains no valid manifest files (checked Magic Bytes & Extension). Contents: {:?}",
                            contents
                        ).into())
                    }
                }
            }

            ManifestFormat::ZlibCompressed => {
                // Zlib-compressed data: decompress it
                use std::io::Read;
                let mut decoder = flate2::read::ZlibDecoder::new(raw_bytes);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)
                    .map_err(|e| format!("Zlib decompression failed: {}", e))?;
                
                // After decompression, verify it looks like a valid manifest
                if decompressed.len() >= 4 {
                    let inner_magic = u32::from_le_bytes([
                        decompressed[0], decompressed[1],
                        decompressed[2], decompressed[3],
                    ]);
                    if inner_magic == STEAM_MANIFEST_MAGIC {
                        return Ok((decompressed, format));
                    }
                }
                
                // Decompressed but doesn't look like a manifest — still save it
                Ok((decompressed, format))
            }

            ManifestFormat::Unknown => {
                // Unknown format. Check if it might be a ZIP with a bad header
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
                                            entry.read_to_end(&mut buf)?;
                                            return Ok((buf, ManifestFormat::ZipArchive));
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                }

                // Last resort: Try zlib decompression even without matching header
                {
                    use std::io::Read;
                    let mut decoder = flate2::read::ZlibDecoder::new(raw_bytes);
                    let mut decompressed = Vec::new();
                    if decoder.read_to_end(&mut decompressed).is_ok() && decompressed.len() > raw_bytes.len() / 2 {
                        return Ok((decompressed, ManifestFormat::ZlibCompressed));
                    }
                }

                // Truly unknown — return as-is
                eprintln!(
                    "[ManifestDownloader] WARNING: Unknown format for depot {} manifest {}. First 8 bytes: {:02X?}",
                    depot_id, manifest_gid,
                    &raw_bytes[..raw_bytes.len().min(8)]
                );
                Ok((raw_bytes.to_vec(), ManifestFormat::Unknown))
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // MAIN DOWNLOAD METHOD — Now with intelligent format handling
    // ═══════════════════════════════════════════════════════════════════

    /// Download and extract a depot manifest from Steam CDN via WUDRM.
    /// 
    /// The CDN may return data in various formats:
    /// - Raw protobuf manifest (direct depotcache-compatible)
    /// - ZIP archive containing .manifest files
    /// - Zlib-compressed manifest data
    ///
    /// This method detects the format automatically and extracts the clean
    /// .manifest file ready for placement in Steam/depotcache/.
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

        // 4. Download raw response
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

        // 5. INTELLIGENT EXTRACTION — Detect format and extract clean manifest
        let (manifest_bytes, format) = Self::extract_manifest_bytes(
            &raw_bytes, depot_id, manifest_gid
        )?;

        if manifest_bytes.len() < 50 {
            return Err(format!(
                "Extracted manifest too small ({} bytes, format: {}). Likely corrupt.",
                manifest_bytes.len(), format
            ).into());
        }

        // 6. Write the CLEAN manifest to output directory
        let filename = format!("{}_{}.manifest", depot_id, manifest_gid);
        let out_path = output_dir.join(&filename);
        
        // Ensure output directory exists
        if !output_dir.exists() {
             std::fs::create_dir_all(output_dir)?;
        }

        tokio::fs::write(&out_path, &manifest_bytes).await?;
        
        // Log format detection for debugging
        if !matches!(format, ManifestFormat::RawProtobuf) {
            println!(
                "[ManifestDownloader] Depot {}: Detected {} format ({} → {} bytes). Extracted successfully.",
                depot_id, format, raw_bytes.len(), manifest_bytes.len()
            );
        }

        Ok(out_path)
    }

    /// Download manifest and return detailed result with metadata.
    /// Used when callers need format information for logging.
    pub async fn download_manifest_detailed(
        &self,
        depot_id: &str,
        manifest_gid: &str,
        output_dir: &Path
    ) -> Result<ManifestResult, Box<dyn Error>> {
        // 1. Resolve CDN
        let cdn_host = self.get_cdn_host().await?;
        
        // 2. Resolve GMRC Code
        let gmrc = self.get_gmrc(manifest_gid).await?;
        
        // 3. Construct URL & Download
        let url = format!(
            "https://{}/depot/{}/manifest/{}/5/{}",
            cdn_host, depot_id, manifest_gid, gmrc
        );

        let resp = self.client.get(&url)
            .header(USER_AGENT, "Valve/Steam HTTP Client 1.0")
            .send().await?;

        if !resp.status().is_success() {
            return Err(format!("Manifest download failed ({})", resp.status()).into());
        }

        let raw_bytes = resp.bytes().await?;
        let original_size = raw_bytes.len();

        if original_size < 100 {
            return Err("Manifest response too small".into());
        }

        // 4. Extract
        let (manifest_bytes, format_detected) = Self::extract_manifest_bytes(
            &raw_bytes, depot_id, manifest_gid
        )?;
        let final_size = manifest_bytes.len();

        // 5. Write
        let filename = format!("{}_{}.manifest", depot_id, manifest_gid);
        let out_path = output_dir.join(&filename);
        
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        tokio::fs::write(&out_path, &manifest_bytes).await?;

        Ok(ManifestResult {
            path: out_path,
            format_detected,
            original_size,
            final_size,
        })
    }
}
