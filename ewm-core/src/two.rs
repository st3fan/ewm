//! The Apple ][+ machine, port of the machine half of `two.c`. Like
//! `ewm_two_init`, the machine owns the CPU and composes its hardware as
//! memory regions: RAM, the `TwoIo` soft-switch device (`$C000-$C07F`), the
//! language card, the Disk II controller and its slot ROM. Dispatch lives in
//! the memory system, not here.

use crate::alc::Alc;
use crate::cpu::{Cpu, Model};
use crate::dsk::{DSK_ROM, Dsk};
use crate::mem::{Device, DeviceHandle, Memory};

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
        })
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
