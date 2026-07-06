//! Disk ][ controller, port of `dsk.c`: a 16-sector controller fixed to
//! slot 6 with two drives. Disk images are nibblized to GCR 6-and-2 tracks
//! on load; writes only reach the in-memory nibble stream and are never
//! written back to the image (quirk #2 in REWRITE.md).
//!
//! Most of this code is based on Beneath Apple DOS and another open source
//! emulator at <https://github.com/whscullin/apple2js> (comment from dsk.c).

use ewm_core::mem::Device;

pub const DSK_TRACKS: usize = 35;
pub const DSK_SECTORS: usize = 16;
pub const DSK_SECTOR_SIZE: usize = 256;
pub const DSK_NIBBLES_PER_TRACK: usize = 6656;

pub const DSK_DRIVE1: usize = 0;
pub const DSK_DRIVE2: usize = 1;

// The slot 6 boot ROM at $C600, embedded in dsk.c as dsk_rom[].
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

#[derive(Default)]
struct Drive {
    loaded: bool,
    volume: u8,
    track: i32, // half-tracks, 0..=69
    head: usize,
    phase: usize,
    readonly: bool,
    tracks: Vec<Vec<u8>>, // 35 nibblized tracks
}

pub struct Dsk {
    pub on: bool,
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
        }

        let drive = &mut self.drives[index];
        drive.loaded = true;
        drive.volume = 254; // Default volume number
        drive.track = 0;
        drive.head = 0;
        drive.phase = 0;
        drive.readonly = readonly;
        drive.tracks.clear();

        match dsk_type {
            DskType::Do | DskType::Po => {
                for t in 0..DSK_TRACKS {
                    let track = convert_track(drive.volume, data, t, dsk_type);
                    drive.tracks.push(track);
                }
            }
            DskType::Nib => {
                for t in 0..DSK_TRACKS {
                    drive.tracks.push(
                        data[t * DSK_NIBBLES_PER_TRACK..(t + 1) * DSK_NIBBLES_PER_TRACK].to_vec(),
                    );
                }
                let volume = locate_volume_number(&drive.tracks[0]);
                if volume != 0 {
                    drive.volume = volume;
                }
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
    pub fn active_drive(&self) -> usize {
        self.drive
    }

    /// Port of `dsk_phase`: stepper motor phase change moves the head by
    /// half-tracks, clamped to the 70 half-track range.
    fn phase(&mut self, phase: usize, on: bool) {
        if on {
            let drive = self.drive();
            drive.track += PHASE_DELTA[drive.phase][phase];
            drive.phase = phase;
            drive.track = drive.track.clamp(0, (DSK_TRACKS * 2 - 1) as i32);
        }
    }

    /// Port of `dsk_write_next`.
    fn write_next(&mut self, v: u8) {
        if self.mode == Mode::Write {
            self.latch = v;
        }
    }

    /// Port of `dsk_read_next`, including the skip counter that makes every
    /// fourth read return 0.
    fn read_next(&mut self) -> u8 {
        let mut result = 0;
        if self.skip != 0 || self.mode == Mode::Write {
            let mode = self.mode;
            let latch = self.latch;
            let drive = self.drive();
            let track_idx = (drive.track >> 1) as usize; // TODO Because drv->track actually goes to 70? (comment from dsk.c)
            let track = &mut drive.tracks[track_idx];

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

        self.skip = (self.skip + 1) % 4;

        result
    }

    /// Port of `dsk_read` ($C0E0-$C0EF).
    pub fn io_read(&mut self, addr: u16) -> u8 {
        let mut result = 0x00;
        match addr {
            0xc0e0 => self.phase(0, false),
            0xc0e1 => self.phase(0, true),
            0xc0e2 => self.phase(1, false),
            0xc0e3 => self.phase(1, true),
            0xc0e4 => self.phase(2, false),
            0xc0e5 => self.phase(2, true),
            0xc0e6 => self.phase(3, false),
            0xc0e7 => self.phase(3, true),

            0xc0e8 => self.on = false,
            0xc0e9 => self.on = true,

            0xc0ea => self.drive = DSK_DRIVE1,
            0xc0eb => self.drive = DSK_DRIVE2,

            // READMODE
            0xc0ee => {
                self.mode = Mode::Read;
                if self.drive().loaded {
                    let readonly = self.drive().readonly;
                    result = (self.read_next() & 0x7f) | if readonly { 0x80 } else { 0x00 };
                }
            }
            // WRITEMODE
            0xc0ef => self.mode = Mode::Write,

            // READ
            0xc0ec => {
                if self.drive().loaded {
                    result = self.read_next();
                }
            }
            // WRITE - Called by code, but doesn't do anything? (comment from dsk.c)
            0xc0ed => {}

            _ => {
                eprintln!("[DSK] Got an unhandled read from ${addr:04X}");
            }
        }
        result
    }

    /// Port of `dsk_write` ($C0E0-$C0EF).
    pub fn io_write(&mut self, addr: u16, b: u8) {
        match addr {
            0xc0ed => self.write_next(b),
            0xc0ef => self.mode = Mode::Write,
            _ => {
                eprintln!("[DSK] Got an unhandled write to ${addr:04X}");
            }
        }
    }
}

impl Default for Dsk {
    fn default() -> Dsk {
        Dsk::new()
    }
}

/// The controller as an IO device at `$C0E0-$C0EF`, as `dsk.c` registered
/// itself with `cpu_add_iom`. The slot ROM at `$C600` is a plain ROM region
/// added by the machine.
impl Device for Dsk {
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        self.io_read(addr)
    }

    fn write(&mut self, addr: u16, b: u8, _cycles: u64) {
        self.io_write(addr, b);
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

        // skip starts at 0: the first $C0EC read is skipped and returns 0.
        assert_eq!(dsk.io_read(0xc0ec), 0x00);
        // The next three return real nibbles (gap bytes here).
        assert_eq!(dsk.io_read(0xc0ec), 0xff);
        assert_eq!(dsk.io_read(0xc0ec), 0xff);
        assert_eq!(dsk.io_read(0xc0ec), 0xff);
        // And the cycle repeats.
        assert_eq!(dsk.io_read(0xc0ec), 0x00);
    }

    #[test]
    fn stepper_phases_move_the_head_in_half_tracks() {
        let mut dsk = Dsk::new();
        // Phase sequence 1, 2 from phase 0 moves in a full track.
        dsk.io_read(0xc0e3); // phase 1 on
        dsk.io_read(0xc0e5); // phase 2 on
        assert_eq!(dsk.drives[0].track, 2);
        // Stepping back down below track 0 clamps.
        dsk.io_read(0xc0e3); // phase 1 on: -1
        dsk.io_read(0xc0e1); // phase 0 on: -1
        assert_eq!(dsk.drives[0].track, 0);
        dsk.io_read(0xc0e7); // phase 3 on from phase 0: -1, clamped
        assert_eq!(dsk.drives[0].track, 0);
    }
}
