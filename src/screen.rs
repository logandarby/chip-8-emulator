use std::io::{stdout, Write};

use crossterm::{self, cursor::{Hide, Show}, execute, queue, terminal::{EnterAlternateScreen, LeaveAlternateScreen}};

pub struct Screen {
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

    pub fn new() -> Self {
        execute!(std::io::stdout(), EnterAlternateScreen, Hide).expect("Could not create terminal");
        Self {
            pixels: [false; Self::N_PIXELS as usize],
            debug_info: String::new(),
            cpu_debug_info: String::new(),
            current_instruction_debug: String::new(),
            step_mode_prompt: String::new(),
        }
    }

    pub fn get_pixel(&self, x: u8, y: u8) -> Option<bool> {
        if x >= Self::N_COLS.into() || y >= Self::N_ROWS.into() {
            return None;
        }
        return Some(self.pixels[Self::get_idx(x, y)]);
    }

    pub fn set_pixel(&mut self, x: u8, y: u8, value: bool) {
        if x >= Self::N_COLS.into() || y >= Self::N_ROWS.into() {
            return;
        }
        self.pixels[Self::get_idx(x, y)] = value;
    }

    pub fn clear(&mut self) {
        self.pixels.fill(false);
    }

    pub fn set_debug_info(&mut self, info: String) {
        self.debug_info = info;
    }

    pub fn clear_debug_info(&mut self) {
        self.debug_info.clear();
    }

    pub fn set_cpu_debug_info(&mut self, info: String) {
        self.cpu_debug_info = info;
    }

    pub fn set_current_instruction_debug(&mut self, info: String) {
        self.current_instruction_debug = info;
    }

    pub fn set_step_mode_prompt(&mut self, prompt: String) {
        self.step_mode_prompt = prompt;
    }

    pub fn clear_step_mode_prompt(&mut self) {
        self.step_mode_prompt.clear();
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

impl Screen {
    fn get_idx(x: u8, y: u8) -> usize {
        assert!(x < Self::N_COLS, "X screen index is out of bounds");
        assert!(y < Self::N_ROWS, "Y screen index is out of bounds");
        return y as usize * Self::N_COLS as usize + x as usize;
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
