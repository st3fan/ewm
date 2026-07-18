//! The Apple ][+: machine and SDL frontend, port of `two.c` — which, like
//! this file, held both `ewm_two_t` and the SDL loop. The machine composes
//! its hardware as memory regions (RAM, the `TwoIo` soft switches, the
//! language card, the Disk II and its slot ROM) and owns the CPU; the loop
//! runs fixed-step frames with the fake ≈1.023 MHz display preserved
//! (quirk #3).

use std::collections::BTreeMap;

use crate::alc::Alc;
use crate::aux::{AuxCard, Ext80Col, LcRegion};
use crate::clk::{Clk, clk_rom};
use crate::config;
use crate::dsk::{DSK_ROM, Dsk};
use crate::hdd::{Hdd, hdd_rom};
use crate::led::{LED_STRIP_HEIGHT, LED_STRIP_WIDTH, render_led_strip};
use crate::liron::{Liron, liron_rom};
use crate::palette::{self, Palette, PaletteAction, PaletteKey};
use crate::saturn::Saturn;
use crate::scr::{
    MonitorStyle, PixelLayout, SCR_HEIGHT, SCR_WIDTH, Scanlines, Scr, encode_bmp, frame_width,
    scanline_overlay,
};
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
/// The WozBug line server's default port. Of course.
const WOZBUG_DEFAULT_PORT: u16 = 6502;

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
// On the //e, $C05E/$C05F drive the DHIRES (double-resolution) switch directly
// via annunciator 3: $C05E clears AN3 -> DHIRES on, $C05F sets AN3 -> DHIRES
// off. (There is no IOUDIS gate; IOUDIS/RDIOUDIS/RDDHIRES at $C07E/$C07F are a
// //c feature the //e Technical Reference documents in error — verified
// floating on real //e hardware. See the AN3/DHIRES note in
// notes/APPLE_IIE_ENHANCED.md.)
const SS_DHIRES_ON: u16 = 0xc05e;
const SS_DHIRES_OFF: u16 = 0xc05f;
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
    /// (slot 0 unused); `None` when no card occupies the slot. Owned copies
    /// so per-slot generated firmware (clock, hard disk) can be installed.
    slot_rom: [Option<[u8; 256]>; 8],

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

    // --- Phase 6a: double-resolution control ---
    /// DHIRES: double-resolution enable, driven directly by annunciator 3 —
    /// `$C05E` (AN3 off) turns it on, `$C05F` (AN3 on) off. Combined with 80COL
    /// this gives double lo-res (LORES) or double hi-res (HIRES). Resets off
    /// (AN3 on). There is no IOUDIS switch on the //e.
    dhires: bool,

    // --- Phase 3b: game-I/O buttons ---
    /// Push-button / paddle inputs, read at `$C061-$C063` (bit 7 = pressed).
    /// On the //e button 0 is Open-Apple and button 1 is Solid-Apple.
    buttons: [u8; 4],
    /// Joystick axes (x, y) as raw SDL values, as in `TwoIo`; the paddle
    /// timers at `$C064`/`$C065` are armed by a `$C070` PTRIG read.
    joystick: Option<(i16, i16)>,
    padl0_time: u64,
    padl0_value: u8,
    padl1_time: u64,
    padl1_value: u8,

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
    /// The card in the auxiliary slot: the aux 48K body, the aux language
    /// card, and the 80STORE/video display pages all answer from it (see
    /// `crate::aux` — Extended 80-Column, plain 1K 80-Column, RamWorks III).
    aux: Box<dyn AuxCard>,

    // --- Phase 4b: the built-in language card ($D000-$FFFF) ---
    // On the //e the language card is soldered onto the board and wired to the
    // MMU, so it lives here rather than in `Alc` (the ][+ peripheral card). Its
    // RAM has main + aux copies selected by ALTZP; the bank-switch mechanism
    // is the same two-reads-to-write-enable dance as the ][+ card.
    /// The 12K banked ROM at `$D000-$FFFF` (the fall-through when card RAM is
    /// not read-enabled).
    lc_rom: Vec<u8>,
    /// The main-side card RAM: two `$D000` banks (4K each) and the `$E000`
    /// bank (8K). The ALTZP-selected aux copies live on the aux card.
    lc_d1: Vec<u8>,
    lc_d2: Vec<u8>,
    lc_e: Vec<u8>,
    /// Card state, as in `Alc`.
    lc_active: bool,
    lc_bank1: bool,
    lc_read: bool,
    lc_write: bool,
    lc_wrtcount: u32,
}

impl IouE {
    fn new(aux: Box<dyn AuxCard>) -> IouE {
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
            dhires: false, // AN3 resets on, so DHIRES is off
            buttons: [0; 4],
            joystick: None,
            padl0_time: 0,
            padl0_value: 0,
            padl1_time: 0,
            padl1_value: 0,
            screen_dirty: true,
            speaker_toggles: Vec::new(),
            main: vec![0; 0xc000],
            aux,
            // The banked $D000-$FFFF ROM: the CD half's upper 4K plus the EF
            // half, the same image the ][+ hands to `Alc`.
            lc_rom: [&ROM_IIE_CD[0x1000..0x2000], ROM_IIE_EF].concat(),
            lc_d1: vec![0; 0x1000],
            lc_d2: vec![0; 0x1000],
            lc_e: vec![0; 0x2000],
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
        self.aux.video_ram()
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
        if addr < 0x0200 {
            return if self.altzp {
                self.aux.read(addr)
            } else {
                self.main[i]
            };
        }
        match self.store80_aux(addr) {
            // The 80STORE display pages go to the card's video memory
            // (bank 0 on RamWorks; the 1K page on the plain 80-col card).
            Some(true) => self.aux.video_read(addr),
            Some(false) => self.main[i],
            None if self.ramrd => self.aux.read(addr),
            None => self.main[i],
        }
    }

    /// Write `$0000-$BFFF`: `$0000-$01FF` follows ALTZP, `$0200-$BFFF` RAMWRT —
    /// unless the 80STORE display-page override claims the address.
    fn write_ram(&mut self, addr: u16, b: u8) {
        let i = addr as usize;
        if addr < 0x0200 {
            if self.altzp {
                self.aux.write(addr, b);
            } else {
                self.main[i] = b;
            }
            return;
        }
        match self.store80_aux(addr) {
            Some(true) => self.aux.video_write(addr, b),
            Some(false) => self.main[i] = b,
            None if self.ramwrt => self.aux.write(addr, b),
            None => self.main[i] = b,
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
            let (region, offset) = self.lc_region(addr);
            return if self.altzp {
                self.aux.lc_read(region, offset)
            } else {
                self.lc_main(region)[offset]
            };
        }
        self.lc_rom[(addr - 0xd000) as usize]
    }

    /// Resolve a `$D000-$FFFF` address to a language-card region + offset,
    /// using the current `$D000` bank selection.
    fn lc_region(&self, addr: u16) -> (LcRegion, usize) {
        if addr < 0xe000 {
            let region = if self.lc_bank1 {
                LcRegion::Bank1
            } else {
                LcRegion::Bank2
            };
            (region, (addr - 0xd000) as usize)
        } else {
            (LcRegion::High, (addr - 0xe000) as usize)
        }
    }

    fn lc_main(&self, region: LcRegion) -> &[u8] {
        match region {
            LcRegion::Bank1 => &self.lc_d1,
            LcRegion::Bank2 => &self.lc_d2,
            LcRegion::High => &self.lc_e,
        }
    }

    /// `$D000-$FFFF` write: to the ALTZP-selected card RAM bank when
    /// write-enabled; swallowed otherwise.
    fn lc_write(&mut self, addr: u16, b: u8) {
        if !self.lc_active || !self.lc_write {
            return;
        }
        let (region, offset) = self.lc_region(addr);
        if self.altzp {
            self.aux.lc_write(region, offset, b);
        } else {
            match region {
                LcRegion::Bank1 => self.lc_d1[offset] = b,
                LcRegion::Bank2 => self.lc_d2[offset] = b,
                LcRegion::High => self.lc_e[offset] = b,
            }
        }
    }

    /// Install a peripheral card's `$Cn00-$CnFF` ROM image for slot `slot`.
    fn set_slot_rom(&mut self, slot: usize, rom: &[u8; 256]) {
        self.slot_rom[slot] = Some(*rom);
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

    /// Access `$C05E`/`$C05F` (on any read or write). On the //e these drive
    /// DHIRES directly through annunciator 3: `$C05E` clears AN3 → DHIRES on,
    /// `$C05F` sets AN3 → DHIRES off. (No IOUDIS gate — that is a //c switch the
    /// //e Tech Ref documents in error.)
    fn access_dhires(&mut self, addr: u16) {
        self.dhires = addr == SS_DHIRES_ON; // $C05E on, $C05F off
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
            // DHIRES ($C05E/$C05F) toggles on read as well as write.
            SS_DHIRES_ON | SS_DHIRES_OFF => {
                self.access_dhires(addr);
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
            // $C07E/$C07F (RDIOUDIS/RDDHIRES) do not exist on the //e — they
            // float; fall through to the open-bus default.

            // Game-I/O buttons: Open-Apple ($C061), Solid-Apple ($C062), and
            // the shift-key mod ($C063). Bit 7 = pressed.
            SS_PB0 => self.buttons[0],
            SS_PB1 => self.buttons[1],
            SS_PB2 => self.buttons[2],

            // Analog paddles, the same 558-timer model as the ][+ (`TwoIo`):
            // a PTRIG read arms the timers from the joystick axes; the PADL
            // reads report bit 7 until their timer expires.
            SS_PTRIG => {
                if let Some((axis_x, axis_y)) = self.joystick {
                    let x = 128 + (axis_x as i64 / 256);
                    self.padl0_time = cycles + (x as u64 * (2820 / 255));
                    self.padl0_value = 0xff;
                    let y = 128 + (axis_y as i64 / 256);
                    self.padl1_time = cycles + (y as u64 * (2820 / 255));
                    self.padl1_value = 0xff;
                }
                0
            }
            SS_PADL0 => {
                if self.padl0_time != 0 && cycles >= self.padl0_time {
                    self.padl0_time = 0;
                    self.padl0_value = 0;
                }
                self.padl0_value
            }
            SS_PADL1 => {
                // As in two.c, PADL1 never clears its timer.
                if self.padl1_time != 0 && cycles >= self.padl1_time {
                    self.padl1_value = 0;
                }
                self.padl1_value
            }

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
                self.slot_rom[slot]
                    .as_ref()
                    .map_or(0, |rom| rom[(addr & 0xff) as usize])
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
            // DHIRES ($C05E/$C05F). ($C07E/$C07F are inert — no IOUDIS on the //e.)
            SS_DHIRES_ON | SS_DHIRES_OFF => self.access_dhires(addr),

            // Aux-slot-visible register writes: the card decodes its own
            // (RamWorks III: the $C073 bank select).
            0xc070..=0xc07f => self.aux.io_write(addr, b),

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
    fn col80(&self) -> bool;
    fn dhires(&self) -> bool;
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
    fn col80(&self) -> bool {
        false // the ][+ has no 80-column mode
    }
    fn dhires(&self) -> bool {
        false // the ][+ has no double-resolution mode
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
    fn col80(&self) -> bool {
        self.col80
    }
    fn dhires(&self) -> bool {
        self.dhires
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

/// Which soft-switch device backs this machine. The per-model `match` lives in
/// exactly one place — `Two::switches()` / `switches_mut()`, which return the
/// device as a `&dyn SoftSwitches` — so no host-facing accessor branches on the
/// model. (`ram`/`aux_ram` are the one exception: the ][+'s RAM lives in
/// `Memory`, not in `TwoIo`, so those stay a small direct match.)
/// ][+ soft switches and game I/O (notes/STATE.md §5): the key latch, the
/// display switches, buttons, paddle timers, and any undrained speaker
/// toggles — cycle stamps saved verbatim, never rebased. Not written:
/// `joystick` (the frontend re-feeds it every frame), `screen_dirty`
/// (restore marks the screen dirty), and `debug` (config).
impl ewm_core::state::Persist for TwoIo {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.put_u8(self.key);
        w.put_u8(match self.screen_mode {
            ScreenMode::Text => 0,
            ScreenMode::Graphics => 1,
        });
        w.put_u8(match self.screen_graphics_mode {
            GraphicsMode::Lgr => 0,
            GraphicsMode::Hgr => 1,
        });
        w.put_u8(match self.screen_graphics_style {
            GraphicsStyle::Full => 0,
            GraphicsStyle::Mixed => 1,
        });
        w.put_u8(match self.screen_page {
            ScreenPage::Page1 => 0,
            ScreenPage::Page2 => 1,
        });
        w.put_bytes(&self.buttons);
        w.put_u64(self.padl0_time);
        w.put_u8(self.padl0_value);
        w.put_u64(self.padl1_time);
        w.put_u8(self.padl1_value);
        w.put_u16(self.speaker_toggles.len() as u16);
        for &t in &self.speaker_toggles {
            w.put_u64(t);
        }
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        use ewm_core::state::Error;

        self.key = r.get_u8()?;
        self.screen_mode = match r.get_u8()? {
            0 => ScreenMode::Text,
            1 => ScreenMode::Graphics,
            other => return Err(Error(format!("unknown screen mode {other}"))),
        };
        self.screen_graphics_mode = match r.get_u8()? {
            0 => GraphicsMode::Lgr,
            1 => GraphicsMode::Hgr,
            other => return Err(Error(format!("unknown graphics mode {other}"))),
        };
        self.screen_graphics_style = match r.get_u8()? {
            0 => GraphicsStyle::Full,
            1 => GraphicsStyle::Mixed,
            other => return Err(Error(format!("unknown graphics style {other}"))),
        };
        self.screen_page = match r.get_u8()? {
            0 => ScreenPage::Page1,
            1 => ScreenPage::Page2,
            other => return Err(Error(format!("unknown screen page {other}"))),
        };
        self.buttons.copy_from_slice(r.get_bytes(4)?);
        self.padl0_time = r.get_u64()?;
        self.padl0_value = r.get_u8()?;
        self.padl1_time = r.get_u64()?;
        self.padl1_value = r.get_u8()?;
        let toggles = r.get_u16()? as usize;
        self.speaker_toggles.clear();
        for _ in 0..toggles {
            self.speaker_toggles.push(r.get_u64()?);
        }
        self.screen_dirty = true;
        Ok(())
    }
}

/// //e MMU/IOU state (notes/STATE.md §5): the key latch, ROM arbitration,
/// the memory and display soft switches, game I/O, main 48K, the built-in
/// language card, and the aux card as a framed child (`AuxCard: Persist`).
/// Not written: the internal/slot ROMs and `lc_rom` (construction data),
/// `joystick`, `screen_dirty`, `debug` — same reasoning as `TwoIo`.
impl ewm_core::state::Persist for IouE {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.put_u8(self.key);
        w.put_bool(self.intcxrom);
        w.put_bool(self.slotc3rom);
        w.put_bool(self.c800_internal);
        w.put_bool(self.store80);
        w.put_bool(self.ramrd);
        w.put_bool(self.ramwrt);
        w.put_bool(self.altzp);
        w.put_bool(self.text);
        w.put_bool(self.mixed);
        w.put_bool(self.page2);
        w.put_bool(self.hires);
        w.put_bool(self.col80);
        w.put_bool(self.altcharset);
        w.put_bool(self.dhires);
        w.put_bytes(&self.buttons);
        w.put_u64(self.padl0_time);
        w.put_u8(self.padl0_value);
        w.put_u64(self.padl1_time);
        w.put_u8(self.padl1_value);
        w.put_u16(self.speaker_toggles.len() as u16);
        for &t in &self.speaker_toggles {
            w.put_u64(t);
        }
        w.put_blob(&self.main);
        w.put_blob(&self.lc_d1);
        w.put_blob(&self.lc_d2);
        w.put_blob(&self.lc_e);
        w.put_bool(self.lc_active);
        w.put_bool(self.lc_bank1);
        w.put_bool(self.lc_read);
        w.put_bool(self.lc_write);
        w.put_u32(self.lc_wrtcount);
        w.chunk(*b"AUX ", |w| self.aux.save(w));
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        self.key = r.get_u8()?;
        self.intcxrom = r.get_bool()?;
        self.slotc3rom = r.get_bool()?;
        self.c800_internal = r.get_bool()?;
        self.store80 = r.get_bool()?;
        self.ramrd = r.get_bool()?;
        self.ramwrt = r.get_bool()?;
        self.altzp = r.get_bool()?;
        self.text = r.get_bool()?;
        self.mixed = r.get_bool()?;
        self.page2 = r.get_bool()?;
        self.hires = r.get_bool()?;
        self.col80 = r.get_bool()?;
        self.altcharset = r.get_bool()?;
        self.dhires = r.get_bool()?;
        self.buttons.copy_from_slice(r.get_bytes(4)?);
        self.padl0_time = r.get_u64()?;
        self.padl0_value = r.get_u8()?;
        self.padl1_time = r.get_u64()?;
        self.padl1_value = r.get_u8()?;
        let toggles = r.get_u16()? as usize;
        self.speaker_toggles.clear();
        for _ in 0..toggles {
            self.speaker_toggles.push(r.get_u64()?);
        }
        crate::alc::restore_ram(&mut self.main, r, "//e main RAM")?;
        crate::alc::restore_ram(&mut self.lc_d1, r, "//e LC bank 1")?;
        crate::alc::restore_ram(&mut self.lc_d2, r, "//e LC bank 2")?;
        crate::alc::restore_ram(&mut self.lc_e, r, "//e LC high")?;
        self.lc_active = r.get_bool()?;
        self.lc_bank1 = r.get_bool()?;
        self.lc_read = r.get_bool()?;
        self.lc_write = r.get_bool()?;
        self.lc_wrtcount = r.get_u32()?;
        let mut aux = r.chunk(*b"AUX ")?;
        self.aux.restore(&mut aux)?;
        aux.done()?;
        self.screen_dirty = true;
        Ok(())
    }
}

/// The machine root (notes/STATE.md §3.3): a small INFO chunk naming the
/// model — the cheap seatbelt edge of the same-configuration precondition,
/// ahead of the backlog fingerprint — then the CPU as a framed child, which
/// carries everything else (Memory owns all devices). `Two`'s other fields
/// are construction-time wiring with no runtime state of their own.
impl ewm_core::state::Persist for Two {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.chunk(*b"INFO", |w| {
            w.put_str(match self.model {
                TwoType::Apple2 => "2",
                TwoType::Apple2Plus => "2plus",
                TwoType::Apple2E => "2e",
            });
        });
        w.chunk(*b"CPU ", |w| self.cpu.save(w));
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        let mut info = r.chunk(*b"INFO")?;
        let model = info.get_str()?;
        info.done()?;
        let ours = match self.model {
            TwoType::Apple2 => "2",
            TwoType::Apple2Plus => "2plus",
            TwoType::Apple2E => "2e",
        };
        if model != ours {
            return Err(ewm_core::state::Error(format!(
                "state was saved by a {model} machine, this is a {ours} \
                 (same-configuration precondition, notes/STATE.md)"
            )));
        }
        let mut cpu = r.chunk(*b"CPU ")?;
        self.cpu.restore(&mut cpu)?;
        cpu.done()?;
        // Everything derived re-derives: force a full redraw.
        self.set_screen_dirty(true);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum MachineIo {
    Plus(DeviceHandle<TwoIo>),
    E(DeviceHandle<IouE>),
}

/// What occupies a peripheral slot at construction time. Media (disk and
/// block images) is inserted afterwards, per slot — see `Two::load_disk_at`
/// and `Two::attach_hdd_at` (the hard-drive card is not listed here because
/// `Hdd::new` needs its image up front).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotDevice {
    DiskII,
    Thunderclock,
    /// The UniDisk 3.5 Controller ("Liron"): two SmartPort 3.5" drives
    /// taking .2mg images, mounted after construction with `load_2mg_at`.
    Liron,
}

/// What occupies the ][+ slot 0 socket — the memory-expansion slot, which
/// has no `$Cn00` firmware space and so takes only bankable-RAM cards.
/// The //e has no slot 0; its language card is soldered onto the board.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot0 {
    /// The 16K Apple Language Card (the classic 64K build).
    Language,
    /// The Saturn Systems 128K RAM Board — eight LC-compatible 16K banks.
    Saturn128,
    /// Nothing: a stock 48K machine with the motherboard ROM on the bus.
    Empty,
}

/// The classic layout the machine ships with when no slot table is given:
/// a Thunderclock Plus in slot 1 and a Disk II controller in slot 6.
pub fn default_slots() -> BTreeMap<u8, SlotDevice> {
    BTreeMap::from([(1, SlotDevice::Thunderclock), (6, SlotDevice::DiskII)])
}

/// A slot's 16-byte DEVSEL I/O range starts here ($C080 + slot*16).
fn slot_io_base(slot: u8) -> u16 {
    0xc080 + slot as u16 * 16
}

/// A slot's 256-byte firmware ROM page ($Cn00).
fn slot_rom_base(slot: u8) -> u16 {
    (0xc0 + slot as u16) << 8
}

pub struct Two {
    pub cpu: Cpu,
    model: TwoType,
    io: MachineIo,
    /// Disk II controllers by slot; the highest slot is the boot controller
    /// (the Autostart scan runs 7 down to 1, as on hardware).
    dsks: BTreeMap<u8, DeviceHandle<Dsk>>,
    /// Hard-drive cards by slot.
    hdds: BTreeMap<u8, DeviceHandle<Hdd>>,
    /// UniDisk 3.5 controllers by slot.
    lirons: BTreeMap<u8, DeviceHandle<Liron>>,
    clk: Option<(u8, DeviceHandle<Clk>)>,
    /// The ][+ slot 0 socket (`Slot0::Empty` = a 48K machine). The //e
    /// records `Language` — its card is soldered onto the board.
    slot0: Slot0,
    /// The Saturn board, when `slot0` is `Saturn128`.
    saturn: Option<DeviceHandle<Saturn>>,
}

impl Two {
    /// Construct a machine with the default slot table. The Apple ][+ is the
    /// `ewm_two_init` port; the Enhanced //e is the Phase 2 bring-up. The
    /// original NMOS Apple ][ remains unsupported (quirk #4 in REWRITE.md).
    pub fn new(two_type: TwoType) -> Result<Two, String> {
        Two::new_with_aux(two_type, None)
    }

    /// Construct a machine with an explicit auxiliary-slot card (the //e
    /// only; `None` = the default Extended 80-Column Text Card) and the
    /// default slot table.
    pub fn new_with_aux(two_type: TwoType, aux: Option<Box<dyn AuxCard>>) -> Result<Two, String> {
        Two::new_with_slots(two_type, aux, Slot0::Language, &default_slots())
    }

    /// Construct a machine from a slot table: each entry puts that card's
    /// I/O device in its slot's DEVSEL range and its firmware ROM at $Cn00;
    /// slots absent from the table stay empty (their ranges read $00, which
    /// the Autostart slot scan skips). The ][+ has no auxiliary slot, so
    /// requesting an aux card there is an error. `slot0` is the ][+
    /// memory-expansion socket (`Empty` = a 48K machine with the
    /// motherboard ROM directly at $D000-$FFFF); the //e ignores it — its
    /// language card is soldered onto the board.
    pub fn new_with_slots(
        two_type: TwoType,
        aux: Option<Box<dyn AuxCard>>,
        slot0: Slot0,
        slots: &BTreeMap<u8, SlotDevice>,
    ) -> Result<Two, String> {
        if let Some(slot) = slots.keys().find(|s| !(1..=7).contains(*s)) {
            return Err(format!("no such slot {slot} (slots are 1 through 7)"));
        }
        let clocks = slots
            .values()
            .filter(|&&c| c == SlotDevice::Thunderclock)
            .count();
        if clocks > 1 {
            return Err("at most one Thunderclock".to_string());
        }
        match two_type {
            TwoType::Apple2Plus => {
                if let Some(card) = aux {
                    return Err(format!(
                        "the Apple ][+ has no auxiliary slot (--aux {})",
                        card.label()
                    ));
                }
                Ok(Two::new_2plus(slot0, slots))
            }
            TwoType::Apple2E => Ok(Two::new_2e(
                aux.unwrap_or_else(|| Box::new(Ext80Col::new())),
                slots,
            )),
            TwoType::Apple2 => Err(format!("unsupported machine type {two_type:?}")),
        }
    }

    /// Port of `ewm_two_init`: the Apple ][+.
    fn new_2plus(slot0: Slot0, slots: &BTreeMap<u8, SlotDevice>) -> Two {
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
        // The slot 0 card shadows the machine ROM, so it owns it and covers
        // both its $C08x switches and the whole $D000-$FFFF bank space.
        let mut saturn = None;
        match slot0 {
            Slot0::Language => {
                let alc = mem.add_device(0xc080, 0xc08f, Alc::new(rom));
                mem.map_device(alc, 0xd000, 0xffff);
            }
            Slot0::Saturn128 => {
                let card = mem.add_device(0xc080, 0xc08f, Saturn::new(rom));
                mem.map_device(card, 0xd000, 0xffff);
                saturn = Some(card);
            }
            Slot0::Empty => {
                // The 48K machine: motherboard ROM directly on the bus and
                // slot 0's DEVSEL range unmapped (reads $00).
                mem.add_rom(0xd000, rom);
            }
        }

        // The peripheral cards: each slot's I/O device in its DEVSEL range,
        // its firmware as a plain ROM region at $Cn00.
        let mut dsks = BTreeMap::new();
        let mut lirons = BTreeMap::new();
        let mut clk = None;
        for (&slot, &card) in slots {
            let base = slot_io_base(slot);
            match card {
                SlotDevice::DiskII => {
                    dsks.insert(slot, mem.add_device(base, base + 0xf, Dsk::new()));
                    mem.add_rom(slot_rom_base(slot), DSK_ROM.to_vec());
                }
                SlotDevice::Thunderclock => {
                    // ProDOS finds the clock by its ID bytes and shows the
                    // host's date and time.
                    clk = Some((slot, mem.add_device(base, base + 0xf, Clk::new())));
                    mem.add_rom(slot_rom_base(slot), clk_rom(slot).to_vec());
                }
                SlotDevice::Liron => {
                    lirons.insert(slot, mem.add_device(base, base + 0xf, Liron::new()));
                    mem.add_rom(slot_rom_base(slot), liron_rom(slot).to_vec());
                }
            }
        }

        Two {
            cpu: Cpu::new(Model::M6502, mem),
            model: TwoType::Apple2Plus,
            io: MachineIo::Plus(io),
            dsks,
            hdds: BTreeMap::new(),
            lirons,
            clk,
            slot0,
            saturn,
        }
    }

    /// The Enhanced //e — a 65C02 with the //e system ROM, the `$C100-$CFFF`
    /// internal-vs-slot ROM arbitration, and (Phase 4a) auxiliary memory. All
    /// RAM below `$C000` lives in the `IouE`, so `Memory` is built with no
    /// base-RAM fast path.
    fn new_2e(aux: Box<dyn AuxCard>, slots: &BTreeMap<u8, SlotDevice>) -> Two {
        assert_eq!(ROM_IIE_CD.len(), 0x2000, "//e CD ROM half must be 8K");
        assert_eq!(ROM_IIE_EF.len(), 0x2000, "//e EF ROM half must be 8K");

        // No base-RAM fast path: the IouE owns main + aux RAM for $0000-$BFFF.
        let mut mem = Memory::new(0);

        // The IouE is the whole //e memory-management unit: the $0000-$BFFF
        // main/aux RAM, the $C000-$C07F soft switches, the $C080-$C08F +
        // $D000-$FFFF built-in language card (RAM banked with an aux copy per
        // ALTZP, ROM held internally), and the $C100-$CFFF ROM arbitration
        // (internal firmware vs the peripheral-slot ROMs it holds). The
        // peripheral I/O devices stay separate below; the //e does not use
        // `Alc`.
        let mut iou = IouE::new(aux);
        for (&slot, &card) in slots {
            match card {
                SlotDevice::DiskII => iou.set_slot_rom(slot as usize, &DSK_ROM),
                SlotDevice::Thunderclock => iou.set_slot_rom(slot as usize, &clk_rom(slot)),
                SlotDevice::Liron => iou.set_slot_rom(slot as usize, &liron_rom(slot)),
            }
        }
        let io = mem.add_device(0xc000, 0xc07f, iou);
        mem.map_device(io, 0xc080, 0xc08f); // language-card switches
        mem.map_device(io, 0xc100, 0xcfff); // $CX ROM

        let mut dsks = BTreeMap::new();
        let mut lirons = BTreeMap::new();
        let mut clk = None;
        for (&slot, &card) in slots {
            let base = slot_io_base(slot);
            match card {
                SlotDevice::DiskII => {
                    dsks.insert(slot, mem.add_device(base, base + 0xf, Dsk::new()));
                }
                SlotDevice::Thunderclock => {
                    clk = Some((slot, mem.add_device(base, base + 0xf, Clk::new())));
                }
                SlotDevice::Liron => {
                    lirons.insert(slot, mem.add_device(base, base + 0xf, Liron::new()));
                }
            }
        }

        // Map the RAM and language-card ROM/RAM ranges last so the region walk
        // (newest-first) checks them first — zero page, the stack, the display
        // pages, and the $D000-$FFFF code space are the hottest on the bus.
        mem.map_device(io, 0xd000, 0xffff);
        mem.map_device(io, 0x0000, 0xbfff);

        Two {
            cpu: Cpu::new(Model::M65C02, mem),
            model: TwoType::Apple2E,
            io: MachineIo::E(io),
            dsks,
            hdds: BTreeMap::new(),
            lirons,
            clk,
            slot0: Slot0::Language,
            saturn: None,
        }
    }

    /// The machine variant this instance was constructed as.
    pub fn model(&self) -> TwoType {
        self.model
    }

    /// The slot 0 socket (slot 0 on the ][+; the //e reports `Language`,
    /// its card being built in).
    pub fn slot0(&self) -> Slot0 {
        self.slot0
    }

    /// The Saturn board's selected 16K bank, when one is installed.
    pub fn saturn_bank(&self) -> Option<usize> {
        self.saturn.map(|h| self.cpu.mem.device(h).bank())
    }

    /// Mount a ProDOS block image (.hdv/.po) as a slot 7 hard drive. The
    /// Autostart slot scan runs 7 before 6, so an attached drive boots
    /// before the Disk II.
    pub fn attach_hdd(&mut self, path: &str) -> Result<(), String> {
        self.attach_hdd_at(7, path)
    }

    /// Mount a ProDOS block image (.hdv/.po) as a hard drive in the given
    /// slot: the card's I/O ports in its DEVSEL range plus its boot/driver
    /// firmware ROM at $Cn00.
    pub fn attach_hdd_at(&mut self, slot: u8, path: &str) -> Result<(), String> {
        if !(1..=7).contains(&slot) {
            return Err(format!("no such slot {slot} (slots are 1 through 7)"));
        }
        if self.slot_occupied(slot) {
            return Err(format!("slot {slot} is already occupied"));
        }
        let hdd = Hdd::new(path)?;
        let base = slot_io_base(slot);
        let handle = self.cpu.mem.add_device(base, base + 0xf, hdd);
        self.hdds.insert(slot, handle);
        // The boot/driver ROM at $Cn00 is a plain region on the ][+, but the
        // //e routes $C100-$CFFF through the IouE's ROM arbitration.
        match self.io {
            MachineIo::Plus(_) => self
                .cpu
                .mem
                .add_rom(slot_rom_base(slot), hdd_rom(slot).to_vec()),
            MachineIo::E(h) => self
                .cpu
                .mem
                .device_mut(h)
                .set_slot_rom(slot as usize, &hdd_rom(slot)),
        }
        Ok(())
    }

    fn slot_occupied(&self, slot: u8) -> bool {
        self.dsks.contains_key(&slot)
            || self.hdds.contains_key(&slot)
            || self.lirons.contains_key(&slot)
            || self.clk.is_some_and(|(s, _)| s == slot)
    }

    /// The highest-slot hard drive, if any.
    pub fn hdd(&self) -> Option<&Hdd> {
        self.hdds
            .values()
            .next_back()
            .map(|&h| self.cpu.mem.device(h))
    }

    /// The hard drive in the given slot, if any.
    pub fn hdd_at(&self, slot: u8) -> Option<&Hdd> {
        self.hdds.get(&slot).map(|&h| self.cpu.mem.device(h))
    }

    /// The slot holding the Thunderclock, if any.
    pub fn clock_slot(&self) -> Option<u8> {
        self.clk.map(|(slot, _)| slot)
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

    /// Save the whole machine to a state file, atomically (`--state`,
    /// notes/STATE.md §6).
    pub fn save_state(&self, path: &str) -> Result<(), String> {
        use ewm_core::state::Persist;
        let mut w = ewm_core::state::Writer::new();
        self.save(&mut w);
        ewm_core::state::write_file(path, w).map_err(|e| e.to_string())
    }

    /// Restore the whole machine from a state file, replacing the initial
    /// reset. All-or-nothing: on `Err` the machine must not be run.
    pub fn restore_state(&mut self, path: &str) -> Result<(), String> {
        use ewm_core::state::Persist;
        let bytes = ewm_core::state::read_file(path).map_err(|e| e.to_string())?;
        let mut r = ewm_core::state::Reader::new(&bytes);
        self.restore(&mut r).map_err(|e| e.to_string())?;
        r.done().map_err(|e| e.to_string())
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

    /// The slot of the boot Disk II controller — the one the Autostart scan
    /// reaches first (highest slot).
    pub fn boot_disk_slot(&self) -> Option<u8> {
        self.dsks.keys().next_back().copied()
    }

    /// The boot Disk II controller. Panics when the machine was built
    /// without one — use `boot_disk_slot()` / `dsk_at()` on machines with
    /// arbitrary slot tables.
    pub fn dsk(&self) -> &Dsk {
        let (_, &h) = self
            .dsks
            .iter()
            .next_back()
            .expect("no Disk II controller in this machine");
        self.cpu.mem.device(h)
    }

    /// See `dsk()`.
    pub fn dsk_mut(&mut self) -> &mut Dsk {
        let (_, &h) = self
            .dsks
            .iter()
            .next_back()
            .expect("no Disk II controller in this machine");
        self.cpu.mem.device_mut(h)
    }

    /// The Disk II controller in the given slot, if any.
    pub fn dsk_at(&self, slot: u8) -> Option<&Dsk> {
        self.dsks.get(&slot).map(|&h| self.cpu.mem.device(h))
    }

    /// The Thunderclock. Panics when the machine was built without one.
    pub fn clk(&self) -> &Clk {
        let (_, h) = self.clk.expect("no Thunderclock in this machine");
        self.cpu.mem.device(h)
    }

    /// See `clk()`.
    pub fn clk_mut(&mut self) -> &mut Clk {
        let (_, h) = self.clk.expect("no Thunderclock in this machine");
        self.cpu.mem.device_mut(h)
    }

    /// Port of `ewm_two_load_disk`: insert a disk into the boot controller.
    pub fn load_disk(&mut self, drive: usize, path: &str) -> Result<(), String> {
        let Some(slot) = self.boot_disk_slot() else {
            return Err("no Disk II controller in this machine".to_string());
        };
        self.load_disk_at(slot, drive, path)
    }

    /// Insert a disk into the controller in the given slot.
    pub fn load_disk_at(&mut self, slot: u8, drive: usize, path: &str) -> Result<(), String> {
        let Some(&h) = self.dsks.get(&slot) else {
            return Err(format!("no Disk II controller in slot {slot}"));
        };
        self.cpu.mem.device_mut(h).set_disk_file(drive, false, path)
    }

    /// Mount a .2mg image (400K or 800K) in a UniDisk 3.5 drive (0 or 1).
    pub fn load_2mg_at(&mut self, slot: u8, drive: usize, path: &str) -> Result<(), String> {
        let Some(&h) = self.lirons.get(&slot) else {
            return Err(format!("no UniDisk 3.5 controller in slot {slot}"));
        };
        self.cpu.mem.device_mut(h).load(drive, path)
    }

    /// The UniDisk 3.5 controller in a slot, if any.
    pub fn liron_at(&self, slot: u8) -> Option<&Liron> {
        self.lirons.get(&slot).map(|&h| self.cpu.mem.device(h))
    }

    /// The two drive lights, OR'ed across every Disk II controller — at any
    /// moment at most one controller is spinning, so the pair reads as "the
    /// active controller's drives". `[false; 2]` on a diskless machine.
    pub fn drive_lights(&self, cycles: u64) -> [bool; 2] {
        let mut lights = [false; 2];
        for &h in self.dsks.values() {
            let dsk: &Dsk = self.cpu.mem.device(h);
            for (i, light) in lights.iter_mut().enumerate() {
                *light |= dsk.drive_lit(i, cycles);
            }
        }
        lights
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

    /// 80COL state (`$C01F`): the //e 80-column display. Always false on the ][+.
    pub fn col80(&self) -> bool {
        self.switches().col80()
    }

    /// DHIRES state (`$C05E`/`$C05F` under IOUDIS): double-resolution enable.
    /// Always false on the ][+.
    pub fn dhires(&self) -> bool {
        self.switches().dhires()
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

    /// Decode //e 80-column text page 1 into 24 lines of 80 characters. The
    /// display interleaves the two banks: aux holds the even columns
    /// (0, 2, …, 78), main the odd (1, 3, …, 79), each bank contributing byte
    /// `base + column/2` of the row. The headless workhorse for the 80-column
    /// gates; only meaningful on the //e.
    pub fn text_screen_80(&self) -> String {
        let main = self.ram();
        let aux = self.aux_ram();
        let alt = self.alt_charset();
        let mut text = String::with_capacity(24 * 81);
        for row in 0..24 {
            let base = 0x400 + 0x80 * (row % 8) + 0x28 * (row / 8);
            for column in 0..80 {
                let bank = if column % 2 == 0 { aux } else { main };
                let code = bank[base + column / 2];
                text.push(screen_code_to_char_e(code, alt));
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
    /// The monitor style; the loop applies changes to the renderer.
    monitor: &'a mut MonitorStyle,
    /// The scanline effect; the loop rebuilds the overlay texture on change.
    scanlines: &'a mut Scanlines,
    /// Set by a "…: choice" row to reopen the palette as that submenu.
    open_submenu: &'a mut Option<Submenu>,
    /// Set by the "Reboot (Power off/on)" row; the frame loop performs the
    /// power cycle (it owns the machine and the options).
    reboot: &'a mut bool,
}

/// The palette's choice submenus (VS Code quick-pick style).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Submenu {
    MonitorStyle,
    Scanlines,
    Controller,
    Speed,
}

/// A palette command's action: either a plain callback, or a data-carrying
/// choice (fn pointers cannot capture which row was picked).
#[derive(Clone, Copy)]
enum TwoAction {
    Run(fn(&mut TwoCtx)),
    /// Make the gamepad with this joystick id the active controller.
    PickController(u32),
}

/// Palette action: switch the emulation speed, keeping the sound in step.
fn set_speed(ctx: &mut TwoCtx, hz: u32) {
    *ctx.speed = hz;
    if let Some(snd) = ctx.snd.as_mut() {
        snd.set_cpu_frequency(hz as u64);
    }
}

/// A choice label with a VS Code-style check mark on the active option.
fn choice_label(text: &str, active: bool) -> String {
    if active {
        format!("{text}  \u{2713}")
    } else {
        text.to_string()
    }
}

/// The human-readable label for an emulation speed: the MHz readout with a
/// parenthetical name for the accelerator preset.
fn speed_label(hz: u32) -> &'static str {
    match hz {
        SPEED_FAST => "3.58 MHz (Fast)",
        SPEED_FASTER => "7.16 MHz (Faster)",
        _ => "1.023 MHz (Normal)",
    }
}

/// Register the speed submenu: one row per preset, the active one checked,
/// picked exactly like a VS Code quick-pick choice.
fn add_speed_commands(palette: &mut Palette<TwoAction>, speed: u32) {
    palette.add_command(
        choice_label(speed_label(SPEED_NORMAL), speed == SPEED_NORMAL),
        TwoAction::Run(|ctx| set_speed(ctx, SPEED_NORMAL)),
    );
    palette.add_command(
        choice_label(speed_label(SPEED_FAST), speed == SPEED_FAST),
        TwoAction::Run(|ctx| set_speed(ctx, SPEED_FAST)),
    );
    palette.add_command(
        choice_label(speed_label(SPEED_FASTER), speed == SPEED_FASTER),
        TwoAction::Run(|ctx| set_speed(ctx, SPEED_FASTER)),
    );
}

/// Register the scanline submenu: one row per level, the active one checked.
fn add_scanline_commands(palette: &mut Palette<TwoAction>, scanlines: Scanlines) {
    palette.add_command(
        choice_label("Off", scanlines == Scanlines::Off),
        TwoAction::Run(|ctx| *ctx.scanlines = Scanlines::Off),
    );
    palette.add_command(
        choice_label("Light", scanlines == Scanlines::Light),
        TwoAction::Run(|ctx| *ctx.scanlines = Scanlines::Light),
    );
    palette.add_command(
        choice_label("Heavy", scanlines == Scanlines::Heavy),
        TwoAction::Run(|ctx| *ctx.scanlines = Scanlines::Heavy),
    );
}

/// Register the monitor-style submenu: one row per style, the active one
/// checked, picked exactly like a VS Code quick-pick choice.
fn add_monitor_style_commands(palette: &mut Palette<TwoAction>, monitor: MonitorStyle) {
    palette.add_command(
        choice_label("Green", monitor == MonitorStyle::Green),
        TwoAction::Run(|ctx| *ctx.monitor = MonitorStyle::Green),
    );
    palette.add_command(
        choice_label("Amber", monitor == MonitorStyle::Amber),
        TwoAction::Run(|ctx| *ctx.monitor = MonitorStyle::Amber),
    );
    palette.add_command(
        choice_label("White", monitor == MonitorStyle::White),
        TwoAction::Run(|ctx| *ctx.monitor = MonitorStyle::White),
    );
    palette.add_command(
        choice_label("Color", monitor == MonitorStyle::Rgb),
        TwoAction::Run(|ctx| *ctx.monitor = MonitorStyle::Rgb),
    );
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
    eprintln!("  --config <path>   configure the machine from a JSON file (flags override it)");
    eprintln!("  --set <key>=<val> override one config value; files and sets layer in order");
    eprintln!("                    (e.g. --set machine:slots:6:drive1=game.dsk)");
    eprintln!("  --model <2plus|2e> machine to emulate (default: 2plus)");
    eprintln!("  --aux <card>      //e aux-slot card: 80col, ext80col (default) or");
    eprintln!("                    ramworksiii[:SIZE] with SIZE 64k..8m (default 8m)");
    eprintln!(
        "  --color [green|amber|white|rgb]  monitor style (bare --color = rgb; default green)"
    );
    eprintln!("  --scanlines [off|light|heavy]  scanline effect (bare = light; default off)");
    eprintln!("  --boot-delay <seconds>  hold the machine at power-on (debugging/recording)");
    eprintln!("  --fps <fps>       set fps for display (default: 30)");
    eprintln!("  --memory <region> add memory region (ram|rom:address:path)");
    eprintln!("  --wozbug [port]   WozBug debugger server on 127.0.0.1 (default port 6502)");
    eprintln!("  --break <addr,..> break at hex addresses or symbols (implies --wozbug)");
    eprintln!("  --trace <file>    trace cpu to file");
    eprintln!("  --strict          run emulator in strict mode");
    eprintln!("  --debug           print debug info");
    eprintln!("  --state <path>    restore machine state at startup, save it at quit");
    eprintln!("  --serve <url>     boot headless and serve over VNC (notes/REMOTE.md),");
    eprintln!(
        "                    e.g. vnc://0.0.0.0:5901?password=secret  (macOS needs a password);"
    );
    eprintln!("                    ?web=5701 adds the browser console (http://host:5701/),");
    eprintln!("                    ?ws=5701 the raw websocket only (bring your own noVNC)");
}

#[derive(Default)]
struct Options {
    model: TwoType,
    /// The machine's slot table, seeded with the default layout (clock in 1,
    /// Disk II in 6) and replaced when the config document carries a
    /// `machine.slots` table (`--config` files and `--set` overrides).
    slots: BTreeMap<u8, config::SlotCard>,
    monitor: MonitorStyle,
    scanlines: Scanlines,
    /// The //e auxiliary-slot card as its validated `--aux` token (None =
    /// the default extended 80-col card). Kept as a token and parsed at
    /// each power-on, so a reboot can construct a fresh card.
    aux: Option<String>,
    /// Seconds to hold the machine before it starts executing (the window is
    /// up and rendering) — for debugging and video recording.
    boot_delay: f64,
    fps: u32,
    memory: Vec<MemoryOption>,
    /// Emulated CPU cycles per second at startup (config-only; the command
    /// palette switches it at runtime).
    speed: u32,
    /// Preferred game-controller name (config-only); hot-plug still applies.
    controller: Option<String>,
    /// WozBug line-server port (None = no server).
    wozbug: Option<u16>,
    /// Breakpoints armed at boot (`--break`); implies the server.
    breakpoints: Vec<u16>,
    trace_path: Option<String>,
    strict: bool,
    debug: bool,
    screenshot: Option<String>,
    /// Remote-console (VNC) serving. When set, `main` boots the machine
    /// headless and serves it over RFB instead of opening an SDL window
    /// (notes/REMOTE.md).
    serve: Option<ServeOptions>,
    /// Machine-state file (notes/STATE.md): restore at startup when it
    /// exists, save at quit.
    state: Option<String>,
}

/// Where and how to serve the machine over the network (the runtime form of
/// the `remote` config block / `--serve` flag).
struct ServeOptions {
    bind: String,
    port: u16,
    /// RFB-over-WebSocket port for browser clients (noVNC connects straight
    /// to it, no websockify); `None` means no WebSocket listener.
    websocket: Option<u16>,
    /// Serve the embedded web console on the WebSocket port; implies a
    /// WebSocket listener (on `WS_DEFAULT_PORT`) when none is configured.
    web: bool,
    view_only: bool,
    /// VNC-auth password (`None` → the `None` security type). Required by
    /// clients that refuse `None`, such as macOS Screen Sharing.
    password: Option<String>,
}

impl Default for ServeOptions {
    fn default() -> ServeOptions {
        ServeOptions {
            bind: "127.0.0.1".to_string(),
            port: RFB_DEFAULT_PORT,
            websocket: None,
            web: false,
            view_only: false,
            password: None,
        }
    }
}

/// The default plain-TCP RFB port when `--serve vnc://…` gives no port.
const RFB_DEFAULT_PORT: u16 = 5901;
/// The default WebSocket port when `--serve …?ws` gives no explicit value.
const WS_DEFAULT_PORT: u16 = 5701;

/// The default slot table in config terms: the classic layout with no media
/// inserted (`default_slots()` is the machine-level equivalent).
fn default_slot_cards() -> BTreeMap<u8, config::SlotCard> {
    BTreeMap::from([
        (0, config::SlotCard::Language),
        (1, config::SlotCard::Thunderclock),
        (
            6,
            config::SlotCard::Diskii {
                drive1: None,
                drive2: None,
            },
        ),
    ])
}

fn parse_options(args: &[String]) -> Result<Options, i32> {
    let mut options = Options {
        fps: TWO_FPS_DEFAULT,
        speed: SPEED_NORMAL,
        slots: default_slot_cards(),
        ..Options::default()
    };
    // Pass 1: build the config document — --config files and --set
    // overrides deep-merge left-to-right — and seed the options from it,
    // so that in pass 2 — the flag loop, which only assigns a field when
    // its flag is present — anything given explicitly on the command line
    // overrides the document.
    let mut doc: Option<serde_json::Value> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--config" => {
                let Some(path) = it.next() else {
                    usage();
                    return Err(1);
                };
                match crate::config::load_document(path) {
                    Ok(value) => match doc.as_mut() {
                        Some(doc) => crate::config::merge_documents(doc, value),
                        None => doc = Some(value),
                    },
                    Err(e) => {
                        eprintln!("{e}");
                        return Err(1);
                    }
                }
            }
            "--set" => {
                let Some(expr) = it.next() else {
                    usage();
                    return Err(1);
                };
                let doc =
                    doc.get_or_insert_with(|| serde_json::json!({"machine": {"model": "2plus"}}));
                if let Err(e) = crate::config::apply_set(doc, expr) {
                    eprintln!("{e}");
                    return Err(1);
                }
            }
            _ => {}
        }
    }
    if let Some(doc) = doc
        && let Err(e) =
            crate::config::from_document(doc).and_then(|c| apply_config(&mut options, c))
    {
        eprintln!("{e}");
        return Err(1);
    }
    let mut it = args.iter().peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" => {
                usage();
                return Err(0);
            }
            "--config" | "--set" => {
                // Applied in pass 1.
                it.next();
            }
            "--model" => match it.next().map(String::as_str) {
                Some("2plus" | "2+" | "][+" | "2") => options.model = TwoType::Apple2Plus,
                Some("2e" | "//e" | "iie") => options.model = TwoType::Apple2E,
                _ => {
                    usage();
                    return Err(1);
                }
            },
            // Bare --color keeps its historical meaning (an RGB color
            // monitor); an optional value picks a monochrome phosphor
            // instead. Peek-don't-consume so `--color --set k=v` works.
            "--color" => {
                options.monitor = it
                    .peek()
                    .and_then(|v| MonitorStyle::parse(v))
                    .inspect(|_| {
                        it.next();
                    })
                    .unwrap_or(MonitorStyle::Rgb);
            }
            // Same optional-value convention: bare --scanlines means light.
            "--scanlines" => {
                options.scanlines = it
                    .peek()
                    .and_then(|v| Scanlines::parse(v))
                    .inspect(|_| {
                        it.next();
                    })
                    .unwrap_or(Scanlines::Light);
            }
            "--aux" => match it.next() {
                Some(token) => {
                    // Validate now, store the token: it is parsed again at
                    // each power-on (a reboot builds a fresh card).
                    if let Err(e) = crate::aux::parse(token) {
                        eprintln!("{e}");
                        usage();
                        return Err(1);
                    }
                    options.aux = Some(token.clone());
                }
                None => {
                    usage();
                    return Err(1);
                }
            },
            "--boot-delay" => {
                options.boot_delay = it
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0)
                    .max(0.0);
            }
            "--fps" => {
                // atoi semantics
                options.fps = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "--memory" => match it.next().and_then(|s| parse_memory_option(s)) {
                Some(m) => options.memory.push(m),
                None => return Err(1),
            },
            "--state" => match it.next() {
                Some(path) => options.state = Some(path.clone()),
                None => {
                    usage();
                    return Err(1);
                }
            },
            "--trace" => options.trace_path = Some("/dev/stderr".to_string()),
            "--strict" => options.strict = true,
            "--debug" => options.debug = true,
            // Optional-value convention like --color: bare --wozbug uses
            // the default port.
            "--wozbug" => {
                options.wozbug = Some(
                    it.peek()
                        .and_then(|v| v.parse::<u16>().ok())
                        .inspect(|_| {
                            it.next();
                        })
                        .unwrap_or(WOZBUG_DEFAULT_PORT),
                );
            }
            "--serve" => match it.next() {
                // Start from any serve options the config already supplied so a
                // config password/view-only survives an explicit --serve.
                Some(url) => match parse_serve(url, options.serve.take().unwrap_or_default()) {
                    Ok(serve) => options.serve = Some(serve),
                    Err(e) => {
                        eprintln!("--serve {url}: {e}");
                        usage();
                        return Err(1);
                    }
                },
                None => {
                    usage();
                    return Err(1);
                }
            },
            "--break" => match it.next() {
                Some(list) => {
                    for part in list.split(',') {
                        match crate::wozbug::parse_addr(part) {
                            Some(addr) => options.breakpoints.push(addr),
                            None => {
                                eprintln!("bad --break address {part:?}");
                                usage();
                                return Err(1);
                            }
                        }
                    }
                    // A breakpoint needs somewhere to land.
                    options.wozbug.get_or_insert(WOZBUG_DEFAULT_PORT);
                }
                None => {
                    usage();
                    return Err(1);
                }
            },
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

/// Parse a `--serve` URL onto a base (usually the config's `remote` block):
/// `vnc://[bind][:port][?ws=5701&web=5701&password=…&view_only=1]`, e.g.
/// `vnc://0.0.0.0:5901?web=5701`, `vnc://:5901` (default bind), or
/// `vnc://127.0.0.1` (default port). `ws` adds the raw RFB-over-WebSocket
/// listener for bring-your-own noVNC (bare `ws` uses 5701); `web` also
/// serves the embedded console page on that port (`web=PORT` is sugar for
/// `web&ws=PORT`). Only the `vnc` scheme is implemented.
fn parse_serve(url: &str, mut serve: ServeOptions) -> Result<ServeOptions, String> {
    let rest = url
        .strip_prefix("vnc://")
        .ok_or("only vnc:// is supported (rdp is a later, optional track)")?;
    let (authority, query) = match rest.split_once('?') {
        Some((authority, query)) => (authority, Some(query)),
        None => (rest, None),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    if !host.is_empty() {
        serve.bind = host.to_string();
    }
    if let Some(port) = port {
        serve.port = port.parse().map_err(|_| format!("invalid port {port:?}"))?;
        if serve.port == 0 {
            return Err("port must be at least 1".to_string());
        }
    }
    for pair in query.into_iter().flat_map(|q| q.split('&')) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "password" => serve.password = (!value.is_empty()).then(|| value.to_string()),
            "view_only" | "viewonly" => serve.view_only = matches!(value, "1" | "true"),
            "ws" => {
                let port = if value.is_empty() {
                    WS_DEFAULT_PORT
                } else {
                    value
                        .parse()
                        .map_err(|_| format!("invalid ws port {value:?}"))?
                };
                if port == 0 {
                    return Err("ws port must be at least 1".to_string());
                }
                serve.websocket = Some(port);
            }
            // The embedded web console: bare `web` (or a truth value) turns
            // it on; a number is sugar for "on, on this WebSocket port".
            "web" => match value {
                "" | "1" | "true" => serve.web = true,
                "0" | "false" => serve.web = false,
                port => {
                    let port: u16 = port
                        .parse()
                        .map_err(|_| format!("invalid web port {port:?}"))?;
                    if port == 0 {
                        return Err("web port must be at least 1".to_string());
                    }
                    serve.web = true;
                    serve.websocket = Some(port);
                }
            },
            "" => {}
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    Ok(serve)
}

/// Seed `Options` from a loaded config file (pass 1 of `parse_options`).
/// `config::load` already validated the layout and resolved relative paths,
/// so slot placement can be trusted here.
fn apply_config(options: &mut Options, config: config::Config) -> Result<(), String> {
    options.model = config.machine.model.two_type();
    if let Some(aux) = &config.machine.aux {
        // Rebuild the --aux flag token so config and CLI share one card
        // construction path.
        let token = match &aux.size {
            Some(size) => format!("{}:{size}", aux.card.flag_token()),
            None => aux.card.flag_token().to_string(),
        };
        crate::aux::parse(&token)?; // validate; parsed again at power-on
        options.aux = Some(token);
    }
    if let Some(slots) = config.machine.slots {
        // A present slots object replaces the table wholesale (an absent one
        // keeps the default layout); the keys were validated by load().
        options.slots = slots
            .into_iter()
            .map(|(key, card)| (key.parse().expect("load() validated slot keys"), card))
            .collect();
    }
    for region in config.machine.memory {
        options.memory.push(MemoryOption {
            rom: region.kind == config::MemoryKind::Rom,
            address: region.address_value()?,
            path: region.path,
        });
    }
    if let Some(monitor) = config.display.monitor {
        options.monitor = monitor.style();
    }
    if let Some(scanlines) = config.display.scanlines {
        options.scanlines = scanlines.scanlines();
    }
    if let Some(fps) = config.display.fps {
        options.fps = fps;
    }
    if let Some(speed) = config.cpu.speed {
        options.speed = match speed {
            config::CpuSpeed::Normal => SPEED_NORMAL,
            config::CpuSpeed::Fast => SPEED_FAST,
            config::CpuSpeed::Faster => SPEED_FASTER,
        };
    }
    if let Some(strict) = config.cpu.strict {
        options.strict = strict;
    }
    if config.input.controller.is_some() {
        options.controller = config.input.controller;
    }
    if let Some(delay) = config.boot.delay {
        options.boot_delay = delay;
    }
    if config.debug.trace.is_some() {
        options.trace_path = config.debug.trace;
    }
    if let Some(enabled) = config.debug.enabled {
        options.debug = enabled;
    }
    if config.state.path.is_some() {
        options.state = config.state.path.clone();
    }
    // A remote block with any field present enables headless VNC serving;
    // validate() has already rejected the reserved "rdp" protocol and port 0.
    let remote = &config.remote;
    if remote.protocol.is_some()
        || remote.bind.is_some()
        || remote.port.is_some()
        || remote.websocket.is_some()
        || remote.web.is_some()
        || remote.view_only.is_some()
        || remote.password.is_some()
    {
        let mut serve = ServeOptions::default();
        if let Some(bind) = &remote.bind {
            serve.bind = bind.clone();
        }
        if let Some(port) = remote.port {
            serve.port = port;
        }
        serve.websocket = remote.websocket;
        if let Some(web) = remote.web {
            serve.web = web;
        }
        if let Some(view_only) = remote.view_only {
            serve.view_only = view_only;
        }
        serve.password = remote.password.clone();
        options.serve = Some(serve);
    }
    Ok(())
}

/// Build the machine `main()` runs from the parsed options: construct from
/// the slot table, then mount the media the table names. Also the machine
/// half of the headless boot-gate test.
fn build_machine(options: &Options) -> Result<Two, String> {
    // Slot 0 never becomes a SlotDevice: it is the ][+ memory-expansion
    // socket, consumed here as a machine-level Slot0. On the //e the
    // language card is built in, so the default table's slot 0 entry is
    // simply that there — but anything else in slot 0 (only reachable
    // through --set plus --model, config validation rejects slot 0 on a
    // //e) is an error, not a silent no-op.
    let slot0 = match options.slots.get(&0) {
        Some(config::SlotCard::Language) => Slot0::Language,
        Some(config::SlotCard::Saturn128) => Slot0::Saturn128,
        _ => Slot0::Empty,
    };
    if options.model == TwoType::Apple2E
        && slot0 != Slot0::Language
        && options.slots.contains_key(&0)
    {
        return Err("the //e has no slot 0 (its language card is built in)".to_string());
    }
    let table: BTreeMap<u8, SlotDevice> = options
        .slots
        .iter()
        .filter(|(slot, _)| **slot != 0)
        .filter_map(|(&slot, card)| match card {
            config::SlotCard::Diskii { .. } => Some((slot, SlotDevice::DiskII)),
            config::SlotCard::Thunderclock => Some((slot, SlotDevice::Thunderclock)),
            config::SlotCard::Liron { .. } => Some((slot, SlotDevice::Liron)),
            // Hard drives attach below (their card needs the image up front).
            config::SlotCard::Harddrive { .. }
            | config::SlotCard::Language
            | config::SlotCard::Saturn128
            | config::SlotCard::Empty => None,
        })
        .collect();
    let aux = options.aux.as_deref().map(crate::aux::parse).transpose()?;
    let mut two = Two::new_with_slots(options.model, aux, slot0, &table)?;
    for (&slot, card) in &options.slots {
        match card {
            config::SlotCard::Diskii { drive1, drive2 } => {
                for (drive, path) in [(0, drive1), (1, drive2)] {
                    if let Some(path) = path {
                        two.load_disk_at(slot, drive, path).map_err(|e| {
                            format!(
                                "cannot load slot {slot} drive {drive} with {path}: {e}",
                                drive = drive + 1
                            )
                        })?;
                    }
                }
            }
            config::SlotCard::Harddrive { image } => {
                two.attach_hdd_at(slot, image)
                    .map_err(|e| format!("cannot mount slot {slot} hard drive {image}: {e}"))?;
            }
            config::SlotCard::Liron { drive1, drive2 } => {
                for (drive, path) in [(0, drive1), (1, drive2)] {
                    if let Some(path) = path {
                        two.load_2mg_at(slot, drive, path).map_err(|e| {
                            format!(
                                "cannot load slot {slot} 3.5 drive {drive} with {path}: {e}",
                                drive = drive + 1
                            )
                        })?;
                    }
                }
            }
            config::SlotCard::Thunderclock
            | config::SlotCard::Language
            | config::SlotCard::Saturn128
            | config::SlotCard::Empty => {}
        }
    }
    Ok(two)
}

/// Everything a power-on does: build the machine from the options, attach
/// the extra `--memory` regions, and apply the machine-level settings. Both
/// frontends boot through this — and the reboot path (Cmd-Shift-R, the
/// palette's "Reboot") is literally this function run again: a power cycle
/// constructs the exact machine a quit and restart would.
fn power_on_machine(options: &Options) -> Result<Two, String> {
    let mut two = build_machine(options)?;
    for m in &options.memory {
        eprintln!(
            "[EWM] Adding {} ${:04X} {}",
            if m.rom { "ROM" } else { "RAM" },
            m.address,
            m.path
        );
        let data = std::fs::read(&m.path).map_err(|e| format!("cannot read {}: {e}", m.path))?;
        if m.rom {
            two.add_rom(m.address, data);
        } else {
            two.add_ram(m.address, data);
        }
    }
    two.set_debug(options.debug);
    two.cpu.strict = options.strict;
    Ok(two)
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
        let lights = two.drive_lights(two.cpu.counter);
        let drive1_active = i == 35 && lights[0];
        let drive2_active = i == 38 && lights[1];
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

/// Most key bytes a remote client can have in flight before we drop input —
/// generous type-ahead, but a bound against a client flooding the queue.
const REMOTE_KEY_QUEUE_MAX: usize = 1024;

/// Modifier state for translating RFB key events, mirroring the SDL loop's
/// `keymod` tracking, plus the pacing queue that feeds the keyboard latch.
/// There is one machine, so one shared state even when several viewers type
/// at once.
///
/// The pacing matters: the Apple II keyboard is a **one-byte latch**, and a
/// browser (noVNC) delivers a whole typed word within a single frame — far
/// faster than any human on the real keyboard. Latching those back-to-back
/// with no CPU cycles in between would overwrite every byte but the last, so
/// translated bytes queue here and [`RemoteKeys::pump`] feeds the next one
/// only after the ROM has consumed the previous (strobe cleared via `$C010`).
/// Free side effect: type-ahead while the machine is busy, like real hardware
/// buffered in the user's fingers.
#[derive(Default)]
struct RemoteKeys {
    ctrl: bool,
    /// Translated key bytes waiting their turn at the keyboard latch.
    queue: std::collections::VecDeque<u8>,
}

impl RemoteKeys {
    /// Apply one RFB input event: translate and queue key bytes, and handle
    /// the immediate actions (modifiers, reset, pointer buttons). X11 keysyms
    /// re-target the SDL keyboard table (notes/REMOTE.md §7); the left
    /// pointer button maps to the Open-Apple / paddle-0 button.
    fn apply(&mut self, two: &mut Two, event: crate::rfb::InputEvent) {
        use crate::rfb::InputEvent;
        match event {
            InputEvent::Key { down, keysym } => self.key(two, down, keysym),
            InputEvent::Pointer { mask, .. } => {
                two.set_button(0, if mask & 1 != 0 { 0x80 } else { 0x00 });
            }
            // Control events are handled by the serve loop, not here (they
            // touch the machine's lifecycle, not its keyboard).
            InputEvent::Control(_) => {}
        }
    }

    /// Translate one X11 keysym press/release; data keys land in the queue,
    /// reset acts immediately (it must work even mid-type-ahead).
    fn key(&mut self, two: &mut Two, down: bool, keysym: u32) {
        match keysym {
            0xffe3 | 0xffe4 => {
                self.ctrl = down; // Control_L / Control_R
                return;
            }
            0xffc9 if down && self.ctrl => {
                two.cpu.reset(); // Ctrl+F12: the reset gesture
                return;
            }
            _ => {}
        }
        if !down {
            return;
        }
        // Ctrl+letter → 1..26, whatever case the client reports.
        if self.ctrl {
            if keysym <= 0x7f {
                let upper = (keysym as u8).to_ascii_uppercase();
                if upper.is_ascii_uppercase() {
                    self.push(upper - b'A' + 1);
                }
            }
            return;
        }
        let byte = match keysym {
            0xff0d | 0xff8d => 0x0d, // Return / KP_Enter
            0xff08 | 0xff51 => 0x08, // BackSpace / Left
            0xff53 => 0x15,          // Right
            0xff52 => 0x0b,          // Up
            0xff54 => 0x0a,          // Down
            0xff1b => 0x1b,          // Escape
            0xffff => 0x7f,          // Delete
            0xff09 => {
                // Tab, mirroring the SDL loop's quirk (TAB also sends DEL).
                self.push(0x09);
                0x7f
            }
            0x20..=0x7e => {
                let b = keysym as u8;
                // The ][+ ROM expects upper case; the //e passes it through.
                if two.model() == TwoType::Apple2E {
                    b
                } else {
                    b.to_ascii_uppercase()
                }
            }
            _ => return,
        };
        self.push(byte);
    }

    /// Queue a translated byte for the latch (dropped past the flood bound).
    fn push(&mut self, byte: u8) {
        if self.queue.len() < REMOTE_KEY_QUEUE_MAX {
            self.queue.push_back(byte);
        }
    }

    /// Feed the next queued byte once the ROM has consumed the previous one
    /// (keyboard strobe clear). Called once per frame, before the CPU burst,
    /// so the machine gets a full frame of cycles to read each byte.
    fn pump(&mut self, two: &mut Two) {
        if two.key_register() & 0x80 == 0
            && let Some(byte) = self.queue.pop_front()
        {
            two.key(byte);
        }
    }
}

/// Apply `--state` at startup: restore when the file exists (replacing the
/// initial reset), cold boot otherwise. A restore failure is fatal — never
/// run a half-restored machine (notes/STATE.md §6). On restore, returns the
/// save time for the paused-start banner — the file's mtime, which the
/// atomic save (write + rename) stamps at the moment of saving.
fn restore_at_startup(two: &mut Two, state: Option<&str>) -> Result<Option<String>, String> {
    let Some(path) = state else { return Ok(None) };
    if !std::path::Path::new(path).exists() {
        eprintln!("[STATE] {path} does not exist yet; cold booting");
        return Ok(None);
    }
    two.restore_state(path)
        .map_err(|e| format!("cannot restore state from {path}: {e}"))?;
    let saved_at = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(format_saved_at)
        .unwrap_or_else(|| "UNKNOWN TIME".to_string());
    eprintln!("[STATE] restored from {path} (saved {saved_at})");
    Ok(Some(saved_at))
}

/// `YYYY-MM-DD HH:MM:SS` in local time, for the restored-state banner.
fn format_saved_at(t: std::time::SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(t)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Save at quit (`--state`); a failure exits nonzero and leaves any previous
/// state file intact (the save is atomic).
fn save_at_quit(two: &Two, state: Option<&str>) -> i32 {
    let Some(path) = state else { return 0 };
    match two.save_state(path) {
        Ok(()) => {
            eprintln!("[STATE] saved to {path}");
            0
        }
        Err(e) => {
            eprintln!("[STATE] cannot save to {path}: {e}");
            1
        }
    }
}

/// SIGINT/SIGTERM → an atomic the headless serve loop polls each frame, so
/// a remote machine saves its state and exits cleanly (notes/STATE.md §6).
/// Raw libc declarations, no new dependency — the platform libc is already
/// linked; the handler only stores a relaxed atomic (async-signal-safe).
/// The SDL frontend does not use this: its window delivers quit events.
static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn request_stop(_sig: i32) {
    STOP.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn install_stop_handlers() {
    unsafe extern "C" {
        fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
    }
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    unsafe {
        signal(SIGINT, request_stop);
        signal(SIGTERM, request_stop);
    }
}

/// Boot the machine headless and serve it over RFB (VNC): the SDL frame loop's
/// shape without SDL. Step the CPU a frame's worth of cycles, render into
/// `Scr`, publish the framebuffer, and drain client input between frames
/// (notes/REMOTE.md Phase 2). Diverges — runs until the process is killed.
fn serve(mut options: Options) -> i32 {
    let serve = options
        .serve
        .take()
        .expect("serve() called without a serve config");
    let fps = if options.fps == 0 {
        TWO_FPS_DEFAULT
    } else {
        options.fps
    };
    let speed = options.speed;

    let mut two = match power_on_machine(&options) {
        Ok(two) => two,
        Err(e) => {
            eprintln!("[TWO] Could not create the machine: {e}");
            return 1;
        }
    };
    if let Some(path) = &options.trace_path {
        match std::fs::File::create(path) {
            Ok(file) => two.cpu.trace = Some(Box::new(std::io::BufWriter::new(file))),
            Err(e) => {
                eprintln!("Cannot open trace file {path}: {e}");
                return 1;
            }
        }
    }
    two.cpu.reset();
    let state_path = options.state.clone();
    // Headless has no pause UI, so a restored machine runs immediately —
    // the paused start is an SDL-frontend behavior (see main).
    if let Err(e) = restore_at_startup(&mut two, state_path.as_deref()) {
        eprintln!("[STATE] {e}");
        return 1;
    }

    // Fix the headless renderer to RGBA8888 so frames ship to the RFB wire
    // format (big-endian RGBA) with no per-pixel conversion (see rfb.rs).
    let mut scr = Scr::new(PixelLayout::Rgba8888);
    scr.set_monitor_style(options.monitor);

    let width = frame_width(two.model()) as u16;
    let name = match two.model() {
        TwoType::Apple2E => "EWM Apple //e",
        _ => "EWM Apple ][+",
    };
    let auth = serve.password.is_some();
    // The web console lives on the WebSocket port, so asking for it without
    // one gets the default WebSocket port.
    let websocket = match (serve.websocket, serve.web) {
        (None, true) => Some(WS_DEFAULT_PORT),
        (websocket, _) => websocket,
    };
    // The speaker's WebAudio side-channel (notes/VNC.md §4): browser clients
    // upgrade on /audio and stream the PCM the frame loop renders below.
    let audio = crate::audio::Hub::new();
    let mut wave = crate::snd::Wave::new();
    wave.set_cpu_frequency(speed as u64);
    let (server, publisher) = match crate::rfb::Server::start(
        crate::rfb::Options {
            bind: serve.bind.clone(),
            port: serve.port,
            websocket,
            web: serve.web,
            audio: Some(audio.clone()),
            name: name.to_string(),
            view_only: serve.view_only,
            password: serve.password,
        },
        width,
        SCR_HEIGHT as u16,
    ) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[RFB] cannot listen on {}:{}: {e}", serve.bind, serve.port);
            return 1;
        }
    };
    eprintln!(
        "[RFB] serving {name} on vnc://{}:{} ({} auth{})",
        serve.bind,
        server.port(),
        if auth { "VNC password" } else { "no" },
        if serve.view_only { ", view-only" } else { "" }
    );
    if let Some(ws_port) = server.websocket_port() {
        if serve.web {
            eprintln!(
                "[RFB] web console on http://{}:{ws_port}/ (same port carries the RFB websocket)",
                serve.bind
            );
        } else {
            eprintln!(
                "[RFB] websocket for browser clients (noVNC) on ws://{}:{ws_port}/",
                serve.bind
            );
        }
    }

    install_stop_handlers();

    let frame_time = std::time::Duration::from_secs_f64(1.0 / fps as f64);
    let mut keys = RemoteKeys::default();
    let mut phase: u32 = 1;
    // The web console's Pause button (notes/VNC.md §2); freezes CPU stepping
    // while the framebuffer keeps being published, so the frozen screen shows.
    let mut paused = false;
    let mut next_frame = std::time::Instant::now();
    loop {
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("[RFB] shutting down");
            return save_at_quit(&two, state_path.as_deref());
        }
        while let Some(event) = server.try_recv_input() {
            match event {
                crate::rfb::InputEvent::Control(control) => match control {
                    crate::rfb::Control::Reset => {
                        eprintln!("[RFB] reset");
                        two.cpu.reset();
                    }
                    crate::rfb::Control::Pause => {
                        paused = !paused;
                        eprintln!("[RFB] {}", if paused { "paused" } else { "resumed" });
                    }
                    crate::rfb::Control::Reboot => {
                        // Power off/on: rebuild from the same options — what a
                        // quit and restart would build — carrying over only
                        // the open trace sink (mirrors the SDL reboot path).
                        eprintln!("[RFB] reboot (power off/on)");
                        let trace = two.cpu.trace.take();
                        match power_on_machine(&options) {
                            Ok(fresh) => two = fresh,
                            Err(e) => eprintln!("[TWO] could not reboot: {e}"),
                        }
                        two.cpu.trace = trace;
                        two.cpu.reset();
                        wave = crate::snd::Wave::new();
                        wave.set_cpu_frequency(speed as u64);
                        keys = RemoteKeys::default();
                        paused = false;
                    }
                },
                other => keys.apply(&mut two, other),
            }
        }

        if !paused {
            // At most one queued key byte per frame, and only once the ROM has
            // consumed the previous one — see the RemoteKeys doc comment.
            keys.pump(&mut two);

            let mut budget = (speed / fps) as i64;
            while budget > 0 {
                match two.cpu.step() {
                    0 => break, // breakpoint (WozBug not wired into serve yet)
                    cycles => budget -= cycles as i64,
                }
            }
        }
        // RFB has no audio channel; the speaker streams over the /audio
        // WebSocket instead (notes/VNC.md). Rendering must run every frame
        // to keep the wave's cycle window in step (silence while paused); the
        // hub only pays for encoding when someone is actually listening.
        let toggles = two.drain_speaker_toggles();
        audio.publish(wave.render(&toggles, two.cpu.counter));

        scr.update(&two, phase, fps);
        publisher.publish(scr.frame(two.model()));

        // Screen time follows machine time: freeze the FLASH blink while
        // paused, exactly as the SDL loop does.
        if !paused {
            phase += 1;
            if phase >= fps {
                phase = 0;
            }
        }

        next_frame += frame_time;
        let now = std::time::Instant::now();
        if now < next_frame {
            std::thread::sleep(next_frame - now);
        } else if now > next_frame + std::time::Duration::from_secs(1) {
            next_frame = now; // resync after a long stall
        }
    }
}

pub fn main(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(code) => return code,
    };
    // A remote block or --serve boots headless over VNC instead of opening an
    // SDL window (notes/REMOTE.md).
    if options.serve.is_some() {
        return serve(options);
    }
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

    // If we have a gamepad, open it. Bluetooth pads usually connect after
    // launch: the ControllerDeviceAdded/Removed arms below handle hot-plug,
    // and the command palette's "Controller" submenu picks between several.

    let open_controller = |subsystem: &sdl3::GamepadSubsystem, id| match subsystem.open(id) {
        Ok(pad) => {
            eprintln!(
                "[SDL] Controller connected: {}",
                pad.name().unwrap_or_else(|| "(unnamed)".to_string())
            );
            Some(pad)
        }
        Err(e) => {
            eprintln!("[SDL] Cannot open controller: {e}");
            None
        }
    };
    // The config's input.controller names a preferred pad (the exact name
    // the palette lists); unmatched or absent falls back to the first one.
    let mut controller = controller_subsystem.as_ref().and_then(|subsystem| {
        let ids = subsystem.gamepads().unwrap_or_default();
        let preferred = options.controller.as_deref().and_then(|want| {
            ids.iter()
                .copied()
                .find(|&id| subsystem.name_for_id(id).is_ok_and(|name| name == want))
        });
        if let Some(want) = &options.controller
            && preferred.is_none()
            && !ids.is_empty()
        {
            eprintln!("[SDL] Preferred controller {want:?} not connected; using the first one");
        }
        preferred
            .or_else(|| ids.first().copied())
            .and_then(|id| open_controller(subsystem, id))
    });

    // Create and configure the Apple II

    let mut two = match power_on_machine(&options) {
        Ok(two) => two,
        Err(e) => {
            eprintln!("[TWO] Could not create the machine: {e}");
            return 1;
        }
    };

    // WozBug: arm --break breakpoints and start the line server. The
    // frame loop drains its commands between frames.
    for &addr in &options.breakpoints {
        two.cpu.add_breakpoint(addr);
    }
    let mut wozbug = crate::wozbug::WozBug::new();
    let wozbug_server = match options.wozbug {
        Some(port) => match crate::wozbug::Server::start(port) {
            Ok(server) => {
                eprintln!("[WOZBUG] listening on 127.0.0.1:{}", server.port());
                Some(server)
            }
            Err(e) => {
                eprintln!("[WOZBUG] cannot listen on 127.0.0.1:{port}: {e}");
                return 1;
            }
        },
        None => None,
    };
    let mut was_stopped = false;

    let layout = match sdl::pixel_format(&canvas) {
        Some(format) if format == PixelFormat::RGBA8888 => PixelLayout::Rgba8888,
        Some(format) if format == PixelFormat::XRGB8888 => PixelLayout::Rgb888,
        _ => PixelLayout::Argb8888,
    };
    let mut scr = Scr::new(layout);
    scr.set_monitor_style(options.monitor);

    let mut snd = audio.as_ref().and_then(|audio| match Snd::new(audio) {
        Ok(snd) => Some(snd),
        Err(e) => {
            eprintln!("[SND] Failed to open audio device: {e}");
            None
        }
    });
    // A config-set CPU speed must rescale the audio's cycle→sample mapping
    // from frame one, the same pairing set_speed() maintains at runtime.
    if options.speed != SPEED_NORMAL
        && let Some(snd) = snd.as_mut()
    {
        snd.set_cpu_frequency(options.speed as u64);
    }

    let mut status_tty = Tty::new(sdl::green(&canvas));
    status_tty.cursor_enabled = false;

    if let Some(path) = &options.trace_path {
        match std::fs::File::create(path) {
            Ok(file) => two.cpu.trace = Some(Box::new(std::io::BufWriter::new(file))),
            Err(e) => {
                eprintln!("Cannot open trace file {path}: {e}");
                return 1;
            }
        }
    }

    // Reset things to a known state — or to the saved state (--state).

    two.cpu.reset();
    // A restored machine starts paused, so it does not run off without you:
    // the pause screen names the save it resumed from; Cmd-P (or the
    // command palette's Unpause) sets it going.
    let mut restored_banner = match restore_at_startup(&mut two, options.state.as_deref()) {
        Ok(banner) => banner,
        Err(e) => {
            eprintln!("[STATE] {e}");
            return 1;
        }
    };

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

    // The disk activity LEDs, drawn over the lower-right corner of the
    // screen while a drive's light is lit (the transparent background makes
    // the rest of the strip invisible).
    let mut led_texture = texture_creator
        .create_texture_streaming(format, LED_STRIP_WIDTH as u32, LED_STRIP_HEIGHT as u32)
        .expect("Failed to create led texture");
    led_texture.set_blend_mode(BlendMode::Blend);
    led_texture.set_scale_mode(ScaleMode::Nearest);

    // The command palette renders at window resolution, not the emulated 3x.
    let mut palette: Palette<TwoAction> = Palette::new(layout);
    let mut palette_visible = false;
    let mut palette_texture = texture_creator
        .create_texture_streaming(format, palette::WIDTH as u32, palette::MAX_HEIGHT as u32)
        .expect("Failed to create palette texture");
    palette_texture.set_scale_mode(ScaleMode::Nearest);

    // The scanline overlay: multiplied over the screen rect (dstRGB =
    // srcRGB * dstRGB), rebuilt only when the setting changes. White rows
    // pass the image through; every third row is dimmed.
    let mut scanline_texture = texture_creator
        .create_texture_streaming(format, SCR_WIDTH as u32 * 3, SCR_HEIGHT as u32 * 3)
        .expect("Failed to create scanline texture");
    scanline_texture.set_blend_mode(BlendMode::Mod);
    scanline_texture.set_scale_mode(ScaleMode::Nearest);
    let fill_scanline_texture = |texture: &mut sdl3::render::Texture, setting: Scanlines| {
        let overlay = scanline_overlay(SCR_WIDTH * 3, SCR_HEIGHT * 3, setting, layout);
        texture
            .update(None, &pixels_to_bytes(&overlay), SCR_WIDTH * 3 * 4)
            .expect("Failed to update scanline texture");
    };
    if options.scanlines != Scanlines::Off {
        fill_scanline_texture(&mut scanline_texture, options.scanlines);
    }

    let mut event_pump = context.event_pump().expect("Failed to get event pump");
    let frame_ms = (1000 / fps) as u64;
    let mut next_frame = sdl3::timer::ticks() + frame_ms;
    // --boot-delay: the window is up and rendering, but the CPU holds at
    // power-on until this tick — lets a screen recorder catch the boot.
    let boot_at = sdl3::timer::ticks() + (options.boot_delay * 1000.0) as u64;
    if options.boot_delay > 0.0 {
        eprintln!("[TWO] Boot delayed {:.1}s", options.boot_delay);
    }
    let mut phase: u32 = 1;
    let mut paused = restored_banner.is_some();
    let mut reboot_requested = false;
    let mut status_bar_visible = false;
    let mut frames: u32 = 0;
    // Emulated CPU speed, seeded from the config (if any) and switchable
    // from the command palette.
    let mut speed: u32 = options.speed;
    // Monitor style, switchable from the command palette; the renderer was
    // seeded from --color above.
    let mut monitor_style = options.monitor;
    // Scanline effect, switchable from the command palette.
    let mut scanlines = options.scanlines;
    // D-pad state (-1/0/1 per axis), merged into the joystick feed.
    let mut dpad: (i8, i8) = (0, 0);

    let mut counter = two.cpu.counter;
    let mut mhz = 1.0f64;

    'outer: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'outer,
                Event::Window { .. } => two.set_screen_dirty(true),

                // A disk image dropped on the running machine swaps drive 1
                // of the boot controller (hard-drive images need a restart --
                // they mount at boot).
                Event::DropFile { filename, .. } => match crate::media::classify(&filename) {
                    Some(crate::media::MediaKind::Floppy) => match two.load_disk(0, &filename) {
                        Ok(()) => eprintln!(
                            "[TWO] Inserted in slot {} drive 1: {filename}",
                            two.boot_disk_slot().unwrap_or_default()
                        ),
                        Err(e) => eprintln!("[TWO] Could not load {filename}: {e}"),
                    },
                    Some(crate::media::MediaKind::HardDrive) => {
                        eprintln!(
                            "[TWO] Hard-drive images mount at boot: restart with \
                             --set machine:slots:7:card=harddrive \
                             --set machine:slots:7:image={filename:?}"
                        );
                    }
                    None => eprintln!("[TWO] Not a disk image: {filename}"),
                },

                // Hot-plug: auto-connect when no pad is active (a pad already
                // present at startup also fires Added — a no-op here); on
                // losing the active pad, fall back to any remaining one.
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Some(subsystem) = controller_subsystem.as_ref() {
                        if controller.is_none() {
                            controller = open_controller(
                                subsystem,
                                sdl3::sys::joystick::SDL_JoystickID(which),
                            );
                        } else if let Ok(name) =
                            subsystem.name_for_id(sdl3::sys::joystick::SDL_JoystickID(which))
                        {
                            eprintln!("[SDL] Controller available: {name}");
                        }
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    let active = controller
                        .as_ref()
                        .and_then(|c| c.id().ok())
                        .is_some_and(|id| u32::from(id) == which);
                    if active {
                        eprintln!("[SDL] Controller disconnected");
                        controller = None;
                        two.set_joystick(None);
                        if let Some(subsystem) = controller_subsystem.as_ref() {
                            controller = subsystem
                                .gamepads()
                                .ok()
                                .and_then(|ids| ids.first().copied())
                                .and_then(|id| open_controller(subsystem, id));
                        }
                    }
                }

                Event::ControllerButtonDown { which, button, .. }
                | Event::ControllerButtonUp { which, button, .. } => {
                    // Only the active pad drives the machine.
                    let active = controller
                        .as_ref()
                        .and_then(|c| c.id().ok())
                        .is_some_and(|id| u32::from(id) == which);
                    if !active {
                        continue;
                    }
                    let pressed = matches!(event, Event::ControllerButtonDown { .. });
                    let state = if pressed { 0x80 } else { 0x00 };
                    match button {
                        // SDL3 renamed A/B/X/Y to their positions.
                        Button::South | Button::LeftShoulder => two.set_button(0, state),
                        Button::East | Button::RightShoulder => two.set_button(1, state),
                        Button::West => two.set_button(2, state),
                        Button::North => two.set_button(3, state),
                        // The D-pad reports as buttons; fold it into the
                        // joystick axes (full deflection, merged with the
                        // analog stick in the per-frame feed).
                        Button::DPadLeft => dpad.0 = if pressed { -1 } else { dpad.0.max(0) },
                        Button::DPadRight => dpad.0 = if pressed { 1 } else { dpad.0.min(0) },
                        Button::DPadUp => dpad.1 = if pressed { -1 } else { dpad.1.max(0) },
                        Button::DPadDown => dpad.1 = if pressed { 1 } else { dpad.1.min(0) },
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
                            PaletteAction::Execute(action) => {
                                palette_visible = false;
                                let mut open_submenu = None;
                                match action {
                                    TwoAction::Run(run) => {
                                        let monitor_before = monitor_style;
                                        let scanlines_before = scanlines;
                                        let mut ctx = TwoCtx {
                                            two: &mut two,
                                            paused: &mut paused,
                                            reboot: &mut reboot_requested,
                                            window: canvas.window_mut(),
                                            speed: &mut speed,
                                            snd: &mut snd,
                                            monitor: &mut monitor_style,
                                            scanlines: &mut scanlines,
                                            open_submenu: &mut open_submenu,
                                        };
                                        run(&mut ctx);
                                        if monitor_style != monitor_before {
                                            scr.set_monitor_style(monitor_style);
                                            two.set_screen_dirty(true);
                                        }
                                        if scanlines != scanlines_before {
                                            if scanlines != Scanlines::Off {
                                                fill_scanline_texture(
                                                    &mut scanline_texture,
                                                    scanlines,
                                                );
                                            }
                                            two.set_screen_dirty(true);
                                        }
                                    }
                                    TwoAction::PickController(id) => {
                                        let already = controller
                                            .as_ref()
                                            .and_then(|c| c.id().ok())
                                            .is_some_and(|active| u32::from(active) == id);
                                        if !already
                                            && let Some(subsystem) = controller_subsystem.as_ref()
                                        {
                                            two.set_joystick(None);
                                            controller = open_controller(
                                                subsystem,
                                                sdl3::sys::joystick::SDL_JoystickID(id),
                                            );
                                        }
                                    }
                                }
                                // Reopen as a choice submenu — a VS Code-style
                                // quick-pick.
                                if let Some(submenu) = open_submenu {
                                    palette.open();
                                    match submenu {
                                        Submenu::MonitorStyle => {
                                            add_monitor_style_commands(&mut palette, monitor_style)
                                        }
                                        Submenu::Scanlines => {
                                            add_scanline_commands(&mut palette, scanlines)
                                        }
                                        Submenu::Speed => add_speed_commands(&mut palette, speed),
                                        Submenu::Controller => {
                                            let active = controller
                                                .as_ref()
                                                .and_then(|c| c.id().ok())
                                                .map(u32::from);
                                            let ids = controller_subsystem
                                                .as_ref()
                                                .and_then(|s| s.gamepads().ok())
                                                .unwrap_or_default();
                                            if ids.is_empty() {
                                                palette.add_command(
                                                    "No controllers found",
                                                    TwoAction::Run(|_| {}),
                                                );
                                            }
                                            for id in ids {
                                                let name = controller_subsystem
                                                    .as_ref()
                                                    .and_then(|s| s.name_for_id(id).ok())
                                                    .unwrap_or_else(|| "(unnamed)".to_string());
                                                palette.add_command(
                                                    choice_label(
                                                        &name,
                                                        active == Some(u32::from(id)),
                                                    ),
                                                    TwoAction::PickController(u32::from(id)),
                                                );
                                            }
                                        }
                                    }
                                    palette_visible = true;
                                }
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
                            // Cmd-R warm reset; Cmd-Shift-R power off/on.
                            Keycode::R => {
                                if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                    reboot_requested = true;
                                } else {
                                    eprintln!("[SDL] Reset");
                                    two.cpu.reset();
                                }
                            }
                            // Cmd-P: pause/unpause, same toggle as the
                            // palette's Pause command.
                            Keycode::P => {
                                paused = !paused;
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
                                    TwoAction::Run(|ctx| {
                                        ctx.two.cpu.reset();
                                    }),
                                );
                                palette.add_command(
                                    "Reboot (Power off/on)",
                                    TwoAction::Run(|ctx| *ctx.reboot = true),
                                );
                                palette.add_command(
                                    if paused { "Unpause" } else { "Pause" },
                                    TwoAction::Run(|ctx| *ctx.paused = !*ctx.paused),
                                );
                                let fullscreen =
                                    canvas.window().fullscreen_state() == FullscreenType::True;
                                palette.add_command(
                                    if fullscreen {
                                        "Leave Full Screen"
                                    } else {
                                        "Enter Full Screen"
                                    },
                                    TwoAction::Run(|ctx| {
                                        let on =
                                            ctx.window.fullscreen_state() == FullscreenType::True;
                                        let _ = ctx.window.set_fullscreen(!on);
                                    }),
                                );
                                // Choice rows open submenus, like VS Code
                                // quick-picks.
                                palette.add_submenu_command(
                                    format!("Monitor Style: {}", monitor_style.label()),
                                    TwoAction::Run(|ctx| {
                                        *ctx.open_submenu = Some(Submenu::MonitorStyle)
                                    }),
                                );
                                palette.add_submenu_command(
                                    format!("Scanlines: {}", scanlines.label()),
                                    TwoAction::Run(|ctx| {
                                        *ctx.open_submenu = Some(Submenu::Scanlines)
                                    }),
                                );
                                palette.add_submenu_command(
                                    format!(
                                        "Controller: {}",
                                        controller
                                            .as_ref()
                                            .and_then(|c| c.name())
                                            .unwrap_or_else(|| "None".to_string())
                                    ),
                                    TwoAction::Run(|ctx| {
                                        *ctx.open_submenu = Some(Submenu::Controller)
                                    }),
                                );
                                // Speed opens a submenu, like the other
                                // choice rows.
                                palette.add_submenu_command(
                                    format!("Speed: {}", speed_label(speed)),
                                    TwoAction::Run(|ctx| *ctx.open_submenu = Some(Submenu::Speed)),
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

        // WozBug: execute queued debugger commands against the machine
        // (works running or stopped), and announce breakpoint hits.
        // Cmd-Shift-R / palette "Reboot": power off/on. Construct the same
        // machine a quit and restart would, carrying over only the host-side
        // debug attachments (the open trace sink, armed breakpoints).
        if reboot_requested {
            reboot_requested = false;
            eprintln!("[SDL] Reboot (power off/on)");
            let trace = two.cpu.trace.take();
            let breakpoints: Vec<u16> = two.cpu.breakpoints().to_vec();
            two = match power_on_machine(&options) {
                Ok(fresh) => fresh,
                Err(e) => {
                    eprintln!("[TWO] Could not reboot the machine: {e}");
                    return 1;
                }
            };
            two.cpu.trace = trace;
            for addr in breakpoints {
                two.cpu.add_breakpoint(addr);
            }
            two.cpu.reset();
            // A fresh machine is a fresh cycle domain: restart the audio
            // stream and the MHz second. A power-on always runs unpaused.
            snd = audio.as_ref().and_then(|audio| Snd::new(audio).ok());
            if speed != SPEED_NORMAL
                && let Some(snd) = snd.as_mut()
            {
                snd.set_cpu_frequency(speed as u64);
            }
            counter = 0;
            paused = false;
            restored_banner = None;
        }

        if let Some(server) = &wozbug_server {
            while let Ok(line) = server.commands.try_recv() {
                let reply = wozbug.execute(&mut two, &line);
                server.reply(&reply);
            }
            let stopped = two.cpu.stopped();
            if stopped && !was_stopped {
                let banner = crate::wozbug::stopped_banner(&mut two);
                eprintln!("[WOZBUG] {}", banner.replace('\n', "\n[WOZBUG] "));
                server.reply(&banner);
            }
            was_stopped = stopped;
        }

        if sdl3::timer::ticks() >= next_frame {
            if !paused {
                // First unpause consumes the restored-state banner; manual
                // pauses from here on show the plain PAUSED box.
                restored_banner = None;
            }
            let running = !paused && !palette_visible && sdl3::timer::ticks() >= boot_at;
            if running {
                // Feed the joystick axes to the paddle logic before the burst.
                two.set_joystick(controller.as_ref().map(|c| {
                    // The D-pad (full deflection) wins over the analog stick.
                    let x = match dpad.0 {
                        0 => c.axis(Axis::LeftX),
                        d => d as i16 * i16::MAX,
                    };
                    let y = match dpad.1 {
                        0 => c.axis(Axis::LeftY),
                        d => d as i16 * i16::MAX,
                    };
                    (x, y)
                }));

                let mut budget = (speed / fps) as i64;
                while budget > 0 {
                    match two.cpu.step() {
                        // Stopped on a breakpoint: give the frame up (the
                        // WozBug pump above owns the machine until G).
                        0 => break,
                        cycles => budget -= cycles as i64,
                    }
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
                if scanlines != Scanlines::Off {
                    let _ = canvas.copy(&scanline_texture, None, screen_dst);
                }

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

                // Disk activity LEDs in the lower-right corner: only drawn
                // while the motor runs (spin-down included) — the selected
                // drive red, the other grey; hidden entirely when idle.
                let lit = two.drive_lights(two.cpu.counter);
                if lit[0] || lit[1] {
                    let strip = render_led_strip(lit, layout);
                    led_texture
                        .update(None, &pixels_to_bytes(&strip), LED_STRIP_WIDTH * 4)
                        .expect("Failed to update led texture");
                    let margin = 4; // logical pixels, scaled 3x like the screen
                    let dst = Rect::new(
                        pad as i32 + (SCR_WIDTH as i32 - margin - LED_STRIP_WIDTH as i32) * 3,
                        pad as i32 + (SCR_HEIGHT as i32 - margin - LED_STRIP_HEIGHT as i32) * 3,
                        LED_STRIP_WIDTH as u32 * 3,
                        LED_STRIP_HEIGHT as u32 * 3,
                    );
                    let _ = canvas.copy(&led_texture, None, dst);
                }

                if paused {
                    canvas.set_blend_mode(BlendMode::Blend);
                    canvas.set_draw_color(Color::RGBA(0, 0, 0, 224));
                    let _ = canvas.fill_rect(None);

                    status_tty.reset();
                    if let Some(saved_at) = &restored_banner {
                        // Started from a saved state (notes/STATE.md §6):
                        // name the save this machine resumed from.
                        status_tty.set_line(7, "      ****************************      ");
                        status_tty.set_line(8, "      *                          *      ");
                        status_tty.set_line(9, "      * RESTORED FROM SAVE STATE *      ");
                        let ts = format!("      *{saved_at:^26}*      ");
                        status_tty.set_line(10, &ts);
                        status_tty.set_line(11, "      *                          *      ");
                        status_tty.set_line(12, "      ****************************      ");
                    } else {
                        status_tty.set_line(8, "          ********************          ");
                        status_tty.set_line(9, "          *                  *          ");
                        status_tty.set_line(10, "          * -+-  PAUSED  -+- *          ");
                        status_tty.set_line(11, "          *                  *          ");
                        status_tty.set_line(12, "          ********************          ");
                    }
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
            // Screen time advances only while machine time does: the FLASH
            // blink (AppleSoft's cursor, flashing text) is derived from
            // `phase` in the renderer, so a paused machine must not keep
            // blinking behind the pause box — the tableau freezes whole.
            if running {
                phase += 1;
                if phase == fps {
                    phase = 0;

                    // Cycles executed over the past second — the true rate,
                    // which the palette's acceleration options make meaningful
                    // (at 1x it is the fake ≈1.023 MHz of quirk #3).
                    mhz = (two.cpu.counter - counter) as f64 / 1_000_000.0;
                    counter = two.cpu.counter;
                }
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

    save_at_quit(&two, options.state.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(args: &[&str]) -> Options {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_options(&args).expect("options must parse")
    }

    /// The drives of the slot 6 Disk II entry in an options table.
    fn slot6_drives(o: &Options) -> (Option<&str>, Option<&str>) {
        match o.slots.get(&6) {
            Some(config::SlotCard::Diskii { drive1, drive2 }) => {
                (drive1.as_deref(), drive2.as_deref())
            }
            other => panic!("slot 6 should be a diskii, got {other:?}"),
        }
    }

    /// The image of a harddrive entry in an options table.
    fn hdd_image(o: &Options, slot: u8) -> Option<&str> {
        match o.slots.get(&slot) {
            Some(config::SlotCard::Harddrive { image }) => Some(image.as_str()),
            _ => None,
        }
    }

    #[test]
    fn color_flag_selects_the_monitor_style() {
        // No flag: the historical green-monochrome default.
        assert_eq!(opts(&[]).monitor, MonitorStyle::Green);
        // Bare --color keeps its historical meaning: an RGB color monitor.
        assert_eq!(opts(&["--color"]).monitor, MonitorStyle::Rgb);
        // A style value picks a phosphor.
        assert_eq!(opts(&["--color", "amber"]).monitor, MonitorStyle::Amber);
        assert_eq!(opts(&["--color", "white"]).monitor, MonitorStyle::White);
        assert_eq!(opts(&["--color", "rgb"]).monitor, MonitorStyle::Rgb);
        // Bare --color followed by another flag: the flag is not consumed.
        let o = opts(&["--color", "--set", "machine:slots:6:drive1=game.dsk"]);
        assert_eq!(o.monitor, MonitorStyle::Rgb);
        assert_eq!(slot6_drives(&o).0, Some("game.dsk"));
    }

    #[test]
    fn scanlines_flag_selects_the_effect() {
        assert_eq!(opts(&[]).scanlines, Scanlines::Off);
        // Bare --scanlines means the light effect.
        assert_eq!(opts(&["--scanlines"]).scanlines, Scanlines::Light);
        assert_eq!(opts(&["--scanlines", "heavy"]).scanlines, Scanlines::Heavy);
        assert_eq!(opts(&["--scanlines", "off"]).scanlines, Scanlines::Off);
        // Bare --scanlines followed by another flag: the flag is not consumed.
        let o = opts(&["--scanlines", "--set", "machine:slots:6:drive1=game.dsk"]);
        assert_eq!(o.scanlines, Scanlines::Light);
        assert_eq!(slot6_drives(&o).0, Some("game.dsk"));
    }

    #[test]
    fn boot_delay_flag_parses_seconds() {
        assert_eq!(opts(&[]).boot_delay, 0.0);
        assert_eq!(opts(&["--boot-delay", "3"]).boot_delay, 3.0);
        assert_eq!(opts(&["--boot-delay", "1.5"]).boot_delay, 1.5);
        // atoi semantics, like --fps: garbage means no delay.
        assert_eq!(opts(&["--boot-delay", "soon"]).boot_delay, 0.0);
        // Negative delays clamp to zero.
        assert_eq!(opts(&["--boot-delay", "-2"]).boot_delay, 0.0);
    }

    /// A fixture path under ewm/tests/configs/.
    macro_rules! fixture {
        ($name:literal) => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/configs/", $name)
        };
    }

    #[test]
    fn config_populates_options() {
        let o = opts(&["--config", fixture!("full.json")]);
        assert_eq!(o.model, TwoType::Apple2E);
        // The aux card travels as its validated token (parsed per power-on).
        let aux = o.aux.as_deref().expect("aux token from config");
        assert!(aux.starts_with("ramworksiii"), "{aux}");
        // The config's slot table replaced the default one.
        assert_eq!(o.slots.len(), 3);
        assert_eq!(o.slots.get(&1), Some(&config::SlotCard::Thunderclock));
        assert_eq!(
            slot6_drives(&o),
            (
                Some(fixture!("../../../disks/DOS33-SystemMaster.dsk")),
                Some(fixture!("../../../disks/DOS33-SamplePrograms.dsk"))
            )
        );
        assert_eq!(
            hdd_image(&o, 7),
            Some(fixture!("../../../disks/ProDOS_2_4_3.po"))
        );
        assert_eq!(o.monitor, MonitorStyle::White);
        assert_eq!(o.scanlines, Scanlines::Heavy);
        assert_eq!(o.fps, 30);
        assert_eq!(o.speed, SPEED_FAST);
        assert!(o.strict);
        assert_eq!(o.controller.as_deref(), Some("Xbox Wireless Controller"));
        assert_eq!(o.boot_delay, 1.5);
        assert_eq!(o.trace_path.as_deref(), Some(fixture!("trace.txt")));
        assert!(o.debug);
        assert_eq!(o.memory.len(), 1);
        assert!(o.memory[0].rom);
        assert_eq!(o.memory[0].address, 0xd000);
        assert_eq!(o.memory[0].path, fixture!("custom.bin"));
    }

    #[test]
    fn cli_flags_override_config() {
        let o = opts(&[
            "--config",
            fixture!("full.json"),
            "--color",
            "amber",
            "--set",
            "machine:slots:6:drive1=other.dsk",
        ]);
        // The explicitly given flag and the later --set win...
        assert_eq!(o.monitor, MonitorStyle::Amber);
        assert_eq!(slot6_drives(&o).0, Some("other.dsk"));
        // ...while everything the command line left alone survives, including
        // the config's drive 2 next to the overridden drive 1 (object-level
        // merge).
        assert_eq!(
            slot6_drives(&o).1,
            Some(fixture!("../../../disks/DOS33-SamplePrograms.dsk"))
        );
        assert_eq!(o.scanlines, Scanlines::Heavy);
        assert_eq!(o.speed, SPEED_FAST);
        assert_eq!(
            hdd_image(&o, 7),
            Some(fixture!("../../../disks/ProDOS_2_4_3.po"))
        );
    }

    #[test]
    fn set_overrides_the_config_document() {
        // Bare --set drives extend the default machine, exactly like the
        // removed --drive1/--drive2 flags did.
        let o = opts(&[
            "--set",
            "machine:slots:6:drive1=a.dsk",
            "--set",
            "machine:slots:6:drive2=b.dsk",
        ]);
        assert_eq!(slot6_drives(&o), (Some("a.dsk"), Some("b.dsk")));
        assert_eq!(o.slots.get(&1), Some(&config::SlotCard::Thunderclock));
        assert_eq!(o.slots.get(&0), Some(&config::SlotCard::Language));

        // Opting out of the language card (the 48K machine) keeps the rest
        // of the default layout.
        let o = opts(&["--set", "machine:slots:0:card=empty"]);
        assert_eq!(o.slots.get(&0), Some(&config::SlotCard::Empty));
        assert_eq!(o.slots.get(&1), Some(&config::SlotCard::Thunderclock));

        // ...or swapping it for the Saturn 128K board.
        let o = opts(&["--set", "machine:slots:0:card=saturn128"]);
        assert_eq!(o.slots.get(&0), Some(&config::SlotCard::Saturn128));

        // A UniDisk 3.5 controller with a .2mg in drive 1.
        let o = opts(&[
            "--set",
            "machine:slots:5:card=liron",
            "--set",
            "machine:slots:5:drive1=work.2mg",
        ]);
        assert_eq!(
            o.slots.get(&5),
            Some(&config::SlotCard::Liron {
                drive1: Some("work.2mg".into()),
                drive2: None,
            })
        );

        // The --hdd replacement: two sets build the slot 7 card...
        let o = opts(&[
            "--set",
            "machine:slots:7:card=harddrive",
            "--set",
            "machine:slots:7:image=c.hdv",
        ]);
        assert_eq!(hdd_image(&o, 7), Some("c.hdv"));
        // ...or one whole-object set.
        let o = opts(&[
            "--set",
            r#"machine:slots:7={"card":"harddrive","image":"c.hdv"}"#,
        ]);
        assert_eq!(hdd_image(&o, 7), Some("c.hdv"));

        // A literal (bare) slots table from a file is not re-materialized:
        // the set creates only what it names, and needs the card first.
        let o = opts(&[
            "--config",
            fixture!("bare.json"),
            "--set",
            "machine:slots:6:card=diskii",
            "--set",
            "machine:slots:6:drive1=a.dsk",
        ]);
        assert_eq!(slot6_drives(&o), (Some("a.dsk"), None));
        assert_eq!(o.slots.len(), 1, "the bare table only gains slot 6");

        // Later sets win; non-slot keys work; a file merged after a set
        // overrides it.
        let o = opts(&[
            "--set",
            "display:monitor=amber",
            "--set",
            "display:monitor=white",
            "--set",
            "display:fps=30",
            "--set",
            "cpu:strict=true",
        ]);
        assert_eq!(o.monitor, MonitorStyle::White);
        assert_eq!(o.fps, 30);
        assert!(o.strict);
        let o = opts(&[
            "--set",
            "machine:model=2plus",
            "--config",
            fixture!("full.json"),
        ]);
        assert_eq!(o.model, TwoType::Apple2E, "the later file wins");

        // Bad expressions fail with exit code 1.
        for bad in [
            "--set machine:slots:9:card=diskii",
            "--set nonsense=1",
            "--set display:monitor",
        ] {
            let args: Vec<String> = ["--set", &bad["--set ".len()..]]
                .iter()
                .map(|s| s.to_string())
                .collect();
            assert!(matches!(parse_options(&args), Err(1)), "{bad}");
        }
    }

    #[test]
    fn config_boots_dos33_like_drive1() {
        // The boot gate: a config naming the DOS 3.3 disk boots it (its
        // relative path resolves against the config's directory, not the
        // CWD). build_machine is the same code main() runs.
        let o = opts(&["--config", fixture!("boot-dos33.json")]);
        // The --set spelling produces the same machine (path as given).
        let via_set = opts(&[
            "--set",
            concat!(
                "machine:slots:6:drive1=",
                fixture!("../../../disks/DOS33-SystemMaster.dsk")
            ),
        ]);
        assert_eq!(via_set.slots.get(&6), o.slots.get(&6));
        let mut two = build_machine(&o).expect("machine must construct");
        two.cpu.reset();

        let mut spent = 0u64;
        loop {
            let text = two.text_screen();
            if text.contains("DOS VERSION 3.3") && text.contains(']') {
                break;
            }
            assert!(
                spent < 400_000_000,
                "gave up waiting for the ] prompt after {spent} cycles; screen was:\n{text}"
            );
            let target = spent + 100_000;
            while spent < target {
                spent += two.cpu.step() as u64;
            }
        }
    }

    #[test]
    fn defaults_without_config_unchanged() {
        let o = opts(&[]);
        assert_eq!(o.speed, SPEED_NORMAL);
        assert!(o.controller.is_none());
        assert_eq!(o.fps, TWO_FPS_DEFAULT);
        assert_eq!(o.slots, default_slot_cards());
    }

    #[test]
    fn wozbug_and_break_flags() {
        assert_eq!(opts(&[]).wozbug, None);
        assert_eq!(opts(&["--wozbug"]).wozbug, Some(6502));
        assert_eq!(opts(&["--wozbug", "7000"]).wozbug, Some(7000));
        // Bare --wozbug followed by another flag: peek-don't-consume.
        let o = opts(&["--wozbug", "--color", "amber"]);
        assert_eq!(o.wozbug, Some(6502));
        assert_eq!(o.monitor, MonitorStyle::Amber);
        // --break takes hex or symbols and implies the server.
        let o = opts(&["--break", "RWTS,C600"]);
        assert_eq!(o.breakpoints, vec![0xbd00, 0xc600]);
        assert_eq!(o.wozbug, Some(6502));
        let bad: Vec<String> = vec!["--break".to_string(), "zzz".to_string()];
        assert!(matches!(parse_options(&bad), Err(1)));
    }

    #[test]
    fn config_flag_rejects_missing_value_and_missing_file() {
        let missing_value: Vec<String> = vec!["--config".to_string()];
        assert!(matches!(parse_options(&missing_value), Err(1)));
        let missing_file: Vec<String> =
            vec!["--config".to_string(), "does-not-exist.json".to_string()];
        assert!(matches!(parse_options(&missing_file), Err(1)));
    }

    #[test]
    fn serve_url_parses_hosts_ports_and_query() {
        let s = parse_serve("vnc://0.0.0.0:5901", ServeOptions::default()).expect("parse");
        assert_eq!((s.bind.as_str(), s.port), ("0.0.0.0", 5901));
        assert_eq!(s.websocket, None);

        // Defaults: bare host, bare port, empty authority.
        let s = parse_serve("vnc://10.0.0.5", ServeOptions::default()).expect("parse");
        assert_eq!((s.bind.as_str(), s.port), ("10.0.0.5", RFB_DEFAULT_PORT));
        let s = parse_serve("vnc://:6000", ServeOptions::default()).expect("parse");
        assert_eq!((s.bind.as_str(), s.port), ("127.0.0.1", 6000));

        // The query: ws (explicit and bare), password, view_only.
        let s = parse_serve(
            "vnc://:5901?ws=5701&password=pw&view_only=1",
            ServeOptions::default(),
        )
        .expect("parse");
        assert_eq!(s.websocket, Some(5701));
        assert_eq!(s.password.as_deref(), Some("pw"));
        assert!(s.view_only);
        let s = parse_serve("vnc://?ws", ServeOptions::default()).expect("parse");
        assert_eq!(s.websocket, Some(WS_DEFAULT_PORT));
        assert!(!s.web, "ws alone does not enable the console");

        // The web console: web=PORT is sugar for web + ws=PORT; bare web
        // leaves the port to the serve-time default.
        let s = parse_serve("vnc://?web=8080", ServeOptions::default()).expect("parse");
        assert!(s.web);
        assert_eq!(s.websocket, Some(8080));
        let s = parse_serve("vnc://?web", ServeOptions::default()).expect("parse");
        assert!(s.web);
        assert_eq!(s.websocket, None);
        // `0`/`false` read as truth values (console off), not as a port.
        let s = parse_serve("vnc://?web=false", ServeOptions::default()).expect("parse");
        assert!(!s.web);
        let s = parse_serve("vnc://?web=0", ServeOptions::default()).expect("parse");
        assert!(!s.web);
        assert!(parse_serve("vnc://?web=x", ServeOptions::default()).is_err());

        // A config-supplied base survives an explicit --serve.
        let base = ServeOptions {
            password: Some("keep".into()),
            websocket: Some(5702),
            ..ServeOptions::default()
        };
        let s = parse_serve("vnc://0.0.0.0:6000", base).expect("parse");
        assert_eq!(s.password.as_deref(), Some("keep"));
        assert_eq!(s.websocket, Some(5702));

        // Rejected shapes.
        assert!(parse_serve("rdp://:5901", ServeOptions::default()).is_err());
        assert!(parse_serve("vnc://:0", ServeOptions::default()).is_err());
        assert!(parse_serve("vnc://:5901?ws=0", ServeOptions::default()).is_err());
        assert!(parse_serve("vnc://:5901?bogus=1", ServeOptions::default()).is_err());
    }

    #[test]
    fn power_on_machine_is_rerunnable_for_reboot() {
        // The aux token survives construction (it is parsed per power-on),
        // so a reboot builds the same machine — aux card included. Before
        // the token change, build_machine consumed the parsed card and a
        // second build would silently fall back to the default aux.
        let o = opts(&["--model", "2e", "--aux", "ramworksiii:128k"]);
        let first = power_on_machine(&o).expect("first power-on");
        let second = power_on_machine(&o).expect("reboot power-on");
        assert_eq!(first.model(), second.model());
        assert_eq!(o.aux.as_deref(), Some("ramworksiii:128k"));
    }

    #[test]
    fn saved_at_banner_fits_the_forty_column_pause_box() {
        let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_770_000_000);
        let saved_at = format_saved_at(t);
        // YYYY-MM-DD HH:MM:SS, local time — shape, not value.
        assert_eq!(saved_at.len(), 19, "{saved_at}");
        assert_eq!(saved_at.as_bytes()[4], b'-');
        assert_eq!(saved_at.as_bytes()[10], b' ');
        assert_eq!(saved_at.as_bytes()[13], b':');
        // Centered in the box's 26 inner columns, the line is exactly the
        // TTY's 40 columns, like every other pause-box line.
        assert_eq!(format!("      *{saved_at:^26}*      ").len(), 40);
    }

    #[test]
    fn state_flag_parses_and_config_maps() {
        assert_eq!(opts(&[]).state, None);
        assert_eq!(
            opts(&["--state", "/tmp/m.state"]).state.as_deref(),
            Some("/tmp/m.state")
        );
        let missing: Vec<String> = vec!["--state".to_string()];
        assert!(matches!(parse_options(&missing), Err(1)));
    }

    /// A remote client types faster than the one-byte keyboard latch can be
    /// read (a browser delivers a whole word within a frame). The queue must
    /// hold bytes back until the ROM consumes each one — no overwrites.
    #[test]
    fn remote_keys_pace_the_one_byte_keyboard_latch() {
        let mut two = Two::new(TwoType::Apple2Plus).expect("machine must construct");
        let mut keys = RemoteKeys::default();
        let key_event = |keysym: u32, down: bool| crate::rfb::InputEvent::Key { down, keysym };

        // "AB" arrives in one burst, as noVNC delivers it.
        for keysym in [b'A' as u32, b'B' as u32] {
            keys.apply(&mut two, key_event(keysym, true));
            keys.apply(&mut two, key_event(keysym, false));
        }
        // Nothing reaches the latch until the frame loop pumps.
        assert_eq!(two.key_register() & 0x80, 0);

        keys.pump(&mut two);
        assert_eq!(two.key_register(), b'A' | 0x80);
        // Unconsumed strobe: pumping again must not clobber the pending byte.
        keys.pump(&mut two);
        assert_eq!(two.key_register(), b'A' | 0x80);

        // The ROM clears the strobe ($C010); only then does the next byte feed.
        two.cpu.mem.read(0xc010);
        keys.pump(&mut two);
        assert_eq!(two.key_register(), b'B' | 0x80);

        // Ctrl tracking still translates through the queue: Ctrl+C → 3.
        two.cpu.mem.read(0xc010);
        keys.apply(&mut two, key_event(0xffe3, true)); // Control down
        keys.apply(&mut two, key_event(b'c' as u32, true));
        keys.apply(&mut two, key_event(0xffe3, false)); // Control up
        keys.pump(&mut two);
        assert_eq!(two.key_register(), 3 | 0x80);
    }
}
