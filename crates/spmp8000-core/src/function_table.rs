// Function table management for SPMP8000 emulator
//
// The SPMP8000 system passes a function table to the game at startup.
// This table contains pointers to system API functions.
// We emulate this by placing SVC instructions that trigger our API handlers.

use crate::memory::Memory;
use anyhow::Result;

/// Function table base address (where we store our trampolines)
pub const FUNC_TABLE_BASE: u32 = 0x00100000;

/// Function table entry size (4 bytes per pointer)
pub const ENTRY_SIZE: u32 = 4;

const TRAMPOLINE_BASE: u32 = FUNC_TABLE_BASE + 0x800;
const TRAMPOLINE_SIZE: u32 = 8;

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
            (NativeFuncIndex::MCatchSetFrameBuffer as u32, 0x0D),
            (NativeFuncIndex::MCatchGetFrameBuffer as u32, 0x0E),
            (NativeFuncIndex::MCatchBitblt as u32, 0x0F),
            (NativeFuncIndex::MCatchSprite as u32, 0x10),
            (NativeFuncIndex::MCatchFillRect as u32, 0x11),
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
    use crate::memory::Permission;

    #[test]
    fn test_function_table_setup() {
        let mut memory = Memory::new();
        memory
            .map_region(FUNC_TABLE_BASE, 4096, Permission::ALL, "func_table")
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
