# DHGR Colour Experiment — sliding 4-bit window

**Status:** experiment only. Lives on the `iie/experiment-dhgr-color` branch
(commit on top of `claude/apple-iie-enhanced` at #235); pushed but **no PR**.
This note records the findings so the work can be picked up later.

## Context

Phase 6b landed DHGR colour using **aligned 4-bit cells**: the 560-bit line is
chopped into 140 fixed cells, each rendered as one of the 16 lo-res colours,
4 px wide (leftmost bit = LSB). That was flagged in the plan doc as a deliberate
starting choice, with a possible switch to a **sliding 4-bit window** (closer to
NTSC composite fringing) after visual review. This experiment is that review.

## What was built

- `scr.rs` gains `DhgrColorMode { Aligned, Sliding }` +
  `Scr::set_dhgr_color_mode`. The **default stays `Aligned`**, so behavior,
  tests, and goldens are unchanged (full suite green).
- **Sliding decode:** each output pixel's colour comes from the trailing 4-bit
  window of the bit stream, each bit weighted by its NTSC colour phase:

  ```
  v(x) = Σ  bits[p] << (p & 3)   for p in x-3 ..= x     (out-of-range = 0)
  colour(x) = lores_palette[v(x)]
  ```

  At cell-aligned positions (`x = 4k+3`) this reduces *exactly* to the aligned
  cell value, so the two modes agree everywhere the bit pattern is stable.
- **Harness:** `ewm/tests/zz_dhgr_experiment.rs` renders a 5-band torture scene
  (colour bars / 1-px and 2-px lines / white blocks with a mid-cell edge and a
  1-px gap / `1010…` and `1100…` checkerboards / a diagonal phase ramp that
  shifts 1 px per line) in mono, aligned, and sliding. Reproduce with:

  ```
  git checkout iie/experiment-dhgr-color
  EWM_DHGR_OUT=/tmp cargo test --test zz_dhgr_experiment
  # -> /tmp/dhgr_{mono,aligned,sliding}.bmp
  ```

## Findings

**Where the modes agree (correctness cross-check):**

- Cell-aligned colour bars: byte-identical.
- The checkerboards are the strong proof of phase-correctness: `1010…` renders
  **solid grey** (v=5) and `1100…` **solid violet** (v=3) in *both* modes —
  periodic patterns produce stable solid NTSC hues, as real composite video
  does.

**Where they differ (only at non-cell-aligned edges — exactly where expected):**

- **White block with a mid-cell edge (x=13):** aligned produces a hard 4-px
  magenta block at the edge; sliding produces a thin 1-px-granular fringe —
  the classic composite look.
- **Diagonal ramp (clearest win):** aligned renders a chunky 4-px staircase
  with blocky magenta artifacts; sliding renders a smooth diagonal with narrow
  leading/trailing rainbow fringes, matching photos of real DHGR output.
- Thin 1–2 px lines: broadly similar (both smear a single bit across ~4 px);
  sliding shows a small colour gradient instead of one solid cell.

## Assessment

**Sliding is strictly better:** identical on stable content, materially more
hardware-like at edges, trivial cost. Caveat: it is still a hard palette lookup
per pixel, not true NTSC filtering — real fringes are softer (full NTSC
modelling à la AppleWin would be a much larger step and is not needed).

## If promoted to a real PR

1. Flip the DHGR Colour-mode default to `Sliding` (or wire a user-facing
   toggle). The mono DHGR golden (`two-e-dhgr.bmp`) is unaffected — it is a
   monochrome render.
2. Update `colour_cell_selects_the_palette` in `ewm/tests/two_e_dhgr.rs` — it
   asserts aligned semantics at a window edge (a lone cell of value 5 at x=0
   decodes differently under the sliding window's partial lead-in).
3. Either keep `zz_dhgr_experiment.rs` as a proper comparison test (rename, add
   assertions) or drop it.
4. Update the 6b "colour convention (revisit candidate)" note in
   `APPLE_IIE_ENHANCED.md` and the parity checklist row; this closes the last
   outstanding item before the `claude/apple-iie-enhanced` → `master`
   promotion.

## Ideal validation (still open)

Render a **known DHGR artwork** (a real disk image or memory dump) in both
modes and compare against a photo/screenshot of real hardware or a reference
emulator (AppleWin, MAME). The synthetic scene shows the mechanics; a real
image would settle the aesthetics.
