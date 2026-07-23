// Save state management for SPMP8000 emulator
//
// Provides a fixed-size binary serialization format for libretro save states.
// Layout: [MAGIC 4B] [VERSION 4B] [PAYLOAD_LEN 4B] [CRC32 4B] [JSON payload ...] [zero padding]

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::arm_cpu::ArmRegisters;
use crate::renderer::PixelFormat;

const MAGIC: &[u8; 4] = b"SPM8";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 16;

/// Fixed serialization buffer size.  libretro requires this value to remain
/// constant for a loaded core.  8 MiB is generous for a JSON snapshot of the
/// SPMP8000 state (typical payloads are well under 1 MiB).
pub(crate) const SERIALIZED_SIZE: usize = 8 * 1024 * 1024;

/// Snapshot of the complete emulator state, serialized as JSON inside the
/// fixed-size binary envelope.
#[derive(Serialize, Deserialize)]
pub(crate) struct EmulatorState {
    // CPU
    pub registers: ArmRegisters,
    // Memory regions (base + data for each mapped region)
    pub memory_regions: Vec<MemoryRegionSnapshot>,
    // Renderer
    pub renderer: RendererState,
    // Audio
    pub audio: AudioState,
    // Timing
    pub tick_count: u64,
    pub is_running: bool,
    // API graphics state
    pub api: ApiState,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct MemoryRegionSnapshot {
    pub base: u32,
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RendererState {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub fb_addr: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AudioState {
    pub sample_rate: u32,
    pub channels: u32,
    pub volume: f32,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ApiState {
    pub framebuffer_addr: Option<u32>,
    pub display_screen_addr: Option<u32>,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_pitch: u32,
    pub fg_color: [u8; 3],
    pub color_rop: u8,
    pub audio_buffer_addr: Option<u32>,
    pub audio_buffer_size: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub raw_key_state: u32,
    pub key_state: u32,
    pub elapsed_instructions: u64,
    pub cpu_frequency: u32,
}

// ---------------------------------------------------------------------------
// Encode / Decode
// ---------------------------------------------------------------------------

/// Encode the emulator state into a fixed-size buffer.
pub(crate) fn encode(state: &EmulatorState, output: &mut [u8]) -> Result<()> {
    if output.len() < SERIALIZED_SIZE {
        bail!(
            "save-state buffer is too small: got {}, need {}",
            output.len(),
            SERIALIZED_SIZE
        );
    }

    let payload = serde_json::to_vec(state).context("failed to encode save state")?;
    if payload.len() > SERIALIZED_SIZE - HEADER_SIZE {
        bail!("save state exceeds the fixed serialization capacity");
    }

    output.fill(0);
    output[..4].copy_from_slice(MAGIC);
    output[4..8].copy_from_slice(&VERSION.to_le_bytes());
    output[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    output[12..16].copy_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    output[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(&payload);
    Ok(())
}

/// Decode the emulator state from a fixed-size buffer.
pub(crate) fn decode(input: &[u8]) -> Result<EmulatorState> {
    if input.len() < HEADER_SIZE {
        bail!("save state is truncated");
    }
    if &input[..4] != MAGIC {
        bail!("invalid save-state signature");
    }

    let version = u32::from_le_bytes(input[4..8].try_into().unwrap());
    if version != VERSION {
        bail!("unsupported save-state version {version}");
    }

    let payload_len = u32::from_le_bytes(input[8..12].try_into().unwrap()) as usize;
    let expected_crc = u32::from_le_bytes(input[12..16].try_into().unwrap());
    let payload_end = HEADER_SIZE
        .checked_add(payload_len)
        .filter(|&end| end <= input.len() && end <= SERIALIZED_SIZE)
        .context("invalid save-state payload length")?;
    let payload = &input[HEADER_SIZE..payload_end];
    if crc32fast::hash(payload) != expected_crc {
        bail!("save-state checksum mismatch");
    }

    serde_json::from_slice(payload).context("failed to decode save state")
}

// ---------------------------------------------------------------------------
// Snapshot helpers — implemented on Emulator so the libretro layer stays thin
// ---------------------------------------------------------------------------

impl super::emulator::Emulator {
    /// Fixed buffer size required by libretro for save states.
    pub fn serialize_size(&self) -> usize {
        SERIALIZED_SIZE
    }

    /// Serialize the full emulator state into `buffer`.
    pub fn serialize(&self, buffer: &mut [u8]) -> Result<()> {
        let state = self.snapshot();
        encode(&state, buffer)
    }

    /// Restore the emulator state from `buffer`.
    pub fn deserialize(&mut self, buffer: &[u8]) -> Result<()> {
        let state = decode(buffer)?;
        self.restore(state);
        Ok(())
    }

    /// Build a snapshot of the current emulator state.
    fn snapshot(&self) -> EmulatorState {
        let memory_regions = self
            .memory
            .regions()
            .iter()
            .map(|r| MemoryRegionSnapshot {
                base: r.base,
                name: r.name.clone(),
                data: r.data.clone(),
            })
            .collect();

        EmulatorState {
            registers: self.cpu.regs.clone(),
            memory_regions,
            renderer: RendererState {
                width: self.renderer.width,
                height: self.renderer.height,
                format: self.renderer.format,
                fb_addr: self.renderer.fb_addr,
            },
            audio: AudioState {
                sample_rate: self.audio.sample_rate,
                channels: self.audio.channels,
                volume: self.audio.volume,
            },
            tick_count: self.tick_count,
            is_running: self.is_running,
            api: ApiState {
                framebuffer_addr: self.api.framebuffer_addr,
                display_screen_addr: self.api.display_screen_addr,
                framebuffer_width: self.api.framebuffer_width,
                framebuffer_height: self.api.framebuffer_height,
                framebuffer_pitch: self.api.framebuffer_pitch,
                fg_color: self.api.fg_color,
                color_rop: self.api.color_rop,
                audio_buffer_addr: self.api.audio_buffer_addr,
                audio_buffer_size: self.api.audio_buffer_size,
                audio_sample_rate: self.api.audio_sample_rate,
                audio_channels: self.api.audio_channels,
                raw_key_state: self.api.raw_key_state,
                key_state: self.api.key_state,
                elapsed_instructions: self.api.elapsed_instructions,
                cpu_frequency: self.api.cpu_frequency,
            },
        }
    }

    /// Restore emulator state from a snapshot.
    fn restore(&mut self, state: EmulatorState) {
        // CPU registers
        self.cpu.regs = state.registers;

        // Memory regions — restore data for matching regions by base+name
        for snap in &state.memory_regions {
            if let Some(region) = self
                .memory
                .regions_mut()
                .iter_mut()
                .find(|r| r.base == snap.base && r.name == snap.name)
            {
                let len = region.data.len().min(snap.data.len());
                region.data[..len].copy_from_slice(&snap.data[..len]);
            }
        }

        // Renderer
        self.renderer.width = state.renderer.width;
        self.renderer.height = state.renderer.height;
        self.renderer.format = state.renderer.format;
        self.renderer.fb_addr = state.renderer.fb_addr;
        self.renderer
            .set_dimensions(state.renderer.width, state.renderer.height);

        // Audio — restore parameters; active playbacks are lost and will be
        // re-populated by the game's audio commands on subsequent frames.
        self.audio
            .set_params(state.audio.sample_rate, state.audio.channels);
        self.audio.volume = state.audio.volume;

        // Timing
        self.tick_count = state.tick_count;
        self.is_running = state.is_running;

        // API state
        self.api.framebuffer_addr = state.api.framebuffer_addr;
        self.api.display_screen_addr = state.api.display_screen_addr;
        self.api.framebuffer_width = state.api.framebuffer_width;
        self.api.framebuffer_height = state.api.framebuffer_height;
        self.api.framebuffer_pitch = state.api.framebuffer_pitch;
        self.api.fg_color = state.api.fg_color;
        self.api.color_rop = state.api.color_rop;
        self.api.audio_buffer_addr = state.api.audio_buffer_addr;
        self.api.audio_buffer_size = state.api.audio_buffer_size;
        self.api.audio_sample_rate = state.api.audio_sample_rate;
        self.api.audio_channels = state.api.audio_channels;
        self.api.raw_key_state = state.api.raw_key_state;
        self.api.key_state = state.api.key_state;
        self.api.elapsed_instructions = state.api.elapsed_instructions;
        self.api.cpu_frequency = state.api.cpu_frequency;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> EmulatorState {
        EmulatorState {
            registers: ArmRegisters::new(),
            memory_regions: vec![MemoryRegionSnapshot {
                base: 0,
                name: "RAM".into(),
                data: vec![0u8; 64],
            }],
            renderer: RendererState {
                width: 320,
                height: 240,
                format: PixelFormat::RGB565,
                fb_addr: None,
            },
            audio: AudioState {
                sample_rate: 22050,
                channels: 2,
                volume: 1.0,
            },
            tick_count: 42,
            is_running: true,
            api: ApiState {
                framebuffer_addr: None,
                display_screen_addr: None,
                framebuffer_width: 320,
                framebuffer_height: 240,
                framebuffer_pitch: 640,
                fg_color: [255, 255, 255],
                color_rop: 0xF0,
                audio_buffer_addr: None,
                audio_buffer_size: 0,
                audio_sample_rate: 22050,
                audio_channels: 1,
                raw_key_state: 0,
                key_state: 0,
                elapsed_instructions: 0,
                cpu_frequency: 7372800,
            },
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let state = test_state();
        let mut buf = vec![0u8; SERIALIZED_SIZE];
        encode(&state, &mut buf).unwrap();
        let restored = decode(&buf).unwrap();
        assert_eq!(restored.tick_count, 42);
        assert_eq!(restored.renderer.width, 320);
        assert_eq!(restored.audio.sample_rate, 22050);
    }

    #[test]
    fn decode_detects_corruption() {
        let state = test_state();
        let mut buf = vec![0u8; SERIALIZED_SIZE];
        encode(&state, &mut buf).unwrap();
        // Flip a byte in the payload area
        buf[HEADER_SIZE] ^= 0xFF;
        assert!(decode(&buf).is_err());
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let mut buf = vec![0u8; SERIALIZED_SIZE];
        buf[..4].copy_from_slice(b"NOPE");
        assert!(decode(&buf).is_err());
    }
}
