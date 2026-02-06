use std::error::Error;
use std::io::{Cursor, Read};

#[derive(Debug, Clone)]
pub struct ProtoDepotManifest {
    pub filenames: Vec<FileMapping>,
}

#[derive(Debug, Clone)]
pub struct FileMapping {
    pub filename: String,
    pub size: u64,
    pub flags: u32,
    pub sha_filename: Vec<u8>,
    pub sha_content: Vec<u8>,
    pub chunks: Vec<ChunkInfo>,
    pub linktarget: String,
}

#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub chunk_id: Vec<u8>, // SHA1
    pub checksum: u32,     // Adler32 (Steam implementation)
    pub offset: u64,
    pub uncompressed_length: u32,
    pub compressed_length: u32,
}

pub struct ManifestParser;

impl ManifestParser {
    pub fn parse(data: &[u8]) -> Result<ProtoDepotManifest, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let mut all_files = Vec::new();

        // Steam Manifest Magic: 0x71F617D0
        // Structure: [MAGIC 4b] [LENGTH 4b] [PAYLOAD]
        // Payload can be repeated.

        while (cursor.position() as usize) < data.len() {
            if (data.len() - cursor.position() as usize) < 8 {
                break;
            }

            let mut header_buf = [0u8; 8];
            cursor.read_exact(&mut header_buf)?;
            let magic = u32::from_le_bytes(header_buf[0..4].try_into()?);
            let len = u32::from_le_bytes(header_buf[4..8].try_into()?) as usize;

            if magic != 0x71F617D0 {
                // If invalid magic at start, might be raw payload
                if cursor.position() == 8 {
                    cursor.set_position(0);
                    return Self::parse_manifest_payload(data);
                }
                break;
            }

            if (data.len() - cursor.position() as usize) < len {
                return Err("Payload truncated".into());
            }

            let mut payload = vec![0u8; len];
            cursor.read_exact(&mut payload)?;

            let manifest = Self::parse_manifest_payload(&payload)?;
            all_files.extend(manifest.filenames);
        }

        if all_files.is_empty() {
            return Self::parse_manifest_payload(data);
        }

        Ok(ProtoDepotManifest {
            filenames: all_files,
        })
    }

    fn parse_manifest_payload(data: &[u8]) -> Result<ProtoDepotManifest, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let mut files = Vec::new();

        while (cursor.position() as usize) < data.len() {
            let Ok(tag) = Self::read_varint(&mut cursor) else {
                break;
            };
            let field = tag >> 3;
            let wire = (tag & 7) as u8;

            match field {
                1 => {
                    // Field 1: Repeated FileMapping (Wire 2)
                    if wire != 2 {
                        Self::skip_field(&mut cursor, wire)?;
                        continue;
                    }
                    let len = Self::read_varint(&mut cursor)? as usize;
                    let start = cursor.position() as usize;
                    if start + len > data.len() {
                        return Err("FileMapping overflow".into());
                    }

                    let file_data = &data[start..start + len];
                    cursor.set_position((start + len) as u64);

                    if let Ok(file) = Self::parse_file_mapping(file_data) {
                        files.push(file);
                    }
                }
                _ => Self::skip_field(&mut cursor, wire)?,
            }
        }

        Ok(ProtoDepotManifest { filenames: files })
    }

    fn parse_file_mapping(data: &[u8]) -> Result<FileMapping, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);

        let mut f = FileMapping {
            filename: String::new(),
            size: 0,
            flags: 0,
            sha_filename: Vec::new(),
            sha_content: Vec::new(),
            chunks: Vec::new(),
            linktarget: String::new(),
        };

        while (cursor.position() as usize) < data.len() {
            let Ok(tag) = Self::read_varint(&mut cursor) else {
                break;
            };
            let field = tag >> 3;
            let wire = (tag & 7) as u8;

            match field {
                1 => {
                    // filename
                    let len = Self::read_varint(&mut cursor)? as usize;
                    let mut buf = vec![0u8; len];
                    cursor.read_exact(&mut buf)?;
                    f.filename = String::from_utf8_lossy(&buf).to_string();
                }
                2 => f.size = Self::read_varint(&mut cursor)?,
                3 => f.flags = Self::read_varint(&mut cursor)? as u32,
                4 => {
                    // sha_filename
                    let len = Self::read_varint(&mut cursor)? as usize;
                    f.sha_filename = vec![0u8; len];
                    cursor.read_exact(&mut f.sha_filename)?;
                }
                5 => {
                    // sha_content
                    let len = Self::read_varint(&mut cursor)? as usize;
                    f.sha_content = vec![0u8; len];
                    cursor.read_exact(&mut f.sha_content)?;
                }
                6 => {
                    // chunks
                    if wire != 2 {
                        Self::skip_field(&mut cursor, wire)?;
                        continue;
                    }
                    let len = Self::read_varint(&mut cursor)? as usize;
                    let start = cursor.position() as usize;
                    if start + len > data.len() {
                        return Err("ChunkData overflow".into());
                    }
                    let chunk_slice = &data[start..start + len];
                    cursor.set_position((start + len) as u64);

                    if let Ok(chunk) = Self::parse_chunk_info(chunk_slice) {
                        f.chunks.push(chunk);
                    }
                }
                7 => {
                    // linktarget
                    let len = Self::read_varint(&mut cursor)? as usize;
                    let mut buf = vec![0u8; len];
                    cursor.read_exact(&mut buf)?;
                    f.linktarget = String::from_utf8_lossy(&buf).to_string();
                }
                _ => Self::skip_field(&mut cursor, wire)?,
            }
        }
        Ok(f)
    }

    fn parse_chunk_info(data: &[u8]) -> Result<ChunkInfo, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let mut c = ChunkInfo {
            chunk_id: Vec::new(),
            checksum: 0,
            offset: 0,
            uncompressed_length: 0,
            compressed_length: 0,
        };

        while (cursor.position() as usize) < data.len() {
            let Ok(tag) = Self::read_varint(&mut cursor) else {
                break;
            };
            let field = tag >> 3;
            let wire = (tag & 7) as u8;

            match field {
                1 => {
                    // chunk_id
                    let len = Self::read_varint(&mut cursor)? as usize;
                    c.chunk_id = vec![0u8; len];
                    cursor.read_exact(&mut c.chunk_id)?;
                }
                2 => {
                    // checksum (Fixed32 - Wire 5)
                    if wire == 5 {
                        let mut buf = [0u8; 4];
                        cursor.read_exact(&mut buf)?;
                        c.checksum = u32::from_le_bytes(buf);
                    } else {
                        Self::skip_field(&mut cursor, wire)?;
                    }
                }
                3 => c.offset = Self::read_varint(&mut cursor)?,
                4 => c.uncompressed_length = Self::read_varint(&mut cursor)? as u32,
                5 => c.compressed_length = Self::read_varint(&mut cursor)? as u32,
                _ => Self::skip_field(&mut cursor, wire)?,
            }
        }
        Ok(c)
    }

    fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<u64, Box<dyn Error>> {
        let mut result = 0u64;
        let mut shift = 0u32;
        loop {
            let mut byte = [0u8; 1];
            if cursor.read(&mut byte)? == 0 {
                return Err("Varint EOF".into());
            }
            result |= ((byte[0] & 0x7F) as u64) << shift;
            if byte[0] & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift > 63 {
                return Err("Varint overflow".into());
            }
        }
    }

    fn skip_field(cursor: &mut Cursor<&[u8]>, wire: u8) -> Result<(), Box<dyn Error>> {
        match wire {
            0 => {
                Self::read_varint(cursor)?;
            }
            1 => {
                cursor.set_position(cursor.position() + 8);
            }
            2 => {
                let len = Self::read_varint(cursor)?;
                cursor.set_position(cursor.position() + len);
            }
            5 => {
                cursor.set_position(cursor.position() + 4);
            }
            _ => return Err(format!("Unknown wire type {}", wire).into()),
        }
        Ok(())
    }
}
