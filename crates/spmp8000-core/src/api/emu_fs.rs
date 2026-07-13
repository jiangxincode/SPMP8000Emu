// emuIf and NativeGE file system API implementation

use super::{FileHandle, NGameApi};
use crate::memory::Memory;
use std::io::{Read, Seek, SeekFrom, Write};

impl NGameApi {
    /// emuIfFsFileOpen - Open a file
    pub fn emu_if_fs_file_open(&mut self, memory: &mut Memory) {
        let pathname_addr = memory.get_register(crate::memory::REG_R0);
        let flags = memory.get_register(crate::memory::REG_R1);

        let pathname = memory.read_string(pathname_addr, 256).unwrap_or_default();

        log::debug!("emuIfFsFileOpen: {} (flags=0x{:X})", pathname, flags);

        // Build host path
        let host_path = format!("{}/{}", self.game_dir, pathname);

        // Check if file exists
        let file_exists = std::path::Path::new(&host_path).exists();

        // Determine open mode
        let is_writable = (flags & 2) != 0;

        if file_exists || (flags & 4) != 0 {
            // Create file handle
            let file = FileHandle {
                host_path: host_path.clone(),
                position: 0,
                size: std::fs::metadata(&host_path)
                    .map(|m| m.len())
                    .unwrap_or(0),
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
            // Open the host file and read
            if let Ok(mut host_file) = std::fs::File::open(&file.host_path) {
                if host_file.seek(SeekFrom::Start(file.position)).is_ok() {
                    let mut buffer = vec![0u8; count as usize];
                    if let Ok(bytes_read) = host_file.read(&mut buffer) {
                        // Write data to emulated memory
                        for (i, &byte) in buffer.iter().enumerate().take(bytes_read) {
                            let _ = memory.write_u8(buf_addr + i as u32, byte);
                        }

                        file.position += bytes_read as u64;
                        memory.set_register(crate::memory::REG_R0, bytes_read as u32);
                        return;
                    }
                }
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
                // Read data from emulated memory
                let mut buffer = Vec::with_capacity(count as usize);
                for i in 0..count {
                    if let Ok(byte) = memory.read_u8(buf_addr + i) {
                        buffer.push(byte);
                    }
                }

                // Write to host file
                if let Ok(mut host_file) = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&file.host_path)
                {
                    if host_file.seek(SeekFrom::Start(file.position)).is_ok() {
                        if let Ok(bytes_written) = host_file.write(&buffer) {
                            file.position += bytes_written as u64;
                            memory.set_register(crate::memory::REG_R0, bytes_written as u32);
                            return;
                        }
                    }
                }
            }
        }

        // Error
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
            let new_pos = match whence {
                0 => SeekFrom::Start(offset as u64),      // SEEK_SET
                1 => SeekFrom::Current(offset as i64),     // SEEK_CUR
                2 => SeekFrom::End(offset as i64),         // SEEK_END
                _ => {
                    memory.set_register(crate::memory::REG_R0, u32::MAX);
                    return;
                }
            };

            // We can't actually seek here since we don't have the file open
            // Store the intended position and apply on next read/write
            match new_pos {
                SeekFrom::Start(pos) => file.position = pos,
                SeekFrom::Current(delta) => {
                    file.position = (file.position as i64 + delta) as u64
                }
                SeekFrom::End(delta) => {
                    file.position = (file.size as i64 + delta) as u64
                }
            }

            memory.set_register(crate::memory::REG_R0, 0); // Success
        } else {
            memory.set_register(crate::memory::REG_R0, u32::MAX); // Error
        }
    }

    /// NativeGE_fsOpen - Open file (NativeGE interface)
    pub fn native_ge_fs_open(&mut self, memory: &mut Memory) {
        let filename_addr = memory.get_register(crate::memory::REG_R0);
        let flags = memory.get_register(crate::memory::REG_R1);
        let fd_addr = memory.get_register(crate::memory::REG_R2);

        let filename = memory.read_string(filename_addr, 256).unwrap_or_default();

        log::debug!("NativeGE_fsOpen: {} (flags=0x{:X})", filename, flags);

        // Build host path
        let host_path = format!("{}/{}", self.game_dir, filename);

        if std::path::Path::new(&host_path).exists() {
            let size = std::fs::metadata(&host_path)
                .map(|m| m.len())
                .unwrap_or(0);
            let file = FileHandle {
                host_path,
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
            if let Ok(mut host_file) = std::fs::File::open(&file.host_path) {
                if host_file.seek(SeekFrom::Start(file.position)).is_ok() {
                    let mut buffer = vec![0u8; count as usize];
                    if let Ok(bytes_read) = host_file.read(&mut buffer) {
                        for (i, &byte) in buffer.iter().enumerate().take(bytes_read) {
                            let _ = memory.write_u8(buf_addr + i as u32, byte);
                        }

                        file.position += bytes_read as u64;
                        let _ = memory.write_u32(result_addr, bytes_read as u32);
                        memory.set_register(crate::memory::REG_R0, 0); // Success
                        return;
                    }
                }
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
                let mut buffer = Vec::with_capacity(count as usize);
                for i in 0..count {
                    if let Ok(byte) = memory.read_u8(buf_addr + i) {
                        buffer.push(byte);
                    }
                }

                if let Ok(mut host_file) = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&file.host_path)
                {
                    if host_file.seek(SeekFrom::Start(file.position)).is_ok() {
                        if let Ok(bytes_written) = host_file.write(&buffer) {
                            file.position += bytes_written as u64;
                            let _ = memory.write_u32(result_addr, bytes_written as u32);
                            memory.set_register(crate::memory::REG_R0, 0);
                            return;
                        }
                    }
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
            match whence {
                0 => file.position = offset as u64,
                1 => file.position = (file.position as i64 + offset as i64) as u64,
                2 => file.position = (file.size as i64 + offset as i64) as u64,
                _ => {}
            }
            memory.set_register(crate::memory::REG_R0, 0);
        } else {
            memory.set_register(crate::memory::REG_R0, 9); // EBADF
        }
    }
}
