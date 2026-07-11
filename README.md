# Emulated Woz Machine

EWM is an emulator for the machines Steve Wozniak built: the *Apple 1*, the
*Replica 1* and the *Apple ][+* — plus the Enhanced *Apple //e*. It started
life many years ago as a tiny 6502 emulator written between christmas and new
year, and has since grown into a full emulator with Disk II support, graphics,
and sound.

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
  low-resolution and high-resolution graphics on a green, amber or white
  monochrome monitor or in color (`--color [green|amber|white|rgb]`) with an
  optional scanline effect (`--scanlines [off|light|heavy]`), both switchable
  at runtime from the command palette,
  speaker sound, joystick paddles and buttons (game controllers hot-plug —
  Bluetooth pads connect any time — and the command palette picks between
  several)
* **Disk images** — `.dsk`/`.do`/`.po` sector images, `.nib` nibble images,
  and bit-accurate [WOZ 1.0](https://applesaucefdc.com/woz/reference1/)
  images with copy-protection support (E7, RWTS18, half-tracks, MC3470
  weak bits — see `notes/WOZ1.md` for the compatibility table)
* **Apple //e (Enhanced)** — 65C02, 128KB main + auxiliary RAM, the built-in
  language card and MMU/IOU soft switches, a swappable auxiliary slot
  (`--aux`): the Extended 80-Column Text Card (64K, default), the plain
  80-Column Text Card (1K), or an Applied Engineering RamWorks III with up
  to 8MB (`--aux ramworksiii:1m`), 40- and 80-column text with lower
  case and MouseText, lo-res / hi-res / double-lo-res / double-hi-res
  graphics, and the //e keyboard (Open/Solid-Apple keys). Reuses the Disk II,
  hard drive, clock and sound. Start it with `two --model 2e`.

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

# A copy-protected WOZ 1.0 image (bit-accurate Disk II emulation)
cargo run --release -- two --color --drive1 "disks/woz/WOZ 1.0/Commando - Disk 1, Side A.woz"

# Enhanced Apple //e booting DOS 3.3 (try PR#3 for 80-column lower case)
cargo run --release -- two --model 2e --color --drive1 disks/DOS33-SystemMaster.dsk

# Enhanced Apple //e with an 8MB RamWorks III in the auxiliary slot
cargo run --release -- two --model 2e --aux ramworksiii --drive1 disks/DOS33-SystemMaster.dsk

# Replica 1 (Woz Monitor + KRUSADER)
cargo run --release -- one --model replica1

# Classic Apple 1
cargo run --release -- one --model apple1
```

A whole machine can also be described in a JSON file and started with
`--config` (explicitly given flags override the file):

```
cargo run --release -- two --config myiie.json
```

```json
{
  "$schema": "https://raw.githubusercontent.com/st3fan/ewm/main/schema/ewm-config.schema.json",
  "machine": {
    "model": "2e",
    "aux": { "card": "ramworksiii", "size": "1m" },
    "slots": {
      "1": { "card": "thunderclock" },
      "5": { "card": "diskii", "drive1": "disks/work.dsk" },
      "6": { "card": "diskii", "drive1": "disks/DOS33-SystemMaster.dsk" },
      "7": { "card": "harddrive", "image": "disks/Total Replay v6.0.1.hdv" }
    }
  },
  "display": { "monitor": "green", "scanlines": "light" },
  "cpu": { "speed": "normal" }
}
```

Ready-made configs for the classic machines live in `configs/` — an
Apple ][+ with a green monitor (`plus.json`) and an Enhanced //e with
the extended 80-column card and a color monitor (`enhanced.json`).
Neither names a disk, so pair them with the drive flags, which merge
into the config's slot 6 card:

```
cargo run --release -- two --config configs/enhanced.json --drive1 disks/DOS33-SystemMaster.dsk
```

The machine's physical layout lives in `slots`: any card in any slot,
up to three Disk ][ controllers (six drives), multiple hard drives,
empty slots — the Autostart scan boots the highest populated slot, as
on hardware. Relative paths resolve against the config file's
directory, so a config travels with its disks. The committed JSON
Schema (`schema/ewm-config.schema.json`) gives editors validation and
autocomplete via the `$schema` key; `notes/JSON_CONFIG.md` has the full
plan.

Each subcommand accepts `--help` for all options. Useful keys while the
emulator runs:

| Key | Action |
|---|---|
| Cmd-R | Reset the machine |
| Cmd-Return | Toggle fullscreen |
| Cmd-P | Pause (Apple ][+ / //e) |
| Cmd-I | Toggle the status bar with drive lights (Apple ][+ / //e) |

There are also headless terminal consoles, handy for quick experiments
without a window:

```
cargo run -p ewm --example one                                  # Woz Monitor
cargo run -p ewm --example two -- disks/DOS33-SystemMaster.dsk  # AppleSoft / DOS 3.3
```

## Native Mac app

`scripts/make-app.sh` assembles a self-contained, double-clickable
`dist/EWM.app` — SDL3 is compiled in statically (CMake required at build
time), the icon is `][` rendered by the emulator's own character generator,
and the bundle is ad-hoc signed for local use. Opening the app boots the
bootloader menu; opening a disk image with it (or dragging one onto the
window) boots the ][+ with that disk — dropping a floppy on a running
machine swaps drive 1. See `notes/MAC_APP.md` for the plan (signing and
notarization for distribution are Phase 2).

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
