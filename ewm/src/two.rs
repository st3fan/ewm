//! The Apple ][+: machine and SDL frontend, port of `two.c` — which, like
//! this file, held both `ewm_two_t` and the SDL loop. The machine composes
//! its hardware as memory regions (RAM, the `TwoIo` soft switches, the
//! language card, the Disk II and its slot ROM) and owns the CPU; the loop
//! runs fixed-step frames with the fake ≈1.023 MHz display preserved
//! (quirk #3).

use crate::alc::Alc;
use crate::dsk::{DSK_ROM, Dsk};
use crate::hdd::{HDD_ROM, Hdd};
use crate::scr::{ColorScheme, PixelLayout, SCR_HEIGHT, SCR_WIDTH, Scr, encode_bmp};
use crate::sdl;
use crate::snd::Snd;
use crate::tty::{TTY_PIXEL_HEIGHT, TTY_PIXEL_WIDTH, Tty};
use ewm_core::cpu::{Cpu, Model};
use ewm_core::mem::{Device, DeviceHandle, Memory};
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button};
use sdl3::keyboard::{Keycode, Mod};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::rect::Rect;
use sdl3::render::BlendMode;
use sdl3::sys::render::SDL_RendererLogicalPresentation;
use sdl3::video::FullscreenType;

pub const TWO_FPS_DEFAULT: u32 = 40;
pub const TWO_SPEED: u32 = 1_023_000;

// The six machine ROMs, $D000-$FFFF (ewm_two_init loads the same files).
static ROM_341_0011: &[u8] = include_bytes!("../../rom/341-0011.bin"); // AppleSoft BASIC D000
static ROM_341_0012: &[u8] = include_bytes!("../../rom/341-0012.bin"); // AppleSoft BASIC D800
static ROM_341_0013: &[u8] = include_bytes!("../../rom/341-0013.bin"); // AppleSoft BASIC E000
static ROM_341_0014: &[u8] = include_bytes!("../../rom/341-0014.bin"); // AppleSoft BASIC E800
static ROM_341_0015: &[u8] = include_bytes!("../../rom/341-0015.bin"); // AppleSoft BASIC F000
static ROM_341_0020: &[u8] = include_bytes!("../../rom/341-0020.bin"); // Autostart Monitor F800

// Soft switches, from two.c.
const SS_KBD: u16 = 0xc000;
const SS_KBDSTRB: u16 = 0xc010;
const SS_TAPEOUT: u16 = 0xc020;
const SS_SPKR: u16 = 0xc030;
const SS_SCREEN_MODE_GRAPHICS: u16 = 0xc050;
const SS_SCREEN_MODE_TEXT: u16 = 0xc051;
const SS_GRAPHICS_STYLE_FULL: u16 = 0xc052;
const SS_GRAPHICS_STYLE_MIXED: u16 = 0xc053;
const SS_SCREEN_PAGE1: u16 = 0xc054;
const SS_SCREEN_PAGE2: u16 = 0xc055;
const SS_GRAPHICS_MODE_LGR: u16 = 0xc056;
const SS_GRAPHICS_MODE_HGR: u16 = 0xc057;
const SS_SETAN0: u16 = 0xc058;
const SS_CLRAN3: u16 = 0xc05f;
const SS_PB3: u16 = 0xc060; // TODO On the gs only? (comment from two.c)
const SS_PB0: u16 = 0xc061;
const SS_PB1: u16 = 0xc062;
const SS_PB2: u16 = 0xc063;
const SS_PADL0: u16 = 0xc064;
const SS_PADL1: u16 = 0xc065;
const SS_PTRIG: u16 = 0xc070;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TwoType {
    Apple2,
    Apple2Plus,
    Apple2E,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenMode {
    Text,
    Graphics,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphicsMode {
    Lgr,
    Hgr,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphicsStyle {
    Full,
    Mixed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenPage {
    Page1,
    Page2,
}

/// The `$C000-$C07F` soft switches and the machine state they own —
/// `ewm_two_iom_read/write` with `obj = two` turned into its own device.
pub struct TwoIo {
    pub key: u8,
    pub screen_mode: ScreenMode,
    pub screen_graphics_mode: GraphicsMode,
    pub screen_graphics_style: GraphicsStyle,
    pub screen_page: ScreenPage,
    pub screen_dirty: bool,
    pub buttons: [u8; 4],
    /// Joystick axes (x, y) as raw SDL values, set by the frontend; `None`
    /// behaves like `two->joystick == NULL` (paddle trigger is a no-op).
    pub joystick: Option<(i16, i16)>,
    padl0_time: u64,
    padl0_value: u8,
    padl1_time: u64,
    padl1_value: u8,
    speaker_toggles: Vec<u64>,
}

impl TwoIo {
    fn new() -> TwoIo {
        TwoIo {
            key: 0,
            screen_mode: ScreenMode::Text,
            screen_graphics_mode: GraphicsMode::Lgr,
            screen_graphics_style: GraphicsStyle::Full,
            screen_page: ScreenPage::Page1,
            screen_dirty: false,
            buttons: [0; 4],
            joystick: None,
            padl0_time: 0,
            padl0_value: 0,
            padl1_time: 0,
            padl1_value: 0,
            speaker_toggles: Vec::new(),
        }
    }
}

impl Device for TwoIo {
    /// Port of `ewm_two_iom_read` ($C000-$C07F). `cycles` is the CPU cycle
    /// counter the C handlers read as `cpu->counter`.
    fn read(&mut self, addr: u16, cycles: u64) -> u8 {
        match addr {
            SS_KBD => return self.key,
            SS_KBDSTRB => {
                self.key &= 0x7f;
                return 0x00;
            }

            SS_SCREEN_MODE_GRAPHICS => {
                self.screen_mode = ScreenMode::Graphics;
                self.screen_dirty = true;
            }
            SS_SCREEN_MODE_TEXT => {
                self.screen_mode = ScreenMode::Text;
                self.screen_dirty = true;
            }

            SS_GRAPHICS_MODE_LGR => {
                self.screen_graphics_mode = GraphicsMode::Lgr;
                self.screen_dirty = true;
            }
            SS_GRAPHICS_MODE_HGR => {
                self.screen_graphics_mode = GraphicsMode::Hgr;
                self.screen_dirty = true;
            }

            SS_GRAPHICS_STYLE_FULL => {
                self.screen_graphics_style = GraphicsStyle::Full;
                self.screen_dirty = true;
            }
            SS_GRAPHICS_STYLE_MIXED => {
                self.screen_graphics_style = GraphicsStyle::Mixed;
                self.screen_dirty = true;
            }

            SS_SCREEN_PAGE1 => {
                self.screen_page = ScreenPage::Page1;
                self.screen_dirty = true;
            }
            SS_SCREEN_PAGE2 => {
                self.screen_page = ScreenPage::Page2;
                self.screen_dirty = true;
            }

            SS_TAPEOUT => {
                // Ignore this
            }

            SS_SPKR => {
                self.speaker_toggles.push(cycles);
            }

            SS_PB0 => return self.buttons[0],
            SS_PB1 => return self.buttons[1],
            SS_PB2 => return self.buttons[2],
            SS_PB3 => return self.buttons[3],

            SS_SETAN0..=SS_CLRAN3 => {
                // Annunciators, ignored as in two.c.
            }

            SS_PTRIG => {
                if let Some((axis_x, axis_y)) = self.joystick {
                    let x = 128 + (axis_x as i64 / 256);
                    self.padl0_time = cycles + (x as u64 * (2820 / 255)); // TODO Remove magic values
                    self.padl0_value = 0xff;
                    let y = 128 + (axis_y as i64 / 256);
                    self.padl1_time = cycles + (y as u64 * (2820 / 255)); // TODO Remove magic values
                    self.padl1_value = 0xff;
                }
            }
            SS_PADL0 => {
                if self.padl0_time != 0 && cycles >= self.padl0_time {
                    self.padl0_time = 0;
                    self.padl0_value = 0;
                }
                return self.padl0_value;
            }
            SS_PADL1 => {
                // As in two.c, PADL1 never clears its timer.
                if self.padl1_time != 0 && cycles >= self.padl1_time {
                    self.padl1_value = 0;
                }
                return self.padl1_value;
            }

            _ => {
                eprintln!("[A2P] Unexpected read at ${addr:04X}");
            }
        }
        0
    }

    /// Port of `ewm_two_iom_write` ($C000-$C07F).
    fn write(&mut self, addr: u16, _b: u8, cycles: u64) {
        match addr {
            SS_KBD => {
                // Ignore - This is CLR80STORE on the IIe
            }

            SS_KBDSTRB => {
                self.key &= 0x7f;
            }

            SS_SCREEN_MODE_GRAPHICS => {
                self.screen_mode = ScreenMode::Graphics;
                self.screen_dirty = true;
            }
            SS_SCREEN_MODE_TEXT => {
                self.screen_mode = ScreenMode::Text;
                self.screen_dirty = true;
            }

            SS_GRAPHICS_MODE_LGR => {
                self.screen_graphics_mode = GraphicsMode::Lgr;
                self.screen_dirty = true;
            }
            SS_GRAPHICS_MODE_HGR => {
                self.screen_graphics_mode = GraphicsMode::Hgr;
                self.screen_dirty = true;
            }

            SS_GRAPHICS_STYLE_FULL => {
                self.screen_graphics_style = GraphicsStyle::Full;
                self.screen_dirty = true;
            }
            SS_GRAPHICS_STYLE_MIXED => {
                self.screen_graphics_style = GraphicsStyle::Mixed;
                self.screen_dirty = true;
            }

            SS_SCREEN_PAGE1 => {
                self.screen_page = ScreenPage::Page1;
                self.screen_dirty = true;
            }
            SS_SCREEN_PAGE2 => {
                self.screen_page = ScreenPage::Page2;
                self.screen_dirty = true;
            }

            SS_TAPEOUT => {
                // Ignore this
            }

            SS_SPKR => {
                self.speaker_toggles.push(cycles);
            }

            SS_SETAN0..=SS_CLRAN3 => {
                // Annunciators, ignored as in two.c.
            }

            _ => {
                eprintln!("[A2P] Unexpected write at ${addr:04X}");
            }
        }
    }
}

pub struct Two {
    pub cpu: Cpu,
    io: DeviceHandle<TwoIo>,
    dsk: DeviceHandle<Dsk>,
    hdd: Option<DeviceHandle<Hdd>>,
}

impl Two {
    /// Port of `ewm_two_init`. Like the C, `apple2` and `apple2e` return an
    /// error (quirk #4 in REWRITE.md).
    pub fn new(two_type: TwoType) -> Result<Two, String> {
        if two_type != TwoType::Apple2Plus {
            return Err(format!("unsupported machine type {two_type:?}"));
        }

        let mut rom = Vec::with_capacity(0x3000);
        for part in [
            ROM_341_0011,
            ROM_341_0012,
            ROM_341_0013,
            ROM_341_0014,
            ROM_341_0015,
            ROM_341_0020,
        ] {
            rom.extend_from_slice(part);
        }
        assert_eq!(rom.len(), 0x3000, "machine ROMs must cover $D000-$FFFF");

        let mut mem = Memory::new(0xc000); // $0000-$BFFF
        let io = mem.add_device(0xc000, 0xc07f, TwoIo::new());
        // The language card shadows the machine ROM, so it owns it and
        // covers both its switches and the whole $D000-$FFFF bank space.
        let alc = mem.add_device(0xc080, 0xc08f, Alc::new(rom));
        mem.map_device(alc, 0xd000, 0xffff);
        let dsk = mem.add_device(0xc0e0, 0xc0ef, Dsk::new());
        mem.add_rom(0xc600, DSK_ROM.to_vec()); // slot 6 boot ROM

        Ok(Two {
            cpu: Cpu::new(Model::M6502, mem),
            io,
            dsk,
            hdd: None,
        })
    }

    /// Mount a ProDOS block image (.hdv/.po) as a slot 7 hard drive: the
    /// card's I/O ports plus its boot/driver firmware ROM at $C700. The
    /// Autostart slot scan runs 7 before 6, so an attached drive boots
    /// before the Disk II.
    pub fn attach_hdd(&mut self, path: &str) -> Result<(), String> {
        let hdd = Hdd::new(path)?;
        self.hdd = Some(self.cpu.mem.add_device(0xc0f0, 0xc0ff, hdd));
        self.cpu.mem.add_rom(0xc700, HDD_ROM.to_vec());
        Ok(())
    }

    pub fn hdd(&self) -> Option<&Hdd> {
        self.hdd.map(|h| self.cpu.mem.device(h))
    }

    fn io(&self) -> &TwoIo {
        self.cpu.mem.device(self.io)
    }

    fn io_mut(&mut self) -> &mut TwoIo {
        self.cpu.mem.device_mut(self.io)
    }

    /// Read access to machine RAM for the renderers, which scan the text
    /// and hires pages directly (the C renderers read `cpu->ram`).
    pub fn ram(&self) -> &[u8] {
        self.cpu.mem.ram()
    }

    pub fn dsk(&self) -> &Dsk {
        self.cpu.mem.device(self.dsk)
    }

    pub fn dsk_mut(&mut self) -> &mut Dsk {
        self.cpu.mem.device_mut(self.dsk)
    }

    /// Port of `ewm_two_load_disk`.
    pub fn load_disk(&mut self, drive: usize, path: &str) -> Result<(), String> {
        self.dsk_mut().set_disk_file(drive, false, path)
    }

    /// Add an extra RAM region (`--memory ram:addr:path`). Like the C mem
    /// list, extras are dispatched before ROM and I/O — but base RAM below
    /// $C000 wins, matching the `addr < ram_size` fast path in mem.c.
    pub fn add_ram(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_ram(start, data);
    }

    /// Add an extra ROM region (`--memory rom:addr:path`).
    pub fn add_rom(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_rom(start, data);
    }

    /// Latch a key into `$C000` with the strobe bit set, as the SDL loop
    /// does with `two->key = ch | 0x80`.
    pub fn key(&mut self, key: u8) {
        self.io_mut().key = key | 0x80;
    }

    /// The keyboard latch, strobe bit included (the C `two->key`).
    pub fn key_register(&self) -> u8 {
        self.io().key
    }

    pub fn screen_mode(&self) -> ScreenMode {
        self.io().screen_mode
    }

    pub fn screen_graphics_mode(&self) -> GraphicsMode {
        self.io().screen_graphics_mode
    }

    pub fn screen_graphics_style(&self) -> GraphicsStyle {
        self.io().screen_graphics_style
    }

    pub fn screen_page(&self) -> ScreenPage {
        self.io().screen_page
    }

    pub fn screen_dirty(&self) -> bool {
        self.io().screen_dirty
    }

    pub fn set_screen_dirty(&mut self, dirty: bool) {
        self.io_mut().screen_dirty = dirty;
    }

    pub fn set_button(&mut self, button: usize, state: u8) {
        self.io_mut().buttons[button] = state;
    }

    pub fn set_joystick(&mut self, joystick: Option<(i16, i16)>) {
        self.io_mut().joystick = joystick;
    }

    /// Cycle-stamped speaker toggles recorded on `$C030` access since the
    /// last drain, for the frontend's sound path.
    pub fn drain_speaker_toggles(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.io_mut().speaker_toggles)
    }

    /// Decode text page 1 (`$0400`, interleaved rows) into 24 lines of 40
    /// characters — the workhorse for the headless gates.
    pub fn text_screen(&self) -> String {
        let ram = self.ram();
        let mut text = String::with_capacity(24 * 41);
        for row in 0..24 {
            let base = 0x400 + 0x80 * (row % 8) + 0x28 * (row / 8);
            for column in 0..40 {
                text.push(screen_code_to_char(ram[base + column]));
            }
            text.push('\n');
        }
        text
    }
}

/// Best-effort screen-code decoding for `text_screen`: normal text maps its
/// low seven bits to ASCII; inverse and flashing map their six-bit codes
/// into `$40-$5F` / `$20-$3F`.
fn screen_code_to_char(code: u8) -> char {
    let v = if code >= 0x80 {
        let v = code & 0x7f;
        if v < 0x20 { v | 0x40 } else { v }
    } else {
        let v = code & 0x3f;
        if v < 0x20 { v | 0x40 } else { v }
    };
    v as char
}

// --- SDL frontend, the loop half of two.c ---

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
    eprintln!("  --hdd <path>      mount a ProDOS block image (.hdv/.po) as a slot 7 hard drive");
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
    hdd: Option<String>,
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
            "--hdd" => options.hdd = it.next().cloned(),
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
    scr_chr: &crate::chr::Chr,
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
        let drive1_active = two.dsk().on && i == 35 && two.dsk().active_drive() == 0;
        let drive2_active = two.dsk().on && i == 38 && two.dsk().active_drive() == 1;
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

    let context = match sdl3::init() {
        Ok(context) => context,
        Err(e) => {
            eprintln!("Failed to initialize SDL: {e}");
            return 1;
        }
    };
    let video = context.video().expect("Failed to initialize SDL video");
    let audio = context.audio().ok();
    let controller_subsystem = context.gamepad().ok();

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

    let mut canvas = window.into_canvas();

    if let Err(e) = sdl::check_renderer(&canvas) {
        eprintln!("{e}");
        return 1;
    }

    canvas
        .set_logical_size(
            SCR_WIDTH as u32,
            SCR_HEIGHT as u32,
            SDL_RendererLogicalPresentation::LETTERBOX,
        )
        .expect("Failed to set logical size");

    if options.debug {
        // SDL3 has no renderer flags anymore; the name is what's left.
        eprintln!("[TWO] Renderer name={}", canvas.renderer_name);
    }

    // If we have a gamepad, open it

    let controller = controller_subsystem.as_ref().and_then(|subsystem| {
        subsystem
            .gamepads()
            .ok()
            .and_then(|ids| ids.first().copied())
            .and_then(|id| subsystem.open(id).ok())
    });

    // Create and configure the Apple II

    let mut two = match Two::new(TwoType::Apple2Plus) {
        Ok(two) => two,
        Err(e) => {
            eprintln!("[TWO] Could not create the machine: {e}");
            return 1;
        }
    };

    let layout = match sdl::pixel_format(&canvas) {
        Some(format) if format == PixelFormat::RGBA8888 => PixelLayout::Rgba8888,
        Some(format) if format == PixelFormat::XRGB8888 => PixelLayout::Rgb888,
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
    if let Some(path) = &options.hdd
        && let Err(e) = two.attach_hdd(path)
    {
        eprintln!("[A2P] Cannot mount hard drive {path}: {e}");
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

    two.cpu.strict = options.strict;
    if let Some(path) = &options.trace_path {
        match std::fs::File::create(path) {
            Ok(file) => two.cpu.trace = Some(Box::new(std::io::BufWriter::new(file))),
            Err(e) => {
                eprintln!("Cannot open trace file {path}: {e}");
                return 1;
            }
        }
    }

    // Reset things to a known state

    two.cpu.reset();

    video.text_input().start(canvas.window());

    let texture_creator = canvas.texture_creator();
    let format = sdl::pixel_format(&canvas).unwrap_or(PixelFormat::ARGB8888);
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
    let mut ticks = sdl3::timer::ticks();
    let mut phase: u32 = 1;
    let mut paused = false;
    let mut status_bar_visible = false;
    let mut frames: u32 = 0;

    let mut counter = two.cpu.counter;
    let mut mhz = 1.0f64;

    'outer: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'outer,
                Event::Window { .. } => two.set_screen_dirty(true),

                Event::ControllerButtonDown { button, .. }
                | Event::ControllerButtonUp { button, .. } => {
                    let pressed = matches!(event, Event::ControllerButtonDown { .. });
                    let state = if pressed { 0x80 } else { 0x00 };
                    match button {
                        // SDL3 renamed A/B/X/Y to their positions.
                        Button::South | Button::LeftShoulder => two.set_button(0, state),
                        Button::East | Button::RightShoulder => two.set_button(1, state),
                        Button::West => two.set_button(2, state),
                        Button::North => two.set_button(3, state),
                        _ => {}
                    }
                }

                Event::KeyDown {
                    keycode,
                    scancode,
                    keymod,
                    ..
                } => {
                    if options.debug {
                        eprintln!(
                            "[SDL] KeyDown keycode={keycode:?} scancode={scancode:?} keymod={keymod:?}"
                        );
                    }
                    let Some(keycode) = keycode else {
                        continue;
                    };
                    let sym = keycode as i32;
                    if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
                        if (Keycode::A as i32..=Keycode::Z as i32).contains(&sym) {
                            two.key(((sym - Keycode::A as i32) + 1) as u8);
                        }
                    } else if keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD) {
                        match keycode {
                            // Cmd-R, not Cmd-Esc: AppKit claims Cmd-Esc as a
                            // cancel key equivalent on macOS, so SDL never
                            // sees it.
                            Keycode::R => {
                                eprintln!("[SDL] Reset");
                                two.cpu.reset();
                            }
                            Keycode::Return => {
                                let window = canvas.window_mut();
                                if window.fullscreen_state() == FullscreenType::True {
                                    let _ = window.set_fullscreen(false);
                                } else {
                                    let _ = window.set_fullscreen(true);
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
                                    SDL_RendererLogicalPresentation::LETTERBOX,
                                );
                                if !status_bar_visible {
                                    let _ = canvas.set_logical_size(
                                        SCR_WIDTH as u32,
                                        SCR_HEIGHT as u32,
                                        SDL_RendererLogicalPresentation::LETTERBOX,
                                    );
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
                            Keycode::_1 => two.set_button(0, 0),
                            Keycode::_2 => two.set_button(1, 0),
                            Keycode::_3 => two.set_button(2, 0),
                            Keycode::_4 => two.set_button(3, 0),
                            _ => {}
                        }
                    }
                }

                Event::TextInput { ref text, .. } if text.len() == 1 => {
                    two.key(text.as_bytes()[0].to_ascii_uppercase());
                }

                _ => {}
            }
        }

        if (sdl3::timer::ticks() - ticks) >= (1000 / fps) as u64 {
            if !paused {
                // Feed the joystick axes to the paddle logic before the burst.
                two.set_joystick(
                    controller
                        .as_ref()
                        .map(|c| (c.axis(Axis::LeftX), c.axis(Axis::LeftY))),
                );

                let mut budget = (TWO_SPEED / fps) as i64;
                while budget > 0 {
                    budget -= two.cpu.step() as i64;
                }
            }

            let toggles = two.drain_speaker_toggles();
            if let Some(snd) = &mut snd {
                snd.update(&toggles, two.cpu.counter);
            }

            // Update the screen when it is flagged dirty or if we enter
            // the second half of the frames we draw each second. The
            // latter because that is when we update flashing text.
            two.set_screen_dirty(true); // (two.c renders every frame too)
            if two.screen_dirty() {
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 255));
                canvas.clear();

                scr.update(&two, phase, fps);
                two.set_screen_dirty(false);

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

            ticks = sdl3::timer::ticks();
            phase += 1;
            if phase == fps {
                phase = 0;

                // Calculate the number of cycles we have done in the past
                // second. TODO This will always equal 1023000 (quirk #3).
                mhz = (two.cpu.counter - counter) as f64 / 1_000_000.0;
                counter = two.cpu.counter;
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
