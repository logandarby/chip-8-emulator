use std::io::{Write, stdout};

use crossterm::{
    self,
    cursor::{Hide, Show},
    execute, queue,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::{
    input::Chip8KeyState,
    primitive::{Instruction, RawInstruction},
    scheduler::PlaybackMode,
};

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub current_pc: u16,
    pub raw_instruction: RawInstruction,
    pub decoded_instruction: Instruction,
    pub index_register: u16,
    pub delay_timer: u8,
    pub sound_timer: u8,
    pub registers: [u8; 16],
    pub key_state: Chip8KeyState,
    pub playback_mode: PlaybackMode,
}

pub struct Screen {
    pixels: [bool; Self::N_PIXELS as usize],
    debug_info: Option<DebugInfo>,
}

impl Screen {
    pub const N_ROWS: u8 = 32;
    pub const N_COLS: u8 = 64;
    pub const N_PIXELS: u16 = Self::N_ROWS as u16 * Self::N_COLS as u16;

    pub fn new() -> Self {
        execute!(std::io::stdout(), EnterAlternateScreen, Hide).expect("Could not create terminal");
        Self {
            pixels: [false; Self::N_PIXELS as usize],
            debug_info: None,
        }
    }

    pub fn get_pixel(&self, x: u8, y: u8) -> Option<bool> {
        if x >= Self::N_COLS || y >= Self::N_ROWS {
            None
        } else {
            Some(self.pixels[Self::get_idx(x, y)])
        }
    }

    pub fn set_pixel(&mut self, x: u8, y: u8, value: bool) {
        if x >= Self::N_COLS || y >= Self::N_ROWS {
            return;
        }
        self.pixels[Self::get_idx(x, y)] = value;
    }

    pub fn clear(&mut self) {
        self.pixels.fill(false);
    }

    pub fn set_debug_info(&mut self, debug_info: DebugInfo) {
        self.debug_info = Some(debug_info);
    }

    // Draws to the console
    pub fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{cursor::*, queue, style::*};
        use std::io::stdout;
        let (term_width, term_height) = crossterm::terminal::size()?;

        // Calculate centering offset
        let display_width = (Screen::N_COLS * 2) as u16;
        let display_height = Screen::N_ROWS as u16;
        let offset_x = (term_width.saturating_sub(display_width)) / 2;

        // Check if we have any debug info to display
        let has_debug_info = self.debug_info.is_some();

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
                Print("Press 'Escape' to quit, Press 'P' to restart")
            )?;
        }

        // Add debug info right after the display (no title when debugging)
        if let Some(ref debug) = self.debug_info {
            self.render_debug_info(debug, offset_x, offset_y + display_height + 1)?;
        }

        stdout().flush()?;
        Ok(())
    }

    fn render_debug_info(
        &self,
        debug: &DebugInfo,
        offset_x: u16,
        start_y: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::style::*;

        let mut debug_line = start_y;

        // Render key state
        self.render_debug_line(
            &self.format_key_state(debug),
            Color::Yellow,
            "INPUT",
            offset_x,
            debug_line,
        )?;
        debug_line += 1;

        // Render CPU state
        self.render_debug_line(
            &self.format_cpu_state(debug),
            Color::Cyan,
            "CPU",
            offset_x,
            debug_line,
        )?;
        debug_line += 1;

        // Render current instruction
        self.render_debug_line(
            &self.format_instruction(debug),
            Color::Magenta,
            "INST",
            offset_x,
            debug_line,
        )?;
        debug_line += 1;

        // Render playback mode
        self.render_debug_line(
            &self.format_playback_mode(debug),
            Color::Green,
            "Mode",
            offset_x,
            debug_line,
        )?;

        Ok(())
    }

    fn render_debug_line(
        &self,
        content: &str,
        color: crossterm::style::Color,
        prefix: &str,
        offset_x: u16,
        y: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{cursor::*, queue, style::*};
        use std::io::stdout;

        queue!(
            stdout(),
            MoveTo(offset_x, y),
            SetForegroundColor(color),
            Print(format!("{}: {}", prefix, content)),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine),
            ResetColor
        )?;
        Ok(())
    }

    fn format_key_state(&self, debug: &DebugInfo) -> String {
        debug.key_state.format_pressed_keys()
    }

    fn format_cpu_state(&self, debug: &DebugInfo) -> String {
        format!(
            "I: 0x{:03X} | DT: {} | ST: {} | V0-F: [{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X}]",
            debug.index_register,
            debug.delay_timer,
            debug.sound_timer,
            debug.registers[0],
            debug.registers[1],
            debug.registers[2],
            debug.registers[3],
            debug.registers[4],
            debug.registers[5],
            debug.registers[6],
            debug.registers[7],
            debug.registers[8],
            debug.registers[9],
            debug.registers[10],
            debug.registers[11],
            debug.registers[12],
            debug.registers[13],
            debug.registers[14],
            debug.registers[15]
        )
    }

    fn format_instruction(&self, debug: &DebugInfo) -> String {
        format!(
            "PC: 0x{:03X} | Raw: {} | {}",
            debug.current_pc, debug.raw_instruction, debug.decoded_instruction
        )
    }

    fn format_playback_mode(&self, debug: &DebugInfo) -> String {
        match debug.playback_mode {
            PlaybackMode::Running => "Running",
            PlaybackMode::Paused => "Paused",
            PlaybackMode::Stepping => "Stepping",
        }
        .to_string()
    }
}

impl Screen {
    fn get_idx(x: u8, y: u8) -> usize {
        assert!(x < Self::N_COLS, "X screen index is out of bounds");
        assert!(y < Self::N_ROWS, "Y screen index is out of bounds");
        y as usize * Self::N_COLS as usize + x as usize
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        crossterm::queue!(
            std::io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )
        .unwrap();
        stdout().flush().unwrap();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);
    }
}
