use crate::primitive::*;

pub struct CPU {
    memory: [u8; Self::MEMORY_SIZE],   // This CPU also has memory lol
    pc_r: u16,                         // Program Counter
    index_r: u16,                      // Index Register
    gen_r: [u8; Self::REGISTER_COUNT], // General Purpose Registers
    stack: Vec<u16>,                   // Stack
    delay_timer: u8,                   // Delay Timer
    sound_timer: u8,                   // Sound Timer
    waiting_for_key: Option<Register>, // Track if CPU is waiting for key input
}

impl CPU {
    pub const MEMORY_SIZE: usize = 4096; // 4KB memory
    pub const REGISTER_COUNT: usize = 16; // 16 General Purpose Registers
    pub const INSTRUCTION_SIZE_B: u16 = 2; // Each instruction is 2 bytes

    pub fn new() -> Self {
        return Self {
            memory: [0; Self::MEMORY_SIZE],
            index_r: 0,
            gen_r: [0; Self::REGISTER_COUNT],
            stack: Vec::new(),
            delay_timer: 0,
            sound_timer: 0,
            pc_r: 0,
            waiting_for_key: None,
        };
    }

    // Return a reference to the value of the VF register
    pub fn vf(&mut self) -> &mut u8 {
        return &mut self.gen_r[Self::REGISTER_COUNT - 1];
    }

    // Value of CPU register
    pub fn register_val(&self, reg: &Register) -> u8 {
        return self.gen_r[reg.get() as usize];
    }

    // Set value of CPU register
    pub fn register_set(&mut self, reg: &Register, value: u8) {
        self.gen_r[reg.get() as usize] = value;
    }

    // Load value from address in memory
    pub fn load_from_addr(&self, addr: u16) -> u8 {
        return self.memory[addr as usize];
    }

    // Store value in memory at address
    pub fn store_in_addr(&mut self, addr: u16, value: u8) {
        self.memory[addr as usize] = value;
    }

    pub fn store_memory_slice(&mut self, start: usize, bytes: &[u8]) -> Result<(), ()> {
        let end = start + bytes.len();
        if end > self.memory.len() {
            return Err(());
        }
        self.memory[start..end].copy_from_slice(bytes);
        return Ok(());
    }

    // Increment the Program Counter
    pub fn increment_pc(&mut self) {
        self.pc_r += Self::INSTRUCTION_SIZE_B;
    }

    pub fn get_pc(&self) -> u16 {
        self.pc_r
    }

    pub fn jump_to(&mut self, addr: &Address) {
        self.pc_r = addr.get();
    }

    pub fn fetch_current_instruction(&self) -> RawInstruction {
        return RawInstruction::new(
            self.memory[self.pc_r as usize],
            self.memory[self.pc_r as usize + 1],
        );
    }

    pub fn get_index(&self) -> u16 {
        self.index_r
    }

    pub fn set_index(&mut self, value: u16) {
        self.index_r = value;
    }

    pub fn dec_delay(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
    }

    pub fn dec_sound(&mut self) {
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    // Timer operations
    pub fn set_delay_timer(&mut self, value: u8) {
        self.delay_timer = value;
    }

    pub fn get_delay_timer(&self) -> u8 {
        self.delay_timer
    }

    pub fn set_sound_timer(&mut self, value: u8) {
        self.sound_timer = value;
    }

    // Stack operations
    pub fn push_stack(&mut self, addr: u16) {
        self.stack.push(addr);
    }

    pub fn pop_stack(&mut self) -> Option<u16> {
        self.stack.pop()
    }

    // Register arithmetic operations
    pub fn add_reg(&mut self, reg: &Register, value: u8) {
        let current = self.register_val(reg);
        let (result, _) = current.overflowing_add(value);
        self.register_set(reg, result);
    }

    pub fn add_index(&mut self, value: u16) {
        self.index_r += value;
    }

    // Binary decimal conversion
    pub fn binary_decimal_conv(&mut self, reg: &Register) {
        let value = self.register_val(reg);
        let first_digit = value / 100;
        let second_digit = (value % 100) / 10;
        let last_digit = value % 10;
        self.store_in_addr(self.index_r, first_digit);
        self.store_in_addr(self.index_r + 1, second_digit);
        self.store_in_addr(self.index_r + 2, last_digit);
    }

    // Memory operations for register range
    pub fn load_registers(&mut self, up_to_reg: &Register) {
        for i in 0x0..=(up_to_reg.get()) {
            let value = self.load_from_addr(self.index_r + i as u16);
            let reg = Register::new(i).unwrap();
            self.register_set(&reg, value);
        }
    }

    pub fn store_registers(&mut self, up_to_reg: &Register) {
        for i in 0x0..=(up_to_reg.get()) {
            let reg = Register::new(i).unwrap();
            let value = self.register_val(&reg);
            self.store_in_addr(self.index_r + i as u16, value);
        }
    }

    // Version-dependent operations that need to modify index
    pub fn load_registers_cosmac(&mut self, up_to_reg: &Register) {
        self.load_registers(up_to_reg);
        self.index_r += up_to_reg.get() as u16 + 1;
    }

    pub fn store_registers_cosmac(&mut self, up_to_reg: &Register) {
        self.store_registers(up_to_reg);
        self.index_r += up_to_reg.get() as u16 + 1;
    }

    // Key waiting state management
    pub fn is_waiting_for_key(&self) -> bool {
        self.waiting_for_key.is_some()
    }

    pub fn start_waiting_for_key(&mut self, reg: Register) {
        self.waiting_for_key = Some(reg);
    }

    pub fn stop_waiting_for_key(&mut self) -> Option<Register> {
        self.waiting_for_key.take()
    }
}
