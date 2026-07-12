//! Disk ][ controller, port of `dsk.c`: a 16-sector controller with two
//! drives, usable in any slot (the machine registers it over its slot's
//! DEVSEL range; the device decodes only the low nibble). Disk images are
//! nibblized to GCR 6-and-2 tracks on load; writes only reach the in-memory
//! nibble stream and are never written back to the image (quirk #2 in
//! REWRITE.md).
//!
//! Most of this code is based on Beneath Apple DOS and another open source
//! emulator at <https://github.com/whscullin/apple2js> (comment from dsk.c).

use ewm_core::mem::Device;

use crate::woz::{WozImage, WozMedia};

pub const DSK_TRACKS: usize = 35;
pub const DSK_SECTORS: usize = 16;
pub const DSK_SECTOR_SIZE: usize = 256;
pub const DSK_NIBBLES_PER_TRACK: usize = 6656;

pub const DSK_DRIVE1: usize = 0;
pub const DSK_DRIVE2: usize = 1;

/// Motor spin-down time after `$C0E8`, ~1 second of CPU cycles.
const MOTOR_OFF_DELAY: u64 = 1_023_000;

// The P5 boot ROM at $Cn00, embedded in dsk.c as dsk_rom[]. Slot-agnostic:
// it derives its own slot from the return address ($20 $58 $FF = JSR $FF58,
// then reads $0100,X) and addresses the soft switches as $C08x,X — the same
// bytes boot a controller in any slot.
pub static DSK_ROM: [u8; 256] = [
    0xa2, 0x20, 0xa0, 0x00, 0xa2, 0x03, 0x86, 0x3c, 0x8a, 0x0a, 0x24, 0x3c, 0xf0, 0x10, 0x05, 0x3c,
    0x49, 0xff, 0x29, 0x7e, 0xb0, 0x08, 0x4a, 0xd0, 0xfb, 0x98, 0x9d, 0x56, 0x03, 0xc8, 0xe8, 0x10,
    0xe5, 0x20, 0x58, 0xff, 0xba, 0xbd, 0x00, 0x01, 0x0a, 0x0a, 0x0a, 0x0a, 0x85, 0x2b, 0xaa, 0xbd,
    0x8e, 0xc0, 0xbd, 0x8c, 0xc0, 0xbd, 0x8a, 0xc0, 0xbd, 0x89, 0xc0, 0xa0, 0x50, 0xbd, 0x80, 0xc0,
    0x98, 0x29, 0x03, 0x0a, 0x05, 0x2b, 0xaa, 0xbd, 0x81, 0xc0, 0xa9, 0x56, 0x20, 0xa8, 0xfc, 0x88,
    0x10, 0xeb, 0x85, 0x26, 0x85, 0x3d, 0x85, 0x41, 0xa9, 0x08, 0x85, 0x27, 0x18, 0x08, 0xbd, 0x8c,
    0xc0, 0x10, 0xfb, 0x49, 0xd5, 0xd0, 0xf7, 0xbd, 0x8c, 0xc0, 0x10, 0xfb, 0xc9, 0xaa, 0xd0, 0xf3,
    0xea, 0xbd, 0x8c, 0xc0, 0x10, 0xfb, 0xc9, 0x96, 0xf0, 0x09, 0x28, 0x90, 0xdf, 0x49, 0xad, 0xf0,
    0x25, 0xd0, 0xd9, 0xa0, 0x03, 0x85, 0x40, 0xbd, 0x8c, 0xc0, 0x10, 0xfb, 0x2a, 0x85, 0x3c, 0xbd,
    0x8c, 0xc0, 0x10, 0xfb, 0x25, 0x3c, 0x88, 0xd0, 0xec, 0x28, 0xc5, 0x3d, 0xd0, 0xbe, 0xa5, 0x40,
    0xc5, 0x41, 0xd0, 0xb8, 0xb0, 0xb7, 0xa0, 0x56, 0x84, 0x3c, 0xbc, 0x8c, 0xc0, 0x10, 0xfb, 0x59,
    0xd6, 0x02, 0xa4, 0x3c, 0x88, 0x99, 0x00, 0x03, 0xd0, 0xee, 0x84, 0x3c, 0xbc, 0x8c, 0xc0, 0x10,
    0xfb, 0x59, 0xd6, 0x02, 0xa4, 0x3c, 0x91, 0x26, 0xc8, 0xd0, 0xef, 0xbc, 0x8c, 0xc0, 0x10, 0xfb,
    0x59, 0xd6, 0x02, 0xd0, 0x87, 0xa0, 0x00, 0xa2, 0x56, 0xca, 0x30, 0xfb, 0xb1, 0x26, 0x5e, 0x00,
    0x03, 0x2a, 0x5e, 0x00, 0x03, 0x2a, 0x91, 0x26, 0xc8, 0xd0, 0xee, 0xe6, 0x27, 0xe6, 0x3d, 0xa5,
    0x3d, 0xcd, 0x00, 0x08, 0xa6, 0x2b, 0x90, 0xdb, 0x4c, 0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
];

// See Beneath Apple DOS 3-21
static WR_TABLE: [u8; 64] = [
    0x96, 0x97, 0x9a, 0x9b, 0x9d, 0x9e, 0x9f, 0xa6, 0xa7, 0xab, 0xac, 0xad, 0xae, 0xaf, 0xb2, 0xb3,
    0xb4, 0xb5, 0xb6, 0xb7, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf, 0xcb, 0xcd, 0xce, 0xcf, 0xd3,
    0xd6, 0xd7, 0xd9, 0xda, 0xdb, 0xdc, 0xdd, 0xde, 0xdf, 0xe5, 0xe6, 0xe7, 0xe9, 0xea, 0xeb, 0xec,
    0xed, 0xee, 0xef, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff,
];

static PHASE_DELTA: [[i32; 4]; 4] = [[0, 1, 2, -1], [-1, 0, 1, 2], [-2, -1, 0, 1], [1, -2, -1, 0]];

static SECTOR_ORDERING_DO: [u8; DSK_SECTORS] = [
    0x00, 0x0d, 0x0b, 0x09, 0x07, 0x05, 0x03, 0x01, 0x0e, 0x0c, 0x0a, 0x08, 0x06, 0x04, 0x02, 0x0f,
];

static SECTOR_ORDERING_PO: [u8; DSK_SECTORS] = [
    0x00, 0x02, 0x04, 0x06, 0x08, 0x0a, 0x0c, 0x0e, 0x01, 0x03, 0x05, 0x07, 0x09, 0x0b, 0x0d, 0x0f,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DskType {
    Do,
    Po,
    Nib,
    Woz,
}

impl DskType {
    /// Port of `ewm_dsk_type_from_path`; `None` is `EWM_DSK_TYPE_UNKNOWN`.
    pub fn from_path(path: &str) -> Option<DskType> {
        if path.ends_with(".dsk") || path.ends_with(".do") {
            Some(DskType::Do)
        } else if path.ends_with(".po") {
            Some(DskType::Po)
        } else if path.ends_with(".nib") {
            Some(DskType::Nib)
        } else if path.ends_with(".woz") {
            Some(DskType::Woz)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Read,
    Write,
}

/// What is in a drive: pre-nibblized tracks (the original `.dsk`/`.po`/`.nib`
/// path, byte-for-byte unchanged) or a WOZ bitstream engine.
#[derive(Default)]
enum Media {
    #[default]
    None,
    Nibbles(Vec<Vec<u8>>), // 35 nibblized tracks
    Woz(Box<WozMedia>),
}

#[derive(Default)]
struct Drive {
    loaded: bool,
    volume: u8,
    track: i32, // half-tracks, 0..=69
    head: usize,
    phase: usize,
    readonly: bool,
    media: Media,
}

pub struct Dsk {
    pub on: bool,
    /// Cycle stamp at which the motor actually stops: the Disk II delays
    /// ~1 second after the motor-off switch (`$C0E8`), and protections read
    /// sectors during the spin-down. Expired lazily on IO access.
    off_at: Option<u64>,
    mode: Mode,
    latch: u8,
    drive: usize,
    skip: u32,
    drives: [Drive; 2],
}

impl Dsk {
    pub fn new() -> Dsk {
        Dsk {
            on: false,
            off_at: None,
            mode: Mode::Read,
            latch: 0,
            drive: 0,
            skip: 0,
            drives: [Drive::default(), Drive::default()],
        }
    }

    /// Port of `ewm_dsk_set_disk_data`.
    pub fn set_disk_data(
        &mut self,
        index: usize,
        readonly: bool,
        data: &[u8],
        dsk_type: DskType,
    ) -> Result<(), String> {
        if index > 1 {
            return Err("drive index out of range".into());
        }

        match dsk_type {
            DskType::Do | DskType::Po => {
                if data.len() != DSK_TRACKS * DSK_SECTORS * DSK_SECTOR_SIZE {
                    return Err(format!("bad image size {}", data.len()));
                }
            }
            DskType::Nib => {
                if data.len() != DSK_TRACKS * DSK_NIBBLES_PER_TRACK {
                    return Err(format!("bad image size {}", data.len()));
                }
            }
            DskType::Woz => {} // the parser validates structure below
        }

        let drive = &mut self.drives[index];
        drive.loaded = true;
        drive.volume = 254; // Default volume number
        drive.track = 0;
        drive.head = 0;
        drive.phase = 0;
        drive.readonly = readonly;
        drive.media = Media::None;

        match dsk_type {
            DskType::Do | DskType::Po => {
                let mut tracks = Vec::with_capacity(DSK_TRACKS);
                for t in 0..DSK_TRACKS {
                    tracks.push(convert_track(drive.volume, data, t, dsk_type));
                }
                drive.media = Media::Nibbles(tracks);
            }
            DskType::Nib => {
                let mut tracks = Vec::with_capacity(DSK_TRACKS);
                for t in 0..DSK_TRACKS {
                    tracks.push(
                        data[t * DSK_NIBBLES_PER_TRACK..(t + 1) * DSK_NIBBLES_PER_TRACK].to_vec(),
                    );
                }
                let volume = locate_volume_number(&tracks[0]);
                if volume != 0 {
                    drive.volume = volume;
                }
                drive.media = Media::Nibbles(tracks);
            }
            DskType::Woz => {
                let image = WozImage::parse(data)?;
                if image.info.disk_type != 1 {
                    drive.loaded = false;
                    return Err("only 5.25\" WOZ images are supported".into());
                }
                // WOZ media is read-only; the image's write-protect flag is
                // what protected software checks via the status bit.
                drive.readonly = readonly || image.info.write_protected;
                drive.media = Media::Woz(Box::new(WozMedia::new(image)));
            }
        }

        Ok(())
    }

    /// Port of `ewm_dsk_set_disk_file`.
    pub fn set_disk_file(
        &mut self,
        index: usize,
        readonly: bool,
        path: &str,
    ) -> Result<(), String> {
        let dsk_type =
            DskType::from_path(path).ok_or_else(|| format!("unknown disk image type: {path}"))?;
        let data = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        self.set_disk_data(index, readonly, &data, dsk_type)
    }

    fn drive(&mut self) -> &mut Drive {
        &mut self.drives[self.drive]
    }

    /// The selected drive (0 or 1), for the frontend's drive lights.
    /// Whether the given drive has a disk inserted.
    pub fn drive_loaded(&self, drive: usize) -> bool {
        self.drives[drive].loaded
    }

    pub fn active_drive(&self) -> usize {
        self.drive
    }

    /// Debug/test: the selected drive's head position in half-tracks.
    pub fn half_track(&self) -> i32 {
        self.drives[self.drive].track
    }

    /// Port of `dsk_phase`: stepper motor phase change moves the head by
    /// half-tracks, clamped to the 70 half-track range. WOZ media is told the
    /// new quarter-track position (half-track × 2; dual-phase quarter
    /// stepping is not modeled — see notes/WOZ1.md).
    fn phase(&mut self, phase: usize, on: bool) {
        if on {
            let drive = self.drive();
            drive.track += PHASE_DELTA[drive.phase][phase];
            drive.phase = phase;
            drive.track = drive.track.clamp(0, (DSK_TRACKS * 2 - 1) as i32);
            let quarter = (drive.track * 2) as usize;
            if let Media::Woz(w) = &mut drive.media {
                w.step_to(quarter);
            }
        }
    }

    /// Lazily expire the spin-down timer; true while the platter turns.
    fn motor_running(&mut self, cycles: u64) -> bool {
        if let Some(t) = self.off_at
            && cycles >= t
        {
            self.on = false;
            self.off_at = None;
        }
        self.on
    }

    /// Whether the drive light should be lit (spin-down keeps it on), for
    /// the frontend status bar.
    pub fn motor_lit(&self, cycles: u64) -> bool {
        self.on && self.off_at.is_none_or(|t| cycles < t)
    }

    /// Whether drive `index`'s activity light is lit: the motor is running
    /// (spin-down included) and this drive is the selected one — the Disk II
    /// shares one motor line and only the selected drive spins. Drives the
    /// frontend status bar and the activity LED overlay.
    pub fn drive_lit(&self, index: usize, cycles: u64) -> bool {
        self.motor_lit(cycles) && self.drive == index
    }

    /// Port of `dsk_write_next`.
    fn write_next(&mut self, v: u8) {
        if self.mode == Mode::Write {
            self.latch = v;
        }
    }

    /// Port of `dsk_read_next`, including the skip counter that makes every
    /// fourth read return 0. Nibble media only; WOZ media is dispatched to
    /// its bit-stream engine in `io_read` instead.
    fn read_next(&mut self) -> u8 {
        let mut result = 0;
        if self.skip != 0 || self.mode == Mode::Write {
            let mode = self.mode;
            let latch = self.latch;
            let drive = self.drive();
            let track_idx = (drive.track >> 1) as usize; // TODO Because drv->track actually goes to 70? (comment from dsk.c)
            if let Media::Nibbles(tracks) = &mut drive.media {
                let track = &mut tracks[track_idx];

                if drive.head >= track.len() {
                    drive.head = 0;
                }

                if mode == Mode::Write {
                    track[drive.head] = latch; // TODO Implement write support (comment from dsk.c)
                } else {
                    result = track[drive.head];
                }

                drive.head += 1;
            }
        }

        self.skip = (self.skip + 1) % 4;

        result
    }

    /// Select a drive; a newly selected WOZ drive was not spinning, so pin
    /// its time stamp to now.
    fn select_drive(&mut self, index: usize, cycles: u64) {
        if self.drive != index {
            self.drive = index;
            if let Media::Woz(w) = &mut self.drives[index].media {
                w.sync(cycles);
            }
        }
    }

    /// Port of `dsk_read`. The controller decodes only the low nibble of the
    /// DEVSEL address ($C080 + slot*16 .. +$F), so the same device works in
    /// any slot — the machine registers it over its slot's 16-byte range.
    pub fn io_read(&mut self, addr: u16, cycles: u64) -> u8 {
        let motor = self.motor_running(cycles);
        let mut result = 0x00;
        match addr & 0x0f {
            0x0 => self.phase(0, false),
            0x1 => self.phase(0, true),
            0x2 => self.phase(1, false),
            0x3 => self.phase(1, true),
            0x4 => self.phase(2, false),
            0x5 => self.phase(2, true),
            0x6 => self.phase(3, false),
            0x7 => self.phase(3, true),

            // MOTOROFF: the Disk II keeps spinning ~1 second (protections
            // read sectors during the spin-down).
            0x8 => {
                if self.on && self.off_at.is_none() {
                    self.off_at = Some(cycles + MOTOR_OFF_DELAY);
                }
            }
            0x9 => {
                let d = self.drive;
                if !motor && let Media::Woz(w) = &mut self.drives[d].media {
                    w.sync(cycles); // was stopped: no time accumulates
                }
                self.on = true;
                self.off_at = None;
            }

            0xa => self.select_drive(DSK_DRIVE1, cycles),
            0xb => self.select_drive(DSK_DRIVE2, cycles),

            // READMODE. Nibbles only stream while the motor runs (spin-down
            // included) — with the disk still, the latch is quiet. RWTS
            // depends on this when switching slots: it waits for the *old*
            // controller's latch to stop changing before starting the new
            // drive, and a latch that streams forever hangs it. (WOZ media
            // models the motor itself.)
            0xe => {
                self.mode = Mode::Read;
                let d = self.drive;
                if self.drives[d].loaded {
                    let wp = if self.drives[d].readonly { 0x80 } else { 0x00 };
                    let r = if let Media::Woz(w) = &mut self.drives[d].media {
                        w.read(cycles, motor, false)
                    } else if motor {
                        self.read_next()
                    } else {
                        0x00
                    };
                    result = (r & 0x7f) | wp;
                }
            }
            // WRITEMODE
            0xf => self.mode = Mode::Write,

            // READ (motor gating as above)
            0xc => {
                let d = self.drive;
                if self.drives[d].loaded {
                    result = if let Media::Woz(w) = &mut self.drives[d].media {
                        w.read(cycles, motor, true)
                    } else if motor {
                        self.read_next()
                    } else {
                        0x00
                    };
                }
            }
            // WRITE - Called by code, but doesn't do anything? (comment from
            // dsk.c). On WOZ media a $C08D read resets the sequencer and
            // clears the latch (the E7 protection depends on it).
            _ => {
                let d = self.drive;
                if let Media::Woz(w) = &mut self.drives[d].media {
                    w.reset_sequencer();
                }
            }
        }
        result
    }

    /// Port of `dsk_write`. The controller decodes the address regardless of
    /// read/write, so the stepper, motor and drive-select switches respond to
    /// writes too — some loaders (found in the WOZ compatibility sweep) step
    /// the head with `STA $C0E1,X`-style writes. Like `io_read`, only the low
    /// nibble is decoded.
    pub fn io_write(&mut self, addr: u16, b: u8, cycles: u64) {
        let motor = self.motor_running(cycles);
        match addr & 0x0f {
            0x0 => self.phase(0, false),
            0x1 => self.phase(0, true),
            0x2 => self.phase(1, false),
            0x3 => self.phase(1, true),
            0x4 => self.phase(2, false),
            0x5 => self.phase(2, true),
            0x6 => self.phase(3, false),
            0x7 => self.phase(3, true),
            0x8 => {
                if self.on && self.off_at.is_none() {
                    self.off_at = Some(cycles + MOTOR_OFF_DELAY);
                }
            }
            0x9 => {
                let d = self.drive;
                if !motor && let Media::Woz(w) = &mut self.drives[d].media {
                    w.sync(cycles);
                }
                self.on = true;
                self.off_at = None;
            }
            0xa => self.select_drive(DSK_DRIVE1, cycles),
            0xb => self.select_drive(DSK_DRIVE2, cycles),
            0xc => {} // Q6L write: loads the write shifter on real hardware
            0xd => self.write_next(b),
            0xe => self.mode = Mode::Read,
            _ => self.mode = Mode::Write,
        }
    }
}

impl Default for Dsk {
    fn default() -> Dsk {
        Dsk::new()
    }
}

/// The controller as an IO device over its slot's 16-byte DEVSEL range, as
/// `dsk.c` registered itself with `cpu_add_iom`. The slot ROM at `$Cn00` is
/// added by the machine.
impl Device for Dsk {
    fn read(&mut self, addr: u16, cycles: u64) -> u8 {
        self.io_read(addr, cycles)
    }

    fn write(&mut self, addr: u16, b: u8, cycles: u64) {
        self.io_write(addr, b, cycles);
    }
}

/// Port of `dsk_native_track_length`.
fn native_track_length(track_idx: usize) -> usize {
    let mut length = 0;
    for sector_idx in 0..DSK_SECTORS {
        // Gap 1
        if sector_idx == 0 {
            length += 0x80;
        } else if track_idx == 0 {
            length += 0x28;
        } else {
            length += 0x26;
        }
        // Address field
        length += 14;
        // Gap 2
        length += 5;
        // Data field
        length += 3 + 342 + 1 + 3;
        // Gap 3
        length += 1;
    }
    length
}

fn fourxfour_hi(v: u8) -> u8 {
    ((v & 0b1010_1010) >> 1) | 0b1010_1010
}

fn fourxfour_lo(v: u8) -> u8 {
    (v & 0b0101_0101) | 0b1010_1010
}

fn defourxfour(h: u8, l: u8) -> u8 {
    ((h << 1) | 0x01) & l
}

/// Port of `dsk_locate_volume_number` (used for .nib images).
fn locate_volume_number(track: &[u8]) -> u8 {
    for i in 0..track.len() / 2 {
        if track[i] == 0xd5 && track[i + 1] == 0xaa && track[i + 2] == 0x96 {
            return defourxfour(track[i + 3], track[i + 4]);
        }
    }
    0
}

/// Port of `dsk_convert_sector`: gaps, the 4-and-4 encoded address field,
/// and the GCR 6-and-2 encoded data field for one 256-byte sector.
fn convert_sector(volume: u8, track_idx: usize, sector_idx: u8, src: &[u8], dst: &mut Vec<u8>) {
    // Gap 1
    if sector_idx == 0 {
        for _ in 0..0x80 {
            dst.push(0xff);
        }
    } else if track_idx == 0 {
        for _ in 0..0x28 {
            dst.push(0xff);
        }
    } else {
        for _ in 0..0x26 {
            dst.push(0xff);
        }
    }

    // Address Field
    let checksum = volume ^ (track_idx as u8) ^ sector_idx;
    dst.push(0xd5);
    dst.push(0xaa);
    dst.push(0x96);
    dst.push(fourxfour_hi(volume));
    dst.push(fourxfour_lo(volume));
    dst.push(fourxfour_hi(track_idx as u8));
    dst.push(fourxfour_lo(track_idx as u8));
    dst.push(fourxfour_hi(sector_idx));
    dst.push(fourxfour_lo(sector_idx));
    dst.push(fourxfour_hi(checksum));
    dst.push(fourxfour_lo(checksum));
    dst.push(0xde);
    dst.push(0xaa);
    dst.push(0xeb);

    // Gap 2
    for _ in 0..5 {
        dst.push(0xff);
    }

    // Data Field
    dst.push(0xd5);
    dst.push(0xaa);
    dst.push(0xad);

    let mut nibbles = [0u8; 0x156];
    let ptr2 = 0usize;
    let ptr6 = 0x56usize;

    let mut idx2 = 0x55i32;
    for idx6 in (0..=0x101i32).rev() {
        let mut val6 = src[(idx6 % 0x100) as usize];
        let mut val2 = nibbles[ptr2 + idx2 as usize];

        val2 = (val2 << 1) | (val6 & 1);
        val6 >>= 1;
        val2 = (val2 << 1) | (val6 & 1);
        val6 >>= 1;

        // The first two iterations (idx6 = 0x100, 0x101) index past the
        // 0x156-byte buffer: the C code writes 2 bytes out of bounds
        // (harmlessly, onto the stack) and the apple2js code this is based
        // on silently discards the write into a JS typed array. Only their
        // low 2 bits, folded into val2 above, matter — so discard.
        if let Some(slot) = nibbles.get_mut(ptr6 + idx6 as usize) {
            *slot = val6;
        }
        nibbles[ptr2 + idx2 as usize] = val2;

        idx2 -= 1;
        if idx2 < 0 {
            idx2 = 0x55;
        }
    }

    let mut last = 0u8;
    for val in nibbles {
        dst.push(WR_TABLE[(last ^ val) as usize]);
        last = val;
    }
    dst.push(WR_TABLE[last as usize]);

    dst.push(0xde);
    dst.push(0xaa);
    dst.push(0xeb);

    // Gap 3
    dst.push(0xff);
}

/// Port of `dsk_convert_track`: physical sectors are written in descending
/// order, with the logical sector number taken from the interleave table.
fn convert_track(volume: u8, data: &[u8], track_idx: usize, dsk_type: DskType) -> Vec<u8> {
    let ordering = if dsk_type == DskType::Do {
        &SECTOR_ORDERING_DO
    } else {
        &SECTOR_ORDERING_PO
    };

    let mut track = Vec::with_capacity(native_track_length(track_idx));
    for sector_idx in 0..DSK_SECTORS {
        let s = 15 - sector_idx;
        let offset = (track_idx * DSK_SECTORS * DSK_SECTOR_SIZE) + (s * DSK_SECTOR_SIZE);
        let src = &data[offset..offset + DSK_SECTOR_SIZE];
        convert_sector(volume, track_idx, ordering[s], src, &mut track);
    }

    assert_eq!(track.len(), native_track_length(track_idx));
    track
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourxfour_round_trips() {
        for v in 0..=255u8 {
            assert_eq!(defourxfour(fourxfour_hi(v), fourxfour_lo(v)), v);
        }
    }

    #[test]
    fn nibblized_sector_has_expected_fields() {
        // A recognizable 256-byte sector at track 2, logical sector 5.
        let src: Vec<u8> = (0..=255u8).collect();
        let mut dst = Vec::new();
        convert_sector(254, 2, 5, &src, &mut dst);

        // Gap 1 for a non-zero sector on a non-zero track is 0x26 bytes.
        assert!(dst[..0x26].iter().all(|&b| b == 0xff));
        let address = &dst[0x26..];

        // Address field prologue, 4-and-4 volume/track/sector/checksum,
        // epilogue.
        assert_eq!(&address[0..3], &[0xd5, 0xaa, 0x96]);
        assert_eq!(defourxfour(address[3], address[4]), 254);
        assert_eq!(defourxfour(address[5], address[6]), 2);
        assert_eq!(defourxfour(address[7], address[8]), 5);
        assert_eq!(defourxfour(address[9], address[10]), 254 ^ 2 ^ 5);
        assert_eq!(&address[11..14], &[0xde, 0xaa, 0xeb]);

        // Gap 2, then the data field prologue.
        let data = &address[14..];
        assert!(data[..5].iter().all(|&b| b == 0xff));
        assert_eq!(&data[5..8], &[0xd5, 0xaa, 0xad]);

        // 342 + 1 GCR nibbles, all from the 6-and-2 write table.
        let nibbles = &data[8..8 + 343];
        assert!(nibbles.iter().all(|&b| WR_TABLE.contains(&b)));

        // Data epilogue and gap 3.
        assert_eq!(&data[8 + 343..8 + 343 + 3], &[0xde, 0xaa, 0xeb]);
        assert_eq!(data[8 + 343 + 3], 0xff);
        assert_eq!(dst.len(), 0x26 + 14 + 5 + 3 + 343 + 3 + 1);
    }

    #[test]
    fn track_zero_layout_and_interleave() {
        let image = vec![0u8; DSK_TRACKS * DSK_SECTORS * DSK_SECTOR_SIZE];
        let track = convert_track(254, &image, 0, DskType::Do);
        assert_eq!(track.len(), native_track_length(0));

        // Walk the address fields: physical order should carry the logical
        // sector numbers of the DOS interleave, reversed.
        let mut logical = Vec::new();
        let mut i = 0;
        while i + 9 < track.len() {
            if track[i] == 0xd5 && track[i + 1] == 0xaa && track[i + 2] == 0x96 {
                logical.push(defourxfour(track[i + 7], track[i + 8]));
                i += 14;
            } else {
                i += 1;
            }
        }
        let expected: Vec<u8> = (0..DSK_SECTORS)
            .rev()
            .map(|s| SECTOR_ORDERING_DO[s])
            .collect();
        assert_eq!(logical, expected);
    }

    #[test]
    fn every_fourth_read_returns_zero() {
        let mut dsk = Dsk::new();
        let image = vec![0x01u8; DSK_TRACKS * DSK_SECTORS * DSK_SECTOR_SIZE];
        dsk.set_disk_data(0, false, &image, DskType::Do).unwrap();

        // With the motor off the latch is quiet (RWTS's slot-switch wait
        // depends on it).
        assert_eq!(dsk.io_read(0xc0ec, 0), 0x00);
        assert_eq!(dsk.io_read(0xc0ec, 0), 0x00);

        dsk.io_read(0xc0e9, 0); // motor on
        // skip starts at 0: the first $C0EC read is skipped and returns 0.
        assert_eq!(dsk.io_read(0xc0ec, 0), 0x00);
        // The next three return real nibbles (gap bytes here).
        assert_eq!(dsk.io_read(0xc0ec, 0), 0xff);
        assert_eq!(dsk.io_read(0xc0ec, 0), 0xff);
        assert_eq!(dsk.io_read(0xc0ec, 0), 0xff);
        // And the cycle repeats.
        assert_eq!(dsk.io_read(0xc0ec, 0), 0x00);
    }

    #[test]
    fn drive_lit_follows_motor_and_drive_select() {
        let mut dsk = Dsk::new();

        // Motor off: neither drive is lit.
        assert!(!dsk.drive_lit(DSK_DRIVE1, 0));
        assert!(!dsk.drive_lit(DSK_DRIVE2, 0));

        // Motor on ($C0E9): only the selected drive (1) is lit.
        dsk.io_read(0xc0e9, 0);
        assert!(dsk.drive_lit(DSK_DRIVE1, 0));
        assert!(!dsk.drive_lit(DSK_DRIVE2, 0));

        // Selecting drive 2 ($C0EB) moves the light.
        dsk.io_read(0xc0eb, 0);
        assert!(!dsk.drive_lit(DSK_DRIVE1, 0));
        assert!(dsk.drive_lit(DSK_DRIVE2, 0));

        // Motor off ($C0E8): lit through the ~1s spin-down, then dark.
        dsk.io_read(0xc0e8, 1000);
        assert!(dsk.drive_lit(DSK_DRIVE2, 1000 + MOTOR_OFF_DELAY - 1));
        assert!(!dsk.drive_lit(DSK_DRIVE2, 1000 + MOTOR_OFF_DELAY));
        assert!(!dsk.drive_lit(DSK_DRIVE1, 1000 + MOTOR_OFF_DELAY));
    }

    #[test]
    fn stepper_phases_move_the_head_in_half_tracks() {
        let mut dsk = Dsk::new();
        // Phase sequence 1, 2 from phase 0 moves in a full track.
        dsk.io_read(0xc0e3, 0); // phase 1 on
        dsk.io_read(0xc0e5, 0); // phase 2 on
        assert_eq!(dsk.drives[0].track, 2);
        // Stepping back down below track 0 clamps.
        dsk.io_read(0xc0e3, 0); // phase 1 on: -1
        dsk.io_read(0xc0e1, 0); // phase 0 on: -1
        assert_eq!(dsk.drives[0].track, 0);
        dsk.io_read(0xc0e7, 0); // phase 3 on from phase 0: -1, clamped
        assert_eq!(dsk.drives[0].track, 0);
    }
}
