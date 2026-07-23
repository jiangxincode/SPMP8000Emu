use crate::api::NGameApi;
use crate::arm_cpu::ArmCpu;
use crate::audio_engine::AudioEngine;
use crate::input_handler::InputHandler;
use crate::memory::Memory;
use crate::renderer::Renderer;
use anyhow::{bail, Context, Result};
use bincode::Options;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"SPM8STAT";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 32;
const MAX_DECODED_SIZE: usize = 256 * 1024 * 1024;

/// libretro requires this value to remain constant while content is loaded.
pub const SERIALIZED_SIZE: usize = 128 * 1024 * 1024;

pub(crate) const MEMORY_LAYOUT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub(crate) struct EmulatorState {
    pub memory_layout_version: u32,
    pub cpu: ArmCpu,
    pub memory: Memory,
    pub api: NGameApi,
    pub renderer: Renderer,
    pub audio: AudioEngine,
    pub input: InputHandler,
    pub tick_count: u64,
    pub is_running: bool,
    pub exit_requested: bool,
}

#[derive(Serialize)]
pub(crate) struct EmulatorStateRef<'a> {
    pub memory_layout_version: u32,
    pub cpu: &'a ArmCpu,
    pub memory: &'a Memory,
    pub api: &'a NGameApi,
    pub renderer: &'a Renderer,
    pub audio: &'a AudioEngine,
    pub input: &'a InputHandler,
    pub tick_count: u64,
    pub is_running: bool,
    pub exit_requested: bool,
}

pub(crate) fn encode(
    state: &EmulatorStateRef<'_>,
    content_crc32: u32,
    output: &mut [u8],
) -> Result<()> {
    if output.len() < SERIALIZED_SIZE {
        bail!(
            "save-state buffer is too small: got {}, need {}",
            output.len(),
            SERIALIZED_SIZE
        );
    }

    let decoded = codec()
        .serialize(state)
        .context("failed to encode save-state payload")?;
    if decoded.len() > MAX_DECODED_SIZE {
        bail!("save-state payload exceeds the decoded size limit");
    }

    let payload = lz4_flex::compress(&decoded);
    if payload.len() > SERIALIZED_SIZE - HEADER_SIZE {
        bail!("save state exceeds the fixed serialization capacity");
    }

    output.fill(0);
    output[..8].copy_from_slice(MAGIC);
    output[8..12].copy_from_slice(&VERSION.to_le_bytes());
    output[12..16].copy_from_slice(&content_crc32.to_le_bytes());
    output[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    output[20..24].copy_from_slice(&(decoded.len() as u32).to_le_bytes());
    output[24..28].copy_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    output[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(&payload);
    Ok(())
}

pub(crate) fn decode(input: &[u8], expected_content_crc32: u32) -> Result<EmulatorState> {
    decode_value(input, expected_content_crc32)
}

fn decode_value<T: DeserializeOwned>(input: &[u8], expected_content_crc32: u32) -> Result<T> {
    if input.len() < HEADER_SIZE {
        bail!("save state is truncated");
    }
    if &input[..8] != MAGIC {
        bail!("invalid save-state signature");
    }

    let version = read_u32(input, 8);
    if version != VERSION {
        bail!("unsupported save-state version {version}");
    }

    let content_crc32 = read_u32(input, 12);
    if content_crc32 != expected_content_crc32 {
        bail!("save state belongs to different content data");
    }

    let payload_len = read_u32(input, 16) as usize;
    let decoded_len = read_u32(input, 20) as usize;
    let expected_payload_crc32 = read_u32(input, 24);
    if decoded_len > MAX_DECODED_SIZE {
        bail!("save-state decoded size exceeds the limit");
    }

    let payload_end = HEADER_SIZE
        .checked_add(payload_len)
        .filter(|&end| end <= input.len() && end <= SERIALIZED_SIZE)
        .context("invalid save-state payload length")?;
    let payload = &input[HEADER_SIZE..payload_end];
    if crc32fast::hash(payload) != expected_payload_crc32 {
        bail!("save-state checksum mismatch");
    }

    let decoded =
        lz4_flex::decompress(payload, decoded_len).context("failed to decompress save state")?;
    codec()
        .with_limit(MAX_DECODED_SIZE as u64)
        .deserialize(&decoded)
        .context("failed to decode save state")
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestValue {
        text: String,
        number: u64,
    }

    fn encoded_value() -> Vec<u8> {
        let value = TestValue {
            text: "SPMP8000".to_string(),
            number: 42,
        };
        let decoded = codec().serialize(&value).unwrap();
        let payload = lz4_flex::compress(&decoded);
        let mut output = vec![0u8; HEADER_SIZE + payload.len()];
        output[..8].copy_from_slice(MAGIC);
        output[8..12].copy_from_slice(&VERSION.to_le_bytes());
        output[12..16].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        output[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        output[20..24].copy_from_slice(&(decoded.len() as u32).to_le_bytes());
        output[24..28].copy_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        output[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(&payload);
        output
    }

    #[test]
    fn codec_round_trip_and_checksum() {
        let mut output = encoded_value();
        assert_eq!(
            decode_value::<TestValue>(&output, 0x1234_5678).unwrap(),
            TestValue {
                text: "SPMP8000".to_string(),
                number: 42,
            }
        );

        output[HEADER_SIZE] ^= 1;
        assert!(decode_value::<TestValue>(&output, 0x1234_5678).is_err());
    }

    #[test]
    fn codec_rejects_wrong_content_version_and_truncation() {
        let mut output = encoded_value();
        assert!(decode_value::<TestValue>(&output, 0x8765_4321).is_err());

        output[8..12].copy_from_slice(&2u32.to_le_bytes());
        assert!(decode_value::<TestValue>(&output, 0x1234_5678).is_err());
        assert!(decode_value::<TestValue>(&output[..HEADER_SIZE - 1], 0x1234_5678).is_err());
    }
}
