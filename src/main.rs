use core::fmt;
use crossterm::{self, cursor::Show, execute, terminal::LeaveAlternateScreen};
use std::{
    fmt::{Debug, Display},
    fs,
    io::{self, Write},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

mod input;

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
    debug_info: String,
    cpu_debug_info: String,
    current_instruction_debug: String,
    step_mode_prompt: String,
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
            debug_info: String::new(),
            cpu_debug_info: String::new(),
            current_instruction_debug: String::new(),
            step_mode_prompt: String::new(),
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

    fn set_debug_info(&mut self, info: String) {
        self.debug_info = info;
    }

    fn clear_debug_info(&mut self) {
        self.debug_info.clear();
    }

    fn set_cpu_debug_info(&mut self, info: String) {
        self.cpu_debug_info = info;
    }

    fn set_current_instruction_debug(&mut self, info: String) {
        self.current_instruction_debug = info;
    }

    fn set_step_mode_prompt(&mut self, prompt: String) {
        self.step_mode_prompt = prompt;
    }

    fn clear_step_mode_prompt(&mut self) {
        self.step_mode_prompt.clear();
    }

    // Draws to the console
    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{cursor::*, queue, style::*};
        use std::io::stdout;
        let (term_width, term_height) = crossterm::terminal::size()?;

        // Calculate centering offset
        let display_width = (Screen::N_COLS * 2) as u16;
        let display_height = Screen::N_ROWS as u16;
        let offset_x = (term_width.saturating_sub(display_width)) / 2;

        // Check if we have any debug info to display
        let has_debug_info = !self.debug_info.is_empty()
            || !self.cpu_debug_info.is_empty()
            || !self.current_instruction_debug.is_empty()
            || !self.step_mode_prompt.is_empty();

        // Reserve space at bottom
        let bottom_reserve = if has_debug_info {
            6 // Up to 4 debug lines + some padding (no title/escape when debugging)
        } else {
            4 // Just title + escape + padding
        };

        let available_height = term_height.saturating_sub(bottom_reserve);
        let offset_y = if available_height < display_height {
            1 // If terminal is too small, start near top
        } else {
            available_height.saturating_sub(display_height) / 2
        };

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

        // Add title (only when not in debug or step mode to save space)
        if !has_debug_info {
            queue!(
                stdout(),
                MoveTo(offset_x, offset_y.saturating_sub(2)),
                Print("CHIP-8 Emulator"),
                MoveTo(offset_x, offset_y + display_height + 1),
                Print("Press 'Escape' to quit")
            )?;
        }

        // Add debug info right after the display (no title when debugging)
        let mut debug_line = offset_y + display_height + 1;
        if !self.debug_info.is_empty() {
            queue!(
                stdout(),
                MoveTo(offset_x, debug_line),
                SetForegroundColor(Color::Yellow),
                Print(format!("INPUT: {}", self.debug_info)),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine),
                ResetColor
            )?;
            debug_line += 1;
        }

        if !self.cpu_debug_info.is_empty() {
            queue!(
                stdout(),
                MoveTo(offset_x, debug_line),
                SetForegroundColor(Color::Cyan),
                Print(format!("CPU: {}", self.cpu_debug_info)),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine),
                ResetColor
            )?;
            debug_line += 1;
        }

        if !self.current_instruction_debug.is_empty() {
            queue!(
                stdout(),
                MoveTo(offset_x, debug_line),
                SetForegroundColor(Color::Magenta),
                Print(format!("INST: {}", self.current_instruction_debug)),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine),
                ResetColor
            )?;
            debug_line += 1;
        }

        if !self.step_mode_prompt.is_empty() {
            queue!(
                stdout(),
                MoveTo(offset_x, debug_line),
                SetForegroundColor(Color::Green),
                Print(format!("{}", self.step_mode_prompt)),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine),
                ResetColor
            )?;
        }

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

// Old InputHandler removed - using new input::KeyEventHandler instead

#[derive(Debug, PartialEq)]
enum Chip8Version {
    COSMAC,
    CHIP48,
    SUPERCHIP,
}

struct Chip8Config {
    debug: bool,
    step_mode: bool,
}

struct Chip8 {
    version: Chip8Version,
    config: Chip8Config,
    screen: Screen,
    input: input::KeyEventHandler,
    memory: [u8; Self::MEMORY_SIZE],
    pc_r: u16,                         // Program Counter
    index_r: u16,                      // Index Register
    gen_r: [u8; Self::REGISTER_COUNT], // General Purpose Registers
    stack: Vec<u16>,                   // Stack
    paused: Arc<Mutex<bool>>,          // For step mode: whether execution is paused
    step_requested: Arc<Mutex<bool>>,  // For step mode: whether a single step is requested
                                       // delay timer
                                       // sound timer
}

impl Chip8 {
    pub const REGISTER_COUNT: usize = 16; // 16 General Purpose Registers
    pub const MEMORY_SIZE: usize = 4096; // 4KB memory
    const ENTRY_POINT: u16 = 0x200; // Where a program is expected to start
    const CPU_FREQ_HZ: u16 = 500;
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

    fn new(
        version: Chip8Version,
        config: Chip8Config,
        input_handler: input::KeyEventHandler,
    ) -> Self {
        let paused = Arc::new(Mutex::new(config.step_mode)); // Start paused if in step mode
        let step_requested = Arc::new(Mutex::new(false));

        let mut chip8 = Self {
            version: version,
            config,
            screen: Screen::new(),
            input: input_handler,
            memory: [0; Self::MEMORY_SIZE],
            gen_r: [0; Self::REGISTER_COUNT],
            pc_r: Self::ENTRY_POINT,
            index_r: 0,
            stack: Vec::new(),
            paused,
            step_requested,
        };

        // Set up step mode callbacks if enabled
        if chip8.config.step_mode {
            chip8.setup_step_mode_callbacks();
        }

        chip8
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

    fn get_cpu_debug_info(&self) -> Option<String> {
        if self.config.debug {
            // Get registers in hex
            let reg_str = (0..Self::REGISTER_COUNT)
                .map(|i| format!("V{:X}={:02X}", i, self.gen_r[i]))
                .collect::<Vec<_>>()
                .join(" ");

            Some(format!(
                "PC:{:04X} I:{:04X} Regs:[{}]",
                self.pc_r, self.index_r, reg_str
            ))
        } else {
            None
        }
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
            AddRegImmediate(reg, value) => {
                let reg_val = self.register_val(reg);
                let (new_val, _) = reg_val.overflowing_add(value.0);
                self.register_set(reg, new_val);
            }
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
            SetSoundTimer(_reg) => { /* TODO */ }
            SetDelayTimer(_reg) => { /* TODO */ }
            GetDelayTimer(_reg) => { /* TODO */ }
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

    fn setup_step_mode_callbacks(&mut self) {
        use crossterm::event::KeyCode;

        // Set up space key callback for play/pause toggle
        let paused_clone = Arc::clone(&self.paused);
        self.input.register_special_key_callback(
            KeyCode::Char(' '),
            Box::new(move || {
                if let Ok(mut paused) = paused_clone.lock() {
                    *paused = !*paused;
                }
            }),
        );

        // Set up enter key callback for single step
        let paused_clone2 = Arc::clone(&self.paused);
        let step_requested_clone = Arc::clone(&self.step_requested);
        self.input.register_special_key_callback(
            KeyCode::Enter,
            Box::new(move || {
                // Single step only works when paused
                if let (Ok(paused), Ok(mut step_requested)) =
                    (paused_clone2.lock(), step_requested_clone.lock())
                {
                    if *paused {
                        *step_requested = true;
                    }
                }
            }),
        );
    }

    fn is_paused(&self) -> bool {
        *self.paused.lock().unwrap_or_else(|_| std::process::exit(1))
    }

    fn take_step_request(&self) -> bool {
        if let Ok(mut step_requested) = self.step_requested.lock() {
            if *step_requested {
                *step_requested = false;
                return true;
            }
        }
        false
    }

    fn update_step_mode_prompt(&mut self) {
        if self.config.step_mode {
            let controls = if self.is_paused() {
                "[PAUSED] SPACE=Play, ENTER=Step, ESC=Quit"
            } else {
                "[PLAYING] SPACE=Pause, ESC=Quit"
            };

            let step_prompt = format!("STEP MODE: {}", controls);
            self.screen.set_step_mode_prompt(step_prompt);
        } else {
            self.screen.clear_step_mode_prompt();
        }
    }

    fn initialize_cycle(&mut self) -> (Duration, Instant) {
        crossterm::terminal::enable_raw_mode().unwrap();
        let cycle_time = Duration::from_nanos(1_000_000_000 / Self::CPU_FREQ_HZ as u64);
        let debug_clear_timer = Instant::now();
        (cycle_time, debug_clear_timer)
    }

    fn handle_input_and_debug(&mut self, debug_clear_timer: &mut Instant) -> bool {
        // Update input and check for quit
        if !self.input.update().unwrap_or(false) {
            return false; // Signal to quit
        }

        // Update debug info from input handler
        if let Some(debug_info) = self.input.get_debug_info() {
            self.screen.set_debug_info(debug_info);
            *debug_clear_timer = Instant::now();
        }

        // Clear debug info after 2 seconds of no new input
        if debug_clear_timer.elapsed() > Duration::from_secs(2) {
            self.screen.clear_debug_info();
            self.input.clear_last_key();
        }

        true // Continue running
    }

    fn should_execute_instruction(&mut self) -> bool {
        let should_execute = if self.config.step_mode {
            if self.is_paused() {
                // Check if a step was requested
                self.take_step_request()
            } else {
                // Not paused, execute normally
                true
            }
        } else {
            // Not in step mode, execute normally
            true
        };

        if !should_execute {
            // In step mode and paused, just update display and wait
            self.update_display_when_paused();
            return false;
        }

        true
    }

    fn update_display_when_paused(&mut self) {
        let raw = self.fetch();
        let inst = self.decode(&raw);

        // Set CPU debug info if debug mode is enabled
        if let Some(cpu_debug) = self.get_cpu_debug_info() {
            self.screen.set_cpu_debug_info(cpu_debug);
        }

        // Set current instruction debug info
        let addr = Address(self.pc_r);
        let inst_debug = format!("{}: Code {}, {}", addr, raw, inst);
        self.screen.set_current_instruction_debug(inst_debug);

        // Update prompt to show current state
        self.update_step_mode_prompt();

        // Flush screen
        let _ = self.screen.flush();

        // Short sleep to avoid busy waiting
        thread::sleep(Duration::from_millis(50));
    }

    fn execute_instruction_cycle(&mut self) {
        let raw = self.fetch();
        let inst = self.decode(&raw);

        // Set CPU debug info if debug mode is enabled
        if let Some(cpu_debug) = self.get_cpu_debug_info() {
            self.screen.set_cpu_debug_info(cpu_debug);
        }

        // Set current instruction debug info if debug mode or step mode is enabled
        if self.config.debug || self.config.step_mode {
            let addr = Address(self.pc_r);
            let inst_debug = format!("{}: Code {}, {}", addr, raw, inst);
            self.screen.set_current_instruction_debug(inst_debug);
            // Update step mode prompt if in step mode
            if self.config.step_mode {
                self.update_step_mode_prompt();
            }
        }
        self.execute(&inst);
        // Flush screen to display debug info if in debug mode or step mode
        if self.config.debug || self.config.step_mode {
            let _ = self.screen.flush();
        }
    }

    fn handle_cycle_timing(&self, cycle_start: Instant, cycle_time: Duration) {
        if !self.config.step_mode || !self.is_paused() {
            let elapsed = cycle_start.elapsed();
            if elapsed < cycle_time {
                thread::sleep(cycle_time - elapsed);
            }
        }
    }

    fn cycle(&mut self) {
        let (cycle_time, mut debug_clear_timer) = self.initialize_cycle();
        loop {
            let cycle_start = Instant::now();
            if !self.handle_input_and_debug(&mut debug_clear_timer) {
                break;
            }
            // Handle step mode logic
            if !self.should_execute_instruction() {
                continue;
            }
            self.execute_instruction_cycle();
            self.handle_cycle_timing(cycle_start, cycle_time);
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

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Enable debug mode showing CPU state each cycle")]
    debug: bool,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Enable step mode - pause after each instruction (requires space/enter to continue)")]
    step: bool,

    #[arg(
        long,
        default_value = "qwerty",
        help = "Keyboard layout: qwerty, natural, or sequential"
    )]
    layout: String,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    println!("Reading file {}", args.rom_file);
    let bytes = fs::read(args.rom_file).expect("Could not read file");

    if args.dump_inst {
        Chip8::dump_inst(&bytes);
    } else {
        // Parse keyboard layout
        let layout = match args.layout.to_lowercase().as_str() {
            "qwerty" => input::KeyboardLayout::Qwerty,
            "natural" => input::KeyboardLayout::Natural,
            "sequential" => input::KeyboardLayout::Sequential,
            _ => {
                eprintln!("Invalid layout '{}'. Using qwerty.", args.layout);
                input::KeyboardLayout::Qwerty
            }
        };

        // Create input configuration
        let input_config = input::InputConfig {
            layout,
            enable_debug: args.debug,
            ..Default::default()
        };

        // Create input handler
        let input_handler = input::KeyEventHandler::new(input_config);

        // Print layout info
        if args.debug {
            println!(
                "Using keyboard layout:\n{}",
                input_handler.get_layout_description()
            );
        }

        // Create emulator
        let config = Chip8Config {
            debug: args.debug,
            step_mode: args.step,
        };
        let mut chip8 = Chip8::new(Chip8Version::COSMAC, config, input_handler);
        chip8.load_rom(&bytes);
        chip8.cycle();
    }

    Ok(())
}
