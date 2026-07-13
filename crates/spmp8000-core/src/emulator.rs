// Core emulator implementation
//
// This is the main emulator struct that ties all components together.
// It is platform-independent and shared by both the standalone frontend
// and the libretro core.

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::api::NGameApi;
use crate::arm_cpu::ArmCpu;
use crate::audio_engine::AudioEngine;
use crate::bin_loader::{self, NGameHeader};
use crate::decompressor;
use crate::function_table::FunctionTable;
use crate::input_handler::InputHandler;
use crate::memory::{self, Memory};
use crate::renderer::Renderer;

/// Platform-independent emulator core
#[derive(Debug)]
pub struct Emulator {
    // Game info
    pub header: NGameHeader,
    pub game_path: PathBuf,

    // Core components
    pub cpu: ArmCpu,
    pub memory: Memory,
    pub function_table: FunctionTable,
    pub api: NGameApi,

    // Output
    pub renderer: Renderer,
    pub audio: AudioEngine,
    pub input: InputHandler,

    // State
    pub tick_count: u64,
    pub is_running: bool,
    pub exit_requested: bool,
}

impl Emulator {
    /// Create a new emulator from a game file path
    pub fn from_path(path: PathBuf, _volume: u32) -> Result<Self> {
        log::info!("Loading game: {}", path.display());

        // Read the BIN file
        let data = std::fs::read(&path)
            .with_context(|| format!("Failed to read game file: {}", path.display()))?;

        // Parse header
        let header = bin_loader::parse_header(&data)
            .context("Failed to parse NGame header")?;

        log::info!(
            "Game: {} ({} {}, {})",
            header.game_name,
            header.chip_type,
            header.version,
            header.media_type
        );

        // Extract and decompress data
        let compressed_data = bin_loader::extract_compressed_data(&data)?;
        let decompressed_data = decompressor::decompress(compressed_data)
            .context("Failed to decompress game data")?;

        log::info!(
            "Decompressed: {} bytes (from {} bytes)",
            decompressed_data.len(),
            compressed_data.len()
        );

        // Initialize components
        let cpu = ArmCpu::new().context("Failed to create ARM CPU")?;
        let mut memory = Memory::new();
        memory.init_default().context("Failed to initialize memory")?;

        let function_table = FunctionTable::new();
        let api = NGameApi::new();

        let resolution = header.default_resolution();
        let renderer = Renderer::new(resolution.0, resolution.1);
        let audio = AudioEngine::new(22050);
        let input = InputHandler::new();

        // Create emulator instance
        let mut emu = Self {
            header: header.clone(),
            game_path: path,
            cpu,
            memory,
            function_table,
            api,
            renderer,
            audio,
            input,
            tick_count: 0,
            is_running: false,
            exit_requested: false,
        };

        // Load the game code into memory
        emu.load_code(&decompressed_data)?;

        // Set up function table
        emu.function_table.setup_in_memory(&mut emu.memory)?;

        // Set up CPU
        emu.cpu.set_pc(memory::CODE_LOAD_ADDR)?;
        emu.cpu.set_sp(0x00F00000)?; // Stack at top of RAM

        // Set game directory for file operations
        if let Some(parent) = emu.game_path.parent() {
            emu.api.set_game_dir(&parent.to_string_lossy());
        }

        log::info!("Emulator initialized successfully");
        Ok(emu)
    }

    /// Load game code into memory
    fn load_code(&mut self, code: &[u8]) -> Result<()> {
        // Load code at the standard load address
        let load_addr = memory::CODE_LOAD_ADDR;

        // Map a region for the code
        let code_size = code.len().max(1024 * 1024); // At least 1MB
        self.memory
            .map_region(
                load_addr,
                code_size as u32,
                crate::memory::Permission::ALL,
                "GAME_CODE",
            )
            .context("Failed to map code region")?;

        // Write code to memory
        self.memory
            .write_block(load_addr, code)
            .context("Failed to write game code")?;

        log::info!(
            "Loaded {} bytes of code at 0x{:08X}",
            code.len(),
            load_addr
        );

        Ok(())
    }

    /// Get the game resolution
    pub fn get_resolution(&self) -> (u32, u32) {
        (self.renderer.width, self.renderer.height)
    }

    /// Get the audio sample rate
    pub fn get_audio_sample_rate(&self) -> f64 {
        self.audio.sample_rate as f64
    }

    /// Set button state from external input
    pub fn set_buttons(&mut self, buttons: u32) {
        self.input.set_buttons(buttons);
        self.api.set_key_state(buttons);
    }

    /// Execute one frame (30fps)
    pub fn tick(&mut self) {
        self.tick_count += 1;
        self.api.tick_count = self.tick_count;

        if !self.is_running {
            return;
        }

        // Execute CPU instructions for one frame
        // SPMP8000 runs at ~7.37MHz, so at 30fps that's ~245,760 instructions per frame
        let instructions_per_frame = self.header.cpu_freq() / 30;

        for _ in 0..instructions_per_frame {
            if self.exit_requested {
                break;
            }

            match self.cpu.step(&mut self.memory) {
                Ok(crate::arm_cpu::CpuResult::Continue) => {}
                Ok(crate::arm_cpu::CpuResult::SvcCall(svc_num)) => {
                    // Handle SVC call
                    self.api.handle_svc(svc_num, &mut self.memory);
                }
                Ok(crate::arm_cpu::CpuResult::Halt) => {
                    self.is_running = false;
                    break;
                }
                Err(e) => {
                    log::error!("CPU error: {:?}", e);
                    self.is_running = false;
                    break;
                }
                _ => {}
            }
        }

        // Update renderer
        self.renderer.update_from_memory(&self.memory);

        // Update audio buffer
        if let Some(addr) = self.api.audio_buffer_addr {
            self.audio
                .update_from_memory(&self.memory, addr, self.api.audio_buffer_size);
        }
    }

    /// Start the emulation
    pub fn start(&mut self) {
        self.is_running = true;
        log::info!("Emulation started");
    }

    /// Stop the emulation
    pub fn stop(&mut self) {
        self.is_running = false;
        log::info!("Emulation stopped");
    }

    /// Request exit
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    /// Check if exit was requested
    pub fn should_exit(&self) -> bool {
        self.exit_requested
    }

    /// Get the framebuffer data
    pub fn get_framebuffer(&self) -> &[u8] {
        self.renderer.get_framebuffer()
    }

    /// Get the audio buffer
    pub fn get_audio_samples(&self) -> &[i16] {
        self.audio.get_buffer()
    }

    /// Get tick count
    pub fn get_tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Check if emulation is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emulator_creation() {
        // This test requires a valid BIN file
        // Skip if test file doesn't exist
        let test_path = PathBuf::from("test_data/test.bin");
        if !test_path.exists() {
            return;
        }

        let emu = Emulator::from_path(test_path, 100);
        assert!(emu.is_ok());
    }
}
