//! The Apple ][+: machine and SDL frontend, port of `two.c` — which, like
//! this file, held both `ewm_two_t` and the SDL loop. The machine composes
//! its hardware as memory regions (RAM, the `TwoIo` soft switches, the
//! language card, the Disk II and its slot ROM) and owns the CPU; the loop
//! runs fixed-step frames with the fake ≈1.023 MHz display preserved
//! (quirk #3).

use crate::alc::Alc;
use crate::clk::{CLK_ROM, Clk};
use crate::dsk::{DSK_ROM, Dsk};
use crate::hdd::{HDD_ROM, Hdd};
use crate::palette::{self, Palette, PaletteAction, PaletteKey};
use crate::scr::{ColorScheme, PixelLayout, SCR_HEIGHT, SCR_WIDTH, Scr, encode_bmp, frame_width};
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
use sdl3::render::{BlendMode, ScaleMode};
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

// The two 8K Enhanced //e system ROM halves: CD = $C000-$DFFF, EF =
// $E000-$FFFF. The language card banks $D000-$FFFF (the CD half's upper 4K
// plus the whole EF half); $C000-$CFFF is I/O and internal firmware.
static ROM_IIE_CD: &[u8] =
    include_bytes!("../../rom/Apple IIe CD Enhanced - 342-0304-A - 2764.bin");
static ROM_IIE_EF: &[u8] =
    include_bytes!("../../rom/Apple IIe EF Enhanced - 342-0303-A - 2764.bin");

// Soft switches, from two.c.
const SS_KBD: u16 = 0xc000;
const SS_KBDSTRB: u16 = 0xc010;
// //e $C100-$CFFF ROM-arbitration switches (Phase 2b). Written to set state.
const SS_SLOTCXROM: u16 = 0xc006; // W: peripheral-slot ROM at $C100-$CFFF
const SS_INTCXROM: u16 = 0xc007; // W: internal ROM at $C100-$CFFF
const SS_INTC3ROM: u16 = 0xc00a; // W: internal $C300 (80-column firmware)
const SS_SLOTC3ROM: u16 = 0xc00b; // W: slot-3 card ROM at $C300
const SS_RDCXROM: u16 = 0xc015; // R: bit 7 = INTCXROM
const SS_RDC3ROM: u16 = 0xc017; // R: bit 7 = SLOTC3ROM
// //e $C010-$C01F status reads (Phase 2c): state reported in bit 7.
const SS_RDLCBNK2: u16 = 0xc011;
const SS_RDLCRAM: u16 = 0xc012;
const SS_RDRAMRD: u16 = 0xc013;
const SS_RDRAMWRT: u16 = 0xc014;
const SS_RDALTZP: u16 = 0xc016;
const SS_RD80STORE: u16 = 0xc018;
const SS_RDVBL: u16 = 0xc019;
const SS_RDTEXT: u16 = 0xc01a;
const SS_RDMIXED: u16 = 0xc01b;
const SS_RDPAGE2: u16 = 0xc01c;
const SS_RDHIRES: u16 = 0xc01d;
const SS_RDALTCHAR: u16 = 0xc01e;
const SS_RD80COL: u16 = 0xc01f;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TwoType {
    Apple2,
    #[default]
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
    debug: bool,
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
            debug: false,
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
                if self.debug {
                    eprintln!("[A2P] Unexpected read at ${addr:04X}");
                }
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
                if self.debug {
                    eprintln!("[A2P] Unexpected write at ${addr:04X}");
                }
            }
        }
    }
}

/// The Enhanced //e memory-management / I/O unit — the MMU and IOU soft
/// switches at `$C000-$C07F`, the //e counterpart to `TwoIo`.
///
/// **Phase 2a is a stub:** it answers the keyboard latch and otherwise returns
/// benign values and ignores writes — enough for the 65C02 to execute //e ROM
/// headlessly. Auxiliary-memory routing and the real `$C000-$C01F` switch
/// semantics arrive in Phases 2c and 4.
struct IouE {
    /// Keyboard latch (`$C000`), strobe bit (`$80`) included, as in `TwoIo`.
    key: u8,
    /// Gate the unhandled-access logging behind `--debug`.
    debug: bool,

    // --- Phase 2b: $C100-$CFFF ROM arbitration ---
    /// INTCXROM: `$C100-$CFFF` reads internal ROM (on) vs slot ROM (off).
    intcxrom: bool,
    /// SLOTC3ROM: `$C300` reads slot-3 card ROM (on) vs internal 80-column
    /// firmware (off). Only consulted when INTCXROM is off.
    slotc3rom: bool,
    /// Whether the internal `$C800-$CFFF` expansion ROM is currently exposed.
    /// Set when internal `$C3xx` firmware is accessed, cleared by a `$CFFF`
    /// access — the standard //e expansion-ROM latch. Only the internal slot-3
    /// firmware has a `$C800` image in EWM (no peripheral card here does), so a
    /// single flag suffices; per-slot expansion is out of scope.
    c800_internal: bool,
    /// Internal `$C100-$CFFF` firmware (the CD ROM half, offset `$100`).
    internal_rom: &'static [u8],
    /// Peripheral card ROM at `$Cn00-$CnFF`, indexed by slot `1..=7`
    /// (slot 0 unused); `None` when no card occupies the slot.
    slot_rom: [Option<&'static [u8]>; 8],

    // --- Phase 2c: $C000-$C00F memory switches (state only; the aux-memory
    // routing they describe arrives in Phase 4) ---
    /// 80STORE (`$C000`/`$C001`): PAGE2 routes the display page to aux.
    store80: bool,
    /// RAMRD (`$C002`/`$C003`): reads of `$0200-$BFFF` come from aux.
    ramrd: bool,
    /// RAMWRT (`$C004`/`$C005`): writes to `$0200-$BFFF` go to aux.
    ramwrt: bool,
    /// ALTZP (`$C008`/`$C009`): zero page / stack / LC RAM come from aux.
    altzp: bool,

    // --- Phase 2c: display soft switches ($C050-$C057, $C00C-$C00F) ---
    /// TEXT (`$C050` graphics / `$C051` text).
    text: bool,
    /// MIXED (`$C052` off / `$C053` on).
    mixed: bool,
    /// PAGE2 (`$C054` page 1 / `$C055` page 2).
    page2: bool,
    /// HIRES (`$C056` lo-res / `$C057` hi-res).
    hires: bool,
    /// 80COL (`$C00C` 40-column / `$C00D` 80-column).
    col80: bool,
    /// ALTCHARSET (`$C00E` primary / `$C00F` alternate).
    altcharset: bool,

    // --- Phase 3b: game-I/O buttons ---
    /// Push-button / paddle inputs, read at `$C061-$C063` (bit 7 = pressed).
    /// On the //e button 0 is Open-Apple and button 1 is Solid-Apple.
    buttons: [u8; 4],

    // --- Frontend bridge (for running under the SDL loop) ---
    /// Set when a display switch changes, so the renderer knows to redraw —
    /// the //e counterpart to `TwoIo::screen_dirty`.
    screen_dirty: bool,
    /// Cycle-stamped speaker toggles recorded on `$C030`, drained by the sound
    /// path — the //e counterpart to `TwoIo::speaker_toggles`.
    speaker_toggles: Vec<u64>,

    // --- Phase 4a: auxiliary memory ---
    /// Main and auxiliary RAM for `$0000-$BFFF` (48K each). The //e's `Memory`
    /// has no base-RAM fast path, so all low memory flows through here: reads
    /// of `$0200-$BFFF` follow RAMRD, writes follow RAMWRT. Zero page and the
    /// stack (`$0000-$01FF`) follow ALTZP (Phase 4b).
    main: Vec<u8>,
    aux: Vec<u8>,

    // --- Phase 4b: the built-in language card ($D000-$FFFF) ---
    // On the //e the language card is soldered onto the board and wired to the
    // MMU, so it lives here rather than in `Alc` (the ][+ peripheral card). Its
    // RAM has main + aux copies selected by ALTZP; the bank-switch mechanism
    // is the same two-reads-to-write-enable dance as the ][+ card.
    /// The 12K banked ROM at `$D000-$FFFF` (the fall-through when card RAM is
    /// not read-enabled).
    lc_rom: Vec<u8>,
    /// The two `$D000` banks (4K each) and the `$E000` bank (8K), in main and
    /// aux. `[bank][main=0/aux=1]`: `d[0]`/`d[1]` are the `$D000` RAM1/RAM2.
    lc_d1: [Vec<u8>; 2],
    lc_d2: [Vec<u8>; 2],
    lc_e: [Vec<u8>; 2],
    /// Card state, as in `Alc`.
    lc_active: bool,
    lc_bank1: bool,
    lc_read: bool,
    lc_write: bool,
    lc_wrtcount: u32,
}

impl IouE {
    fn new() -> IouE {
        IouE {
            key: 0,
            debug: false,
            intcxrom: false,
            slotc3rom: false,
            c800_internal: false,
            internal_rom: &ROM_IIE_CD[0x100..0x1000],
            slot_rom: [None; 8],
            store80: false,
            ramrd: false,
            ramwrt: false,
            altzp: false,
            // The //e powers up in 40-column text, page 1, primary char set.
            text: true,
            mixed: false,
            page2: false,
            hires: false,
            col80: false,
            altcharset: false,
            buttons: [0; 4],
            screen_dirty: true,
            speaker_toggles: Vec::new(),
            main: vec![0; 0xc000],
            aux: vec![0; 0xc000],
            // The banked $D000-$FFFF ROM: the CD half's upper 4K plus the EF
            // half, the same image the ][+ hands to `Alc`.
            lc_rom: [&ROM_IIE_CD[0x1000..0x2000], ROM_IIE_EF].concat(),
            lc_d1: [vec![0; 0x1000], vec![0; 0x1000]],
            lc_d2: [vec![0; 0x1000], vec![0; 0x1000]],
            lc_e: [vec![0; 0x2000], vec![0; 0x2000]],
            lc_active: false,
            lc_bank1: false,
            lc_read: false,
            lc_write: false,
            lc_wrtcount: 0,
        }
    }

    fn main_bank(&self) -> &[u8] {
        &self.main
    }

    fn aux_bank(&self) -> &[u8] {
        &self.aux
    }

    /// Read `$0000-$BFFF`: zero page and the stack (`$0000-$01FF`) follow
    /// ALTZP; `$0200-$BFFF` follows RAMRD.
    /// The 80STORE display-page override. When 80STORE is on, PAGE2 selects
    /// aux (on) or main (off) for text page 1 (`$0400-$07FF`) and — only when
    /// HIRES is also on — hi-res page 1 (`$2000-$3FFF`), regardless of
    /// RAMRD/RAMWRT. This sits *above* RAMRD/RAMWRT and, unlike them, uses the
    /// same PAGE2 selector for both reads and writes. `None` means no override
    /// applies, so the caller falls through to RAMRD/RAMWRT.
    fn store80_aux(&self, addr: u16) -> Option<bool> {
        if !self.store80 {
            return None;
        }
        let text_page1 = (0x0400..0x0800).contains(&addr);
        let hires_page1 = self.hires && (0x2000..0x4000).contains(&addr);
        (text_page1 || hires_page1).then_some(self.page2)
    }

    /// Read `$0000-$BFFF`: `$0000-$01FF` follows ALTZP, `$0200-$BFFF` RAMRD —
    /// unless the 80STORE display-page override claims the address.
    fn read_ram(&self, addr: u16) -> u8 {
        let i = addr as usize;
        let aux = if addr < 0x0200 {
            self.altzp
        } else {
            self.store80_aux(addr).unwrap_or(self.ramrd)
        };
        if aux { self.aux[i] } else { self.main[i] }
    }

    /// Write `$0000-$BFFF`: `$0000-$01FF` follows ALTZP, `$0200-$BFFF` RAMWRT —
    /// unless the 80STORE display-page override claims the address.
    fn write_ram(&mut self, addr: u16, b: u8) {
        let i = addr as usize;
        let aux = if addr < 0x0200 {
            self.altzp
        } else {
            self.store80_aux(addr).unwrap_or(self.ramwrt)
        };
        if aux {
            self.aux[i] = b;
        } else {
            self.main[i] = b;
        }
    }

    fn lc_select_banks(&mut self, addr: u16) {
        self.lc_active = true;
        self.lc_bank1 = addr & 0b0000_1000 != 0;
    }

    /// `$C080-$C08F` read: the two-reads-to-write-enable bank switching (the
    /// same sequence as `Alc::iom_read`).
    fn lc_iom_read(&mut self, addr: u16) -> u8 {
        self.lc_select_banks(addr);
        match addr & 0b0011 {
            0b00 => {
                self.lc_wrtcount = 0;
                self.lc_read = true;
                self.lc_write = false;
            }
            0b01 => {
                self.lc_wrtcount += 1;
                self.lc_read = false;
                if self.lc_wrtcount >= 2 {
                    self.lc_write = true;
                }
            }
            0b10 => {
                self.lc_wrtcount = 0;
                self.lc_read = false;
                self.lc_write = false;
            }
            _ => {
                self.lc_wrtcount += 1;
                self.lc_read = true;
                if self.lc_wrtcount >= 2 {
                    self.lc_write = true;
                }
            }
        }
        0
    }

    /// `$C080-$C08F` write: resets the write count and never enables writes.
    fn lc_iom_write(&mut self, addr: u16) {
        self.lc_select_banks(addr);
        match addr & 0b0011 {
            0b00 => {
                self.lc_wrtcount = 0;
                self.lc_read = true;
                self.lc_write = false;
            }
            0b01 => {
                self.lc_wrtcount = 0;
                self.lc_read = false;
            }
            0b10 => {
                self.lc_wrtcount = 0;
                self.lc_read = false;
                self.lc_write = false;
            }
            _ => {
                self.lc_wrtcount = 0;
                self.lc_read = true;
            }
        }
    }

    /// `$D000-$FFFF` read: the ALTZP-selected card RAM bank when read-enabled,
    /// else the banked ROM.
    fn lc_read(&self, addr: u16) -> u8 {
        if self.lc_active && self.lc_read {
            let z = self.altzp as usize;
            match addr {
                0xd000..=0xdfff => {
                    let bank = if self.lc_bank1 {
                        &self.lc_d1[z]
                    } else {
                        &self.lc_d2[z]
                    };
                    return bank[(addr - 0xd000) as usize];
                }
                0xe000..=0xffff => return self.lc_e[z][(addr - 0xe000) as usize],
                _ => {}
            }
        }
        self.lc_rom[(addr - 0xd000) as usize]
    }

    /// `$D000-$FFFF` write: to the ALTZP-selected card RAM bank when
    /// write-enabled; swallowed otherwise.
    fn lc_write(&mut self, addr: u16, b: u8) {
        if !self.lc_active || !self.lc_write {
            return;
        }
        let z = self.altzp as usize;
        match addr {
            0xd000..=0xdfff => {
                let bank = if self.lc_bank1 {
                    &mut self.lc_d1[z]
                } else {
                    &mut self.lc_d2[z]
                };
                bank[(addr - 0xd000) as usize] = b;
            }
            0xe000..=0xffff => self.lc_e[z][(addr - 0xe000) as usize] = b,
            _ => {}
        }
    }

    /// Install a peripheral card's `$Cn00-$CnFF` ROM image for slot `slot`.
    fn set_slot_rom(&mut self, slot: usize, rom: &'static [u8]) {
        self.slot_rom[slot] = Some(rom);
    }

    /// Set a display soft switch. On the //e the `$C050-$C057` switches toggle
    /// on *any* access, so this is called from both reads and writes.
    fn set_display_switch(&mut self, addr: u16) {
        match addr {
            SS_SCREEN_MODE_GRAPHICS => self.text = false, // $C050
            SS_SCREEN_MODE_TEXT => self.text = true,      // $C051
            SS_GRAPHICS_STYLE_FULL => self.mixed = false, // $C052
            SS_GRAPHICS_STYLE_MIXED => self.mixed = true, // $C053
            SS_SCREEN_PAGE1 => self.page2 = false,        // $C054
            SS_SCREEN_PAGE2 => self.page2 = true,         // $C055
            SS_GRAPHICS_MODE_LGR => self.hires = false,   // $C056
            SS_GRAPHICS_MODE_HGR => self.hires = true,    // $C057
            _ => {}
        }
        self.screen_dirty = true;
    }

    fn read_io(&mut self, addr: u16, cycles: u64) -> u8 {
        match addr {
            SS_KBD => self.key, // $C000 KBD: bit 7 = key-down
            SS_KBDSTRB => {
                // $C010 KBDSTRB / AKD: clear the strobe, report key-down.
                let down = self.key & 0x80;
                self.key &= 0x7f;
                down
            }

            // Display switches respond to reads as well as writes.
            SS_SCREEN_MODE_GRAPHICS..=SS_GRAPHICS_MODE_HGR => {
                self.set_display_switch(addr);
                0
            }

            // $C010-$C01F status reads: state in bit 7. RDLCBNK2/RDLCRAM
            // ($C011/$C012) are answered by the language card, which shadows
            // those two addresses (see new_2e).
            SS_RDRAMRD => (self.ramrd as u8) << 7,
            SS_RDRAMWRT => (self.ramwrt as u8) << 7,
            SS_RDCXROM => (self.intcxrom as u8) << 7,
            SS_RDALTZP => (self.altzp as u8) << 7,
            SS_RDC3ROM => (self.slotc3rom as u8) << 7,
            // Language-card status: bank 2 selected / card RAM read-enabled.
            SS_RDLCBNK2 => ((!self.lc_bank1) as u8) << 7,
            SS_RDLCRAM => (self.lc_read as u8) << 7,
            SS_RD80STORE => (self.store80 as u8) << 7,
            // RDVBL is not cycle-modelled (quirk #3); derive a plausible
            // toggling value from the cycle counter so VBL busy-waits progress.
            SS_RDVBL => (((cycles >> 14) & 1) as u8) << 7,
            SS_RDTEXT => (self.text as u8) << 7,
            SS_RDMIXED => (self.mixed as u8) << 7,
            SS_RDPAGE2 => (self.page2 as u8) << 7,
            SS_RDHIRES => (self.hires as u8) << 7,
            SS_RDALTCHAR => (self.altcharset as u8) << 7,
            SS_RD80COL => (self.col80 as u8) << 7,

            // Game-I/O buttons: Open-Apple ($C061), Solid-Apple ($C062), and
            // the shift-key mod ($C063). Bit 7 = pressed.
            SS_PB0 => self.buttons[0],
            SS_PB1 => self.buttons[1],
            SS_PB2 => self.buttons[2],

            // Speaker: any $C030 access toggles it; record the cycle stamp.
            SS_SPKR => {
                self.speaker_toggles.push(cycles);
                0
            }

            _ => {
                // Annunciators / DHIRES ($C058-$C05F) are 6a; paddles
                // ($C064-$C07F) are later.
                if self.debug {
                    eprintln!("[A2E] Unhandled read at ${addr:04X}");
                }
                0
            }
        }
    }

    /// Read `$C100-$CFFF`, arbitrating internal firmware vs slot-card ROM and
    /// maintaining the `$C800` expansion-ROM latch.
    fn read_cxrom(&mut self, addr: u16) -> u8 {
        let internal = |a: u16| self.internal_rom[(a - 0xc100) as usize];

        if self.intcxrom {
            return internal(addr); // internal everywhere, incl. $C800-$CFFF
        }

        match addr {
            // $CFFF also resets the expansion-ROM latch as a side effect.
            0xcfff => {
                let v = if self.c800_internal {
                    internal(addr)
                } else {
                    0
                };
                self.c800_internal = false;
                v
            }
            0xc800..=0xcffe => {
                if self.c800_internal {
                    internal(addr)
                } else {
                    0 // no peripheral card in EWM has a $C800 expansion ROM
                }
            }
            0xc300..=0xc3ff => {
                if self.slotc3rom {
                    0 // slot-3 card ROM — no slot-3 card in EWM
                } else {
                    // Internal 80-column firmware; touching it exposes the
                    // internal $C800 expansion ROM.
                    self.c800_internal = true;
                    internal(addr)
                }
            }
            0xc100..=0xc7ff => {
                let slot = ((addr >> 8) & 0x0f) as usize; // $Cn -> n
                self.slot_rom[slot].map_or(0, |rom| rom[(addr & 0xff) as usize])
            }
            _ => 0,
        }
    }
}

impl Device for IouE {
    fn read(&mut self, addr: u16, cycles: u64) -> u8 {
        match addr {
            0x0000..=0xbfff => self.read_ram(addr),
            0xc000..=0xc07f => self.read_io(addr, cycles),
            0xc080..=0xc08f => self.lc_iom_read(addr),
            0xc100..=0xcfff => self.read_cxrom(addr),
            0xd000..=0xffff => self.lc_read(addr),
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, b: u8, _cycles: u64) {
        match addr {
            0x0000..=0xbfff => self.write_ram(addr, b),
            0xc080..=0xc08f => self.lc_iom_write(addr),
            0xd000..=0xffff => self.lc_write(addr, b),

            // KBDSTRB clears the keyboard strobe on *any* access — the //e
            // firmware clears it with a write (STA $C010), not just a read.
            SS_KBDSTRB => self.key &= 0x7f,

            // $C000-$C00F memory switches, write-to-set. State only in 2c —
            // the aux-memory routing arrives in Phase 4.
            0xc000 => self.store80 = false,
            0xc001 => self.store80 = true,
            0xc002 => self.ramrd = false,
            0xc003 => self.ramrd = true,
            0xc004 => self.ramwrt = false,
            0xc005 => self.ramwrt = true,
            SS_SLOTCXROM => self.intcxrom = false, // $C006
            SS_INTCXROM => self.intcxrom = true,   // $C007
            0xc008 => self.altzp = false,
            0xc009 => self.altzp = true,
            SS_INTC3ROM => self.slotc3rom = false, // $C00A
            SS_SLOTC3ROM => self.slotc3rom = true, // $C00B
            0xc00c => self.col80 = false,
            0xc00d => self.col80 = true,
            0xc00e => self.altcharset = false,
            0xc00f => self.altcharset = true,

            // Display switches respond to writes as well as reads.
            SS_SCREEN_MODE_GRAPHICS..=SS_GRAPHICS_MODE_HGR => self.set_display_switch(addr),

            // $C100-$CFFF is ROM (writes swallowed); other $C0xx are 2c/later.
            _ => {
                if self.debug && (0xc000..=0xc07f).contains(&addr) {
                    eprintln!("[A2E] Unhandled write at ${addr:04X}");
                }
            }
        }
    }
}

/// The host-facing soft-switch and input surface shared by both machines' I/O
/// devices (`TwoIo` and `IouE`). `Two`'s public accessors delegate through a
/// single `switches()` / `switches_mut()` dispatch, so callers never branch on
/// the machine model. The `Device` (`read`/`write`) trait is the *bus* side;
/// this is the *host* side.
trait SoftSwitches {
    fn key(&self) -> u8;
    fn set_key(&mut self, key: u8);
    fn screen_mode(&self) -> ScreenMode;
    fn screen_graphics_mode(&self) -> GraphicsMode;
    fn screen_graphics_style(&self) -> GraphicsStyle;
    fn screen_page(&self) -> ScreenPage;
    fn alt_charset(&self) -> bool;
    fn screen_dirty(&self) -> bool;
    fn set_screen_dirty(&mut self, dirty: bool);
    fn set_button(&mut self, button: usize, state: u8);
    fn set_joystick(&mut self, joystick: Option<(i16, i16)>);
    fn drain_speaker_toggles(&mut self) -> Vec<u64>;
    fn set_debug(&mut self, debug: bool);
}

impl SoftSwitches for TwoIo {
    fn key(&self) -> u8 {
        self.key
    }
    fn set_key(&mut self, key: u8) {
        self.key = key | 0x80;
    }
    fn screen_mode(&self) -> ScreenMode {
        self.screen_mode
    }
    fn screen_graphics_mode(&self) -> GraphicsMode {
        self.screen_graphics_mode
    }
    fn screen_graphics_style(&self) -> GraphicsStyle {
        self.screen_graphics_style
    }
    fn screen_page(&self) -> ScreenPage {
        self.screen_page
    }
    fn alt_charset(&self) -> bool {
        false // the ][+ has no alternate character set
    }
    fn screen_dirty(&self) -> bool {
        self.screen_dirty
    }
    fn set_screen_dirty(&mut self, dirty: bool) {
        self.screen_dirty = dirty;
    }
    fn set_button(&mut self, button: usize, state: u8) {
        self.buttons[button] = state;
    }
    fn set_joystick(&mut self, joystick: Option<(i16, i16)>) {
        self.joystick = joystick;
    }
    fn drain_speaker_toggles(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.speaker_toggles)
    }
    fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }
}

impl SoftSwitches for IouE {
    fn key(&self) -> u8 {
        self.key
    }
    fn set_key(&mut self, key: u8) {
        self.key = key | 0x80;
    }
    fn screen_mode(&self) -> ScreenMode {
        if self.text {
            ScreenMode::Text
        } else {
            ScreenMode::Graphics
        }
    }
    fn screen_graphics_mode(&self) -> GraphicsMode {
        if self.hires {
            GraphicsMode::Hgr
        } else {
            GraphicsMode::Lgr
        }
    }
    fn screen_graphics_style(&self) -> GraphicsStyle {
        if self.mixed {
            GraphicsStyle::Mixed
        } else {
            GraphicsStyle::Full
        }
    }
    fn screen_page(&self) -> ScreenPage {
        if self.page2 {
            ScreenPage::Page2
        } else {
            ScreenPage::Page1
        }
    }
    fn alt_charset(&self) -> bool {
        self.altcharset
    }
    fn screen_dirty(&self) -> bool {
        self.screen_dirty
    }
    fn set_screen_dirty(&mut self, dirty: bool) {
        self.screen_dirty = dirty;
    }
    fn set_button(&mut self, button: usize, state: u8) {
        self.buttons[button] = state;
    }
    fn set_joystick(&mut self, _joystick: Option<(i16, i16)>) {
        // //e analog paddles / joystick are not modelled yet (a later phase).
    }
    fn drain_speaker_toggles(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.speaker_toggles)
    }
    fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }
}

/// Which soft-switch device backs this machine. The per-model `match` lives in
/// exactly one place — `Two::switches()` / `switches_mut()`, which return the
/// device as a `&dyn SoftSwitches` — so no host-facing accessor branches on the
/// model. (`ram`/`aux_ram` are the one exception: the ][+'s RAM lives in
/// `Memory`, not in `TwoIo`, so those stay a small direct match.)
#[derive(Clone, Copy)]
enum MachineIo {
    Plus(DeviceHandle<TwoIo>),
    E(DeviceHandle<IouE>),
}

pub struct Two {
    pub cpu: Cpu,
    model: TwoType,
    io: MachineIo,
    dsk: DeviceHandle<Dsk>,
    hdd: Option<DeviceHandle<Hdd>>,
    clk: DeviceHandle<Clk>,
}

impl Two {
    /// Construct a machine. The Apple ][+ is the `ewm_two_init` port; the
    /// Enhanced //e is the Phase 2 bring-up. The original NMOS Apple ][
    /// remains unsupported (quirk #4 in REWRITE.md).
    pub fn new(two_type: TwoType) -> Result<Two, String> {
        match two_type {
            TwoType::Apple2Plus => Ok(Two::new_2plus()),
            TwoType::Apple2E => Ok(Two::new_2e()),
            TwoType::Apple2 => Err(format!("unsupported machine type {two_type:?}")),
        }
    }

    /// Port of `ewm_two_init`: the Apple ][+.
    fn new_2plus() -> Two {
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

        // Slot 1 Thunderclock Plus: I/O ports plus its firmware ROM at $C100.
        // ProDOS finds it by the ID bytes and shows the host's date and time.
        let clk = mem.add_device(0xc090, 0xc09f, Clk::new());
        mem.add_rom(0xc100, CLK_ROM.to_vec());

        Two {
            cpu: Cpu::new(Model::M6502, mem),
            model: TwoType::Apple2Plus,
            io: MachineIo::Plus(io),
            dsk,
            hdd: None,
            clk,
        }
    }

    /// The Enhanced //e — a 65C02 with the //e system ROM, the `$C100-$CFFF`
    /// internal-vs-slot ROM arbitration, and (Phase 4a) auxiliary memory. All
    /// RAM below `$C000` lives in the `IouE`, so `Memory` is built with no
    /// base-RAM fast path.
    fn new_2e() -> Two {
        assert_eq!(ROM_IIE_CD.len(), 0x2000, "//e CD ROM half must be 8K");
        assert_eq!(ROM_IIE_EF.len(), 0x2000, "//e EF ROM half must be 8K");

        // No base-RAM fast path: the IouE owns main + aux RAM for $0000-$BFFF.
        let mut mem = Memory::new(0);

        // The IouE is the whole //e memory-management unit: the $0000-$BFFF
        // main/aux RAM, the $C000-$C07F soft switches, the $C080-$C08F +
        // $D000-$FFFF built-in language card (RAM banked with an aux copy per
        // ALTZP, ROM held internally), and the $C100-$CFFF ROM arbitration
        // (internal firmware vs the peripheral-slot ROMs it holds). The Disk II
        // / clock I/O devices stay separate below; the //e does not use `Alc`.
        let mut iou = IouE::new();
        iou.set_slot_rom(1, &CLK_ROM); // slot 1 Thunderclock Plus
        iou.set_slot_rom(6, &DSK_ROM); // slot 6 Disk II boot ROM
        let io = mem.add_device(0xc000, 0xc07f, iou);
        mem.map_device(io, 0xc080, 0xc08f); // language-card switches
        mem.map_device(io, 0xc100, 0xcfff); // $CX ROM

        let dsk = mem.add_device(0xc0e0, 0xc0ef, Dsk::new());
        let clk = mem.add_device(0xc090, 0xc09f, Clk::new());

        // Map the RAM and language-card ROM/RAM ranges last so the region walk
        // (newest-first) checks them first — zero page, the stack, the display
        // pages, and the $D000-$FFFF code space are the hottest on the bus.
        mem.map_device(io, 0xd000, 0xffff);
        mem.map_device(io, 0x0000, 0xbfff);

        Two {
            cpu: Cpu::new(Model::M65C02, mem),
            model: TwoType::Apple2E,
            io: MachineIo::E(io),
            dsk,
            hdd: None,
            clk,
        }
    }

    /// The machine variant this instance was constructed as.
    pub fn model(&self) -> TwoType {
        self.model
    }

    /// Mount a ProDOS block image (.hdv/.po) as a slot 7 hard drive: the
    /// card's I/O ports plus its boot/driver firmware ROM at $C700. The
    /// Autostart slot scan runs 7 before 6, so an attached drive boots
    /// before the Disk II.
    pub fn attach_hdd(&mut self, path: &str) -> Result<(), String> {
        let hdd = Hdd::new(path)?;
        self.hdd = Some(self.cpu.mem.add_device(0xc0f0, 0xc0ff, hdd));
        // The slot 7 boot/driver ROM at $C700 is a plain region on the ][+, but
        // the //e routes $C100-$CFFF through the IouE's ROM arbitration.
        match self.io {
            MachineIo::Plus(_) => self.cpu.mem.add_rom(0xc700, HDD_ROM.to_vec()),
            MachineIo::E(h) => self.cpu.mem.device_mut(h).set_slot_rom(7, &HDD_ROM),
        }
        Ok(())
    }

    pub fn hdd(&self) -> Option<&Hdd> {
        self.hdd.map(|h| self.cpu.mem.device(h))
    }

    /// The machine's soft-switch device as a `SoftSwitches` — the single point
    /// where the model dispatch lives, so the accessors below don't repeat it.
    fn switches(&self) -> &dyn SoftSwitches {
        match self.io {
            MachineIo::Plus(h) => self.cpu.mem.device(h),
            MachineIo::E(h) => self.cpu.mem.device(h),
        }
    }

    fn switches_mut(&mut self) -> &mut dyn SoftSwitches {
        match self.io {
            MachineIo::Plus(h) => self.cpu.mem.device_mut(h),
            MachineIo::E(h) => self.cpu.mem.device_mut(h),
        }
    }

    /// Enable the soft-switch catch-all's unexpected/unhandled read/write
    /// logging (`--debug`); see `notes/TOTAL_RECALL_WRITE_WARNINGS.md`. Applies
    /// to whichever soft-switch device backs this machine.
    pub fn set_debug(&mut self, debug: bool) {
        self.switches_mut().set_debug(debug);
    }

    /// Read access to the machine's main RAM for the renderers, which scan the
    /// text and hires pages directly (the C renderers read `cpu->ram`). On the
    /// //e this is the main bank; the display pages live there until 80STORE
    /// routing lands (Phase 4c).
    pub fn ram(&self) -> &[u8] {
        match self.io {
            MachineIo::Plus(_) => self.cpu.mem.ram(),
            MachineIo::E(h) => self.cpu.mem.device(h).main_bank(),
        }
    }

    /// The //e auxiliary RAM bank (`$0000-$BFFF`); empty on the ][+.
    pub fn aux_ram(&self) -> &[u8] {
        match self.io {
            MachineIo::Plus(_) => &[],
            MachineIo::E(h) => self.cpu.mem.device(h).aux_bank(),
        }
    }

    pub fn dsk(&self) -> &Dsk {
        self.cpu.mem.device(self.dsk)
    }

    pub fn dsk_mut(&mut self) -> &mut Dsk {
        self.cpu.mem.device_mut(self.dsk)
    }

    pub fn clk(&self) -> &Clk {
        self.cpu.mem.device(self.clk)
    }

    pub fn clk_mut(&mut self) -> &mut Clk {
        self.cpu.mem.device_mut(self.clk)
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
        self.switches_mut().set_key(key);
    }

    /// The keyboard latch, strobe bit included (the C `two->key`).
    pub fn key_register(&self) -> u8 {
        self.switches().key()
    }

    pub fn screen_mode(&self) -> ScreenMode {
        self.switches().screen_mode()
    }

    pub fn screen_graphics_mode(&self) -> GraphicsMode {
        self.switches().screen_graphics_mode()
    }

    pub fn screen_graphics_style(&self) -> GraphicsStyle {
        self.switches().screen_graphics_style()
    }

    pub fn screen_page(&self) -> ScreenPage {
        self.switches().screen_page()
    }

    /// ALTCHARSET state (`$C01E`): the //e alternate character set (lower case +
    /// MouseText). The ][+ has no alternate set, so this is always false there.
    pub fn alt_charset(&self) -> bool {
        self.switches().alt_charset()
    }

    pub fn screen_dirty(&self) -> bool {
        self.switches().screen_dirty()
    }

    pub fn set_screen_dirty(&mut self, dirty: bool) {
        self.switches_mut().set_screen_dirty(dirty);
    }

    /// Set a game-I/O button (Open-Apple = 0, Solid-Apple = 1 on the //e).
    pub fn set_button(&mut self, button: usize, state: u8) {
        self.switches_mut().set_button(button, state);
    }

    pub fn set_joystick(&mut self, joystick: Option<(i16, i16)>) {
        self.switches_mut().set_joystick(joystick);
    }

    /// Cycle-stamped speaker toggles recorded on `$C030` access since the
    /// last drain, for the frontend's sound path.
    pub fn drain_speaker_toggles(&mut self) -> Vec<u64> {
        self.switches_mut().drain_speaker_toggles()
    }

    /// Decode text page 1 (`$0400`, interleaved rows) into 24 lines of 40
    /// characters — the workhorse for the headless gates. On the //e the
    /// alternate character set (ALTCHARSET) is honored, so lower case is
    /// preserved instead of being folded to upper case.
    pub fn text_screen(&self) -> String {
        let ram = self.ram();
        let alt = self.alt_charset();
        let iie = matches!(self.io, MachineIo::E(_));
        let mut text = String::with_capacity(24 * 41);
        for row in 0..24 {
            let base = 0x400 + 0x80 * (row % 8) + 0x28 * (row / 8);
            for column in 0..40 {
                let code = ram[base + column];
                let ch = if iie {
                    screen_code_to_char_e(code, alt)
                } else {
                    screen_code_to_char(code)
                };
                text.push(ch);
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

/// //e screen-code decoding. With ALTCHARSET on, the alternate set carries
/// lower case (`$60-$7F` inverse, `$E0-$FF` normal), so the low seven bits map
/// straight to ASCII (control-range codes fold to `$40-$5F`); MouseText
/// (`$40-$5F`) has no text form and shows as its underlying `$40-$5F` glyph
/// letter. With ALTCHARSET off the primary set is upper case + symbols only,
/// identical to the ][+ decode.
fn screen_code_to_char_e(code: u8, altcharset: bool) -> char {
    if !altcharset {
        return screen_code_to_char(code);
    }
    let v = code & 0x7f;
    if v < 0x20 {
        (v | 0x40) as char
    } else {
        v as char
    }
}

// --- SDL frontend, the loop half of two.c ---

const STATUS_BAR_HEIGHT: u32 = 9; // logical pixels, scaled 3x like the C

// Emulation speeds offered by the command palette, in emulated CPU cycles
// per second: the Apple II's 1.023 MHz and the classic accelerator-card
// multiples (3.5x and 7x). The values are exact enough that the status bar's
// MHz readout matches the labels.
const SPEED_NORMAL: u32 = TWO_SPEED; // 1.023 MHz
const SPEED_FAST: u32 = 3_580_000; // 3.58 MHz
const SPEED_FASTER: u32 = 7_160_000; // 7.16 MHz

/// What palette command callbacks get to work with: the machine plus the
/// frontend state the commands mutate.
struct TwoCtx<'a> {
    two: &'a mut Two,
    paused: &'a mut bool,
    window: &'a mut sdl3::video::Window,
    /// Emulated CPU cycles per second driving the per-frame burst.
    speed: &'a mut u32,
    /// The audio path, so a speed change can rescale its cycle→sample
    /// mapping and keep the sound real-time (pitched up when accelerated).
    snd: &'a mut Option<Snd>,
}

type TwoAction = fn(&mut TwoCtx);

/// Palette action: switch the emulation speed, keeping the sound in step.
fn set_speed(ctx: &mut TwoCtx, hz: u32) {
    *ctx.speed = hz;
    if let Some(snd) = ctx.snd.as_mut() {
        snd.set_cpu_frequency(hz as u64);
    }
}

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
    eprintln!("  --model <2plus|2e> machine to emulate (default: 2plus)");
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
    model: TwoType,
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
            "--model" => match it.next().map(String::as_str) {
                Some("2plus" | "2+" | "][+" | "2") => options.model = TwoType::Apple2Plus,
                Some("2e" | "//e" | "iie") => options.model = TwoType::Apple2E,
                _ => {
                    usage();
                    return Err(1);
                }
            },
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
    let pad = sdl::window_padding();

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

    let title = match options.model {
        TwoType::Apple2E => "EWM v0.1 / Apple //e",
        _ => "EWM v0.1 / Apple ][+",
    };
    let window = video
        .window(title, 280 * 3 + 2 * pad, 192 * 3 + 2 * pad)
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

    // Logical units are window pixels: the screen texture is drawn at 3x
    // into an explicit rect, leaving pad window pixels around it.
    canvas
        .set_logical_size(
            SCR_WIDTH as u32 * 3 + 2 * pad,
            SCR_HEIGHT as u32 * 3 + 2 * pad,
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

    let mut two = match Two::new(options.model) {
        Ok(two) => two,
        Err(e) => {
            eprintln!("[TWO] Could not create the machine: {e}");
            return 1;
        }
    };
    two.set_debug(options.debug);

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
    // The //e presents a 560-wide frame (Phase 5a); the ][+ 280. The screen
    // texture is that wide and is nearest-stretched into the same on-screen
    // rect, so the window size is model-independent (//e pixels are half-width).
    let render_width = frame_width(two.model());
    let mut texture = texture_creator
        .create_texture_streaming(format, render_width as u32, SCR_HEIGHT as u32)
        .expect("Failed to create screen texture");
    // SDL3 defaults textures to linear filtering (SDL2 defaulted to nearest),
    // which blurs the upscaled low-res screen.
    texture.set_scale_mode(ScaleMode::Nearest);
    let mut bar_texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, STATUS_BAR_HEIGHT)
        .expect("Failed to create status bar texture");
    bar_texture.set_scale_mode(ScaleMode::Nearest);
    let mut tty_texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to create tty texture");
    tty_texture.set_blend_mode(BlendMode::Blend);
    tty_texture.set_scale_mode(ScaleMode::Nearest);

    // The command palette renders at window resolution, not the emulated 3x.
    let mut palette: Palette<TwoAction> = Palette::new(layout);
    let mut palette_visible = false;
    let mut palette_texture = texture_creator
        .create_texture_streaming(format, palette::WIDTH as u32, palette::MAX_HEIGHT as u32)
        .expect("Failed to create palette texture");
    palette_texture.set_scale_mode(ScaleMode::Nearest);

    let mut event_pump = context.event_pump().expect("Failed to get event pump");
    let frame_ms = (1000 / fps) as u64;
    let mut next_frame = sdl3::timer::ticks() + frame_ms;
    let mut phase: u32 = 1;
    let mut paused = false;
    let mut status_bar_visible = false;
    let mut frames: u32 = 0;
    // Emulated CPU speed, switchable from the command palette.
    let mut speed: u32 = SPEED_NORMAL;

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

                    // While the palette is open it owns the keyboard.
                    if palette_visible {
                        let action = if keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD)
                            && keycode == Keycode::K
                        {
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
                                let mut ctx = TwoCtx {
                                    two: &mut two,
                                    paused: &mut paused,
                                    window: canvas.window_mut(),
                                    speed: &mut speed,
                                    snd: &mut snd,
                                };
                                run(&mut ctx);
                            }
                            PaletteAction::None => {}
                        }
                        continue;
                    }

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
                                let _ = canvas.window_mut().set_size(
                                    SCR_WIDTH as u32 * 3 + 2 * pad,
                                    SCR_HEIGHT as u32 * 3 + 2 * pad + extra,
                                );
                                let _ = canvas.set_logical_size(
                                    SCR_WIDTH as u32 * 3 + 2 * pad,
                                    SCR_HEIGHT as u32 * 3 + 2 * pad + extra,
                                    SDL_RendererLogicalPresentation::LETTERBOX,
                                );
                            }
                            Keycode::K => {
                                // Commands are registered per activation so
                                // the labels reflect the current state.
                                palette.open();
                                palette.add_command(
                                    "Reset",
                                    (|ctx| {
                                        ctx.two.cpu.reset();
                                    }) as TwoAction,
                                );
                                palette
                                    .add_command(if paused { "Unpause" } else { "Pause" }, |ctx| {
                                        *ctx.paused = !*ctx.paused
                                    });
                                let fullscreen =
                                    canvas.window().fullscreen_state() == FullscreenType::True;
                                palette.add_command(
                                    if fullscreen {
                                        "Leave Full Screen"
                                    } else {
                                        "Enter Full Screen"
                                    },
                                    |ctx| {
                                        let on =
                                            ctx.window.fullscreen_state() == FullscreenType::True;
                                        let _ = ctx.window.set_fullscreen(!on);
                                    },
                                );
                                // Speed choices; the active one carries a check.
                                let speed_label = |hz: u32, text: &str| {
                                    if speed == hz {
                                        format!("{text}  \u{2713}")
                                    } else {
                                        text.to_string()
                                    }
                                };
                                palette.add_command(
                                    speed_label(SPEED_NORMAL, "Speed: 1.023 MHz (normal)"),
                                    (|ctx| set_speed(ctx, SPEED_NORMAL)) as TwoAction,
                                );
                                palette.add_command(
                                    speed_label(SPEED_FAST, "Speed: 3.58 MHz"),
                                    (|ctx| set_speed(ctx, SPEED_FAST)) as TwoAction,
                                );
                                palette.add_command(
                                    speed_label(SPEED_FASTER, "Speed: 7.16 MHz"),
                                    (|ctx| set_speed(ctx, SPEED_FASTER)) as TwoAction,
                                );
                                palette_visible = true;
                            }
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

                Event::TextInput { ref text, .. } => {
                    if palette_visible {
                        let _ = palette.handle_text(text);
                    } else if text.len() == 1 {
                        // The ][+ has no lower case, so its ROM expects
                        // upper-cased input; the //e passes lower case through.
                        let b = text.as_bytes()[0];
                        let b = if two.model() == TwoType::Apple2E {
                            b
                        } else {
                            b.to_ascii_uppercase()
                        };
                        two.key(b);
                    }
                }

                _ => {}
            }
        }

        if sdl3::timer::ticks() >= next_frame {
            if !paused && !palette_visible {
                // Feed the joystick axes to the paddle logic before the burst.
                two.set_joystick(
                    controller
                        .as_ref()
                        .map(|c| (c.axis(Axis::LeftX), c.axis(Axis::LeftY))),
                );

                let mut budget = (speed / fps) as i64;
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
                    .update(
                        None,
                        &pixels_to_bytes(scr.frame(two.model())),
                        render_width * 4,
                    )
                    .expect("Failed to update texture");
                let screen_dst = Rect::new(
                    pad as i32,
                    pad as i32,
                    SCR_WIDTH as u32 * 3,
                    SCR_HEIGHT as u32 * 3,
                );
                canvas
                    .copy(&texture, None, screen_dst)
                    .expect("Failed to copy texture");

                if status_bar_visible {
                    let bar = render_status_bar(scr.chr(), &two, mhz, layout);
                    bar_texture
                        .update(None, &pixels_to_bytes(&bar), TTY_PIXEL_WIDTH * 4)
                        .expect("Failed to update bar texture");
                    let dst = Rect::new(
                        pad as i32,
                        pad as i32 + SCR_HEIGHT as i32 * 3,
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
                    let _ = canvas.copy(&tty_texture, None, screen_dst);
                }

                if palette_visible {
                    palette.render();
                    palette_texture
                        .update(None, &pixels_to_bytes(&palette.pixels), palette::WIDTH * 4)
                        .expect("Failed to update palette texture");
                    let height = palette.height();
                    let src = Rect::new(0, 0, palette::WIDTH as u32, height as u32);
                    let window_width = SCR_WIDTH as i32 * 3 + 2 * pad as i32;
                    let dst = Rect::new(
                        (window_width - palette::WIDTH as i32) / 2,
                        40,
                        palette::WIDTH as u32,
                        height as u32,
                    );
                    let _ = canvas.copy(&palette_texture, src, dst);
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
            if phase == fps {
                phase = 0;

                // Cycles executed over the past second — the true rate, which
                // the palette's acceleration options make meaningful (at 1x it
                // is the fake ≈1.023 MHz of quirk #3).
                mhz = (two.cpu.counter - counter) as f64 / 1_000_000.0;
                counter = two.cpu.counter;
            }

            frames += 1;
            if let Some(path) = &options.screenshot
                && frames >= SCREENSHOT_FRAMES
            {
                let bmp = encode_bmp(scr.frame(two.model()), render_width, SCR_HEIGHT);
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
