# Disk Activity LEDs — Implementation Plan

A working document for adding disk activity "LEDs" to the `two` frontend:
two small squares in the lower-right corner of the screen, one per Disk II
drive. Modeled on `WOZ1.md` / `APPLE_IIE_ENHANCED.md`: small phases, each
independently verifiable. **The tree must build and pass all verification
gates (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`) after every phase.**

> **Branch:** `claude/disk-activity-leds-fodk1w`. This is an S-sized feature;
> all phases land as a single PR into `master`.

## What we are building

When there is disk activity, two small filled squares appear in the
lower-right corner of the emulated screen — one for drive 1, one for
drive 2:

- **Red** — the drive is active.
- **Grey** — the drive is idle (but the other one is active).
- **Hidden** — when no drive is active, the squares are not drawn at all.

## Semantics: what "active" means

The Disk II controller shares a single motor line between its two drives,
and only the *selected* drive spins. `Dsk` already models this and exposes
exactly the right signals, today used only by the status bar's `[1][2]`
text lights (`render_status_bar` in `two.rs`):

- `Dsk::motor_lit(cycles)` — true while the platter turns, **including the
  ~1 second spin-down** after `$C0E8` (`MOTOR_OFF_DELAY`). This matches the
  real Disk II "IN USE" lamp, which is wired to drive-enable and stays lit
  through the spin-down.
- `Dsk::active_drive()` — the selected drive (0 or 1).

So the mapping is:

| State                                  | Drive 1 LED | Drive 2 LED |
|----------------------------------------|-------------|-------------|
| Motor off                              | hidden      | hidden      |
| Motor on/spinning down, drive 1 selected | red       | grey        |
| Motor on/spinning down, drive 2 selected | grey      | red         |

No new state or IO-access tracking is needed in `dsk.rs` — the motor is the
activity signal, exactly like the original hardware's lamp. (An alternative
— stamping the cycle of every `$C0EC/$C0ED` access and lighting the LED for
N ms after — was considered and rejected: it flickers, needs new state, and
is *less* faithful than the motor line.)

The slot 7 hard drive (`hdd.rs`) is out of scope; a third LED for it is a
natural follow-up (it would need a last-access cycle stamp since it has no
motor concept).

## Rendering approach

An SDL-side overlay texture, exactly like the scanline overlay and the
command palette — **`scr.rs` and the shared `pixels` buffer are never
touched**, so the golden-BMP tests and the hidden `--screenshot` flag remain
byte-for-byte unchanged, and the Apple 1 (`one`) frontend is unaffected.

- A new pure module `ewm/src/led.rs` renders the LED strip into a small
  `Vec<u32>` pixel buffer with a **transparent background** (alpha 0), so it
  is unit-testable headless, following the house pattern (pure renderer +
  frame-loop upload).
- Geometry, in emulated (1x) pixels, drawn at 3x by the frame loop like
  everything else so the LEDs look chunky-pixel consistent with the screen:
  - square side **5 px**, gap **3 px** → strip buffer **13×5** — plain
    filled squares, no anti-aliasing, matching the crisp nearest-neighbor
    look. (v1 drew 7 px circles; squares read better at this size.)
- Colors, packed via the existing `PixelLayout::pack`:
  - active: red `(255, 0, 0, 255)` — same red the status bar uses;
  - idle: grey `(128, 128, 128, 255)`.
- The texture is created once (`BlendMode::Blend`, `ScaleMode::Nearest`,
  13×5), updated per frame while lit (it is tiny — same policy as the
  status bar texture), and copied to a dst rect anchored to the
  lower-right corner of `screen_dst` with a 4-logical-px margin:

  ```text
  x = pad + (SCR_WIDTH*3) - margin*3 - 13*3
  y = pad + (SCR_HEIGHT*3) - margin*3 - 5*3
  w = 13*3, h = 5*3
  ```

- Draw order: after the screen + scanline copy and the status bar, **before**
  the paused dim (so the LEDs dim with the screen when paused) and before
  the palette (so the palette stays on top).
- The overlay is independent of the status bar toggle (`I`); the `[1][2]`
  status-bar lights stay as they are.

## Phases

### Phase 1 — `Dsk::drive_lit` helper *(S)*

Add a small query to `dsk.rs` so the frontend asks one question per drive:

```rust
/// Whether drive `index`'s activity light is lit: the motor is running
/// (spin-down included) and this drive is the selected one.
pub fn drive_lit(&self, index: usize, cycles: u64) -> bool {
    self.motor_lit(cycles) && self.drive == index
}
```

Rewire the two `motor_lit(...) && active_drive() == n` sites in
`render_status_bar` to use it.

**Gate:** unit tests in `dsk.rs`: motor off → neither lit; motor on →
selected drive lit, other not; after `$C0E8` both the spin-down window
(lit) and expiry (unlit) via the `cycles` argument; `$C0EA`/`$C0EB` select
switches move the light. Existing tests stay green.

### Phase 2 — pure LED strip renderer *(S)*

New `ewm/src/led.rs`:

```rust
pub const LED_STRIP_WIDTH: usize = 13;
pub const LED_STRIP_HEIGHT: usize = 5;

/// Render the two-drive LED strip: a filled square per drive, red when
/// lit, grey when not, on a transparent background. The caller only
/// renders the strip at all when at least one drive is lit.
pub fn render_led_strip(lit: [bool; 2], layout: PixelLayout) -> Vec<u32>
```

**Gate:** unit tests: buffer is `13*5`; the gap between the squares has
alpha 0; every pixel of each square is red/grey per `lit`; both
`PixelLayout` packings sanity-checked.

### Phase 3 — frontend wiring *(S)*

In `two::main`:

1. Create `led_texture` (streaming, 13×5, `BlendMode::Blend`,
   `ScaleMode::Nearest`) next to the other textures.
2. In the render block, after the status bar copy:

   ```rust
   let lit = [
       two.dsk().drive_lit(0, two.cpu.counter),
       two.dsk().drive_lit(1, two.cpu.counter),
   ];
   if lit[0] || lit[1] {
       // update led_texture from render_led_strip(lit, layout)
       // and copy to the lower-right dst rect
   }
   ```

**Gate (manual):** boot the DOS 3.3 System Master — drive 1's LED shows red
(drive 2 grey) during boot and both vanish ~1 second after the prompt
appears; `CATALOG,D2` lights drive 2 red with drive 1 grey; the LEDs render
above the screen contents, dim under the pause overlay, sit below the
palette, and stay put in fullscreen (letterboxed logical coordinates).
`--screenshot` output and all golden BMPs are unchanged (`cargo test`).

## Out of scope / future work

- **Hard drive LED** — a third square for the slot 7 HDD, driven by a
  last-access cycle stamp (no motor to observe). Trivial to add once the
  strip renderer exists (make the strip width a function of LED count).
- **Palette toggle** — a "Drive LEDs: on/off" command palette entry if the
  overlay ever bothers anyone; not needed for v1 since the overlay is
  invisible when drives are idle.
- **Apple 1 (`one`)** — no disk hardware, nothing to do.
