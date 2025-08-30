use std::{
    fs,
    io::{self, Write},
    panic,
};

mod chip8;
mod cpu;
mod decoder;
mod hardware;
mod input;
mod macros;
mod primitive;
mod scheduler;
mod screen;
mod util;

use chip8::*;
use clap::Parser;

#[derive(Parser)]
#[command(name = "chip8-emulator")]
#[command(about = "A CHIP-8 emulator written in Rust")]
struct Args {
    #[arg(help = "Path to the CHIP-8 ROM file")]
    rom_file: String,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Dump the HEX instructions in the ROM")]
    dump_inst: bool,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Enable debug mode showing CPU state each cycle")]
    debug: bool,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Enable step mode - pause after each instruction (requires space/enter to continue)")]
    step: bool,

    #[arg(
        long,
        default_value = "qwerty",
        help = "Keyboard layout: qwerty, natural, or sequential"
    )]
    layout: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    panic::set_hook(Box::new(|panic_info| {
        let panic_msg = format!(
            "PANIC:
  {}\n",
            panic_info
        );
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("panic.log")
        {
            let _ = file.write_all(panic_msg.as_bytes());
        }
        // Also print to stderr if possible
        eprintln!("{}", panic_msg);
    }));

    let args = Args::parse();
    println!("Reading file {}", args.rom_file);
    let bytes = fs::read(args.rom_file).expect("Could not read file");

    if args.dump_inst {
        Chip8::dump_inst(&bytes);
    } else {
        // Parse keyboard layout
        let layout = match args.layout.to_lowercase().as_str() {
            "qwerty" => input::KeyboardLayout::Qwerty,
            "natural" => input::KeyboardLayout::Natural,
            "sequential" => input::KeyboardLayout::Sequential,
            _ => {
                eprintln!("Invalid layout '{}'. Using qwerty.", args.layout);
                input::KeyboardLayout::Qwerty
            }
        };

        // Create input configuration
        let input_config = input::InputConfig {
            layout,
            ..Default::default()
        };

        // Create input handler
        let input_handler = input::KeyEventHandler::new(input_config);

        // Print layout info
        if args.debug {
            println!(
                "Using keyboard layout:\n{}",
                input_handler.get_layout_description()
            );
        }

        // Create emulator
        let config = Chip8Config {
            version: Chip8Version::COSMAC,
            debug: args.debug,
            step_mode: args.step,
        };
        let mut chip8 = Chip8::new(config, input_handler);
        chip8.load_rom(&bytes).expect("Could not load the ROM");
        chip8.cycle().await;
    }

    Ok(())
}
