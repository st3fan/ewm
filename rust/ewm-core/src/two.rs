//! The Apple ][+ machine, port of the machine half of `two.c`: RAM, the
//! AppleSoft/Autostart ROMs, the `$C000-$C07F` soft-switch dispatch, and the
//! language card. The SDL loop half of `two.c` lands in the frontend crate
//! in Phase 7; the Disk II in Phase 6.

use crate::alc::Alc;
use crate::bus::Bus;
use crate::cpu::Model;
use crate::dsk::{DSK_ROM, Dsk};

pub const TWO_FPS_DEFAULT: u32 = 40;
pub const TWO_SPEED: u32 = 1_023_000;

// The six machine ROMs, $D000-$FFFF (ewm_two_init loads the same files).
static ROM_341_0011: &[u8] = include_bytes!("../../../src/rom/341-0011.bin"); // AppleSoft BASIC D000
static ROM_341_0012: &[u8] = include_bytes!("../../../src/rom/341-0012.bin"); // AppleSoft BASIC D800
static ROM_341_0013: &[u8] = include_bytes!("../../../src/rom/341-0013.bin"); // AppleSoft BASIC E000
static ROM_341_0014: &[u8] = include_bytes!("../../../src/rom/341-0014.bin"); // AppleSoft BASIC E800
static ROM_341_0015: &[u8] = include_bytes!("../../../src/rom/341-0015.bin"); // AppleSoft BASIC F000
static ROM_341_0020: &[u8] = include_bytes!("../../../src/rom/341-0020.bin"); // Autostart Monitor F800

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

pub struct Two {
    pub screen_mode: ScreenMode,
    pub screen_graphics_mode: GraphicsMode,
    pub screen_graphics_style: GraphicsStyle,
    pub screen_page: ScreenPage,
    pub screen_dirty: bool,

    pub key: u8,
    pub buttons: [u8; 4],

    /// Joystick axes (x, y) as raw SDL values, set by the frontend; `None`
    /// behaves like `two->joystick == NULL` (paddle trigger is a no-op).
    pub joystick: Option<(i16, i16)>,
    padl0_time: u64,
    padl0_value: u8,
    padl1_time: u64,
    padl1_value: u8,

    /// Mirror of `cpu.counter`, updated by the run loop before each step —
    /// the C handlers read `cpu->counter` directly for speaker and paddle
    /// timestamps; the Bus trait has no CPU access, so the loop provides it.
    pub cycles: u64,
    speaker_toggles: Vec<u64>,

    ram: Vec<u8>, // $0000-$BFFF
    rom: Vec<u8>, // $D000-$FFFF, the six ROM files combined
    pub alc: Alc, // language card, $C080-$C08F + $D000-$FFFF banks
    pub dsk: Dsk, // Disk ][ controller, $C600 ROM + $C0E0-$C0EF
    extra: Vec<Region>,
}

/// An extra memory region added with `--memory`.
struct Region {
    start: u16,
    data: Vec<u8>,
    writable: bool,
}

impl Region {
    fn contains(&self, addr: u16) -> bool {
        addr >= self.start && ((addr - self.start) as usize) < self.data.len()
    }
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

        Ok(Two {
            screen_mode: ScreenMode::Text,
            screen_graphics_mode: GraphicsMode::Lgr,
            screen_graphics_style: GraphicsStyle::Full,
            screen_page: ScreenPage::Page1,
            screen_dirty: false,
            key: 0,
            buttons: [0; 4],
            joystick: None,
            padl0_time: 0,
            padl0_value: 0,
            padl1_time: 0,
            padl1_value: 0,
            cycles: 0,
            speaker_toggles: Vec::new(),
            ram: vec![0; 0xc000],
            rom,
            alc: Alc::new(),
            dsk: Dsk::new(),
            extra: Vec::new(),
        })
    }

    /// Read access to machine RAM for the renderers, which scan the text
    /// and hires pages directly (the C renderers read `cpu->ram`).
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }

    /// Add an extra RAM region (`--memory ram:addr:path`). Like the C mem
    /// list, extras are dispatched before ROM and I/O — but base RAM below
    /// $C000 wins, matching the `addr < ram_size` fast path in mem.c.
    pub fn add_ram(&mut self, start: u16, data: Vec<u8>) {
        self.extra.insert(
            0,
            Region {
                start,
                data,
                writable: true,
            },
        );
    }

    /// Add an extra ROM region (`--memory rom:addr:path`).
    pub fn add_rom(&mut self, start: u16, data: Vec<u8>) {
        self.extra.insert(
            0,
            Region {
                start,
                data,
                writable: false,
            },
        );
    }

    /// Port of `ewm_two_load_disk`.
    pub fn load_disk(&mut self, drive: usize, path: &str) -> Result<(), String> {
        self.dsk.set_disk_file(drive, false, path)
    }

    /// The Apple ][+ is wired with a 6502.
    pub fn cpu_model(&self) -> Model {
        Model::M6502
    }

    /// Latch a key into `$C000` with the strobe bit set, as the SDL loop
    /// does with `two->key = ch | 0x80`.
    pub fn key(&mut self, key: u8) {
        self.key = key | 0x80;
    }

    /// Cycle-stamped speaker toggles recorded on `$C030` access since the
    /// last drain, for the frontend's sound path (Phase 7).
    pub fn drain_speaker_toggles(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.speaker_toggles)
    }

    /// Decode text page 1 (`$0400`, interleaved rows) into 24 lines of 40
    /// characters — the workhorse for the headless gates.
    pub fn text_screen(&self) -> String {
        let mut text = String::with_capacity(24 * 41);
        for row in 0..24 {
            let base = 0x400 + 0x80 * (row % 8) + 0x28 * (row / 8);
            for column in 0..40 {
                text.push(screen_code_to_char(self.ram[base + column]));
            }
            text.push('\n');
        }
        text
    }

    /// Port of `ewm_two_iom_read` ($C000-$C07F).
    fn iom_read(&mut self, addr: u16) -> u8 {
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
                self.speaker_toggles.push(self.cycles);
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
                    self.padl0_time = self.cycles + (x as u64 * (2820 / 255)); // TODO Remove magic values
                    self.padl0_value = 0xff;
                    let y = 128 + (axis_y as i64 / 256);
                    self.padl1_time = self.cycles + (y as u64 * (2820 / 255)); // TODO Remove magic values
                    self.padl1_value = 0xff;
                }
            }
            SS_PADL0 => {
                if self.padl0_time != 0 && self.cycles >= self.padl0_time {
                    self.padl0_time = 0;
                    self.padl0_value = 0;
                }
                return self.padl0_value;
            }
            SS_PADL1 => {
                // As in two.c, PADL1 never clears its timer.
                if self.padl1_time != 0 && self.cycles >= self.padl1_time {
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
    fn iom_write(&mut self, addr: u16, _b: u8) {
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
                self.speaker_toggles.push(self.cycles);
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

impl Bus for Two {
    fn read(&mut self, addr: u16) -> u8 {
        if addr >= 0xc000 {
            for region in &self.extra {
                if region.contains(addr) {
                    return region.data[(addr - region.start) as usize];
                }
            }
        }
        match addr {
            0x0000..=0xbfff => self.ram[addr as usize],
            0xc000..=0xc07f => self.iom_read(addr),
            0xc080..=0xc08f => self.alc.iom_read(addr),
            0xc0e0..=0xc0ef => self.dsk.io_read(addr),
            0xc600..=0xc6ff => DSK_ROM[(addr - 0xc600) as usize],
            // Remaining $C090-$CFFF slot space is unmapped; unmatched reads
            // return 0 like mem_get_byte.
            0xc090..=0xcfff => 0,
            0xd000..=0xffff => match self.alc.read(addr) {
                Some(b) => b,
                None => self.rom[(addr - 0xd000) as usize],
            },
        }
    }

    fn write(&mut self, addr: u16, b: u8) {
        if addr >= 0xc000 {
            for region in &mut self.extra {
                if region.contains(addr) {
                    if region.writable {
                        region.data[(addr - region.start) as usize] = b;
                    }
                    return;
                }
            }
        }
        match addr {
            0x0000..=0xbfff => self.ram[addr as usize] = b,
            0xc000..=0xc07f => self.iom_write(addr, b),
            0xc080..=0xc08f => self.alc.iom_write(addr),
            0xc0e0..=0xc0ef => self.dsk.io_write(addr, b),
            0xc090..=0xcfff => {}
            // Language-card RAM when write-enabled; otherwise swallowed
            // (ROM), as in the C mem walk.
            0xd000..=0xffff => self.alc.write(addr, b),
        }
    }
}
