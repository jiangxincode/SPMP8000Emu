// NGame API implementation module
//
// This module implements the system API functions that SPMP8000 games call
// through the function table. Each API is implemented as a native Rust function
// that manipulates the emulator state.

pub mod emu_graph;
pub mod emu_sound;
pub mod emu_key;
pub mod emu_fs;
pub mod native_ge;

use std::collections::HashMap;
use crate::memory::Memory;

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
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_pitch: u32,
    pub fg_color: [u8; 3], // RGB

    // Audio state
    pub audio_buffer_addr: Option<u32>,
    pub audio_buffer_size: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,

    // Input state
    pub key_state: u32,
    pub key_map: [u32; 20],

    // File system state
    pub open_files: HashMap<i32, FileHandle>,
    pub next_fd: i32,
    pub game_dir: String,

    // Timing state
    pub start_time: u64,
    pub tick_count: u64,

    // Resource table
    pub resource_table: Vec<(String, u32)>, // (name, addr)
}

impl NGameApi {
    /// Create a new API state
    pub fn new() -> Self {
        Self {
            framebuffer_addr: None,
            framebuffer_width: 320,
            framebuffer_height: 240,
            framebuffer_pitch: 640, // width * 2 for RGB565
            fg_color: [255, 255, 255],

            audio_buffer_addr: None,
            audio_buffer_size: 0,
            audio_sample_rate: 22050,
            audio_channels: 1,

            key_state: 0,
            key_map: [0; 20],

            open_files: HashMap::new(),
            next_fd: 3, // 0, 1, 2 reserved for stdin/stdout/stderr
            game_dir: String::from("."),

            start_time: 0,
            tick_count: 0,

            resource_table: Vec::new(),
        }
    }

    /// Set the game directory for file operations
    pub fn set_game_dir(&mut self, dir: &str) {
        self.game_dir = dir.to_string();
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
            0x06 => self.mcatch_init_graph(memory),
            0x0D => self.mcatch_set_framebuffer(memory),
            0x11 => self.mcatch_fill_rect(memory),
            0x12 => self.native_ge_init_res(memory),
            0x14 => self.native_ge_play_res(memory),
            0x17 => self.native_ge_stop_res(memory),
            0x1A => self.native_ge_get_key_input(memory),
            0x1B => self.cyg_thread_delay(memory),
            0x1C => self.native_ge_get_time(memory),
            0x1D => self.native_ge_game_exit(memory),
            0x1F => self.native_ge_fs_open(memory),
            0x20 => self.native_ge_fs_read(memory),
            0x21 => self.native_ge_fs_write(memory),
            0x22 => self.native_ge_fs_close(memory),
            0x23 => self.native_ge_fs_seek(memory),

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
            0x3F => self.emu_if_fs_file_read(memory),
            0x43 => self.emu_if_fs_file_close(memory),

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
