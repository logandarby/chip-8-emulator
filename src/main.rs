use core::fmt;
use crossterm::{self, cursor::Show, execute, terminal::LeaveAlternateScreen};
use std::{
    fs,
    io::{self, Write},
    thread,
    time::Duration,
};

#[derive(Debug)]
struct Register(u8);
#[derive(Debug)]
struct Address(u16);
#[derive(Debug)]
struct Immediate8(u8);
#[derive(Debug)]
struct Immediate4(u8);
#[derive(Clone)]
struct RawInstruction(u16);

/*
    Convention:
    X: The second nibble. Used to look up one of the 16 registers (VX) from V0 through VF.
    Y: The third nibble. Also used to look up one of the 16 registers (VY) from V0 through VF.
    N: The fourth nibble. A 4-bit number.
    NN: The second byte (third and fourth nibbles). An 8-bit immediate number.
    NNN: The second, third and fourth nibbles. A 12-bit immediate memory address.
*/

impl RawInstruction {
    fn new(byte1: u8, byte2: u8) -> RawInstruction {
        RawInstruction(u16::from_be_bytes([byte1, byte2]))
    }

    fn to_nibbles(&self) -> (u8, u8, u8, u8) {
        (
            ((self.0 & 0xF000) >> 12) as u8,
            ((self.0 & 0x0F00) >> 8) as u8,
            ((self.0 & 0x00F0) >> 4) as u8,
            (self.0 & 0x000F) as u8,
        )
    }

    // fn opcode(&self) -> u8 {
    //     self.to_nibbles().0
    // }

    fn nnn(&self) -> Address {
        Address(0x0FFF & self.0)
    }

    fn nn(&self) -> Immediate8 {
        Immediate8((0x00FF & self.0) as u8)
    }

    fn x(&self) -> Register {
        Register(self.to_nibbles().1)
    }

    fn y(&self) -> Register {
        Register(self.to_nibbles().2)
    }

    fn n(&self) -> Immediate4 {
        Immediate4(self.to_nibbles().3)
    }
}

impl fmt::Display for RawInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#06X}", self.0)
    }
}

#[derive(Debug)]
enum RegOperation {
    Set,
    OR,
    AND,
    XOR,
    Add,
    Sub,
    SubInv,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, PartialEq)]
enum SkipIf {
    Eq,
    NotEq,
}

#[derive(Debug)]
enum Instruction {
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
}

impl Instruction {
    fn decode(raw: &RawInstruction) -> Option<Instruction> {
        let (nibble1, nibble2, nibble3, nibble4) = raw.to_nibbles();
        Some(match (nibble1, nibble2, nibble3, nibble4) {
            // Display/Draw
            (0, 0, 0xE, 0) => Instruction::ClearScreen,
            (0xD, _, _, _) => Instruction::Draw(raw.x(), raw.y(), raw.n()),
            (0xF, _, 0x2, 0x9) => Instruction::SetFont(raw.x()),

            // Subroutine
            (0x1, _, _, _) => Instruction::Jump(raw.nnn()),
            (0xB, _, _, _) => Instruction::JumpWithOffset(raw.nnn()),
            (0x0, 0x0, 0xE, 0xE) => Instruction::Return,
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
                    0x1 => RegOperation::OR,
                    0x2 => RegOperation::AND,
                    0x3 => RegOperation::XOR,
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

struct Screen {
    pixels: [bool; Self::N_PIXELS as usize],
}

impl Screen {
    pub const N_ROWS: u8 = 32;
    pub const N_COLS: u8 = 64;
    pub const N_PIXELS: u16 = Self::N_ROWS as u16 * Self::N_COLS as u16;
    // Hide cursor, clear screen, and move to top-left

    fn new() -> Self {
        use crossterm::cursor::*;
        use crossterm::terminal::*;
        execute!(std::io::stdout(), EnterAlternateScreen, Hide).expect("Could not create terminal");
        Self {
            pixels: [false; Self::N_PIXELS as usize],
        }
    }

    fn get_idx(x: u8, y: u8) -> usize {
        assert!(x < Self::N_COLS, "X screen index is out of bounds");
        assert!(y < Self::N_ROWS, "Y screen index is out of bounds");
        return y as usize * Self::N_COLS as usize + x as usize;
    }

    fn get_pixel(&self, x: u8, y: u8) -> Option<bool> {
        if x >= Self::N_COLS.into() || y >= Self::N_ROWS.into() {
            return None;
        }
        return Some(self.pixels[Self::get_idx(x, y)]);
    }

    fn set_pixel(&mut self, x: u8, y: u8, value: bool) {
        if x >= Self::N_COLS.into() || y >= Self::N_ROWS.into() {
            return;
        }
        self.pixels[Self::get_idx(x, y)] = value;
    }

    fn clear(&mut self) {
        self.pixels.fill(false);
        self.flush();
    }

    // Draws to the console
    fn flush(&mut self) -> Option<()> {
        use crossterm::{cursor::*, queue, style::*, terminal::*};
        use std::io::stdout;
        let (term_width, term_height) = crossterm::terminal::size().ok()?;

        // Calculate centering offset
        let display_width = (Screen::N_COLS * 2) as u16;
        let display_height = Screen::N_ROWS as u16;
        let offset_x = (term_width.saturating_sub(display_width)) / 2;
        let offset_y = (term_height.saturating_sub(display_height)) / 2;

        // Clear screen
        queue!(stdout(), Clear(ClearType::All)).ok()?;

        // Draw display centered
        for y in 0..Screen::N_ROWS {
            queue!(stdout(), MoveTo(offset_x, offset_y + y as u16)).ok()?;
            for x in 0..Screen::N_COLS {
                let pixel = self.get_pixel(x, y).unwrap();
                if pixel {
                    queue!(stdout(), SetBackgroundColor(Color::Green), Print("  ")).ok()?;
                } else {
                    queue!(stdout(), SetBackgroundColor(Color::Black), Print("  ")).ok()?;
                }
            }
            queue!(stdout(), ResetColor).ok()?;
        }

        // Add title
        queue!(
            stdout(),
            MoveTo(offset_x, offset_y.saturating_sub(2)),
            Print("CHIP-8 Emulator"),
            MoveTo(offset_x, offset_y + display_height + 1),
            Print("Press 'q' to quit")
        )
        .ok()?;

        stdout().flush().ok()?;
        Some(())
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        crossterm::queue!(
            std::io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )
        .unwrap();
        std::io::stdout().flush().unwrap();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);
    }
}

struct Chip8 {
    screen: Screen,
    memory: [u8; Self::MEMORY_SIZE],
    pc_r: u16,                         // Program Counter
    index_r: u16,                      // Index Register
    gen_r: [u8; Self::REGISTER_COUNT], // General Purpose Registers
    stack: Vec<u16>,                   // Stack
                                       // delay timer
                                       // sound timer
}

impl Chip8 {
    pub const REGISTER_COUNT: usize = 16; // 16 General Purpose Registers
    pub const MEMORY_SIZE: usize = 4096; // 4KB memory
    const ENTRY_POINT: u16 = 0x200; // Where a program is expected to start

    // Default font loaded into memory before the application
    const FONT_START_ADDR: u16 = 0x50;
    const FONT: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
        0x20, 0x60, 0x20, 0x20, 0x70, // 1
        0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
        0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
        0x90, 0x90, 0xF0, 0x10, 0x10, // 4
        0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
        0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
        0xF0, 0x10, 0x20, 0x40, 0x40, // 7
        0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
        0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
        0xF0, 0x90, 0xF0, 0x90, 0x90, // A
        0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
        0xF0, 0x80, 0x80, 0x80, 0xF0, // C
        0xE0, 0x90, 0x90, 0x90, 0xE0, // D
        0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
        0xF0, 0x80, 0xF0, 0x80, 0x80, // F
    ];
    const BYTES_PER_FONT: u16 = 5;

    fn new() -> Self {
        Self {
            screen: Screen::new(),
            memory: [0; Self::MEMORY_SIZE],
            gen_r: [0; Self::REGISTER_COUNT],
            pc_r: Self::ENTRY_POINT,
            index_r: 0,
            stack: Vec::new(),
        }
    }

    fn load_rom(&mut self, bytes: &Vec<u8>) {
        // TODO: Do better loading here, with better error handling
        // Load Fonts into memory
        let font_start: usize = Self::FONT_START_ADDR as usize;
        let font_end: usize = font_start + Self::FONT.len();
        self.memory[font_start..font_end].copy_from_slice(&Self::FONT);

        // Load ROM into memory
        let start: usize = Self::ENTRY_POINT as usize;
        let end: usize = Self::ENTRY_POINT as usize + bytes.len();
        self.memory[start..end].copy_from_slice(bytes);
    }

    fn vf(&mut self) -> &mut u8 {
        return &mut self.gen_r[Self::REGISTER_COUNT - 1];
    }

    fn register_val_ref(&mut self, reg: &Register) -> &mut u8 {
        return &mut self.gen_r[reg.0 as usize];
    }

    fn register_val(&self, reg: &Register) -> u8 {
        return self.gen_r[reg.0 as usize];
    }

    fn register_set(&mut self, reg: &Register, value: u8) {
        self.gen_r[reg.0 as usize] = value;
    }

    fn load_from_addr(&self, addr: u16) -> u8 {
        return self.memory[addr as usize];
    }

    fn increment_pc(&mut self) {
        self.pc_r += 2;
    }

    fn fetch(&self) -> RawInstruction {
        return RawInstruction::new(
            self.memory[self.pc_r as usize],
            self.memory[self.pc_r as usize + 1],
        );
    }

    fn decode(&self, raw: &RawInstruction) -> Instruction {
        return Instruction::decode(raw).expect(
            format!(
                "Could not decode instruction {raw} at PC location {}",
                self.pc_r
            )
            .as_str(),
        );
    }

    fn execute_font_char(&mut self, reg: &Register) {
        self.index_r = Self::FONT_START_ADDR + (reg.0 as u16 * Self::BYTES_PER_FONT); 
    }

    fn execute_reg_op(&mut self, reg_op: &RegOperation, regx: &Register, regy: &Register) {
        let vx = self.register_val(regx);
        let vy = self.register_val(regy);
        // TODO: Carry flag
        match *reg_op {
            RegOperation::Set => {
                self.register_set(regx, vy);
            }
            RegOperation::OR => {
                self.register_set(regx, vx | vy);
            }
            RegOperation::XOR => {
                self.register_set(regx, vx ^ vy);
            }
            RegOperation::AND => {
                self.register_set(regx, vx & vy);
            }
            RegOperation::Add => {
                let (result, overflow) = vx.overflowing_add(vy);
                self.register_set(regx, result);
                *self.vf() = overflow as u8;
            }
            RegOperation::Sub => {
                let (result, _) = vx.overflowing_sub(vy);
                self.register_set(regx, result);
                *self.vf() = if vx > vy { 1 } else { 0 };
            }
            RegOperation::SubInv => {
                let (result, _) = vy.overflowing_sub(vx);
                self.register_set(regx, result);
                *self.vf() = if vy > vx { 1 } else { 0 };
            }
            RegOperation::ShiftLeft => {
                todo!()
            }
            RegOperation::ShiftRight => {
                todo!()
            }
        }
    }

    // Draws sprite N pixels tall located at the index register
    // at the coordinate x, y in the regX and regY registers respectively
    // All the pixels that are "on" in the sprite will flip the screen.
    // If a pixel is turned off this way, the VF register is set to 1. Otherwise, it's set
    // to 0
    // The starting coordinate wraps, but the drawing is clipped
    fn execute_draw(&mut self, regx: &Register, regy: &Register, row_count: &Immediate4) {
        let start_x = self.register_val(regx) % Screen::N_COLS;
        let start_y = self.register_val(regy) % Screen::N_ROWS;
        *self.vf() = 0;
        let index_addr = self.index_r;

        for row in 0..row_count.0 {
            let y = start_y + row;
            if y >= Screen::N_ROWS {
                break;
            }

            let sprite_data = self.load_from_addr(index_addr + row as u16);

            for bit_pos in 0..8 {
                let x = start_x + bit_pos;
                if x >= Screen::N_COLS {
                    break;
                }

                let sprite_bit = (sprite_data >> (7 - bit_pos)) & 1;
                if sprite_bit == 1 {
                    let pixel = self.screen.get_pixel(x, y).unwrap();
                    if pixel {
                        self.screen.set_pixel(x, y, false);
                        *self.vf() = 1;
                    } else {
                        self.screen.set_pixel(x, y, true);
                    }
                }
            }
        }
        self.screen.flush();
    }

    fn execute(&mut self, inst: &Instruction) {
        use Instruction::*;
        match inst {
            ClearScreen => self.screen.clear(),
            Jump(addr) => {
                self.pc_r = addr.0;
                return;
            }
            RegOp(reg_op, regx, regy) => self.execute_reg_op(reg_op, regx, regy),
            SetRegImmediate(reg, value) => *self.register_val_ref(reg) = value.0,
            AddRegImmediate(reg, value) => *self.register_val_ref(reg) += value.0,
            SetIndex(addr) => self.index_r = addr.0,
            AddIndex(_reg) => todo!(),
            Draw(regx, regy, row_count) => {
                self.execute_draw(regx, regy, row_count);
            },
            LoadAddr(_reg) => { todo!() }
            StoreAddr(_reg) => { todo!() }
            SetFont(reg) => self.execute_font_char(reg),
            JumpWithOffset(_addr) => todo!(),
            CallSubroutine(addr) => {
                self.stack.push(self.pc_r);
                self.pc_r = addr.0;
                return;
            },
            Return => {
                let return_addr = self.stack.pop().expect("CRITICAL: Stack is empty");
                self.pc_r = return_addr;
                return;
            },
            Skip(skipif, reg, value) => {
                let eq = self.register_val(reg) == value.0.into();
                if *skipif == SkipIf::Eq && eq || *skipif == SkipIf::NotEq && !eq {
                    self.increment_pc();
                }
            },
            SkipReg(skipif, regx, regy) => {
                let eq = self.register_val(regx) == self.register_val(regy);
                if *skipif == SkipIf::Eq && eq || *skipif == SkipIf::NotEq && !eq {
                    self.increment_pc();
                }
            },
            SkipKeyPress(_skipif, _reg) => todo!(),
            GetKey(_reg) => todo!(),
            Random(reg, value) => {
                let random: u8 = rand::random();
                self.register_set(reg, value.0 & random);
            },
            SetSoundTimer(_reg) => todo!(),
            SetDelayTimer(_reg) => todo!(),
            GetDelayTimer(_reg) => todo!(),
            BinaryDecimalConv(_reg) => todo!(),
        };
        self.increment_pc();
    }

    fn cycle(&mut self) {
        loop {
            let raw = self.fetch();
            let inst = self.decode(&raw);
            self.execute(&inst);
            // println!("Executing instruction {raw} as {:#?}", inst);
            thread::sleep(Duration::from_millis(102));
        }
    }
}

fn main() -> io::Result<()> {
    let filename = "roms/IBM Logo.ch8";
    let bytes = fs::read(filename).expect(format!("Could not read file {}", filename).as_str());
    let mut chip8 = Chip8::new();
    chip8.load_rom(&bytes);
    chip8.cycle();
    Ok(())
}
