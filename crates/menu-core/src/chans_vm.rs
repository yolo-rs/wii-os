//! Pure-Rust implementation of Nintendo's Channel Script Virtual Machine (`CHANSVm`).
//!
//! Ported directly from the authentic Wii IPL reverse-engineering sources
//! (`src/channelScript/CHANSVm.c`, `include/channelScript/CHANSVm/VmOpcodes.h`, `VmTypes.h`).
//! This engine executes compiled bytecode inside channel banners (`banner.bin`) and UI scripts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CHANSVm Instruction Opcodes matching `VmOpcodes.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChansVmOpcode {
    /* Base instruction set (opcode & 0xC0 == 0x00) */
    Return = 0x00,
    ReturnValue = 0x01,
    IsClass = 0x02,
    NewArray = 0x03,
    Push = 0x04,
    Pop = 0x05,

    /* Binary operations */
    Add = 0x06,
    Sub = 0x07,
    Mul = 0x08,
    Div = 0x09,
    Mod = 0x0A,
    BitAnd = 0x0B,
    BitOr = 0x0C,
    BitXor = 0x0D,
    UlShift = 0x0E,
    ArShift = 0x0F,
    CmpEq = 0x10,
    CmpNeq = 0x11,
    CmpLt = 0x12,
    CmpGt = 0x13,
    CmpLeq = 0x14,
    CmpGeq = 0x15,

    /* Unary operations */
    BitNot = 0x16,
    LogNot = 0x17,

    /* Immediate loads */
    LoadImm1 = 0x18,
    AddImm = 0x19,
    SubImm = 0x1A,
    MulImm = 0x1B,
    DivImm = 0x1C,
    ModImm = 0x1D,
    AndImm = 0x1E,
    OrImm = 0x1F,
    XorImm = 0x20,
    CmpEqImm = 0x21,
    CmpNeqImm = 0x22,
    CmpLtImm = 0x23,
    CmpGtImm = 0x24,
    CmpLeqImm = 0x25,
    CmpGeqImm = 0x26,
    LoadImm2 = 0x27,
    LoadImm4 = 0x28,
    LoadImm8 = 0x29,
    LoadFloat = 0x2A,

    /* Object access */
    StoreUndefined = 0x2B,
    LoadStringConst = 0x2C,
    SetIndex = 0x2D,

    /* Invocation */
    CallMethod = 0x30,
    CallFunction = 0x31,
    PropGet = 0x32,
    PropSet = 0x33,

    /* Iteration & jump */
    GetPropertyName = 0x34,
    Jump = 0x35,

    /* Deletion */
    DeleteSymbol = 0x36,
    DeleteIndirect = 0x37,

    /* Indirect access */
    LoadIndirect = 0x3E,
    StoreIndirect = 0x3F,
}

/// Dynamic value representation within CHANSVm matching `CHANSVmObjHdr`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ChansVmValue {
    Undefined,
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<ChansVmValue>),
    Object(HashMap<String, ChansVmValue>),
    NativeHandle(u32),
}

impl Default for ChansVmValue {
    fn default() -> Self {
        Self::Undefined
    }
}

impl ChansVmValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Boolean(b) => *b,
            Self::Integer(i) => *i != 0,
            Self::Float(f) => *f != 0.0 && !f.is_nan(),
            Self::String(s) => !s.is_empty(),
            Self::Array(a) => !a.is_empty(),
            Self::Object(_) | Self::NativeHandle(_) => true,
        }
    }

    pub fn to_int(&self) -> i64 {
        match self {
            Self::Undefined | Self::Null => 0,
            Self::Boolean(b) => if *b { 1 } else { 0 },
            Self::Integer(i) => *i,
            Self::Float(f) => *f as i64,
            Self::String(s) => s.parse::<i64>().unwrap_or(0),
            Self::Array(a) => a.len() as i64,
            Self::Object(_) | Self::NativeHandle(_) => 0,
        }
    }

    pub fn to_float(&self) -> f64 {
        match self {
            Self::Undefined | Self::Null => 0.0,
            Self::Boolean(b) => if *b { 1.0 } else { 0.0 },
            Self::Integer(i) => *i as f64,
            Self::Float(f) => *f,
            Self::String(s) => s.parse::<f64>().unwrap_or(0.0),
            Self::Array(a) => a.len() as f64,
            Self::Object(_) | Self::NativeHandle(_) => 0.0,
        }
    }
}

/// Execution state of the Channel Script Virtual Machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChansVm {
    /// Primary accumulator register
    pub accumulator: ChansVmValue,
    /// Operand stack
    pub stack: Vec<ChansVmValue>,
    /// Global and local symbol table (indexed by 13-bit symbol ID)
    pub symbols: HashMap<u16, ChansVmValue>,
    /// String constant table
    pub string_table: Vec<String>,
    /// Instruction pointer (PC)
    pub pc: usize,
    /// Bytecode buffer
    #[serde(skip)]
    pub bytecode: Vec<u8>,
    /// Execution status flag
    pub halted: bool,
    /// Instruction step counter for profiling/watchdog
    pub cycle_count: u64,
}

impl ChansVm {
    /// Create a new virtual machine instance with bytecode and string table.
    pub fn new(bytecode: Vec<u8>, string_table: Vec<String>) -> Self {
        Self {
            accumulator: ChansVmValue::Undefined,
            stack: Vec::with_capacity(128),
            symbols: HashMap::new(),
            string_table,
            pc: 0,
            bytecode,
            halted: false,
            cycle_count: 0,
        }
    }

    /// Reset VM state while preserving bytecode and string pool.
    pub fn reset(&mut self) {
        self.accumulator = ChansVmValue::Undefined;
        self.stack.clear();
        self.symbols.clear();
        self.pc = 0;
        self.halted = false;
        self.cycle_count = 0;
    }

    /// Step execution of a single instruction. Returns `Ok(true)` if still running.
    pub fn step(&mut self) -> Result<bool, String> {
        if self.halted || self.pc >= self.bytecode.len() {
            self.halted = true;
            return Ok(false);
        }

        let op_byte = self.bytecode[self.pc];
        self.pc += 1;
        self.cycle_count += 1;

        match op_byte & 0xC0 {
            0x00 => self.execute_base(op_byte)?,
            0x40 => self.execute_symbol(op_byte)?,
            0x80 | 0xC0 => self.execute_branch(op_byte)?,
            _ => unreachable!(),
        }

        Ok(!self.halted && self.pc < self.bytecode.len())
    }

    /// Run execution until halt or until maximum instruction cycles are reached.
    pub fn run(&mut self, max_cycles: u64) -> Result<u64, String> {
        let start_cycles = self.cycle_count;
        while !self.halted && self.pc < self.bytecode.len() {
            if self.cycle_count - start_cycles >= max_cycles {
                return Err(format!("CHANSVm execution exceeded watchdog budget of {max_cycles} cycles"));
            }
            if !self.step()? {
                break;
            }
        }
        Ok(self.cycle_count - start_cycles)
    }

    /* ---- Opcode Execution Handlers ---- */

    fn execute_base(&mut self, op: u8) -> Result<(), String> {
        match op {
            0x00 => { // RETURN
                self.accumulator = ChansVmValue::Undefined;
                self.halted = true;
            }
            0x01 => { // RETURN_VALUE
                self.halted = true;
            }
            0x03 => { // NEW_ARRAY
                let size = self.accumulator.to_int().max(0) as usize;
                self.accumulator = ChansVmValue::Array(vec![ChansVmValue::Undefined; size]);
            }
            0x04 => { // PUSH
                self.stack.push(self.accumulator.clone());
            }
            0x05 => { // POP
                self.accumulator = self.stack.pop().unwrap_or(ChansVmValue::Undefined);
            }
            0x06 => { // ADD
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0));
                self.accumulator = match (&left, &self.accumulator) {
                    (ChansVmValue::Integer(a), ChansVmValue::Integer(b)) => ChansVmValue::Integer(a.wrapping_add(*b)),
                    (ChansVmValue::Float(a), ChansVmValue::Float(b)) => ChansVmValue::Float(a + b),
                    (ChansVmValue::String(a), ChansVmValue::String(b)) => ChansVmValue::String(format!("{a}{b}")),
                    (a, b) => ChansVmValue::Float(a.to_float() + b.to_float()),
                };
            }
            0x07 => { // SUB
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0));
                self.accumulator = match (&left, &self.accumulator) {
                    (ChansVmValue::Integer(a), ChansVmValue::Integer(b)) => ChansVmValue::Integer(a.wrapping_sub(*b)),
                    (a, b) => ChansVmValue::Float(a.to_float() - b.to_float()),
                };
            }
            0x08 => { // MUL
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0));
                self.accumulator = match (&left, &self.accumulator) {
                    (ChansVmValue::Integer(a), ChansVmValue::Integer(b)) => ChansVmValue::Integer(a.wrapping_mul(*b)),
                    (a, b) => ChansVmValue::Float(a.to_float() * b.to_float()),
                };
            }
            0x09 => { // DIV
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0));
                let denom = self.accumulator.to_float();
                if denom == 0.0 {
                    self.accumulator = ChansVmValue::Float(f64::NAN);
                } else {
                    self.accumulator = ChansVmValue::Float(left.to_float() / denom);
                }
            }
            0x0A => { // MOD
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int();
                let right = self.accumulator.to_int();
                if right == 0 {
                    self.accumulator = ChansVmValue::Integer(0);
                } else {
                    self.accumulator = ChansVmValue::Integer(left % right);
                }
            }
            0x0B => { // BIT_AND
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int();
                self.accumulator = ChansVmValue::Integer(left & self.accumulator.to_int());
            }
            0x0C => { // BIT_OR
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int();
                self.accumulator = ChansVmValue::Integer(left | self.accumulator.to_int());
            }
            0x0D => { // BIT_XOR
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int();
                self.accumulator = ChansVmValue::Integer(left ^ self.accumulator.to_int());
            }
            0x0E => { // ULSHIFT
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int() as u64;
                let shift = (self.accumulator.to_int() & 63) as u32;
                self.accumulator = ChansVmValue::Integer((left << shift) as i64);
            }
            0x0F => { // ARSHIFT
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int();
                let shift = (self.accumulator.to_int() & 63) as u32;
                self.accumulator = ChansVmValue::Integer(left >> shift);
            }
            0x10 => { // CMP_EQ
                let left = self.stack.pop().unwrap_or(ChansVmValue::Undefined);
                self.accumulator = ChansVmValue::Boolean(left == self.accumulator);
            }
            0x11 => { // CMP_NEQ
                let left = self.stack.pop().unwrap_or(ChansVmValue::Undefined);
                self.accumulator = ChansVmValue::Boolean(left != self.accumulator);
            }
            0x12 => { // CMP_LT
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_float();
                self.accumulator = ChansVmValue::Boolean(left < self.accumulator.to_float());
            }
            0x13 => { // CMP_GT
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_float();
                self.accumulator = ChansVmValue::Boolean(left > self.accumulator.to_float());
            }
            0x14 => { // CMP_LEQ
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_float();
                self.accumulator = ChansVmValue::Boolean(left <= self.accumulator.to_float());
            }
            0x15 => { // CMP_GEQ
                let left = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_float();
                self.accumulator = ChansVmValue::Boolean(left >= self.accumulator.to_float());
            }
            0x16 => { // BIT_NOT
                self.accumulator = ChansVmValue::Integer(!self.accumulator.to_int());
            }
            0x17 => { // LOG_NOT
                self.accumulator = ChansVmValue::Boolean(!self.accumulator.is_truthy());
            }
            0x18 => { // LOAD_IMM_1
                let imm = self.fetch_u8()? as i8 as i64;
                self.accumulator = ChansVmValue::Integer(imm);
            }
            0x19 => { // ADD_IMM
                let imm = self.fetch_u8()? as i8 as i64;
                self.accumulator = ChansVmValue::Integer(self.accumulator.to_int().wrapping_add(imm));
            }
            0x27 => { // LOAD_IMM_2
                let b0 = self.fetch_u8()? as i16;
                let b1 = self.fetch_u8()? as i16;
                let imm = ((b0 << 8) | b1) as i64;
                self.accumulator = ChansVmValue::Integer(imm);
            }
            0x28 => { // LOAD_IMM_4
                let imm = self.fetch_u32()? as i32 as i64;
                self.accumulator = ChansVmValue::Integer(imm);
            }
            0x29 => { // LOAD_IMM_8
                let imm = self.fetch_u64()? as i64;
                self.accumulator = ChansVmValue::Integer(imm);
            }
            0x2A => { // LOAD_FLOAT
                let bits = self.fetch_u32()?;
                let f = f32::from_bits(bits) as f64;
                self.accumulator = ChansVmValue::Float(f);
            }
            0x2B => { // STORE_UNDEFINED
                self.accumulator = ChansVmValue::Undefined;
            }
            0x2C => { // LOAD_STRING_CONST
                let idx = self.fetch_u16()? as usize;
                if idx < self.string_table.len() {
                    self.accumulator = ChansVmValue::String(self.string_table[idx].clone());
                } else {
                    self.accumulator = ChansVmValue::String(String::new());
                }
            }
            0x2D => { // SET_INDEX
                let val = self.stack.pop().unwrap_or(ChansVmValue::Undefined);
                let idx = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int().max(0) as usize;
                if let ChansVmValue::Array(ref mut arr) = self.accumulator {
                    if idx >= arr.len() {
                        arr.resize(idx + 1, ChansVmValue::Undefined);
                    }
                    arr[idx] = val;
                }
            }
            0x35 => { // JUMP (24-bit signed relative jump)
                let b0 = self.fetch_u8()? as u32;
                let b1 = self.fetch_u8()? as u32;
                let b2 = self.fetch_u8()? as u32;
                let raw = (b0 << 16) | (b1 << 8) | b2;
                let offset = if (raw & 0x0080_0000) != 0 {
                    // Sign extend 24-bit negative offset
                    (raw | 0xFF00_0000) as i32
                } else {
                    raw as i32
                };
                self.apply_pc_offset(offset)?;
            }
            0x3E => { // LOAD_INDIRECT
                let idx = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int().max(0) as usize;
                self.accumulator = match &self.accumulator {
                    ChansVmValue::Array(arr) => arr.get(idx).cloned().unwrap_or(ChansVmValue::Undefined),
                    _ => ChansVmValue::Undefined,
                };
            }
            0x3F => { // STORE_INDIRECT
                let val = self.stack.pop().unwrap_or(ChansVmValue::Undefined);
                let idx = self.stack.pop().unwrap_or(ChansVmValue::Integer(0)).to_int().max(0) as usize;
                if let ChansVmValue::Array(ref mut arr) = self.accumulator {
                    if idx >= arr.len() {
                        arr.resize(idx + 1, ChansVmValue::Undefined);
                    }
                    arr[idx] = val;
                }
            }
            _ => return Err(format!("Unimplemented CHANSVm base opcode 0x{op:02X} at PC 0x{:04X}", self.pc - 1)),
        }
        Ok(())
    }

    fn execute_symbol(&mut self, op: u8) -> Result<(), String> {
        let is_store = (op & 0x20) != 0;
        let high = (op & 0x1F) as u16;
        let low = self.fetch_u8()? as u16;
        let symbol_id = (high << 8) | low;

        if is_store {
            self.symbols.insert(symbol_id, self.accumulator.clone());
        } else {
            self.accumulator = self.symbols.get(&symbol_id).cloned().unwrap_or(ChansVmValue::Undefined);
        }
        Ok(())
    }

    fn execute_branch(&mut self, op: u8) -> Result<(), String> {
        let cond = op & 0x60;
        let sign = (op & 0x10) != 0;
        let high = (op & 0x0F) as u16;
        let low = self.fetch_u8()? as u16;
        let magnitude = ((high << 8) | low) as i32;
        let offset = if sign { -magnitude } else { magnitude };

        let take_branch = match cond {
            0x00 => { // BRANCH_CASE (equality against top of stack)
                let top = self.stack.last().unwrap_or(&ChansVmValue::Undefined);
                *top == self.accumulator
            }
            0x20 => !self.accumulator.is_truthy(), // BRANCH_FALSE
            0x40 => self.accumulator.is_truthy(),  // BRANCH_TRUE
            0x60 => true,                          // BRANCH_ALWAYS
            _ => false,
        };

        if take_branch {
            self.apply_pc_offset(offset)?;
        }
        Ok(())
    }

    /* ---- Helpers ---- */

    #[inline]
    fn fetch_u8(&mut self) -> Result<u8, String> {
        if self.pc >= self.bytecode.len() {
            return Err("Unexpected EOF reading byte operand".into());
        }
        let b = self.bytecode[self.pc];
        self.pc += 1;
        Ok(b)
    }

    #[inline]
    fn fetch_u16(&mut self) -> Result<u16, String> {
        let b0 = self.fetch_u8()? as u16;
        let b1 = self.fetch_u8()? as u16;
        Ok((b0 << 8) | b1)
    }

    #[inline]
    fn fetch_u32(&mut self) -> Result<u32, String> {
        let b0 = self.fetch_u8()? as u32;
        let b1 = self.fetch_u8()? as u32;
        let b2 = self.fetch_u8()? as u32;
        let b3 = self.fetch_u8()? as u32;
        Ok((b0 << 24) | (b1 << 16) | (b2 << 8) | b3)
    }

    #[inline]
    fn fetch_u64(&mut self) -> Result<u64, String> {
        let hi = self.fetch_u32()? as u64;
        let lo = self.fetch_u32()? as u64;
        Ok((hi << 32) | lo)
    }

    #[inline]
    fn apply_pc_offset(&mut self, offset: i32) -> Result<(), String> {
        let new_pc = (self.pc as i64) + (offset as i64);
        if new_pc < 0 || (new_pc as usize) > self.bytecode.len() {
            return Err(format!("Branch offset {offset} targets out of bounds PC {new_pc}"));
        }
        self.pc = new_pc as usize;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chans_vm_arithmetic() {
        // Compute (10 + 25) * 2 = 70
        let bytecode = vec![
            0x18, 10,   // LoadImm1 10
            0x04,       // Push
            0x18, 25,   // LoadImm1 25
            0x06,       // Add -> accumulator = 35
            0x04,       // Push
            0x18, 2,    // LoadImm1 2
            0x08,       // Mul -> accumulator = 70
            0x01,       // ReturnValue
        ];
        let mut vm = ChansVm::new(bytecode, vec![]);
        let cycles = vm.run(100).expect("run failed");
        assert!(cycles > 0);
        assert_eq!(vm.accumulator, ChansVmValue::Integer(70));
    }

    #[test]
    fn test_chans_vm_symbol_store_and_load() {
        let bytecode = vec![
            0x18, 42,       // LoadImm1 42
            0x60, 0x07,     // SymbolStore symbol #7
            0x18, 0,        // LoadImm1 0
            0x40, 0x07,     // SymbolLoad symbol #7
            0x01,           // ReturnValue
        ];
        let mut vm = ChansVm::new(bytecode, vec![]);
        vm.run(100).expect("run failed");
        assert_eq!(vm.accumulator, ChansVmValue::Integer(42));
        assert_eq!(vm.symbols.get(&7), Some(&ChansVmValue::Integer(42)));
    }

    #[test]
    fn test_chans_vm_branching() {
        // if (5 < 10) branch over error to success
        let bytecode = vec![
            0x18, 5,        // LoadImm1 5
            0x04,           // Push
            0x18, 10,       // LoadImm1 10
            0x12,           // CmpLt -> true
            0xC0, 0x02,     // BranchTrue +2 bytes (skip LoadImm1 99: opcode 0x18, operand 99)
            0x18, 99,       // LoadImm1 99
            0x18, 1,        // LoadImm1 1 (target)
            0x01,           // ReturnValue
        ];
        let mut vm = ChansVm::new(bytecode, vec![]);
        vm.run(100).expect("run failed");
        assert_eq!(vm.accumulator, ChansVmValue::Integer(1));
    }
}
