// NGame BIN file loader and header parser
//
// SPMP8000 games use the NGame1.0 format with the following structure:
// - 0x00-0x07: Magic "NGame1.0"
// - 0x08-0x0B: Flags (0x80000000)
// - 0x0C-0x13: Vendor "Sunplus"
// - 0x1C-0x23: Chip ID "SPCA556" or "SPMP8000"
// - 0x2C-0x43: Game name
// - 0x44-0x53: Media type "Sunmedia" or "Punmedia"
// - 0x70-0x73: Version string
// - 0x74-0x77: Code size (little-endian)
// - 0x78-0x7F: Alignment values
// - 0x80+:     Compressed code and data

use anyhow::Result;
use std::fmt;

/// Magic bytes for NGame format
const NGAME_MAGIC: &[u8; 8] = b"NGame1.0";

/// Header offset constants
const OFFSET_MAGIC: usize = 0x00;
const OFFSET_FLAGS: usize = 0x08;
const OFFSET_VENDOR: usize = 0x0C;
const OFFSET_CHIP_ID: usize = 0x1C;
const OFFSET_GAME_NAME: usize = 0x2C;
const OFFSET_MEDIA_TYPE: usize = 0x44;
const OFFSET_VERSION: usize = 0x74;
const OFFSET_CODE_SIZE: usize = 0x78;

/// Chip type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipType {
    SPCA556,
    SPMP8000,
    Unknown,
}

impl fmt::Display for ChipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChipType::SPCA556 => write!(f, "SPCA556"),
            ChipType::SPMP8000 => write!(f, "SPMP8000"),
            ChipType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Parsed NGame header
#[derive(Debug, Clone)]
pub struct NGameHeader {
    /// Magic bytes (should be "NGame1.0")
    pub magic: [u8; 8],
    /// Flags (typically 0x80000000)
    pub flags: u32,
    /// Vendor string
    pub vendor: String,
    /// Chip type
    pub chip_type: ChipType,
    /// Game name
    pub game_name: String,
    /// Media type string
    pub media_type: String,
    /// Version string
    pub version: String,
    /// Code size (from header)
    pub code_size: u32,
    /// Total file size
    pub file_size: usize,
    /// Offset where encrypted game payload starts
    pub data_offset: usize,
}

impl NGameHeader {
    /// Check if the magic bytes are valid
    pub fn is_valid(&self) -> bool {
        &self.magic == NGAME_MAGIC
    }

    /// Get expected resolution based on chip type
    pub fn default_resolution(&self) -> (u32, u32) {
        // SPMP8000 games typically use 320x240
        (320, 240)
    }

    /// Get expected CPU frequency
    pub fn cpu_freq(&self) -> u32 {
        // SPMP8000 runs at ~7.37MHz
        7_372_800
    }
}

/// Parse a null-terminated string from a byte slice
fn parse_null_string(data: &[u8], offset: usize, max_len: usize) -> String {
    let end = data[offset..]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(max_len)
        .min(max_len);
    String::from_utf8_lossy(&data[offset..offset + end]).to_string()
}

/// Parse NGame header from binary data
pub fn parse_header(data: &[u8]) -> Result<NGameHeader> {
    if data.len() < 0x80 {
        anyhow::bail!("File too small to be a valid NGame binary");
    }

    // Verify magic
    let mut magic = [0u8; 8];
    magic.copy_from_slice(&data[OFFSET_MAGIC..OFFSET_MAGIC + 8]);
    if &magic != NGAME_MAGIC {
        anyhow::bail!(
            "Invalid magic bytes: expected {:?}, got {:?}",
            NGAME_MAGIC,
            &magic
        );
    }

    // Parse flags
    let flags = u32::from_le_bytes([
        data[OFFSET_FLAGS],
        data[OFFSET_FLAGS + 1],
        data[OFFSET_FLAGS + 2],
        data[OFFSET_FLAGS + 3],
    ]);

    // Parse vendor
    let vendor = parse_null_string(data, OFFSET_VENDOR, 8);

    // Parse chip ID
    let chip_id_str = parse_null_string(data, OFFSET_CHIP_ID, 8);
    let chip_type = match chip_id_str.as_str() {
        "SPCA556" => ChipType::SPCA556,
        "SPMP8000" => ChipType::SPMP8000,
        _ => ChipType::Unknown,
    };

    // Parse game name
    let game_name = parse_null_string(data, OFFSET_GAME_NAME, 24);

    // Parse media type
    let media_type = parse_null_string(data, OFFSET_MEDIA_TYPE, 16);

    // Parse version
    let version = parse_null_string(data, OFFSET_VERSION, 4);

    // Parse code size
    let code_size = u32::from_le_bytes([
        data[OFFSET_CODE_SIZE],
        data[OFFSET_CODE_SIZE + 1],
        data[OFFSET_CODE_SIZE + 2],
        data[OFFSET_CODE_SIZE + 3],
    ]);

    Ok(NGameHeader {
        magic,
        flags,
        vendor,
        chip_type,
        game_name,
        media_type,
        version,
        code_size,
        file_size: data.len(),
        data_offset: 0x80,
    })
}

/// Extract encrypted game payload from BIN file
pub fn extract_game_payload(data: &[u8]) -> Result<&[u8]> {
    if data.len() < 0x80 {
        anyhow::bail!("File too small");
    }
    Ok(&data[0x80..])
}

/// Extract compressed data from BIN file.
///
/// Kept for compatibility with older call sites; NGame BIN payloads are
/// encrypted rather than conventionally compressed.
pub fn extract_compressed_data(data: &[u8]) -> Result<&[u8]> {
    extract_game_payload(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header_valid() {
        let mut data = vec![0u8; 256];
        // Set magic
        data[0..8].copy_from_slice(b"NGame1.0");
        // Set flags
        data[8..12].copy_from_slice(&0x80000000u32.to_le_bytes());
        // Set vendor
        data[12..20].copy_from_slice(b"Sunplus\0");
        // Set chip ID
        data[28..36].copy_from_slice(b"SPMP8000");
        // Set game name
        data[44..56].copy_from_slice(b"TestGame\0\0\0\0");
        // Set media type
        data[68..78].copy_from_slice(b"Sunmedia\0\0");
        // Set version
        data[116..120].copy_from_slice(b"100\0");
        // Set code size
        data[120..124].copy_from_slice(&1024u32.to_le_bytes());

        let header = parse_header(&data).unwrap();
        assert!(header.is_valid());
        assert_eq!(header.chip_type, ChipType::SPMP8000);
        assert_eq!(header.game_name, "TestGame");
        assert_eq!(header.code_size, 1024);
    }

    #[test]
    fn test_parse_header_invalid_magic() {
        let data = vec![0u8; 256];
        assert!(parse_header(&data).is_err());
    }

    #[test]
    fn test_parse_header_too_small() {
        let data = vec![0u8; 64];
        assert!(parse_header(&data).is_err());
    }
}
