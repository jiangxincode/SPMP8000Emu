// emuIf and NativeGE file system API implementation

use super::{FileHandle, NGameApi};
use crate::memory::Memory;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

impl NGameApi {
    fn resolve_host_path(&self, pathname: &str) -> PathBuf {
        let normalized = pathname.replace('\\', "/");
        let relative = normalized.trim_start_matches('/');
        PathBuf::from(&self.game_dir).join(relative)
    }

    fn read_host_file_into_memory(
        host_path: &std::path::Path,
        position: u64,
        memory: &mut Memory,
        buf_addr: u32,
        count: u32,
    ) -> Option<usize> {
        let mut host_file = std::fs::File::open(host_path).ok()?;
        host_file.seek(SeekFrom::Start(position)).ok()?;
        let mut buffer = vec![0u8; count as usize];
        let bytes_read = host_file.read(&mut buffer).ok()?;
        for (i, &byte) in buffer.iter().enumerate().take(bytes_read) {
            let _ = memory.write_u8(buf_addr + i as u32, byte);
        }
        Some(bytes_read)
    }

    fn write_memory_to_host_file(
        host_path: &std::path::Path,
        position: u64,
        memory: &Memory,
        buf_addr: u32,
        count: u32,
    ) -> Option<usize> {
        let mut buffer = Vec::with_capacity(count as usize);
        for i in 0..count {
            buffer.push(memory.read_u8(buf_addr + i).ok()?);
        }

        let mut host_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(host_path)
            .ok()?;
        host_file.seek(SeekFrom::Start(position)).ok()?;
        host_file.write(&buffer).ok()
    }

    fn seek_file_position(file: &mut FileHandle, offset: i32, whence: u32) -> Option<u64> {
        let new_position = match whence {
            0 => offset as i64,
            1 => file.position as i64 + offset as i64,
            2 => file.size as i64 + offset as i64,
            _ => return None,
        };

        file.position = new_position.max(0) as u64;
        Some(file.position)
    }

    /// emuIfFsFileOpen - Open a file
    pub fn emu_if_fs_file_open(&mut self, memory: &mut Memory) {
        let pathname_addr = memory.get_register(crate::memory::REG_R0);
        let flags = memory.get_register(crate::memory::REG_R1);

        let pathname = memory.read_string(pathname_addr, 256).unwrap_or_default();

        log::debug!("emuIfFsFileOpen: {} (flags=0x{:X})", pathname, flags);

        let host_path = self.resolve_host_path(&pathname);

        // Check if file exists
        let file_exists = host_path.exists();

        // Determine open mode
        let is_writable = (flags & 2) != 0;

        if file_exists || (flags & 4) != 0 {
            // Create file handle
            let file = FileHandle {
                host_path: host_path.to_string_lossy().to_string(),
                position: 0,
                size: std::fs::metadata(&host_path).map(|m| m.len()).unwrap_or(0),
                is_writable,
            };

            let fd = self.allocate_fd();
            self.open_files.insert(fd, file);

            memory.set_register(crate::memory::REG_R0, fd as u32);
        } else {
            // File not found
            memory.set_register(crate::memory::REG_R0, u32::MAX); // -1
        }
    }

    /// emuIfFsFileRead - Read from file
    pub fn emu_if_fs_file_read(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let buf_addr = memory.get_register(crate::memory::REG_R1);
        let count = memory.get_register(crate::memory::REG_R2);

        if let Some(file) = self.open_files.get_mut(&fd) {
            if let Some(bytes_read) = Self::read_host_file_into_memory(
                std::path::Path::new(&file.host_path),
                file.position,
                memory,
                buf_addr,
                count,
            ) {
                file.position += bytes_read as u64;
                memory.set_register(crate::memory::REG_R0, bytes_read as u32);
                return;
            }
        }

        // Error
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfFsFileWrite - Write to file
    pub fn emu_if_fs_file_write(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let buf_addr = memory.get_register(crate::memory::REG_R1);
        let count = memory.get_register(crate::memory::REG_R2);

        if let Some(file) = self.open_files.get_mut(&fd) {
            if file.is_writable {
                if let Some(bytes_written) = Self::write_memory_to_host_file(
                    std::path::Path::new(&file.host_path),
                    file.position,
                    memory,
                    buf_addr,
                    count,
                ) {
                    file.position += bytes_written as u64;
                    file.size = file.size.max(file.position);
                    memory.set_register(crate::memory::REG_R0, bytes_written as u32);
                    return;
                }
            }
        }

        // Error
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfFsFileGetSize - Get open file size
    pub fn emu_if_fs_file_get_size(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let size = self.open_files.get(&fd).map(|file| file.size).unwrap_or(0);
        memory.set_register(crate::memory::REG_R0, size as u32);
    }

    /// emuIfFsFileGetChar - Read one byte from file
    pub fn emu_if_fs_file_get_char(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;

        if let Some(file) = self.open_files.get_mut(&fd) {
            let mut byte = [0u8; 1];
            if let Ok(mut host_file) = std::fs::File::open(&file.host_path) {
                if host_file.seek(SeekFrom::Start(file.position)).is_ok()
                    && host_file.read(&mut byte).ok() == Some(1)
                {
                    file.position += 1;
                    memory.set_register(crate::memory::REG_R0, byte[0] as u32);
                    return;
                }
            }
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfFsFileClose - Close file
    pub fn emu_if_fs_file_close(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;

        if self.close_file(fd) {
            memory.set_register(crate::memory::REG_R0, 0); // Success
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX); // Error
        }
    }

    /// emuIfFsFileSeek - Seek in file
    pub fn emu_if_fs_file_seek(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let offset = memory.get_register(crate::memory::REG_R1) as i32;
        let whence = memory.get_register(crate::memory::REG_R2);

        if let Some(file) = self.open_files.get_mut(&fd) {
            if Self::seek_file_position(file, offset, whence).is_some() {
                memory.set_register(crate::memory::REG_R0, 0); // Success
            } else {
                memory.set_register(crate::memory::REG_R0, u32::MAX);
            }
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX); // Error
        }
    }

    /// emuIfFsFileCurPos - Get current file position
    pub fn emu_if_fs_file_cur_pos(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;

        if let Some(file) = self.open_files.get(&fd) {
            memory.set_register(crate::memory::REG_R0, file.position as u32);
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX);
        }
    }

    /// NativeGE_fsOpen - Open file (NativeGE interface)
    pub fn native_ge_fs_open(&mut self, memory: &mut Memory) {
        let filename_addr = memory.get_register(crate::memory::REG_R0);
        let flags = memory.get_register(crate::memory::REG_R1);
        let fd_addr = memory.get_register(crate::memory::REG_R2);

        let filename = memory.read_string(filename_addr, 256).unwrap_or_default();

        log::debug!("NativeGE_fsOpen: {} (flags=0x{:X})", filename, flags);

        let host_path = self.resolve_host_path(&filename);

        if host_path.exists() {
            let size = std::fs::metadata(&host_path).map(|m| m.len()).unwrap_or(0);
            let file = FileHandle {
                host_path: host_path.to_string_lossy().to_string(),
                position: 0,
                size,
                is_writable: (flags & 2) != 0,
            };

            let fd = self.allocate_fd();
            self.open_files.insert(fd, file);

            let _ = memory.write_u32(fd_addr, fd as u32);
            memory.set_register(crate::memory::REG_R0, 0); // Success
        } else {
            memory.set_register(crate::memory::REG_R0, 2); // ENOENT
        }
    }

    /// NativeGE_fsRead - Read from file (NativeGE interface)
    pub fn native_ge_fs_read(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let buf_addr = memory.get_register(crate::memory::REG_R1);
        let count = memory.get_register(crate::memory::REG_R2);
        let result_addr = memory.get_register(crate::memory::REG_R3);

        if let Some(file) = self.open_files.get_mut(&fd) {
            if let Some(bytes_read) = Self::read_host_file_into_memory(
                std::path::Path::new(&file.host_path),
                file.position,
                memory,
                buf_addr,
                count,
            ) {
                file.position += bytes_read as u64;
                let _ = memory.write_u32(result_addr, bytes_read as u32);
                memory.set_register(crate::memory::REG_R0, 0); // Success
                return;
            }
        }

        let _ = memory.write_u32(result_addr, 0);
        memory.set_register(crate::memory::REG_R0, 5); // EIO
    }

    /// NativeGE_fsWrite - Write to file (NativeGE interface)
    pub fn native_ge_fs_write(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let buf_addr = memory.get_register(crate::memory::REG_R1);
        let count = memory.get_register(crate::memory::REG_R2);
        let result_addr = memory.get_register(crate::memory::REG_R3);

        if let Some(file) = self.open_files.get_mut(&fd) {
            if file.is_writable {
                if let Some(bytes_written) = Self::write_memory_to_host_file(
                    std::path::Path::new(&file.host_path),
                    file.position,
                    memory,
                    buf_addr,
                    count,
                ) {
                    file.position += bytes_written as u64;
                    file.size = file.size.max(file.position);
                    let _ = memory.write_u32(result_addr, bytes_written as u32);
                    memory.set_register(crate::memory::REG_R0, 0);
                    return;
                }
            }
        }

        let _ = memory.write_u32(result_addr, 0);
        memory.set_register(crate::memory::REG_R0, 5); // EIO
    }

    /// NativeGE_fsClose - Close file (NativeGE interface)
    pub fn native_ge_fs_close(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;

        if self.close_file(fd) {
            memory.set_register(crate::memory::REG_R0, 0);
        } else {
            memory.set_register(crate::memory::REG_R0, 9); // EBADF
        }
    }

    /// NativeGE_fsSeek - Seek in file (NativeGE interface)
    pub fn native_ge_fs_seek(&mut self, memory: &mut Memory) {
        let fd = memory.get_register(crate::memory::REG_R0) as i32;
        let offset = memory.get_register(crate::memory::REG_R1) as i32;
        let whence = memory.get_register(crate::memory::REG_R2);

        if let Some(file) = self.open_files.get_mut(&fd) {
            if let Some(position) = Self::seek_file_position(file, offset, whence) {
                memory.set_register(crate::memory::REG_R0, 0);
                memory.set_register(crate::memory::REG_R1, position as u32);
            } else {
                memory.set_register(crate::memory::REG_R0, 22); // EINVAL
                memory.set_register(crate::memory::REG_R1, 0);
            }
        } else {
            memory.set_register(crate::memory::REG_R0, 9); // EBADF
            memory.set_register(crate::memory::REG_R1, 0);
        }
    }

    /// NativeGE_readRecord - Read a file range by path
    pub fn native_ge_read_record(&mut self, memory: &mut Memory) {
        let pathname_addr = memory.get_register(crate::memory::REG_R0);
        let buf_addr = memory.get_register(crate::memory::REG_R1);
        let _flags = memory.get_register(crate::memory::REG_R2);
        let offset = memory.get_register(crate::memory::REG_R3) as u64;
        let count = memory
            .read_u32(memory.get_register(crate::memory::REG_SP))
            .unwrap_or(0);

        let pathname = memory.read_string(pathname_addr, 256).unwrap_or_default();
        let host_path = self.resolve_host_path(&pathname);

        log::debug!(
            "NativeGE_readRecord: {} offset={} count={}",
            pathname,
            offset,
            count
        );

        if Self::read_host_file_into_memory(&host_path, offset, memory, buf_addr, count).is_some() {
            memory.set_register(crate::memory::REG_R0, 0);
        } else {
            memory.set_register(crate::memory::REG_R0, 5); // EIO
        }
    }

    /// NativeGE_writeRecord - Write a file range by path
    pub fn native_ge_write_record(&mut self, memory: &mut Memory) {
        let pathname_addr = memory.get_register(crate::memory::REG_R0);
        let buf_addr = memory.get_register(crate::memory::REG_R1);
        let _flags = memory.get_register(crate::memory::REG_R2);
        let offset = memory.get_register(crate::memory::REG_R3) as u64;
        let count = memory
            .read_u32(memory.get_register(crate::memory::REG_SP))
            .unwrap_or(0);

        let pathname = memory.read_string(pathname_addr, 256).unwrap_or_default();
        let host_path = self.resolve_host_path(&pathname);

        log::debug!(
            "NativeGE_writeRecord: {} offset={} count={}",
            pathname,
            offset,
            count
        );

        if Self::write_memory_to_host_file(&host_path, offset, memory, buf_addr, count).is_some() {
            memory.set_register(crate::memory::REG_R0, 0);
        } else {
            memory.set_register(crate::memory::REG_R0, 5); // EIO
        }
    }
}
