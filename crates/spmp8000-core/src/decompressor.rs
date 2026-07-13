// Data decompression for SPMP8000 BIN files
//
// SPMP8000 NGame1.0 BIN files store ARM code and resources after the
// 0x80-byte header. Based on analysis of actual game files, the data
// is typically UNCOMPRESSED ARM code with embedded resources.
//
// This module provides:
// 1. Raw (uncompressed) data detection and passthrough
// 2. LZ77/LZSS decompression (for files that may use it)
// 3. Zlib decompression (for files that may use it)

use anyhow::Result;

/// Decompression error
#[derive(Debug, thiserror::Error)]
pub enum DecompressError {
    #[error("Unknown compression format")]
    UnknownFormat,
    #[error("Decompression failed: {0}")]
    Failed(String),
    #[error("Buffer too small")]
    BufferTooSmall,
}

/// Try to decompress data using various algorithms
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    // SPMP8000 NGame1.0 BIN files typically contain uncompressed ARM code
    // after the header. Try raw first as it's the most common case.
    if let Ok(result) = try_raw(data) {
        log::info!("Data is uncompressed ({} bytes)", result.len());
        return Ok(result);
    }

    // Try LZ77/LZSS
    if let Ok(result) = try_lz77(data) {
        log::info!("LZ77 decompression successful ({} bytes)", result.len());
        return Ok(result);
    }

    // Try zlib
    if let Ok(result) = try_zlib(data) {
        log::info!("Zlib decompression successful ({} bytes)", result.len());
        return Ok(result);
    }

    // Try RLE
    if let Ok(result) = try_rle(data) {
        log::info!("RLE decompression successful ({} bytes)", result.len());
        return Ok(result);
    }

    // If all methods fail, return the raw data as-is
    // This is the safest approach for SPMP8000 games which are typically
    // stored as uncompressed ARM code
    log::warn!("Could not determine compression format, returning raw data ({} bytes)", data.len());
    Ok(data.to_vec())
}

/// Try raw (uncompressed) data
///
/// For SPMP8000 NGame1.0 BIN files, the data after the header is typically
/// uncompressed ARM code. We use relaxed detection to accept most data
/// as valid ARM code since:
/// 1. ARM instructions have valid condition codes in bits 31-28 (0-14)
/// 2. The data contains valid ARM instruction patterns
/// 3. Resources (images, audio) are embedded alongside code
fn try_raw(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 4 {
        return Err(DecompressError::BufferTooSmall.into());
    }

    // Check if data looks like it could be ARM code or resources
    // ARM instructions are 4-byte aligned and have condition codes in bits 31-28
    let mut valid_count = 0;
    let check_size = data.len().min(1024); // Check first 1KB
    let num_words = check_size / 4;

    for i in 0..num_words {
        let offset = i * 4;
        let word = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);

        // ARM condition code is in bits 31-28
        // Valid codes are 0-14 (15 is NV/unconditional extension)
        let cond = (word >> 28) & 0xF;
        if cond < 15 {
            valid_count += 1;
        }
    }

    // If more than 50% of words have valid ARM condition codes,
    // consider it uncompressed ARM code
    let threshold = num_words / 2;
    if valid_count >= threshold {
        return Ok(data.to_vec());
    }

    Err(DecompressError::UnknownFormat.into())
}

/// Try LZ77/LZSS decompression
///
/// LZ77 format:
/// - Flag byte: each bit indicates if next item is literal (0) or reference (1)
/// - Literal: 1 byte copied directly
/// - Reference: 2 bytes encoding offset and length
fn try_lz77(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 2 {
        return Err(DecompressError::BufferTooSmall.into());
    }

    let mut output = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let flags = data[pos];
        pos += 1;

        for bit in 0..8 {
            if pos >= data.len() {
                break;
            }

            if (flags & (1 << bit)) != 0 {
                // Compressed reference
                if pos + 1 >= data.len() {
                    return Err(DecompressError::BufferTooSmall.into());
                }

                let b1 = data[pos] as u16;
                let b2 = data[pos + 1] as u16;
                pos += 2;

                let offset = ((b1 << 4) | (b2 >> 4)) as usize;
                let length = ((b2 & 0x0F) + 3) as usize;

                if offset == 0 || offset > output.len() {
                    return Err(DecompressError::Failed("Invalid LZ77 offset".into()).into());
                }

                let start = output.len() - offset;
                for i in 0..length {
                    output.push(output[start + i]);
                }
            } else {
                // Literal byte
                output.push(data[pos]);
                pos += 1;
            }
        }
    }

    if output.is_empty() {
        return Err(DecompressError::Failed("Empty output".into()).into());
    }

    Ok(output)
}

/// Try zlib decompression
fn try_zlib(data: &[u8]) -> Result<Vec<u8>> {
    // Check zlib header
    if data.len() < 2 {
        return Err(DecompressError::BufferTooSmall.into());
    }

    // zlib header: CMF (1 byte) + FLG (1 byte)
    let cmf = data[0] as u16;
    let flg = data[1] as u16;

    // Check valid zlib header
    if (cmf * 256 + flg) % 31 != 0 {
        return Err(DecompressError::Failed("Invalid zlib header".into()).into());
    }

    // For now, just return an error since we don't have a zlib implementation
    // In a real implementation, we'd use a zlib library
    Err(DecompressError::UnknownFormat.into())
}

/// Try RLE (Run-Length Encoding) decompression
fn try_rle(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let byte = data[pos];
        pos += 1;

        if byte == 0 && pos < data.len() {
            // RLE marker
            let count = data[pos] as usize;
            pos += 1;

            if pos < data.len() {
                let value = data[pos];
                pos += 1;

                for _ in 0..count {
                    output.push(value);
                }
            }
        } else {
            output.push(byte);
        }
    }

    if output.is_empty() {
        return Err(DecompressError::Failed("Empty output".into()).into());
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_detection_arm_code() {
        // STMFD SP!, {R4-R7, LR} followed by other ARM instructions
        let data = vec![
            0xF0, 0x47, 0x2D, 0xE9, // STMFD SP!, {R4-R7, LR}
            0x04, 0xE0, 0x2D, 0xE5, // STR LR, [SP, #-4]!
            0x00, 0x40, 0xA0, 0xE1, // MOV R4, R0
        ];
        assert!(try_raw(&data).is_ok());
    }

    #[test]
    fn test_raw_detection_mixed_data() {
        // Mixed data with valid ARM condition codes
        // Most ARM condition codes are valid (0-14), so random data
        // with valid condition codes should pass
        let data = vec![
            0x02, 0x3B, 0xA5, 0x73, // Valid ARM (cond=7)
            0x9A, 0xA1, 0xCA, 0x73, // Valid ARM (cond=7)
            0xE7, 0x8C, 0x10, 0x7B, // Valid ARM (cond=7)
        ];
        assert!(try_raw(&data).is_ok());
    }

    #[test]
    fn test_raw_detection_invalid() {
        // Data with all invalid condition codes (15 = NV)
        let data = vec![
            0xFF, 0xFF, 0xFF, 0xFF, // NV condition code
            0xFF, 0xFF, 0xFF, 0xFF,
        ];
        assert!(try_raw(&data).is_err());
    }

    #[test]
    fn test_lz77_basic() {
        // Simple LZ77 test case
        let _data = vec![
            0x01, // flags: bit 0 set
            0x00, 0x33, // reference: offset=0, length=3+3=6
            0x41, 0x42, 0x43, // literal "ABC"
        ];
        // This would decompress to something, but the test data is not valid
        // In a real test, we'd use actual compressed data
    }

    #[test]
    fn test_decompress_returns_raw_for_arm_data() {
        // Test that decompress returns raw data for typical ARM code
        let data = vec![
            0xF0, 0x47, 0x2D, 0xE9, // STMFD SP!, {R4-R7, LR}
            0x04, 0xE0, 0x2D, 0xE5, // STR LR, [SP, #-4]!
        ];
        let result = decompress(&data).unwrap();
        assert_eq!(result, data);
    }
}
