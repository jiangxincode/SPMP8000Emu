// Memory management for SPMP8000 emulator
//
// SPMP8000 memory map:
// - 0x00000000 - 0x00FFFFFF: 16MB RAM (code + data)
// - 0x009FF000 - 0x009FFFFF: HLE function table and trampolines
// - 0x00A00000:              Code load address (from libgame.ld)
// - 0x01000000 - 0x01FFFFFF: 16MB Video memory / framebuffer
// - 0x02000000 - 0x02FFFFFF: Peripheral registers
// - 0x00280000 - 0x00A00000: Firmware area (FW_START - FW_END)

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Memory permissions (simplified)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Permission {
    pub const READ: Permission = Permission {
        read: true,
        write: false,
        execute: false,
    };
    pub const WRITE: Permission = Permission {
        read: false,
        write: true,
        execute: false,
    };
    pub const EXEC: Permission = Permission {
        read: false,
        write: false,
        execute: true,
    };
    pub const ALL: Permission = Permission {
        read: true,
        write: true,
        execute: true,
    };
}

impl std::ops::BitOr for Permission {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Permission {
            read: self.read || rhs.read,
            write: self.write || rhs.write,
            execute: self.execute || rhs.execute,
        }
    }
}

/// Memory region with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRegion {
    pub base: u32,
    pub size: u32,
    pub data: Vec<u8>,
    pub permissions: Permission,
    pub name: String,
}

/// Main memory manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    regions: Vec<MemoryRegion>,
    /// Register file (R0-R15, CPSR)
    registers: [u32; 17],
}

/// Register indices
pub const REG_R0: usize = 0;
pub const REG_R1: usize = 1;
pub const REG_R2: usize = 2;
pub const REG_R3: usize = 3;
pub const REG_R4: usize = 4;
pub const REG_R5: usize = 5;
pub const REG_R6: usize = 6;
pub const REG_R7: usize = 7;
pub const REG_R8: usize = 8;
pub const REG_R9: usize = 9;
pub const REG_R10: usize = 10;
pub const REG_R11: usize = 11;
pub const REG_R12: usize = 12;
pub const REG_SP: usize = 13;
pub const REG_LR: usize = 14;
pub const REG_PC: usize = 15;
pub const REG_CPSR: usize = 16;

/// Well-known memory addresses
pub const CODE_LOAD_ADDR: u32 = 0x00A00000;
pub const RAM_BASE: u32 = 0x00000000;
pub const RAM_SIZE: u32 = 16 * 1024 * 1024; // 16MB
pub const FUNC_TABLE_SIZE: u32 = 4096;
pub const FUNC_TABLE_BASE: u32 = CODE_LOAD_ADDR - FUNC_TABLE_SIZE;
pub const VRAM_BASE: u32 = 0x01000000;
pub const VRAM_SIZE: u32 = 16 * 1024 * 1024; // 16MB
pub const PERIPHERAL_BASE: u32 = 0x02000000;
pub const PERIPHERAL_SIZE: u32 = 16 * 1024 * 1024; // 16MB
pub const KEY_STATE_ADDR: u32 = 0x00200000; // Address for key state

impl Memory {
    /// Create a new memory manager with default regions
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            registers: [0; 17],
        }
    }

    /// Initialize default memory regions
    pub fn init_default(&mut self) -> Result<()> {
        // Main RAM
        self.map_region(RAM_BASE, RAM_SIZE, Permission::ALL, "RAM")?;

        // Video RAM
        self.map_region(
            VRAM_BASE,
            VRAM_SIZE,
            Permission::READ | Permission::WRITE,
            "VRAM",
        )?;

        // Memory-mapped peripheral registers.
        self.map_region(
            PERIPHERAL_BASE,
            PERIPHERAL_SIZE,
            Permission::READ | Permission::WRITE,
            "PERIPHERAL",
        )?;

        Ok(())
    }

    /// Map a new memory region
    pub fn map_region(
        &mut self,
        base: u32,
        size: u32,
        permissions: Permission,
        name: &str,
    ) -> Result<()> {
        // Check for overlapping regions
        for region in &self.regions {
            if base < region.base + region.size && base + size > region.base {
                anyhow::bail!(
                    "Memory region overlaps with existing region '{}' at 0x{:08X}",
                    region.name,
                    region.base
                );
            }
        }

        self.regions.push(MemoryRegion {
            base,
            size,
            data: vec![0; size as usize],
            permissions,
            name: name.to_string(),
        });

        log::info!(
            "Mapped memory region: {} at 0x{:08X} ({} bytes)",
            name,
            base,
            size
        );
        Ok(())
    }

    /// Find the region containing the given address
    fn find_region(&self, addr: u32) -> Option<&MemoryRegion> {
        self.regions
            .iter()
            .find(|r| addr >= r.base && addr < r.base + r.size)
    }

    /// Find the mutable region containing the given address
    fn find_region_mut(&mut self, addr: u32) -> Option<&mut MemoryRegion> {
        self.regions
            .iter_mut()
            .find(|r| addr >= r.base && addr < r.base + r.size)
    }

    /// Read a single byte
    pub fn read_u8(&self, addr: u32) -> Result<u8> {
        let region = self
            .find_region(addr)
            .ok_or_else(|| anyhow::anyhow!("Read from unmapped address: 0x{:08X}", addr))?;
        let offset = (addr - region.base) as usize;
        Ok(region.data[offset])
    }

    /// Write a single byte
    pub fn write_u8(&mut self, addr: u32, value: u8) -> Result<()> {
        let region = self
            .find_region_mut(addr)
            .ok_or_else(|| anyhow::anyhow!("Write to unmapped address: 0x{:08X}", addr))?;
        let offset = (addr - region.base) as usize;
        region.data[offset] = value;
        Ok(())
    }

    /// Read a 16-bit value (little-endian)
    pub fn read_u16(&self, addr: u32) -> Result<u16> {
        let lo = self.read_u8(addr)? as u16;
        let hi = self.read_u8(addr + 1)? as u16;
        Ok(lo | (hi << 8))
    }

    /// Write a 16-bit value (little-endian)
    pub fn write_u16(&mut self, addr: u32, value: u16) -> Result<()> {
        self.write_u8(addr, (value & 0xFF) as u8)?;
        self.write_u8(addr + 1, ((value >> 8) & 0xFF) as u8)?;
        Ok(())
    }

    /// Read a 32-bit value (little-endian)
    pub fn read_u32(&self, addr: u32) -> Result<u32> {
        let b0 = self.read_u8(addr)? as u32;
        let b1 = self.read_u8(addr + 1)? as u32;
        let b2 = self.read_u8(addr + 2)? as u32;
        let b3 = self.read_u8(addr + 3)? as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    /// Write a 32-bit value (little-endian)
    pub fn write_u32(&mut self, addr: u32, value: u32) -> Result<()> {
        self.write_u8(addr, (value & 0xFF) as u8)?;
        self.write_u8(addr + 1, ((value >> 8) & 0xFF) as u8)?;
        self.write_u8(addr + 2, ((value >> 16) & 0xFF) as u8)?;
        self.write_u8(addr + 3, ((value >> 24) & 0xFF) as u8)?;
        Ok(())
    }

    /// Read a block of memory
    pub fn read_block(&self, addr: u32, size: usize) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(size);
        for i in 0..size {
            result.push(self.read_u8(addr + i as u32)?);
        }
        Ok(result)
    }

    /// Write a block of memory
    pub fn write_block(&mut self, addr: u32, data: &[u8]) -> Result<()> {
        for (i, &byte) in data.iter().enumerate() {
            self.write_u8(addr + i as u32, byte)?;
        }
        Ok(())
    }

    /// Read a null-terminated string from memory
    pub fn read_string(&self, addr: u32, max_len: usize) -> Result<String> {
        let mut bytes = Vec::new();
        for i in 0..max_len {
            let byte = self.read_u8(addr + i as u32)?;
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    /// Get register value
    pub fn get_register(&self, reg: usize) -> u32 {
        if reg < 17 {
            self.registers[reg]
        } else {
            0
        }
    }

    /// Set register value
    pub fn set_register(&mut self, reg: usize, value: u32) {
        if reg < 17 {
            self.registers[reg] = value;
        }
    }

    /// Get all regions (for debugging)
    pub fn regions(&self) -> &[MemoryRegion] {
        &self.regions
    }

    pub(crate) fn validate_state(&self) -> Result<()> {
        let expected = [
            (RAM_BASE, RAM_SIZE, Permission::ALL, "RAM"),
            (
                VRAM_BASE,
                VRAM_SIZE,
                Permission::READ | Permission::WRITE,
                "VRAM",
            ),
            (
                PERIPHERAL_BASE,
                PERIPHERAL_SIZE,
                Permission::READ | Permission::WRITE,
                "PERIPHERAL",
            ),
        ];
        if self.regions.len() != expected.len() {
            anyhow::bail!(
                "save state has {} memory regions, expected {}",
                self.regions.len(),
                expected.len()
            );
        }

        for (region, (base, size, permissions, name)) in self.regions.iter().zip(expected) {
            if region.base != base
                || region.size != size
                || region.permissions != permissions
                || region.name != name
                || region.data.len() != size as usize
            {
                anyhow::bail!("save state has an incompatible {name} memory region");
            }
        }
        Ok(())
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_read_write() {
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 4096, Permission::ALL, "test")
            .unwrap();

        memory.write_u32(0x1000, 0x12345678).unwrap();
        assert_eq!(memory.read_u32(0x1000).unwrap(), 0x12345678);

        memory.write_u16(0x1004, 0xABCD).unwrap();
        assert_eq!(memory.read_u16(0x1004).unwrap(), 0xABCD);

        memory.write_u8(0x1006, 0x42).unwrap();
        assert_eq!(memory.read_u8(0x1006).unwrap(), 0x42);
    }

    #[test]
    fn test_memory_block_operations() {
        let mut memory = Memory::new();
        memory
            .map_region(0x0, 4096, Permission::ALL, "test")
            .unwrap();

        let data = vec![1, 2, 3, 4, 5];
        memory.write_block(0x100, &data).unwrap();

        let read_data = memory.read_block(0x100, 5).unwrap();
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_memory_string() {
        let mut memory = Memory::new();
        memory
            .map_region(0x0, 4096, Permission::ALL, "test")
            .unwrap();

        let s = "Hello";
        for (i, c) in s.bytes().enumerate() {
            memory.write_u8(0x200 + i as u32, c).unwrap();
        }
        memory.write_u8(0x205, 0).unwrap();

        assert_eq!(memory.read_string(0x200, 32).unwrap(), "Hello");
    }

    #[test]
    fn test_memory_out_of_bounds() {
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 256, Permission::ALL, "test")
            .unwrap();

        assert!(memory.read_u32(0x2000).is_err());
        assert!(memory.write_u32(0x2000, 0).is_err());
    }
}
