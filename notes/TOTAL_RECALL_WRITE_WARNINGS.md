# Total Replay: "[A2P] Unexpected write" warnings explained

Booting a Total Replay `.hdv` image produces a burst of warnings like:

```
[A2P] Unexpected write at $C00E
[A2P] Unexpected write at $C00C
[A2P] Unexpected write at $C002
...
[A2P] Unexpected write at $C06A
[A2P] Unexpected write at $C074
```

These are **harmless and expected**. They are Total Replay probing or
normalizing hardware that does not exist on an Apple ][+. On real hardware
these writes are no-ops; the emulator treats them the same way, just noisily
(the messages come from the C-inherited `eprintln!` in `TwoIo`'s soft-switch
catch-all, `ewm/src/two.rs`).

> **On the Enhanced //e these are *not* no-ops.** Every `$C00x`/`$C05x` address
> below is a real, implemented soft switch on the //e (`IouE` in `two.rs` — see
> `notes/APPLE_IIE_ENHANCED.md`): `$C00E`/`$C00C` are ALTCHARSET/80COL,
> `$C002`/`$C004` are RAMRD/RAMWRT, `$C008` is ALTZP, `$C00A`/`$C00B` are
> INTC3ROM/SLOTC3ROM, and `$C05E`/`$C05F` are the DHIRES switch (under IOUDIS).
> The "unexpected write" warnings therefore come only from the **][+** path;
> the //e path acts on them. The `$C06x`/`$C074` accelerator registers remain
> unimplemented on both machines.

Every address in the log maps to a specific place in the Total Replay source
(https://github.com/a2-4am/4cade, `src/`):

## `$C000-$C00F` — IIe memory-management soft switches

**Cold-boot machine normalization** — `4cade.init.machine.a:8-16`, whose
header says "assumes absolutely nothing about machine state". TR
unconditionally resets the IIe soft switches, which are harmless writes on a
][+:

| Address | IIe switch | Purpose |
|---|---|---|
| `$C00E` | PRIMARYCHARSET | no mousetext character set |
| `$C00C` | CLR80VID | 40 columns |
| `$C000` | STOREOFF (80STORE off) | *not in the log*: this write is silently ignored (issue #84 quirk) |
| `$C002` | READMAINMEM | read from main (not aux) memory |
| `$C004` | WRITEMAINMEM | write to main memory |
| `$C008` | SETSTDZP | main-memory stack and zero page |

**VidHD detection** — `4cade.init.a:44-47`: `$C00B` (SETC3ROM, external
slot 3 ROM) before calling `HasVidHDCard`, `$C00A` (CLRC3ROM) after. TR
supports super-hires artwork via a VidHD card even on 8-bit machines.

**The 128K memory check** — `hw.memcheck.a:76-77` restores
WRITEMAINMEM/READMAINMEM (`$C004`/`$C002`) after probing aux memory; later
`fx.lib.a` brackets DHGR screen transitions the same way. This is TR
discovering the machine has 64K, not 128K — which is why it offers 472 games
instead of the full catalog.

## `$C06A/$C06B/$C06D` — FASTChip //e accelerator

`hw.accel.a:119-127` and `hw.joystick.a:30-31`. The FASTChip unlock protocol
is writing `$6A` to `$C06A` **four times**, then `$C06B` to enable, `$C06D`
to set speed, and a final `$C06A` write to lock — the exact
`C06A ×4, C06B, C06D, C06A` signature in the log. It repeats because TR
re-normalizes accelerator speed around disk I/O, joystick reads, and game
launch (games must run at authentic 1 MHz).

## `$C074` — TransWarp I / Laser 128EX speed register

`hw.accel.a:112,131` (the source comments that it "may overlap with paddle
trigger"). Same purpose: force 1 MHz if such an accelerator is installed.

## Why the Zip Chip probe is *not* in the log

TR also probes the Zip Chip, whose lock register `$C05A` overlaps the
annunciator range `$C058-$C05F` — which `TwoIo` ignores silently, as the
C did. TR's own source flags that overlap.

## The noise is now gated

The "Unexpected read/write" messages are gated behind `--debug` (PR #215), so
a normal `two` run is quiet; pass `--debug` to see them.
