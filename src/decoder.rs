// Decodes a raw instruction into an instruction enum

use crate::primitive::*;

pub struct Decoder;

impl Decoder {
    pub fn decode(raw: &RawInstruction) -> Option<Instruction> {
        let (nibble1, nibble2, nibble3, nibble4) = raw.to_nibbles();
        Some(match (nibble1, nibble2, nibble3, nibble4) {
            // Display/Draw
            (0, 0, 0xE, 0) => Instruction::ClearScreen,
            (0x0, 0x0, 0xE, 0xE) => Instruction::Return,
            (0, _, _, _) => Instruction::ExecuteMachineLangRoutine,
            (0xD, _, _, _) => Instruction::Draw(raw.x(), raw.y(), raw.n()),
            (0xF, _, 0x2, 0x9) => Instruction::SetFont(raw.x()),

            // Subroutine
            (0x1, _, _, _) => Instruction::Jump(raw.nnn()),
            (0xB, _, _, _) => Instruction::JumpWithOffset(raw.nnn()),
            (0x2, _, _, _) => Instruction::CallSubroutine(raw.nnn()),

            // Control Flow
            (0x3, _, _, _) => Instruction::Skip(SkipIf::Eq, raw.x(), raw.nn()),
            (0x4, _, _, _) => Instruction::Skip(SkipIf::NotEq, raw.x(), raw.nn()),
            (0x5, _, _, 0x0) => Instruction::SkipReg(SkipIf::Eq, raw.x(), raw.y()),
            (0x9, _, _, 0x0) => Instruction::SkipReg(SkipIf::NotEq, raw.x(), raw.y()),

            // Keys
            (0xF, _, 0x0, 0xA) => Instruction::GetKey(raw.x()),
            (0xE, _, 0x9, 0xE) => Instruction::SkipKeyPress(SkipIf::Eq, raw.x()),
            (0xE, _, 0xA, 0x1) => Instruction::SkipKeyPress(SkipIf::NotEq, raw.x()),

            // Register Logic
            (0x6, _, _, _) => Instruction::SetRegImmediate(raw.x(), raw.nn()),
            (0x7, _, _, _) => Instruction::AddRegImmediate(raw.x(), raw.nn()),
            (0x8, _, _, op) => {
                let reg_op: RegOperation = match op {
                    0x0 => RegOperation::Set,
                    0x1 => RegOperation::Or,
                    0x2 => RegOperation::And,
                    0x3 => RegOperation::Xor,
                    0x4 => RegOperation::Add,
                    0x5 => RegOperation::Sub,
                    0x7 => RegOperation::SubInv,
                    0x6 => RegOperation::ShiftRight,
                    0xE => RegOperation::ShiftLeft,
                    _ => return None,
                };
                Instruction::RegOp(reg_op, raw.x(), raw.y())
            }

            // Store and Load
            (0xF, _, 0x5, 0x5) => Instruction::StoreAddr(raw.x()),
            (0xF, _, 0x6, 0x5) => Instruction::LoadAddr(raw.x()),

            // Timers
            (0xF, _, 0x0, 0x7) => Instruction::GetDelayTimer(raw.x()),
            (0xF, _, 0x1, 0x5) => Instruction::SetDelayTimer(raw.x()),
            (0xF, _, 0x1, 0x8) => Instruction::SetSoundTimer(raw.x()),

            // Index
            (0xA, _, _, _) => Instruction::SetIndex(raw.nnn()),
            (0xF, _, 0x1, 0xE) => Instruction::AddIndex(raw.x()),

            // Misc
            (0xC, _, _, _) => Instruction::Random(raw.x(), raw.nn()),
            (0xF, _, 0x3, 0x3) => Instruction::BinaryDecimalConv(raw.x()),
            _ => return None,
        })
    }
}
