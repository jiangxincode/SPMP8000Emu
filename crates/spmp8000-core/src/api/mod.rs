// NGame API implementation module
//
// This module implements the system API functions that SPMP8000 games call
// through the function table. Each API is implemented as a native Rust function
// that manipulates the emulator state.

pub mod emu_fs;
pub mod emu_graph;
pub mod emu_key;
pub mod emu_sound;
pub mod native_ge;

use crate::memory::Memory;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Surface {
    pub data_addr: u32,
    pub width: u16,
    pub height: u16,
    pub img_type: u32,
    pub palette_addr: u32,
    pub palette_entries: u16,
}

/// File handle for emulated file system
#[derive(Debug, Clone)]
pub struct FileHandle {
    pub host_path: String,
    pub position: u64,
    pub size: u64,
    pub is_writable: bool,
}

/// NGame API state
#[derive(Debug)]
pub struct NGameApi {
    // Graphics state
    pub framebuffer_addr: Option<u32>,
    pub display_screen_addr: Option<u32>,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_pitch: u32,
    pub fg_color: [u8; 3], // RGB
    pub color_rop: u8,
    pub surfaces: HashMap<u8, Surface>,
    pub next_surface_id: u8,

    // Audio state
    pub audio_buffer_addr: Option<u32>,
    pub audio_buffer_size: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,

    // Input state
    pub raw_key_state: u32,
    pub key_state: u32,
    pub key_map: [u32; 20],

    // File system state
    pub open_files: HashMap<i32, FileHandle>,
    pub next_fd: i32,
    pub game_dir: String,

    // Timing state
    pub start_time: u64,
    pub tick_count: u64,
    elapsed_instructions: u64,
    cpu_frequency: u32,

    // Resource table
    pub resource_table: Vec<(String, u32)>, // (name, addr)
}

impl NGameApi {
    /// Create a new API state
    pub fn new() -> Self {
        Self {
            framebuffer_addr: None,
            display_screen_addr: None,
            framebuffer_width: 320,
            framebuffer_height: 240,
            framebuffer_pitch: 640, // width * 2 for RGB565
            fg_color: [255, 255, 255],
            color_rop: 0xF0,
            surfaces: HashMap::new(),
            next_surface_id: 1,

            audio_buffer_addr: None,
            audio_buffer_size: 0,
            audio_sample_rate: 22050,
            audio_channels: 1,

            raw_key_state: 0,
            key_state: 0,
            key_map: [0; 20],

            open_files: HashMap::new(),
            next_fd: 3, // 0, 1, 2 reserved for stdin/stdout/stderr
            game_dir: String::from("."),

            start_time: 0,
            tick_count: 0,
            elapsed_instructions: 0,
            cpu_frequency: 1,

            resource_table: Vec::new(),
        }
    }

    /// Set the game directory for file operations
    pub fn set_game_dir(&mut self, dir: &str) {
        self.game_dir = dir.to_string();
    }

    pub(crate) fn set_cpu_frequency(&mut self, cpu_frequency: u32) {
        self.cpu_frequency = cpu_frequency.max(1);
    }

    pub(crate) fn advance_instructions(&mut self, instructions: u64) {
        self.elapsed_instructions = self.elapsed_instructions.saturating_add(instructions);
    }

    pub(crate) fn emulated_time_ms(&self) -> u32 {
        (self.elapsed_instructions.saturating_mul(1000) / u64::from(self.cpu_frequency)) as u32
    }

    /// Update key state
    pub fn set_key_state(&mut self, state: u32) {
        self.key_state = state;
    }

    /// Get current key state
    pub fn get_key_state(&self) -> u32 {
        self.key_state
    }

    /// Allocate a new file descriptor
    fn allocate_fd(&mut self) -> i32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        fd
    }

    /// Get file handle by descriptor
    pub fn get_file(&self, fd: i32) -> Option<&FileHandle> {
        self.open_files.get(&fd)
    }

    /// Get mutable file handle by descriptor
    pub fn get_file_mut(&mut self, fd: i32) -> Option<&mut FileHandle> {
        self.open_files.get_mut(&fd)
    }

    /// Close a file
    pub fn close_file(&mut self, fd: i32) -> bool {
        self.open_files.remove(&fd).is_some()
    }

    fn return_success(&mut self, memory: &mut Memory) {
        memory.set_register(crate::memory::REG_R0, 0);
    }
    /// Handle an SVC call
    pub fn handle_svc(&mut self, svc_num: u32, memory: &mut Memory) {
        match svc_num {
            // NativeGE functions
            0x01 => {
                // diag_printf - just log the message
                let fmt_addr = memory.get_register(crate::memory::REG_R0);
                if let Ok(msg) = memory.read_string(fmt_addr, 256) {
                    log::info!("[diag_printf] {}", msg);
                }
                memory.set_register(crate::memory::REG_R0, 0);
            }
            0x02 => self.return_success(memory), // MCatchFlush
            0x03 => self.mcatch_update_screen(memory), // MCatchPaint
            0x04 => self.mcatch_load_image(memory),
            0x05 => self.mcatch_free_image(memory),
            0x06 => self.mcatch_init_graph(memory),
            0x07 => self.mcatch_set_color_rop(memory),
            0x08 => self.return_success(memory), // MCatchGetColorROP
            0x09 => self.mcatch_set_fg_color(memory),
            0x0A => self.return_success(memory), // MCatchGetFGColor
            0x0B => self.mcatch_set_display_screen(memory),
            0x0C => self.mcatch_get_display_screen(memory),
            0x25 => self.mcatch_set_alpha_blend(memory),
            0x26 => self.mcatch_get_alpha_blend(memory),
            0x27 => self.mcatch_enable_feature(memory),
            0x28 => self.mcatch_disable_feature(memory),
            0x29 => self.mcatch_set_camera_mode(memory),
            0x0D => self.mcatch_set_framebuffer(memory),
            0x0E => self.mcatch_get_framebuffer(memory),
            0x0F => self.mcatch_bitblt(memory),
            0x10 => self.mcatch_sprite(memory),
            0x11 => self.mcatch_fill_rect(memory),
            0x2A => self.mcatch_enable_double_buffer(memory),
            0x2B => self.mcatch_update_screen(memory),
            0x12 => self.native_ge_init_res(memory),
            0x13 => self.native_ge_get_res(memory),
            0x14 => self.native_ge_play_res(memory),
            0x15 => self.return_success(memory), // NativeGEPauseRes
            0x16 => self.return_success(memory), // NativeGEResumeRes
            0x17 => self.native_ge_stop_res(memory),
            0x18 => self.native_ge_write_record(memory),
            0x19 => self.native_ge_read_record(memory),
            0x1A => self.native_ge_get_key_input(memory),
            0x2C => self.native_ge_show_fps(memory),
            0x1B => self.cyg_thread_delay(memory),
            0x1C => self.native_ge_get_time(memory),
            0x1D => self.native_ge_game_exit(memory),
            0x1F => self.native_ge_fs_open(memory),
            0x20 => self.native_ge_fs_read(memory),
            0x21 => self.native_ge_fs_write(memory),
            0x22 => self.native_ge_fs_close(memory),
            0x23 => self.native_ge_fs_seek(memory),
            0x10A4 => self.mcatch_query_image(memory),

            // emuIf functions
            0x30 => self.emu_if_graph_init(memory),
            0x31 => self.emu_if_graph_show(memory),
            0x33 => self.emu_if_graph_cleanup(memory),
            0x34 => self.emu_if_sound_init(memory),
            0x35 => self.emu_if_sound_play(memory),
            0x36 => self.emu_if_sound_cleanup(memory),
            0x37 => self.emu_if_key_init(memory),
            0x38 => self.emu_if_key_get_input(memory),
            0x3C => self.emu_if_fs_file_open(memory),
            0x3D => self.emu_if_fs_file_get_size(memory),
            0x3E => self.emu_if_fs_file_write(memory),
            0x3F => self.emu_if_fs_file_read(memory),
            0x40 => self.emu_if_fs_file_get_char(memory),
            0x41 => self.emu_if_fs_file_seek(memory),
            0x42 => self.emu_if_fs_file_cur_pos(memory),
            0x43 => self.emu_if_fs_file_close(memory),

            0x1000..=0x2000 => {
                log::debug!(
                    "Default native function at table offset 0x{:03X}: r0=0x{:08X} r1=0x{:08X} r2=0x{:08X} r3=0x{:08X}",
                    svc_num - 0x1000,
                    memory.get_register(crate::memory::REG_R0),
                    memory.get_register(crate::memory::REG_R1),
                    memory.get_register(crate::memory::REG_R2),
                    memory.get_register(crate::memory::REG_R3)
                );
                self.return_success(memory);
            }
            _ => {
                log::warn!("Unhandled SVC call: 0x{:02X}", svc_num);
            }
        }
    }
}

impl Default for NGameApi {
    fn default() -> Self {
        Self::new()
    }
}
