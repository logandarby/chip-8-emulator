# CHIP-8 Emulator

Play games like it's the 1970s using the Chip8 interpreted language, right in the comfort of your own terminal!


![Rush Hour being played](/images/2025-09-03-18-01-53.png)

![Image of Pong being played in the terminal on the Chip-8 emulator](/images/2025-09-03-17-33-02.png)

![Space Invaders splash screen](/images/2025-09-03-18-01-02.png)

## Features

- Run Chip8 ROMs in your terminal of choice
- Customizable color, input-bindings, and Chip8 version
- Asynchronous (amost) event-driven runtime using message-passing architecture to emulate seperate components accurately
- Debug feature with CPU state, keybinding state, debug instruction stepping, and debug pause/play
- ROM dissassembly

## Installation 

### Prerequisites

- A computer
- Some terminal program (Git Bash, Powershell, etc.) that the user can run the emulator inside.

### Install Cargo

This project uses the rust programming language. An installation guide can be found [here](https://doc.rust-lang.org/cargo/getting-started/installation.html)

### Build the Project

Run `cargo buld --release`, and the executable will be available under `./target/release/chip-8-emulator.exe`

## How to Use

Simply run

```
./target/release/chip-8-emulator.exe <path-to-ROM>
```

to run a Chip8 ROM. Use the `--help` flag to see the options for customization.

### ROM Files

There are several rom files (programs) available in the [roms folder](/roms/). These were originally forked from [David Matlack](https://github.com/dmatlack/chip8), and I believe put together originaly by [Revival Studios](https://revival-studios.com/) (Although I'm not 100% sure of the original source).

One of my personal favourites is the Rush Hour ROM, which you can run using 

```
./target/release/chip-8-emulator.exe ./roms/games/Rush\ Hour\ \[Hap,\ 2006\].ch8
```

### Controls

The controls for games are not standardized. If a ROM file comes with an accompanying `.txt` file, then you can read that.

The original computers supporting CHIP-8 had a hexidecimal keypad with the following layout:

```
1 2 3 C
4 5 6 D
7 8 9 E
A 0 B F
```

In this emulator, this is mapped to a QWERTY keyboard by using the leftmost 4x4 block of keys on a keyboard:

```
1 2 3 4
Q W E R
A S D F
Z X C V
```

The user must be weary of this when reading old instruction for games. If this is too confusing, then the user can specify the `--layout sequential` option to map each QWERTY key to its corresponding key on the CHIP-8 machine.

The user may also use `Escape` to exit, and `P` to restart the game they are playing.

### Customization

The user can specify the color of the emulator using the `--color` flag.

### CHIP-8 Version

There were several different versions of the Chip8 language, which each had slightly different behaviour. If you notice your program is buggy, perhaps it was meant for a different version of the interpreter. This can be specified using the `--version` flag

### Debug Mode

Specifying the `--debug` flag gives the user several new controls for debugging.

- Pause/Play the emulator with `Space`
- Step the simulation forward one instruction with `Enter`

In addition to this, much more information about the internal state of the CPU, and the input handling is shown

### Troubleshooting

- Do not run this emulator in WSL, as it handles keybindings stragely
- If your program is buggy, try changing the Chip8 Version with the `--version flag`
- Some programs cannot be run in a sandboxed Chip8 emulator, because they require (no longer existent) subroutines from their host machine. If your program does not work, this could be the case

## Credits

- Thank you to [Tobias V. Langhoff's Original blog post](https://tobiasvl.github.io/blog/write-a-chip-8-emulator/) for inspiring the project
- [Cowgod's Chip-8 Technical Reference](http://devernay.free.fr/hacks/chip8/C8TECH10.HTM) for a detailed spec of the interpreter
- [Revival Studios](https://revival-studios.com/) for their great ROM preservation efforts and original games
- [David Matlack](https://github.com/dmatlack/chip8/tree/master/roms) for the list of ROMs 

