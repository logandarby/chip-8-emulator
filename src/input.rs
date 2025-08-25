use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Keyboard layout options for CHIP-8 input mapping
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyboardLayout {
    /// Maps number keys 1-9,0,A-F to CHIP-8 keys 1-9,0,A-F
    /// 1 2 3 4    =>    1 2 3 C
    /// Q W E R    =>    4 5 6 D  
    /// A S D F    =>    7 8 9 E
    /// Z X C V    =>    A 0 B F
    Qwerty,
    
    /// Maps keys in a more natural hex layout
    /// 1 2 3 4    =>    1 2 3 C
    /// Q W E R    =>    4 5 6 D
    /// A S D F    =>    7 8 9 E  
    /// Z X C V    =>    A 0 B F
    Natural,
    
    /// Maps number row and letters in sequence
    /// 1 2 3 4 5 6 7 8 9 0 Q W E R T Y
    /// to CHIP-8: 1 2 3 4 5 6 7 8 9 0 A B C D E F
    Sequential,
}

/// Configuration for the keyboard input handler
#[derive(Debug, Clone)]
pub struct InputConfig {
    pub layout: KeyboardLayout,
    pub enable_debug: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            layout: KeyboardLayout::Qwerty,
            enable_debug: false,
        }
    }
}

/// Handles keyboard input for CHIP-8 emulator with multiple layout support
pub struct KeyEventHandler {
    config: InputConfig,
    key_mapping: HashMap<KeyCode, u8>,
    chip8_keys: [bool; 16],
    key_press_times: [Option<Instant>; 16],
    last_key_pressed: Option<u8>,
}

impl KeyEventHandler {
    /// Create a new keyboard event handler with the specified configuration
    pub fn new(config: InputConfig) -> Self {
        let mut handler = Self {
            config: config.clone(),
            key_mapping: HashMap::new(),
            chip8_keys: [false; 16],
            key_press_times: [None; 16],
            last_key_pressed: None,
        };
        
        handler.setup_key_mapping();
        handler
    }


    /// Setup the key mapping based on the current layout
    fn setup_key_mapping(&mut self) {
        self.key_mapping.clear();
        
        match self.config.layout {
            KeyboardLayout::Qwerty => {
                // Standard CHIP-8 QWERTY layout
                // 1 2 3 C
                // 4 5 6 D
                // 7 8 9 E
                // A 0 B F
                self.key_mapping.insert(KeyCode::Char('1'), 0x1);
                self.key_mapping.insert(KeyCode::Char('2'), 0x2);
                self.key_mapping.insert(KeyCode::Char('3'), 0x3);
                self.key_mapping.insert(KeyCode::Char('4'), 0xC);
                
                self.key_mapping.insert(KeyCode::Char('q'), 0x4);
                self.key_mapping.insert(KeyCode::Char('w'), 0x5);
                self.key_mapping.insert(KeyCode::Char('e'), 0x6);
                self.key_mapping.insert(KeyCode::Char('r'), 0xD);
                
                self.key_mapping.insert(KeyCode::Char('a'), 0x7);
                self.key_mapping.insert(KeyCode::Char('s'), 0x8);
                self.key_mapping.insert(KeyCode::Char('d'), 0x9);
                self.key_mapping.insert(KeyCode::Char('f'), 0xE);
                
                self.key_mapping.insert(KeyCode::Char('z'), 0xA);
                self.key_mapping.insert(KeyCode::Char('x'), 0x0);
                self.key_mapping.insert(KeyCode::Char('c'), 0xB);
                self.key_mapping.insert(KeyCode::Char('v'), 0xF);
            },
            
            KeyboardLayout::Natural => {
                // More intuitive hex layout
                // 1 2 3 4
                // 5 6 7 8  
                // 9 A B C
                // D E F 0
                self.key_mapping.insert(KeyCode::Char('1'), 0x1);
                self.key_mapping.insert(KeyCode::Char('2'), 0x2);
                self.key_mapping.insert(KeyCode::Char('3'), 0x3);
                self.key_mapping.insert(KeyCode::Char('4'), 0x4);
                
                self.key_mapping.insert(KeyCode::Char('q'), 0x5);
                self.key_mapping.insert(KeyCode::Char('w'), 0x6);
                self.key_mapping.insert(KeyCode::Char('e'), 0x7);
                self.key_mapping.insert(KeyCode::Char('r'), 0x8);
                
                self.key_mapping.insert(KeyCode::Char('a'), 0x9);
                self.key_mapping.insert(KeyCode::Char('s'), 0xA);
                self.key_mapping.insert(KeyCode::Char('d'), 0xB);
                self.key_mapping.insert(KeyCode::Char('f'), 0xC);
                
                self.key_mapping.insert(KeyCode::Char('z'), 0xD);
                self.key_mapping.insert(KeyCode::Char('x'), 0xE);
                self.key_mapping.insert(KeyCode::Char('c'), 0xF);
                self.key_mapping.insert(KeyCode::Char('v'), 0x0);
            },
            
            KeyboardLayout::Sequential => {
                // Sequential mapping across keyboard
                // 1 2 3 4 5 6 7 8 9 0 Q W E R T Y
                self.key_mapping.insert(KeyCode::Char('1'), 0x1);
                self.key_mapping.insert(KeyCode::Char('2'), 0x2);
                self.key_mapping.insert(KeyCode::Char('3'), 0x3);
                self.key_mapping.insert(KeyCode::Char('4'), 0x4);
                self.key_mapping.insert(KeyCode::Char('5'), 0x5);
                self.key_mapping.insert(KeyCode::Char('6'), 0x6);
                self.key_mapping.insert(KeyCode::Char('7'), 0x7);
                self.key_mapping.insert(KeyCode::Char('8'), 0x8);
                self.key_mapping.insert(KeyCode::Char('9'), 0x9);
                self.key_mapping.insert(KeyCode::Char('0'), 0x0);
                
                self.key_mapping.insert(KeyCode::Char('q'), 0xA);
                self.key_mapping.insert(KeyCode::Char('w'), 0xB);
                self.key_mapping.insert(KeyCode::Char('e'), 0xC);
                self.key_mapping.insert(KeyCode::Char('r'), 0xD);
                self.key_mapping.insert(KeyCode::Char('t'), 0xE);
                self.key_mapping.insert(KeyCode::Char('y'), 0xF);
            },
        }
    }

    /// Update the key states by polling crossterm events with timeout-based key release
    pub fn update(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        // Poll for new key events
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = event::read()? {
                match key_event.code {
                    KeyCode::Esc => return Ok(false), // Signal to quit
                    _ => self.handle_key_event(key_event),
                }
            }
        }

        Ok(true) // Continue running
    }

    /// Handle individual key events from crossterm
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let pressed = matches!(key_event.kind, KeyEventKind::Press);
        
        // Map physical key to CHIP-8 key
        if let Some(&chip8_key) = self.key_mapping.get(&key_event.code) {
            if pressed {
                // Key pressed
                self.chip8_keys[chip8_key as usize] = true;
                self.key_press_times[chip8_key as usize] = Some(Instant::now());
                self.last_key_pressed = Some(chip8_key);
            } else {
                // Key released (if crossterm supports it)
                self.chip8_keys[chip8_key as usize] = false;
                self.key_press_times[chip8_key as usize] = None;
            }
        }
    }

    /// Check if a specific CHIP-8 key is currently pressed
    pub fn is_key_pressed(&self, key: u8) -> bool {
        if key <= 0xF {
            self.chip8_keys[key as usize]
        } else {
            false
        }
    }

    /// Get the first currently pressed key, if any
    pub fn get_pressed_key(&self) -> Option<u8> {
        for (i, &pressed) in self.chip8_keys.iter().enumerate() {
            if pressed {
                return Some(i as u8);
            }
        }
        None
    }


    /// Clear the last key pressed (useful for debugging display)
    pub fn clear_last_key(&mut self) {
        self.last_key_pressed = None;
    }

    /// Get debug information about current key states
    pub fn get_debug_info(&self) -> Option<String> {
        if !self.config.enable_debug {
            return None;
        }

        let pressed_keys: Vec<String> = (0u8..16)
            .filter(|&k| self.is_key_pressed(k))
            .map(|k| format!("{:X}", k))
            .collect();

        if pressed_keys.is_empty() {
            Some("Keys: None".to_string())
        } else {
            Some(format!("Keys: [{}]", pressed_keys.join(",")))
        }
    }

    /// Get a description of the current keyboard layout
    pub fn get_layout_description(&self) -> String {
        match self.config.layout {
            KeyboardLayout::Qwerty => {
                "QWERTY Layout:\n\
                 1 2 3 4  =>  1 2 3 C\n\
                 Q W E R  =>  4 5 6 D\n\
                 A S D F  =>  7 8 9 E\n\
                 Z X C V  =>  A 0 B F".to_string()
            },
            KeyboardLayout::Natural => {
                "Natural Layout:\n\
                 1 2 3 4  =>  1 2 3 4\n\
                 Q W E R  =>  5 6 7 8\n\
                 A S D F  =>  9 A B C\n\
                 Z X C V  =>  D E F 0".to_string()
            },
            KeyboardLayout::Sequential => {
                "Sequential Layout:\n\
                 1 2 3 4 5 6 7 8 9 0 Q W E R T Y\n\
                 1 2 3 4 5 6 7 8 9 0 A B C D E F".to_string()
            },
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_mapping_setup() {
        let config = InputConfig::default();
        let handler = KeyEventHandler::new(config);
        
        // Test that key mappings are set up
        assert!(!handler.key_mapping.is_empty());
        
        // Test QWERTY layout has expected mappings
        assert_eq!(handler.key_mapping.get(&KeyCode::Char('1')), Some(&0x1));
        assert_eq!(handler.key_mapping.get(&KeyCode::Char('q')), Some(&0x4));
        assert_eq!(handler.key_mapping.get(&KeyCode::Char('x')), Some(&0x0));
    }

    #[test]
    fn test_different_layouts() {
        // Test QWERTY
        let qwerty_config = InputConfig { layout: KeyboardLayout::Qwerty, enable_debug: false };
        let qwerty_handler = KeyEventHandler::new(qwerty_config);
        assert_eq!(qwerty_handler.key_mapping.get(&KeyCode::Char('1')), Some(&0x1));
        assert_eq!(qwerty_handler.key_mapping.get(&KeyCode::Char('q')), Some(&0x4));
        
        // Test Natural
        let natural_config = InputConfig { layout: KeyboardLayout::Natural, enable_debug: false };
        let natural_handler = KeyEventHandler::new(natural_config);
        assert_eq!(natural_handler.key_mapping.get(&KeyCode::Char('1')), Some(&0x1));
        assert_eq!(natural_handler.key_mapping.get(&KeyCode::Char('v')), Some(&0x0));
        
        // Test Sequential
        let seq_config = InputConfig { layout: KeyboardLayout::Sequential, enable_debug: false };
        let seq_handler = KeyEventHandler::new(seq_config);
        assert_eq!(seq_handler.key_mapping.get(&KeyCode::Char('0')), Some(&0x0));
        assert_eq!(seq_handler.key_mapping.get(&KeyCode::Char('y')), Some(&0xF));
    }

    #[test]
    fn test_config_default() {
        let config = InputConfig::default();
        assert_eq!(config.layout, KeyboardLayout::Qwerty);
        assert_eq!(config.enable_debug, false);
    }
}
