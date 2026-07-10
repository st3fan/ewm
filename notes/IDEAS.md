# EWM Feature Ideas

A living backlog of feature ideas, gathered from a survey of the established
Apple II emulators — AppleWin (Windows), Virtual ][ (macOS), MAME's apple2e
driver, izapple2 (Go), Apple2TS (TypeScript) — plus the "Future work" lists
already in `REWRITE.md`, `APPLE_IIE_ENHANCED.md` and `WOZ1.md`. This is not a
plan: when an idea graduates, it gets its own working document in the style
of `APPLE_IIE_ENHANCED.md` / `WOZ1.md` (phases, gates, one PR per phase).

Sizes are the house scale (S/M/L). *(infra: …)* notes point at existing EWM
code an idea builds on.

**Where EWM stands today**, for contrast: Apple 1 / Replica 1 / ][+ /
Enhanced //e; 6502 + 65C02; Disk II with `.dsk`/`.do`/`.po`/`.nib`/WOZ 1.0
(read-only); slot 7 hard drive (boots Total Replay); Thunderclock; speaker
audio; paddles/buttons/joystick; 40/80-col text, LGR/HGR/DLGR/DHGR in color
or green mono; command palette (reset, pause, fullscreen, 3 CPU speeds);
status bar with drive lights; `--trace`; headless terminal examples; a
deterministic golden-BMP test culture.

## Hardware support (peripheral cards)

- **Mockingboard / Phasor** (M/L) — the single biggest game-compatibility
  card: AY-3-8910 PSGs driven by 6522 VIAs (slot 4/5). Ultima III–V, Skyfox,
  many demos. AppleWin and Virtual ][ both emulate it; AppleWin adds the
  SSI263 speech chip. *(infra: `snd.rs` already has a cycle-stamped audio
  path; the 6522 is a well-contained new device.)*
- **Super Serial Card** (M) — slot 2 serial. The killer feature is the
  **virtual modem**: bridge the SSC to a TCP socket so period terminal
  software can "dial" modern telnet BBSes. AppleWin and Virtual ][ do this.
- **AppleMouse card** (M) — unlocks MousePaint, Dazzle Draw menus, GEOS.
  Natural fit: we already have SDL mouse events in the frame loop.
- **RamWorks III aux-slot memory** (M) — banked aux memory beyond 128K (up
  to 8–16MB in AppleWin/izapple2). AppleWorks and RAM-disk software love it.
  *(infra: `IouE` already owns the aux bank; RamWorks generalizes it to N
  banks selected at `$C073`.)*
- **Z80 SoftCard → CP/M** (L) — a second CPU core (Z80) sharing the bus;
  boots CP/M, WordStar, Turbo Pascal. MAME/AppleWin/izapple2 have it. Big,
  self-contained, very fun.
- **Uthernet II / FujiNet** (L) — networking: a W5100-style ethernet card
  (AppleWin) or the FujiNet virtual network device (izapple2, fujinet-
  AppleWin fork). Unlocks Marinetti-era and modern network software.
- **No-Slot Clock** (S) — a phantom DS1216 clock accessed through ROM-space
  reads; some ProDOS setups expect it. We already ship a Thunderclock, so
  this is polish.
- **Cassette interface** (S/M) — `$C060`/`$C020` tape in/out against WAV
  files (Virtual ][ emulates this "for nostalgic reasons"). Pairs nicely
  with the Apple 1 side of EWM, too.
- **Printers** (M) — Virtual ][ emulates an ImageWriter II / Epson FX-80
  and renders the output to PDF; even PrintShop graphics work. An EWM
  version could render to PNG/PDF in the host filesystem.
- **VidHD ROM signature** (S) — izapple2/AppleWin implement just enough of
  VidHD for Total Replay to detect it and enable Super Hi-Res artwork.
  (Full SHR rendering is IIgs-sized; the signature stub is small.)

## Machine models

- **NMOS //e (unenhanced)** (M) — 6502 + the original //e ROMs; some early
  software behaves differently. Explicitly out of scope during the //e
  project; the machinery (ROM sets, `TwoType`) is all in place.
- **Original Apple ][ (Integer BASIC)** (M) — needs the Integer ROM set and
  the 13-sector boot ROM (see Storage below); completes the Woz lineage
  story the README tells.
- **Apple //c** (L) — MMU/IOU cousins of the //e work, built-in drives,
  mouse, serial. The //e plan's "future work".
- **PAL variants / 50 Hz** (S/M) — a `--region pal` that adjusts the frame
  loop's cycle budget and vertical timing; matters for European software
  timing loops.

## Storage & disk images

- **Disk II write support + write-back** (M) — currently writes only touch
  the in-memory nibble stream (REWRITE quirk #2). Real write support (nibble
  writes for `.dsk`/`.nib`, then saving back to the image file, dirty-flag
  in the status bar) makes the emulator usable for actually *working* in
  AppleWorks/DOS. WOZ stays read-only (spec discourages WOZ1 writes).
- **WOZ 2.0** (M) — variable-size TRKS, optimal bit timing field, FLUX
  chunk, `requires_machine` metadata. The parser already detects and rejects
  `WOZ2` cleanly; the bit engine is container-agnostic. *(infra: `woz.rs`.)*
- **13-sector boot ROM (P5)** (S/M) — a `--boot13` option or automatic
  selection, unlocking DOS 3.2 disks (`DOS 3.2 System Master.woz` is already
  in the test set, documented out of scope in `WOZ1.md`).
- **Remaining WOZ stragglers** (M?) — Stargate (`I/O ERROR` after boot) and
  First Math Adventures (`BRK` at `$A853`) still defeat the bit engine; the
  investigation notes live in `WOZ1.md`'s compatibility table.
- **`.2mg` and compressed images** (S) — 2IMG headers for ProDOS images;
  transparently reading `.gz`/`.zip` images the way AppleWin does.
- **Disk image tools** (M) — a `ewm disk catalog <image>` subcommand that
  lists DOS 3.3/ProDOS files without booting; maybe extract/insert files.
  Great for tests and scripting, all headless.

## Video & display

- **NTSC artifact color** (M/L) — a shader-quality composite model for HGR
  (and the DHGR **sliding-window** color already prototyped on the
  `iie/experiment-dhgr-color` branch — see `notes/DHGR_COLOR_EXPERIMENT.md`
  for its promotion checklist). AppleWin's NTSC mode is the reference.
- **Monitor styles** (S) — green / amber / white monochrome and "RGB
  monitor" color as runtime choices instead of the boot-time `--color` flag.
  *(infra: `Scr::set_color_scheme` already exists; this is palette wiring +
  a couple of palettes.)*
- **Scanlines / CRT flavor** (S/M) — optional scanline dimming or a simple
  CRT bloom for the 3× window; every mainstream emulator offers some.
- **Video recording** (M) — capture the frame buffer to animated GIF or
  MP4; pairs with the deterministic emulation for perfectly reproducible
  demo captures. *(infra: `encode_bmp` + the `--screenshot` plumbing.)*
- **Screenshot polish** (S) — promote the hidden `--screenshot` flag to a
  palette command ("Save Screenshot") writing PNG with a timestamped name.

## Audio

- **Speaker filtering & volume** (S) — a simple low-pass over the 1-bit
  speaker stream and a volume control (palette entries); reduces the harsh
  edge on click-heavy software.
- **Tape-out as audio** (S) — see the cassette interface above; recording
  `SAVE` output to WAV doubles as a fun preservation trick.
- *(Mockingboard/SSI263 belong here too — listed under Hardware.)*

## Debugging tools

The REWRITE explicitly lists "a debugger" as never-implemented future work,
and AppleWin's symbolic debugger is the genre benchmark.

- **Interactive debugger** (L) — pause-and-inspect: breakpoints (PC, memory
  read/write watchpoints, soft-switch access), single-step / step-over, and
  a disassembly view. *(infra: `ewm-core/fmt.rs` already disassembles; the
  CPU is a clean step()-driven core; the palette overlay proves we can draw
  UI panels over the emulated screen.)*
- **Memory viewer/editor** (M) — hex view of main/aux/LC banks with live
  updates; AppleWin-style mini-views for the text/hires pages.
- **Soft-switch state panel** (S) — we already track every //e switch in
  `IouE`; a debug overlay showing TEXT/MIXED/PAGE2/HIRES/80COL/ALTCHARSET/
  RAMRD/RAMWRT/ALTZP/80STORE/DHIRES and the LC state would have shortened
  several of our own investigations.
- **Symbol tables** (M) — load Merlin/LISA/`.sym` labels; ship built-in
  symbols for the Monitor, DOS 3.3 and ProDOS entry points (AppleWin does).
- **Real measured MHz + cycle counters** (S) — replace the fake status-bar
  MHz (REWRITE quirk #3) with the real measured rate; add a user cycle
  counter (reset/read) for timing work.
- **Trace improvements** (S/M) — `--trace` exists; add address-range
  filters, soft-switch access logging (we hand-rolled this repeatedly during
  the //e work), and trace-to-ring-buffer with a palette "dump last 10k".
- **Disk activity inspector** (M) — current track/sector/bit position, head
  movement log, nibble stream tap. Would have cut the WOZ protection
  debugging time in half. *(infra: `Dsk::half_track()` was the seed.)*

## Command palette

Today: Reset, Pause, Full Screen, three CPU speeds. Cheap, high-value adds:

- **Disk operations** (S) — insert/eject/swap disks per drive with a file
  picker; "Swap drive 1 ↔ 2" (the classic two-disk-game convenience);
  show the mounted image names.
- **Save Screenshot** (S) — see Video above.
- **Monitor style & color scheme** (S) — runtime toggle instead of
  `--color`.
- **Save/Load State** (see next section) as palette entries.
- **Copy screen text** (S) — `text_screen()`/`text_screen_80()` already
  produce exactly what "Copy Text Screen" should put on the clipboard.
- **Paste text** (S/M) — feed clipboard text through the keyboard latch
  (respecting the strobe), like AppleWin/Virtual ][ paste; makes typing
  BASIC listings from the web painless.
- **Machine info** (S) — a read-only panel: model, ROM hashes, mounted
  images, switch states.

## Save states & rewind

- **Save/load state** (L) — full machine snapshots (CPU, RAM/aux, devices,
  disk position). AppleWin's `.aws.yaml` is the reference; EWM's flat,
  `Rc`-free ownership model makes serialization unusually tractable.
  Versioned format + a golden round-trip test.
- **Rewind** (M, after save states) — Apple2TS's "go back in time" feature:
  a ring buffer of periodic snapshots with a palette "Rewind 5s". EWM's
  determinism makes replay-from-snapshot exact.
- **Boot snapshots** (S, after save states) — `--resume <state>` to skip
  the ~10s DOS boot during development and testing.

## Automation & scripting

- **Lua scripting via `mlua`** (M/L) — the original C EWM had Lua hooks
  (dropped in the Rust port, listed as REWRITE future work): per-instruction
  and soft-switch callbacks, machine control from scripts. Virtual ][ ships
  AppleScript automation for the same reasons — scripted dev environments.
- **Expect-style headless driver** (S/M) — our own tests already do
  "boot, wait for text, type line, assert screen" (`two_dos.rs`,
  `two_woz.rs`). Exposing that as `ewm two --script <file>` (type/wait/
  screenshot/assert commands) would make it a user feature and a great
  CI tool for *other people's* Apple II software.
- **Control socket / REPL** (M) — a simple line protocol (read/write
  memory, key injection, screenshot) for driving a windowed instance from
  outside; the automation counterpart of the debugger.

## References

- [AppleWin](https://github.com/AppleWin/AppleWin) — debugger, save states,
  NTSC/RGB video, Mockingboard/Phasor/SAM + SSI263, Uthernet I/II, RamWorks,
  VidHD, CP/M SoftCard.
- [Virtual ][](https://www.virtualii.com/) — ImageWriter/Epson printing to
  PDF, cassette tape, AppleScript scripting, mouse, Z80, configurable
  machines.
- [MAME Apple II driver](https://wiki.mamedev.org/index.php/Driver:Apple_II)
  — breadth of peripheral cards, Z80 SoftCard, accuracy reference.
- [izapple2](https://github.com/ivanizag/izapple2) — FujiNet, RamWorks,
  VidHD signature, Z80, portable Go design.
- [Apple2TS](https://apple2ts.com) — rewind ("go back in time"), on-demand
  state saves, RamWorks.
- EWM's own future-work lists: `REWRITE.md` (Lua, disk writes, debugger,
  real MHz), `APPLE_IIE_ENHANCED.md` (//c, NMOS //e), `WOZ1.md` (WOZ 2.0,
  13-sector, protection stragglers), `DHGR_COLOR_EXPERIMENT.md` (sliding
  window promotion).
