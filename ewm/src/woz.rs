//! WOZ 1.0 disk image support: the container parser and the bit-stream
//! engine the Disk II controller (`dsk.rs`) drives. See `notes/WOZ1.md` for
//! the plan and the format digest, and
//! <https://applesaucefdc.com/woz/reference1/> for the specification.
//!
//! A WOZ image is a bit-accurate recording of a floppy: each track is a
//! stream of 4 µs bit cells (1 = flux transition), not nibbles, and the
//! TMAP maps the head's 160 quarter-track positions onto track images.

/// Bitstream bytes per TRKS track entry.
pub const WOZ_TRACK_BYTES: usize = 6646;
/// Total bytes per TRKS track entry (bitstream + trailing fields).
const WOZ_TRK_SIZE: usize = 6656;
/// Synthetic bit length for empty (`0xFF`) TMAP positions, per the spec.
pub const WOZ_EMPTY_TRACK_BITS: usize = 51200;

/// The INFO chunk.
pub struct WozInfo {
    pub version: u8,
    /// 1 = 5.25", 2 = 3.5" (only 5.25" can be loaded into the Disk II).
    pub disk_type: u8,
    pub write_protected: bool,
    pub synchronized: bool,
    pub cleaned: bool,
    pub creator: String,
}

/// One TRKS track: a bitstream of `bit_count` cells, MSB of each byte first.
pub struct WozTrack {
    bits: Vec<u8>, // WOZ_TRACK_BYTES long
    pub bit_count: usize,
    pub splice_point: u16,
}

impl WozTrack {
    /// The bit at cell `pos` (0-based, `pos < bit_count`).
    pub fn bit(&self, pos: usize) -> u8 {
        (self.bits[pos >> 3] >> (7 - (pos & 7))) & 1
    }
}

/// A parsed WOZ 1.0 image.
pub struct WozImage {
    pub info: WozInfo,
    /// Quarter-track map: index 0 = track 0.00, 1 = 0.25, … 159 = 39.75.
    /// Values index `tracks`; `0xFF` = no track (the head reads noise).
    pub tmap: [u8; 160],
    pub tracks: Vec<WozTrack>,
    /// META key/value pairs, if present (parsed leniently).
    pub meta: Vec<(String, String)>,
}

impl WozImage {
    pub fn from_file(path: &str) -> Result<WozImage, String> {
        let data = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        WozImage::parse(&data)
    }

    /// Parse a WOZ 1.0 image: header + CRC, then the chunk walk. Unknown
    /// chunks are skipped by size (forward compatibility).
    pub fn parse(data: &[u8]) -> Result<WozImage, String> {
        if data.len() < 12 {
            return Err("not a WOZ file: too short".into());
        }
        if &data[0..4] == b"WOZ2" {
            return Err("WOZ 2.0 images are not supported yet (WOZ 1.0 only)".into());
        }
        if &data[0..4] != b"WOZ1" {
            return Err("not a WOZ 1.0 file: bad signature".into());
        }
        if data[4] != 0xff || data[5..8] != [0x0a, 0x0d, 0x0a] {
            return Err("corrupt WOZ header (7-bit or line-ending damage)".into());
        }
        let stored_crc = u32::from_le_bytes(data[8..12].try_into().unwrap());
        if stored_crc != 0 {
            let crc = crc32(&data[12..]);
            if crc != stored_crc {
                return Err(format!(
                    "WOZ CRC mismatch: stored {stored_crc:08x}, computed {crc:08x}"
                ));
            }
        }

        let mut info: Option<WozInfo> = None;
        let mut tmap: Option<[u8; 160]> = None;
        let mut tracks: Option<Vec<WozTrack>> = None;
        let mut meta = Vec::new();

        let mut off = 12usize;
        while off + 8 <= data.len() {
            let id = &data[off..off + 4];
            let size = u32::from_le_bytes(data[off + 4..off + 8].try_into().unwrap()) as usize;
            let start = off + 8;
            let end = start.checked_add(size).filter(|&e| e <= data.len());
            let Some(end) = end else {
                return Err(format!(
                    "chunk {} overruns the file",
                    String::from_utf8_lossy(id)
                ));
            };
            let chunk = &data[start..end];
            match id {
                b"INFO" => info = Some(parse_info(chunk)?),
                b"TMAP" => tmap = Some(parse_tmap(chunk)?),
                b"TRKS" => tracks = Some(parse_trks(chunk)?),
                b"META" => meta = parse_meta(chunk),
                _ => {} // unknown chunk: skip
            }
            off = end;
        }

        let info = info.ok_or("WOZ file has no INFO chunk")?;
        let tmap = tmap.ok_or("WOZ file has no TMAP chunk")?;
        let tracks = tracks.ok_or("WOZ file has no TRKS chunk")?;

        // Every mapped quarter track must point at a real track image.
        for (q, &t) in tmap.iter().enumerate() {
            if t != 0xff && (t as usize) >= tracks.len() {
                return Err(format!(
                    "TMAP quarter-track {q} points at track {t}, but TRKS has only {}",
                    tracks.len()
                ));
            }
        }

        Ok(WozImage {
            info,
            tmap,
            tracks,
            meta,
        })
    }
}

fn parse_info(chunk: &[u8]) -> Result<WozInfo, String> {
    if chunk.len() < 37 {
        return Err(format!("INFO chunk too short: {} bytes", chunk.len()));
    }
    Ok(WozInfo {
        version: chunk[0],
        disk_type: chunk[1],
        write_protected: chunk[2] == 1,
        synchronized: chunk[3] == 1,
        cleaned: chunk[4] == 1,
        creator: String::from_utf8_lossy(&chunk[5..37])
            .trim_end()
            .to_string(),
    })
}

fn parse_tmap(chunk: &[u8]) -> Result<[u8; 160], String> {
    chunk
        .try_into()
        .map_err(|_| format!("TMAP chunk must be 160 bytes, got {}", chunk.len()))
}

fn parse_trks(chunk: &[u8]) -> Result<Vec<WozTrack>, String> {
    if !chunk.len().is_multiple_of(WOZ_TRK_SIZE) {
        return Err(format!(
            "TRKS size {} is not a multiple of {WOZ_TRK_SIZE}",
            chunk.len()
        ));
    }
    let mut tracks = Vec::with_capacity(chunk.len() / WOZ_TRK_SIZE);
    for (i, trk) in chunk.chunks_exact(WOZ_TRK_SIZE).enumerate() {
        let bytes_used = u16::from_le_bytes(trk[6646..6648].try_into().unwrap()) as usize;
        let bit_count = u16::from_le_bytes(trk[6648..6650].try_into().unwrap()) as usize;
        let splice_point = u16::from_le_bytes(trk[6650..6652].try_into().unwrap());
        if bytes_used > WOZ_TRACK_BYTES {
            return Err(format!(
                "track {i}: bytes used {bytes_used} exceeds {WOZ_TRACK_BYTES}"
            ));
        }
        if bit_count == 0 || bit_count > bytes_used * 8 {
            return Err(format!(
                "track {i}: bit count {bit_count} out of range for {bytes_used} bytes"
            ));
        }
        tracks.push(WozTrack {
            bits: trk[..WOZ_TRACK_BYTES].to_vec(),
            bit_count,
            splice_point,
        });
    }
    Ok(tracks)
}

/// META is `key\tvalue\n` lines in UTF-8. Parsed leniently — nothing in the
/// emulator depends on it; it is exposed for tooling/debugging.
fn parse_meta(chunk: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(chunk)
        .lines()
        .filter_map(|line| {
            line.split_once('\t')
                .map(|(k, v)| (k.to_string(), v.to_string()))
        })
        .collect()
}

// --- The bit-stream engine (Phase 2) ---
//
// The Disk II controller in `dsk.rs` drives this per WOZ-loaded drive. The
// disk "spins" lazily: on every latch access the engine advances its bit
// cursor by elapsed-CPU-cycles / 4 (one 4 µs cell per bit at 1.023 MHz) and
// runs each bit through a simplified Logic State Sequencer: a shift register
// that latches a byte when its MSB arrives, holds it readable for two more
// bit cells (the hardware's read window), then tracks the next partial byte.

// One bit cell is 4 µs: at EWM's 1,023,000 cycles/second that is
// 1023/250 = 4.092 CPU cycles per bit. The fraction matters: cycle-counted
// readers (RWTS18 and friends) are tuned to the real ~32.7-cycle nibble
// spacing and drift out of the latch window at an even 32.0.
const BIT_NUM: u64 = 1023; // cycles per BIT_DEN bits
const BIT_DEN: u64 = 250;
/// Completed bytes stay readable in the latch this many bit cells.
const LATCH_HOLD_CELLS: u8 = 2;
/// When a huge time gap elapses, re-sequence only this many trailing bits.
const RESYNC_BITS: usize = 64;

/// A WOZ image mounted in a drive, plus the read-head state.
pub struct WozMedia {
    image: WozImage,
    /// The TMAP value under the head: a TRKS index, or `0xFF` for no track.
    tmap_val: u8,
    /// Bit cursor within the current track (`< track_len()`).
    bit_pos: usize,
    /// Cycle stamp of the last time the cursor advanced.
    last_cycles: u64,
    /// Fractional-bit remainder, in units of 1/BIT_NUM bit.
    bit_acc: u64,
    shifter: u8,
    latch: u8,
    /// Bit cells the completed byte in `latch` remains held.
    hold: u8,
    /// Consecutive raw zero cells seen (MC3470 fake-bit trigger).
    zero_run: u32,
    /// Q6 is high (`$C08D` was read): the shift register is parked and bits
    /// fly past unshifted until the next `$C08C` access pulls Q6 low again.
    held: bool,
    /// Fake-bit noise source: a free-running xorshift32 (fixed seed, so runs
    /// are reproducible, but the period is 2^32-1 — retry loops see fresh
    /// noise on every revolution, unlike a short circular buffer).
    rng: u32,
}

impl WozMedia {
    pub fn new(image: WozImage) -> WozMedia {
        let tmap_val = image.tmap[0];
        WozMedia {
            image,
            tmap_val,
            bit_pos: 0,
            last_cycles: 0,
            bit_acc: 0,
            shifter: 0,
            latch: 0,
            hold: 0,
            zero_run: 0,
            held: false,
            rng: 0x00c0_ffee,
        }
    }

    pub fn write_protected(&self) -> bool {
        self.image.info.write_protected
    }

    /// The bit length of the track under the head (empty positions get the
    /// spec's synthetic 51,200-bit length so position math keeps working).
    fn track_len(&self) -> usize {
        if self.tmap_val == 0xff {
            WOZ_EMPTY_TRACK_BITS
        } else {
            self.image.tracks[self.tmap_val as usize].bit_count
        }
    }

    /// Move the head to a quarter-track position (the stepper landed there).
    /// If the TMAP entry is unchanged the stream continues untouched; on a
    /// real change the rotational position is preserved by scaling, per the
    /// spec: `new_pos = pos × new_len / old_len`.
    pub fn step_to(&mut self, quarter_track: usize) {
        let new = self.image.tmap[quarter_track.min(159)];
        if new == self.tmap_val {
            return;
        }
        let old_len = self.track_len();
        self.tmap_val = new;
        let new_len = self.track_len();
        self.bit_pos = self.bit_pos * new_len / old_len;
        if self.bit_pos >= new_len {
            self.bit_pos = 0;
        }
    }

    /// One bit of MC3470 noise (fixed-seed xorshift32: reproducible runs,
    /// effectively non-periodic within a session).
    fn fake_bit(&mut self) -> u8 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng & 1) as u8
    }

    /// The next bit cell under the head, with MC3470 behavior: an empty
    /// track position is pure noise, and once the last **four** cells are
    /// all zero the amplifier turns background noise into fake bits (the
    /// 4-bit head-window rule from the WOZ reference implementation —
    /// runs of exactly three zeros are deliberate, readable data on
    /// `cleaned` images and must come back as real zeros).
    fn next_bit(&mut self) -> u8 {
        let len = self.track_len();
        if self.bit_pos >= len {
            self.bit_pos = 0;
        }
        let raw = if self.tmap_val == 0xff {
            None
        } else {
            Some(self.image.tracks[self.tmap_val as usize].bit(self.bit_pos))
        };
        self.bit_pos += 1;
        if self.bit_pos == len {
            self.bit_pos = 0;
        }
        match raw {
            None => self.fake_bit(),
            Some(1) => {
                self.zero_run = 0;
                1
            }
            Some(_) => {
                self.zero_run += 1;
                if self.zero_run > 2 {
                    self.fake_bit()
                } else {
                    0
                }
            }
        }
    }

    /// Run `n` bit cells through the sequencer.
    fn run(&mut self, mut n: usize) {
        let len = self.track_len();
        // A long gap (seconds of spinning without reads): jump the cursor and
        // re-sequence only the trailing bits — the sequencer's state depends
        // only on recent history.
        if n > len + RESYNC_BITS {
            let skip = n - RESYNC_BITS;
            self.bit_pos = (self.bit_pos + skip) % len;
            self.zero_run = 0;
            n = RESYNC_BITS;
        }
        for _ in 0..n {
            let bit = self.next_bit();
            if self.hold > 0 {
                self.hold -= 1;
            }
            self.shifter = (self.shifter << 1) | bit;
            if self.shifter & 0x80 != 0 {
                // A byte completed: latch it and hold it readable while the
                // sequencer starts on the next byte.
                self.latch = self.shifter;
                self.shifter = 0;
                self.hold = LATCH_HOLD_CELLS;
            } else if self.hold == 0 {
                // Outside the hold window the latch tracks the partial byte
                // (MSB clear), so polls wait for the next completion.
                self.latch = self.shifter;
            }
        }
    }

    /// Advance the disk to `cycles` and return the data latch. While the
    /// motor is off the disk is not spinning: the cursor stays put and the
    /// stamp is pinned so no time is accumulated.
    ///
    /// `resume` is true for `$C08C` accesses (Q6 low): if the register was
    /// parked by a `$C08D` read, the bits that flew past during the hold are
    /// skipped (the platter kept turning) and framing restarts *here* — the
    /// heart of the E7 protection's re-framing trick.
    pub fn read(&mut self, cycles: u64, motor_on: bool, resume: bool) -> u8 {
        if self.held {
            if resume {
                let n = self.elapsed_bits(cycles, motor_on);
                self.skip_bits(n);
                self.held = false;
            }
            return self.latch;
        }
        let n = self.elapsed_bits(cycles, motor_on);
        self.run(n);
        self.latch
    }

    /// Whole bit cells elapsed since the last advance (4.092 cycles each,
    /// tracked with a fractional remainder). While the motor is off the disk
    /// is not spinning: the stamp is pinned and no time accumulates.
    fn elapsed_bits(&mut self, cycles: u64, motor_on: bool) -> usize {
        if !motor_on {
            self.last_cycles = cycles;
            return 0;
        }
        let elapsed = cycles.saturating_sub(self.last_cycles);
        self.last_cycles = cycles;
        self.bit_acc += elapsed * BIT_DEN;
        let n = self.bit_acc / BIT_NUM;
        self.bit_acc %= BIT_NUM;
        n as usize
    }

    /// Move the cursor without sequencing (bits pass while Q6 is high).
    fn skip_bits(&mut self, n: usize) {
        let len = self.track_len();
        self.bit_pos = (self.bit_pos + n) % len;
        self.zero_run = 0;
    }

    /// `$C08D,X` read (Q6 high): clear and park the shift register and the
    /// latch until the next `$C08C` access. The E7 protection uses this to
    /// re-frame the bit stream, turning timing bits into data bits.
    pub fn reset_sequencer(&mut self) {
        self.shifter = 0;
        self.latch = 0;
        self.hold = 0;
        self.held = true;
    }

    /// Pin the time stamp (drive just selected or motor just started, after
    /// a period of not spinning).
    pub fn sync(&mut self, cycles: u64) {
        self.last_cycles = cycles;
    }
}

/// Standard CRC-32 (the Gary S. Brown 1986 table-driven variant the WOZ spec
/// ships; identical to zlib's crc32 with initial value 0).
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    let mut crc = !0u32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

/// The drive electronics mid-flight (notes/STATE.md §5): head cursor, shift
/// register, MC3470 fake-bit state, the noise RNG — cycle stamps saved
/// verbatim, never rebased. The image itself is not written: WOZ media is
/// read-only in EWM, so construction reloads it from the file.
impl ewm_core::state::Persist for WozMedia {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.put_u8(self.tmap_val);
        w.put_u64(self.bit_pos as u64);
        w.put_u64(self.last_cycles);
        w.put_u64(self.bit_acc);
        w.put_u8(self.shifter);
        w.put_u8(self.latch);
        w.put_u8(self.hold);
        w.put_u32(self.zero_run);
        w.put_bool(self.held);
        w.put_u32(self.rng);
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        self.tmap_val = r.get_u8()?;
        self.bit_pos = r.get_u64()? as usize;
        self.last_cycles = r.get_u64()?;
        self.bit_acc = r.get_u64()?;
        self.shifter = r.get_u8()?;
        self.latch = r.get_u8()?;
        self.hold = r.get_u8()?;
        self.zero_run = r.get_u32()?;
        self.held = r.get_bool()?;
        self.rng = r.get_u32()?;
        // Keep the cursor within the (reloaded) track, whatever the file
        // says — the same-configuration precondition makes mismatch UB, but
        // an in-bounds cursor keeps it non-crashing UB.
        if self.tmap_val != 0xff
            && let Some(track) = self.image.tracks.get(self.tmap_val as usize)
            && track.bit_count > 0
        {
            self.bit_pos %= track.bit_count;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn woz_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../disks/woz/WOZ 1.0")
    }

    /// The Phase 1 gate: every reference image present parses, its CRC
    /// verifies, and its structure is internally consistent. Only the DOS 3.3
    /// System Master is committed (matching the repo's `.dsk` precedent);
    /// the other 20 reference images are exercised when present locally.
    #[test]
    fn all_reference_images_parse() {
        let mut count = 0;
        for entry in std::fs::read_dir(woz_dir()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("woz") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let image = WozImage::from_file(path.to_str().unwrap())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(image.info.version, 1, "{name}: INFO version");
            assert_eq!(image.info.disk_type, 1, "{name}: 5.25\" disk");
            assert!(!image.tracks.is_empty(), "{name}: has tracks");
            // Spot-check the bitstream accessor against the raw first byte.
            let t0 = &image.tracks[0];
            assert!(t0.bit_count <= WOZ_TRACK_BYTES * 8);
            count += 1;
        }
        assert!(count >= 1, "at least the committed reference image parsed");
    }

    #[test]
    fn dos33_system_master_fields() {
        let path = woz_dir().join("DOS 3.3 System Master.woz");
        let image = WozImage::from_file(path.to_str().unwrap()).unwrap();

        assert_eq!(image.info.version, 1);
        assert_eq!(image.info.disk_type, 1);
        assert!(image.info.write_protected);
        assert!(!image.info.synchronized);
        assert!(image.info.cleaned);
        assert_eq!(image.info.creator, "Applesauce v0.24");

        // 35 tracks; the head over track 0.00/0.25 sees track 0, the gap at
        // 0.50 is empty, and 0.75/1.00/1.25 see track 1.
        assert_eq!(image.tracks.len(), 35);
        assert_eq!(&image.tmap[0..6], &[0, 0, 0xff, 1, 1, 1]);
        assert_eq!(image.tmap[4 * 34], 34); // track 34.00
        assert_eq!(image.tmap[159], 0xff); // 39.75: beyond the last track

        // Track 0 starts with 10-bit sync FFs: 1111111100 1111111100 …
        let t0 = &image.tracks[0];
        assert_eq!(t0.bit_count, 50304);
        let leader: Vec<u8> = (0..20).map(|i| t0.bit(i)).collect();
        assert_eq!(
            leader,
            [1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0]
        );
    }

    #[test]
    fn rejects_woz2_and_garbage() {
        let mut woz2 = b"WOZ2\xff\x0a\x0d\x0a\0\0\0\0".to_vec();
        woz2.extend_from_slice(&[0; 8]);
        let Err(err) = WozImage::parse(&woz2) else {
            panic!("WOZ2 must be rejected");
        };
        assert!(err.contains("WOZ 2.0"), "clear WOZ2 message: {err}");

        assert!(WozImage::parse(b"not a woz").is_err());
        // A truncated real header fails the CRC/structure checks, not a panic.
        let path = woz_dir().join("DOS 3.3 System Master.woz");
        let data = std::fs::read(path).unwrap();
        assert!(WozImage::parse(&data[..2000]).is_err());
    }

    #[test]
    fn crc32_matches_known_vector() {
        // The standard check value for "123456789".
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
