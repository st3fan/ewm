//! The Apple 1 / Replica 1: machine and SDL frontend, port of `one.c` —
//! which, like this file, held both `ewm_one_t` and the SDL loop. The
//! machine composes its hardware as memory regions (RAM, ROM, PIA) and owns
//! the CPU; the frame structure of the loop is the C one: event pump →
//! burst of CPU cycles → tty render.

use crate::pia::{A1_PIA6820_ADDR, A1_PIA6820_LENGTH, Pia};
use crate::sdl;
use crate::tty::{TTY_PIXEL_HEIGHT, TTY_PIXEL_WIDTH, Tty};
use ewm_core::cpu::{Cpu, Model};
use ewm_core::mem::{DeviceHandle, Memory};
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Mod};
use sdl3::pixels::PixelFormat;
use sdl3::sys::render::SDL_RendererLogicalPresentation;
use sdl3::video::FullscreenType;

static APPLE1_ROM: &[u8] = include_bytes!("../../rom/apple1.rom");
static KRUSADER_ROM: &[u8] = include_bytes!("../../rom/krusader.rom");

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
    /// $FF00; Replica 1 = 65C02 + 32K RAM + Krusader ROM at $E000. The PIA
    /// sits at $D010 on both.
    pub fn new(model: OneModel) -> One {
        let (cpu_model, ram_size, rom, rom_start) = match model {
            OneModel::Apple1 => (Model::M6502, 8 * 1024, APPLE1_ROM, 0xff00),
            OneModel::Replica1 => (Model::M65C02, 32 * 1024, KRUSADER_ROM, 0xe000),
        };
        let mut mem = Memory::new(ram_size);
        mem.add_rom(rom_start, rom.to_vec());
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

    /// Add an extra RAM region (`--memory ram:addr:path`). Like the C
    /// linked list, regions added later are dispatched first — but base RAM
    /// wins, per the `addr < ram_size` fast path in mem.c.
    pub fn add_ram(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_ram(start, data);
    }

    /// Add an extra ROM region (`--memory rom:addr:path`).
    pub fn add_rom(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_rom(start, data);
    }
}

// --- SDL frontend, the loop half of one.c ---

const ONE_FPS: u32 = 40;
const ONE_CPS: u32 = 1_023_000;

struct MemoryOption {
    rom: bool,
    address: u16,
    path: String,
}

/// Port of `parse_memory_option`: `ram|rom:address:path`, address parsed
/// with atoi semantics (decimal, 0 on garbage).
fn parse_memory_option(s: &str) -> Option<MemoryOption> {
    let mut parts = s.splitn(3, ':');
    let kind = parts.next()?;
    if kind != "ram" && kind != "rom" {
        return None;
    }
    let address = parts.next()?;
    let path = parts.next()?;
    Some(MemoryOption {
        rom: kind == "rom",
        address: address.parse::<i64>().unwrap_or(0) as u16,
        path: path.to_string(),
    })
}

fn usage() {
    eprintln!("Usage: ewm one [options]");
    eprintln!("  --model <model>   model to emulate (default: apple1)");
    eprintln!("  --memory <region> add memory region (ram|rom:address:path)");
    eprintln!("  --trace <file>    trace cpu to file");
    eprintln!("  --strict          run emulator in strict mode");
    eprintln!();
    eprintln!("Supported models:");
    eprintln!("  apple1    Classic Apple 1, 6502, 8KB RAM, Woz Monitor");
    eprintln!("  replica1  Replica 1, 65C02, 48KB RAM, KRUSADER");
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
    // Parse Apple 1 specific options. The C default model is the Replica 1.
    let mut model = OneModel::Replica1;
    let mut memory: Vec<MemoryOption> = Vec::new();
    let mut trace_path: Option<String> = None;
    let mut strict = false;

    let mut it = args.iter().peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" => {
                usage();
                return 0;
            }
            "--model" => match it.next().map(String::as_str) {
                Some("apple1") => model = OneModel::Apple1,
                Some("replica1") => model = OneModel::Replica1,
                _ => {
                    eprintln!("Unknown --model specified");
                    return 1;
                }
            },
            "--memory" => {
                let Some(m) = it.next().and_then(|s| parse_memory_option(s)) else {
                    return 1;
                };
                memory.push(m);
            }
            "--trace" => {
                // getopt optional_argument: the value comes as --trace=file.
                trace_path = Some("/dev/stderr".to_string());
            }
            "--strict" => strict = true,
            _ => {
                if let Some(path) = arg.strip_prefix("--trace=") {
                    trace_path = Some(path.to_string());
                } else {
                    usage();
                    return 1;
                }
            }
        }
    }

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
        .window("EWM v0.1 - Apple 1", 280 * 3, 192 * 3)
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

    canvas
        .set_logical_size(
            TTY_PIXEL_WIDTH as u32,
            TTY_PIXEL_HEIGHT as u32,
            SDL_RendererLogicalPresentation::LETTERBOX,
        )
        .expect("Failed to set logical size");

    // Create the machine

    let mut one = One::new(model);
    let mut tty = Tty::new(sdl::green(&canvas));

    // Add extra memory, if any

    for m in memory {
        eprintln!(
            "[EWM] Adding {} ${:04X} {}",
            if m.rom { "ROM" } else { "RAM" },
            m.address,
            m.path
        );
        let data = match std::fs::read(&m.path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!(
                    "[MEM] Failed to add {} from {}: {e}",
                    if m.rom { "ROM" } else { "RAM" },
                    m.path
                );
                return 1;
            }
        };
        if m.rom {
            one.add_rom(m.address, data);
        } else {
            one.add_ram(m.address, data);
        }
    }

    one.cpu.strict = strict;
    if let Some(path) = &trace_path {
        match std::fs::File::create(path) {
            Ok(file) => one.cpu.trace = Some(Box::new(std::io::BufWriter::new(file))),
            Err(e) => {
                eprintln!("Cannot open trace file {path}: {e}");
                return 1;
            }
        }
    }

    one.cpu.reset();

    // Main loop

    video.text_input().start(canvas.window());

    let texture_creator = canvas.texture_creator();
    let format = sdl::pixel_format(&canvas).unwrap_or(PixelFormat::ARGB8888);
    let mut texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to create texture");

    let mut event_pump = context.event_pump().expect("Failed to get event pump");
    let mut ticks = sdl3::timer::ticks();
    let mut phase: u32 = 1;

    'outer: loop {
        for event in event_pump.poll_iter() {
            match &event {
                Event::Quit { .. } => break 'outer,
                Event::Window { .. } => tty.screen_dirty = true,
                Event::KeyDown { .. } => keydown(&mut one, &mut tty, canvas.window_mut(), &event),
                Event::TextInput { text, .. } if text.len() == 1 => {
                    one.key(text.as_bytes()[0].to_ascii_uppercase());
                }
                _ => {}
            }
        }

        // This is very basic throttling that does bursts of CPU cycles.

        if (sdl3::timer::ticks() - ticks) >= (1000 / ONE_FPS) as u64 {
            step_cpu(&mut one, ONE_CPS / ONE_FPS);
            for b in one.drain_display() {
                tty.write(b);
            }

            if tty.screen_dirty || phase == 0 || phase.is_multiple_of(ONE_FPS / 4) {
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
                canvas
                    .copy(&texture, None, None)
                    .expect("Failed to copy texture");

                canvas.present();
            }

            ticks = sdl3::timer::ticks();

            phase += 1;
            if phase == ONE_FPS {
                phase = 0;
            }
        }
    }

    0
}
