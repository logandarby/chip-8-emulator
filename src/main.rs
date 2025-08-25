use core::fmt;
use std::{fs, io::{self, Write}, thread, time::Duration};

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
enum Instruction {
    ClearScreen,
    Jump(Address),
    SetReg(Register, Immediate8),
    AddReg(Register, Immediate8),
    SetIndex(Address),
    Draw(Register, Register, Immediate4),
}
/*
    Implementing the following first:
    00E0 (clear screen)
    1NNN (jump)
    6XNN (set register VX)
    7XNN (add value to register VX)
    ANNN (set index register I)
    DXYN (display/draw)
*/
impl Instruction {
    fn decode(raw: &RawInstruction) -> Option<Instruction> {
        let (nibble1, nibble2, nibble3, nibble4) = raw.to_nibbles();
        Some(match (nibble1, nibble2, nibble3, nibble4) {
            (0, 0, 0xE, 0) => Instruction::ClearScreen,
            (0x01, _, _, _) => Instruction::Jump(raw.nnn()),
            (0x06, _, _, _) => Instruction::SetReg(raw.x(), raw.nn()),
            (0x07, _, _, _) => Instruction::AddReg(raw.x(), raw.nn()),
            (0x0A, _, _, _) => Instruction::SetIndex(raw.nnn()),
            (0x0D, _, _, _) => Instruction::Draw(raw.x(), raw.y(), raw.n()),
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

    fn new() -> Self {
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
        if x >= Self::N_COLS.into() || y >= Self::N_ROWS.into() {return None;}
        return Some(self.pixels[Self::get_idx(x, y)]);
    }

    fn set_pixel(&mut self, x: u8, y: u8, value: bool) {
        if x >= Self::N_COLS.into() || y >= Self::N_ROWS.into() { return; }
        self.pixels[Self::get_idx(x, y)] = value;
    }

    fn clear(&mut self) {
        self.pixels.fill(false);
        self.flush();
    }
  
    // Draws to the console
    fn flush(&self) {
        // return;
         // Clear screen (ANSI escape code)
        print!("\x1B[2J\x1B[H");
        for y in 0..Self::N_ROWS {
            for x in 0..Self::N_COLS {
                let pixel = self.get_pixel(x, y).unwrap();
                print!("{}", if pixel { "██" } else { "  " });
            }
            println!();
        }
        io::stdout().flush().unwrap();
    }
}


struct Chip8 {
    screen: Screen,
    memory: [u8; Self::MEMORY_SIZE],
    pc_r: u16,    // Program Counter
    index_r: u16, // Index Register
    gen_r: [u8; Self::REGISTER_COUNT], // General Purpose Registers
                  // stack
                  // delay timer
                  // sound timer
}

impl Chip8 {
    pub const REGISTER_COUNT: usize = 16; // 16 General Purpose Registers
    pub const MEMORY_SIZE: usize = 4096; // 4KB memory
    const ENTRY_POINT: u16 = 0x200;      // Where a program is expected to start
   
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
        0xF0, 0x80, 0xF0, 0x80, 0x80  // F
    ];

    fn new() -> Self {
        Self {
            screen: Screen::new(),
            memory: [0; Self::MEMORY_SIZE],
            gen_r: [0; Self::REGISTER_COUNT],
            pc_r: Self::ENTRY_POINT,
            index_r: 0,
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

    fn register_val(&mut self, reg: &Register) -> &mut u8 {
        return &mut self.gen_r[reg.0 as usize];
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

    fn execute(&mut self, inst: &Instruction) {
        use Instruction::*;
        match inst {
            ClearScreen => self.screen.clear(),
            Jump(addr) => { self.pc_r = addr.0; return; },
            SetReg(reg, value) => *self.register_val(reg) = value.0,
            AddReg(reg, value) => *self.register_val(reg) += value.0,
            SetIndex(addr) => self.index_r = addr.0,
            // Draws sprite N pixels tall located at the index register
            // at the coordinate x, y in the regX and regY registers respectively
            // All the pixels that are "on" in the sprite will flip the screen.
            // If a pixel is turned off this way, the VF register is set to 1. Otherwise, it's set
            // to 0
            // The starting coordinate wraps, but the drawing is clipped
            Draw(regx, regy, row_count) => {
                let mut x = *self.register_val(regx) % Screen::N_COLS;
                let mut y = *self.register_val(regy) % Screen::N_ROWS;
                *self.vf() = 0;
                let index_addr = self.index_r;
                for row in 0..row_count.0 {
                    let sprite_data = self.load_from_addr(index_addr + row as u16);
                    for bit_pos in 0..8 {
                        let sprite_bit = (sprite_data >> (7 - bit_pos)) & 1;
                        let pixel = match self.screen.get_pixel(x, y) {
                            Some(x) => x,
                            None => continue,
                        };
                        if sprite_bit == 1 && pixel == true {
                            self.screen.set_pixel(x, y, false);
                            *self.vf() = 1;
                        } else if sprite_bit == 1 && pixel == false {
                            self.screen.set_pixel(x, y, true);
                        }
                        x += 1;
                    }
                    y += 1;
                }
                self.screen.flush();
            } // _ => panic!("Invalid Instruction {:#?}", inst),
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
