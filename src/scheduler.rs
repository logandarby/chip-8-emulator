use crate::{
    chip8::Chip8,
    decoder::Decoder,
    hardware::Hardware,
    input::{
        Chip8Command, Chip8InputEvent, Chip8KeyEvent, Chip8KeyEventKind, Chip8KeyState,
        KeyEventHandler,
    },
    util,
};

#[derive(Clone, Debug)]
pub enum PlaybackMode {
    Running,
    Paused,
    Stepping,
}
use tokio::{select, sync::mpsc, time::interval};

// Manages messages to the hardware
pub struct HardwareScheduler;

pub enum HardwareMessage {
    ExecuteInstruction,
    UpdateKeyState(Chip8KeyState),
    HandleKeyEvent(Chip8KeyEvent),
    DecrementTimers,
    FlushScreen,
    UpdateDebugInfo,
}

impl HardwareScheduler {
    pub async fn run(hardware: &mut Hardware, mut inbox: mpsc::Receiver<HardwareMessage>) {
        while let Some(message) = inbox.recv().await {
            use HardwareMessage::*;
            match message {
                ExecuteInstruction => {
                    // Skip execution if CPU is waiting for key input
                    if !hardware.is_waiting_for_key() {
                        let raw = hardware.cpu.fetch_current_instruction();
                        hardware
                            .execute_instruction(&Decoder::decode(&raw).unwrap())
                            .await;
                    }
                }
                HandleKeyEvent(Chip8KeyEvent { key, kind }) => {
                    // Try to handle key event if CPU is waiting
                    hardware.handle_key_when_waiting(key, kind);
                }
                DecrementTimers => {
                    hardware.cpu.dec_delay();
                    hardware.cpu.dec_sound();
                }
                UpdateKeyState(key_state) => {
                    hardware.set_key_state(&key_state);
                }
                FlushScreen => {
                    hardware.screen.flush().unwrap();
                }
                UpdateDebugInfo => {
                    hardware.update_debug_info();
                }
            }
        }
    }
}

// Manages the main clock cycle of the CPU, with pause/play controls
pub struct ClockSheduler {
    pub hz: f64,
}

pub enum ClockControlMessage {
    TogglePausePlay,
    Step,
    Shutdown,
}

impl ClockSheduler {
    pub async fn run(
        &self,
        mut inbox: mpsc::Receiver<ClockControlMessage>,
        hardware_sender: mpsc::Sender<HardwareMessage>,
        initial_is_running: bool,
        playback_state_sender: Option<mpsc::Sender<PlaybackMode>>,
    ) {
        let mut exec_interval = interval(util::hertz(self.hz));
        let mut is_running = initial_is_running;
        let mut single_step_pending = false;

        // Send initial state
        if let Some(ref sender) = playback_state_sender {
            let _ = sender
                .send(if is_running {
                    PlaybackMode::Running
                } else {
                    PlaybackMode::Paused
                })
                .await;
        }
        loop {
            select! {
                message = inbox.recv() => {
                    match message {
                       Some(ClockControlMessage::TogglePausePlay) => {
                           is_running = !is_running;
                           if is_running {
                               exec_interval.reset();
                           }
                           // Update playback state
                           if let Some(ref sender) = playback_state_sender {
                               let _ = sender.send(if is_running { PlaybackMode::Running } else { PlaybackMode::Paused }).await;
                           }
                       },
                        Some(ClockControlMessage::Shutdown) => break,
                        Some(ClockControlMessage::Step) => {
                            single_step_pending = true;
                            // Update playback state to show stepping
                            if let Some(ref sender) = playback_state_sender {
                                let _ = sender.send(PlaybackMode::Stepping).await;
                            }
                        },
                        None => break,
                    }
                },
                _ = exec_interval.tick(), if is_running => {
                    let _ = hardware_sender.send(HardwareMessage::ExecuteInstruction).await;
                },
                _ = async {}, if single_step_pending => {
                    let _ = hardware_sender.send(HardwareMessage::ExecuteInstruction).await;
                    single_step_pending = false;
                }
            }
        }
    }
}

// Manages the decrementing of the CPUs timers
struct TimerScheduler {
    pub hz: f64,
}

impl TimerScheduler {
    pub async fn run(&self, hardware_sender: mpsc::Sender<HardwareMessage>) {
        let mut exec_interval = interval(util::hertz(self.hz));
        loop {
            exec_interval.tick().await;
            if hardware_sender
                .send(HardwareMessage::DecrementTimers)
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

// Manages the screen refresh rate
struct ScreenScheduler {
    pub hz: f64,
}

impl ScreenScheduler {
    pub async fn run(&self, hardware_sender: mpsc::Sender<HardwareMessage>, debug_enabled: bool) {
        let mut exec_interval = interval(util::hertz(self.hz));
        loop {
            exec_interval.tick().await;

            // Update debug info if enabled
            if debug_enabled {
                if hardware_sender
                    .send(HardwareMessage::UpdateDebugInfo)
                    .await
                    .is_err()
                {
                    break;
                }
            }

            if hardware_sender
                .send(HardwareMessage::FlushScreen)
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

pub struct InputScheduler {
    key_state: Chip8KeyState,
}

impl InputScheduler {
    pub fn new() -> Self {
        Self {
            key_state: Chip8KeyState::default(),
        }
    }

    pub async fn run(
        &mut self,
        input: &KeyEventHandler,
        hardware_sender: mpsc::Sender<HardwareMessage>,
        clock_sender: mpsc::Sender<ClockControlMessage>,
    ) {
        loop {
            let input_event = input.next_input_event().await;
            match input_event {
                Chip8InputEvent::Chip8KeyEvent(Chip8KeyEvent { key, kind }) => {
                    // Update local key state
                    if kind == Chip8KeyEventKind::Press {
                        self.key_state.press(key);
                    } else {
                        self.key_state.release(key);
                    }

                    // Send key event to hardware (for GetKey instruction handling)
                    let _ = hardware_sender
                        .send(HardwareMessage::HandleKeyEvent(Chip8KeyEvent { key, kind }))
                        .await;

                    // Update hardware key state (for SkipKeyPress instructions)
                    let _ = hardware_sender
                        .send(HardwareMessage::UpdateKeyState(self.key_state.clone()))
                        .await;
                }
                Chip8InputEvent::CommandEvent { command, kind }
                    if kind == Chip8KeyEventKind::Press =>
                {
                    match command {
                        Chip8Command::Quit => {
                            let _ = clock_sender.send(ClockControlMessage::Shutdown).await;
                        }
                        Chip8Command::DebugPlayPause => {
                            let _ = clock_sender
                                .send(ClockControlMessage::TogglePausePlay)
                                .await;
                        }
                        Chip8Command::DebugStep => {
                            let _ = clock_sender.send(ClockControlMessage::Step).await;
                        }
                    };
                }
                _ => {}
            };
        }
    }
}

pub struct Chip8Orchaestrator;

impl Chip8Orchaestrator {
    pub async fn run(chip8: &mut Chip8) {
        // Comm channels
        let (hard_send, hard_recv) = mpsc::channel::<HardwareMessage>(100);
        let (clock_send, clock_recv) = mpsc::channel::<ClockControlMessage>(100);
        let (playback_send, playback_recv) = mpsc::channel::<PlaybackMode>(100);

        let timer_scheduler = TimerScheduler {
            hz: Chip8::TIMER_HZ,
        };
        let clock_scheulder = ClockSheduler {
            hz: Chip8::CPU_FREQ_HZ,
        };
        let screen_scheulder = ScreenScheduler {
            hz: Chip8::SCREEN_HZ,
        };
        let mut input_scheduler = InputScheduler::new();

        // Set up hardware to receive playback state updates
        chip8.hardware.set_playback_receiver(playback_recv);

        select! {
            _ = timer_scheduler.run(hard_send.clone()) => {},
            _ = clock_scheulder.run(clock_recv, hard_send.clone(), !chip8.config.debug, if chip8.config.debug { Some(playback_send) } else { None }) => {},
            _ = screen_scheulder.run(hard_send.clone(), chip8.config.debug) => {},
            _ = HardwareScheduler::run(&mut chip8.hardware, hard_recv) => {},
            _ = input_scheduler.run(&chip8.input, hard_send, clock_send) => {},
        }
    }
}
