use crate::chip8::{Chip8, Chip8Version};
use crate::cpu::CPU;
use crate::input::{Chip8KeyEventKind, Chip8KeyState};
use crate::primitive::*;
use crate::scheduler::PlaybackMode;
use crate::screen::{DebugInfo, Screen, ScreenColor};

#[derive(Debug, Clone)]
pub struct HardwareExecutionConfig {
    pub version: Chip8Version,
    pub screen_color: ScreenColor,
}

// Manages the internal state of the CPU and the Screen
pub struct Hardware<'a> {
    pub cpu: CPU,
    pub screen: Screen,
    key_state: Chip8KeyState,
    config: HardwareExecutionConfig,
    playback_state: PlaybackMode,
    playback_receiver: Option<tokio::sync::mpsc::Receiver<PlaybackMode>>,
    rom_ref: Option<&'a [u8]>,
}

impl<'a> Hardware<'a> {
    pub fn new(config: HardwareExecutionConfig) -> Self {
        Self {
            cpu: CPU::new(),
            screen: Screen::new(config.screen_color),
            key_state: Chip8KeyState::default(),
            config,
            playback_state: PlaybackMode::Running,
            playback_receiver: None,
            rom_ref: None,
        }
    }

    pub fn set_playback_receiver(&mut self, receiver: tokio::sync::mpsc::Receiver<PlaybackMode>) {
        self.playback_receiver = Some(receiver);
    }

    pub fn set_key_state(&mut self, key_state: &Chip8KeyState) {
        self.key_state = *key_state;
    }

    pub fn handle_key_when_waiting(&mut self, key: u8, kind: Chip8KeyEventKind) -> bool {
        if let Some(reg) = self.cpu.stop_waiting_for_key() {
            let expected_kind = if self.config.version == Chip8Version::Cosmac {
                Chip8KeyEventKind::Release
            } else {
                Chip8KeyEventKind::Press
            };
            if kind == expected_kind {
                self.cpu.register_set(&reg, key);
                self.cpu.increment_pc(); // Now we can move to the next instruction
                return true;
            } else {
                // Wrong event type, continue waiting
                self.cpu.start_waiting_for_key(reg);
            }
        }
        false
    }

    pub fn is_waiting_for_key(&self) -> bool {
        self.cpu.is_waiting_for_key()
    }

    pub fn load_rom(&mut self, bytes: &'a [u8]) -> Result<(), ()> {
        // Load Fonts into memory
        self.cpu
            .store_memory_slice(Chip8::FONT_START_ADDR as usize, &Chip8::FONT)
            .expect("Fonts should fit into memory");
        // Load ROM into memory
        self.cpu
            .store_memory_slice(Chip8::ENTRY_POINT.into(), bytes)?;
        self.cpu.jump_to(&Address::new(Chip8::ENTRY_POINT).unwrap());
        self.rom_ref = Some(bytes);
        Ok(())
    }

    pub fn restart_rom(&mut self) {
        self.cpu.reset();
        self.screen.clear();
        self.screen.flush().unwrap();
        if let Some(rom_ref) = self.rom_ref {
            let _ = self.load_rom(rom_ref);
        }
    }

    pub async fn execute_instruction(&mut self, inst: &Instruction) {
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
                if self.config.version == Chip8Version::Cosmac {
                    self.cpu.load_registers_cosmac(reg);
                } else {
                    self.cpu.load_registers(reg);
                }
            }
            StoreAddr(reg) => {
                if self.config.version == Chip8Version::Cosmac {
                    self.cpu.store_registers_cosmac(reg);
                } else {
                    self.cpu.store_registers(reg);
                }
            }
            SetFont(reg) => {
                let font_addr = Chip8::FONT_START_ADDR
                    + ((self.cpu.register_val(reg) & 0x0F) as u16 * Chip8::BYTES_PER_FONT);
                self.cpu.set_index(font_addr);
            }
            JumpWithOffset(addr) => {
                let addr_to_jump = if self.config.version == Chip8Version::Cosmac {
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
                let pressed = self.key_state.is_key_pressed(self.cpu.register_val(reg));
                if (*skipif == SkipIf::Eq && pressed) || (*skipif == SkipIf::NotEq && !pressed) {
                    self.cpu.increment_pc();
                }
            }
            GetKey(reg) => {
                // Set CPU to waiting state and don't increment PC
                self.cpu.start_waiting_for_key(*reg);
                return;
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

    fn execute_reg_op(&mut self, reg_op: &RegOperation, regx: &Register, regy: &Register) {
        let vx = self.cpu.register_val(regx);
        let vy = self.cpu.register_val(regy);
        match *reg_op {
            RegOperation::Set => {
                self.cpu.register_set(regx, vy);
            }
            RegOperation::Or => {
                self.cpu.register_set(regx, vx | vy);
            }
            RegOperation::Xor => {
                self.cpu.register_set(regx, vx ^ vy);
            }
            RegOperation::And => {
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
                let val = if self.config.version == Chip8Version::Cosmac {
                    self.cpu.register_set(regx, vy);
                    vy
                } else {
                    vx
                };
                *self.cpu.vf() = (val & 0x80) >> 7;
                self.cpu.register_set(regx, val << 1);
            }
            RegOperation::ShiftRight => {
                let val = if self.config.version == Chip8Version::Cosmac {
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
    }

    pub fn update_debug_info(&mut self) {
        // Check for playback state updates
        if let Some(ref mut receiver) = self.playback_receiver {
            while let Ok(state) = receiver.try_recv() {
                self.playback_state = state;
            }
        }

        let debug_info = self.get_debug_info();
        self.screen.set_debug_info(debug_info);
    }

    pub fn get_debug_info(&self) -> DebugInfo {
        let raw_inst = self.cpu.fetch_current_instruction();
        let decoded_inst = crate::decoder::Decoder::decode(&raw_inst)
            .unwrap_or(crate::primitive::Instruction::Invalid);

        let registers = self.cpu.all_register_val();

        DebugInfo {
            current_pc: self.cpu.get_pc(),
            raw_instruction: raw_inst,
            decoded_instruction: decoded_inst,
            index_register: self.cpu.get_index(),
            delay_timer: self.cpu.get_delay_timer(),
            sound_timer: self.cpu.get_sound_timer(),
            registers,
            key_state: self.key_state,
            playback_mode: self.playback_state.clone(),
        }
    }
}
