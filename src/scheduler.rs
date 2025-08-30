use crate::{chip8::Chip8, decoder::Decoder, hardware::Hardware, input::Chip8KeyState, util};
use tokio::{select, sync::mpsc, time::interval};

// Manages messages to the hardware
pub struct HardwareScheduler;

pub enum HardwareMessage {
    ExecuteInstruction,
    UpdateKeyState(Chip8KeyState),
    DecrementTimers,
    FlushScreen,
}

impl HardwareScheduler {
    pub async fn run(hardware: &mut Hardware, mut inbox: mpsc::Receiver<HardwareMessage>) {
        while let Some(message) = inbox.recv().await {
            use HardwareMessage::*;
            match message {
                ExecuteInstruction => {
                    let raw = hardware.cpu.fetch_current_instruction();
                    hardware.execute_instruction(&Decoder::decode(&raw).unwrap());
                }
                DecrementTimers => {
                    hardware.cpu.dec_delay();
                    hardware.cpu.dec_sound();
                }
                UpdateKeyState(key_state) => {
                    hardware.set_key_state(&key_state);
                }
                FlushScreen => {
                    hardware.screen.flush();
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
    Pause,
    Play,
    Step,
    Shutdown,
}

impl ClockSheduler {
    pub async fn run(
        &self,
        mut inbox: mpsc::Receiver<ClockControlMessage>,
        hardware_sender: mpsc::Sender<HardwareMessage>,
    ) {
        let mut exec_interval = interval(util::hertz(self.hz));
        let mut is_running = true;
        let mut single_step_pending = false;
        loop {
            select! {
                message = inbox.recv() => {
                    match message {
                       Some(ClockControlMessage::Pause) => is_running = false,
                        Some(ClockControlMessage::Play) => is_running = true,
                        Some(ClockControlMessage::Shutdown) => break,
                        Some(ClockControlMessage::Step) => single_step_pending = true,
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
    pub async fn run(&self, hardware_sender: mpsc::Sender<HardwareMessage>) {
        let mut exec_interval = interval(util::hertz(self.hz));
        loop {
            exec_interval.tick().await;
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

pub struct Chip8Orchaestrator;

impl Chip8Orchaestrator {
    pub async fn run(hardware: &mut Hardware) {
        // Comm channels
        let (hard_send, hard_recv) = mpsc::channel::<HardwareMessage>(100);
        let (_clock_send, clock_recv) = mpsc::channel::<ClockControlMessage>(100);

        let timer_scheduler = TimerScheduler {
            hz: Chip8::TIMER_HZ,
        };
        let clock_scheulder = ClockSheduler {
            hz: Chip8::CPU_FREQ_HZ,
        };
        let screen_scheulder = ScreenScheduler {
            hz: Chip8::SCREEN_HZ,
        };

        select! {
            _ = timer_scheduler.run(hard_send.clone()) => {},
            _ = clock_scheulder.run(clock_recv, hard_send.clone()) => {},
            _ = screen_scheulder.run(hard_send.clone()) => {},
            _ = HardwareScheduler::run(hardware, hard_recv) => {},
        }
    }
}
