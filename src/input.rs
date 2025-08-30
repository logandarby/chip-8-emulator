use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::collections::HashMap;
use std::time::Duration;

// Struct to store and send key state to different components
#[derive(Default, Clone, Copy)]
pub struct Chip8KeyState {
    keys_pressed: [bool; Self::TOTAL_KEYS],
}

impl Chip8KeyState {
    const TOTAL_KEYS: usize = 16;
    pub fn press(&mut self, key: u8) {
        self.keys_pressed[key as usize] = true;
    }
    pub fn release(&mut self, key: u8) {
        self.keys_pressed[key as usize] = false;
    }
    pub fn is_key_pressed(&self, key: u8) -> bool {
        self.keys_pressed[key as usize]
    }
}

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

impl KeyboardLayout {
    pub fn get_key_map(layout: &Self) -> HashMap<KeyCode, u8> {
        match layout {
            KeyboardLayout::Qwerty => {
                // Standard CHIP-8 QWERTY layout
                // 1 2 3 C
                // 4 5 6 D
                // 7 8 9 E
                // A 0 B F
                HashMap::from([
                    (KeyCode::Char('1'), 0x1),
                    (KeyCode::Char('2'), 0x2),
                    (KeyCode::Char('3'), 0x3),
                    (KeyCode::Char('4'), 0xC),
                    (KeyCode::Char('q'), 0x4),
                    (KeyCode::Char('w'), 0x5),
                    (KeyCode::Char('e'), 0x6),
                    (KeyCode::Char('r'), 0xD),
                    (KeyCode::Char('s'), 0x8),
                    (KeyCode::Char('d'), 0x9),
                    (KeyCode::Char('f'), 0xE),
                    (KeyCode::Char('z'), 0xA),
                    (KeyCode::Char('x'), 0x0),
                    (KeyCode::Char('c'), 0xB),
                    (KeyCode::Char('v'), 0xF),
                ])
            }

            KeyboardLayout::Natural => {
                // More intuitive hex layout
                // 1 2 3 4
                // 5 6 7 8
                // 9 A B C
                // D E F 0
                HashMap::from([
                    (KeyCode::Char('1'), 0x1),
                    (KeyCode::Char('2'), 0x2),
                    (KeyCode::Char('3'), 0x3),
                    (KeyCode::Char('4'), 0x4),
                    (KeyCode::Char('q'), 0x5),
                    (KeyCode::Char('w'), 0x6),
                    (KeyCode::Char('e'), 0x7),
                    (KeyCode::Char('r'), 0x8),
                    (KeyCode::Char('a'), 0x9),
                    (KeyCode::Char('s'), 0xA),
                    (KeyCode::Char('d'), 0xB),
                    (KeyCode::Char('f'), 0xC),
                    (KeyCode::Char('z'), 0xD),
                    (KeyCode::Char('x'), 0xE),
                    (KeyCode::Char('c'), 0xF),
                    (KeyCode::Char('v'), 0x0),
                ])
            }

            KeyboardLayout::Sequential => {
                // Sequential mapping across keyboard
                // 1 2 3 4 5 6 7 8 9 0 Q W E R T Y
                HashMap::from([
                    (KeyCode::Char('1'), 0x1),
                    (KeyCode::Char('2'), 0x2),
                    (KeyCode::Char('3'), 0x3),
                    (KeyCode::Char('4'), 0x4),
                    (KeyCode::Char('5'), 0x5),
                    (KeyCode::Char('6'), 0x6),
                    (KeyCode::Char('7'), 0x7),
                    (KeyCode::Char('8'), 0x8),
                    (KeyCode::Char('9'), 0x9),
                    (KeyCode::Char('0'), 0x0),
                    (KeyCode::Char('q'), 0xA),
                    (KeyCode::Char('w'), 0xB),
                    (KeyCode::Char('e'), 0xC),
                    (KeyCode::Char('r'), 0xD),
                    (KeyCode::Char('t'), 0xE),
                    (KeyCode::Char('y'), 0xF),
                ])
            }
        }
    }
}

/// Configuration for the keyboard input handler
#[derive(Debug, Clone)]
pub struct InputConfig {
    pub layout: KeyboardLayout,
    pub poll_rate: Duration,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            layout: KeyboardLayout::Qwerty,
            poll_rate: Duration::from_millis(10),
        }
    }
}

#[derive(PartialEq)]
pub enum Chip8KeyEventKind {
    Press,
    Release,
}

pub enum Chip8Command {
    Quit,
    DebugStep,
    DebugPlayPause,
}

pub enum Chip8InputEvent {
    CommandEvent {
        command: Chip8Command,
        kind: Chip8KeyEventKind,
    },
    Chip8KeyEvent {
        key: u8,
        kind: Chip8KeyEventKind,
    },
}

pub struct KeyEventHandler {
    config: InputConfig,
    key_mapping: HashMap<KeyCode, u8>,
    chip8_keys: Chip8KeyState,
}

impl KeyEventHandler {
    pub fn new(config: InputConfig) -> Self {
        Self {
            config: config.clone(),
            key_mapping: KeyboardLayout::get_key_map(&config.layout),
            chip8_keys: Chip8KeyState::default(),
        }
    }

    /// Update the key states by polling crossterm events
    pub async fn next_input_event(&self) -> Chip8InputEvent {
        let rate = self.config.poll_rate.clone();
        loop {
            match tokio::task::spawn_blocking(move || {
                event::poll(rate)
                    .ok()
                    .filter(|&has_event| has_event)
                    .and_then(|_| event::read().ok())
            })
            .await
            {
                Ok(Some(Event::Key(key_event))) => {
                    if let Some(key_event) = self.handle_key_event(key_event) {
                        return key_event;
                    } else {
                        continue;
                    }
                }
                _ => {
                    tokio::time::sleep(rate).await;
                    continue;
                }
            }
        }
    }

    fn handle_key_event(&self, key_event: KeyEvent) -> Option<Chip8InputEvent> {
        let pressed = match key_event.kind {
            KeyEventKind::Press => Chip8KeyEventKind::Press,
            KeyEventKind::Release => Chip8KeyEventKind::Release,
            _ => return None,
        };

        // Map physical key to CHIP-8 key
        if let Some(&chip8_key) = self.key_mapping.get(&key_event.code) {
            Some(Chip8InputEvent::Chip8KeyEvent {
                key: chip8_key,
                kind: pressed,
            })
        // Physical key for debug/quit commands
        } else {
            let command = match key_event.code {
                KeyCode::Esc => Chip8Command::Quit,
                KeyCode::Char(' ') => Chip8Command::DebugPlayPause,
                KeyCode::Enter => Chip8Command::DebugStep,
                _ => return None,
            };
            Some(Chip8InputEvent::CommandEvent {
                command,
                kind: pressed,
            })
        }
    }

    /// Get a description of the current keyboard layout
    pub fn get_layout_description(&self) -> String {
        match self.config.layout {
            KeyboardLayout::Qwerty => "QWERTY Layout:\n\
                 1 2 3 4  =>  1 2 3 C\n\
                 Q W E R  =>  4 5 6 D\n\
                 A S D F  =>  7 8 9 E\n\
                 Z X C V  =>  A 0 B F"
                .to_string(),
            KeyboardLayout::Natural => "Natural Layout:\n\
                 1 2 3 4  =>  1 2 3 4\n\
                 Q W E R  =>  5 6 7 8\n\
                 A S D F  =>  9 A B C\n\
                 Z X C V  =>  D E F 0"
                .to_string(),
            KeyboardLayout::Sequential => "Sequential Layout:\n\
                 1 2 3 4 5 6 7 8 9 0 Q W E R T Y\n\
                 1 2 3 4 5 6 7 8 9 0 A B C D E F"
                .to_string(),
        }
    }
}
