//! The Apple 1 / Replica 1: machine and SDL frontend, port of `one.c` —
//! which, like this file, held both `ewm_one_t` and the SDL loop. The
//! machine composes its hardware as memory regions (RAM, ROM, PIA) and owns
//! the CPU; the frame structure of the loop is the C one: event pump →
//! burst of CPU cycles → tty render.

use crate::palette::{self, Palette, PaletteAction, PaletteKey};
use crate::pia::{A1_PIA6820_ADDR, A1_PIA6820_LENGTH, Pia};
use crate::scr::PixelLayout;
use crate::sdl;
use crate::tty::{TTY_PIXEL_HEIGHT, TTY_PIXEL_WIDTH, Tty};
use ewm_core::cpu::{Cpu, Model};
use ewm_core::mem::{DeviceHandle, Memory};
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Mod};
use sdl3::pixels::PixelFormat;
use sdl3::rect::Rect;
use sdl3::render::ScaleMode;
use sdl3::sys::render::SDL_RendererLogicalPresentation;
use sdl3::video::FullscreenType;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OneModel {
    Apple1,
    Replica1,
}

impl OneModel {
    /// The model's schema token — also the name of the built-in config
    /// that *is* the model's board description.
    pub fn token(self) -> &'static str {
        match self {
            OneModel::Apple1 => "apple1",
            OneModel::Replica1 => "replica1",
        }
    }
}

pub struct One {
    pub model: OneModel,
    pub cpu: Cpu,
    pia: DeviceHandle<Pia>,
}

impl One {
    /// The model's stock machine: built from its built-in config — the
    /// single description of the board (R3 of
    /// plans/20260719-03-one-machine-components.md; layouts in
    /// notes/APPLE1.md).
    pub fn new(model: OneModel) -> One {
        let config =
            crate::config::load_builtin(model.token()).expect("builtins are pinned valid by test");
        let machine = config.machine.expect("builtins are complete");
        One::from_components(model, machine.cpu, &machine.memory)
            .expect("builtin boards are pinned buildable by test")
    }

    /// Build a machine from its components: the CPU (`None` = the model's)
    /// and the memory regions — RAM banks, RAM/ROM images — that describe
    /// the whole board. The only fixed hardware is the PIA at
    /// $D010-$D013; `model` also decides the terminal behavior (the
    /// Apple 1 masks display output to 7 bits).
    pub fn from_components(
        model: OneModel,
        cpu: Option<crate::config::CpuModel>,
        regions: &[crate::config::MemoryRegion],
    ) -> Result<One, String> {
        let cpu_model = match cpu {
            Some(crate::config::CpuModel::M6502) => Model::M6502,
            Some(crate::config::CpuModel::M65C02) => Model::M65C02,
            None => match model {
                OneModel::Apple1 => Model::M6502,
                OneModel::Replica1 => Model::M65C02,
            },
        };
        // No base RAM: every byte of the board comes from the regions.
        let mut mem = Memory::new(0);
        for region in regions {
            let address = region
                .address_value()
                .map_err(|e| format!("machine.memory: {e}"))?;
            let (rom, data) = match (&region.path, &region.size) {
                (Some(path), None) => {
                    let data = crate::config::read_memory_image(path)?;
                    (region.kind == crate::config::MemoryKind::Rom, data)
                }
                (None, Some(size)) => {
                    let bytes = crate::config::parse_memory_size(size)
                        .map_err(|e| format!("machine.memory: {e}"))?;
                    (false, vec![0; bytes as usize])
                }
                _ => {
                    return Err("machine.memory: a region takes exactly one of path or size".into());
                }
            };
            if rom {
                mem.add_rom(address, data);
            } else {
                mem.add_ram(address, data);
            }
        }
        let pia = mem.add_device(
            A1_PIA6820_ADDR,
            A1_PIA6820_ADDR + A1_PIA6820_LENGTH - 1,
            Pia::new(),
        );
        Ok(One {
            model,
            cpu: Cpu::new(cpu_model, mem),
            pia,
        })
    }

    /// Port of `ewm_one_keydown`: latch the key into the PIA with bit 7 set
    /// and raise IRQA1.
    pub fn key(&mut self, key: u8) {
        let pia = self.cpu.mem.device_mut(self.pia);
        pia.set_ina(key | 0x80);
        pia.set_irqa1();
    }

    /// A previously injected key is still waiting in the PIA's one-byte
    /// latch — hold the next byte until this clears (the tty frontend's
    /// pacing; the SDL frontend relies on human typing speed instead).
    pub fn key_pending(&mut self) -> bool {
        self.cpu.mem.device_mut(self.pia).key_pending()
    }

    /// Bytes the machine wrote to the display since the last drain — the
    /// same stream `ewm_one_pia_callback` fed into the tty, including its
    /// model check: the Apple 1 masks display output to 7 bits.
    pub fn drain_display(&mut self) -> Vec<u8> {
        let model = self.model;
        self.cpu
            .mem
            .device_mut(self.pia)
            .drain_out()
            .into_iter()
            .map(|(_ddr, v)| {
                if model == OneModel::Apple1 {
                    v & 0x7f
                } else {
                    v
                }
            })
            .collect()
    }

    /// Add an extra RAM region (config `machine.memory`). Like the C
    /// linked list, regions added later are dispatched first — but base RAM
    /// wins, per the `addr < ram_size` fast path in mem.c.
    pub fn add_ram(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_ram(start, data);
    }

    /// Add an extra ROM region (config `machine.memory`).
    pub fn add_rom(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_rom(start, data);
    }
}

// --- SDL frontend, the loop half of one.c ---

const ONE_FPS: u32 = 40;
const ONE_CPS: u32 = 1_023_000;

/// What palette command callbacks get to work with: the machine plus the
/// frontend state the commands mutate.
struct OneCtx<'a> {
    one: &'a mut One,
    tty: &'a mut Tty,
    paused: &'a mut bool,
    window: &'a mut sdl3::video::Window,
}

type OneAction = fn(&mut OneCtx);

#[derive(Debug, PartialEq)]
pub(crate) struct Options {
    model: OneModel,
    /// The window title's machine name (config `title`): `EWM - <title>`,
    /// or plain `EWM` when None. Comes from the document only — a bare
    /// `ewm one` stays `EWM` even though `normalize` fills the board.
    title: Option<String>,
    /// The CPU (`machine.cpu`); filled from the model's builtin by
    /// `normalize` when the document doesn't say.
    cpu: Option<crate::config::CpuModel>,
    /// The whole board's memory regions; filled from the model's builtin
    /// by `normalize` when the document doesn't say.
    memory: Vec<crate::config::MemoryRegion>,
    trace_path: Option<String>,
    strict: bool,
    /// Headless: stdin/stdout is the keyboard and display (`--tty`).
    tty: bool,
    /// Text file printed to the session before the machine boots
    /// (`--tty-banner`) — instructions for telnet visitors.
    tty_banner: Option<String>,
    /// Proactively negotiate telnet (server echo, char-at-a-time) on
    /// connect (`--tty-telnet`, implies `--tty`) — so a telnet client
    /// stops echoing locally and lines appear once. Bare `--tty` stays
    /// byte-clean for local terminals and `nc`.
    tty_telnet: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            // The C default model is the Replica 1.
            model: OneModel::Replica1,
            title: None,
            cpu: None,
            memory: Vec::new(),
            trace_path: None,
            strict: false,
            tty: false,
            tty_banner: None,
            tty_telnet: false,
        }
    }
}

/// Fill the component fields from the model's built-in config when the
/// document didn't spell them — the builtin *is* the board description,
/// so bare `ewm one` and `--config builtin:replica1` build the same
/// machine, and `--print-config` always shows the full board.
fn normalize(options: &mut Options) {
    if options.cpu.is_some() && !options.memory.is_empty() {
        return;
    }
    let builtin = crate::config::load_builtin(options.model.token())
        .expect("builtins are pinned valid by test");
    let machine = builtin.machine.expect("builtins are complete");
    if options.cpu.is_none() {
        options.cpu = machine.cpu;
    }
    if options.memory.is_empty() {
        options.memory = machine.memory;
    }
}

fn usage() {
    eprintln!("Usage: ewm one [options]");
    eprintln!("  --config <source> configure the machine from a JSON file or a built-in");
    eprintln!("                    config (builtin:apple1, builtin:replica1; builtin:list");
    eprintln!("                    lists them); at most one, the base of the document");
    eprintln!("  --config-overlay <source>  layer a partial config on top; repeatable,");
    eprintln!("                    applied in order with --config and --set");
    eprintln!("  --set <key>=<val> override one config value; files and sets layer in order");
    eprintln!("                    (e.g. --set cpu:strict=true)");
    eprintln!("  --print-config    print the machine the command line describes (sources");
    eprintln!("                    plus flags) as config JSON and exit");
    eprintln!("  --tty             headless: the terminal (stdin/stdout) is the keyboard");
    eprintln!("                    and display; Meta-R resets, EOF ends the session");
    eprintln!("  --tty-banner <path>  text file printed to the session before the machine");
    eprintln!("                    boots (instructions for telnet visitors)");
    eprintln!("  --tty-telnet      negotiate telnet on connect so a telnet client stops");
    eprintln!("                    echoing locally (implies --tty; for the systemd units)");
}

/// Seed `Options` from the layered config document (pass 1 of
/// `parse_options`). `config::from_document` validated it — structurally,
/// for completeness, and against the one-family key table — so what is
/// left is the model boundary and the straight field mapping.
fn apply_config(options: &mut Options, config: crate::config::Config) -> Result<(), String> {
    if config.title.is_some() {
        options.title = config.title.clone();
    }
    let machine = config
        .machine
        .expect("from_document guarantees a machine section");
    let model = machine
        .model
        .expect("from_document guarantees machine.model");
    // A two-family document is a valid *config* but not a `one` machine —
    // the mirror of two's cross-subcommand check.
    options.model = match model {
        crate::config::Model::Apple1 => OneModel::Apple1,
        crate::config::Model::Replica1 => OneModel::Replica1,
        other => {
            return Err(format!(
                "machine.model: {:?} is an `ewm two` machine (run: ewm two --config …)",
                other.token()
            ));
        }
    };
    options.cpu = machine.cpu;
    // One-family memory regions describe the *whole board*; an absent
    // (or empty) list means the model's built-in board, filled in by
    // `normalize` after the document is applied.
    options.memory = machine.memory;
    if let Some(strict) = config.cpu.strict {
        options.strict = strict;
    }
    if config.debug.trace.is_some() {
        options.trace_path = config.debug.trace;
    }
    Ok(())
}

/// Serialize `Options` back into a `Config` — the inverse of
/// `apply_config`, the one-family sibling of `two::options_to_config`.
/// Used by `--print-config`.
fn options_to_config(options: &Options) -> crate::config::Config {
    crate::config::Config {
        schema: Some(
            "https://raw.githubusercontent.com/st3fan/ewm/main/schema/ewm-config.schema.json"
                .to_string(),
        ),
        description: None,
        title: options.title.clone(),
        machine: Some(crate::config::Machine {
            model: Some(match options.model {
                OneModel::Apple1 => crate::config::Model::Apple1,
                OneModel::Replica1 => crate::config::Model::Replica1,
            }),
            cpu: options.cpu,
            aux: None,
            slots: None,
            memory: options.memory.clone(),
        }),
        display: crate::config::Display::default(),
        cpu: crate::config::Cpu {
            speed: None,
            strict: options.strict.then_some(true),
        },
        input: crate::config::Input::default(),
        boot: crate::config::Boot::default(),
        debug: crate::config::Debug {
            trace: options.trace_path.clone(),
            enabled: None,
        },
        remote: crate::config::Remote::default(),
        state: crate::config::State::default(),
    }
}

pub(crate) fn parse_options(args: &[String]) -> Result<Options, i32> {
    let mut options = Options::default();
    // Pass 1: the config document — the same sources, order rules, and
    // built-ins as `ewm two` — seeds the options; anything given
    // explicitly in pass 2 overrides the document.
    let doc = match crate::config::collect_document(args, "replica1", false) {
        crate::config::Collected::Document(doc) => doc,
        crate::config::Collected::Listed => return Err(0),
        crate::config::Collected::Failed => return Err(1),
        crate::config::Collected::MissingValue => {
            usage();
            return Err(1);
        }
    };
    if let Some(doc) = doc
        && let Err(e) =
            crate::config::from_document(doc).and_then(|c| apply_config(&mut options, c))
    {
        eprintln!("{e}");
        return Err(1);
    }
    // Fill anything the document left unsaid from the model's builtin,
    // so the built machine and --print-config always show the full board.
    normalize(&mut options);
    let mut print_config = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" => {
                usage();
                return Err(0);
            }
            "--config" | "--config-overlay" | "--set" => {
                // Applied in pass 1.
                it.next();
            }
            "--print-config" => print_config = true,
            "--tty" => options.tty = true,
            "--tty-telnet" => {
                options.tty = true;
                options.tty_telnet = true;
            }
            "--tty-banner" => match it.next() {
                Some(path) => options.tty_banner = Some(path.clone()),
                None => {
                    usage();
                    return Err(1);
                }
            },
            _ => {
                usage();
                return Err(1);
            }
        }
    }
    if print_config {
        // "What machine did I just describe?" — same contract as two's.
        let config = options_to_config(&options);
        let mut doc = serde_json::to_value(&config).expect("options serialize as a config");
        crate::config::compact_document(&mut doc);
        println!(
            "{}",
            serde_json::to_string_pretty(&doc).expect("document prints")
        );
        return Err(0);
    }
    Ok(options)
}

/// Build the machine `parse_options` described — the components straight
/// into `One::from_components`, then strict/trace — the machine half of
/// `main`, shared with the boot-gate tests.
fn build_machine(options: &Options) -> Result<One, String> {
    let mut one = One::from_components(options.model, options.cpu, &options.memory)?;
    one.cpu.strict = options.strict;
    if let Some(path) = &options.trace_path {
        let file = std::fs::File::create(path)
            .map_err(|e| format!("Cannot open trace file {path}: {e}"))?;
        one.cpu.trace = Some(Box::new(std::io::BufWriter::new(file)));
    }
    Ok(one)
}

// --- tty frontend (plans/20260719-04-apple1-telnet.md T1) ---
//
// The Apple 1 is a terminal machine — one PIA, bytes in, bytes out — so
// a byte stream *is* a faithful display. `--tty` runs headless with
// stdin/stdout as the keyboard and display: an Apple 1 in the local
// terminal, under `nc`, or per-connection under systemd socket
// activation (`StandardInput=socket` — the emulator does no networking).

/// Emulated cycles per tick: the loop steps the machine in fiftieths of
/// an emulated second.
const TTY_TICK_CYCLES: u64 = (ONE_CPS / 50) as u64;
/// Wall-clock length of a tick when throttling to 1.023 MHz.
const TTY_TICK: std::time::Duration = std::time::Duration::from_millis(20);
/// How long a received ESC waits for the `r` of Meta-R before it is
/// forwarded to the machine as the monitor's cancel-line key.
const TTY_ESC_WINDOW: std::time::Duration = std::time::Duration::from_millis(50);
/// Emulated cycles run after input EOF so the machine finishes printing
/// whatever the last command started (two emulated seconds).
const TTY_EOF_GRACE_CYCLES: u64 = 2 * ONE_CPS as u64;

// Telnet (RFC 854), the ~40-line subset (plan T2): raw telnet clients
// default to line mode with local echo, but the monitor wants
// character-at-a-time and echoes itself. The filter stays dormant until
// the first inbound IAC — nc and local terminals never see a byte of
// negotiation — then announces WILL ECHO + WILL SGA, strips and refuses
// everything else, and maps BREAK/Interrupt-Process (telnet's `send
// brk`, the serial-terminal "attention") to the RESET button.
const IAC: u8 = 255;
const IAC_SE: u8 = 240;
const IAC_BRK: u8 = 243;
const IAC_IP: u8 = 244;
const IAC_SB: u8 = 250;
const IAC_WILL: u8 = 251;
const IAC_WONT: u8 = 252;
const IAC_DO: u8 = 253;
const IAC_DONT: u8 = 254;
const OPT_ECHO: u8 = 1;
const OPT_SGA: u8 = 3;

/// The server's offer to drive the session character-at-a-time with
/// server-side echo: `WILL ECHO` + `WILL SUPPRESS-GO-AHEAD`. A telnet
/// client that honours it stops echoing locally — the machine's own
/// echo is then the only one, so lines appear once (not twice). Sent
/// proactively on connect in `--tty-telnet` (`--tty` alone stays
/// byte-clean for local terminals and `nc`); also the reactive reply
/// when a plain `--tty` client speaks telnet first.
const TELNET_ANNOUNCE: [u8; 6] = [IAC, IAC_WILL, OPT_ECHO, IAC, IAC_WILL, OPT_SGA];

/// What one inbound byte turned into, once telnet framing is peeled off.
enum TelnetOut {
    /// Nothing for the machine (protocol bytes, or swallowed).
    None,
    /// A key for the machine.
    Key(u8),
    /// BREAK / Interrupt Process: press the RESET button.
    Reset,
}

/// The inbound telnet state machine. Dormant (pure passthrough) until
/// the first IAC; `replies` collects protocol responses for the caller
/// to write raw.
#[derive(Default)]
struct TelnetFilter {
    active: bool,
    state: TelnetState,
}

#[derive(Default, PartialEq)]
enum TelnetState {
    #[default]
    Data,
    /// Seen IAC, waiting for the command byte.
    Command,
    /// Seen IAC WILL/WONT/DO/DONT, waiting for the option byte.
    Option(u8),
    /// Inside IAC SB … IAC SE subnegotiation.
    Subnegotiation,
    /// Seen IAC inside a subnegotiation (SE ends it).
    SubnegotiationCommand,
}

impl TelnetFilter {
    fn feed(&mut self, b: u8, replies: &mut Vec<u8>) -> TelnetOut {
        match self.state {
            TelnetState::Data => {
                if b != IAC {
                    return TelnetOut::Key(b);
                }
                if !self.active {
                    // First contact from a telnet client (plain --tty):
                    // negotiate character-at-a-time with remote echo.
                    // (--tty-telnet has already announced proactively and
                    // set `active`, so this fires only in the reactive
                    // fallback path.)
                    self.active = true;
                    replies.extend_from_slice(&TELNET_ANNOUNCE);
                }
                self.state = TelnetState::Command;
                TelnetOut::None
            }
            TelnetState::Command => {
                self.state = TelnetState::Data;
                match b {
                    IAC_BRK | IAC_IP => TelnetOut::Reset,
                    IAC_WILL | IAC_WONT | IAC_DO | IAC_DONT => {
                        self.state = TelnetState::Option(b);
                        TelnetOut::None
                    }
                    IAC_SB => {
                        self.state = TelnetState::Subnegotiation;
                        TelnetOut::None
                    }
                    // IAC IAC would be a literal 0xFF — not an Apple 1
                    // key; dropped like every other 8-bit byte. The rest
                    // (NOP, AYT, …) are swallowed.
                    _ => TelnetOut::None,
                }
            }
            TelnetState::Option(verb) => {
                self.state = TelnetState::Data;
                match verb {
                    // DO ECHO / DO SGA: already announced, stay silent
                    // (replying again would loop). Anything else the
                    // client asks us to enable: refuse.
                    IAC_DO if b != OPT_ECHO && b != OPT_SGA => {
                        replies.extend_from_slice(&[IAC, IAC_WONT, b]);
                    }
                    // Whatever the client offers to enable on its side:
                    // decline; we speak plain bytes.
                    IAC_WILL => replies.extend_from_slice(&[IAC, IAC_DONT, b]),
                    _ => {}
                }
                TelnetOut::None
            }
            TelnetState::Subnegotiation => {
                if b == IAC {
                    self.state = TelnetState::SubnegotiationCommand;
                }
                TelnetOut::None
            }
            TelnetState::SubnegotiationCommand => {
                self.state = if b == IAC_SE {
                    TelnetState::Data
                } else {
                    TelnetState::Subnegotiation
                };
                TelnetOut::None
            }
        }
    }
}

/// Run the machine as a character terminal over `input`/`output` until
/// input EOF or the far side hangs up. Generic over the streams so tests
/// drive it with in-memory pipes; `throttle` paces to 1.023 MHz
/// wall-clock (tests pass `false`).
///
/// Key mapping: a–z uppercase (the keyboard had no lower case); LF and
/// CRLF become the CR the machine expects; Ctrl bytes pass through (the
/// Apple 1 had a real CTRL key) and so does a bare ESC (the monitor's
/// cancel-line key). The one stolen chord is **Meta-R = reset** — the
/// keyboard had no Meta, so `ESC` `r` back-to-back (what "Alt sends
/// Escape" terminals transmit) warm-resets the machine, RESET-button
/// style; an ESC followed by anything else, or by silence, is forwarded.
/// How a tty session behaves, separate from the machine it drives.
#[derive(Default)]
struct TtyConfig<'a> {
    /// Pace to 1.023 MHz wall-clock (the real thing; tests pass `false`).
    throttle: bool,
    /// Printed before the machine boots (`--tty-banner`), CRLF-normalized.
    banner: Option<&'a str>,
    /// Announce telnet char-at-a-time + server echo on connect
    /// (`--tty-telnet`) so a telnet client stops echoing locally.
    telnet: bool,
}

fn tty_session<R, W>(one: &mut One, input: R, output: &mut W, cfg: TtyConfig) -> Result<(), String>
where
    R: std::io::Read + Send + 'static,
    W: std::io::Write,
{
    use std::sync::mpsc::{RecvTimeoutError, TryRecvError, channel};

    let mut telnet = TelnetFilter::default();

    // In telnet mode, offer WILL ECHO / WILL SGA up front — before the
    // banner and before the client could type — so its local echo is off
    // by the time it matters; mark the filter active so the reactive path
    // does not announce a second time.
    if cfg.telnet {
        telnet.active = true;
        if output.write_all(&TELNET_ANNOUNCE).is_err() || output.flush().is_err() {
            return Ok(());
        }
    }

    // The banner greets the caller before the machine says anything —
    // instructions for a telnet visitor (--tty-banner). Line endings are
    // normalized to CRLF for raw terminals.
    if let Some(banner) = cfg.banner {
        let mut text = Vec::new();
        for &b in banner.as_bytes() {
            match b {
                b'\r' => {}
                b'\n' => text.extend_from_slice(b"\r\n"),
                b => text.push(b),
            }
        }
        if output.write_all(&text).is_err() || output.flush().is_err() {
            return Ok(());
        }
    }

    // Blocking reads on their own thread (house style); a closed channel
    // is EOF.
    let (tx, rx) = channel::<u8>();
    std::thread::spawn(move || {
        let mut input = input;
        let mut buf = [0u8; 256];
        loop {
            match input.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    for &b in &buf[..n] {
                        if tx.send(b).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });

    // Bytes wait here until the PIA's one-byte latch is free.
    let mut pending: std::collections::VecDeque<u8> = std::collections::VecDeque::new();
    let mut eof = false;
    let mut grace = TTY_EOF_GRACE_CYCLES;
    // The Meta-R window: when the *last* pending byte is an ESC, hold it
    // until this deadline before letting it through to the machine.
    let mut esc_deadline: Option<std::time::Instant> = None;
    let mut last_fed: u8 = 0;

    loop {
        let tick_start = std::time::Instant::now();
        // One tick of machine time, then whatever it printed.
        let mut spent = 0u64;
        while spent < TTY_TICK_CYCLES {
            spent += one.cpu.step() as u64;
        }
        let mut chunk = Vec::new();
        for b in one.drain_display() {
            match b & 0x7f {
                0x0d => chunk.extend_from_slice(b"\r\n"),
                c @ 0x20..=0x7e => chunk.push(c),
                _ => {}
            }
        }
        if !chunk.is_empty() {
            // The far side hanging up is a clean end, not an error.
            if output.write_all(&chunk).is_err() || output.flush().is_err() {
                return Ok(());
            }
        }

        // Gather input. When throttling, most of the tick's wall-clock
        // budget is spent waiting here…
        let mut raw: Vec<u8> = Vec::new();
        if cfg.throttle {
            let budget = TTY_TICK.saturating_sub(tick_start.elapsed());
            match rx.recv_timeout(budget) {
                Ok(b) => raw.push(b),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => eof = true,
            }
        }
        loop {
            match rx.try_recv() {
                Ok(b) => raw.push(b),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    eof = true;
                    break;
                }
            }
        }
        let mut replies: Vec<u8> = Vec::new();
        for b in raw {
            match telnet.feed(b, &mut replies) {
                TelnetOut::None => {}
                TelnetOut::Key(b) => pending.push_back(b),
                TelnetOut::Reset => {
                    // The RESET button: immediate, and typed-ahead keys
                    // are gone with the press.
                    pending.clear();
                    esc_deadline = None;
                    last_fed = 0;
                    one.cpu.reset();
                }
            }
        }
        if !replies.is_empty() && (output.write_all(&replies).is_err() || output.flush().is_err()) {
            return Ok(());
        }

        // Feed the machine, at most one byte per tick, latch permitting.
        if !one.key_pending()
            && let Some(&next) = pending.front()
        {
            if next == 0x1b && pending.len() == 1 && !eof {
                // A lone ESC might be the start of Meta-R: hold it for
                // the window, then let it through as the monitor's key.
                let deadline =
                    *esc_deadline.get_or_insert_with(|| std::time::Instant::now() + TTY_ESC_WINDOW);
                if std::time::Instant::now() < deadline {
                    continue;
                }
            }
            esc_deadline = None;
            let b = pending.pop_front().expect("front was Some");
            if b == 0x1b && pending.front().is_some_and(|r| matches!(r, b'r' | b'R')) {
                // Meta-R: the RESET button, not machine input.
                pending.pop_front();
                one.cpu.reset();
                last_fed = 0;
            } else {
                match b {
                    // CRLF and lone LF are the terminal's spellings of
                    // the CR the machine expects.
                    b'\n' if last_fed == b'\r' => last_fed = 0,
                    b'\n' => {
                        one.key(0x0d);
                        last_fed = b'\r';
                    }
                    b => {
                        one.key(b.to_ascii_uppercase());
                        last_fed = b;
                    }
                }
            }
        }

        // After EOF, run out the grace period so the last command's
        // output makes it to the stream, then end the session.
        if eof && pending.is_empty() {
            if grace == 0 {
                return Ok(());
            }
            grace = grace.saturating_sub(TTY_TICK_CYCLES);
        }

        // …and any budget left (input arrived early) is slept off, so a
        // paste cannot sprint the machine past 1.023 MHz.
        if cfg.throttle {
            let remaining = TTY_TICK.saturating_sub(tick_start.elapsed());
            if !remaining.is_zero() {
                std::thread::sleep(remaining);
            }
        }
    }
}

fn keydown(one: &mut One, tty: &mut Tty, window: &mut sdl3::video::Window, event: &Event) {
    let Event::KeyDown {
        keycode: Some(keycode),
        keymod,
        ..
    } = event
    else {
        return;
    };
    let sym = *keycode as i32;

    if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
        if (Keycode::A as i32..=Keycode::Z as i32).contains(&sym) {
            // As in one.c: ctrl-a maps to 0x00 (sym - SDLK_a).
            one.key((sym - Keycode::A as i32) as u8);
        }
        // TODO Implement control codes 1b - 1f (comment from one.c)
    } else if keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD) {
        match *keycode {
            // Cmd-R, not Cmd-Esc: AppKit claims Cmd-Esc as a cancel key
            // equivalent on macOS, so SDL never sees it.
            Keycode::R => {
                one.cpu.reset();
                tty.reset();
            }
            Keycode::Return => {
                if window.fullscreen_state() == FullscreenType::True {
                    let _ = window.set_fullscreen(false);
                } else {
                    let _ = window.set_fullscreen(true);
                }
            }
            _ => {}
        }
    } else if keymod.is_empty() {
        match *keycode {
            Keycode::Return => one.key(0x0d), // CR
            Keycode::Tab => {
                // one.c is missing a break here, so TAB also sends DEL.
                one.key(0x09); // HT
                one.key(0x7f); // DEL
            }
            Keycode::Delete => one.key(0x7f), // DEL
            Keycode::Left => one.key(0x08),   // BS
            Keycode::Right => one.key(0x15),  // NAK
            Keycode::Up => one.key(0x0b),     // VT
            Keycode::Down => one.key(0x0a),   // LF
            Keycode::Escape => one.key(0x1b), // ESC
            _ => {}
        }
    }
}

/// Port of `ewm_one_step_cpu`: run one frame's cycle budget.
fn step_cpu(one: &mut One, cycles: u32) {
    let mut budget = cycles as i64;
    while budget > 0 {
        budget -= one.cpu.step() as i64;
    }
}

pub fn main(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(code) => return code,
    };

    // --tty never touches SDL: the terminal is the machine's terminal.
    if options.tty {
        let mut one = match build_machine(&options) {
            Ok(one) => one,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
        one.cpu.reset();
        let banner = match &options.tty_banner {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(text) => Some(text),
                Err(e) => {
                    eprintln!("cannot read banner {path}: {e}");
                    return 1;
                }
            },
            None => None,
        };
        let stdout = std::io::stdout();
        return match tty_session(
            &mut one,
            std::io::stdin(),
            &mut stdout.lock(),
            TtyConfig {
                throttle: true,
                banner: banner.as_deref(),
                telnet: options.tty_telnet,
            },
        ) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
    }

    let pad = sdl::window_padding();

    // Setup SDL

    let context = match sdl3::init() {
        Ok(context) => context,
        Err(e) => {
            eprintln!("Failed to initialize SDL: {e}");
            return 1;
        }
    };
    let video = context.video().expect("Failed to initialize SDL video");

    let title = crate::config::window_title(options.title.as_deref());
    let window = video
        .window(&title, 280 * 3 + 2 * pad, 192 * 3 + 2 * pad)
        .position_centered()
        .build();
    let window = match window {
        Ok(window) => window,
        Err(e) => {
            eprintln!("Failed create window: {e}");
            return 1;
        }
    };

    let mut canvas = window.into_canvas();

    if let Err(e) = sdl::check_renderer(&canvas) {
        eprintln!("{e}");
        return 1;
    }

    // Logical units are window pixels: the tty texture is drawn at 3x into
    // an explicit rect, leaving pad window pixels around it.
    canvas
        .set_logical_size(
            TTY_PIXEL_WIDTH as u32 * 3 + 2 * pad,
            TTY_PIXEL_HEIGHT as u32 * 3 + 2 * pad,
            SDL_RendererLogicalPresentation::LETTERBOX,
        )
        .expect("Failed to set logical size");

    // Create the machine

    let mut one = match build_machine(&options) {
        Ok(one) => one,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let mut tty = Tty::new(sdl::green(&canvas));

    one.cpu.reset();

    // Main loop

    video.text_input().start(canvas.window());

    let texture_creator = canvas.texture_creator();
    let format = sdl::pixel_format(&canvas).unwrap_or(PixelFormat::ARGB8888);
    let mut texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to create texture");
    // SDL3 defaults textures to linear filtering (SDL2 defaulted to nearest),
    // which blurs the upscaled low-res screen.
    texture.set_scale_mode(ScaleMode::Nearest);

    // The command palette renders at window resolution, not the emulated 3x.
    let layout = match sdl::pixel_format(&canvas) {
        Some(format) if format == PixelFormat::RGBA8888 => PixelLayout::Rgba8888,
        Some(format) if format == PixelFormat::XRGB8888 => PixelLayout::Rgb888,
        _ => PixelLayout::Argb8888,
    };
    let mut palette: Palette<OneAction> = Palette::new(layout);
    let mut palette_visible = false;
    let mut palette_texture = texture_creator
        .create_texture_streaming(format, palette::WIDTH as u32, palette::MAX_HEIGHT as u32)
        .expect("Failed to create palette texture");
    palette_texture.set_scale_mode(ScaleMode::Nearest);

    let mut event_pump = context.event_pump().expect("Failed to get event pump");
    let frame_ms = (1000 / ONE_FPS) as u64;
    let mut next_frame = sdl3::timer::ticks() + frame_ms;
    let mut phase: u32 = 1;
    let mut paused = false;

    'outer: loop {
        for event in event_pump.poll_iter() {
            match &event {
                Event::Quit { .. } => break 'outer,
                Event::Window { .. } => tty.screen_dirty = true,
                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    let command = keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD);
                    // While the palette is open it owns the keyboard.
                    if palette_visible {
                        let action = if command && *keycode == Keycode::K {
                            PaletteAction::Dismiss
                        } else {
                            match keycode {
                                Keycode::Escape => palette.handle_key(PaletteKey::Escape),
                                Keycode::Up => palette.handle_key(PaletteKey::Up),
                                Keycode::Down => palette.handle_key(PaletteKey::Down),
                                Keycode::Return => palette.handle_key(PaletteKey::Enter),
                                Keycode::Backspace => palette.handle_key(PaletteKey::Backspace),
                                _ => PaletteAction::None,
                            }
                        };
                        match action {
                            PaletteAction::Dismiss => palette_visible = false,
                            PaletteAction::Execute(run) => {
                                palette_visible = false;
                                let mut ctx = OneCtx {
                                    one: &mut one,
                                    tty: &mut tty,
                                    paused: &mut paused,
                                    window: canvas.window_mut(),
                                };
                                run(&mut ctx);
                            }
                            PaletteAction::None => {}
                        }
                    } else if command && *keycode == Keycode::K {
                        // Commands are registered per activation so the
                        // labels reflect the current state.
                        palette.open();
                        palette.add_command(
                            "Reset",
                            (|ctx| {
                                ctx.one.cpu.reset();
                                ctx.tty.reset();
                            }) as OneAction,
                        );
                        palette.add_command(if paused { "Unpause" } else { "Pause" }, |ctx| {
                            *ctx.paused = !*ctx.paused
                        });
                        let fullscreen = canvas.window().fullscreen_state() == FullscreenType::True;
                        palette.add_command(
                            if fullscreen {
                                "Leave Full Screen"
                            } else {
                                "Enter Full Screen"
                            },
                            |ctx| {
                                let on = ctx.window.fullscreen_state() == FullscreenType::True;
                                let _ = ctx.window.set_fullscreen(!on);
                            },
                        );
                        palette_visible = true;
                    } else {
                        keydown(&mut one, &mut tty, canvas.window_mut(), &event);
                    }
                }
                Event::TextInput { text, .. } => {
                    if palette_visible {
                        let _ = palette.handle_text(text);
                    } else if text.len() == 1 {
                        one.key(text.as_bytes()[0].to_ascii_uppercase());
                    }
                }
                _ => {}
            }
        }

        // This is very basic throttling that does bursts of CPU cycles.

        if sdl3::timer::ticks() >= next_frame {
            if !paused && !palette_visible {
                step_cpu(&mut one, ONE_CPS / ONE_FPS);
            }
            for b in one.drain_display() {
                tty.write(b);
            }

            if palette_visible
                || tty.screen_dirty
                || phase == 0
                || phase.is_multiple_of(ONE_FPS / 4)
            {
                canvas.set_draw_color(sdl3::pixels::Color::RGBA(0, 0, 0, 255));
                canvas.clear();

                tty.refresh(phase, ONE_FPS);
                tty.screen_dirty = false;

                let mut bytes = Vec::with_capacity(tty.pixels.len() * 4);
                for p in &tty.pixels {
                    bytes.extend_from_slice(&p.to_ne_bytes());
                }
                texture
                    .update(None, &bytes, TTY_PIXEL_WIDTH * 4)
                    .expect("Failed to update texture");
                let dst = Rect::new(
                    pad as i32,
                    pad as i32,
                    TTY_PIXEL_WIDTH as u32 * 3,
                    TTY_PIXEL_HEIGHT as u32 * 3,
                );
                canvas
                    .copy(&texture, None, dst)
                    .expect("Failed to copy texture");

                if palette_visible {
                    palette.render();
                    let mut bytes = Vec::with_capacity(palette.pixels.len() * 4);
                    for p in &palette.pixels {
                        bytes.extend_from_slice(&p.to_ne_bytes());
                    }
                    palette_texture
                        .update(None, &bytes, palette::WIDTH * 4)
                        .expect("Failed to update palette texture");
                    let height = palette.height();
                    let src = Rect::new(0, 0, palette::WIDTH as u32, height as u32);
                    let window_width = TTY_PIXEL_WIDTH as i32 * 3 + 2 * pad as i32;
                    let palette_dst = Rect::new(
                        (window_width - palette::WIDTH as i32) / 2,
                        40,
                        palette::WIDTH as u32,
                        height as u32,
                    );
                    let _ = canvas.copy(&palette_texture, src, palette_dst);
                }

                canvas.present();
            }

            // Advance the deadline instead of re-reading the clock, so render
            // time does not stretch every frame; resync only after a long
            // stall (window drag) rather than bursting to catch up.
            next_frame += frame_ms;
            let now = sdl3::timer::ticks();
            if now > next_frame + 1000 {
                next_frame = now + frame_ms;
            }

            phase += 1;
            if phase == ONE_FPS {
                phase = 0;
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    fn opts(args: &[&str]) -> Options {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_options(&args).expect("options must parse")
    }

    /// A scratch file under the OS temp dir.
    fn scratch(name: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("ewm-one-config-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(name);
        std::fs::write(&path, text).expect("write scratch config");
        path
    }

    #[test]
    fn sources_compose_for_one() {
        // Bare: the default machine, matching the C default.
        assert_eq!(opts(&[]).model, OneModel::Replica1);
        // A builtin selects the model...
        assert_eq!(
            opts(&["--config", "builtin:apple1"]).model,
            OneModel::Apple1
        );
        // ...and --set layers on top, in order.
        let o = opts(&["--config", "builtin:apple1", "--set", "cpu:strict=true"]);
        assert_eq!(o.model, OneModel::Apple1);
        assert!(o.strict);
        // An overlay without a --config extends the default machine.
        let overlay = scratch("strict.json", r#"{"cpu": {"strict": true}}"#);
        let o = opts(&["--config-overlay", overlay.to_str().unwrap()]);
        assert_eq!(o.model, OneModel::Replica1);
        assert!(o.strict);
        // Memory regions come from the document — hex addresses, per-file
        // path resolution, the config upgrades over the old flag.
        let config = scratch(
            "basic.json",
            r#"{"machine": {"model": "apple1",
                "memory": [{"type": "rom", "address": "0xc000", "path": "basic.rom"}]},
                "debug": {"trace": "one.trace"}}"#,
        );
        let o = opts(&["--config", config.to_str().unwrap()]);
        assert_eq!(o.memory.len(), 1);
        assert_eq!(o.memory[0].kind, crate::config::MemoryKind::Rom);
        assert_eq!(o.memory[0].address, "0xc000");
        let path = o.memory[0].path.as_deref().expect("an image region");
        assert!(path.ends_with("basic.rom"), "{path}");
        assert!(std::path::Path::new(path).is_absolute(), "{path}");
        assert!(o.trace_path.as_deref().unwrap().ends_with("one.trace"));
        // The document left the CPU unsaid: normalize filled it from the
        // model's builtin.
        assert_eq!(o.cpu, Some(crate::config::CpuModel::M6502));
        // A bare command line gets the whole default board the same way.
        let o = opts(&[]);
        assert_eq!(o.cpu, Some(crate::config::CpuModel::M65C02));
        assert_eq!(o.memory.len(), 3);
    }

    #[test]
    fn retired_flags_are_unknown() {
        // Plan 20260719-02 O4: model, memory, trace and strict are config
        // keys; the flags fall into the generic usage error.
        for retired in [
            "--model",
            "--memory",
            "--trace",
            "--strict",
            "--trace=/dev/stderr",
        ] {
            let args: Vec<String> = vec![retired.to_string()];
            assert!(matches!(parse_options(&args), Err(1)), "{retired}");
        }
    }

    #[test]
    fn two_family_models_are_rejected_by_one() {
        // The mirror of two's boundary: a two-family document is a valid
        // config, but one can't run it.
        for model in ["apple2plus", "apple2enhanced"] {
            let doc = serde_json::json!({"machine": {"model": model}});
            let config = config::from_document(doc).expect("a valid document");
            let mut options = Options::default();
            let err = apply_config(&mut options, config).unwrap_err();
            assert!(err.contains("machine.model"), "{err}");
            assert!(err.contains(model), "{err}");
            assert!(err.contains("ewm two"), "{err}");
            // The command-line spellings exit 1.
            for args in [
                vec!["--config".to_string(), format!("builtin:{model}")],
                vec!["--set".to_string(), format!("machine:model={model}")],
            ] {
                assert!(matches!(parse_options(&args), Err(1)), "{args:?}");
            }
        }
        // Family-invalid keys error through the shared validation too.
        let args: Vec<String> = ["--set", "display:monitor=green"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(matches!(parse_options(&args), Err(1)));
    }

    #[test]
    fn print_config_round_trips_for_one() {
        let region = scratch("region.bin", "");
        let config = scratch(
            "printable.json",
            &format!(
                r#"{{"machine": {{"model": "apple1",
                    "memory": [{{"type": "ram", "address": "0x4000", "path": {:?}}}]}}}}"#,
                region.to_str().unwrap()
            ),
        );
        let o = opts(&[
            "--config",
            config.to_str().unwrap(),
            "--set",
            "cpu:strict=true",
            "--set",
            "debug:trace=/dev/stderr",
        ]);
        let printed = options_to_config(&o);
        let mut doc = serde_json::to_value(&printed).expect("options serialize");
        config::compact_document(&mut doc);
        let path = scratch(
            "printed.json",
            &serde_json::to_string_pretty(&doc).expect("document prints"),
        );
        let fed_back = opts(&["--config", path.to_str().unwrap()]);
        assert_eq!(o, fed_back);
        // The query flags exit like --help.
        for query in [["--print-config"].as_slice(), &["--config", "builtin:list"]] {
            let args: Vec<String> = query.iter().map(|s| s.to_string()).collect();
            assert!(matches!(parse_options(&args), Err(0)), "{query:?}");
        }
    }

    #[test]
    fn cpu_and_ram_banks_come_from_the_document() {
        // The document describes the whole board: cpu, banks, images.
        let config = scratch(
            "components.json",
            r#"{"machine": {"model": "apple1", "cpu": "65C02",
                "memory": [
                    {"type": "ram", "address": "0x0000", "size": "4k"},
                    {"type": "ram", "address": "0x4000", "size": "4k"},
                    {"type": "rom", "address": "0xe000", "path": "builtin:apple1-basic"},
                    {"type": "rom", "address": "0xff00", "path": "builtin:WozMon"}]}}"#,
        );
        let o = opts(&["--config", config.to_str().unwrap()]);
        assert_eq!(o.cpu, Some(crate::config::CpuModel::M65C02));
        assert_eq!(o.memory.len(), 4);

        // A board with nothing at the reset vector is rejected up front.
        let headless = scratch(
            "headless.json",
            r#"{"machine": {"model": "apple1",
                "memory": [{"type": "ram", "address": "0x0000", "size": "4k"}]}}"#,
        );
        let args: Vec<String> = ["--config", headless.to_str().unwrap()]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(matches!(parse_options(&args), Err(1)));

        let mut one = build_machine(&o).expect("machine builds");
        assert_eq!(one.cpu.model, ewm_core::cpu::Model::M65C02);
        // The bank is writable RAM...
        one.cpu.mem.write(0x4000, 0x42);
        assert_eq!(one.cpu.mem.read(0x4000), 0x42);
        // ...and BASIC's entry point is mounted read-only at $E000.
        assert_eq!(one.cpu.mem.read(0xe000), 0x4c);
        one.cpu.mem.write(0xe000, 0x00);
        assert_eq!(one.cpu.mem.read(0xe000), 0x4c);

        // The whole component description survives a print round trip
        // (the bank prints back as "4k").
        let printed = options_to_config(&o);
        let mut doc = serde_json::to_value(&printed).expect("options serialize");
        crate::config::compact_document(&mut doc);
        let text = serde_json::to_string_pretty(&doc).expect("document prints");
        assert!(text.contains(r#""size": "4k""#), "{text}");
        let path = scratch("components-printed.json", &text);
        let fed_back = opts(&["--config", path.to_str().unwrap()]);
        assert_eq!(o, fed_back);
    }

    #[test]
    fn builtin_apple1_boots_to_the_woz_monitor() {
        // The O3 gate: the built-in config describes a machine that boots
        // to the Woz monitor prompt, through the same build path main runs.
        let o = opts(&["--config", "builtin:apple1"]);
        let mut one = build_machine(&o).expect("machine must construct");
        one.cpu.reset();
        let mut spent = 0u64;
        while spent < 1_000_000 {
            spent += one.cpu.step() as u64;
        }
        let text: String = one
            .drain_display()
            .iter()
            .map(|&b| (b & 0x7f) as char)
            .collect();
        assert!(text.contains('\\'), "no Woz monitor prompt, got {text:?}");
    }

    /// Step the machine and collect what it printed, one_boot-style.
    fn run(one: &mut One, cycles: u64, output: &mut Vec<u8>) {
        let mut spent = 0u64;
        while spent < cycles {
            spent += one.cpu.step() as u64;
        }
        output.extend(one.drain_display());
    }

    /// Feed keys one at a time — no keyboard queue, just the PIA latch.
    fn type_keys(one: &mut One, keys: &str, output: &mut Vec<u8>) {
        for &b in keys.as_bytes() {
            one.key(b);
            run(one, 50_000, output);
        }
    }

    #[test]
    fn builtin_apple1_runs_integer_basic() {
        // The R3 gate: the Apple 1 profile preloads Integer BASIC in the
        // $E000 RAM bank (cassette-faithful, minus the cassette), so
        // E000R from the monitor lands at the BASIC prompt.
        let o = opts(&["--config", "builtin:apple1"]);
        let mut one = build_machine(&o).expect("machine must construct");
        one.cpu.reset();
        let mut output = Vec::new();
        run(&mut one, 1_000_000, &mut output);
        type_keys(&mut one, "E000R\r", &mut output);
        run(&mut one, 1_000_000, &mut output);
        let text: String = output.iter().map(|&b| (b & 0x7f) as char).collect();
        assert!(text.contains('>'), "no Integer BASIC prompt, got {text:?}");
    }

    /// Drive a whole tty session over in-memory streams: `input` is
    /// typed (then EOF), and whatever the machine printed comes back.
    fn tty(config: &str, input: &str) -> String {
        let o = opts(&["--config", config]);
        let mut one = build_machine(&o).expect("machine must construct");
        one.cpu.reset();
        let mut out = Vec::new();
        tty_session(
            &mut one,
            std::io::Cursor::new(input.as_bytes().to_vec()),
            &mut out,
            TtyConfig::default(),
        )
        .expect("session runs to EOF");
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn tty_session_reaches_the_monitor_and_dumps_memory() {
        // Lower case, LF line ending — both get translated on the way in;
        // the dump proves the whole loop: boot, latch-paced typing,
        // display draining, EOF grace.
        let text = tty("builtin:replica1", "e000.e003\n");
        assert!(text.contains('\\'), "no monitor prompt: {text:?}");
        assert!(text.contains("E000: 4C B0 E2"), "no dump: {text:?}");
        // CRLF is one Enter, not two: the dump line appears once.
        let text = tty("builtin:replica1", "e000.e003\r\n");
        assert_eq!(text.matches("E000:").count(), 1, "{text:?}");
    }

    #[test]
    fn tty_meta_r_resets_the_machine() {
        // Start Integer BASIC, then Meta-R (ESC r back to back): the
        // machine warm-resets to the monitor — a fresh "\" prompt after
        // BASIC's ">" — instead of BASIC seeing ESC and a stray r.
        let text = tty("builtin:apple1", "E000R\r\x1br");
        let basic = text.find('>').expect("no BASIC prompt");
        let after = &text[basic..];
        assert!(
            after.contains('\\'),
            "no monitor prompt after reset: {text:?}"
        );
    }

    #[test]
    fn tty_bare_esc_belongs_to_the_monitor() {
        // ESC is the Woz monitor's cancel-line key: a held ESC that is
        // *not* followed by r goes to the machine, which answers with a
        // fresh prompt. (At EOF the hold window collapses immediately.)
        let boot = tty("builtin:apple1", "");
        let baseline = boot.matches('\\').count();
        let text = tty("builtin:apple1", "E000.\x1b");
        assert!(
            text.matches('\\').count() > baseline,
            "cancel did not reach the monitor: {text:?}"
        );
        // ESC followed by a non-r byte: both reach the machine (the A is
        // echoed after the cancel prompt).
        let text = tty("builtin:apple1", "\x1bA");
        assert!(text.matches('\\').count() > baseline, "{text:?}");
        let cancel = text.rfind('\\').expect("cancel prompt");
        assert!(text[cancel..].contains('A'), "{text:?}");
    }

    #[test]
    fn tty_banner_greets_before_the_machine() {
        // The banner prints first, CRLF-normalized, and the machine's
        // own output follows it.
        let o = opts(&["--config", "builtin:apple1"]);
        let mut one = build_machine(&o).expect("machine must construct");
        one.cpu.reset();
        let mut out = Vec::new();
        tty_session(
            &mut one,
            std::io::Cursor::new(b"".to_vec()),
            &mut out,
            TtyConfig {
                banner: Some("Welcome!\nMeta-R resets.\n"),
                ..TtyConfig::default()
            },
        )
        .expect("session runs to EOF");
        let text = String::from_utf8_lossy(&out);
        assert!(
            text.starts_with("Welcome!\r\nMeta-R resets.\r\n"),
            "{text:?}"
        );
        let banner_end = text.find("resets.").unwrap();
        assert!(
            text[banner_end..].contains('\\'),
            "no prompt after the banner: {text:?}"
        );

        // The flag needs its value.
        let args: Vec<String> = ["--tty-banner"].iter().map(|s| s.to_string()).collect();
        assert!(matches!(parse_options(&args), Err(1)));
    }

    #[test]
    fn telnet_filter_negotiates_and_strips() {
        let mut f = TelnetFilter::default();
        let mut replies = Vec::new();
        // Plain bytes: dormant passthrough, not a reply byte in sight.
        assert!(matches!(f.feed(b'A', &mut replies), TelnetOut::Key(b'A')));
        assert!(replies.is_empty() && !f.active);
        // First IAC activates: announce WILL ECHO + WILL SGA once.
        // Client volley: IAC DO ECHO, IAC WILL NAWS (31).
        for b in [255, 253, 1, 255, 251, 31] {
            assert!(matches!(f.feed(b, &mut replies), TelnetOut::None));
        }
        assert!(f.active);
        assert_eq!(
            replies,
            // WILL ECHO, WILL SGA, then DONT NAWS (no reply to DO ECHO —
            // already announced; replying again would loop).
            vec![255, 251, 1, 255, 251, 3, 255, 254, 31]
        );
        // DO of something we cannot serve: refuse. (DO LINEMODE = 34.)
        replies.clear();
        for b in [255, 253, 34] {
            assert!(matches!(f.feed(b, &mut replies), TelnetOut::None));
        }
        assert_eq!(replies, vec![255, 252, 34]);
        // Subnegotiation is swallowed whole, including inner IACs.
        replies.clear();
        for b in [255, 250, 31, 0, 80, 0, 24, 255, 240] {
            assert!(matches!(f.feed(b, &mut replies), TelnetOut::None));
        }
        assert!(replies.is_empty());
        // BREAK and IP are the RESET button; data flows again after.
        assert!(matches!(f.feed(255, &mut replies), TelnetOut::None));
        assert!(matches!(f.feed(243, &mut replies), TelnetOut::Reset));
        assert!(matches!(f.feed(b'B', &mut replies), TelnetOut::Key(b'B')));
        // IAC IAC (a literal 0xFF) is not an Apple 1 key: dropped.
        assert!(matches!(f.feed(255, &mut replies), TelnetOut::None));
        assert!(matches!(f.feed(255, &mut replies), TelnetOut::None));
    }

    /// A `Read` that delivers fixed stages with a pause between them —
    /// for exercising out-of-band arrivals (telnet BREAK is immediate
    /// and flushes typed-ahead, so it must arrive *after* earlier input
    /// has been consumed, as it would in a real session).
    struct StagedInput {
        stages: std::vec::IntoIter<Vec<u8>>,
        pause: std::time::Duration,
        started: bool,
    }

    impl std::io::Read for StagedInput {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.stages.next() {
                Some(stage) => {
                    if self.started {
                        std::thread::sleep(self.pause);
                    }
                    self.started = true;
                    buf[..stage.len()].copy_from_slice(&stage);
                    Ok(stage.len())
                }
                None => Ok(0),
            }
        }
    }

    #[test]
    fn tty_telnet_announces_before_the_machine() {
        // --tty-telnet leads the session with WILL ECHO + WILL SGA, so a
        // passive telnet client (one that waits for the server) suppresses
        // its local echo before anything else is sent — the fix for the
        // double-echo when a client doesn't negotiate first.
        let o = opts(&["--config", "builtin:apple1"]);
        let mut one = build_machine(&o).expect("machine must construct");
        one.cpu.reset();
        let mut out = Vec::new();
        tty_session(
            &mut one,
            std::io::Cursor::new(b"".to_vec()),
            &mut out,
            TtyConfig {
                banner: Some("Hi\n"),
                telnet: true,
                ..TtyConfig::default()
            },
        )
        .expect("session runs to EOF");
        // The very first bytes are the negotiation, ahead of the banner.
        assert_eq!(&out[..6], &[255, 251, 1, 255, 251, 3], "{out:?}");
        let banner = out.windows(2).position(|w| w == b"Hi").expect("banner");
        assert!(banner >= 6, "banner before negotiation: {out:?}");

        // Proactive mode must not double-announce when the client then
        // sends its own IAC: the filter is already active, so a following
        // IAC DO ECHO draws no second WILL.
        let o = opts(&["--config", "builtin:apple1"]);
        let mut one = build_machine(&o).expect("machine must construct");
        one.cpu.reset();
        let mut out = Vec::new();
        tty_session(
            &mut one,
            std::io::Cursor::new(vec![255u8, 253, 1]), // IAC DO ECHO
            &mut out,
            TtyConfig {
                telnet: true,
                ..TtyConfig::default()
            },
        )
        .expect("session runs to EOF");
        // Exactly one WILL ECHO (bytes 251, 1) in the whole stream.
        let wills = out.windows(2).filter(|w| *w == [251u8, 1]).count();
        assert_eq!(wills, 1, "double-announced: {out:?}");

        // The flag implies --tty and parses.
        let o = opts(&["--config", "builtin:apple1", "--tty-telnet"]);
        assert!(o.tty && o.tty_telnet);
    }

    #[test]
    fn tty_session_speaks_telnet_when_spoken_to() {
        // A telnet client's opening volley and a monitor command, then —
        // once BASIC is up — BREAK: the reply stream carries our
        // negotiation, and BREAK resets back to the monitor.
        let mut volley = vec![255u8, 253, 1]; // IAC DO ECHO
        volley.extend_from_slice(b"E000R\r"); // into Integer BASIC
        let input = StagedInput {
            stages: vec![volley, vec![255, 243]].into_iter(), // IAC BRK
            pause: std::time::Duration::from_millis(300),
            started: false,
        };
        let o = opts(&["--config", "builtin:apple1"]);
        let mut one = build_machine(&o).expect("machine must construct");
        one.cpu.reset();
        let mut out = Vec::new();
        tty_session(&mut one, input, &mut out, TtyConfig::default()).expect("session runs to EOF");
        // Negotiation bytes are in the output stream, before the text.
        let wills = [255u8, 251, 1, 255, 251, 3];
        assert!(
            out.windows(wills.len()).any(|w| w == wills),
            "no WILL ECHO/SGA in {out:?}"
        );
        let text: String = out.iter().map(|&b| (b & 0x7f) as char).collect();
        let basic = text.find('>').expect("no BASIC prompt");
        assert!(
            text[basic..].contains('\\'),
            "BREAK did not reset: {text:?}"
        );
    }

    #[test]
    fn profiles_reassemble_the_real_rom_images() {
        // The R3 byte-identity gate: the composed board's $E000-$FFFF is
        // the real ROM, byte for byte — the same SHA-1s the provenance
        // test in config.rs pins for the raw images.
        fn top_8k_sha1(one: &mut One) -> String {
            let bytes: Vec<u8> = (0xe000..=0xffffu32)
                .map(|a| one.cpu.mem.read(a as u16))
                .collect();
            crate::ws::sha1(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        }

        // builtin:replica1 mounts the 65C02 Krusader distribution…
        let o = opts(&["--config", "builtin:replica1"]);
        let mut one = build_machine(&o).expect("replica1 builds");
        assert_eq!(
            top_8k_sha1(&mut one),
            "f038b2d8761171ff770ce032ce0a22918cc96872"
        );

        // …and swapping in the 6502 slice reproduces the historical
        // krusader.rom machine exactly.
        let legacy = scratch(
            "legacy-replica1.json",
            r#"{"machine": {"model": "replica1", "cpu": "6502",
                "memory": [
                    {"type": "ram", "address": "0x0000", "size": "32k"},
                    {"type": "rom", "address": "0xe000", "path": "builtin:apple1-basic"},
                    {"type": "rom", "address": "0xf000", "path": "builtin:Krusader-1.3-6502"}]}}"#,
        );
        let o = opts(&["--config", legacy.to_str().unwrap()]);
        let mut one = build_machine(&o).expect("legacy replica1 builds");
        assert_eq!(one.cpu.model, ewm_core::cpu::Model::M6502);
        assert_eq!(
            top_8k_sha1(&mut one),
            "5e5ca9d94bc83a79e06806a9df180aa29d8e1a0a"
        );
    }
}
