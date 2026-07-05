//! The `ewm two` subcommand: the SDL frontend loop for the Apple ][+,
//! port of the SDL half of `two.c`. Fixed-step frames (default 40 fps,
//! 1023000/fps cycles per frame), renderer + sound + joystick + keyboard,
//! with the fake ≈1.023 MHz display preserved (quirk #3).

use ewm_core::cpu::Cpu;
use ewm_core::two::{Two, TwoType};
use sdl2::controller::Button;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::BlendMode;
use sdl2::video::FullscreenType;

use crate::scr::{ColorScheme, PixelLayout, SCR_HEIGHT, SCR_WIDTH, Scr, encode_bmp};
use crate::sdl;
use crate::snd::Snd;
use crate::tty::{TTY_PIXEL_HEIGHT, TTY_PIXEL_WIDTH, Tty};

const TWO_FPS_DEFAULT: u32 = 40;
const TWO_SPEED: u32 = 1_023_000;
const STATUS_BAR_HEIGHT: u32 = 9; // logical pixels, scaled 3x like the C

// Frames to run before dumping the hidden --screenshot and exiting.
const SCREENSHOT_FRAMES: u32 = 120;

struct MemoryOption {
    rom: bool,
    address: u16,
    path: String,
}

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
    eprintln!("Usage: ewm two [options]");
    eprintln!("  --drive1 <path>   load .dsk, .po or nib at path in slot 6 drive 1");
    eprintln!("  --drive2 <path>   load .dsk, .po or nib at path in slot 6 drive 2");
    eprintln!("  --color           enable color");
    eprintln!("  --fps <fps>       set fps for display (default: 30)");
    eprintln!("  --memory <region> add memory region (ram|rom:address:path)");
    eprintln!("  --trace <file>    trace cpu to file");
    eprintln!("  --strict          run emulator in strict mode");
    eprintln!("  --debug           print debug info");
}

#[derive(Default)]
struct Options {
    drive1: Option<String>,
    drive2: Option<String>,
    color: bool,
    fps: u32,
    memory: Vec<MemoryOption>,
    trace_path: Option<String>,
    strict: bool,
    debug: bool,
    screenshot: Option<String>,
}

fn parse_options(args: &[String]) -> Result<Options, i32> {
    let mut options = Options {
        fps: TWO_FPS_DEFAULT,
        ..Options::default()
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" => {
                usage();
                return Err(0);
            }
            "--drive1" => options.drive1 = it.next().cloned(),
            "--drive2" => options.drive2 = it.next().cloned(),
            "--color" => options.color = true,
            "--fps" => {
                // atoi semantics
                options.fps = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "--memory" => match it.next().and_then(|s| parse_memory_option(s)) {
                Some(m) => options.memory.push(m),
                None => return Err(1),
            },
            "--trace" => options.trace_path = Some("/dev/stderr".to_string()),
            "--strict" => options.strict = true,
            "--debug" => options.debug = true,
            _ => {
                if let Some(path) = arg.strip_prefix("--trace=") {
                    options.trace_path = Some(path.to_string());
                } else if let Some(path) = arg.strip_prefix("--screenshot=") {
                    // Hidden debug flag: dump a BMP of the screen after a
                    // fixed number of frames, then exit.
                    options.screenshot = Some(path.to_string());
                } else {
                    usage();
                    return Err(1);
                }
            }
        }
    }
    Ok(options)
}

/// Render the status bar into a small pixel strip: the fake MHz display and
/// the [1][2] drive lights, red text with the active drive in green — the
/// pixel version of `ewm_two_update_status_bar`.
fn render_status_bar(
    scr_chr: &ewm_core::chr::Chr,
    two: &Two,
    mhz: f64,
    layout: PixelLayout,
) -> Vec<u32> {
    let width = TTY_PIXEL_WIDTH;
    let mut pixels = vec![layout.pack(39, 39, 39, 255); width * STATUS_BAR_HEIGHT as usize];

    let text = format!("{mhz:1.3} MHZ                         [1][2]");
    let red = layout.pack(255, 0, 0, 255);
    let green = layout.pack(145, 193, 75, 255);

    for (i, ch) in text.bytes().take(40).enumerate() {
        let code = ch.wrapping_add(0x80);
        let Some(glyph) = scr_chr.bitmap(code) else {
            continue;
        };
        let drive1_active = two.dsk.on && i == 35 && two.dsk.active_drive() == 0;
        let drive2_active = two.dsk.on && i == 38 && two.dsk.active_drive() == 1;
        let color = if drive1_active || drive2_active {
            green
        } else {
            red
        };
        for y in 0..8 {
            for x in 0..7 {
                if glyph[y * 7 + x] {
                    pixels[(y + 1) * width + i * 7 + x] = color;
                }
            }
        }
    }
    pixels
}

fn pixels_to_bytes(pixels: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixels.len() * 4);
    for p in pixels {
        bytes.extend_from_slice(&p.to_ne_bytes());
    }
    bytes
}

pub fn main(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(code) => return code,
    };
    let fps = options.fps;

    // Initialize SDL

    let context = match sdl2::init() {
        Ok(context) => context,
        Err(e) => {
            eprintln!("Failed to initialize SDL: {e}");
            return 1;
        }
    };
    let video = context.video().expect("Failed to initialize SDL video");
    let timer = context.timer().expect("Failed to initialize SDL timer");
    let audio = context.audio().ok();
    let controller_subsystem = context.game_controller().ok();

    let window = video
        .window("EWM v0.1 / Apple ][+", 280 * 3, 192 * 3)
        .position(400, 60)
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
        .set_logical_size(SCR_WIDTH as u32, SCR_HEIGHT as u32)
        .expect("Failed to set logical size");

    if options.debug {
        let info = canvas.info();
        eprintln!("[TWO] Renderer name={} flags={:#x}", info.name, info.flags);
    }

    // If we have a game controller, open it

    let controller = controller_subsystem.as_ref().and_then(|subsystem| {
        let count = subsystem.num_joysticks().unwrap_or(0);
        (count > 0).then(|| subsystem.open(0).ok()).flatten()
    });

    // Create and configure the Apple II

    let mut two = match Two::new(TwoType::Apple2Plus) {
        Ok(two) => two,
        Err(e) => {
            eprintln!("[TWO] Could not create the machine: {e}");
            return 1;
        }
    };
    let mut cpu = Cpu::new(two.cpu_model());

    let layout = match sdl::pixel_format(&canvas) {
        Some(PixelFormatEnum::RGBA8888) => PixelLayout::Rgba8888,
        Some(PixelFormatEnum::RGB888) => PixelLayout::Rgb888,
        _ => PixelLayout::Argb8888,
    };
    let mut scr = Scr::new(layout);
    if options.color {
        scr.set_color_scheme(ColorScheme::Color);
    }

    let mut snd = audio.as_ref().and_then(|audio| match Snd::new(audio) {
        Ok(snd) => Some(snd),
        Err(e) => {
            eprintln!("[SND] Failed to open audio device: {e}");
            None
        }
    });

    let mut status_tty = Tty::new(sdl::green(&canvas));
    status_tty.cursor_enabled = false;

    if let Some(path) = &options.drive1
        && let Err(e) = two.load_disk(0, path)
    {
        eprintln!("[A2P] Cannot load Drive 1 with {path}: {e}");
        return 1;
    }
    if let Some(path) = &options.drive2
        && let Err(e) = two.load_disk(1, path)
    {
        eprintln!("[A2P] Cannot load Drive 2 with {path}: {e}");
        return 1;
    }

    for m in &options.memory {
        eprintln!(
            "[EWM] Adding {} ${:04X} {}",
            if m.rom { "ROM" } else { "RAM" },
            m.address,
            m.path
        );
        let data = match std::fs::read(&m.path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("[MEM] Failed to add memory from {}: {e}", m.path);
                return 1;
            }
        };
        if m.rom {
            two.add_rom(m.address, data);
        } else {
            two.add_ram(m.address, data);
        }
    }

    cpu.strict = options.strict;
    if let Some(path) = &options.trace_path {
        match std::fs::File::create(path) {
            Ok(file) => cpu.trace = Some(Box::new(std::io::BufWriter::new(file))),
            Err(e) => {
                eprintln!("Cannot open trace file {path}: {e}");
                return 1;
            }
        }
    }

    // Reset things to a known state

    cpu.reset(&mut two);

    video.text_input().start();

    let texture_creator = canvas.texture_creator();
    let format = sdl::pixel_format(&canvas).unwrap_or(PixelFormatEnum::ARGB8888);
    let mut texture = texture_creator
        .create_texture_streaming(format, SCR_WIDTH as u32, SCR_HEIGHT as u32)
        .expect("Failed to create screen texture");
    let mut bar_texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, STATUS_BAR_HEIGHT)
        .expect("Failed to create status bar texture");
    let mut tty_texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to create tty texture");
    tty_texture.set_blend_mode(BlendMode::Blend);

    let mut event_pump = context.event_pump().expect("Failed to get event pump");
    let mut ticks = timer.ticks();
    let mut phase: u32 = 1;
    let mut paused = false;
    let mut status_bar_visible = false;
    let mut frames: u32 = 0;

    let mut counter = cpu.counter;
    let mut mhz = 1.0f64;

    'outer: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'outer,
                Event::Window { .. } => two.screen_dirty = true,

                Event::ControllerButtonDown { button, .. }
                | Event::ControllerButtonUp { button, .. } => {
                    let pressed = matches!(event, Event::ControllerButtonDown { .. });
                    let state = if pressed { 0x80 } else { 0x00 };
                    match button {
                        Button::A | Button::LeftShoulder => two.buttons[0] = state,
                        Button::B | Button::RightShoulder => two.buttons[1] = state,
                        Button::X => two.buttons[2] = state,
                        Button::Y => two.buttons[3] = state,
                        _ => {}
                    }
                }

                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    let sym = keycode.into_i32();
                    if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
                        if (Keycode::A.into_i32()..=Keycode::Z.into_i32()).contains(&sym) {
                            two.key(((sym - Keycode::A.into_i32()) + 1) as u8);
                        }
                    } else if keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD) {
                        match keycode {
                            Keycode::Escape => {
                                eprintln!("[SDL] Reset");
                                cpu.reset(&mut two);
                            }
                            Keycode::Return => {
                                let window = canvas.window_mut();
                                if window.fullscreen_state() == FullscreenType::True {
                                    let _ = window.set_fullscreen(FullscreenType::Off);
                                } else {
                                    let _ = window.set_fullscreen(FullscreenType::True);
                                }
                            }
                            Keycode::I => {
                                status_bar_visible = !status_bar_visible;
                                let extra = if status_bar_visible {
                                    STATUS_BAR_HEIGHT * 3
                                } else {
                                    0
                                };
                                let _ = canvas
                                    .window_mut()
                                    .set_size(SCR_WIDTH as u32 * 3, SCR_HEIGHT as u32 * 3 + extra);
                                let _ = canvas.set_logical_size(
                                    SCR_WIDTH as u32 * 3,
                                    SCR_HEIGHT as u32 * 3 + extra,
                                );
                                if !status_bar_visible {
                                    let _ = canvas
                                        .set_logical_size(SCR_WIDTH as u32, SCR_HEIGHT as u32);
                                }
                            }
                            Keycode::P => paused = !paused,
                            _ => {}
                        }
                    } else if keymod.is_empty() {
                        match keycode {
                            Keycode::Return => two.key(0x0d), // CR
                            Keycode::Tab => {
                                // two.c is missing a break: TAB also sends DEL.
                                two.key(0x09);
                                two.key(0x7f);
                            }
                            Keycode::Delete => two.key(0x7f),
                            Keycode::Backspace | Keycode::Left => two.key(0x08),
                            Keycode::Right => two.key(0x15),
                            Keycode::Up => two.key(0x0b),
                            Keycode::Down => two.key(0x0a),
                            Keycode::Escape => two.key(0x1b),
                            _ => {}
                        }
                    }
                }

                Event::KeyUp {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    // As in two.c: only alt-keyup clears the buttons.
                    if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD) {
                        match keycode {
                            Keycode::Num1 => two.buttons[0] = 0,
                            Keycode::Num2 => two.buttons[1] = 0,
                            Keycode::Num3 => two.buttons[2] = 0,
                            Keycode::Num4 => two.buttons[3] = 0,
                            _ => {}
                        }
                    }
                }

                Event::TextInput { ref text, .. } => {
                    if text.len() == 1 {
                        two.key(text.as_bytes()[0].to_ascii_uppercase());
                    }
                }

                _ => {}
            }
        }

        if (timer.ticks() - ticks) >= (1000 / fps) {
            if !paused {
                // Feed the joystick axes to the paddle logic before the burst.
                two.joystick = controller.as_ref().map(|c| {
                    (
                        c.axis(sdl2::controller::Axis::LeftX),
                        c.axis(sdl2::controller::Axis::LeftY),
                    )
                });

                let mut budget = (TWO_SPEED / fps) as i64;
                while budget > 0 {
                    two.cycles = cpu.counter;
                    budget -= cpu.step(&mut two) as i64;
                }
            }

            let toggles = two.drain_speaker_toggles();
            if let Some(snd) = &mut snd {
                snd.update(&toggles, cpu.counter);
            }

            // Update the screen when it is flagged dirty or if we enter
            // the second half of the frames we draw each second. The
            // latter because that is when we update flashing text.
            two.screen_dirty = true; // (two.c renders every frame too)
            if two.screen_dirty {
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 255));
                canvas.clear();

                scr.update(&two, phase, fps);
                two.screen_dirty = false;

                texture
                    .update(None, &pixels_to_bytes(&scr.pixels), SCR_WIDTH * 4)
                    .expect("Failed to update texture");
                canvas
                    .copy(&texture, None, None)
                    .expect("Failed to copy texture");

                if status_bar_visible {
                    let bar = render_status_bar(scr.chr(), &two, mhz, layout);
                    bar_texture
                        .update(None, &pixels_to_bytes(&bar), TTY_PIXEL_WIDTH * 4)
                        .expect("Failed to update bar texture");
                    let dst = Rect::new(
                        0,
                        SCR_HEIGHT as i32 * 3,
                        SCR_WIDTH as u32 * 3,
                        STATUS_BAR_HEIGHT * 3,
                    );
                    let _ = canvas.copy(&bar_texture, None, dst);
                }

                if paused {
                    canvas.set_blend_mode(BlendMode::Blend);
                    canvas.set_draw_color(Color::RGBA(0, 0, 0, 224));
                    let _ = canvas.fill_rect(None);

                    status_tty.reset();
                    status_tty.set_line(8, "          ********************          ");
                    status_tty.set_line(9, "          *                  *          ");
                    status_tty.set_line(10, "          * -+-  PAUSED  -+- *          ");
                    status_tty.set_line(11, "          *                  *          ");
                    status_tty.set_line(12, "          ********************          ");
                    status_tty.refresh(0, 0);
                    tty_texture
                        .update(
                            None,
                            &pixels_to_bytes(&status_tty.pixels),
                            TTY_PIXEL_WIDTH * 4,
                        )
                        .expect("Failed to update tty texture");
                    let _ = canvas.copy(&tty_texture, None, None);
                }

                canvas.present();
            }

            ticks = timer.ticks();
            phase += 1;
            if phase == fps {
                phase = 0;

                // Calculate the number of cycles we have done in the past
                // second. TODO This will always equal 1023000 (quirk #3).
                mhz = (cpu.counter - counter) as f64 / 1_000_000.0;
                counter = cpu.counter;
            }

            frames += 1;
            if let Some(path) = &options.screenshot
                && frames >= SCREENSHOT_FRAMES
            {
                let bmp = encode_bmp(&scr.pixels, SCR_WIDTH, SCR_HEIGHT);
                if let Err(e) = std::fs::write(path, &bmp) {
                    eprintln!("Cannot write screenshot {path}: {e}");
                    return 1;
                }
                eprintln!("[TWO] Wrote screenshot to {path}");
                break 'outer;
            }
        }
    }

    0
}
