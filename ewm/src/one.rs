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

// The mountable one-family ROM images (notes/APPLE1.md): the pristine
// Woz Monitor, Integer BASIC, and the Krusader $F000-$FFFF slice (which
// carries its own modified monitor page). The historical 8KB
// krusader.rom was exactly BASIC + the 6502 Krusader slice — pinned by
// the provenance test in config.rs.
static WOZMON_ROM: &[u8] = include_bytes!("../../roms/WozMon.rom");
static BASIC_ROM: &[u8] = include_bytes!("../../roms/apple1-basic.rom");
static KRUSADER_6502_ROM: &[u8] = include_bytes!("../../roms/Krusader-1.3-6502.rom");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OneModel {
    Apple1,
    Replica1,
}

pub struct One {
    pub model: OneModel,
    pub cpu: Cpu,
    pia: DeviceHandle<Pia>,
}

impl One {
    /// Port of `ewm_one_init`: Apple 1 = 6502 + 8K RAM + Woz monitor ROM at
    /// $FF00; Replica 1 = 65C02 + 32K RAM + BASIC at $E000 + Krusader at
    /// $F000 (byte-identical to the historical single 8K ROM mount). The
    /// PIA sits at $D010 on both.
    pub fn new(model: OneModel) -> One {
        One::new_with_cpu(model, None)
    }

    /// `new` with an optional CPU override (config `machine.cpu`); `None`
    /// keeps the model's CPU.
    pub fn new_with_cpu(model: OneModel, cpu: Option<Model>) -> One {
        let (cpu_model, ram_size) = match model {
            OneModel::Apple1 => (Model::M6502, 8 * 1024),
            OneModel::Replica1 => (Model::M65C02, 32 * 1024),
        };
        let cpu_model = cpu.unwrap_or(cpu_model);
        let mut mem = Memory::new(ram_size);
        match model {
            OneModel::Apple1 => mem.add_rom(0xff00, WOZMON_ROM.to_vec()),
            OneModel::Replica1 => {
                mem.add_rom(0xe000, BASIC_ROM.to_vec());
                mem.add_rom(0xf000, KRUSADER_6502_ROM.to_vec());
            }
        }
        let pia = mem.add_device(
            A1_PIA6820_ADDR,
            A1_PIA6820_ADDR + A1_PIA6820_LENGTH - 1,
            Pia::new(),
        );
        One {
            model,
            cpu: Cpu::new(cpu_model, mem),
            pia,
        }
    }

    /// Port of `ewm_one_keydown`: latch the key into the PIA with bit 7 set
    /// and raise IRQA1.
    pub fn key(&mut self, key: u8) {
        let pia = self.cpu.mem.device_mut(self.pia);
        pia.set_ina(key | 0x80);
        pia.set_irqa1();
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

/// What fills a memory region: an image (a file path or `builtin:<name>`)
/// or an empty RAM bank of a given byte size (R2 of
/// plans/20260719-03-one-machine-components.md).
#[derive(Debug, PartialEq)]
enum MemorySource {
    Image(String),
    Bank(u32),
}

#[derive(Debug, PartialEq)]
struct MemoryOption {
    rom: bool,
    address: u16,
    source: MemorySource,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Options {
    model: OneModel,
    /// CPU override (`machine.cpu`); None = the model's CPU.
    cpu: Option<crate::config::CpuModel>,
    memory: Vec<MemoryOption>,
    trace_path: Option<String>,
    strict: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            // The C default model is the Replica 1.
            model: OneModel::Replica1,
            cpu: None,
            memory: Vec::new(),
            trace_path: None,
            strict: false,
        }
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
}

/// Seed `Options` from the layered config document (pass 1 of
/// `parse_options`). `config::from_document` validated it — structurally,
/// for completeness, and against the one-family key table — so what is
/// left is the model boundary and the straight field mapping.
fn apply_config(options: &mut Options, config: crate::config::Config) -> Result<(), String> {
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
    for region in machine.memory {
        let address = region.address_value()?;
        let source = match (region.path, region.size) {
            (Some(path), None) => MemorySource::Image(path),
            (None, Some(size)) => MemorySource::Bank(
                crate::config::parse_memory_size(&size).expect("validated structurally"),
            ),
            _ => unreachable!("validated structurally: exactly one of path or size"),
        };
        options.memory.push(MemoryOption {
            rom: region.kind == crate::config::MemoryKind::Rom,
            address,
            source,
        });
    }
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
        machine: Some(crate::config::Machine {
            model: Some(match options.model {
                OneModel::Apple1 => crate::config::Model::Apple1,
                OneModel::Replica1 => crate::config::Model::Replica1,
            }),
            cpu: options.cpu,
            aux: None,
            slots: None,
            memory: options
                .memory
                .iter()
                .map(|region| {
                    let (path, size) = match &region.source {
                        MemorySource::Image(path) => (Some(path.clone()), None),
                        // Whole KiB print as "Nk", exact bytes otherwise.
                        MemorySource::Bank(bytes) => (
                            None,
                            Some(if bytes % 1024 == 0 {
                                format!("{}k", bytes / 1024)
                            } else {
                                format!("{bytes}")
                            }),
                        ),
                    };
                    crate::config::MemoryRegion {
                        kind: if region.rom {
                            crate::config::MemoryKind::Rom
                        } else {
                            crate::config::MemoryKind::Ram
                        },
                        address: format!("0x{:04x}", region.address),
                        path,
                        size,
                    }
                })
                .collect(),
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

/// Build the machine `parse_options` described: construct the model
/// (with the `machine.cpu` override, if any), load the extra memory
/// regions and RAM banks, arm strict/trace — the machine half of `main`,
/// shared with the boot-gate test.
fn build_machine(options: &Options) -> Result<One, String> {
    let cpu = options.cpu.map(|cpu| match cpu {
        crate::config::CpuModel::M6502 => Model::M6502,
        crate::config::CpuModel::M65C02 => Model::M65C02,
    });
    let mut one = One::new_with_cpu(options.model, cpu);
    for m in &options.memory {
        let data = match &m.source {
            MemorySource::Image(path) => {
                eprintln!(
                    "[EWM] Adding {} ${:04X} {}",
                    if m.rom { "ROM" } else { "RAM" },
                    m.address,
                    path
                );
                crate::config::read_memory_image(path).map_err(|e| format!("[MEM] {e}"))?
            }
            MemorySource::Bank(bytes) => {
                eprintln!("[EWM] Adding RAM bank ${:04X} ({bytes} bytes)", m.address);
                vec![0; *bytes as usize]
            }
        };
        if m.rom {
            one.add_rom(m.address, data);
        } else {
            one.add_ram(m.address, data);
        }
    }
    one.cpu.strict = options.strict;
    if let Some(path) = &options.trace_path {
        let file = std::fs::File::create(path)
            .map_err(|e| format!("Cannot open trace file {path}: {e}"))?;
        one.cpu.trace = Some(Box::new(std::io::BufWriter::new(file)));
    }
    Ok(one)
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
    let pad = sdl::window_padding();

    let options = match parse_options(args) {
        Ok(options) => options,
        Err(code) => return code,
    };

    // Setup SDL

    let context = match sdl3::init() {
        Ok(context) => context,
        Err(e) => {
            eprintln!("Failed to initialize SDL: {e}");
            return 1;
        }
    };
    let video = context.video().expect("Failed to initialize SDL video");

    let window = video
        .window("EWM v0.1 - Apple 1", 280 * 3 + 2 * pad, 192 * 3 + 2 * pad)
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
        assert!(o.memory[0].rom);
        assert_eq!(o.memory[0].address, 0xc000);
        let MemorySource::Image(path) = &o.memory[0].source else {
            panic!("expected an image region");
        };
        assert!(path.ends_with("basic.rom"), "{path}");
        assert!(std::path::Path::new(path).is_absolute(), "{path}");
        assert!(o.trace_path.as_deref().unwrap().ends_with("one.trace"));
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
        for model in ["2plus", "2e"] {
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
        // machine.cpu overrides the model's CPU; a size region mounts an
        // empty RAM bank; a builtin: region mounts the embedded image.
        let config = scratch(
            "components.json",
            r#"{"machine": {"model": "apple1", "cpu": "65C02",
                "memory": [
                    {"type": "ram", "address": "0x4000", "size": "4k"},
                    {"type": "rom", "address": "0xe000", "path": "builtin:apple1-basic"}]}}"#,
        );
        let o = opts(&["--config", config.to_str().unwrap()]);
        assert_eq!(o.cpu, Some(crate::config::CpuModel::M65C02));
        assert_eq!(o.memory[0].source, MemorySource::Bank(4096));
        assert_eq!(
            o.memory[1].source,
            MemorySource::Image("builtin:apple1-basic".to_string())
        );

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
}
