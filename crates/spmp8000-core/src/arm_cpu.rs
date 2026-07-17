// ARM CPU emulation
//
// A lightweight ARM instruction interpreter for SPMP8000 emulation.
// This implements the subset of ARM instructions needed by SPMP8000 games.

use crate::memory::Memory;
use anyhow::Result;

/// ARM CPU registers
#[derive(Debug, Clone)]
pub struct ArmRegisters {
    pub r0: u32,
    pub r1: u32,
    pub r2: u32,
    pub r3: u32,
    pub r4: u32,
    pub r5: u32,
    pub r6: u32,
    pub r7: u32,
    pub r8: u32,
    pub r9: u32,
    pub r10: u32,
    pub r11: u32,
    pub r12: u32,
    pub sp: u32,   // R13
    pub lr: u32,   // R14
    pub pc: u32,   // R15
    pub cpsr: u32, // Current Program Status Register
}

impl Default for ArmRegisters {
    fn default() -> Self {
        Self::new()
    }
}
impl ArmRegisters {
    pub fn new() -> Self {
        Self {
            r0: 0,
            r1: 0,
            r2: 0,
            r3: 0,
            r4: 0,
            r5: 0,
            r6: 0,
            r7: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            sp: 0,
            lr: 0,
            pc: 0,
            cpsr: 0,
        }
    }

    /// Get register by index (0-15)
    pub fn get(&self, reg: u32) -> u32 {
        match reg {
            0 => self.r0,
            1 => self.r1,
            2 => self.r2,
            3 => self.r3,
            4 => self.r4,
            5 => self.r5,
            6 => self.r6,
            7 => self.r7,
            8 => self.r8,
            9 => self.r9,
            10 => self.r10,
            11 => self.r11,
            12 => self.r12,
            13 => self.sp,
            14 => self.lr,
            15 => self.pc,
            _ => 0,
        }
    }

    /// Set register by index (0-15)
    pub fn set(&mut self, reg: u32, value: u32) {
        match reg {
            0 => self.r0 = value,
            1 => self.r1 = value,
            2 => self.r2 = value,
            3 => self.r3 = value,
            4 => self.r4 = value,
            5 => self.r5 = value,
            6 => self.r6 = value,
            7 => self.r7 = value,
            8 => self.r8 = value,
            9 => self.r9 = value,
            10 => self.r10 = value,
            11 => self.r11 = value,
            12 => self.r12 = value,
            13 => self.sp = value,
            14 => self.lr = value,
            15 => self.pc = value,
            _ => {}
        }
    }

    /// Check if condition is met
    pub fn check_condition(&self, cond: u32) -> bool {
        let z = (self.cpsr >> 30) & 1;
        let c = (self.cpsr >> 29) & 1;
        let n = (self.cpsr >> 31) & 1;
        let v = (self.cpsr >> 28) & 1;

        match cond {
            0x0 => z == 1,           // EQ
            0x1 => z == 0,           // NE
            0x2 => c == 1,           // CS/HS
            0x3 => c == 0,           // CC/LO
            0x4 => n == 1,           // MI
            0x5 => n == 0,           // PL
            0x6 => v == 1,           // VS
            0x7 => v == 0,           // VC
            0x8 => c == 1 && z == 0, // HI
            0x9 => c == 0 || z == 1, // LS
            0xA => n == v,           // GE
            0xB => n != v,           // LT
            0xC => z == 0 && n == v, // GT
            0xD => z == 1 || n != v, // LE
            0xE => true,             // AL (always)
            0xF => false,            // NV (never)
            _ => true,
        }
    }
}

/// CPU execution result
#[derive(Debug)]
pub enum CpuResult {
    /// Normal execution
    Continue,
    /// SVC instruction was executed
    SvcCall(u32),
    /// Branch/Branch with link
    Branch(u32),
    /// CPU halted
    Halt,
}

/// ARM CPU error
#[derive(Debug, thiserror::Error)]
pub enum CpuError {
    #[error("Invalid instruction: 0x{0:08X}")]
    InvalidInstruction(u32),
    #[error("Memory access error: {0}")]
    MemoryError(String),
    #[error("Undefined behavior")]
    UndefinedBehavior,
}

/// ARM CPU emulator
pub struct ArmCpu {
    /// Registers
    pub regs: ArmRegisters,
    /// Thumb mode flag
    pub thumb_mode: bool,
    /// Debug mode
    pub debug: bool,
    /// Instruction count
    pub instruction_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShiftedOperand {
    value: u32,
    carry_out: Option<u32>,
}

impl ArmCpu {
    /// Create a new ARM CPU
    pub fn new() -> Result<Self> {
        Ok(Self {
            regs: ArmRegisters::new(),
            thumb_mode: false,
            debug: false,
            instruction_count: 0,
        })
    }

    fn read_operand_register(&self, reg: u32) -> u32 {
        if reg == 15 {
            self.regs.pc.wrapping_add(4)
        } else {
            self.regs.get(reg)
        }
    }

    fn shifted_register_operand(&self, instr: u32) -> u32 {
        self.shifted_register_operand_with_carry(instr).value
    }

    fn data_processing_operand2(&self, instr: u32) -> ShiftedOperand {
        if (instr >> 25) & 1 == 1 {
            let imm = instr & 0xFF;
            let rotate = ((instr >> 8) & 0xF) * 2;
            let value = imm.rotate_right(rotate);
            ShiftedOperand {
                value,
                carry_out: if rotate == 0 {
                    None
                } else {
                    Some((value >> 31) & 1)
                },
            }
        } else {
            self.shifted_register_operand_with_carry(instr)
        }
    }

    fn shifted_register_operand_with_carry(&self, instr: u32) -> ShiftedOperand {
        let rm = instr & 0xF;
        let value = self.read_operand_register(rm);
        let shift_type = (instr >> 5) & 0x3;
        let register_shift = (instr >> 4) & 1 == 1;
        let shift_amount = if register_shift {
            let rs = (instr >> 8) & 0xF;
            self.read_operand_register(rs) & 0xFF
        } else {
            (instr >> 7) & 0x1F
        };

        let old_carry = (self.regs.cpsr >> 29) & 1;
        let unchanged = ShiftedOperand {
            value,
            carry_out: None,
        };

        match (shift_type, register_shift, shift_amount) {
            (0x0, _, 0) => unchanged,
            (0x0, _, amount @ 1..=31) => ShiftedOperand {
                value: value << amount,
                carry_out: Some((value >> (32 - amount)) & 1),
            },
            (0x0, true, 32) => ShiftedOperand {
                value: 0,
                carry_out: Some(value & 1),
            },
            (0x0, true, _) => ShiftedOperand {
                value: 0,
                carry_out: Some(0),
            },
            (0x1, false, 0) => ShiftedOperand {
                value: 0,
                carry_out: Some((value >> 31) & 1),
            },
            (0x1, true, 0) => unchanged,
            (0x1, _, amount @ 1..=31) => ShiftedOperand {
                value: value >> amount,
                carry_out: Some((value >> (amount - 1)) & 1),
            },
            (0x1, true, 32) => ShiftedOperand {
                value: 0,
                carry_out: Some((value >> 31) & 1),
            },
            (0x1, true, _) => ShiftedOperand {
                value: 0,
                carry_out: Some(0),
            },
            (0x2, false, 0) => ShiftedOperand {
                value: if value & 0x8000_0000 != 0 {
                    u32::MAX
                } else {
                    0
                },
                carry_out: Some((value >> 31) & 1),
            },
            (0x2, true, 0) => unchanged,
            (0x2, _, amount @ 1..=31) => ShiftedOperand {
                value: ((value as i32) >> amount) as u32,
                carry_out: Some((value >> (amount - 1)) & 1),
            },
            (0x2, true, _) => ShiftedOperand {
                value: if value & 0x8000_0000 != 0 {
                    u32::MAX
                } else {
                    0
                },
                carry_out: Some((value >> 31) & 1),
            },
            (0x3, false, 0) => ShiftedOperand {
                value: (value >> 1) | (old_carry << 31),
                carry_out: Some(value & 1),
            },
            (0x3, true, 0) => unchanged,
            (0x3, _, amount) => {
                let rotate = amount & 0x1F;
                if rotate == 0 {
                    ShiftedOperand {
                        value,
                        carry_out: Some((value >> 31) & 1),
                    }
                } else {
                    let shifted = value.rotate_right(rotate);
                    ShiftedOperand {
                        value: shifted,
                        carry_out: Some((value >> (rotate - 1)) & 1),
                    }
                }
            }
            _ => unchanged,
        }
    }

    /// Set the program counter
    pub fn set_pc(&mut self, addr: u32) -> Result<()> {
        self.regs.pc = addr;
        Ok(())
    }

    /// Get the program counter
    pub fn get_pc(&self) -> Result<u32> {
        Ok(self.regs.pc)
    }

    /// Set the stack pointer
    pub fn set_sp(&mut self, addr: u32) -> Result<()> {
        self.regs.sp = addr;
        Ok(())
    }

    /// Set a register value
    pub fn set_register(&mut self, reg: u32, value: u32) -> Result<()> {
        self.regs.set(reg, value);
        Ok(())
    }

    /// Get a register value
    pub fn get_register(&self, reg: u32) -> Result<u64> {
        Ok(self.regs.get(reg) as u64)
    }

    /// Execute a single instruction
    pub fn step(&mut self, memory: &mut Memory) -> std::result::Result<CpuResult, CpuError> {
        self.instruction_count += 1;

        // Fetch instruction
        let pc = self.regs.pc;
        if let Some(svc_num) = crate::function_table::fake_firmware_direct_svc(pc) {
            self.regs.pc = self.regs.lr;
            return Ok(CpuResult::SvcCall(svc_num));
        }

        let instr = memory
            .read_u32(pc)
            .map_err(|e| CpuError::MemoryError(e.to_string()))?;

        // Advance PC
        self.regs.pc = pc.wrapping_add(4);

        // Decode and execute
        self.execute_arm_instruction(instr, memory)
    }

    /// Execute an ARM instruction
    fn execute_arm_instruction(
        &mut self,
        instr: u32,
        memory: &mut Memory,
    ) -> std::result::Result<CpuResult, CpuError> {
        let cond = (instr >> 28) & 0xF;

        // Check condition
        if !self.regs.check_condition(cond) {
            return Ok(CpuResult::Continue);
        }

        if (instr & 0x0FFFFFF0) == 0x012FFF10 || (instr & 0x0FFFFFF0) == 0x012FFF30 {
            return self.execute_branch_exchange(instr);
        }

        if (instr & 0x0F8000F0) == 0x00800090 {
            return self.execute_multiply_long(instr);
        }

        if (instr & 0x0FC000F0) == 0x00000090 {
            return self.execute_multiply(instr);
        }

        if (instr & 0x0E000090) == 0x00000090 && ((instr >> 5) & 0x3) != 0 {
            return self.execute_halfword_transfer(instr, memory);
        }

        // Decode instruction type
        let opcode = (instr >> 24) & 0xF;
        let _i_bit = (instr >> 25) & 1;

        match opcode {
            // Data processing (ALU operations)
            0x0..=0x3 => self.execute_data_processing(instr, memory),
            // Load/Store
            0x4..=0x7 => self.execute_load_store(instr, memory),
            // Block data transfer (LDM/STM)
            0x8 | 0x9 => self.execute_block_transfer(instr, memory),
            // Branch
            0xA | 0xB => self.execute_branch(instr),
            // SVC (SWI)
            0xF => {
                let svc_num = instr & 0x00FFFFFF;
                Ok(CpuResult::SvcCall(svc_num))
            }
            _ => {
                log::warn!(
                    "Unknown instruction: 0x{:08X} at PC=0x{:08X}",
                    instr,
                    self.regs.pc.wrapping_sub(4)
                );
                Err(CpuError::InvalidInstruction(instr))
            }
        }
    }

    /// Execute data processing instruction
    fn execute_data_processing(
        &mut self,
        instr: u32,
        _memory: &mut Memory,
    ) -> std::result::Result<CpuResult, CpuError> {
        let opcode = (instr >> 21) & 0xF;
        let s_bit = (instr >> 20) & 1;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;

        let rn_val = self.read_operand_register(rn);
        let shifted_operand = self.data_processing_operand2(instr);
        let operand2 = shifted_operand.value;

        let result = match opcode {
            0x0 => {
                // AND
                rn_val & operand2
            }
            0x1 => {
                // EOR (XOR)
                rn_val ^ operand2
            }
            0x2 => {
                // SUB
                rn_val.wrapping_sub(operand2)
            }
            0x3 => {
                // RSB
                operand2.wrapping_sub(rn_val)
            }
            0x4 => {
                // ADD
                rn_val.wrapping_add(operand2)
            }
            0x5 => {
                // ADC
                rn_val
                    .wrapping_add(operand2)
                    .wrapping_add((self.regs.cpsr >> 29) & 1)
            }
            0x6 => {
                // SBC
                rn_val
                    .wrapping_sub(operand2)
                    .wrapping_sub(1 - ((self.regs.cpsr >> 29) & 1))
            }
            0x7 => {
                // RSC
                operand2
                    .wrapping_sub(rn_val)
                    .wrapping_sub(1 - ((self.regs.cpsr >> 29) & 1))
            }
            0x8 => {
                // TST (test, no write)
                let result = rn_val & operand2;
                if s_bit == 1 {
                    self.update_flags_logical(result, shifted_operand.carry_out);
                }
                return Ok(CpuResult::Continue);
            }
            0x9 => {
                // TEQ (test equivalence, no write)
                let result = rn_val ^ operand2;
                if s_bit == 1 {
                    self.update_flags_logical(result, shifted_operand.carry_out);
                }
                return Ok(CpuResult::Continue);
            }
            0xA => {
                // CMP (compare, no write)
                let result = rn_val.wrapping_sub(operand2);
                if s_bit == 1 {
                    self.update_flags_cmp(rn_val, operand2, result);
                }
                return Ok(CpuResult::Continue);
            }
            0xB => {
                // CMN (compare negative, no write)
                let result = rn_val.wrapping_add(operand2);
                if s_bit == 1 {
                    self.update_flags_add(rn_val, operand2, result);
                }
                return Ok(CpuResult::Continue);
            }
            0xC => {
                // ORR
                rn_val | operand2
            }
            0xD => {
                // MOV
                operand2
            }
            0xE => {
                // BIC (bit clear)
                rn_val & !operand2
            }
            0xF => {
                // MVN (move NOT)
                !operand2
            }
            _ => {
                return Err(CpuError::InvalidInstruction(instr));
            }
        };

        self.regs.set(rd, result);

        if s_bit == 1 {
            match opcode {
                0x2 => self.update_flags_cmp(rn_val, operand2, result),
                0x3 => self.update_flags_cmp(operand2, rn_val, result),
                0x4 => self.update_flags_add(rn_val, operand2, result),
                0x5 => {
                    let carry = (self.regs.cpsr >> 29) & 1;
                    self.update_flags_add_with_carry(rn_val, operand2, carry, result);
                }
                0x6 => {
                    let borrow = 1 - ((self.regs.cpsr >> 29) & 1);
                    self.update_flags_sub_with_borrow(rn_val, operand2, borrow, result);
                }
                0x7 => {
                    let borrow = 1 - ((self.regs.cpsr >> 29) & 1);
                    self.update_flags_sub_with_borrow(operand2, rn_val, borrow, result);
                }
                _ => self.update_flags_logical(result, shifted_operand.carry_out),
            }
        }

        Ok(CpuResult::Continue)
    }

    /// Execute MUL/MLA instructions.
    fn execute_multiply(&mut self, instr: u32) -> std::result::Result<CpuResult, CpuError> {
        let accumulate = ((instr >> 21) & 1) == 1;
        let set_flags = ((instr >> 20) & 1) == 1;
        let rd = (instr >> 16) & 0xF;
        let rn = (instr >> 12) & 0xF;
        let rs = (instr >> 8) & 0xF;
        let rm = instr & 0xF;

        let mut result = self.regs.get(rm).wrapping_mul(self.regs.get(rs));
        if accumulate {
            result = result.wrapping_add(self.regs.get(rn));
        }

        self.regs.set(rd, result);

        if set_flags {
            self.update_flags(result);
        }

        Ok(CpuResult::Continue)
    }

    /// Execute UMULL/UMLAL/SMULL/SMLAL instructions.
    fn execute_multiply_long(&mut self, instr: u32) -> std::result::Result<CpuResult, CpuError> {
        let signed = ((instr >> 22) & 1) == 1;
        let accumulate = ((instr >> 21) & 1) == 1;
        let set_flags = ((instr >> 20) & 1) == 1;
        let rd_hi = (instr >> 16) & 0xF;
        let rd_lo = (instr >> 12) & 0xF;
        let rs = (instr >> 8) & 0xF;
        let rm = instr & 0xF;

        let mut result = if signed {
            let lhs = self.regs.get(rm) as i32 as i64;
            let rhs = self.regs.get(rs) as i32 as i64;
            lhs.wrapping_mul(rhs) as u64
        } else {
            (self.regs.get(rm) as u64).wrapping_mul(self.regs.get(rs) as u64)
        };

        if accumulate {
            let acc = ((self.regs.get(rd_hi) as u64) << 32) | self.regs.get(rd_lo) as u64;
            result = result.wrapping_add(acc);
        }

        self.regs.set(rd_lo, result as u32);
        self.regs.set(rd_hi, (result >> 32) as u32);

        if set_flags {
            self.update_flags_64(result);
        }

        Ok(CpuResult::Continue)
    }

    /// Execute halfword and signed byte/halfword transfer instructions.
    fn execute_halfword_transfer(
        &mut self,
        instr: u32,
        memory: &mut Memory,
    ) -> std::result::Result<CpuResult, CpuError> {
        let p_bit = (instr >> 24) & 1;
        let u_bit = (instr >> 23) & 1;
        let i_bit = (instr >> 22) & 1;
        let w_bit = (instr >> 21) & 1;
        let l_bit = (instr >> 20) & 1;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;
        let op = (instr >> 5) & 0x3;

        let base = self.read_operand_register(rn);
        let offset = if i_bit == 1 {
            ((instr >> 4) & 0xF0) | (instr & 0xF)
        } else {
            self.read_operand_register(instr & 0xF)
        };

        let addr = if p_bit == 1 {
            if u_bit == 1 {
                base.wrapping_add(offset)
            } else {
                base.wrapping_sub(offset)
            }
        } else {
            base
        };

        if l_bit == 1 {
            let value = match op {
                0x1 => memory
                    .read_u16(addr)
                    .map_err(|e| CpuError::MemoryError(e.to_string()))?
                    as u32,
                0x2 => {
                    let value = memory
                        .read_u8(addr)
                        .map_err(|e| CpuError::MemoryError(e.to_string()))?;
                    (value as i8 as i32) as u32
                }
                0x3 => {
                    let value = memory
                        .read_u16(addr)
                        .map_err(|e| CpuError::MemoryError(e.to_string()))?;
                    (value as i16 as i32) as u32
                }
                _ => return Err(CpuError::InvalidInstruction(instr)),
            };
            self.regs.set(rd, value);
        } else if op == 0x1 {
            let value = self.regs.get(rd) as u16;
            memory
                .write_u16(addr, value)
                .map_err(|e| CpuError::MemoryError(e.to_string()))?;
        } else {
            return Err(CpuError::InvalidInstruction(instr));
        }

        if p_bit == 0 || w_bit == 1 {
            let writeback = if u_bit == 1 {
                base.wrapping_add(offset)
            } else {
                base.wrapping_sub(offset)
            };
            self.regs.set(rn, writeback);
        }

        Ok(CpuResult::Continue)
    }

    /// Execute load/store instruction
    fn execute_load_store(
        &mut self,
        instr: u32,
        memory: &mut Memory,
    ) -> std::result::Result<CpuResult, CpuError> {
        let i_bit = (instr >> 25) & 1;
        let p_bit = (instr >> 24) & 1;
        let u_bit = (instr >> 23) & 1;
        let b_bit = (instr >> 22) & 1;
        let w_bit = (instr >> 21) & 1;
        let l_bit = (instr >> 20) & 1;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;

        let base = self.read_operand_register(rn);

        // Calculate offset
        let offset = if i_bit == 0 {
            // Immediate offset
            instr & 0xFFF
        } else {
            self.shifted_register_operand(instr)
        };

        // Calculate address
        let addr = if p_bit == 1 {
            if u_bit == 1 {
                base.wrapping_add(offset)
            } else {
                base.wrapping_sub(offset)
            }
        } else {
            base
        };

        // Perform load or store
        if l_bit == 1 {
            // Load
            let value = if b_bit == 1 {
                memory
                    .read_u8(addr)
                    .map_err(|e| CpuError::MemoryError(e.to_string()))? as u32
            } else {
                memory
                    .read_u32(addr)
                    .map_err(|e| CpuError::MemoryError(e.to_string()))?
            };
            self.regs.set(rd, value);
        } else {
            // Store
            let value = self.regs.get(rd);
            if b_bit == 1 {
                memory
                    .write_u8(addr, value as u8)
                    .map_err(|e| CpuError::MemoryError(e.to_string()))?;
            } else {
                memory
                    .write_u32(addr, value)
                    .map_err(|e| CpuError::MemoryError(e.to_string()))?;
            }
        }

        // Writeback
        if p_bit == 0 || w_bit == 1 {
            if u_bit == 1 {
                self.regs.set(rn, base.wrapping_add(offset));
            } else {
                self.regs.set(rn, base.wrapping_sub(offset));
            }
        }

        Ok(CpuResult::Continue)
    }

    /// Execute block data transfer (LDM/STM)
    fn execute_block_transfer(
        &mut self,
        instr: u32,
        memory: &mut Memory,
    ) -> std::result::Result<CpuResult, CpuError> {
        let p_bit = (instr >> 24) & 1;
        let u_bit = (instr >> 23) & 1;
        let _s_bit = (instr >> 22) & 1;
        let w_bit = (instr >> 21) & 1;
        let l_bit = (instr >> 20) & 1;
        let rn = (instr >> 16) & 0xF;
        let register_list = instr & 0xFFFF;

        let base = self.read_operand_register(rn);
        let mut addr = base;

        // Count registers
        let count = register_list.count_ones();

        // Calculate start address
        if u_bit == 1 {
            if p_bit == 1 {
                addr = base.wrapping_add(4);
            }
        } else {
            addr = base.wrapping_sub(count * 4);
            if p_bit == 0 {
                addr = addr.wrapping_add(4);
            }
        }

        // Transfer registers
        for i in 0..16 {
            if (register_list >> i) & 1 == 1 {
                if l_bit == 1 {
                    // Load
                    let value = memory
                        .read_u32(addr)
                        .map_err(|e| CpuError::MemoryError(e.to_string()))?;
                    self.regs.set(i, value);
                } else {
                    // Store
                    let value = if i == 15 {
                        self.regs.pc.wrapping_add(8) // Store PC+8 for STM
                    } else {
                        self.regs.get(i)
                    };
                    memory
                        .write_u32(addr, value)
                        .map_err(|e| CpuError::MemoryError(e.to_string()))?;
                }
                addr = addr.wrapping_add(4);
            }
        }

        // Writeback
        if w_bit == 1 {
            if u_bit == 1 {
                self.regs.set(rn, base.wrapping_add(count * 4));
            } else {
                self.regs.set(rn, base.wrapping_sub(count * 4));
            }
        }

        Ok(CpuResult::Continue)
    }

    /// Execute BX/BLX register branch instruction
    fn execute_branch_exchange(&mut self, instr: u32) -> std::result::Result<CpuResult, CpuError> {
        let rn = instr & 0xF;
        let target = self.regs.get(rn);
        let link = (instr & 0x00000020) != 0;
        if link {
            self.regs.lr = self.regs.pc;
        }
        if target == 0xFFE0_FFE0 {
            self.regs.r0 = 0;
            self.regs.pc = self.regs.lr;
            return Ok(CpuResult::Continue);
        }
        self.thumb_mode = (target & 1) != 0;
        if self.thumb_mode {
            return Err(CpuError::InvalidInstruction(instr));
        }
        self.regs.pc = target & !1;
        Ok(CpuResult::Continue)
    }
    /// Execute branch instruction
    fn execute_branch(&mut self, instr: u32) -> std::result::Result<CpuResult, CpuError> {
        let link = (instr >> 24) & 1;
        let offset = instr & 0x00FFFFFF;

        // Sign extend offset
        let offset = if (offset >> 23) & 1 == 1 {
            offset | 0xFF000000 // Sign extend
        } else {
            offset
        };

        // Calculate target (PC + 8 + offset*4)
        let pc = self.regs.pc;
        let target = pc.wrapping_add(4).wrapping_add(offset << 2);

        if link == 1 {
            // Branch with link (BL)
            self.regs.lr = pc; // Return address
        }

        self.regs.pc = target;

        Ok(CpuResult::Continue)
    }

    fn update_flags_add_with_carry(&mut self, lhs: u32, rhs: u32, carry: u32, result: u32) {
        let z = if result == 0 { 1 } else { 0 };
        let n = (result >> 31) & 1;
        let c = if lhs as u64 + rhs as u64 + carry as u64 > 0xFFFFFFFF {
            1
        } else {
            0
        };
        let signed_sum = lhs as i32 as i64 + rhs as i32 as i64 + carry as i64;
        let v = if signed_sum < i32::MIN as i64 || signed_sum > i32::MAX as i64 {
            1
        } else {
            0
        };
        self.regs.cpsr =
            (self.regs.cpsr & !0xF0000000) | (n << 31) | (z << 30) | (c << 29) | (v << 28);
    }

    fn update_flags_sub_with_borrow(&mut self, lhs: u32, rhs: u32, borrow: u32, result: u32) {
        let z = if result == 0 { 1 } else { 0 };
        let n = (result >> 31) & 1;
        let c = if lhs as u64 >= rhs as u64 + borrow as u64 {
            1
        } else {
            0
        };
        let signed_diff = lhs as i32 as i64 - rhs as i32 as i64 - borrow as i64;
        let v = if signed_diff < i32::MIN as i64 || signed_diff > i32::MAX as i64 {
            1
        } else {
            0
        };
        self.regs.cpsr =
            (self.regs.cpsr & !0xF0000000) | (n << 31) | (z << 30) | (c << 29) | (v << 28);
    }
    /// Update flags for general result
    fn update_flags(&mut self, result: u32) {
        let z = if result == 0 { 1 } else { 0 };
        let n = (result >> 31) & 1;
        self.regs.cpsr = (self.regs.cpsr & !0xC0000000) | (n << 31) | (z << 30);
    }

    /// Update N/Z flags for a 64-bit multiply result.
    fn update_flags_64(&mut self, result: u64) {
        let z = if result == 0 { 1 } else { 0 };
        let n = ((result >> 63) & 1) as u32;
        self.regs.cpsr = (self.regs.cpsr & !0xC0000000) | (n << 31) | (z << 30);
    }

    /// Update flags for logical operations and move operations.
    fn update_flags_logical(&mut self, result: u32, carry_out: Option<u32>) {
        let z = if result == 0 { 1 } else { 0 };
        let n = (result >> 31) & 1;
        self.regs.cpsr = if let Some(c) = carry_out {
            (self.regs.cpsr & !0xE0000000) | (n << 31) | (z << 30) | (c << 29)
        } else {
            (self.regs.cpsr & !0xC0000000) | (n << 31) | (z << 30)
        };
    }

    /// Update flags for CMP
    fn update_flags_cmp(&mut self, rn: u32, operand2: u32, result: u32) {
        let z = if result == 0 { 1 } else { 0 };
        let n = (result >> 31) & 1;
        let c = if rn >= operand2 { 1 } else { 0 };
        let v = ((rn ^ operand2) & (rn ^ result)) >> 31;
        self.regs.cpsr =
            (self.regs.cpsr & !0xF0000000) | (n << 31) | (z << 30) | (c << 29) | (v << 28);
    }

    /// Update flags for ADD
    fn update_flags_add(&mut self, rn: u32, operand2: u32, result: u32) {
        let z = if result == 0 { 1 } else { 0 };
        let n = (result >> 31) & 1;
        let c = if (rn as u64 + operand2 as u64) > 0xFFFFFFFF {
            1
        } else {
            0
        };
        let v = ((rn ^ result) & (operand2 ^ result)) >> 31;
        self.regs.cpsr =
            (self.regs.cpsr & !0xF0000000) | (n << 31) | (z << 30) | (c << 29) | (v << 28);
    }
}

impl std::fmt::Debug for ArmCpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArmCpu")
            .field("pc", &format!("0x{:08X}", self.regs.pc))
            .field("sp", &format!("0x{:08X}", self.regs.sp))
            .field("lr", &format!("0x{:08X}", self.regs.lr))
            .field("instruction_count", &self.instruction_count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_creation() {
        let cpu = ArmCpu::new().unwrap();
        assert_eq!(cpu.regs.pc, 0);
        assert_eq!(cpu.regs.sp, 0);
    }

    #[test]
    fn test_registers() {
        let mut cpu = ArmCpu::new().unwrap();

        cpu.set_register(0, 0x12345678).unwrap();
        assert_eq!(cpu.get_register(0).unwrap(), 0x12345678);

        cpu.set_pc(0x1000).unwrap();
        assert_eq!(cpu.get_pc().unwrap(), 0x1000);

        cpu.set_sp(0x2000).unwrap();
        assert_eq!(cpu.get_register(13).unwrap(), 0x2000);
    }

    #[test]
    fn test_blx_register_sets_link_and_branches() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();

        cpu.regs.pc = 0x1004;
        cpu.regs.r3 = 0x2000;
        cpu.execute_arm_instruction(0xE12FFF33, &mut memory)
            .unwrap();

        assert_eq!(cpu.regs.pc, 0x2000);
        assert_eq!(cpu.regs.lr, 0x1004);
    }

    #[test]
    fn test_shifted_register_operand() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();
        memory
            .map_region(0x0, 0x1000, crate::memory::Permission::ALL, "test")
            .unwrap();

        cpu.regs.r1 = 3;
        cpu.execute_arm_instruction(0xE1A00081, &mut memory)
            .unwrap();

        assert_eq!(cpu.regs.r0, 6);
    }

    #[test]
    fn test_logical_shift_updates_carry_flag() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();

        cpu.regs.r1 = 0x8000_0000;
        cpu.execute_arm_instruction(0xE1B00081, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r0, 0);
        assert_eq!((cpu.regs.cpsr >> 29) & 1, 1);

        cpu.regs.r1 = 1;
        cpu.execute_arm_instruction(0xE1B000A1, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r0, 0);
        assert_eq!((cpu.regs.cpsr >> 29) & 1, 1);
    }

    #[test]
    fn test_immediate_rotate_updates_carry_flag() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();

        cpu.execute_arm_instruction(0xE3B00102, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r0, 0x8000_0000);
        assert_eq!((cpu.regs.cpsr >> 29) & 1, 1);
    }

    #[test]
    fn test_ldrsh_immediate_offset() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();
        memory
            .map_region(0x0, 0x2000, crate::memory::Permission::ALL, "test")
            .unwrap();

        cpu.regs.r5 = 0x1000;
        memory.write_u16(0x100A, 0xFF80).unwrap();
        cpu.execute_arm_instruction(0xE1D500FA, &mut memory)
            .unwrap();

        assert_eq!(cpu.regs.r0, 0xFFFFFF80);
    }

    #[test]
    fn test_subs_updates_carry_flag() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();

        cpu.regs.r1 = 15;
        cpu.execute_arm_instruction(0xE2512001, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r2, 14);
        assert_eq!((cpu.regs.cpsr >> 29) & 1, 1);

        cpu.regs.r1 = 0;
        cpu.execute_arm_instruction(0xE2512001, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r2, u32::MAX);
        assert_eq!((cpu.regs.cpsr >> 29) & 1, 0);
    }
    #[test]
    fn test_multiply() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();

        cpu.regs.r1 = 6;
        cpu.regs.r2 = 7;
        cpu.execute_arm_instruction(0xE0010291, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r1, 42);

        cpu.regs.r1 = 6;
        cpu.regs.r3 = 5;
        cpu.execute_arm_instruction(0xE0213291, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r1, 47);
    }

    #[test]
    fn test_multiply_long() {
        let mut cpu = ArmCpu::new().unwrap();
        let mut memory = Memory::new();

        cpu.regs.r0 = u32::MAX;
        cpu.regs.r1 = 2;
        cpu.execute_arm_instruction(0xE0832190, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r2, 0xFFFF_FFFE);
        assert_eq!(cpu.regs.r3, 1);

        cpu.regs.r0 = u32::MAX - 1;
        cpu.regs.r1 = 3;
        cpu.execute_arm_instruction(0xE0C32190, &mut memory)
            .unwrap();
        assert_eq!(cpu.regs.r2, 0xFFFF_FFFA);
        assert_eq!(cpu.regs.r3, 0xFFFF_FFFF);
    }

    #[test]
    fn test_condition_check() {
        let mut cpu = ArmCpu::new().unwrap();

        // Test AL (always)
        assert!(cpu.regs.check_condition(0xE));

        // Test EQ (equal, Z=1)
        cpu.regs.cpsr = 1 << 30; // Set Z flag
        assert!(cpu.regs.check_condition(0x0));
        assert!(!cpu.regs.check_condition(0x1));
    }
}
