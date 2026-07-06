# Emulated Woz Machine

EWM is an emulator for the machines Steve Wozniak built: the *Apple 1*, the
*Replica 1* and the *Apple ][+*. It started life many years ago as a tiny
6502 emulator written between christmas and new year, and has since grown
into a full emulator with Disk II support, graphics, and sound.

> **Note:** EWM is a hobby project and still under development. Things may
> be incomplete, quirky, or broken — bug reports and pull requests are
> welcome. See [REWRITE.md](notes/REWRITE.md) for the project's verification
> gates and a list of known quirks and deliberate divergences.

![The EWM bootloader menu](screenshots/Screenshot%202026-07-05%20at%2011.32.44.png)

| | |
|---|---|
| ![Replica 1 running KRUSADER](screenshots/Screenshot%202026-07-05%20at%2011.37.23.png) | ![Apple \]\[+ booting DOS 3.3](screenshots/Screenshot%202026-07-05%20at%2011.30.59.png) |
| *Replica 1 — KRUSADER 1.3 from the Woz Monitor* | *Apple ][+ — booting the DOS 3.3 sample programs disk* |
| ![Frogger](screenshots/Screenshot%202026-07-05%20at%2011.31.23.png) | ![Bandits](screenshots/Screenshot%202026-07-05%20at%2011.32.01.png) |
| *Frogger in color hi-res graphics* | *Bandits by Sirius Software* |

## What's emulated

* **Apple 1** — 6502, 8KB RAM, Woz Monitor
* **Replica 1** — 65C02, 32KB RAM, KRUSADER assembler ROM
* **Apple ][+** — 6502, 48KB RAM, Apple Language Card, Disk II with two
  drives, a slot 7 hard drive for 32MB ProDOS block images (boots
  [Total Replay](https://archive.org/details/TotalReplay)!), 40-column text,
  low-resolution and high-resolution graphics (color or green monochrome),
  speaker sound, joystick paddles and buttons

## Requirements

* A [Rust toolchain](https://rustup.rs) (the pinned version is in
  `rust-toolchain.toml`; rustup picks it up automatically)
* SDL2 — `brew install sdl2` on macOS, `apt install libsdl2-dev` on
  Debian/Ubuntu

## Building

```
cargo build --release
```

## Running

Running EWM with no arguments opens the *bootloader*, a menu where keys
1/2/3 select the machine to start:

```
cargo run --release
```

Or start a machine directly:

```
# Apple ][+ with color graphics and the DOS 3.3 sample programs disk
cargo run --release -- two --color --drive1 disks/DOS33-SamplePrograms.dsk

# Apple ][+ booting Total Replay from a ProDOS hard drive image
cargo run --release -- two --color --hdd "disks/Total Replay v6.0.1.hdv"

# Replica 1 (Woz Monitor + KRUSADER)
cargo run --release -- one --model replica1

# Classic Apple 1
cargo run --release -- one --model apple1
```

Each subcommand accepts `--help` for all options. Useful keys while the
emulator runs:

| Key | Action |
|---|---|
| Cmd-Esc | Reset the machine |
| Cmd-Return | Toggle fullscreen |
| Cmd-P | Pause (Apple ][+) |
| Cmd-I | Toggle the status bar with drive lights (Apple ][+) |

There are also headless terminal consoles, handy for quick experiments
without a window:

```
cargo run -p ewm --example one                                  # Woz Monitor
cargo run -p ewm --example two -- disks/DOS33-SystemMaster.dsk  # AppleSoft / DOS 3.3
```

## Testing

```
cargo test
```

This runs the full verification suite: the Klaus Dormann 6502 and 65C02
functional tests, golden instruction traces, headless machine boot tests,
a complete DOS 3.3 boot with `CATALOG`, and a golden screenshot comparison.
The cc65 assembly sources under `tests/` are manual test programs for the
machines themselves and are not part of the automated suite.

## History

EWM was originally written in C and was ported to Rust in 2026 — the full
phase-by-phase plan, parity checklist, and benchmark numbers are preserved
in [REWRITE.md](notes/REWRITE.md). The original C implementation lives in the git
history.

## License

MIT, as declared in `Cargo.toml` (the same license the original C carried
in its source headers).
