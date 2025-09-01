use crate::cpu::*;
use crate::decoder::*;
use crate::hardware::Hardware;
use crate::hardware::HardwareExecutionConfig;
use crate::input::KeyEventHandler;
use crate::primitive::*;
use crate::scheduler::*;

#[derive(Clone, Debug, PartialEq, clap::ValueEnum)]
pub enum Chip8Version {
    COSMAC,
    CHIP48,
    SUPERCHIP,
}

impl std::fmt::Display for Chip8Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Chip8Version::*;
        write!(
            f,
            "{}",
            match self {
                COSMAC => "cosmac",
                CHIP48 => "chip48",
                SUPERCHIP => "superchip",
            }
        )
    }
}

#[derive(Clone)]
pub struct Chip8Config {
    pub version: Chip8Version,
    pub debug: bool,
}

pub struct Chip8 {
    // Config
    pub config: Chip8Config,
    // CPU & Screen
    pub hardware: Hardware,
    // Input,
    pub input: KeyEventHandler,
}

impl Chip8 {
    pub const ENTRY_POINT: u16 = 0x200; // Where a program is expected to start
    pub const CPU_FREQ_HZ: f64 = 500.0;
    pub const TIMER_HZ: f64 = 60.0;
    pub const SCREEN_HZ: f64 = 60.0;
    pub const INPUT_POLL_RATE_MS: u64 = 10;

    // Default font loaded into memory before the application
    pub const FONT_START_ADDR: u16 = 0x50;
    pub const FONT: [u8; 80] = [
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
    pub const BYTES_PER_FONT: u16 = 5;

    pub fn new(config: Chip8Config, input_handler: KeyEventHandler) -> Self {
        Self {
            config: config.clone(),
            hardware: Hardware::new(HardwareExecutionConfig {
                version: config.version,
            }),
            input: input_handler,
        }
    }

    // Loads a program `bytes` into ROM starting at the entry point, and gets CPU ready for
    // execution
    pub fn load_rom(&mut self, bytes: &Vec<u8>) -> Result<(), ()> {
        // Load Fonts into memory
        self.hardware
            .cpu
            .store_memory_slice(Self::FONT_START_ADDR as usize, &Self::FONT)
            .expect("Fonts should fit into memory");
        // Load ROM into memory
        self.hardware
            .cpu
            .store_memory_slice(Self::ENTRY_POINT.into(), bytes)?;
        self.hardware
            .cpu
            .jump_to(&Address::new(Self::ENTRY_POINT).unwrap());
        Ok(())
    }

    // Dumps the instructions contained in the bytes to stdio in a readible format
    pub fn dump_inst(bytes: &Vec<u8>) {
        println!("Dumping instruction hex codes:");
        bytes
            .chunks_exact(CPU::INSTRUCTION_SIZE_B.into())
            .map(|chunk| RawInstruction::new(chunk[0], chunk[1]))
            .enumerate()
            .for_each(|(index, raw)| {
                let inst = Decoder::decode(&raw);
                let addr = Address::new(Self::ENTRY_POINT + index as u16 * 2).unwrap();
                println!(
                    "{}: Code {}, {}",
                    addr,
                    raw,
                    inst.unwrap_or(Instruction::Invalid)
                );
            });
    }

    pub async fn cycle(&mut self) {
        crossterm::terminal::enable_raw_mode().unwrap();
        Chip8Orchaestrator::run(self).await;
        crossterm::terminal::disable_raw_mode().unwrap();
    }
}

impl Drop for Chip8 {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().unwrap();
    }
}
