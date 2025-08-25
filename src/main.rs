use core::fmt;
use crossterm::{self, cursor::Show, execute, terminal::LeaveAlternateScreen};
use std::{
    fmt::{Debug, Display},
    fs,
    io::{self, Write},
    thread,
    time::{Duration, Instant},
};

struct Register(u8);
impl Display for Register {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "V{:X}", self.0)
    }
}
struct Address(u16);
impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ax{:06X}", self.0)
    }
}

struct Immediate8(u8);
impl Display for Immediate8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#04X}", self.0)
    }
}
struct Immediate4(u8);
impl Display for Immediate4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#02X}", self.0)
    }
}

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

impl Display for RegOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use RegOperation::*;
        let op = match self {
            Set => "=",
            OR => "|",
            AND => "&",
            XOR => "^",
            Add => "+",
            SubInv | Sub => "-",
            ShiftLeft => "<<",
            ShiftRight => ">>",
        };
        write!(f, "{op}")
    }
}

#[derive(PartialEq)]
enum SkipIf {
    Eq,
    NotEq,
}

impl Display for SkipIf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if *self == SkipIf::Eq { "==" } else { "!=" })
    }
}

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
                    AND | OR | XOR | Add | Sub => write!(f, "{regx} = {regx} {regop} {regy}"),
                    SubInv => write!(f, "{regx} = {regy} {regop} {regx}"),
                    // Implementation Dependent
                    ShiftLeft => write!(f, "Shift Left on {regx} {regy}"),
                    ShiftRight => write!(f, "Shift Right on {regx} {regy}"),
                }
            },
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

impl Instruction {
    fn decode(raw: &RawInstruction) -> Option<Instruction> {
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
        // self.flush().unwrap();
    }

    // Draws to the console
    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{cursor::*, queue, style::*, terminal::*};
        use std::io::stdout;
        let (term_width, term_height) = crossterm::terminal::size()?;

        // Calculate centering offset
        let display_width = (Screen::N_COLS * 2) as u16;
        let display_height = Screen::N_ROWS as u16;
        let offset_x = (term_width.saturating_sub(display_width)) / 2;
        let offset_y = (term_height.saturating_sub(display_height)) / 2;

        // Draw display centered
        for y in 0..Screen::N_ROWS {
            queue!(stdout(), MoveTo(offset_x, offset_y + y as u16))?;
            for x in 0..Screen::N_COLS {
                let pixel = self.get_pixel(x, y).unwrap();
                if pixel {
                    queue!(stdout(), SetBackgroundColor(Color::Green), Print("  "))?;
                } else {
                    queue!(stdout(), SetBackgroundColor(Color::Black), Print("  "))?;
                }
            }
            queue!(stdout(), ResetColor)?;
        }

        // Add title
        queue!(
            stdout(),
            MoveTo(offset_x, offset_y.saturating_sub(2)),
            Print("CHIP-8 Emulator"),
            MoveTo(offset_x, offset_y + display_height + 1),
            Print("Press 'Escape' to quit")
        )?;

        stdout().flush()?;
        Ok(())
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

pub struct InputHandler {
    keys: [bool; 16], // CHIP-8 has 16 keys (0-F)
}

impl InputHandler {
    pub fn new() -> Self {
        Self { keys: [false; 16] }
    }

    // Non-blocking input polling
    pub fn update(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        use crossterm::event;
        use crossterm::event::*;
        // Poll for events with timeout
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = event::read()? {
                match key_event.code {
                    KeyCode::Esc => return Ok(false), // Quit
                    _ => self.handle_key_event(key_event),
                }
            }
        }
        Ok(true) // Continue running
    }

    fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        use crossterm::event::*;
        let pressed = match key_event.kind {
            crossterm::event::KeyEventKind::Press => true,
            crossterm::event::KeyEventKind::Release => false,
            _ => return,
        };

        // Map physical keys to CHIP-8 hex keypad
        let chip8_key = match key_event.code {
            KeyCode::Char('1') => Some(0x1),
            KeyCode::Char('2') => Some(0x2),
            KeyCode::Char('3') => Some(0x3),
            KeyCode::Char('4') => Some(0xC),
            KeyCode::Char('q') => Some(0x4),
            KeyCode::Char('w') => Some(0x5),
            KeyCode::Char('e') => Some(0x6),
            KeyCode::Char('r') => Some(0xD),
            KeyCode::Char('a') => Some(0x7),
            KeyCode::Char('s') => Some(0x8),
            KeyCode::Char('d') => Some(0x9),
            KeyCode::Char('f') => Some(0xE),
            KeyCode::Char('z') => Some(0xA),
            KeyCode::Char('x') => Some(0x0),
            KeyCode::Char('c') => Some(0xB),
            KeyCode::Char('v') => Some(0xF),
            _ => None,
        };

        if let Some(key_id) = chip8_key {
            self.keys[key_id] = pressed;
            println!(
                "Key {:X}: {}",
                key_id,
                if pressed { "pressed" } else { "released" }
            );
        }
    }

    // Check if a specific CHIP-8 key is pressed
    pub fn is_key_pressed(&self, key: u8) -> bool {
        if key <= 0xF {
            self.keys[key as usize]
        } else {
            false
        }
    }

    // Get the first pressed key (for CHIP-8 wait-for-key instruction)
    pub fn get_pressed_key(&self) -> Option<u8> {
        for (i, &pressed) in self.keys.iter().enumerate() {
            if pressed {
                return Some(i as u8);
            }
        }
        None
    }
}

#[derive(Debug, PartialEq)]
enum Chip8Version {
    COSMAC,
    CHIP48,
    SUPERCHIP,
}

struct Chip8 {
    version: Chip8Version,
    screen: Screen,
    input: InputHandler,
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
    const CPU_FREQ_HZ: u16 = 500;
    const TIMER_FREQ_HQ: u16 = 60;
    const INPUT_POLL_RATE: Duration = Duration::from_millis(50);

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

    fn new(version: Chip8Version) -> Self {
        Self {
            version: version,
            screen: Screen::new(),
            input: InputHandler::new(),
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

    // Dumps the instructions contained in the bytes
    fn dump_inst(bytes: &Vec<u8>) {
        println!("Dumping instruction hex codes:");
        bytes
            .chunks_exact(2)
            .map(|chunk| RawInstruction::new(chunk[0], chunk[1]))
            .enumerate()
            .for_each(|(index, raw)| {
                let inst = Instruction::decode(&raw);
                let addr = Address(Self::ENTRY_POINT + index as u16 * 2);
                println!(
                    "{}: Code {}, {}",
                    addr,
                    raw,
                    inst.unwrap_or(Instruction::Invalid)
                );
            });
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

    fn store_in_addr(&mut self, addr: u16, value: u8) {
        self.memory[addr as usize] = value;
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

    fn execute_reg_op(&mut self, reg_op: &RegOperation, regx: &Register, regy: &Register) {
        let vx = self.register_val(regx);
        let vy = self.register_val(regy);
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
                let val = if self.version == Chip8Version::COSMAC {
                    self.register_set(regx, vy);
                    vy
                } else {
                    vx
                };
                *self.vf() = (val & 0x80) >> 7;
                self.register_set(regx, val << 1);
            }
            RegOperation::ShiftRight => {
                let val = if self.version == Chip8Version::COSMAC {
                    self.register_set(regx, vy);
                    vy
                } else {
                    vx
                };
                *self.vf() = val & 1;
                self.register_set(regx, val >> 1);
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
        self.screen.flush().unwrap();
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
            AddIndex(reg) => {
                // TODO: Might need to add overflow behaviour depnding on the game (See Amiga
                // Spaceflight 2091)
                self.index_r += self.register_val(reg) as u16;
            }
            Draw(regx, regy, row_count) => {
                self.execute_draw(regx, regy, row_count);
            }
            LoadAddr(reg) => {
                // Takes the value starting from addr I, and loads it into registers from v0 to
                // vreg
                for i in 0x0..(reg.0 + 1) {
                    let value = self.load_from_addr(self.index_r + i as u16);
                    self.register_set(&Register(i), value);
                }
                if self.version == Chip8Version::COSMAC {
                    self.index_r += reg.0 as u16 + 1;
                }
            }
            StoreAddr(reg) => {
                for i in 0x0..(reg.0 + 1) {
                    let value = self.register_val(&Register(i));
                    self.store_in_addr(self.index_r + i as u16, value);
                }
                if self.version == Chip8Version::COSMAC {
                    self.index_r += reg.0 as u16 + 1;
                }
            }
            SetFont(reg) => {
                self.index_r =
                    Self::FONT_START_ADDR + ((reg.0 & 0x0F) as u16 * Self::BYTES_PER_FONT);
            }
            JumpWithOffset(addr) => {
                let addr_to_jump = if self.version == Chip8Version::COSMAC {
                    addr.0 + self.register_val(&Register(0)) as u16
                } else {
                    // Strange quirk in newer interpreters where the addr was interpreted as XNN
                    addr.0 + self.register_val(&Register(((addr.0 >> 8) & 0xF) as u8)) as u16
                };
                self.pc_r = addr_to_jump;
                return;
            }
            CallSubroutine(addr) => {
                self.stack.push(self.pc_r);
                self.pc_r = addr.0;
                return;
            }
            Return => {
                let return_addr = self.stack.pop().expect("CRITICAL: Stack is empty");
                self.pc_r = return_addr;
                return;
            }
            Skip(skipif, reg, value) => {
                let eq = self.register_val(reg) == value.0.into();
                if *skipif == SkipIf::Eq && eq || *skipif == SkipIf::NotEq && !eq {
                    self.increment_pc();
                }
            }
            SkipReg(skipif, regx, regy) => {
                let eq = self.register_val(regx) == self.register_val(regy);
                if (*skipif == SkipIf::Eq && eq) || (*skipif == SkipIf::NotEq && !eq) {
                    self.increment_pc();
                }
            }
            SkipKeyPress(skipif, reg) => {
                let pressed = self.input.is_key_pressed(self.register_val(reg));
                if (*skipif == SkipIf::Eq && pressed) || (*skipif == SkipIf::NotEq && !pressed) {
                    self.increment_pc();
                }
            }
            // Block exec and get next key press
            GetKey(reg) => {
                // TODO: On COSMAC, should only be registered when pressed THEN released
                loop {
                    self.input.update().unwrap();
                    let key = self.input.get_pressed_key();
                    if let Some(key) = key {
                        self.register_set(reg, key);
                        break;
                    };
                    thread::sleep(Self::INPUT_POLL_RATE);
                }
            }
            Random(reg, value) => {
                let random: u8 = rand::random();
                self.register_set(reg, value.0 & random);
            }
            SetSoundTimer(_reg) => todo!(),
            SetDelayTimer(_reg) => todo!(),
            GetDelayTimer(_reg) => todo!(),
            // Takes the decimal digits of the value in reg and stores them in memory starting with
            // index
            BinaryDecimalConv(reg) => {
                let value = self.register_val(reg);
                let first_digit = value / 100;
                let second_digit = (value % 100) / 10;
                let last_digit = value % 10;
                self.store_in_addr(self.index_r, first_digit);
                self.store_in_addr(self.index_r + 1, second_digit);
                self.store_in_addr(self.index_r + 2, last_digit);
            }
            Invalid => panic!("Invalid instruction encountered"),
            ExecuteMachineLangRoutine => {}
        };
        self.increment_pc();
    }

    fn cycle(&mut self) {
        crossterm::terminal::enable_raw_mode().unwrap();
        let cycle_time = Duration::from_nanos(1_000_000_000 / Self::CPU_FREQ_HZ as u64);
        loop {
            let cycle_start = Instant::now();

            if !self.input.update().unwrap_or(false) {
                break;
            };

            let raw = self.fetch();
            let inst = self.decode(&raw);
            self.execute(&inst);

            let elapsed = cycle_start.elapsed();
            if elapsed < cycle_time {
                thread::sleep(cycle_time - elapsed);
            }
        }
        crossterm::terminal::disable_raw_mode().unwrap();
    }
}

impl Drop for Chip8 {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().unwrap();
    }
}

use clap::Parser;

#[derive(Parser)]
#[command(name = "chip8-emulator")]
#[command(about = "A CHIP-8 emulator written in Rust")]
struct Args {
    #[arg(help = "Path to the CHIP-8 ROM file")]
    rom_file: String,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Dump the HEX instructions in the ROM")]
    dump_inst: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let bytes = fs::read(args.rom_file).expect("Could not read file");
    if args.dump_inst {
        Chip8::dump_inst(&bytes);
    } else {
        let mut chip8 = Chip8::new(Chip8Version::COSMAC);
        chip8.load_rom(&bytes);
        chip8.cycle();
    }
    Ok(())
}
