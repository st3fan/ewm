//! The `ewm one` subcommand: the SDL frontend loop for the Apple 1 /
//! Replica 1, port of the SDL half of `one.c`. The frame structure is the
//! C one: event pump → burst of CPU cycles → tty render.

use ewm_core::cpu::Cpu;
use ewm_core::one::{One, OneModel};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::PixelFormatEnum;
use sdl2::video::FullscreenType;

use crate::sdl;
use crate::tty::{TTY_PIXEL_HEIGHT, TTY_PIXEL_WIDTH, Tty};

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

fn keydown(
    cpu: &mut Cpu,
    one: &mut One,
    tty: &mut Tty,
    window: &mut sdl2::video::Window,
    event: &Event,
) {
    let Event::KeyDown {
        keycode: Some(keycode),
        keymod,
        ..
    } = event
    else {
        return;
    };
    let sym = keycode.into_i32();

    if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
        if (Keycode::A.into_i32()..=Keycode::Z.into_i32()).contains(&sym) {
            // As in one.c: ctrl-a maps to 0x00 (sym - SDLK_a).
            one.key((sym - Keycode::A.into_i32()) as u8);
        }
        // TODO Implement control codes 1b - 1f (comment from one.c)
    } else if keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD) {
        match *keycode {
            Keycode::Escape => {
                cpu.reset(one);
                tty.reset();
            }
            Keycode::Return => {
                if window.fullscreen_state() == FullscreenType::True {
                    let _ = window.set_fullscreen(FullscreenType::Off);
                } else {
                    let _ = window.set_fullscreen(FullscreenType::True);
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
fn step_cpu(cpu: &mut Cpu, one: &mut One, cycles: u32) {
    let mut budget = cycles as i64;
    while budget > 0 {
        budget -= cpu.step(one) as i64;
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

    let context = match sdl2::init() {
        Ok(context) => context,
        Err(e) => {
            eprintln!("Failed to initialize SDL: {e}");
            return 1;
        }
    };
    let video = context.video().expect("Failed to initialize SDL video");
    let timer = context.timer().expect("Failed to initialize SDL timer");

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

    let mut canvas = match window.into_canvas().accelerated().build() {
        Ok(canvas) => canvas,
        Err(e) => {
            eprintln!("Failed to create renderer: {e}");
            return 1;
        }
    };

    if let Err(e) = sdl::check_renderer(&canvas) {
        eprintln!("{e}");
        return 1;
    }

    canvas
        .set_logical_size(TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to set logical size");

    // Create the machine

    let mut one = One::new(model);
    let mut cpu = Cpu::new(one.cpu_model());
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

    cpu.strict = strict;
    if let Some(path) = &trace_path {
        match std::fs::File::create(path) {
            Ok(file) => cpu.trace = Some(Box::new(std::io::BufWriter::new(file))),
            Err(e) => {
                eprintln!("Cannot open trace file {path}: {e}");
                return 1;
            }
        }
    }

    cpu.reset(&mut one);

    // Main loop

    video.text_input().start();

    let texture_creator = canvas.texture_creator();
    let format = sdl::pixel_format(&canvas).unwrap_or(PixelFormatEnum::ARGB8888);
    let mut texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to create texture");

    let mut event_pump = context.event_pump().expect("Failed to get event pump");
    let mut ticks = timer.ticks();
    let mut phase: u32 = 1;

    'outer: loop {
        for event in event_pump.poll_iter() {
            match &event {
                Event::Quit { .. } => break 'outer,
                Event::Window { .. } => tty.screen_dirty = true,
                Event::KeyDown { .. } => {
                    keydown(&mut cpu, &mut one, &mut tty, canvas.window_mut(), &event)
                }
                Event::TextInput { text, .. } => {
                    if text.len() == 1 {
                        one.key(text.as_bytes()[0].to_ascii_uppercase());
                    }
                }
                _ => {}
            }
        }

        // This is very basic throttling that does bursts of CPU cycles.

        if (timer.ticks() - ticks) >= (1000 / ONE_FPS) {
            step_cpu(&mut cpu, &mut one, ONE_CPS / ONE_FPS);
            for b in one.drain_display() {
                tty.write(b);
            }

            if tty.screen_dirty || phase == 0 || phase.is_multiple_of(ONE_FPS / 4) {
                canvas.set_draw_color(sdl2::pixels::Color::RGBA(0, 0, 0, 255));
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

            ticks = timer.ticks();

            phase += 1;
            if phase == ONE_FPS {
                phase = 0;
            }
        }
    }

    0
}
