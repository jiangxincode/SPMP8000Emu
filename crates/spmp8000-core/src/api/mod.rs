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

use crate::audio_resource::AudioCommand;
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

#[derive(Debug, Clone, Copy)]
struct GraphicsTransformation {
    reference_x: i32,
    reference_y: i32,
    kind: u8,
}

/// File handle for emulated file system
#[derive(Debug, Clone)]
pub struct FileHandle {
    pub host_path: String,
    pub position: u64,
    pub size: u64,
    pub is_writable: bool,
}

#[derive(Debug, Clone)]
pub struct DirHandle {
    pub entries: Vec<String>,
    pub position: usize,
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
    pending_transformation: Option<GraphicsTransformation>,

    // Audio state
    pub audio_buffer_addr: Option<u32>,
    pub audio_buffer_size: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub(crate) audio_commands: Vec<AudioCommand>,

    // Input state
    pub raw_key_state: u32,
    pub key_state: u32,
    pub key_map: [u32; 20],

    // File system state
    pub open_files: HashMap<i32, FileHandle>,
    pub open_dirs: HashMap<u32, DirHandle>,
    pub next_fd: i32,
    pub next_dir_handle: u32,
    pub game_dir: String,
    pub current_dir: String,

    // Timing state
    pub start_time: u64,
    pub tick_count: u64,
    elapsed_instructions: u64,
    cpu_frequency: u32,

    // Resource table
    pub resource_table: Vec<(String, u32)>, // (name, addr)

    // Legacy runtime heap for games that call C library helpers through slot 0.
    legacy_heap_next: u32,
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
            pending_transformation: None,

            audio_buffer_addr: None,
            audio_buffer_size: 0,
            audio_sample_rate: 22050,
            audio_channels: 1,
            audio_commands: Vec::new(),

            raw_key_state: 0,
            key_state: 0,
            key_map: [0; 20],

            open_files: HashMap::new(),
            open_dirs: HashMap::new(),
            next_fd: 3, // 0, 1, 2 reserved for stdin/stdout/stderr
            next_dir_handle: crate::memory::KEY_STATE_ADDR + 0x400,
            game_dir: String::from("."),
            current_dir: String::from("/GAME"),

            start_time: 0,
            tick_count: 0,
            elapsed_instructions: 0,
            cpu_frequency: 1,

            resource_table: Vec::new(),
            legacy_heap_next: 0x0040_0000,
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

    pub(crate) fn take_audio_commands(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.audio_commands)
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

    fn handle_legacy_native_zero(&mut self, memory: &mut Memory) {
        let r0 = memory.get_register(crate::memory::REG_R0);
        let r1 = memory.get_register(crate::memory::REG_R1);
        let r2 = memory.get_register(crate::memory::REG_R2);
        let r3 = memory.get_register(crate::memory::REG_R3);
        if r0 <= 0x10000 && r1 <= 0x10000 && r2 <= 0x10000 && (r0 != 0 || r1 != 0 || r2 != 0) {
            let size = r0.saturating_mul(r1).max(r2).max(4);
            let addr = self.legacy_heap_next;
            self.legacy_heap_next = (self.legacy_heap_next + size + 3) & !3;
            for offset in 0..size {
                let _ = memory.write_u8(addr + offset, 0);
            }
            log::debug!("Legacy native alloc: size={} addr=0x{:08X}", size, addr);
            memory.set_register(crate::memory::REG_R0, addr);
            return;
        }

        if (1..=4096).contains(&r1)
            && (0x0010_0000..=0x00ff_ffff).contains(&r0)
            && (0x0010_0000..=0x00ff_ffff).contains(&r2)
        {
            if let Ok(bytes) = memory.read_block(r2, r1 as usize) {
                let _ = memory.write_block(r0, &bytes);
                log::debug!(
                    "Legacy native memcpy: dst=0x{:08X} src=0x{:08X} size={}",
                    r0,
                    r2,
                    r1
                );
                memory.set_register(crate::memory::REG_R0, r0);
                return;
            }
        }

        if r1 == 0x100 && (r2 == 0 || r2 >= 0xFE00_0000) && memory.read_u8(r0).is_ok() {
            let existing = memory
                .read_string(r0, r1.min(64) as usize)
                .unwrap_or_default();
            if existing != "." && existing != ".." && !existing.starts_with('/') {
                let cwd = b"/GAME\0";
                for (i, byte) in cwd.iter().enumerate() {
                    let _ = memory.write_u8(r0 + i as u32, *byte);
                }
                log::debug!("Legacy native getcwd: buf=0x{:08X} size={}", r0, r1);
                memory.set_register(crate::memory::REG_R0, r0);
                return;
            }

            log::debug!("Legacy native chdir-like call: path={}", existing);
            self.return_success(memory);
            return;
        }

        log::debug!(
            "Default native function at table offset 0x000: r0=0x{:08X} r1=0x{:08X} r2=0x{:08X} r3=0x{:08X}",
            r0,
            r1,
            r2,
            r3
        );
        self.return_success(memory);
    }

    fn ecos_host_path(&self, pathname: &str) -> std::path::PathBuf {
        let normalized = pathname.replace('\\', "/");
        let without_device = normalized
            .strip_prefix("/fat20a2/hda2")
            .or_else(|| normalized.strip_prefix("/hda2"))
            .unwrap_or(&normalized);
        let relative = without_device.trim_start_matches('/');
        let game_dir = std::path::PathBuf::from(&self.game_dir);
        let game_name = game_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        let base = if without_device.starts_with('/') {
            if let Some(rest) = relative.strip_prefix("GAME/") {
                if let Some(rest) = rest
                    .strip_prefix(game_name)
                    .and_then(|s| s.strip_prefix('/'))
                {
                    game_dir.join(rest)
                } else if rest.eq_ignore_ascii_case(game_name) || rest.is_empty() {
                    game_dir.clone()
                } else {
                    game_dir
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(rest)
                }
            } else {
                game_dir.join(relative)
            }
        } else if self
            .current_dir
            .trim_matches('/')
            .eq_ignore_ascii_case("GAME")
        {
            game_dir
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(relative)
        } else {
            game_dir.join(relative)
        };

        let direct = base;
        if direct.exists() {
            return direct;
        }

        let lower = relative.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "heretic.wad"
                | "heretic1.wad"
                | "heretic/data/heretic.wad"
                | "heretic/data/heretic1.wad"
        ) {
            let local = std::path::Path::new(&self.game_dir).join("heretic1.wad");
            if local.exists() {
                return local;
            }
            if let Some(parent) = std::path::Path::new(&self.game_dir).parent() {
                let sibling = parent.join("Heretic").join("doom1.wad");
                if sibling.exists() {
                    return sibling;
                }
            }
        }

        if matches!(lower.as_str(), "doom1.wad" | "doom.wad") {
            if let Some(parent) = std::path::Path::new(&self.game_dir).parent() {
                let sibling = parent.join("Heretic").join("doom1.wad");
                if sibling.exists() {
                    return sibling;
                }
            }
        }

        if matches!(lower.as_str(), "id1/pak0.pak" | "pak0.pak") {
            if let Some(parent) = std::path::Path::new(&self.game_dir).parent() {
                let sibling = parent.join("Quake").join("id1").join("pak0.pak");
                if sibling.exists() {
                    return sibling;
                }
            }
        }

        direct
    }

    fn default_keymap_data(path: &std::path::Path) -> Option<Vec<u8>> {
        const MAPPINGS: [u32; 36] = [
            0x0001, 0x0002, 0x0004, 0x0008, 0x0010, 0x0020, 0x0040, 0x0080, 0x0100, 0x0200, 0x0400,
            0x0800, 0, 0, 0, 0, 0, 0, 0, 0, 0x0020, 0x0040, 0x0010, 0x0080, 0x0200, 0x0100, 0x0008,
            0x0004, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let filename = path.file_name()?.to_str()?;
        let mut data = Vec::with_capacity(if filename.eq_ignore_ascii_case("keymap.cfg") {
            148
        } else if filename.eq_ignore_ascii_case("keys.map") {
            144
        } else {
            return None;
        });

        if filename.eq_ignore_ascii_case("keymap.cfg") {
            data.extend_from_slice(&0xFFF0_0001u32.to_le_bytes());
        }
        for mapping in MAPPINGS {
            data.extend_from_slice(&mapping.to_le_bytes());
        }
        Some(data)
    }

    fn normalize_ecos_dir(path: &str) -> String {
        let mut parts = Vec::new();
        let normalized = path.replace('\\', "/");
        for part in normalized.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                _ => parts.push(part),
            }
        }
        format!("/{}", parts.join("/"))
    }

    fn ecos_open(&mut self, memory: &mut Memory) {
        let pathname_addr = memory.get_register(crate::memory::REG_R0);
        let flags = memory.get_register(crate::memory::REG_R1);
        let pathname = memory.read_string(pathname_addr, 256).unwrap_or_default();
        let host_path = self.ecos_host_path(&pathname);
        log::debug!("eCos open: {} -> {}", pathname, host_path.display());

        let default_size = (!host_path.exists())
            .then(|| Self::default_keymap_data(&host_path))
            .flatten()
            .map(|data| data.len() as u64);
        if host_path.exists() || default_size.is_some() || (flags & (1 << 3)) != 0 {
            let file = FileHandle {
                host_path: host_path.to_string_lossy().to_string(),
                position: 0,
                size: std::fs::metadata(&host_path)
                    .map(|m| m.len())
                    .unwrap_or_else(|_| default_size.unwrap_or(0)),
                is_writable: (flags & 2) != 0,
            };
            let fd = self.allocate_fd();
            self.open_files.insert(fd, file);
            memory.set_register(crate::memory::REG_R0, fd as u32);
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX);
        }
    }

    fn ecos_read(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let buf_addr = memory.get_register(crate::memory::REG_R1);
        let count = memory.get_register(crate::memory::REG_R2);
        if let Some(file) = self.open_files.get_mut(&fd) {
            let host_path = std::path::Path::new(&file.host_path);
            if !host_path.exists() {
                if let Some(data) = Self::default_keymap_data(host_path) {
                    let start = file.position.min(data.len() as u64) as usize;
                    let end = start.saturating_add(count as usize).min(data.len());
                    let bytes = &data[start..end];
                    log::debug!(
                        "eCos read: using built-in {} ({} bytes at offset {})",
                        host_path.display(),
                        bytes.len(),
                        start
                    );
                    let _ = memory.write_block(buf_addr, bytes);
                    file.position = end as u64;
                    memory.set_register(crate::memory::REG_R0, bytes.len() as u32);
                    return;
                }
            }
            if let Ok(mut host_file) = std::fs::File::open(&file.host_path) {
                use std::io::{Read, Seek};
                let _ = host_file.seek(std::io::SeekFrom::Start(file.position));
                let mut buffer = vec![0u8; count as usize];
                if let Ok(bytes_read) = host_file.read(&mut buffer) {
                    for (i, byte) in buffer.iter().take(bytes_read).enumerate() {
                        let _ = memory.write_u8(buf_addr + i as u32, *byte);
                    }
                    file.position += bytes_read as u64;
                    memory.set_register(crate::memory::REG_R0, bytes_read as u32);
                    return;
                }
            }
        }
        memory.set_register(crate::memory::REG_R0, u32::MAX);
    }

    fn ecos_write(&mut self, memory: &mut Memory) {
        let count = memory.get_register(crate::memory::REG_R2);
        memory.set_register(crate::memory::REG_R0, count);
    }

    fn ecos_close(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        self.open_files.remove(&fd);
        self.return_success(memory);
    }

    fn ecos_lseek(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let offset = memory.get_register(crate::memory::REG_R1) as i32;
        let whence = memory.get_register(crate::memory::REG_R2);
        if let Some(file) = self.open_files.get_mut(&fd) {
            let base = match whence {
                0 => 0,
                1 => file.position as i64,
                2 => file.size as i64,
                _ => {
                    memory.set_register(crate::memory::REG_R0, u32::MAX);
                    return;
                }
            };
            file.position = (base + offset as i64).max(0) as u64;
            memory.set_register(crate::memory::REG_R0, file.position as u32);
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX);
        }
    }

    fn write_ecos_stat(memory: &mut Memory, stat_addr: u32, size: u64, is_dir: bool) {
        let mode = if is_dir { 1 } else { 1 << 3 };
        let _ = memory.write_u32(stat_addr, mode);
        let _ = memory.write_u32(stat_addr + 4, 1);
        let _ = memory.write_u16(stat_addr + 8, 0);
        let _ = memory.write_u16(stat_addr + 10, 1);
        let _ = memory.write_u16(stat_addr + 12, 0);
        let _ = memory.write_u16(stat_addr + 14, 0);
        let _ = memory.write_u32(stat_addr + 16, size as u32);
        let _ = memory.write_u32(stat_addr + 20, 0);
        let _ = memory.write_u32(stat_addr + 24, 0);
        let _ = memory.write_u32(stat_addr + 28, 0);
    }

    fn ecos_fstat(&mut self, memory: &mut Memory) {
        let lr = memory.get_register(crate::memory::REG_LR);
        if lr == 0x00A52CC8 || lr == 0x00A52C84 || lr == 0x00A52C68 {
            memory.set_register(crate::memory::REG_R0, self.emulated_time_ms());
            memory.set_register(crate::memory::REG_R1, 0);
            return;
        }

        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let stat_addr = memory.get_register(crate::memory::REG_R1);
        if let Some(file) = self.open_files.get(&fd) {
            Self::write_ecos_stat(memory, stat_addr, file.size, false);
            self.return_success(memory);
        } else if (0..=2).contains(&fd) {
            Self::write_ecos_stat(memory, stat_addr, 0, false);
            self.return_success(memory);
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX);
        }
    }

    fn ecos_stat(&mut self, memory: &mut Memory) {
        let path_addr = memory.get_register(crate::memory::REG_R0);
        let stat_addr = memory.get_register(crate::memory::REG_R1);
        let pathname = memory.read_string(path_addr, 256).unwrap_or_default();
        if pathname.eq_ignore_ascii_case("/Rom/IMAGE/GAME/SPDF.BIN") {
            Self::write_ecos_stat(memory, stat_addr, 1, false);
            self.return_success(memory);
            return;
        }
        let host_path = self.ecos_host_path(&pathname);
        if let Ok(meta) = std::fs::metadata(&host_path) {
            Self::write_ecos_stat(memory, stat_addr, meta.len(), meta.is_dir());
            self.return_success(memory);
        } else if let Some(data) = Self::default_keymap_data(&host_path) {
            Self::write_ecos_stat(memory, stat_addr, data.len() as u64, false);
            self.return_success(memory);
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX);
        }
    }

    fn ecos_getcwd(&mut self, memory: &mut Memory) {
        let buf_addr = memory.get_register(crate::memory::REG_R0);
        let size = memory.get_register(crate::memory::REG_R1).max(1);
        let visible_dir = self.current_dir.clone();
        let mut cwd = visible_dir.into_bytes();
        cwd.push(0);
        for (i, byte) in cwd.iter().take(size as usize).enumerate() {
            let _ = memory.write_u8(buf_addr + i as u32, *byte);
        }
        memory.set_register(crate::memory::REG_R0, buf_addr);
    }

    fn ecos_chdir(&mut self, memory: &mut Memory) {
        let path_addr = memory.get_register(crate::memory::REG_R0);
        let path = memory.read_string(path_addr, 256).unwrap_or_default();
        let next = if path.starts_with('/') {
            Self::normalize_ecos_dir(&path)
        } else {
            Self::normalize_ecos_dir(&format!("{}/{}", self.current_dir, path))
        };
        self.current_dir = if next == "/" { String::from("/") } else { next };
        self.return_success(memory);
    }

    fn ecos_errno_ptr(&mut self, memory: &mut Memory) {
        let addr = crate::memory::KEY_STATE_ADDR + 0x100;
        let _ = memory.write_u32(addr, 0);
        memory.set_register(crate::memory::REG_R0, addr);
    }

    fn ecos_opendir(&mut self, memory: &mut Memory) {
        let path_addr = memory.get_register(crate::memory::REG_R0);
        let pathname = memory.read_string(path_addr, 256).unwrap_or_default();
        let host_path = self.ecos_host_path(&pathname);

        let Ok(read_dir) = std::fs::read_dir(&host_path) else {
            memory.set_register(crate::memory::REG_R0, 0);
            return;
        };

        let mut entries: Vec<String> = read_dir
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        self.add_virtual_dir_entries(&pathname, &mut entries);
        entries.sort_unstable_by_key(|name| name.to_ascii_lowercase());
        entries.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        let handle = self.next_dir_handle;
        self.next_dir_handle += 0x400;
        self.open_dirs.insert(
            handle,
            DirHandle {
                entries,
                position: 0,
            },
        );
        memory.set_register(crate::memory::REG_R0, handle);
    }

    fn add_virtual_dir_entries(&self, pathname: &str, entries: &mut Vec<String>) {
        let game_name = std::path::Path::new(&self.game_dir)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let normalized = pathname
            .replace('\\', "/")
            .trim_matches('/')
            .to_ascii_lowercase();

        if game_name == "spmp8k-doom"
            && (normalized.is_empty() || normalized == ".")
            && self.ecos_host_path("doom1.wad").exists()
        {
            entries.push(String::from("doom1.wad"));
        }

        if game_name == "spmp8k-quake"
            && normalized.ends_with("id1")
            && self.ecos_host_path("id1/pak0.pak").exists()
        {
            entries.push(String::from("pak0.pak"));
        }
    }

    fn ecos_readdir_r(&mut self, memory: &mut Memory) {
        let dir_handle = memory.get_register(crate::memory::REG_R0);
        let entry_addr = memory.get_register(crate::memory::REG_R1);
        let result_addr = memory.get_register(crate::memory::REG_R2);
        let Some(dir) = self.open_dirs.get_mut(&dir_handle) else {
            let _ = memory.write_u32(result_addr, 0);
            memory.set_register(crate::memory::REG_R0, u32::MAX);
            return;
        };

        if dir.position >= dir.entries.len() {
            let _ = memory.write_u32(result_addr, 0);
            self.return_success(memory);
            return;
        }

        let name = dir.entries[dir.position].clone();
        dir.position += 1;
        Self::write_ecos_dirent(memory, entry_addr, &name);
        let _ = memory.write_u32(result_addr, entry_addr);
        self.return_success(memory);
    }

    fn ecos_readdir(&mut self, memory: &mut Memory) {
        let entry_addr = crate::memory::KEY_STATE_ADDR + 0x200;
        let result_addr = crate::memory::KEY_STATE_ADDR + 0x380;
        memory.set_register(crate::memory::REG_R1, entry_addr);
        memory.set_register(crate::memory::REG_R2, result_addr);
        self.ecos_readdir_r(memory);
        let result = memory.read_u32(result_addr).unwrap_or(0);
        memory.set_register(crate::memory::REG_R0, result);
    }

    fn write_ecos_dirent(memory: &mut Memory, entry_addr: u32, name: &str) {
        for offset in 0..=256u32 {
            let _ = memory.write_u8(entry_addr + offset, 0);
        }
        for (offset, byte) in name.bytes().take(256).enumerate() {
            let _ = memory.write_u8(entry_addr + offset as u32, byte);
        }
    }

    fn fake_firmware_dummy(&mut self, memory: &mut Memory) {
        let lr = memory.get_register(crate::memory::REG_LR);
        if lr == 0x00A30D04 {
            memory.set_register(crate::memory::REG_R0, 1);
        } else {
            self.return_success(memory);
        }
    }

    /// Handle an SVC call
    pub fn handle_svc(&mut self, svc_num: u32, memory: &mut Memory) {
        match svc_num {
            0x1000 => self.handle_legacy_native_zero(memory),
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
            0x10A0 => self.mcatch_set_transformation(memory),
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
            0x32 => self.emu_if_graph_chg_view(memory),
            0x33 => self.emu_if_graph_cleanup(memory),
            0x34 => self.emu_if_sound_init(memory),
            0x35 => self.emu_if_sound_play(memory),
            0x36 => self.emu_if_sound_cleanup(memory),
            0x37 => self.emu_if_key_init(memory),
            0x38 => self.emu_if_key_get_input(memory),
            0x39 => self.emu_if_key_cleanup(memory),
            0x3A => self.native_ge_get_time(memory),
            0x3B => self.cyg_thread_delay(memory),
            0x3C => self.emu_if_fs_file_open(memory),
            0x3D => self.emu_if_fs_file_get_size(memory),
            0x3E => self.emu_if_fs_file_write(memory),
            0x3F => self.emu_if_fs_file_read(memory),
            0x40 => self.emu_if_fs_file_get_char(memory),
            0x41 => self.emu_if_fs_file_seek(memory),
            0x42 => self.emu_if_fs_file_cur_pos(memory),
            0x43 => self.emu_if_fs_file_close(memory),
            0x60 => self.ecos_open(memory),
            0x61 => self.ecos_read(memory),
            0x62 => self.ecos_write(memory),
            0x63 => self.ecos_close(memory),
            0x64 => self.ecos_lseek(memory),
            0x65 => self.ecos_fstat(memory),
            0x66 => self.ecos_stat(memory),
            0x67 => self.ecos_getcwd(memory),
            0x68 => self.ecos_chdir(memory),
            0x69 => self.ecos_errno_ptr(memory),
            0x6A => self.return_success(memory), // cyg_fd_alloc probe helper
            0x6B => self.ecos_opendir(memory),
            0x6C => self.ecos_readdir_r(memory),
            0x6D => self.ecos_readdir(memory),
            0x6F => self.fake_firmware_dummy(memory),

            0x1001..=0x2000 => {
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

#[cfg(test)]
mod tests {
    use super::NGameApi;
    use std::path::Path;

    #[test]
    fn default_keymaps_match_spmp8k_file_layouts() {
        let keys = NGameApi::default_keymap_data(Path::new("keys.map")).unwrap();
        assert_eq!(keys.len(), 36 * size_of::<u32>());
        assert_eq!(u32::from_le_bytes(keys[0..4].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(keys[44..48].try_into().unwrap()), 0x0800);
        assert_eq!(u32::from_le_bytes(keys[80..84].try_into().unwrap()), 0x0020);

        let config = NGameApi::default_keymap_data(Path::new("KEYMAP.CFG")).unwrap();
        assert_eq!(config.len(), keys.len() + size_of::<u32>());
        assert_eq!(
            u32::from_le_bytes(config[0..4].try_into().unwrap()),
            0xFFF0_0001
        );
        assert_eq!(&config[4..], keys.as_slice());

        assert!(NGameApi::default_keymap_data(Path::new("other.cfg")).is_none());
    }

    #[test]
    fn heretic_wad_alias_prefers_the_local_game_asset() {
        let temp = tempfile::tempdir().unwrap();
        let game_dir = temp.path().join("spmp8k-Heretic");
        std::fs::create_dir(&game_dir).unwrap();
        let local_wad = game_dir.join("heretic1.wad");
        std::fs::write(&local_wad, []).unwrap();

        let mut api = NGameApi::new();
        api.set_game_dir(&game_dir.to_string_lossy());

        assert_eq!(api.ecos_host_path("heretic.wad"), local_wad);
    }
}
