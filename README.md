# Emulated Woz Machine


## Introduction

Two years ago between christmas and new year I wrote a tiny and incomplete 6502 emulator and turned it into an original *Apple 1* emulator. It was a fun and nostalgic project to work on. I grew up with the *Apple II* and never had a change to see an *Apple 1* in action.

A few weeks ago I decided to pick this project up again. I am extremely motivated to turn this into a high quality emulator that supports the *Apple 1*, *Replica 1*, *Apple ][+* and *Apple IIe*. Some of that work is really close to being finished, other work will take many months of spare time hacking.

![](https://raw.githubusercontent.com/st3fan/ewm/master/screenshots/Screen%20Shot%202016-11-16%20at%203.59.44%20PM.png)

## Goals & Status

Here are some of the things I want to accomplish for each emulated machine:

### CPU Emulator 

* ~~6502 support~~
* ~~65C02 support~~
* ~~Tracing facility~~
* Debugger
* Speed throttling

### Apple 1

*8K / 6502 / Classic ROM*

* ~~Terminal based emulation~~
* ~~Classic display emulation (SDL based)~~
* Cassette interface

### Replica 1

*32K / 65C02 / KRUSADER ROM*

* ~~Terminal based emulation~~
* ~~Classic display emulation (SDL based)~~
* Cassette interface
* [CFFA1](http://dreher.net/?s=projects/CFforApple1&c=projects/CFforApple1/main.php) Support

### Apple ][+

*48K / 6502*

* ~~Basic Apple ][+ architecture implementation - In progress~~
* ~~Disk II emulation - In progress~~
* ~~Display Emulation - 40 Column mode~~
* ~~Display Emulation - Low resolution graphics~~
* ~~Apple Language Card~~
* ~~Joystick Support~~
* Audio Support
* Display Emulation - High resolution graphics - Mostly works.

## Building the emulator

EWM is written in Rust (ported from the original C implementation; see
[REWRITE.md](REWRITE.md) for the full story). You need a Rust toolchain
(rustup) and SDL2 (`brew install sdl2` / `apt install libsdl2-dev`).

```
cargo build --release
```

## Running the emulator

```
cargo run --release -- two --color --drive1 disks/DOS33-SamplePrograms.dsk
```

Running `cargo run --release` with no arguments opens the bootloader menu,
where keys 1/2/3 pick the machine. See `--help` on each subcommand
(`one`, `two`) for all options.

`cargo test` runs the full verification suite: the Klaus Dormann 6502/65C02
functional tests, golden instruction traces captured from the C emulator,
headless machine boot tests, a full DOS 3.3 boot with CATALOG, and a golden
screenshot comparison. There are also headless consoles for quick
experiments without a window:

```
cargo run -p ewm-core --example one                                  # Woz monitor
cargo run -p ewm-core --example two -- disks/DOS33-SystemMaster.dsk  # AppleSoft/DOS
```

The cc65 assembly sources under `tests/` are manual test programs for the
machines themselves and are not part of the automated suite.
