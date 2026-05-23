use aes::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};
use aes::Aes256;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use std::error::Error;

/// Decrypts and Decompresses a Steam CDN payload (Manifest or Chunk)
///
/// Logic:
/// 1. Split first 16 bytes (IV) from the rest (Body).
/// 2. Decrypt IV using AES-256-ECB and the Depot Key.
/// 3. Decrypt Body using AES-256-CBC, Depot Key, and the decrypted IV.
/// 4. Attempt decompression (Zstd, Zip, etc.).
pub fn decrypt_and_decompress(
    encrypted_data: &[u8],
    depot_key: &[u8; 32],
) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < 16 {
        return Err("Data too short for IV".to_string());
    }

    let key = GenericArray::from_slice(depot_key);

    // 1. Decrypt IV (ECB)
    let ecb_cipher = Aes256::new(key);
    let (iv_slice, body_slice) = encrypted_data.split_at(16);
    let mut iv = GenericArray::clone_from_slice(iv_slice);
    ecb_cipher.decrypt_block(&mut iv);

    // 2. Decrypt Body (CBC)
    type Aes256CbcDec = cbc::Decryptor<Aes256>;
    let mut body = body_slice.to_vec();
    let decryptor = Aes256CbcDec::new(key, &iv);

    let final_len =
        match decryptor.decrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut body) {
            Ok(s) => s.len(),
            Err(_) => return Err("CBC Decryption/Padding Failed".to_string()),
        };
    body.truncate(final_len);
    let decrypted_data = body;

    // 3. Decompress
    // We assume the uncompressed size is unknown, so we try our best.
    // For manifests, the size is small enough to fit in memory.
    perform_decompression(&decrypted_data).map_err(|e| format!("Decompression Failed: {}", e))
}

// Reuse the robust decompression logic from download_engine
fn perform_decompression(input: &[u8]) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let input_len = input.len();

    // 1. VZstd
    if input_len > 4 && input[0] == b'V' && input[1] == b'S' && input[2] == b'Z' && input[3] == b'a'
    {
        if input_len < 23 {
            return Err("VZstd Data too short".into());
        }
        let body = &input[8..input_len - 15];
        match zstd::stream::decode_all(std::io::Cursor::new(body)) {
            Ok(b) => return Ok(b),
            Err(e) => return Err(format!("Zstd Error: {}", e).into()),
        }
    }

    // 2. VZip (LZMA) - Note: expected_size is often needed for LZMA header.
    // If we don't have it (Manifest), we might struggle here if strict.
    // However, Steam Manifest VZip often has the size in the header?
    // Let's look at download_engine: it constructs a header using `expected_size`.
    // If we don't know it, we might fail LZMA.
    // But manifests are usually Zstd or Zip (Protobuf).

    // 3. Zip
    if input_len > 4 && input[0] == 0x50 && input[1] == 0x4B && input[2] == 0x03 && input[3] == 0x04
    {
        let cursor = std::io::Cursor::new(input);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| Box::<dyn Error + Send + Sync>::from(e))?;
        if archive.len() > 0 {
            let mut file = archive
                .by_index(0)
                .map_err(|e| Box::<dyn Error + Send + Sync>::from(e))?;
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut buffer)
                .map_err(|e| Box::<dyn Error + Send + Sync>::from(e))?;
            return Ok(buffer);
        }
    }

    // 4. Deflate
    {
        let mut decoder = flate2::read::DeflateDecoder::new(input);
        let mut buffer = Vec::new();
        if std::io::Read::read_to_end(&mut decoder, &mut buffer).is_ok() {
            return Ok(buffer);
        }
    }

    // 5. Gzip
    {
        let mut decoder = flate2::read::GzDecoder::new(input);
        let mut buffer = Vec::new();
        if std::io::Read::read_to_end(&mut decoder, &mut buffer).is_ok() {
            return Ok(buffer);
        }
    }

    Err("All Decompression Strategies Failed".into())
}
