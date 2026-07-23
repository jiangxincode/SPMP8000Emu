// Cheat system for SPMP8000 emulator
//
// Supports two cheat target types:
//   mem:<addr>=<value>   — write a value to a memory address
//   reg:<name>=<value>   — set a CPU register
//
// Address/value literals are hexadecimal by default (prefix 0x optional).
// Memory writes auto-detect width from the value: 0..=0xFF → u8,
// 0x100..=0xFFFF → u16, otherwise → u32.  Explicit width prefixes
// (b:, h:, w:) override auto-detection.

use std::collections::BTreeMap;
use std::str::FromStr;

use crate::arm_cpu::ArmCpu;
use crate::memory::Memory;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum CheatParseError {
    #[error("empty cheat code")]
    Empty,
    #[error("missing '=' (expected <target>=<value>)")]
    MissingValue,
    #[error("unknown cheat target '{0}' (expected mem: or reg:)")]
    UnknownTarget(String),
    #[error("invalid memory address: {0}")]
    InvalidAddress(String),
    #[error("invalid register name: {0}")]
    InvalidRegister(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

// ---------------------------------------------------------------------------
// Cheat rule
// ---------------------------------------------------------------------------

/// A single parsed cheat rule.
#[derive(Debug, Clone)]
pub enum CheatRule {
    /// Write `value` to memory at `addr`.  Width determined by `width`.
    Memory {
        addr: u32,
        value: u32,
        width: MemWidth,
    },
    /// Set CPU register `name` to `value`.
    Register { name: String, value: u32 },
}

/// Memory access width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemWidth {
    /// Auto-detect from value magnitude.
    Auto,
    /// Byte (u8).
    Byte,
    /// Halfword (u16).
    Halfword,
    /// Word (u32).
    Word,
}

impl FromStr for CheatRule {
    type Err = CheatParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(CheatParseError::Empty);
        }

        let (target, value_str) = s.split_once('=').ok_or(CheatParseError::MissingValue)?;

        let target = target.trim();
        let value_str = value_str.trim();

        if let Some(addr_part) = target.strip_prefix("mem:") {
            // Parse optional width prefix
            let (width, addr_str) = if let Some(rest) = addr_part.strip_prefix("b:") {
                (MemWidth::Byte, rest.trim())
            } else if let Some(rest) = addr_part.strip_prefix("h:") {
                (MemWidth::Halfword, rest.trim())
            } else if let Some(rest) = addr_part.strip_prefix("w:") {
                (MemWidth::Word, rest.trim())
            } else {
                (MemWidth::Auto, addr_part.trim())
            };

            let addr = parse_u32(addr_str)
                .map_err(|_| CheatParseError::InvalidAddress(addr_str.to_string()))?;
            let value = parse_u32(value_str)
                .map_err(|_| CheatParseError::InvalidValue(value_str.to_string()))?;

            Ok(CheatRule::Memory { addr, value, width })
        } else if let Some(reg_str) = target.strip_prefix("reg:") {
            let reg_str = reg_str.trim().to_lowercase();
            let value = parse_u32(value_str)
                .map_err(|_| CheatParseError::InvalidValue(value_str.to_string()))?;

            // Validate register name
            let _ = register_index(&reg_str)
                .map_err(|_| CheatParseError::InvalidRegister(reg_str.clone()))?;

            Ok(CheatRule::Register {
                name: reg_str,
                value,
            })
        } else {
            Err(CheatParseError::UnknownTarget(target.to_string()))
        }
    }
}

impl CheatRule {
    /// Apply this cheat rule to the emulator state.
    pub fn apply(&self, cpu: &mut ArmCpu, memory: &mut Memory) {
        match self {
            CheatRule::Memory { addr, value, width } => {
                let effective_width = match width {
                    MemWidth::Auto => {
                        if *value <= 0xFF {
                            MemWidth::Byte
                        } else if *value <= 0xFFFF {
                            MemWidth::Halfword
                        } else {
                            MemWidth::Word
                        }
                    }
                    w => *w,
                };
                let result = match effective_width {
                    MemWidth::Auto => unreachable!("resolved above"),
                    MemWidth::Byte => memory.write_u8(*addr, *value as u8),
                    MemWidth::Halfword => memory.write_u16(*addr, *value as u16),
                    MemWidth::Word => memory.write_u32(*addr, *value),
                };
                if let Err(e) = result {
                    log::warn!("Cheat mem write failed at 0x{:08X}: {}", addr, e);
                }
            }
            CheatRule::Register { name, value } => {
                if let Ok(idx) = register_index(name) {
                    if idx <= 15 {
                        let _ = cpu.set_register(idx as u32, *value);
                    } else if idx == 16 {
                        cpu.regs.cpsr = *value;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cheat slot and manager
// ---------------------------------------------------------------------------

/// A single cheat entry with enable state.
#[derive(Debug, Clone)]
pub struct CheatSlot {
    pub enabled: bool,
    pub code: String,
    pub rule: CheatRule,
}

/// Manages all cheat slots and applies them each frame.
#[derive(Debug, Default)]
pub struct CheatManager {
    slots: BTreeMap<u32, CheatSlot>,
}

impl CheatManager {
    /// Create an empty cheat manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse and insert a cheat into a specific slot.  Empty code removes the slot.
    pub fn set_slot(
        &mut self,
        index: u32,
        enabled: bool,
        code: &str,
    ) -> Result<(), CheatParseError> {
        let code = code.trim();
        if code.is_empty() {
            self.slots.remove(&index);
            return Ok(());
        }
        let rule = CheatRule::from_str(code)?;
        self.slots.insert(
            index,
            CheatSlot {
                enabled,
                code: code.to_string(),
                rule,
            },
        );
        Ok(())
    }

    /// Parse and insert a cheat with an auto-assigned slot index.
    pub fn add_code(&mut self, code: &str) -> Result<u32, CheatParseError> {
        let next = self
            .slots
            .keys()
            .next_back()
            .map_or(0, |k| k.saturating_add(1));
        self.set_slot(next, true, code)?;
        Ok(next)
    }

    /// Remove all cheat slots.
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// Apply all enabled cheats.
    pub fn apply(&self, cpu: &mut ArmCpu, memory: &mut Memory) {
        for slot in self.slots.values() {
            if slot.enabled {
                slot.rule.apply(cpu, memory);
            }
        }
    }

    /// Return the number of enabled cheats.
    pub fn active_count(&self) -> usize {
        self.slots.values().filter(|s| s.enabled).count()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a hexadecimal or decimal u32.
fn parse_u32(s: &str) -> Result<u32, std::num::ParseIntError> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
    } else {
        // Try hex first (no prefix), fall back to decimal
        u32::from_str_radix(s, 16).or_else(|_| s.parse::<u32>())
    }
}

/// Map a register name to its index (0-15 for r0-r15, 16 for cpsr).
fn register_index(name: &str) -> Result<usize, ()> {
    match name {
        "r0" => Ok(0),
        "r1" => Ok(1),
        "r2" => Ok(2),
        "r3" => Ok(3),
        "r4" => Ok(4),
        "r5" => Ok(5),
        "r6" => Ok(6),
        "r7" => Ok(7),
        "r8" => Ok(8),
        "r9" => Ok(9),
        "r10" => Ok(10),
        "r11" => Ok(11),
        "r12" => Ok(12),
        "sp" | "r13" => Ok(13),
        "lr" | "r14" => Ok(14),
        "pc" | "r15" => Ok(15),
        "cpsr" => Ok(16),
        _ => Err(()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_memory_cheat_auto_width() {
        let rule: CheatRule = "mem:0x00A00100=0xFF".parse().unwrap();
        match rule {
            CheatRule::Memory { addr, value, width } => {
                assert_eq!(addr, 0x00A00100);
                assert_eq!(value, 0xFF);
                assert_eq!(width, MemWidth::Auto);
            }
            _ => panic!("expected Memory"),
        }
    }

    #[test]
    fn parse_memory_cheat_explicit_width() {
        let rule: CheatRule = "mem:h:0A000200=1234".parse().unwrap();
        match rule {
            CheatRule::Memory { addr, value, width } => {
                assert_eq!(addr, 0x0A000200);
                assert_eq!(value, 0x1234);
                assert_eq!(width, MemWidth::Halfword);
            }
            _ => panic!("expected Memory"),
        }
    }

    #[test]
    fn parse_register_cheat() {
        let rule: CheatRule = "reg:pc=0x00A00000".parse().unwrap();
        match rule {
            CheatRule::Register { name, value } => {
                assert_eq!(name, "pc");
                assert_eq!(value, 0x00A00000);
            }
            _ => panic!("expected Register"),
        }
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(CheatRule::from_str("").is_err());
        assert!(CheatRule::from_str("  ").is_err());
    }

    #[test]
    fn parse_rejects_unknown_target() {
        assert!(CheatRule::from_str("foo:bar=1").is_err());
    }

    #[test]
    fn parse_rejects_missing_value() {
        assert!(CheatRule::from_str("mem:0x100").is_err());
    }

    #[test]
    fn parse_rejects_invalid_register() {
        assert!(CheatRule::from_str("reg:r99=0").is_err());
    }

    #[test]
    fn manager_set_and_clear() {
        let mut mgr = CheatManager::new();
        mgr.set_slot(0, true, "reg:r0=42").unwrap();
        assert_eq!(mgr.active_count(), 1);

        mgr.set_slot(0, true, "").unwrap(); // empty removes
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn manager_add_code_auto_index() {
        let mut mgr = CheatManager::new();
        let idx0 = mgr.add_code("reg:r0=1").unwrap();
        let idx1 = mgr.add_code("reg:r1=2").unwrap();
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(mgr.active_count(), 2);
    }

    #[test]
    fn apply_memory_cheat() {
        let mut memory = Memory::new();
        memory
            .map_region(0x00A00000, 0x1000, crate::memory::Permission::ALL, "code")
            .unwrap();
        let mut cpu = ArmCpu::new().unwrap();

        let rule: CheatRule = "mem:0x00A00100=0xAB".parse().unwrap();
        rule.apply(&mut cpu, &mut memory);

        assert_eq!(memory.read_u8(0x00A00100).unwrap(), 0xAB);
    }

    #[test]
    fn apply_register_cheat() {
        let mut memory = Memory::new();
        let mut cpu = ArmCpu::new().unwrap();

        let rule: CheatRule = "reg:sp=0x00F00000".parse().unwrap();
        rule.apply(&mut cpu, &mut memory);

        assert_eq!(cpu.regs.sp, 0x00F00000);
    }

    #[test]
    fn manager_disabled_slot_not_applied() {
        let mut mgr = CheatManager::new();
        mgr.set_slot(0, false, "reg:r0=999").unwrap();

        let mut memory = Memory::new();
        let mut cpu = ArmCpu::new().unwrap();
        cpu.regs.r0 = 0;

        mgr.apply(&mut cpu, &mut memory);
        assert_eq!(cpu.regs.r0, 0); // not modified
    }
}
