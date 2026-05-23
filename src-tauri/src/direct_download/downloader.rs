use std::sync::Arc;
use std::error::Error;
use std::path::PathBuf;

use aes::cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray};
use aes::Aes256;
use steam_vent::connection::Connection;
use steam_vent::ServerList; 

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, AsyncSeekExt, SeekFrom};

// Import sibling modules
// Import shared manifest parser
use crate::services::downloader::manifest_parser::{ProtoDepotManifest, ManifestParser};

// Data Structures
#[derive(Clone, Debug)]
pub struct ChunkData {
    pub id: String, // Hex String (SHA1)
    pub checksum: u32, // Adler32
    pub offset: u64,
    pub compressed_length: u32,
    pub uncompressed_length: u32,
    pub filename: String,
}

pub struct DirectDownloader {
    http_client: reqwest::Client,
}

impl DirectDownloader {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .user_agent("Valve/Steam HTTP Client 1.0 (Windows)")
                .pool_max_idle_per_host(8)
                .build()
                .unwrap_or_default(),
        }
    }

    /// Authenticate Anonymously with Steam
    pub async fn authenticate_anonymous() -> Result<Connection, Box<dyn Error>> {
        println!("[IGNITION] Discovering Steam Servers...");
        let server_list = ServerList::discover().await.map_err(|e| format!("Server discovery failed: {}", e))?;
        
        println!("[IGNITION] Connecting to Steam Network...");
        let conn = Connection::anonymous(&server_list).await.map_err(|e| format!("Connection failed: {}", e))?;
        
        println!("[IGNITION] Connected! Anonymous Handshake Successful.");
        Ok(conn)
    }

    /// Process a single chunk: Download -> Decrypt -> Decompress -> Verify
    pub async fn process_chunk(
        &self,
        chunk: &ChunkData,
        depot_id: u32,
        depot_key: &[u8; 32],
        base_url: &str,
        cdn_token: &str,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        // 1. Download
        let url = if cdn_token.is_empty() {
             format!("{}/depot/{}/chunk/{}", base_url, depot_id, chunk.id)
        } else {
             format!("{}/depot/{}/chunk/{}?token={}", base_url, depot_id, chunk.id, cdn_token)
        };
        
        let resp = self.http_client.get(&url).send().await?;
        if !resp.status().is_success() {
             return Err(format!("CDN Error: {} | URL: {}", resp.status(), url).into());
        }
        
        let encrypted_data = resp.bytes().await?;

        // 2. Decrypt (AES-256-ECB IV + AES-256-CBC Body)
        if encrypted_data.len() < 16 {
            return Err("Data too short for IV".into());
        }

        let key = GenericArray::from_slice(depot_key);
        let ecb_cipher = Aes256::new(key);

        let (iv_slice, body_slice) = encrypted_data.split_at(16);
        let mut iv = GenericArray::clone_from_slice(iv_slice);
        ecb_cipher.decrypt_block(&mut iv);

        use cbc::cipher::{BlockDecryptMut, KeyIvInit};
        type Aes256CbcDec = cbc::Decryptor<Aes256>;

        let mut body = body_slice.to_vec();
        let decryptor = Aes256CbcDec::new(key, &iv);
        
        let final_len = match decryptor.decrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut body) {
            Ok(s) => s.len(),
            Err(_) => return Err("CBC Decryption/Padding Failed".into()),
        };
        body.truncate(final_len);
        let decrypted_data = body;

        // 3. Decompress
        match perform_decompression(&decrypted_data, chunk.uncompressed_length) {
            Ok(decompressed) => {
                // 4. Verify Checksum
                let calculated_checksum = calc_adler32(&decompressed);
                if calculated_checksum != chunk.checksum {
                     return Err(format!("Checksum Mismatch! Expected {:08x}, got {:08x}", chunk.checksum, calculated_checksum).into());
                }
                Ok(decompressed)
            },
            Err(e) => {
                 let header = if decrypted_data.len() >= 4 {
                     format!("{:02x} {:02x} {:02x} {:02x}", decrypted_data[0], decrypted_data[1], decrypted_data[2], decrypted_data[3])
                 } else {
                     "EMPTY".to_string()
                 };
                Err(format!("Decompression Failed: {} | Header: {}", e, header).into())
            }
        }
    }

    /// Fetch and Parse Manifest from CDN
    pub async fn fetch_manifest(
        &self,
        depot_id: u32,
        manifest_id: u64,
        depot_key: &[u8; 32],
        base_url: &str,
        cdn_token: &str,
    ) -> Result<ProtoDepotManifest, Box<dyn Error>> {
        let url = format!("{}/depot/{}/manifest/{}/5?token={}", base_url, depot_id, manifest_id, cdn_token);
        
        let resp = self.http_client.get(&url).send().await?;
        if !resp.status().is_success() {
             return Err(format!("Manifest Fetch Error: {}", resp.status()).into());
        }
        let encrypted_data = resp.bytes().await?;
        
        // Decrypt (AES-256-ECB)
        let key = GenericArray::from_slice(depot_key);
        let cipher = Aes256::new(key);
        let mut decrypted_data = encrypted_data.to_vec();
        
        for block in decrypted_data.chunks_mut(16) {
            if block.len() == 16 {
                let mut generic_block = GenericArray::from_mut_slice(block);
                cipher.decrypt_block(&mut generic_block);
            }
        }
        
        // Decompress (Zip)
        let cursor = std::io::Cursor::new(decrypted_data);
        match zip::ZipArchive::new(cursor) {
            Ok(mut archive) => {
                 if archive.len() > 0 {
                     let mut file = archive.by_index(0)?;
                     let mut buffer = Vec::new();
                     std::io::Read::read_to_end(&mut file, &mut buffer)?;
                     
                     let manifest = ManifestParser::parse(&buffer)?;
                     Ok(manifest)
                 } else {
                     Err("Empty Manifest Archive".into())
                 }
            },
            Err(_) => {
                Err("Manifest Decryption/Decompression Failed".into())
            }
        }
    }

    /// Parse raw manifest bytes (e.g. from Cache or manual load)
    pub fn load_manifest_from_bytes(
        &self,
        manifest_bytes: &[u8],
    ) -> Result<ProtoDepotManifest, Box<dyn Error>> {
        let manifest = ManifestParser::parse(manifest_bytes)?;
        Ok(manifest)
    }

    /// Generate Chunk Jobs from Manifest
    pub fn generate_jobs(manifest: &ProtoDepotManifest) -> Vec<ChunkData> {
        let mut jobs = Vec::new();
        for file in &manifest.filenames {
            for chunk in &file.chunks {
                jobs.push(ChunkData {
                    id: hex::encode(&chunk.chunk_id),
                    checksum: chunk.checksum,
                    offset: chunk.offset,
                    compressed_length: chunk.compressed_length,
                    uncompressed_length: chunk.uncompressed_length,
                    filename: file.filename.clone(),
                });
            }
        }
        jobs
    }

    /// Start Parallel Download
    pub async fn start_download_pool(
        self: Arc<Self>,
        jobs: Vec<ChunkData>,
        depot_key: [u8; 32],
        base_url: String,
        target_path: PathBuf,
        state_arc: Arc<std::sync::Mutex<super::state::DownloadState>>,
        _app_id: String, // Kept for interface consistency but unused currently
        depot_id: u32,
        cdn_token: String,
    ) -> Result<(), Box<dyn Error>> {
        
        let file_map: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<tokio::fs::File>>>>> = 
            Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
        
        let semaphore = Arc::new(Semaphore::new(32)); 
        let mut join_set: JoinSet<Result<(), String>> = JoinSet::new();
        
        for job in &jobs {
            let job = job.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let downloader = self.clone();
            let map_ref = file_map.clone();
            let root_path = target_path.clone();
            let s_arc = state_arc.clone();
            let key = depot_key;
            let url = base_url.clone();
            let token = cdn_token.clone();
            
            join_set.spawn(async move {
                let _permit = permit;
                let mut attempts = 0;
                let mut last_error = String::from("None");
                
                loop {
                    attempts += 1;
                    if attempts > 5 {
                        let err_msg = format!("Chunk {} Failed after 5 attempts. Last Error: {}", job.id, last_error);
                        if let Ok(mut s) = s_arc.lock() {
                            s.status = super::state::DownloadStatus::Error(err_msg.clone());
                        }
                        return Err(err_msg);
                    }
                    
                    let execution_result = match downloader.process_chunk(&job, depot_id, &key, &url, &token).await {
                        Ok(data) => Ok(data),
                        Err(e) => Err(e.to_string()),
                    };

                    match execution_result {
                        Ok(data) => {
                            let file_handle = {
                                let mut map = map_ref.lock().await;
                                if let Some(handle) = map.get(&job.filename) {
                                    handle.clone()
                                } else {
                                    let full_path = root_path.join(&job.filename);
                                    if let Some(parent) = full_path.parent() {
                                        let _ = tokio::fs::create_dir_all(parent).await;
                                    }
                                    
                                    let f = OpenOptions::new()
                                        .read(true)
                                        .write(true)
                                        .create(true)
                                        .open(&full_path)
                                        .await
                                        .map_err(|e| e.to_string())?;
                                        
                                    let handle = Arc::new(tokio::sync::Mutex::new(f));
                                    map.insert(job.filename.clone(), handle.clone());
                                    handle
                                }
                            };
                            
                            {
                                let mut f = file_handle.lock().await;
                                f.seek(SeekFrom::Start(job.offset)).await.map_err(|e| e.to_string())?;
                                f.write_all(&data).await.map_err(|e| e.to_string())?;
                            }
                            
                            if let Ok(mut s) = s_arc.lock() {
                                if let super::state::DownloadStatus::Downloading { 
                                    bytes_downloaded, .. 
                                } = &mut s.status {
                                    *bytes_downloaded += data.len() as u64;
                                }
                            }
                            break;
                        },
                        Err(err_msg) => {
                            last_error = err_msg;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempts)).await;
                }
                Ok(())
            });
        }
        
        while let Some(res) = join_set.join_next().await {
             match res {
                Ok(worker_result) => {
                     if let Err(e) = worker_result {
                         return Err(format!("Worker Failure: {}", e).into());
                     }
                }
                Err(e) => return Err(format!("Worker Panic: {}", e).into()),
            }
        }
        
        self.verify_download(&jobs, &target_path).await?;
        
        Ok(())
    }

    async fn verify_download(&self, jobs: &Vec<ChunkData>, root_path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let mut expected_sizes: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        
        for job in jobs {
            let entry = expected_sizes.entry(job.filename.clone()).or_insert(0);
            *entry += job.uncompressed_length as u64;
        }
        
        for (filename, expected_size) in expected_sizes {
            let path = root_path.join(&filename);
            match tokio::fs::metadata(&path).await {
                Ok(metadata) => {
                    if metadata.len() != expected_size {
                         return Err(format!("File mismatch: {}. Expected {}, Got {}", filename, expected_size, metadata.len()).into());
                    }
                },
                Err(e) => {
                    return Err(format!("Missing file: {}. Error: {}", filename, e).into());
                }
            }
        }
        Ok(())
    }
}

// Helper: Robust Decompression
fn perform_decompression(input: &[u8], expected_size: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let input_len = input.len();
    
    // 1. VZstd
    if input_len > 4 && input[0] == b'V' && input[1] == b'S' && input[2] == b'Z' && input[3] == b'a' {
        if input_len < 23 { return Err("VZstd Data too short".into()); }
        let body = &input[8 .. input_len - 15]; 
        match zstd::stream::decode_all(std::io::Cursor::new(body)) {
            Ok(b) => return Ok(b),
            Err(e) => return Err(format!("Zstd Error: {}", e).into()),
        }
    }

    // 2. VZip (LZMA)
    if input_len > 3 && input[0] == b'V' && input[1] == b'Z' && input[2] == b'a' {
         if input.len() < 22 { return Err("VZip Data too short".into()); }
         let props = input[7];
         let dict_size = &input[8..12];
         
         let mut lzma_header = Vec::with_capacity(13);
         lzma_header.push(props);
         lzma_header.extend_from_slice(dict_size);
         lzma_header.extend_from_slice(&(expected_size as u64).to_le_bytes());
         
         let body_msg = &input[12 .. input_len - 10];
         let mut full_lzma_stream = Vec::new();
         full_lzma_stream.extend(lzma_header);
         full_lzma_stream.extend_from_slice(body_msg); 
         
         let mut output = Vec::new();
         let mut reader = std::io::Cursor::new(full_lzma_stream);
         match lzma_rs::lzma_decompress(&mut reader, &mut output) {
             Ok(_) => return Ok(output),
             Err(e) => return Err(format!("LZMA Error: {}", e).into()),
         }
    }
    
    // 3. Zip
    if input_len > 4 && input[0] == 0x50 && input[1] == 0x4B && input[2] == 0x03 && input[3] == 0x04 {
        let cursor = std::io::Cursor::new(input);
        let mut archive = zip::ZipArchive::new(cursor)?;
        if archive.len() > 0 {
             let mut file = archive.by_index(0)?;
             let mut buffer = Vec::new();
             std::io::Read::read_to_end(&mut file, &mut buffer)?;
             return Ok(buffer);
        }
    }

    // 4. Deflate
    {
        let mut decoder = flate2::read::DeflateDecoder::new(input);
        let mut buffer = Vec::with_capacity(expected_size as usize);
        if std::io::Read::read_to_end(&mut decoder, &mut buffer).is_ok() {
            return Ok(buffer);
        }
    }
    
    // 5. Gzip
    {
        let mut decoder = flate2::read::GzDecoder::new(input);
        let mut buffer = Vec::with_capacity(expected_size as usize);
        if std::io::Read::read_to_end(&mut decoder, &mut buffer).is_ok() {
            return Ok(buffer);
        }
    }

    Err("All Decompression Strategies Failed".into())
}

fn calc_adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 0;
    let mut b: u32 = 0;
    for byte in data {
        a = (a + *byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
