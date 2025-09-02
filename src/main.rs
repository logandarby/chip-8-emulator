use std::{
    fs,
    io::{self, Write},
    panic::{self, PanicHookInfo},
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

    #[arg(
        long,
        default_value_t = input::KeyboardLayout::Qwerty,
        help = "Keyboard layout: qwerty, natural, or sequential"
    )]
    layout: input::KeyboardLayout,

    #[arg(
        long,
        default_value_t = Chip8Version::Cosmac,
        help = "CHIP-8 version: cosmac, chip48, or superchip"
    )]
    version: Chip8Version,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    panic::set_hook(Box::new(panic_handler));

    let args = Args::parse();
    let bytes = fs::read(args.rom_file)?;

    if args.dump_inst {
        Chip8::dump_inst(&bytes);
        return Ok(());
    }
    // Create input configuration
    let input_config = input::InputConfig {
        layout: args.layout,
        ..Default::default()
    };

    // Create input handler
    let input_handler = input::KeyEventHandler::new(input_config);

    // Create emulator
    let config = Chip8Config {
        version: args.version,
        debug: args.debug,
    };
    let mut chip8 = Chip8::new(config, input_handler);
    chip8.load_rom(&bytes).expect("Could not load the ROM");
    chip8.cycle().await;

    Ok(())
}

fn panic_handler(panic_info: &PanicHookInfo) {
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
}
