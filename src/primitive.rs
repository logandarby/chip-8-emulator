// Low level primitives, like what defines an address, or an instruction, etc.

use crate::validated_struct;
use std::fmt::Display;

validated_struct! {
    pub struct Register(u8) {
        validate: is_4_bit
    }
}

impl Display for Register {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "V{:X}", self.0)
    }
}

validated_struct! {
    pub struct Address(u16) {
        validate: |value| if value == value & 0x0FFF { Ok(())} else {Err(format!("Address {value} is not a 6-bit value"))}
    }
}
impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ax{:06X}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Immediate8(u8);
impl Immediate8 {
    pub fn get(&self) -> u8 {
        self.0
    }
}
impl Display for Immediate8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#04X}", self.0)
    }
}

validated_struct! {
pub struct Immediate4(pub u8) {
  validate: is_4_bit
}
}
impl Display for Immediate4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#02X}", self.0)
    }
}

fn is_4_bit(value: u8) -> Result<(), String> {
    if value & 0xF == value {
        Ok(())
    } else {
        Err(format!("Value {value} is not 4-bits"))
    }
}

#[derive(Clone, Debug)]
pub struct RawInstruction(u16);

/*
    Convention:
    X: The second nibble. Used to look up one of the 16 registers (VX) from V0 through VF.
    Y: The third nibble. Also used to look up one of the 16 registers (VY) from V0 through VF.
    N: The fourth nibble. A 4-bit number.
    NN: The second byte (third and fourth nibbles). An 8-bit immediate number.
    NNN: The second, third and fourth nibbles. A 12-bit immediate memory address.
*/
impl RawInstruction {
    pub fn new(byte1: u8, byte2: u8) -> RawInstruction {
        RawInstruction(u16::from_be_bytes([byte1, byte2]))
    }

    pub fn to_nibbles(&self) -> (u8, u8, u8, u8) {
        (
            ((self.0 & 0xF000) >> 12) as u8,
            ((self.0 & 0x0F00) >> 8) as u8,
            ((self.0 & 0x00F0) >> 4) as u8,
            (self.0 & 0x000F) as u8,
        )
    }

    pub fn nnn(&self) -> Address {
        Address(0x0FFF & self.0)
    }

    pub fn nn(&self) -> Immediate8 {
        Immediate8((0x00FF & self.0) as u8)
    }

    pub fn x(&self) -> Register {
        Register(self.to_nibbles().1)
    }

    pub fn y(&self) -> Register {
        Register(self.to_nibbles().2)
    }

    pub fn n(&self) -> Immediate4 {
        Immediate4(self.to_nibbles().3)
    }
}

impl Display for RawInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#06X}", self.0)
    }
}

#[derive(Clone, Debug)]
pub enum RegOperation {
    Set,
    Or,
    And,
    Xor,
    Add,
    Sub,
    SubInv,
    ShiftLeft,
    ShiftRight,
}

impl Display for RegOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use RegOperation::*;
        let op = match self {
            Set => "=",
            Or => "|",
            And => "&",
            Xor => "^",
            Add => "+",
            SubInv | Sub => "-",
            ShiftLeft => "<<",
            ShiftRight => ">>",
        };
        write!(f, "{op}")
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum SkipIf {
    Eq,
    NotEq,
}

impl Display for SkipIf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if *self == SkipIf::Eq { "==" } else { "!=" })
    }
}

#[derive(Clone, Debug)]
pub enum Instruction {
    // Draw
    ClearScreen,
    Draw(Register, Register, Immediate4),
    SetFont(Register),
    // Subroutine
    Jump(Address),
    JumpWithOffset(Address),
    CallSubroutine(Address),
    Return,
    // Controlflow
    Skip(SkipIf, Register, Immediate8),
    SkipReg(SkipIf, Register, Register),
    // Keys
    SkipKeyPress(SkipIf, Register),
    GetKey(Register),
    // Register Logic
    RegOp(RegOperation, Register, Register),
    SetRegImmediate(Register, Immediate8),
    AddRegImmediate(Register, Immediate8),
    Random(Register, Immediate8),
    // Store & Load
    StoreAddr(Register),
    LoadAddr(Register),
    //Timers
    SetSoundTimer(Register),
    SetDelayTimer(Register),
    GetDelayTimer(Register),
    // Index
    SetIndex(Address),
    AddIndex(Register),
    // Misc
    BinaryDecimalConv(Register),
    // Debugging and NoOp
    // This cannot be implemented in an interpreter, and so it is a No
    ExecuteMachineLangRoutine,
    Invalid,
}

impl Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Instruction::*;
        match self {
            ClearScreen => write!(f, "ClearScreen"),
            Draw(regx, regy, value) => write!(f, "Draw {regx} {regy} {value}"),
            SetFont(regx) => write!(f, "SetFont {regx}"),
            Jump(addr) => write!(f, "Jump to {addr}"),
            JumpWithOffset(addr) => write!(f, "Jump With Offset {addr}"),
            CallSubroutine(addr) => write!(f, "Call {addr}"),
            Return => write!(f, "Return"),
            Skip(skipif, regx, value) => write!(f, "Skip if {regx} {skipif} {value}"),
            SkipReg(skipif, regx, regy) => write!(f, "Skip if {regx} {skipif} {regy}"),
            SkipKeyPress(skipif, regx) => write!(f, "Skip if key {skipif} {regx}"),
            GetKey(regx) => write!(f, "Key into {regx}"),
            RegOp(regop, regx, regy) => {
                use RegOperation::*;
                match regop {
                    Set => write!(f, "{regx} {regop} {regy}"),
                    // Equal
                    And | Or | Xor | Add | Sub => write!(f, "{regx} = {regx} {regop} {regy}"),
                    SubInv => write!(f, "{regx} = {regy} {regop} {regx}"),
                    // Implementation Dependent
                    ShiftLeft => write!(f, "Shift Left on {regx} {regy}"),
                    ShiftRight => write!(f, "Shift Right on {regx} {regy}"),
                }
            }
            SetRegImmediate(regx, value) => write!(f, "{regx} = {value}"),
            AddRegImmediate(regx, value) => write!(f, "{regx} = {regx} + {value}"),
            Random(regx, value) => write!(f, "{regx} = RANDOM & {value}"),
            StoreAddr(regx) => write!(f, "Store {}-{regx}", Register(0)),
            LoadAddr(regx) => write!(f, "Load {}-{regx}", Register(0)),
            SetSoundTimer(regx) => write!(f, "Set Sound {regx}"),
            SetDelayTimer(regx) => write!(f, "Set Delay {regx}"),
            GetDelayTimer(regx) => write!(f, "Get Delay {regx}"),
            SetIndex(addr) => write!(f, "Set Index {addr}"),
            AddIndex(regx) => write!(f, "Add Index {regx}"),
            BinaryDecimalConv(regx) => write!(f, "BinaryDecimalConv {regx}"),
            ExecuteMachineLangRoutine => write!(f, "ExecMachineLangRoutine"),
            Invalid => write!(f, "INVALID"),
        }
    }
}
