// Core emulator implementation
//
// This is the main emulator struct that ties all components together.
// It is platform-independent and shared by both the standalone frontend
// and the libretro core.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

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
    boot_code: Arc<[u8]>,

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
    pub fn from_path(path: PathBuf, volume: u32) -> Result<Self> {
        log::info!("Loading game: {}", path.display());

        // Read the BIN file
        let data = std::fs::read(&path)
            .with_context(|| format!("Failed to read game file: {}", path.display()))?;

        // Parse header
        let header = bin_loader::parse_header(&data).context("Failed to parse NGame header")?;

        log::info!(
            "Game: {} ({} {}, {})",
            header.game_name,
            header.chip_type,
            header.version,
            header.media_type
        );

        // Extract and decompress data
        let compressed_data = bin_loader::extract_compressed_data(&data)?;
        let decompressed_data =
            decompressor::decompress(compressed_data).context("Failed to decompress game data")?;

        log::info!(
            "Decompressed: {} bytes (from {} bytes)",
            decompressed_data.len(),
            compressed_data.len()
        );

        let emu = Self::from_loaded_game(
            path,
            header,
            Arc::from(decompressed_data),
            volume,
            InputHandler::new(),
        )?;

        log::info!("Emulator initialized successfully");
        Ok(emu)
    }

    fn from_loaded_game(
        path: PathBuf,
        header: NGameHeader,
        boot_code: Arc<[u8]>,
        volume: u32,
        mut input: InputHandler,
    ) -> Result<Self> {
        let cpu = ArmCpu::new().context("Failed to create ARM CPU")?;
        let mut memory = Memory::new();
        memory
            .init_default()
            .context("Failed to initialize memory")?;

        let function_table = FunctionTable::new();
        Self::load_code(&mut memory, &boot_code)?;
        function_table.setup_in_memory(&mut memory)?;

        let mut cpu = cpu;
        cpu.set_pc(memory::CODE_LOAD_ADDR)?;
        cpu.set_sp(0x00F00000)?;
        cpu.set_register(0, memory::FUNC_TABLE_BASE)?;

        let mut api = NGameApi::new();
        api.set_cpu_frequency(header.cpu_freq());
        if let Some(parent) = path.parent() {
            api.set_game_dir(&parent.to_string_lossy());
        }

        let resolution = header.default_resolution();
        let renderer = Renderer::new(resolution.0, resolution.1);
        let mut audio = AudioEngine::new(22050);
        audio.set_volume(volume);
        input.clear();

        Ok(Self {
            header,
            game_path: path,
            boot_code,
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
        })
    }

    /// Load game code into memory
    fn load_code(memory: &mut Memory, code: &[u8]) -> Result<()> {
        // Load code at the standard load address
        let load_addr = memory::CODE_LOAD_ADDR;

        // Write code to memory
        memory
            .write_block(load_addr, code)
            .context("Failed to write game code")?;

        log::info!("Loaded {} bytes of code at 0x{:08X}", code.len(), load_addr);

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
        self.api.translate_buttons(buttons);
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

            let cpu_result = self.cpu.step(&mut self.memory);
            self.api.advance_instructions(1);

            match cpu_result {
                Ok(crate::arm_cpu::CpuResult::Continue) => {}
                Ok(crate::arm_cpu::CpuResult::SvcCall(svc_num)) => {
                    self.sync_cpu_registers_to_memory();
                    self.api.handle_svc(svc_num, &mut self.memory);
                    self.sync_memory_registers_to_cpu();
                }
                Ok(crate::arm_cpu::CpuResult::Halt) => {
                    self.is_running = false;
                    break;
                }
                Err(e) => {
                    let pc = self.cpu.get_pc().unwrap_or(0);
                    log::error!("CPU error at PC=0x{:08X}: {:?}", pc, e);
                    log::error!(
                        "regs: r0=0x{:08X} r1=0x{:08X} r2=0x{:08X} r3=0x{:08X} r4=0x{:08X} r5=0x{:08X} r6=0x{:08X} r7=0x{:08X} r8=0x{:08X} r9=0x{:08X} r10=0x{:08X} r11=0x{:08X} r12=0x{:08X} sp=0x{:08X} lr=0x{:08X}",
                        self.cpu.regs.r0,
                        self.cpu.regs.r1,
                        self.cpu.regs.r2,
                        self.cpu.regs.r3,
                        self.cpu.regs.r4,
                        self.cpu.regs.r5,
                        self.cpu.regs.r6,
                        self.cpu.regs.r7,
                        self.cpu.regs.r8,
                        self.cpu.regs.r9,
                        self.cpu.regs.r10,
                        self.cpu.regs.r11,
                        self.cpu.regs.r12,
                        self.cpu.regs.sp,
                        self.cpu.regs.lr
                    );
                    for addr in pc.saturating_sub(16)..pc.saturating_add(16) {
                        if addr % 4 == 0 {
                            if let Ok(instr) = self.memory.read_u32(addr) {
                                log::error!("code 0x{:08X}: 0x{:08X}", addr, instr);
                            }
                        }
                    }
                    self.is_running = false;
                    break;
                }
                _ => {}
            }
        }

        // Update renderer
        if self.renderer.fb_addr != self.api.framebuffer_addr {
            self.renderer
                .set_framebuffer_address(self.api.framebuffer_addr);
        }
        if (1..=640).contains(&self.api.framebuffer_width)
            && (1..=480).contains(&self.api.framebuffer_height)
            && (self.renderer.width != self.api.framebuffer_width
                || self.renderer.height != self.api.framebuffer_height)
        {
            self.renderer
                .set_dimensions(self.api.framebuffer_width, self.api.framebuffer_height);
        }
        self.renderer.update_from_memory(&self.memory);

        for command in self.api.take_audio_commands() {
            self.audio.handle_command(command);
        }
        let streamed_pcm = self
            .api
            .audio_buffer_addr
            .map(|address| (address, self.api.audio_buffer_size, self.api.audio_channels));
        self.audio.render_frame(&self.memory, streamed_pcm);
    }

    fn sync_cpu_registers_to_memory(&mut self) {
        for reg in 0..=15 {
            let value = self.cpu.regs.get(reg as u32);
            self.memory.set_register(reg, value);
        }
        self.memory
            .set_register(memory::REG_CPSR, self.cpu.regs.cpsr);
    }

    fn sync_memory_registers_to_cpu(&mut self) {
        for reg in 0..=15 {
            self.cpu.regs.set(reg as u32, self.memory.get_register(reg));
        }
        self.cpu.regs.cpsr = self.memory.get_register(memory::REG_CPSR);
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

    /// Rebuild all mutable runtime state from the cached boot image.
    pub fn reset(&mut self) -> Result<()> {
        let debug_enabled = self.cpu.debug;
        let volume = self.audio.get_volume();
        let input = self.input.clone();

        let mut replacement = Self::from_loaded_game(
            self.game_path.clone(),
            self.header.clone(),
            Arc::clone(&self.boot_code),
            volume,
            input,
        )
        .context("Failed to rebuild emulator state")?;
        replacement.cpu.debug = debug_enabled;

        *self = replacement;
        log::info!("Emulator reset");
        Ok(())
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
    use crate::api::FileHandle;
    use crate::bin_loader::ChipType;
    use crate::input_handler::BUTTON_O;
    use crate::memory::{FUNC_TABLE_BASE, PERIPHERAL_BASE, RAM_BASE, VRAM_BASE};

    fn test_emulator(volume: u32) -> Emulator {
        let header = NGameHeader {
            magic: *b"NGame1.0",
            flags: 0x8000_0000,
            vendor: "Sunplus".to_string(),
            chip_type: ChipType::SPMP8000,
            game_name: "Reset Test".to_string(),
            media_type: "Sunmedia".to_string(),
            version: "1.0".to_string(),
            code_size: 4,
            file_size: 0x84,
            data_offset: 0x80,
        };

        Emulator::from_loaded_game(
            PathBuf::from("games/reset-test.bin"),
            header,
            Arc::from([0x01, 0x02, 0x03, 0x04]),
            volume,
            InputHandler::new(),
        )
        .unwrap()
    }

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

    #[test]
    fn reset_rebuilds_runtime_state_and_preserves_configuration() {
        let mut emu = test_emulator(37);
        let initial_trampoline = emu.memory.read_u32(FUNC_TABLE_BASE).unwrap();

        emu.input.set_key_mapping(BUTTON_O, Some(0x20));
        emu.input.set_repeat_timing(250, 50);
        emu.set_buttons(1 << BUTTON_O);
        emu.cpu.debug = true;
        emu.cpu.thumb_mode = true;
        emu.cpu.instruction_count = 123;
        emu.cpu.set_register(4, 0xDEAD_BEEF).unwrap();
        emu.memory
            .write_u32(RAM_BASE + 0x1000, 0x1111_1111)
            .unwrap();
        emu.memory.write_u32(VRAM_BASE, 0x2222_2222).unwrap();
        emu.memory.write_u32(PERIPHERAL_BASE, 0x3333_3333).unwrap();
        emu.memory.write_u32(memory::CODE_LOAD_ADDR, 0).unwrap();
        emu.memory.write_u32(FUNC_TABLE_BASE, 0).unwrap();
        emu.api.framebuffer_addr = Some(VRAM_BASE);
        emu.api.audio_buffer_addr = Some(RAM_BASE + 0x2000);
        emu.api.resource_table.push(("sound".to_string(), 0x1234));
        emu.api.open_files.insert(
            3,
            FileHandle {
                host_path: "save.dat".to_string(),
                position: 4,
                size: 8,
                is_writable: true,
            },
        );
        emu.api.next_fd = 4;
        emu.renderer.set_dimensions(160, 120);
        emu.renderer.set_framebuffer_address(Some(VRAM_BASE));
        emu.renderer.get_framebuffer_mut().fill(0xFF);
        emu.audio.set_params(44100, 1);
        emu.audio.get_buffer_mut().extend_from_slice(&[1, 2, 3, 4]);
        emu.tick_count = 42;
        emu.api.tick_count = 42;
        emu.request_exit();
        emu.start();

        emu.reset().unwrap();

        assert!(!emu.is_running());
        assert!(!emu.should_exit());
        assert_eq!(emu.get_tick_count(), 0);
        assert_eq!(emu.cpu.instruction_count, 0);
        assert!(!emu.cpu.thumb_mode);
        assert!(emu.cpu.debug);
        assert_eq!(emu.cpu.get_pc().unwrap(), memory::CODE_LOAD_ADDR);
        assert_eq!(emu.cpu.regs.sp, 0x00F0_0000);
        assert_eq!(emu.cpu.regs.r0, FUNC_TABLE_BASE);
        assert_eq!(
            emu.memory.read_u32(memory::CODE_LOAD_ADDR).unwrap(),
            0x0403_0201
        );
        assert_eq!(
            emu.memory.read_u32(FUNC_TABLE_BASE).unwrap(),
            initial_trampoline
        );
        assert_eq!(emu.memory.read_u32(RAM_BASE + 0x1000).unwrap(), 0);
        assert_eq!(emu.memory.read_u32(VRAM_BASE).unwrap(), 0);
        assert_eq!(emu.memory.read_u32(PERIPHERAL_BASE).unwrap(), 0);
        assert_eq!(emu.api.framebuffer_addr, None);
        assert_eq!(emu.api.audio_buffer_addr, None);
        assert!(emu.api.resource_table.is_empty());
        assert!(emu.api.open_files.is_empty());
        assert_eq!(emu.api.next_fd, 3);
        assert_eq!(emu.api.game_dir, "games");
        assert_eq!(emu.api.get_key_state(), 0);
        assert_eq!(emu.renderer.width, 320);
        assert_eq!(emu.renderer.height, 240);
        assert_eq!(emu.renderer.fb_addr, None);
        assert!(emu.renderer.get_framebuffer().iter().all(|byte| *byte == 0));
        assert_eq!(emu.audio.sample_rate, 22050);
        assert_eq!(emu.audio.channels, 2);
        assert_eq!(emu.audio.get_volume(), 37);
        assert!(emu.audio.get_buffer().is_empty());
        assert_eq!(emu.input.get_buttons(), 0);
        assert_eq!(emu.input.get_key_mapping(BUTTON_O), Some(0x20));
        assert_eq!(emu.input.get_repeat_delay(), 250);
        assert_eq!(emu.input.get_repeat_period(), 50);
    }

    #[test]
    fn reset_uses_the_cached_boot_image_without_rereading_content() {
        let mut emu = test_emulator(100);
        assert!(!emu.game_path.exists());
        emu.memory.write_u32(memory::CODE_LOAD_ADDR, 0).unwrap();

        emu.reset().unwrap();

        assert_eq!(
            emu.memory.read_u32(memory::CODE_LOAD_ADDR).unwrap(),
            0x0403_0201
        );
    }
}
