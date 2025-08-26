use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Instant, Duration};

use crate::cpu::*;
use crate::input::KeyEventHandler;
use crate::primitive::*;
use crate::screen::*;
use crate::decoder::*;

#[derive(Debug, PartialEq)]
pub enum Chip8Version {
    COSMAC,
    CHIP48,
    SUPERCHIP,
}

pub struct Chip8Config {
    pub version: Chip8Version,
    pub debug: bool,
    pub step_mode: bool,
}

pub struct Chip8 {
    // Config
    config: Chip8Config,
    // CPU
    cpu: CPU,
    // I/O
    screen: Screen,
    input: KeyEventHandler,
    // Debugging Utils
    paused: Arc<Mutex<bool>>, // For step mode: whether execution is paused
    step_requested: Arc<Mutex<bool>>, // For step mode: whether a single step is requested
}

impl Chip8 {
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

    pub fn new(config: Chip8Config, input_handler: KeyEventHandler) -> Self {
        let paused = Arc::new(Mutex::new(config.step_mode)); // Start paused if in step mode
        let step_requested = Arc::new(Mutex::new(false));

        let mut chip8 = Self {
            config,
            screen: Screen::new(),
            input: input_handler,
            cpu: CPU::new(),
            paused,
            step_requested,
        };

        // Set up step mode callbacks if enabled
        if chip8.config.step_mode {
            chip8.setup_step_mode_callbacks();
        }

        chip8
    }

    // Loads a program `bytes` into ROM starting at the entry point, and gets CPU ready for
    // execution
    pub fn load_rom(&mut self, bytes: &Vec<u8>) -> Result<(), ()> {
        // Load Fonts into memory
        self.cpu
            .store_memory_slice(Self::FONT_START_ADDR as usize, &Self::FONT)
            .expect("Fonts should fit into memory");
        // Load ROM into memory
        self.cpu.store_memory_slice(Self::ENTRY_POINT.into(), bytes)?;
        self.cpu.jump_to(&Address::new(Self::ENTRY_POINT).unwrap());
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

    fn fetch(&self) -> RawInstruction {
        self.cpu.fetch_current_instruction()
    }

    fn decode(&self, raw: &RawInstruction) -> Instruction {
        return Decoder::decode(raw).expect(
            format!(
                "Could not decode instruction {raw} at PC location {}",
                self.cpu.get_pc()
            )
            .as_str(),
        );
    }

    fn execute_reg_op(&mut self, reg_op: &RegOperation, regx: &Register, regy: &Register) {
        let vx = self.cpu.register_val(regx);
        let vy = self.cpu.register_val(regy);
        match *reg_op {
            RegOperation::Set => {
                self.cpu.register_set(regx, vy);
            }
            RegOperation::OR => {
                self.cpu.register_set(regx, vx | vy);
            }
            RegOperation::XOR => {
                self.cpu.register_set(regx, vx ^ vy);
            }
            RegOperation::AND => {
                self.cpu.register_set(regx, vx & vy);
            }
            RegOperation::Add => {
                let (result, overflow) = vx.overflowing_add(vy);
                self.cpu.register_set(regx, result);
                *self.cpu.vf() = overflow as u8;
            }
            RegOperation::Sub => {
                let (result, _) = vx.overflowing_sub(vy);
                self.cpu.register_set(regx, result);
                *self.cpu.vf() = if vx > vy { 1 } else { 0 };
            }
            RegOperation::SubInv => {
                let (result, _) = vy.overflowing_sub(vx);
                self.cpu.register_set(regx, result);
                *self.cpu.vf() = if vy > vx { 1 } else { 0 };
            }
            RegOperation::ShiftLeft => {
                let val = if self.config.version == Chip8Version::COSMAC {
                    self.cpu.register_set(regx, vy);
                    vy
                } else {
                    vx
                };
                *self.cpu.vf() = (val & 0x80) >> 7;
                self.cpu.register_set(regx, val << 1);
            }
            RegOperation::ShiftRight => {
                let val = if self.config.version == Chip8Version::COSMAC {
                    self.cpu.register_set(regx, vy);
                    vy
                } else {
                    vx
                };
                *self.cpu.vf() = val & 1;
                self.cpu.register_set(regx, val >> 1);
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
        let start_x = self.cpu.register_val(regx) % Screen::N_COLS;
        let start_y = self.cpu.register_val(regy) % Screen::N_ROWS;
        *self.cpu.vf() = 0;
        let index_addr = self.cpu.get_index();

        for row in 0..row_count.get() {
            let y = start_y + row;
            if y >= Screen::N_ROWS {
                break;
            }

            let sprite_data = self.cpu.load_from_addr(index_addr + row as u16);

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
                        *self.cpu.vf() = 1;
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
            Some(self.cpu.get_debug_str())
        } else {
            None
        }
    }

    fn execute(&mut self, inst: &Instruction) {
        use Instruction::*;

        match inst {
            ClearScreen => self.screen.clear(),
            Jump(addr) => {
                self.cpu.jump_to(addr);
                return;
            }
            RegOp(reg_op, regx, regy) => self.execute_reg_op(reg_op, regx, regy),
            SetRegImmediate(reg, value) => self.cpu.register_set(reg, value.get()),
            AddRegImmediate(reg, value) => self.cpu.add_reg(reg, value.get()),
            SetIndex(addr) => self.cpu.set_index(addr.get()),
            AddIndex(reg) => {
                let reg_val = self.cpu.register_val(reg) as u16;
                self.cpu.add_index(reg_val);
            }
            Draw(regx, regy, row_count) => {
                self.execute_draw(regx, regy, row_count);
            }
            LoadAddr(reg) => {
                if self.config.version == Chip8Version::COSMAC {
                    self.cpu.load_registers_cosmac(reg);
                } else {
                    self.cpu.load_registers(reg);
                }
            }
            StoreAddr(reg) => {
                if self.config.version == Chip8Version::COSMAC {
                    self.cpu.store_registers_cosmac(reg);
                } else {
                    self.cpu.store_registers(reg);
                }
            }
            SetFont(reg) => {
                let font_addr = Self::FONT_START_ADDR
                    + ((self.cpu.register_val(reg) & 0x0F) as u16 * Self::BYTES_PER_FONT);
                self.cpu.set_index(font_addr);
            }
            JumpWithOffset(addr) => {
                let addr_to_jump = if self.config.version == Chip8Version::COSMAC {
                    addr.get() + self.cpu.register_val(&Register::new(0).unwrap()) as u16
                } else {
                    // Strange quirk in newer interpreters where the addr was interpreted as XNN
                    let reg_index = ((addr.get() >> 8) & 0xF) as u8;
                    addr.get() + self.cpu.register_val(&Register::new(reg_index).unwrap()) as u16
                };
                let jump_addr = Address::new(addr_to_jump).unwrap();
                self.cpu.jump_to(&jump_addr);
                return;
            }
            CallSubroutine(addr) => {
                self.cpu.push_stack(self.cpu.get_pc());
                self.cpu.jump_to(addr);
                return;
            }
            Return => {
                let return_addr = self.cpu.pop_stack().expect("CRITICAL: Stack is empty");
                let addr = Address::new(return_addr).unwrap();
                self.cpu.jump_to(&addr);
            }
            Skip(skipif, reg, value) => {
                let eq = self.cpu.register_val(reg) == value.get();
                if (*skipif == SkipIf::Eq && eq) || (*skipif == SkipIf::NotEq && !eq) {
                    self.cpu.increment_pc();
                }
            }
            SkipReg(skipif, regx, regy) => {
                let eq = self.cpu.register_val(regx) == self.cpu.register_val(regy);
                if (*skipif == SkipIf::Eq && eq) || (*skipif == SkipIf::NotEq && !eq) {
                    self.cpu.increment_pc();
                }
            }
            SkipKeyPress(skipif, reg) => {
                let pressed = self.input.is_key_pressed(self.cpu.register_val(reg));
                if (*skipif == SkipIf::Eq && pressed) || (*skipif == SkipIf::NotEq && !pressed) {
                    self.cpu.increment_pc();
                }
            }
            GetKey(reg) => {
                // TODO: On COSMAC, should only be registered when pressed THEN released
                loop {
                    self.input.update().unwrap();
                    let key = self.input.get_pressed_key();
                    if let Some(key) = key {
                        self.cpu.register_set(reg, key);
                        break;
                    };
                    thread::sleep(Self::INPUT_POLL_RATE);
                }
            }
            Random(reg, value) => {
                let random: u8 = rand::random();
                self.cpu.register_set(reg, value.get() & random);
            }
            SetSoundTimer(reg) => self.cpu.set_sound_timer(self.cpu.register_val(reg)),
            SetDelayTimer(reg) => self.cpu.set_delay_timer(self.cpu.register_val(reg)),
            GetDelayTimer(reg) => self.cpu.register_set(reg, self.cpu.get_delay_timer()),
            BinaryDecimalConv(reg) => self.cpu.binary_decimal_conv(reg),
            Invalid => panic!("Invalid instruction encountered"),
            ExecuteMachineLangRoutine => {}
        };
        self.cpu.increment_pc();
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
        let addr = Address::new(self.cpu.get_pc()).unwrap();
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
            let addr = Address::new(self.cpu.get_pc()).unwrap();
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

    pub fn cycle(&mut self) {
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

            // TODO: Refactor to be independent of cycles
            self.cpu.dec_delay();
            self.cpu.dec_sound();

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
