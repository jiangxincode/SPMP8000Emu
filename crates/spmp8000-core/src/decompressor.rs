// Data decompression for SPMP8000 BIN files
//
// The compressed data format is currently unknown.
// This module provides a framework for implementing various decompression algorithms.

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

    // Try different decompression methods
    // 1. Try raw (uncompressed)
    if let Ok(result) = try_raw(data) {
        log::info!("Data is uncompressed ({} bytes)", result.len());
        return Ok(result);
    }

    // 2. Try LZ77/LZSS
    if let Ok(result) = try_lz77(data) {
        log::info!("LZ77 decompression successful ({} bytes)", result.len());
        return Ok(result);
    }

    // 3. Try zlib
    if let Ok(result) = try_zlib(data) {
        log::info!("Zlib decompression successful ({} bytes)", result.len());
        return Ok(result);
    }

    // 4. Try RLE
    if let Ok(result) = try_rle(data) {
        log::info!("RLE decompression successful ({} bytes)", result.len());
        return Ok(result);
    }

    // If all methods fail, return the raw data as-is
    log::warn!("Could not determine compression format, returning raw data");
    Ok(data.to_vec())
}

/// Try raw (uncompressed) data
fn try_raw(data: &[u8]) -> Result<Vec<u8>> {
    // Check if the data looks like valid ARM code
    // ARM instructions are 4-byte aligned and have specific patterns
    if data.len() >= 4 {
        let first_word = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        // Check for common ARM instruction patterns
        // STMFD SP!, {...} is a common function prologue
        if (first_word & 0xFFFF0000) == 0xE92D0000 {
            return Ok(data.to_vec());
        }

        // LDR PC, [PC, #-4] is a common trampoline
        if first_word == 0xE51FF004 {
            return Ok(data.to_vec());
        }
    }

    Err(DecompressError::UnknownFormat.into())
}

/// Try LZ77/LZSS decompression
fn try_lz77(data: &[u8]) -> Result<Vec<u8>> {
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
    fn test_raw_detection() {
        // STMFD SP!, {R4-R7, LR}
        let data = vec![0xF0, 0x47, 0x2D, 0xE9];
        assert!(try_raw(&data).is_ok());
    }

    #[test]
    fn test_lz77_basic() {
        // Simple LZ77 test case
        let data = vec![
            0x01, // flags: bit 0 set
            0x00, 0x33, // reference: offset=0, length=3+3=6
            0x41, 0x42, 0x43, // literal "ABC"
        ];
        // This would decompress to something, but the test data is not valid
        // In a real test, we'd use actual compressed data
    }
}
