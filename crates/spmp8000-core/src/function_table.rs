// Function table management for SPMP8000 emulator
//
// The SPMP8000 system passes a function table to the game at startup.
// This table contains pointers to system API functions.
// We emulate this by placing SVC instructions that trigger our API handlers.

use crate::memory::Memory;
use anyhow::Result;

pub use crate::memory::FUNC_TABLE_BASE;

/// Function table entry size (4 bytes per pointer)
pub const ENTRY_SIZE: u32 = 4;

const TRAMPOLINE_BASE: u32 = FUNC_TABLE_BASE + 0x800;
const TRAMPOLINE_SIZE: u32 = 8;

const FAKE_FW_BASE: u32 = 0x0028_0000;
const FAKE_FW_G_ST_EMU_FUNCS: u32 = FAKE_FW_BASE;
const FAKE_FW_DUMMY: u32 = FAKE_FW_BASE + 0x100;
const FAKE_FW_ECOS_CLOSE: u32 = FAKE_FW_BASE + 0x120;
const FAKE_FW_ECOS_READ: u32 = FAKE_FW_BASE + 0x140;
const FAKE_FW_ECOS_WRITE: u32 = FAKE_FW_BASE + 0x160;
const FAKE_FW_ECOS_LSEEK: u32 = FAKE_FW_BASE + 0x180;
const FAKE_FW_ECOS_FSTAT: u32 = FAKE_FW_BASE + 0x1A0;
const FAKE_FW_ECOS_OPEN: u32 = FAKE_FW_BASE + 0x1C0;
const FAKE_FW_ERRNO_PTR: u32 = FAKE_FW_BASE + 0x1E0;
const FAKE_FW_FD_ALLOC: u32 = FAKE_FW_BASE + 0x200;
const FAKE_FW_ECOS_STAT: u32 = FAKE_FW_BASE + 0x220;
const FAKE_FW_ECOS_GETCWD: u32 = FAKE_FW_BASE + 0x240;
const FAKE_FW_ECOS_CHDIR: u32 = FAKE_FW_BASE + 0x260;
const FAKE_FW_ECOS_OPENDIR: u32 = FAKE_FW_BASE + 0x280;
const FAKE_FW_ECOS_READDIR_R: u32 = FAKE_FW_BASE + 0x2A0;
const FAKE_FW_ECOS_READDIR: u32 = FAKE_FW_BASE + 0x2C0;
const FAKE_FW_NATIVE_FS_OPEN: u32 = FAKE_FW_BASE + 0x300;
const FAKE_FW_NATIVE_FS_READ: u32 = FAKE_FW_BASE + 0x320;
const FAKE_FW_NATIVE_FS_WRITE: u32 = FAKE_FW_BASE + 0x340;
const FAKE_FW_EMUIF_BASE: u32 = FAKE_FW_BASE + 0x500;

pub fn fake_firmware_direct_svc(pc: u32) -> Option<u32> {
    let offset = pc.checked_sub(FAKE_FW_G_ST_EMU_FUNCS)?;
    match offset {
        0x00 => Some(0x6F),
        0x04 => Some(0x31),
        0x08 => Some(0x32),
        0x0C => Some(0x6F),
        0x10 => Some(0x33),
        0x14 => Some(0x34),
        0x18 => Some(0x35),
        0x1C => Some(0x36),
        0x20 => Some(0x37),
        0x24 => Some(0x38),
        0x28 => Some(0x39),
        0x2C => Some(0x3A),
        0x30 => Some(0x3B),
        0x34 => Some(0x3C),
        0x38 => Some(0x3D),
        0x3C => Some(0x3E),
        0x40 => Some(0x3F),
        0x44 => Some(0x40),
        0x48 => Some(0x41),
        0x4C => Some(0x42),
        0x50 => Some(0x43),
        0x54..=0x70 if offset % 4 == 0 => Some(0x6F),
        _ => None,
    }
}

/// NGame API function indices (from libgame.c analysis)
/// These correspond to the FUNC() macro offsets
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFuncIndex {
    // NativeGE graphics
    DiagPrintf = 0x04,
    MCatchFlush = 0x0C,
    MCatchPaint = 0x10,
    MCatchLoadImage = 0x14,
    MCatchFreeImage = 0x18,
    MCatchInitGraph = 0x38,
    MCatchSetColorROP = 0x3C,
    MCatchGetColorROP = 0x40,
    MCatchSetFGColor = 0x44,
    MCatchGetFGColor = 0x48,
    MCatchSetDisplayScreen = 0x54,
    MCatchGetDisplayScreen = 0x58,
    MCatchSetAlphaBld = 0x74,
    MCatchGetAlphaBld = 0x78,
    MCatchEnableFeature = 0x7C,
    MCatchDisableFeature = 0x80,
    MCatchSetCameraMode = 0x8C,
    MCatchSetFrameBuffer = 0x90,
    MCatchGetFrameBuffer = 0x94,
    MCatchSetTransformation = 0xA0,
    MCatchQueryImage = 0xA4,
    MCatchEnableDoubleBuffer = 0xA8,
    MCatchBitblt = 0xB4,
    MCatchSprite = 0xB8,
    MCatchUpdateScreen = 0xC8,
    MCatchFillRect = 0xC4,

    // NativeGE audio
    NativeGEInitRes = 0xD4,
    NativeGEGetRes = 0xD8,
    NativeGEPlayRes = 0xDC,
    NativeGEPauseRes = 0xE0,
    NativeGEResumeRes = 0xE4,
    NativeGEStopRes = 0xE8,

    // NativeGE file I/O
    NativeGEWriteRecord = 0xEC,
    NativeGEReadRecord = 0xF0,

    // NativeGE input
    NativeGEGetKeyInput4Ntv = 0x100,
    NativeGEShowFPS = 0x108,

    // NativeGE timing
    CygThreadDelay = 0x11C,
    NativeGEGetTime = 0x124,

    // NativeGE control
    NativeGEGameExit = 0x130,
    NativeGEGetTPEvent = 0x134,

    // NativeGE filesystem
    NativeGEFsOpen = 0x13C,
    NativeGEFsRead = 0x140,
    NativeGEFsWrite = 0x144,
    NativeGEFsClose = 0x148,
    NativeGEFsSeek = 0x14C,

    // SPU command (if ftab_length > 85)
    NativeGESPUCommand = 0x154,
}

/// emuIf API function indices (from g_stEmuFuncs)
/// These are accessed via the emulator function table
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmuIfFuncIndex {
    GraphInit = 0x00,
    GraphShow = 0x04,
    GraphChgView = 0x08,
    Unknown0C = 0x0C,
    GraphCleanup = 0x10,
    SoundInit = 0x14,
    SoundPlay = 0x18,
    SoundCleanup = 0x1C,
    KeyInit = 0x20,
    KeyGetInput = 0x24,
    KeyCleanup = 0x28,
    GetCurTime = 0x2C,
    TimeDelay = 0x30,
    FsFileOpen = 0x34,
    FsFileGetSize = 0x38,
    FsFileWrite = 0x3C,
    FsFileRead = 0x40,
    FsFileGetChar = 0x44,
    FsFileSeek = 0x48,
    FsFileCurPos = 0x4C,
    FsFileClose = 0x50,
}

/// Function table manager
#[derive(Debug)]
pub struct FunctionTable {
    /// Maximum number of entries
    pub max_entries: u32,
}

impl FunctionTable {
    /// Create a new function table manager
    pub fn new() -> Self {
        Self { max_entries: 256 }
    }

    /// Set up the function table in memory with SVC trampolines
    pub fn setup_in_memory(&self, memory: &mut Memory) -> Result<()> {
        // For each known API function, write an SVC instruction at the
        // corresponding address in the function table area.
        // When the game calls through the function table, it will execute
        // our SVC instruction, which we intercept.

        let mut trampoline_index = 0;
        for offset in (0..=NativeFuncIndex::NativeGESPUCommand as u32).step_by(ENTRY_SIZE as usize)
        {
            let entry_addr = FUNC_TABLE_BASE + offset;
            let trampoline_addr = TRAMPOLINE_BASE + trampoline_index * TRAMPOLINE_SIZE;
            Self::write_svc_trampoline(memory, entry_addr, trampoline_addr, 0x1000 + offset)?;
            trampoline_index += 1;
        }

        let native_funcs: Vec<(u32, u32)> = vec![
            (NativeFuncIndex::DiagPrintf as u32, 0x01),
            (NativeFuncIndex::MCatchFlush as u32, 0x02),
            (NativeFuncIndex::MCatchPaint as u32, 0x03),
            (NativeFuncIndex::MCatchLoadImage as u32, 0x04),
            (NativeFuncIndex::MCatchFreeImage as u32, 0x05),
            (NativeFuncIndex::MCatchInitGraph as u32, 0x06),
            (NativeFuncIndex::MCatchSetColorROP as u32, 0x07),
            (NativeFuncIndex::MCatchGetColorROP as u32, 0x08),
            (NativeFuncIndex::MCatchSetFGColor as u32, 0x09),
            (NativeFuncIndex::MCatchGetFGColor as u32, 0x0A),
            (NativeFuncIndex::MCatchSetDisplayScreen as u32, 0x0B),
            (NativeFuncIndex::MCatchGetDisplayScreen as u32, 0x0C),
            (NativeFuncIndex::MCatchSetAlphaBld as u32, 0x25),
            (NativeFuncIndex::MCatchGetAlphaBld as u32, 0x26),
            (NativeFuncIndex::MCatchEnableFeature as u32, 0x27),
            (NativeFuncIndex::MCatchDisableFeature as u32, 0x28),
            (NativeFuncIndex::MCatchSetCameraMode as u32, 0x29),
            (NativeFuncIndex::MCatchSetFrameBuffer as u32, 0x0D),
            (NativeFuncIndex::MCatchGetFrameBuffer as u32, 0x0E),
            (NativeFuncIndex::MCatchEnableDoubleBuffer as u32, 0x2A),
            (NativeFuncIndex::MCatchBitblt as u32, 0x0F),
            (NativeFuncIndex::MCatchSprite as u32, 0x10),
            (NativeFuncIndex::MCatchUpdateScreen as u32, 0x2B),
            (NativeFuncIndex::MCatchFillRect as u32, 0x11),
            (NativeFuncIndex::MCatchSetTransformation as u32, 0x10A0),
            (NativeFuncIndex::MCatchQueryImage as u32, 0x10A4),
            (NativeFuncIndex::NativeGEInitRes as u32, 0x12),
            (NativeFuncIndex::NativeGEGetRes as u32, 0x13),
            (NativeFuncIndex::NativeGEPlayRes as u32, 0x14),
            (NativeFuncIndex::NativeGEPauseRes as u32, 0x15),
            (NativeFuncIndex::NativeGEResumeRes as u32, 0x16),
            (NativeFuncIndex::NativeGEStopRes as u32, 0x17),
            (NativeFuncIndex::NativeGEWriteRecord as u32, 0x18),
            (NativeFuncIndex::NativeGEReadRecord as u32, 0x19),
            (NativeFuncIndex::NativeGEGetKeyInput4Ntv as u32, 0x1A),
            (NativeFuncIndex::NativeGEShowFPS as u32, 0x2C),
            (NativeFuncIndex::CygThreadDelay as u32, 0x1B),
            (NativeFuncIndex::NativeGEGetTime as u32, 0x1C),
            (NativeFuncIndex::NativeGEGameExit as u32, 0x1D),
            (NativeFuncIndex::NativeGEGetTPEvent as u32, 0x1E),
            (NativeFuncIndex::NativeGEFsOpen as u32, 0x1F),
            (NativeFuncIndex::NativeGEFsRead as u32, 0x20),
            (NativeFuncIndex::NativeGEFsWrite as u32, 0x21),
            (NativeFuncIndex::NativeGEFsClose as u32, 0x22),
            (NativeFuncIndex::NativeGEFsSeek as u32, 0x23),
            (NativeFuncIndex::NativeGESPUCommand as u32, 0x24),
        ];

        for (offset, svc_num) in native_funcs {
            let entry_addr = FUNC_TABLE_BASE + offset;
            let trampoline_addr = TRAMPOLINE_BASE + trampoline_index * TRAMPOLINE_SIZE;
            Self::write_svc_trampoline(memory, entry_addr, trampoline_addr, svc_num)?;
            trampoline_index += 1;
        }

        Self::setup_fake_firmware(memory)?;

        // Also set up emuIf function table at a different location
        // The emuIf table is typically at a different address
        let emuif_base = FUNC_TABLE_BASE + 0x400; // Offset for emuIf table
        let emuif_funcs: Vec<(u32, u32)> = vec![
            (EmuIfFuncIndex::GraphInit as u32, 0x30),
            (EmuIfFuncIndex::GraphShow as u32, 0x31),
            (EmuIfFuncIndex::GraphChgView as u32, 0x32),
            (EmuIfFuncIndex::GraphCleanup as u32, 0x33),
            (EmuIfFuncIndex::SoundInit as u32, 0x34),
            (EmuIfFuncIndex::SoundPlay as u32, 0x35),
            (EmuIfFuncIndex::SoundCleanup as u32, 0x36),
            (EmuIfFuncIndex::KeyInit as u32, 0x37),
            (EmuIfFuncIndex::KeyGetInput as u32, 0x38),
            (EmuIfFuncIndex::KeyCleanup as u32, 0x39),
            (EmuIfFuncIndex::GetCurTime as u32, 0x3A),
            (EmuIfFuncIndex::TimeDelay as u32, 0x3B),
            (EmuIfFuncIndex::FsFileOpen as u32, 0x3C),
            (EmuIfFuncIndex::FsFileGetSize as u32, 0x3D),
            (EmuIfFuncIndex::FsFileWrite as u32, 0x3E),
            (EmuIfFuncIndex::FsFileRead as u32, 0x3F),
            (EmuIfFuncIndex::FsFileGetChar as u32, 0x40),
            (EmuIfFuncIndex::FsFileSeek as u32, 0x41),
            (EmuIfFuncIndex::FsFileCurPos as u32, 0x42),
            (EmuIfFuncIndex::FsFileClose as u32, 0x43),
        ];

        for (offset, svc_num) in emuif_funcs {
            let entry_addr = emuif_base + offset;
            let trampoline_addr = TRAMPOLINE_BASE + trampoline_index * TRAMPOLINE_SIZE;
            Self::write_svc_trampoline(memory, entry_addr, trampoline_addr, svc_num)?;
            trampoline_index += 1;
        }

        log::info!("Function table initialized at 0x{:08X}", FUNC_TABLE_BASE);
        Ok(())
    }

    fn write_svc_trampoline(
        memory: &mut Memory,
        entry_addr: u32,
        trampoline_addr: u32,
        svc_num: u32,
    ) -> Result<()> {
        memory.write_u32(entry_addr, trampoline_addr)?;
        memory.write_u32(trampoline_addr, 0xEF000000u32 | svc_num)?;
        memory.write_u32(trampoline_addr + 4, 0xE12FFF1E)?; // BX LR
        Ok(())
    }

    fn setup_fake_firmware(memory: &mut Memory) -> Result<()> {
        Self::write_ecos_stub(memory, FAKE_FW_DUMMY, 0x6F, &[])?;
        Self::write_ecos_stub(memory, FAKE_FW_ECOS_CLOSE, 0x63, &[])?;
        Self::write_ecos_stub(memory, FAKE_FW_ECOS_READ, 0x61, &[])?;
        Self::write_ecos_stub(memory, FAKE_FW_ECOS_WRITE, 0x62, &[])?;
        Self::write_ecos_stub(memory, FAKE_FW_ECOS_LSEEK, 0x64, &[])?;
        Self::write_ecos_stub(memory, FAKE_FW_ECOS_FSTAT, 0x65, &[])?;
        Self::write_ecos_stub(
            memory,
            FAKE_FW_ECOS_OPEN,
            0x60,
            &[FAKE_FW_ERRNO_PTR, FAKE_FW_FD_ALLOC],
        )?;
        Self::write_ecos_stub(memory, FAKE_FW_ERRNO_PTR, 0x69, &[])?;
        Self::write_ecos_stub(memory, FAKE_FW_FD_ALLOC, 0x6A, &[])?;
        Self::write_fs_probe_stub(memory, FAKE_FW_ECOS_STAT, 0x66, 0x34)?;
        Self::write_fs_probe_stub(memory, FAKE_FW_ECOS_GETCWD, 0x67, 0x38)?;
        Self::write_fs_probe_stub(memory, FAKE_FW_ECOS_CHDIR, 0x68, 0x30)?;
        Self::write_ecos_stub(memory, FAKE_FW_ECOS_OPENDIR, 0x6B, &[FAKE_FW_FD_ALLOC])?;
        Self::write_readdir_r_stub(memory)?;
        Self::write_readdir_stub(memory)?;

        Self::write_native_wrapper(
            memory,
            FUNC_TABLE_BASE + NativeFuncIndex::NativeGEFsOpen as u32,
            FAKE_FW_NATIVE_FS_OPEN,
            0x1F,
            &[FAKE_FW_ECOS_OPEN, FAKE_FW_ECOS_FSTAT],
        )?;
        Self::write_native_wrapper(
            memory,
            FUNC_TABLE_BASE + NativeFuncIndex::NativeGEFsRead as u32,
            FAKE_FW_NATIVE_FS_READ,
            0x20,
            &[FAKE_FW_ECOS_READ],
        )?;
        Self::write_native_wrapper(
            memory,
            FUNC_TABLE_BASE + NativeFuncIndex::NativeGEFsWrite as u32,
            FAKE_FW_NATIVE_FS_WRITE,
            0x21,
            &[FAKE_FW_ECOS_WRITE],
        )?;

        let emuif_funcs: &[(u32, u32)] = &[
            (0x00, 0x30),
            (0x04, 0x31),
            (0x08, 0x32),
            (0x0C, 0x6F),
            (0x10, 0x33),
            (0x14, 0x34),
            (0x18, 0x35),
            (0x1C, 0x36),
            (0x20, 0x37),
            (0x24, 0x38),
            (0x28, 0x39),
            (0x2C, 0x3A),
            (0x30, 0x3B),
            (0x34, 0x3C),
            (0x38, 0x3D),
            (0x3C, 0x3E),
            (0x40, 0x3F),
            (0x44, 0x40),
            (0x48, 0x41),
            (0x4C, 0x42),
            (0x50, 0x43),
            (0x54, 0x6F),
            (0x58, 0x6F),
            (0x5C, 0x6F),
            (0x60, 0x6F),
            (0x64, 0x6F),
            (0x68, 0x6F),
            (0x6C, 0x6F),
        ];

        for (offset, svc_num) in emuif_funcs {
            let wrapper_addr = FAKE_FW_EMUIF_BASE + offset * 4;
            let bl_targets: &[u32] = match *offset {
                0x38 => &[FAKE_FW_ECOS_FSTAT],
                0x4C => &[FAKE_FW_ECOS_LSEEK],
                0x50 => &[FAKE_FW_ECOS_CLOSE],
                _ => &[],
            };
            Self::write_direct_wrapper(memory, wrapper_addr, *svc_num, bl_targets)?;
            memory.write_u32(FAKE_FW_G_ST_EMU_FUNCS + offset, wrapper_addr)?;
        }

        let diag_ptr = memory.read_u32(FUNC_TABLE_BASE + NativeFuncIndex::DiagPrintf as u32)?;
        for index in 0..28u32 {
            let entry_addr = FAKE_FW_G_ST_EMU_FUNCS + index * 4;
            if memory.read_u32(entry_addr)? == 0 {
                memory.write_u32(entry_addr, FAKE_FW_DUMMY)?;
            }
        }
        memory.write_u32(FAKE_FW_G_ST_EMU_FUNCS + 28 * 4, diag_ptr)?;
        Ok(())
    }

    fn write_ecos_stub(
        memory: &mut Memory,
        addr: u32,
        svc_num: u32,
        bl_targets: &[u32],
    ) -> Result<()> {
        memory.write_u32(addr, 0xE92D4000)?; // STMFD SP!, {LR}
        memory.write_u32(addr + 4, 0xEF000000u32 | svc_num)?;
        memory.write_u32(addr + 8, 0xE8BD8000)?; // LDMFD SP!, {PC}
        for (index, target) in bl_targets.iter().enumerate() {
            Self::write_bl(memory, addr + 12 + index as u32 * 4, *target)?;
        }
        Ok(())
    }

    fn write_fs_probe_stub(
        memory: &mut Memory,
        addr: u32,
        svc_num: u32,
        getinfo_offset: u32,
    ) -> Result<()> {
        Self::write_ecos_stub(memory, addr, svc_num, &[FAKE_FW_ERRNO_PTR])?;
        memory.write_u32(addr + 16, 0xE590F000 | getinfo_offset)?; // LDR PC, [R0, #offset]
        Ok(())
    }

    fn write_readdir_r_stub(memory: &mut Memory) -> Result<()> {
        memory.write_u32(FAKE_FW_ECOS_READDIR_R, 0xE92D4000)?; // STMFD SP!, {LR}
        memory.write_u32(FAKE_FW_ECOS_READDIR_R + 4, 0xEF00006C)?;
        memory.write_u32(FAKE_FW_ECOS_READDIR_R + 8, 0xE8BD8000)?; // LDMFD SP!, {PC}
        memory.write_u32(FAKE_FW_ECOS_READDIR_R + 12, 0xE3A02F41)?; // MOV R2, #0x104
        Self::write_bl(memory, FAKE_FW_ECOS_READDIR_R + 16, FAKE_FW_ECOS_READ)?;
        Ok(())
    }

    fn write_readdir_stub(memory: &mut Memory) -> Result<()> {
        memory.write_u32(FAKE_FW_ECOS_READDIR, 0xE92D4000)?; // STMFD SP!, {LR}
        memory.write_u32(FAKE_FW_ECOS_READDIR + 4, 0xEF00006D)?;
        memory.write_u32(FAKE_FW_ECOS_READDIR + 8, 0xE8BD8000)?; // LDMFD SP!, {PC}
        Self::write_bl(memory, FAKE_FW_ECOS_READDIR + 12, FAKE_FW_ECOS_READDIR_R)?;
        Self::write_bl(memory, FAKE_FW_ECOS_READDIR + 16, FAKE_FW_ERRNO_PTR)?;
        Ok(())
    }

    fn write_native_wrapper(
        memory: &mut Memory,
        entry_addr: u32,
        wrapper_addr: u32,
        svc_num: u32,
        bl_targets: &[u32],
    ) -> Result<()> {
        memory.write_u32(entry_addr, wrapper_addr)?;
        memory.write_u32(wrapper_addr, 0xEF000000u32 | svc_num)?;
        memory.write_u32(wrapper_addr + 4, 0xE12FFF1E)?; // BX LR
        for (index, target) in bl_targets.iter().enumerate() {
            Self::write_bl(memory, wrapper_addr + 8 + index as u32 * 4, *target)?;
        }
        Ok(())
    }

    fn write_direct_wrapper(
        memory: &mut Memory,
        wrapper_addr: u32,
        svc_num: u32,
        bl_targets: &[u32],
    ) -> Result<()> {
        memory.write_u32(wrapper_addr, 0xEF000000u32 | svc_num)?;
        memory.write_u32(wrapper_addr + 4, 0xE12FFF1E)?; // BX LR
        for (index, target) in bl_targets.iter().enumerate() {
            Self::write_bl(memory, wrapper_addr + 8 + index as u32 * 4, *target)?;
        }
        Ok(())
    }

    fn write_bl(memory: &mut Memory, addr: u32, target: u32) -> Result<()> {
        let diff = ((target as i64 - (addr as i64 + 8)) / 4) as i32;
        let imm = (diff as u32) & 0x00FF_FFFF;
        memory.write_u32(addr, 0xEB00_0000 | imm)?;
        Ok(())
    }

    /// Create an ARM trampoline that jumps to a given address
    pub fn create_trampoline(target_addr: u32) -> Vec<u8> {
        // ARM branch instruction: B target
        // The offset is calculated from PC+8 (ARM pipeline)
        // For simplicity, we use an absolute jump via register
        // LDR PC, [PC, #-4] followed by the target address
        vec![
            0x04,
            0xF0,
            0x1F,
            0xE5, // LDR PC, [PC, #-4]
            (target_addr & 0xFF) as u8,
            ((target_addr >> 8) & 0xFF) as u8,
            ((target_addr >> 16) & 0xFF) as u8,
            ((target_addr >> 24) & 0xFF) as u8,
        ]
    }
}

impl Default for FunctionTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Permission, FUNC_TABLE_SIZE};

    #[test]
    fn test_function_table_setup() {
        let mut memory = Memory::new();
        memory
            .map_region(
                FUNC_TABLE_BASE,
                FUNC_TABLE_SIZE,
                Permission::ALL,
                "func_table",
            )
            .unwrap();
        memory
            .map_region(FAKE_FW_BASE, 4096, Permission::ALL, "fake_fw")
            .unwrap();

        let ft = FunctionTable::new();
        ft.setup_in_memory(&mut memory).unwrap();

        // Verify the table entry points to an SVC trampoline.
        let ptr = memory
            .read_u32(FUNC_TABLE_BASE + NativeFuncIndex::DiagPrintf as u32)
            .unwrap();
        let val = memory.read_u32(ptr).unwrap();
        assert_eq!(val & 0xFF000000, 0xEF000000); // SVC instruction prefix
        assert_eq!(val & 0x00FFFFFF, 0x01); // SVC #1
        assert_eq!(memory.read_u32(ptr + 4).unwrap(), 0xE12FFF1E); // BX LR
    }

    #[test]
    fn low_ram_framebuffer_does_not_overwrite_trampolines() {
        const FRAMEBUFFER_ADDR: u32 = 0x000E_0010;
        const FRAMEBUFFER_SIZE: usize = 320 * 240 * 2;

        let mut memory = Memory::new();
        memory.init_default().unwrap();
        let ft = FunctionTable::new();
        ft.setup_in_memory(&mut memory).unwrap();

        let trampoline = memory
            .read_u32(FUNC_TABLE_BASE + NativeFuncIndex::DiagPrintf as u32)
            .unwrap();
        let svc_instruction = memory.read_u32(trampoline).unwrap();

        memory
            .write_block(FRAMEBUFFER_ADDR, &vec![0xA5; FRAMEBUFFER_SIZE])
            .unwrap();

        assert_eq!(memory.read_u32(trampoline).unwrap(), svc_instruction);
    }

    #[test]
    fn test_trampoline_creation() {
        let trampoline = FunctionTable::create_trampoline(0x12345678);
        assert_eq!(trampoline.len(), 8);
        // Check LDR PC instruction
        assert_eq!(trampoline[0..4], [0x04, 0xF0, 0x1F, 0xE5]);
        // Check target address
        assert_eq!(
            u32::from_le_bytes(trampoline[4..8].try_into().unwrap()),
            0x12345678
        );
    }
}
