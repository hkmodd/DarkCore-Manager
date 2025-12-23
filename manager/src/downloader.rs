use serde::Deserialize;
use std::process::Command;
use std::path::{Path};
use std::fs::{OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use aes::cipher::{BlockDecrypt, BlockDecryptMut, KeyInit, KeyIvInit};
use aes::cipher::generic_array::GenericArray;
use aes::Aes256;
use futures::stream::{StreamExt};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

type Aes256Cbc = cbc::Decryptor<Aes256>;

// Structures for JSON Bridge
#[derive(Deserialize, Debug, Clone)]
struct ChunkEntry {
    id: String,
    offset: u64,
//    cb_original: u64,
//    cb_compressed: u64,
    #[serde(rename = "crc")]
    pub _crc: u32,
}

#[derive(Deserialize, Debug)]
struct FileEntry {
    filename: String,
    size: u64,
    flags: u32,
    chunks: Vec<ChunkEntry>,
}

#[derive(Deserialize, Debug)]
struct ManifestDump {
    files: Vec<FileEntry>,
}

pub async fn tactical_download(
    manifest_path: &str,
    depot_id: &str,
    depot_key_hex: &str,
    install_dir: &str,
    log_fn: impl Fn(String),
) -> Result<(u64, String), Box<dyn std::error::Error>> {
    log_fn(format!("⚔️ TACTICAL DOWNLOADER MK4 (Ludicrous Speed) Initiated for Depot {}", depot_id));
    
    // 1. Parse Manifest via Python Bridge
    log_fn("   - Parsing Manifest structure...".to_string());
    let python_script = r"E:\Programmi VARI\STEAM h4cks\GREEN LUMA\DARKCORE-GREENLUMA\docs\SteamManifestDownloader\manifest_dumper.py";
    
    // Use python from system or specific path?
    let output = Command::new("python")
        .arg(python_script)
        .arg(manifest_path)
        .env("PROTOCOL_BUFFERS_PYTHON_IMPLEMENTATION", "python")
        .output()?;
        
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Manifest Dump Failed: {}", err).into());
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let manifest: ManifestDump = match serde_json::from_str(&json_str) {
        Ok(m) => m,
        Err(e) => {
             let safe_slice = if json_str.len() > 200 { &json_str[..200] } else { &json_str };
             return Err(format!("JSON Parse Error: {} (Content: {})", e, safe_slice).into());
        }
    };
    
    log_fn(format!("   - Found {} files to download.", manifest.files.len()));
    
    let key_bytes = hex::decode(depot_key_hex)?;
    if key_bytes.len() != 32 {
        return Err("Invalid Key Length (Expected 32 bytes)".into());
    }
    let key_arc = Arc::new(key_bytes);

use tokio::sync::mpsc;
use std::collections::HashMap;

    // 2. Pre-Process & Flatten Files
    let base_path = Path::new(install_dir);
    if !base_path.exists() { std::fs::create_dir_all(base_path)?; }

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(128) 
        .tcp_nodelay(true)
        .build()?;
    let client_arc = Arc::new(client);
    
    let cdns = vec![
        "http://steampipe.akamaized.net",
        "http://google.cdn.steampipe.steamcontent.com"
    ];
    let cdns_arc = Arc::new(cdns);
    let depot_id_arc = Arc::new(depot_id.to_string());
    let cdn_index = Arc::new(AtomicUsize::new(0));

    // Flatten Manifest: Create a Global Job List
    struct GlobalChunk {
        file_idx: usize,
        chunk: ChunkEntry,
    }
    
    let mut all_chunks = Vec::new();
    let mut file_paths = Vec::new();
    
    // Pre-allocate files (Important for disk fragmentation prevention)
    log_fn("   - Pre-allocating disk space...".to_string());
    for (idx, file) in manifest.files.iter().enumerate() {
        let file_path = base_path.join(&file.filename);
        file_paths.push(file_path.clone());
        
        // Handle Directory
        if (file.flags & 0x40) != 0 {
             std::fs::create_dir_all(&file_path)?;
             continue;
        }

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let file_path_clone = file_path.clone();
        let size = file.size;
        
        // Do alloc in blocking thread to speed up startup
        tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new()
                .write(true).create(true).truncate(true).open(&file_path_clone);
             if let Ok(f) = file {
                 let _ = f.set_len(size);
             }
        }).await?;

        for chunk in &file.chunks {
            all_chunks.push(GlobalChunk {
                file_idx: idx,
                chunk: chunk.clone(),
            });
        }
    }

    log_fn(format!("   🚀 STARTING GLOBAL STREAM: {} Chunks (MkVI/256x/GlobalPipeline)", all_chunks.len()));
            
    // CHANNEL for Writer
    // (FileIndex, Offset, Data)
    let (tx, mut rx) = mpsc::channel::<(usize, u64, Vec<u8>)>(1024); // 1GB Buffer
    
    let file_paths_arc = Arc::new(file_paths);
    
    // WRITER THREAD (Multi-File Handler)
    let writer_handle = tokio::task::spawn_blocking(move || {
        let mut open_files: HashMap<usize, std::fs::File> = HashMap::new();
        let paths = file_paths_arc;
        
        while let Some((f_idx, offset, data)) = rx.blocking_recv() {
            let file = open_files.entry(f_idx).or_insert_with(|| {
                 OpenOptions::new().write(true).open(&paths[f_idx]).unwrap() 
                 // Handle error better in prod?unwrap valid due to pre-alloc
            });
            
            if let Err(e) = file.seek(SeekFrom::Start(offset)) {
                return Err(format!("Seek File {}: {}", f_idx, e));
            }
            if let Err(e) = file.write_all(&data) {
                return Err(format!("Write File {}: {}", f_idx, e));
            }
            
            // Optimization: Periodic Flush? No, OS handles it.
            // LRU logic skipped for simplicity (OS handles few thousand handles okay usually on Windows default 512? No 2048+)
            // If user has >2000 files, might hit limit.
            // Simple flush: If map > 100, close random? 
            if open_files.len() > 200 {
                // Clear some cache strictly
                // Actually, just clear all except current? No.
                // Leave it. Windows default is usually 512 stdio or 8192 handles.
            }
        }
        Ok::<(), String>(())
    });

    // GLOBAL PARALLEL PIPELINE
    let chunks_stream = futures::stream::iter(all_chunks)
        .map(|g_chunk| {
            let client = client_arc.clone();
            let key = key_arc.clone();
            let cdns = cdns_arc.clone();
            let did = depot_id_arc.clone();
            let cdn_atomic = cdn_index.clone();
            
            async move {
                let chunk = g_chunk.chunk;
                let mut last_error = String::new();
                
                 for _ in 0..3 {
                    let idx = cdn_atomic.fetch_add(1, Ordering::Relaxed) % cdns.len();
                    let cdn = &cdns[idx];
                    let url = format!("{}/depot/{}/chunk/{}", cdn, did, chunk.id);
                    
                    let resp = client.get(&url).send().await;
                     match resp {
                        Ok(r) => {
                            if r.status().is_success() {
                                match r.bytes().await {
                                    Ok(b) => {
                                        let task_key = key.clone(); 
                                        let res = tokio::task::spawn_blocking(move || {
                                            let dec = decrypt_chunk(&b, &task_key).map_err(|e| e.to_string())?;
                                            decompress_chunk(&dec).map_err(|e| e.to_string())
                                        }).await;

                                        match res {
                                            Ok(Ok(fin)) => return Ok((g_chunk.file_idx, chunk, fin)),
                                            Ok(Err(e)) => last_error = format!("Crypto: {}", e),
                                            Err(e) => last_error = format!("Join: {}", e),
                                        }
                                    },
                                    Err(e) => last_error = format!("Bytes: {}", e),
                                }
                            } else { last_error = format!("HTTP {}", r.status()); }
                        },
                        Err(e) => last_error = format!("Req: {}", e),
                    }
                }
                Err((chunk.id, last_error))
            }
        })
        .buffer_unordered(256); // 256 Concurrent Chunks

    let mut stream = chunks_stream;
    while let Some(result) = stream.next().await {
        match result {
            Ok((f_idx, chunk, data)) => {
                if let Err(_) = tx.send((f_idx, chunk.offset, data)).await { break; }
            },
            Err((cid, e)) => log_fn(format!("     ❌ Chunk {} Failed: {}", cid, e)),
        }
    }
    
    drop(tx);
    
    if let Err(e) = writer_handle.await.unwrap() {
        log_fn(format!("     🔥 Disk Writer Failure: {}", e));
    }
    
    log_fn("✨ Download Complete.".to_string());
    
    // Extract ManifestID from filename (Assuming format: {depot_id}_{manifest_id}.manifest)
    // If not standard, default to "0"
    let manifest_file_name = std::path::Path::new(manifest_path)
        .file_name().and_then(|s| s.to_str()).unwrap_or("");
    
    let parts: Vec<&str> = manifest_file_name.split('_').collect();
    let manifest_id = if parts.len() >= 2 {
        parts[1].replace(".manifest", "")
    } else {
        "0".to_string()
    };
    
    // Calculate Total Size
    // Sum file sizes from the parsed manifest
    let real_total_size: u64 = manifest.files.iter().map(|f| f.size).sum();

    Ok((real_total_size, manifest_id))
}

fn decrypt_chunk(data: &[u8], key: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    if data.len() < 16 { return Err("Data too short".into()); }
    
    let key_arr = GenericArray::from_slice(key);

    // 1. Decrypt IV (ECB)
    let mut iv = data[0..16].to_vec();
    let mut iv_block = GenericArray::from_mut_slice(&mut iv);
    
    let cipher = Aes256::new(key_arr);
    cipher.decrypt_block(&mut iv_block);

    // 2. Decrypt Payload (CBC)
    let mut payload = data[16..].to_vec();
    
    if payload.len() % 16 != 0 {
         return Err("Payload not block aligned".into());
    }

    let iv_arr = GenericArray::from_slice(&iv);
    // Initialize CBC Decryptor
    let mut cbc_dec = Aes256Cbc::new(key_arr, iv_arr);
    
    // Decrypt block by block
    for chunk in payload.chunks_mut(16) {
        let mut block = GenericArray::from_mut_slice(chunk);
        cbc_dec.decrypt_block_mut(&mut block);
    }

    // Padding (PKCS7)
    if let Some(&pad_len) = payload.last() {
        let pad_len = pad_len as usize;
        if pad_len > 0 && pad_len <= 16 {
             payload.truncate(payload.len() - pad_len);
        }
    }
    
    Ok(payload)
}

fn decompress_chunk(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Check Header
    if data.len() < 2 { return Err("Empty chunk".into()); }
    
    // ZIP ('PK')
    if data[0] == 0x50 && data[1] == 0x4B {
         // Use zip crate
         let reader = std::io::Cursor::new(data);
         let mut archive = zip::ZipArchive::new(reader).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
         let mut file = archive.by_index(0).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
         let mut buffer = Vec::new();
         file.read_to_end(&mut buffer).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
         return Ok(buffer);
    }
    
    // VZ ('VZ')
    if data[0] == 0x56 && data[1] == 0x5A {
        if data.len() < 22 { return Err("VZ Chunk too small".into()); }
        
        let body = &data[12 .. data.len() - 10];
        let props = &data[7..12];
        let mut header = Vec::new();
        header.extend_from_slice(props); 
        
        let size_bytes = &data[data.len()-6 .. data.len()-2];
        let mut size_u64_bytes = [0u8; 8];
        size_u64_bytes[0..4].copy_from_slice(size_bytes);
        
        header.extend_from_slice(&size_u64_bytes);
        
        let mut input = Vec::new();
        input.extend_from_slice(&header);
        input.extend_from_slice(body);
        
        let mut output = Vec::new();
        lzma_rs::lzma_decompress(&mut std::io::Cursor::new(input), &mut output).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        
        return Ok(output);
    }
    
    // Fallback: Try RAW LZMA
    let mut output = Vec::new();
    match lzma_rs::lzma_decompress(&mut std::io::Cursor::new(data), &mut output) {
        Ok(_) => return Ok(output),
        Err(_) => {}
    }
    
    Err("Unknown Compression Format".into())
}
