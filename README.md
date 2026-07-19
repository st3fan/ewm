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
  monochrome monitor or in color (`--set display:monitor=rgb`) with an
  optional scanline effect (`--set display:scanlines=light`), both switchable
  at runtime from the command palette,
  speaker sound, joystick paddles and buttons (game controllers hot-plug —
  Bluetooth pads connect any time — and the command palette picks between
  several)
* **Disk images** — `.dsk`/`.do`/`.po` sector images, `.nib` nibble images,
  bit-accurate [WOZ 1.0](https://applesaucefdc.com/woz/reference1/)
  images with copy-protection support (E7, RWTS18, half-tracks, MC3470
  weak bits — see `notes/WOZ1.md` for the compatibility table), and
  `.2mg` images of 400K/800K 3.5" disks in the UniDisk 3.5 Controller
  ("Liron", `{"card": "liron"}`), a SmartPort card ProDOS boots from
* **Apple //e (Enhanced)** — 65C02, 128KB main + auxiliary RAM, the built-in
  language card and MMU/IOU soft switches, a swappable auxiliary slot
  (`machine.aux`): the Extended 80-Column Text Card (64K, default), the plain
  80-Column Text Card (1K), or an Applied Engineering RamWorks III with up
  to 8MB (`--set 'machine:aux={"card":"ramworksiii","size":"1m"}'`), 40- and
  80-column text with lower
  case and MouseText, lo-res / hi-res / double-lo-res / double-hi-res
  graphics, and the //e keyboard (Open/Solid-Apple keys). Reuses the Disk II,
  hard drive, clock and sound. Start it with `two --config builtin:2e`.

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
cargo run --release -- two --set display:monitor=rgb \
    --set machine:slots:6:drive1=disks/DOS33-SamplePrograms.dsk

# Apple ][+ booting Total Replay from a ProDOS hard drive image
cargo run --release -- two --set display:monitor=rgb \
    --set 'machine:slots:7={"card":"harddrive","image":"disks/Total Replay v6.0.1.hdv"}'

# A copy-protected WOZ 1.0 image (bit-accurate Disk II emulation)
cargo run --release -- two --set display:monitor=rgb \
    --set "machine:slots:6:drive1=disks/woz/WOZ 1.0/Commando - Disk 1, Side A.woz"

# Enhanced Apple //e (a built-in config) booting DOS 3.3 (try PR#3 for 80-column lower case)
cargo run --release -- two --config builtin:2e \
    --set machine:slots:6:drive1=disks/DOS33-SystemMaster.dsk

# Enhanced Apple //e with an 8MB RamWorks III in the auxiliary slot
cargo run --release -- two --config builtin:2e \
    --set machine:aux:card=ramworksiii \
    --set machine:slots:6:drive1=disks/DOS33-SystemMaster.dsk

# Replica 1 (Woz Monitor + KRUSADER)
cargo run --release -- one --model replica1

# Classic Apple 1
cargo run --release -- one --model apple1
```

### The three `two` machine profiles

Bare `ewm two` boots the **default machine**: an Apple ][+ with the 16K
Language Card in slot 0 (the classic 64K build), a Thunderclock in
slot 1, and a Disk II controller in slot 6, on a green monochrome
monitor. In config terms (`--print-config` prints the full document):

```json
{
  "machine": {
    "model": "2plus",
    "slots": {
      "0": { "card": "language" },
      "1": { "card": "thunderclock" },
      "6": { "card": "diskii" }
    }
  }
}
```

Two more profiles ship *inside the binary* as built-in configs —
`--config builtin:list` lists them, and the same files live in
`configs/`:

**`builtin:2plus`** — an Apple ][+ with the 64K Language Card and a
Disk II in slot 6, on a green monochrome monitor. The default machine
minus the clock card:

```json
{
  "description": "Apple ][+ — 64K Language Card, Disk II in slot 6, green monitor",
  "machine": {
    "model": "2plus",
    "slots": {
      "0": { "card": "language" },
      "6": { "card": "diskii" }
    }
  },
  "display": { "monitor": "green" }
}
```

```
cargo run --release -- two --config builtin:2plus
```

**`builtin:2e`** — an Enhanced Apple //e with the Extended 80-Column
Card (64K) in the auxiliary slot, a UniDisk 3.5 controller in slot 5,
a Disk II in slot 6, and an RGB color monitor:

```json
{
  "description": "Enhanced Apple //e — Extended 80-Column Card, UniDisk 3.5 in slot 5, Disk II in slot 6, RGB monitor",
  "machine": {
    "model": "2e",
    "aux": { "card": "ext80col" },
    "slots": {
      "5": { "card": "liron" },
      "6": { "card": "diskii" }
    }
  },
  "display": { "monitor": "rgb" }
}
```

```
cargo run --release -- two --config builtin:2e
```

None of the profiles mounts a disk — pair them with a `--set` override
or an overlay, as in the examples above.

### Composing a machine

`--set <key>=<value>` overrides one value in the machine configuration
by its colon-separated key path — any key the JSON config (below)
accepts, so `--set display:monitor=amber` or `--set cpu:speed=3.58mhz`
work the same way. Values are JSON when they parse as JSON (numbers,
booleans, whole objects like the hard-drive example above) and plain
strings otherwise.

The machine configuration is fully compositional: four source kinds,
one document, merged strictly in command-line order —

- **`--config <source>`** — a *complete* machine, from a JSON file or a
  built-in config; at most one, the base of the document;
- **`--config-overlay <source>`** — a *partial* config layered on top;
  repeatable;
- **`--set <key>=<value>`** — single-value overrides;
- and `--serve <url>`, structured sugar for the whole `remote` block,
  which overrides the finished document.

An overlay describes just the part of the machine it cares about. This
one (`examples/drive-with-total-replay.json`) adds a hard drive with
Total Replay to whatever machine it lands on:

```json
{
  "$schema": "https://raw.githubusercontent.com/st3fan/ewm/main/schema/ewm-config-overlay.schema.json",
  "machine": {
    "slots": {
      "7": { "card": "harddrive", "image": "../disks/Total Replay v6.0.1.hdv" }
    }
  }
}
```

```
cargo run --release -- two --config builtin:2plus \
    --config-overlay examples/drive-with-total-replay.json \
    --set display:monitor=amber
```

Overlays without a `--config` extend the *default* machine, so the same
overlay alone means "the default ][+ plus a hard drive in slot 7":

```
cargo run --release -- two --config-overlay examples/drive-with-total-replay.json
```

A whole machine also fits in one file (`examples/myiie.json`):

```json
{
  "$schema": "https://raw.githubusercontent.com/st3fan/ewm/main/schema/ewm-config.schema.json",
  "machine": {
    "model": "2e",
    "aux": { "card": "ramworksiii", "size": "1m" },
    "slots": {
      "1": { "card": "thunderclock" },
      "5": { "card": "diskii", "drive1": "../disks/work.dsk" },
      "6": { "card": "diskii", "drive1": "../disks/DOS33-SystemMaster.dsk" },
      "7": { "card": "harddrive", "image": "../disks/Total Replay v6.0.1.hdv" }
    }
  },
  "display": { "monitor": "green", "scanlines": "light" },
  "cpu": { "speed": "normal" }
}
```

```
cargo run --release -- two --config examples/myiie.json
```

The machine's physical layout lives in `slots`: any card in any slot,
up to three Disk ][ controllers (six drives), multiple hard drives,
empty slots — the Autostart scan boots the highest populated slot, as
on hardware. On the ][+, slot 0 is the memory-expansion socket: the
default machine has a Language Card (the classic 64K build), and
`--set machine:slots:0:card=saturn128` swaps it for a Saturn Systems
128K RAM Board — eight Language-Card-compatible 16K banks. An explicit
`slots` table is taken literally, so leave out `"0"` — or pass
`--set machine:slots:0:card=empty` — for a stock 48K machine.

Relative paths resolve against their file's directory, so a config —
or an overlay — travels with its disks. The committed JSON Schemas
(`schema/ewm-config.schema.json` for complete configs,
`schema/ewm-config-overlay.schema.json` for overlays) give editors
validation and autocomplete via the `$schema` key;
`notes/JSON_CONFIG.md` has the full plan.

With sources layering, `--print-config` answers "what machine did I
just describe?" — it prints the final merged configuration (sources
*and* convenience flags applied) as JSON and exits, nonzero on any
error, so it doubles as a config linter for scripts and CI:

```
cargo run --release -- two --config examples/myiie.json --set display:monitor=amber --print-config
```

### Debugging with WozBug

`--wozbug` starts WozBug — a minimal, Woz-Monitor-dialect debugger — as a
line server on a local TCP port (default 6502, naturally). `--break`
arms breakpoints at boot, by hex address or built-in symbol (RWTS, MLI,
COUT, …), and implies the server:

```
cargo run --release -- two --break RWTS \
    --set machine:slots:6:drive1=disks/DOS33-SystemMaster.dsk
# in another terminal:
nc localhost 6502
```

When a breakpoint hits, the machine freezes and the client sees the
registers. `280.29F` dumps memory (a bare Return continues), `300:A9 20`
deposits, `R`/`PC=BD00` show and set registers, `S` single-steps, `G`
resumes, and `DSK`/`SW`/`TEXT`/`SLOTS` dump the disk controllers, soft
switches, text screen, and slot table. `?` lists everything; the plan
lives in `notes/DEBUGGING_TOOLS.md`.

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
