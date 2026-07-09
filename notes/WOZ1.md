# WOZ 1.0 Disk Image Support — Implementation Plan

A working document for adding **WOZ 1.0** disk image support to EWM's Disk II
emulation, alongside the existing `.dsk` / `.do` / `.po` / `.nib` formats
(which must keep working unchanged). Modeled on `APPLE_IIE_ENHANCED.md`:
re-read at the start of every session, update as phases land. **The tree must
build and pass all verification gates (`cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`) after every phase.**

> **Branch:** All WOZ work happens on the **`claude/woz-support`** branch and
> lands on `master` as **one PR**, with **each phase as a separate commit**
> (owner's decision). The tree must pass all gates at every phase commit.

Reference: the WOZ 1.0 specification at
<https://applesaucefdc.com/woz/reference1/>. Test images (21, imaged with
Applesauce in 2018) live in `disks/woz/WOZ 1.0/`.

## Status

| Phase | Description | Size | Status |
|---|---|---|---|
| 1 | WOZ 1.0 container parser (`ewm/src/woz.rs`) + parse gates over all 21 images | M | Done |
| 2 | Bit-stream engine + controller wiring → boots DOS 3.3 System Master.woz | L | Done |
| 3 | Protection compatibility sweep + fixes + per-image table | M | Done |
| 4 | CLI/README/quirks polish | S | Done |

## Why not "just another `DskType`"

`dsk.rs` is a **nibble-per-read** engine: tracks are pre-nibblized byte
buffers, `$C0EC` returns the next whole nibble, the head advances *per access*
(with the `skip % 4` hack approximating timing), and the `cycles` argument of
`Device::read` is ignored. WOZ is the opposite philosophy: a flux-derived
**bitstream** (1 bit = one 4 µs cell), where nibbles are variable-length
(a sync FF is 10 bits), timing itself is the copy-protection medium, and
faithful emulation advances a *bit position by elapsed CPU cycles* into a
shift register + data latch.

## Architecture (decided)

- **New module `ewm/src/woz.rs`** — the genuinely new driver: the container
  parser and the bit-stream engine.
- **The Disk II controller front stays in `dsk.rs`** — soft switches
  (`$C0E0-$C0EF`), stepper phases, motor, drive select are the same physical
  card regardless of media. Sharing it lets drive 1 hold a `.dsk` while
  drive 2 holds a `.woz`, and keeps the frontend drive lights working as-is.
- **Each `Drive` gets a media dispatch** — `Media::Nibbles(...)` (the existing
  path, **byte-for-byte untouched**, `skip % 4` and all) vs `Media::Woz(...)`
  (the new engine, cycle-driven). The nibble path keeps ignoring `cycles`.
- Selection by file magic/extension at load: `.woz` + `WOZ1` magic → WOZ
  media; a `WOZ2` magic is rejected with a clear "WOZ 2.0 not yet supported".

## WOZ 1.0 format digest (from the spec — keep this accurate)

### Header (12 bytes)

| Bytes | Value | Meaning |
|---|---|---|
| 0–3 | `57 4F 5A 31` | `WOZ1` signature |
| 4 | `FF` | high-bit set (7-bit transmission detector) |
| 5–7 | `0A 0D 0A` | LF CR LF (line-ending translation detector) |
| 8–11 | u32 LE | CRC32 of every byte after byte 11; **0 = skip check** |

CRC32 is the Gary S. Brown 1986 variant (standard table-driven CRC-32,
`crc ^ ~0U` in and out, initial value 0). Hand-roll it (~20 lines), no crate.

### Chunks (from byte 12)

`[4-byte ASCII ID][u32 LE data size][data]`, back to back. Unknown chunks are
skipped by size (forward compatibility). In practice INFO, TMAP, TRKS in that
order; META optional.

### INFO (60 bytes data)

| Off | Field | Notes |
|---|---|---|
| +0 | version (u8) | 1 |
| +1 | disk type (u8) | 1 = 5.25″ (all we support), 2 = 3.5″ (reject) |
| +2 | write protected (u8) | honored in the `$C0EE` status bit |
| +3 | synchronized (u8) | cross-track sync applied during imaging |
| +4 | cleaned (u8) | MC3470 fake bits removed |
| +5 | creator (32 bytes) | UTF-8, space-padded |

### TMAP (160 bytes data)

One u8 per **quarter track**: index 0 = track 0.00, 1 = 0.25, 2 = 0.50, …
(5.25″). Value = index into TRKS; `0xFF` = no track (emit random bits).
Multiple entries commonly point at the same TRKS track (head width).

### TRKS (n × 6656 bytes data; each track starts on a 256-byte file boundary)

Per track:

| Off | Size | Field |
|---|---|---|
| +0 | 6646 | bitstream, **MSB first** within each byte |
| +6646 | u16 LE | bytes used |
| +6648 | u16 LE | bit count (the authoritative track length) |
| +6650 | u16 LE | splice point (bit index; `0xFFFF` = unknown) |
| +6652 | u8 | splice nibble |
| +6653 | u8 | splice bit count |
| +6654 | u16 LE | reserved |

### META (optional)

UTF-8 key/value: `key\tvalue\n` lines; standard keys (title, publisher, …).
Parse leniently, expose as a map; nothing depends on it.

### Emulation rules (the ones that matter)

- **Timing:** one bit per 4 µs = **1023/250 ≈ 4.092 CPU cycles** at EWM's
  1.023 MHz (tracked with a fractional remainder — cycle-counted readers are
  tuned to the real ~32.7-cycle nibble spacing). 1 bit = flux transition,
  0 = none.
- **MC3470 fake bits:** once the last **four** cells are all zero the
  amplifier turns background noise into fake bits (the 4-bit head-window rule
  from the WOZ reference implementation). *The spec's prose says "more than
  two zeros", but the sweep proved the window rule is what images are
  mastered against: Stargate's track 0 carries 416 deliberate runs of exactly
  three zeros that must read back as real zeros on a `cleaned` image.* Noise
  comes from a **fixed-seed free-running xorshift32** — reproducible runs,
  but no short period (a periodic buffer locks deterministic retry loops into
  repeating the same failure every revolution). Empty (`0xFF`) TMAP entries
  are pure noise with the synthetic **51,200-bit** length for position math.
- **Track change:** if the TMAP value is unchanged, keep streaming — do not
  reset anything. Otherwise preserve rotational position:
  `new_pos = pos × new_len / old_len`.
- **Latch semantics:** even soft-switch addresses return the data latch;
  a completed byte (MSB set) stays readable for two bit cells, then the
  latch tracks the next partial byte. **`$C08D,X` (Q6 high) clears and
  *parks* the shift register — bits fly past unshifted until the next
  `$C08C` access pulls Q6 low, and framing restarts there.** Getting both
  halves right (the park *and* the platter continuing to turn during it) is
  exactly what the E7 protection measures.
- **Motor-off delay:** ~1 second after `$C088,X` (`$C0E8`) before the motor
  actually stops; protections read sectors during the spin-down.

## Phases

### Phase 1 — Container parser (M)

`ewm/src/woz.rs`: header validation, CRC32 verify, chunk walk, INFO/TMAP/TRKS
into a typed `WozImage` (+ lenient META). Errors are descriptive strings, as
in `Dsk::set_disk_data`. No wiring into the machine yet.

**Gate:** unit tests parse **all 21 images** in `disks/woz/WOZ 1.0/`
(header + CRC pass, INFO is 5.25″/v1, TMAP/TRKS consistent: every non-FF TMAP
entry indexes a real track, bit count ≤ bytes used × 8) plus field-level
assertions on `DOS 3.3 System Master.woz`.

### Phase 2 — Bit-stream engine + wiring (L)

- Per-drive bit cursor advanced by elapsed cycles / 4; shift register + data
  latch per the rules above; `$C08D` reset hook.
- Fake-bit generator (fixed-seed xorshift32); empty-track handling.
- Stepping: keep the existing half-track stepper; **TMAP index =
  half-track × 2**. True quarter-track (dual-phase) stepping is out of scope
  — standard stepping only reaches even quarter indices — recorded as a
  quirk. Track-change position scaling per the spec.
- Motor-off 1-second delay (cycle-stamped).
- Read-only: honor INFO write-protect in the `$C0EE` status bit; writes to
  WOZ media are ignored (consistent with quirk #2; WOZ1 writing is
  discouraged by the spec itself).
- `Media` dispatch in `dsk.rs`; `.woz` detection in `set_disk_file`.

**Gate:** `ewm/tests/two_woz.rs` — boot `DOS 3.3 System Master.woz` headless
to the `]` prompt and run `CATALOG` (mirrors `two_boot`/`two_dos`). Every
existing `.dsk`/`.po`/`.nib` gate stays byte-for-byte green.

### Phase 3 — Protection compatibility sweep (M)

Boot the protected titles headless (Hard Hat Mack, Wings of Fury, The
Bilestoad for half-tracks, Blazing Paddles, Commando, …) with pragmatic
assertions (reaches hi-res / title state / past the boot sector). Fix what
the sweep exposes (latch persistence, fake-bit behavior, motor timing).
Record a per-image compatibility table below.

**Gate:** the sweep table checked in; a chosen subset asserted in CI
(deterministic ones only).

> **Landed.** The sweep (`ewm/tests/zz_woz_sweep.rs`, run with `--ignored`)
> surfaced and fixed four engine/controller bugs: **(1)** soft switches must
> respond to *writes* too (loaders step the head with `STA $C0E1,X`);
> **(2)** the `$C08D` Q6 hold semantics above (unblocked the **E7**
> protection: Commando, Wings of Fury); **(3)** the MC3470 threshold is the
> **4-bit window**, not "3 zeros" (unblocked Stargate's boot stage);
> **(4)** the fake-bit source must not be short-periodic. CI now asserts
> Commando (E7), The Bilestoad (half-tracks) and Wings of Fury on the //e
> (RWTS18 + 128K) in `two_woz.rs`. Results table below.

### Compatibility (21 reference images)

| Image | Result |
|---|---|
| DOS 3.3 System Master | ✅ boots + CATALOG (CI gate) |
| Blazing Paddles (Baudville) | ✅ graphics |
| Bouncing Kamungas | ✅ graphics |
| Commando | ✅ graphics (E7; CI gate) |
| Crisis Mountain | ✅ graphics |
| Dino Eggs | ✅ graphics |
| Hard Hat Mack | ✅ graphics |
| Miner 2049er II | ✅ graphics |
| Planetfall | ✅ gameplay (text adventure) |
| Rescue Raiders Side B | ✅ graphics |
| Sammy Lightfoot | ✅ graphics |
| Stickybear Town Builder | ✅ graphics |
| Take 1 (Baudville) | ✅ graphics |
| The Apple at Play | ✅ menu |
| The Bilestoad | ✅ graphics (half-tracks; CI gate) |
| The Print Shop Companion | ✅ graphics |
| Wings of Fury Side A | ✅ graphics **on the //e** (RWTS18; CI gate). On a ][+ it crashes into zero page — it is a 128K //e title; a real ][+ fares no better |
| Wings of Fury Side B | ➖ data side; shows the boot banner and stops on both machines (believed not bootable by design) |
| DOS 3.2 System Master | ➖ out of scope: 13-sector disks need the 13-sector boot ROM (quirk #3) |
| Stargate | ❌ boots, then `I/O ERROR` (at the physically-correct 4.092 clock it fails slightly differently) — protection not yet cracked |
| First Math Adventures | ❌ loads, then `BRK` at $A853 on both machines and both clock models — protection not yet cracked |

### Phase 4 — Polish (S)

`--drive1/--drive2` usage strings + README mention `.woz`; quirks recorded;
this note finalized.

## Quirks & divergences (append as decided)

1. **Read-only WOZ.** Writes to WOZ media are ignored (existing Disk II write
   support is a no-op anyway — REWRITE.md quirk #2). WOZ1 writing is
   explicitly discouraged by the spec (WOZ2 is the writable generation).
2. **No dual-phase (true quarter-track) stepping.** The stepper moves in
   half-tracks; TMAP odd (0.25/0.75) entries are only reachable on real
   hardware by energizing two phases at once, which EWM does not model.
   Half-track protections (e.g. The Bilestoad) are covered.
3. **13-sector disks are out of scope.** `DOS 3.2 System Master.woz` parses
   fine but cannot boot: the slot 6 boot ROM in EWM is the 16-sector P5A; DOS
   3.2 needs the 13-sector ROM. A boot-ROM option is future work, not a WOZ
   problem.
4. **Deterministic weak bits.** MC3470 noise comes from a fixed-seed
   free-running xorshift32, so test gates are reproducible; real hardware is
   truly random. The period is 2^32-1 bits, so retry loops still see fresh
   noise every revolution.
5. **3.5″ WOZ images are rejected** (INFO disk type 2) — EWM has no 3.5″
   drive.

## Test images (`disks/woz/WOZ 1.0/`)

DOS 3.3 System Master (the unprotected boot gate), DOS 3.2 System Master
(13-sector, parse-only), The Apple at Play, First Math Adventures, Blazing
Paddles, Bouncing Kamungas, Commando, Crisis Mountain, Dino Eggs, Hard Hat
Mack, Miner 2049er II, Planetfall, Rescue Raiders, Sammy Lightfoot, Stargate,
Stickybear Town Builder, Take 1, The Bilestoad (half-tracks), The Print Shop
Companion, Wings of Fury (A+B).

## Risks & open questions

- **Latch/sequencer fidelity** is the classic hard part: too-simple models
  boot DOS but fail protections (E7, spiradisc-style timing). The Phase 3
  sweep is designed to surface this early; the fallback position is "DOS 3.3
  and most titles boot; the stragglers are documented in the table".
- **CPU acceleration** (the 3.5×/7× palette options) scales the cycle counter
  and the disk position together, so relative timing is preserved — expected
  to be safe, verify in Phase 3.
- **Determinism vs weak bits:** fixed-seed noise keeps gates stable, but a
  protection that *requires* different values on every read of the same weak
  region across multiple passes still sees variation (the buffer free-runs
  with the bit position). Validate on a weak-bit title in the sweep.
