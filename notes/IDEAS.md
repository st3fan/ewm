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
- ~~**RamWorks III aux-slot memory**~~ — **landed**: `--aux
  ramworksiii[:SIZE]` (64K..8MB, bank register `$C073`), built on a new
  `AuxCard` trait (`ewm/src/aux/`) that also brought the plain 1K
  80-Column Text Card; the aux slot is now swappable per card file.
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
- ~~**Monitor styles**~~ — **landed**: green / amber / white monochrome and
  color via `--color [green|amber|white|rgb]` and the palette's
  "Monitor Style" choice submenu.
- ~~**Scanlines**~~ — **landed**: `--scanlines [off|light|heavy]` + the
  palette's "Scanlines" choice submenu (a multiply-blend overlay). A real
  **CRT bloom** stays open — it needs shader/render-target work.
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

## Input

- ~~**//e paddle timers**~~ — **landed**: the `$C070`/`$C064`-`$C065`
  timer model ported from `TwoIo` into `IouE`, parity-tested against the
  ][+ (found the moment a real Xbox pad met Wings of Fury on the //e).
- **Gamepad support polish** (M) — *partially landed*: **hot-plug**
  (Bluetooth pads auto-connect on appearance, auto-fallback on disconnect)
  and the **palette "Controller" picker** (choice submenu, active pad ✓,
  button events filtered to the active pad) are done, and the **D-pad**
  maps onto the joystick axes (full deflection, wins over the stick).
  Still open: custom button mapping, per-axis deadzone/calibration.
- **Keyboard-as-joystick** (S) — arrows/WASD emulating the stick with
  configurable throw, for laptops without a controller.
- **Mouse-as-paddles** (S) — map mouse position to the paddle timers, the
  classic way to play paddle games (and how a Koala Pad could be faked).

## Debugging tools

The REWRITE explicitly lists "a debugger" as never-implemented future work,
and AppleWin's symbolic debugger is the genre benchmark. **Planned**: the
minimal always-present slice of this section — WozBug, CPU breakpoints,
device-state commands, built-in symbols — has a plan in
`notes/DEBUGGING_TOOLS.md`; the bullets below remain the long-range
backlog beyond it.

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
- **Trace improvements** (S/M) — CPU tracing exists (`debug.trace` /
  `--set debug:trace=…`); add address-range
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
  the startup setting.
- **Save/Load State** (see next section) as palette entries.
- **Copy screen text** (S) — `text_screen()`/`text_screen_80()` already
  produce exactly what "Copy Text Screen" should put on the clipboard.
- **Paste text** (S/M) — feed clipboard text through the keyboard latch
  (respecting the strobe), like AppleWin/Virtual ][ paste; makes typing
  BASIC listings from the web painless.
- **Machine info** (S) — a read-only panel: model, ROM hashes, mounted
  images, switch states.
- **Game cheats** (M) — a "Cheats" palette submenu: pick "Hard Hat Mack —
  more lives" and it pokes the right zero-page/memory locations (or patches
  code) on the running machine. Backed by a small cheat database: per-title
  TOML files (name, description, pokes `addr = value`, optional code
  patches), matched to the mounted image by name or content hash, shipped
  in-repo and user-extendable in the disk library. The 4am write-ups
  document many of these addresses. *(infra: the bus makes pokes trivial;
  the palette's per-open command registration means cheats can appear only
  when a matching disk is mounted.)*

## Configuration & launch experience

The north star: **start `ewm` with no command-line options and fully use it
from there** — boot to the menu, configure a machine, insert disks, play.
Everything in this section serves that goal.

- **Machine config files** (M) — **landed**: see `notes/JSON_CONFIG.md`
  — `ewm two --config myiie.json` with the committed
  `schema/ewm-config.schema.json` covering every CLI and palette
  setting; real slot flexibility (multiple Disk ][ controllers, empty
  slots, cards in any slot); and the full source surface from
  `plans/20260718-02-config-sources.md`: built-ins
  (`--config builtin:2plus`), partial `--config-overlay` layers,
  `--set`, `--print-config`. Remaining: "save current setup" from the
  palette (JSON_CONFIG Phase C; the Options→Config mapping already
  exists). Virtual ]['s configurable virtual machines remain the
  reference.
- **Disk management** (M) — the full story, not just palette entries:
  start with **no disks mounted**, then insert/eject/swap floppies and
  mount HD volumes on the HDD card at runtime from the Command Palette
  (file picker + recently-used list). Drive doors and image names in the
  status bar. Ejecting flushes (once write support lands).
- **A disk library** (S/M) — a well-known images directory the pickers
  default to (`~/Library/Application Support/EWM/Disks` on macOS,
  `$XDG_DATA_HOME/ewm/disks` elsewhere; the repo's `disks/` as a dev
  fallback), with recently-used tracking. Where downloads (next item) land.
- **Fetch from the Internet Archive** (M/L) — archive.org has a documented
  JSON API (advanced search + per-item metadata endpoints, direct file
  downloads), and the 4am collections ("Apple II Library: The 4am
  Collection", the woz-a-day series) are exactly the WOZ images our bit
  engine was built for. A palette flow — search archive.org, browse the 4am
  collection, download into the disk library, insert into drive 1 — would
  make EWM self-sufficient for software. (Respect rate limits; cache
  downloads; surface item metadata/credits.)
- **Zero-flag startup** (theme) — with config files + disk management +
  the library in place, the CLI becomes optional: `ewm` → boo menu →
  pick/configure machine → insert disks from the palette. Tracks the
  gaps blocking that flow.

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
- **Quit & resume** (S, after save states) — persist across emulator
  restarts: auto-save the machine state on exit (opt-in, per machine
  config), restore it on the next launch — quit mid-game tonight, continue
  exactly there tomorrow. Needs care with mounted-image paths moving
  between runs.

## Automation & scripting

- **Embedded scripting** (M/L) — the original C EWM had Lua hooks
  (dropped in the Rust port, listed as REWRITE future work): per-instruction
  and soft-switch callbacks, machine control from scripts. Virtual ][ ships
  AppleScript automation for the same reasons. Candidate runtimes below —
  the owner is open to Lua but prefers something more static (TinyGo-ish).

### Embedded scripting: runtime options

Whatever the language, the real design work is the **hook API**, and it is
shared across all options: machine control (boot/reset/insert/key/screen),
memory peek/poke, soft-switch and I/O-access callbacks, and per-instruction
hooks (perf-sensitive — see the note below the table).

| Option | Crate | Typing | Notes |
|---|---|---|---|
| **Lua 5.4** | `mlua` | dynamic | The incumbent — closest to the original C EWM scripts. Mature bindings, tiny runtime, instant iteration (edit + rerun). |
| **Luau** | `mlua` (feature flag) | **gradual** | Roblox's typed Lua: type annotations + a real type checker, sandboxed by design, faster than stock Lua. The strongest "Lua, but more static" answer — same `mlua` API either way, so this choice can even be deferred. |
| **Rhai** | `rhai` | dynamic | Pure-Rust embedded language built exactly for this; zero C deps, painless `#[derive]`-style API binding, no `unsafe`. Weakest typing story. |
| **Rune** | `rune` | dynamic | Pure-Rust, Rust-flavored syntax, async-friendly. Similar trade to Rhai with nicer syntax, smaller ecosystem. |
| **Starlark** | `starlark` (Meta's) | dynamic, **hermetic** | Python-ish and *deterministic by construction* (no ambient I/O, reproducible) — a striking match for EWM's deterministic-test culture. Optional type annotations in the Meta implementation. |
| **WASM plugins** | `wasmtime` | **static — any language** | The TinyGo answer: EWM embeds a WASM runtime and defines a host API; scripts are compiled `.wasm` written in **TinyGo**, Rust, Zig, C, AssemblyScript… Real type systems and toolchains, strong sandboxing, near-native hook performance. Cost: a compile step per iteration (no REPL feel) and the host-API/ABI design (WIT or hand-rolled hostcalls), plus the heaviest runtime dependency of the table. |
| **TypeScript / JS** | `rustyscript` (deno_core) or `rquickjs` | static *authoring* (TS) | Typed developer experience, huge ecosystem. deno_core is a heavy dependency; QuickJS is light but plain JS. |
| **Gluon** | `gluon` | **static** (ML-family) | A genuinely statically-typed embeddable language — but a niche syntax and quiet maintenance make it a risky bet. |

Notes that cut across all of them:

- **Per-instruction hooks are the perf cliff.** At ~1M instructions/second,
  calling into *any* scripting runtime per instruction hurts (WASM least,
  interpreted Lua most). Mitigate in the host API: hooks register address
  ranges / soft-switch filters and the emulator only calls out on matches —
  the debugger's breakpoint machinery and this filter can be the same code.
- **Determinism:** scripts become part of a run's behavior; hermetic
  runtimes (Starlark, WASM without WASI) keep scripted runs reproducible
  and CI-able, which is very EWM.
- **They compose:** a plausible endgame is *two* tiers sharing one hook
  API — Luau (or Rhai) for interactive poking, WASM for serious typed
  plugins (a TinyGo-written debugger extension, a protection analyzer).
- **Recommendation shape:** if the old Lua feel matters, **Luau via
  `mlua`** is the best of both (types + Lua compatibility + one crate). If
  the static preference wins, **`wasmtime` + TinyGo/Rust plugins** — and
  since the MCP server / control socket / expect-driver (above) already
  demand the same host API, building that API first keeps every option
  open.
- **Expect-style headless driver** (S/M) — our own tests already do
  "boot, wait for text, type line, assert screen" (`two_dos.rs`,
  `two_woz.rs`). Exposing that as `ewm two --script <file>` (type/wait/
  screenshot/assert commands) would make it a user feature and a great
  CI tool for *other people's* Apple II software.
- **Control socket / REPL** (M) — a simple line protocol (read/write
  memory, key injection, screenshot) for driving a windowed instance from
  outside; the automation counterpart of the debugger.

## Distribution & platforms

- **Native macOS app** (M/L) — **planned**: see `notes/MAC_APP.md` for the
  four-phase plan (self-contained `EWM.app` with file associations →
  signing/notarization → `.ewmachine` machine documents → a VMware-style
  library app). Virtual ][ is the native-feel benchmark.
- **Raspberry Pi boot image** (L) — flash a card, power on, and the Pi *is*
  an Apple ][+ / //e: a minimal Linux (or bare KMS/DRM console) image that
  boots straight into the boo menu, from which you pick the machine — the
  bootloader's original purpose, finally literal. Needs SDL on KMS without
  X, GPIO-friendly shutdown, and the disk library on the card. (Appliance
  cousins: RetroPie ships emulators this way.)
- **Hosted / virtual Apple //e** (L) — run the emulator server-side and use
  it from a browser: boot instances from a small web UI, interact via VNC
  or a web VNC client (noVNC). The headless core already runs without SDL;
  the missing piece is a frame-buffer → RFB/WebSocket bridge plus keyboard
  injection (which the control socket above provides). "Virtual Apple //e
  hosting" — shareable, embeddable, demo-able.
- **WebAssembly build — EWM in the browser** (L) — compile the emulator to
  WASM and run it entirely client-side: a canvas for the 560-wide frame,
  Web Audio for the speaker, keyboard/gamepad events in, no server at all.
  The cleanest "try EWM in one click", an embeddable widget for blog posts
  ("here, *play* the bug I'm describing"), the no-install path on mobile
  Safari, and the zero-ops sibling of the hosted instances above. *(infra:
  the `ewm-core` / frontend crate split already isolates SDL; the pure
  `Scr` renderer and headless machines are WASM-ready as-is — the work is
  a `wasm32` frontend crate and asset/file plumbing.)*

## AI

The headless core is unusually AI-friendly: deterministic execution,
`text_screen()`/`text_screen_80()` as ground-truth screen text (no OCR),
keyboard injection, and save states (once landed) as perfect episode resets.

- **MCP server** (M) — expose EWM to AI agents as tools: boot a machine,
  insert a disk, type, wait-for-text, read the screen, screenshot, poke/peek
  memory. Everything our own integration tests do, packaged for Claude and
  friends — "load Hard Hat Mack and tell me how the protection works", or an
  agent that plays Planetfall. *(infra: the expect-style driver and control
  socket under Automation are 90% of this; MCP is a thin protocol layer.)*
- **AI debugging assistant** (M) — feed the trace ring buffer, disassembly,
  and soft-switch log to an LLM to explain what the machine is doing —
  literally how the E7 protection and the //e lowercase bug were cracked
  during EWM's own development, productized.
- **Cheat finder** (M) — diff memory snapshots across "lost a life" events
  to locate lives/score counters automatically, then emit a cheat-file entry
  (see Game cheats). Classic Cheat-Engine flow; determinism plus snapshots
  make it reliable, and an LLM can name/describe what it found.
- **BASIC copilot** (S/M) — prompt → AppleSoft BASIC → auto-pasted through
  the keyboard latch and run; the screen scrape closes the loop for
  self-correction. A lovely demo of paste + scripting + AI in one.
- **RL / game-playing harness** (M/L) — frame buffer + joystick as a gym
  environment; deterministic stepping and snapshot resets are exactly what
  RL wants and real hardware can't give.

## Cloud & web

Extends "Hosted / virtual Apple //e" under Distribution; the client-side,
no-server counterpart is the **WebAssembly build** there.

- **Cloud disk library & save-state sync** (M) — the disk library and
  quit-&-resume states in S3/iCloud; start a game on the desk, resume it
  anywhere. Pairs with the Archive.org fetcher.
- **Shareable replays** (M) — determinism means a session is fully defined
  by (config, images, timestamped inputs): record tiny replay files,
  share a link, the server re-executes into video or a live view. Demo
  captures, bug reports, and speedruns all fall out of one feature.
- **Spectator mode / pass-the-controller** (M/L) — one instance, one
  driver, N viewers over the RFB/WebSocket fan-out; hand the keyboard to
  another viewer. Retro game night, remote demos, teaching.
- **Emulation API service** (M) — the control socket as a hosted REST/WS
  API: POST a disk image, drive it, get screenshots/screen text back.
  CI-for-Apple-II-software as a service; the expect-style driver is the
  local version of the same contract.

## Mobile

- **iPad / iPhone app** (L) — App Store rules now allow retro emulators
  (2024's policy change; Delta et al.). Touch keyboard overlay with the
  //e layout, tap-to-insert disks from the Files app / disk library,
  MFi + Bluetooth gamepads, external keyboards. The SDL frontend would be
  replaced by a small native shell over `ewm-core` — the crate seam again.
- **Touch & motion controls** (M, after the app) — drag = paddle, virtual
  stick overlay, tilt-as-joystick for Choplifter.
- **Handoff** (S, after cloud sync) — quit on the Mac, resume the same
  state on the iPad via the synced save states.
- *(The WASM build is the no-install mobile fallback: EWM in mobile
  Safari with a touch keyboard, zero App Store involvement.)*

## Apple Watch (yes, really)

Not the emulator *on* the watch — the watch as the world's smallest Apple II
peripheral, talking to a Mac/hosted instance:

- **Digital Crown = paddle** (M) — the crown is a rotary encoder and Apple
  paddles were rotary knobs; this is the most faithful paddle controller
  Apple has shipped since 1983. Breakout on a //e, controlled from the
  wrist. *(infra: paddle injection already exists via `set_joystick`; this
  is a WebSocket bridge + a tiny watch app.)*
- **Status complication** (S, after hosted instances) — your cloud //e's
  drive light and MHz on a watch face; tap to peek at the text screen
  (40×24 fits a watch better than most modern UIs).
- **Wrist notifications** (S) — the speaker BELL or a finished long-running
  BASIC program pings your wrist: "your //e wants attention."

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
