# Machine State — Save on Quit, Restore on Start

A working document for **persistent machine state**: start the emulator with
`--state=/some/mystate` and it restores that state at startup (when the file
exists) and saves it back at quit — suspend/resume for an Apple II, the way a
VM hibernates. Like `REMOTE.md` and `VNC.md`, re-read at the start of every
session and updated as phases land. **The tree must stay green
(`cargo fmt --check`, `clippy --all-targets -D warnings`, `cargo test`) after
every phase, and the golden-BMP tests remain the SDL tripwire.**

**Scope assumption (per the design brief):** the emulator is restarted with
the **same hardware configuration** (model, CPU, cards, media) that saved the
state. Detecting and rejecting a mismatch is explicitly **backlog** (§10) —
v1 may misbehave arbitrarily if you lie to it. This assumption is not
incidental: it is what lets the design cleanly split *construction* (from
config, as today) from *restoration* (state overlays a built machine), and it
resolves device identity (§3.3).

---

## 1. Goals and UX

```
ewm two --state ~/machines/dos33.state \
    --set machine:slots:6:drive1=disks/DOS33-SystemMaster.dsk
```

- **First run** (file absent): normal cold boot; the file is written at quit.
- **Later runs**: the machine resumes *exactly* where it left off — the
  BASIC program still in memory, the cursor mid-line, the disk arm on the
  same half-track, the accumulated CPU cycle count intact.
- Works in **both frontends**: the SDL window (save on quit/close) and the
  headless remote console (save on SIGINT/SIGTERM — this is what makes the
  REMOTE.md "VM farm" feel real: suspend and resume headless machines).
- Config equivalent (`state.path` block) so a machine's JSON config is
  self-sufficient; CLI overrides config, per the established pattern.
- Non-goals for v1: multiple named snapshots, save-while-running hotkey
  (easy later — the machine quiesces every frame), cross-version migration,
  compression, `one` (Apple 1) support. All §10.

---

## 2. Where the state actually lives (the architectural gift)

The Rust rewrite already centralized machine state under a single ownership
tree, which makes the serialization design almost fall out:

```
Two
 ├─ cpu: Cpu                      registers A X Y SP P PC, cycle counter
 │   └─ mem: Memory
 │       ├─ base_ram: Vec<u8>     the 48K fast path
 │       ├─ regions: Vec<Region>  Backing::Ram(Vec<u8>) | Rom(..) | Io(idx)
 │       └─ devices: Vec<Box<dyn Device>>   ← every stateful peripheral:
 │             TwoIo / IouE       soft switches, key latch, paddles,
 │                                speaker toggles; IouE owns the
 │                                aux: Box<dyn AuxCard> (aux RAM, RamWorks)
 │             Alc / Saturn       language-card & Saturn banking + RAM
 │             Dsk                Disk II: drives, arm, motor, latch, media
 │             Hdd / Liron        block devices: registers (+ see §5)
 │             Clk                Thunderclock (reads the host clock)
 ├─ model, slot0, DeviceHandle<…> construction data — NOT state
 └─ io: MachineIo                 a handle — NOT state
```

Everything mutable at runtime is reachable as `Cpu` → `Memory` →
(`base_ram` | `Ram` regions | `devices[i]`). `Two`'s own fields are
construction-time wiring. **The ownership tree is the serialization tree**;
no cross-cutting state collector is needed, and no component's internals
leak out of its module — which is exactly the "keep store/load local to the
component" property the design brief asks for.

---

## 3. The trait

### 3.1 Shape

One trait, both directions (the brief's `StateRestoration` idea; suggested
name **`Persist`**, since it covers save *and* restore — `StateRestoration`
names only half, and `State` collides with the file/module noun. Final
bikeshed at implementation time):

```rust
/// Component-local machine-state persistence (notes/STATE.md). Lives in
/// ewm-core next to `Device`; zero dependencies, like everything else.
pub trait Persist {
    /// Append this component's state to the writer. Only *runtime* state:
    /// anything reconstructible from config/ROMs is not written.
    fn save(&self, w: &mut state::Writer);

    /// Restore from a reader positioned at this component's payload.
    /// Must fully overwrite every field that `save` recorded; on `Err` the
    /// machine is considered unusable (§6, all-or-nothing).
    fn restore(&mut self, r: &mut state::Reader) -> Result<(), state::Error>;
}
```

### 3.2 Enforcement: make it a supertrait

```rust
pub trait Device: Any + Persist { … }     // ewm-core
pub trait AuxCard: … + Persist { … }      // ewm (IouE delegates to it)
```

This is the key "consistent restore" mechanism: **the compiler refuses any
new card or device that has not answered the persistence question.** A
stateless device (Thunderclock) writes an explicitly empty impl — a visible,
reviewable decision rather than a silent omission. The known residual gap:
nothing forces state *outside* the device tree into the snapshot — mitigated
by the fact that there essentially isn't any (§2), and by the determinism
gate (§8), which fails loudly when something observable was missed.

Consequence to accept up front: the supertrait makes "implement persistence
one device per PR" impossible — the moment `Device: Persist` lands, every
impl must exist. §9 sequences around this: the trait and the structural
(CPU/RAM) work land first without the supertrait bound; one mechanical PR
then adds the bound plus all device impls.

### 3.3 Composite orchestration and identity

`Persist` composes down the ownership tree; each level orchestrates its
children **in a fixed, explicit order**:

- `Two::save` → a small `INFO` chunk (model name, media paths — advisory
  today, the §10 fingerprint later) + the `CPU` chunk.
- `Cpu::save` → registers/counter + the `MEM` chunk.
- `Memory::save` → `base_ram`, each `Backing::Ram` region (by region
  index; `Rom` regions are skipped — immutable, rebuilt at construction),
  then `devices[i]` for every `i`, each as an index-tagged `DEV` chunk.
- Owners delegate: `IouE::save` includes its aux card's payload,
  `Dsk::save` its drives and media.

**Device identity is the construction index.** Construction is
deterministic from the config, and same-config is the stated precondition —
so index identity is sound, needs no registry of names, and can never
disagree with the machine actually built. (The backlog fingerprint (§10)
is what will turn "you lied about the config" from undefined behavior into
a clean error.)

Restore mirrors save exactly: the machine is **built from config first,
exactly as today** (ROMs loaded, cards constructed, media inserted), then
state overlays it in the same fixed order, replacing the initial
`cpu.reset()`. The state file never constructs anything — it is an overlay,
not a machine description. That separation is what keeps this feature small.

---

## 4. The container format (`state.rs`, hand-rolled)

In the house style (BMP, PNG, WOZ, DES, SHA-1…): a tagged-chunk binary
container, no serde, no compression, everything little-endian. serde/JSON
was considered and rejected: the payload is dominated by RAM and media
images (hundreds of KB to MB), which JSON+base64 handles embarrassingly and
which would drag mirror "state structs" alongside every component.

```
file   := magic "EWMS" | u32 version (=1) | chunk*
chunk  := tag [u8;4] | u32 length | payload (length bytes)
```

- Chunks nest freely (`CPU` contains `MEM` contains `DEV`s); `Writer` and
  `Reader` are a Vec builder and a bounds-checked slice cursor with typed
  helpers (`u8/u16/u32/u64/bytes/str`). Corrupt or truncated input is an
  `Err`, never a panic — state files are local and trusted, but the parser
  is still total.
- **Strict v1**: version must match; unknown or missing chunks are errors.
  (Skip-unknown forward compatibility is a §10 decision, not an accident.)
- **Atomic save**: write `path.tmp`, then rename over `path`. A crash
  mid-save leaves the previous state intact.
- Expected sizes: ][+ ≈ 80–100 KB + ~230 KB per loaded floppy; a //e with
  RamWorks: up to a few MB. Fine uncompressed.

---

## 5. Component inventory — what saves, what pointedly does not

| Component | Saves | Skips (and why) |
|---|---|---|
| `Cpu` | A X Y SP P PC, `counter` | breakpoints, trace, `strict` — debug/session config, not machine state |
| `Memory` | `base_ram`, `Ram` regions, `cycles` mirror, all devices | `Rom` regions (immutable), watchpoints (debug) |
| `TwoIo` / `IouE` | every soft switch, key latch, paddle/button state, pending speaker toggles, aux card payload | — |
| `AuxCard` impls | aux RAM banks, bank-select (RamWorks) | — |
| `Alc` / `Saturn` | banking state, banked RAM | — |
| `Dsk` | selected drive, half-track, motor + spin-down stamp, latch/mode, **full media contents + dirty flags** | — (see below) |
| `Hdd` / `Liron` | controller registers/command state | media — writes already flush to the backing file, which is the durable copy; restore re-reads it at construction |
| `Clk` | *nothing* (empty impl) | it answers from the host clock — "now" is correct by definition |
| `Scr`, `Snd`, palette, frontends | *nothing* — not in the tree | pure/derived state; after restore the frontend marks the screen dirty and rendering/audio re-derive from the machine |

**The Disk II is the one component where media belongs in the snapshot**:
floppy writes are in-memory only (nothing in `dsk.rs` writes the image file
back), so a resumed session must carry the modified image or silently lose
writes. This also documents an existing sharp edge worth its own backlog
line: today, quitting *without* `--state` discards floppy writes.

**Cycle-stamp rule**: several components hold absolute cycle timestamps
(motor spin-down, speaker toggles, paddle timers) relative to
`cpu.counter`. The counter is saved and restored verbatim, so **stamps are
never rebased** — save them as-is and the arithmetic stays coherent. No
component may store wall-clock-derived state (only `Clk` touches the host
clock, and it stores nothing).

**Quiescence rule**: save and restore only happen between CPU steps — the
frame boundary, where both frontends already sit. There is no
mid-instruction state anywhere, so a frame-boundary snapshot is complete by
construction.

---

## 6. Lifecycle wiring

- **CLI**: `--state <path>` on `ewm two` (and the config block
  `"state": { "path": … }`, resolved relative to the config file like other
  paths; CLI overrides config).
- **Startup**: `build_machine()` exactly as today → if the state file
  exists, `two.restore_state(path)` *instead of* `cpu.reset()`; absent file
  → normal reset. **Restore failure is fatal**: print why and exit — never
  run a half-restored machine. (All-or-nothing comes free: restore happens
  before the first step, so the failure path has nothing to unwind.)
- **Quit paths**:
  - SDL frontend: the existing quit event / window close → save, then exit.
  - Headless serve loop: install SIGINT/SIGTERM handlers that set an
    `AtomicBool`; the frame loop checks it, saves, and exits. No new
    dependency: declare the two libc functions with a direct
    `unsafe extern "C"` block (the platform libc is already linked) —
    the handler only stores a relaxed atomic, which is async-signal-safe.
  - Save failure at quit: report and exit nonzero, leaving any previous
    state file untouched (§4 atomicity).
- **Remote synergy**: with the serve path wired, REMOTE.md Phase 6
  orchestration gets VM-style suspend/resume for free
  (`systemctl stop ewm-vnc@dos33` hibernates the machine).

---

## 7. Design decisions in one place

| Decision | Choice | Rejected alternative |
|---|---|---|
| Trait shape | one `Persist` trait, save+restore, composed down the ownership tree | separate Save/Restore traits (two halves that must agree anyway); visitor walking a machine description (duplicates the ownership tree) |
| Enforcement | `Device: Persist`, `AuxCard: Persist` supertraits | opt-in registry (new devices silently skipped — the bug class this feature least wants) |
| Identity | device construction index | stable name registry (more machinery; solves only the config-mismatch case that is explicitly backlog) |
| Format | hand-rolled LE chunk container | serde: mirror-struct boilerplate + pathological on big binary payloads; bincode-style crate: dependency |
| Restore model | config builds, state overlays | state file describes the machine (turns the file into a second config format; huge surface) |
| Failure | all-or-nothing, fatal at startup | best-effort partial restore (undebuggable half-machines) |
| Media | floppies in the snapshot; block devices via their backing files | all media in snapshot (bloats; block files are already the durable copy) |

---

## 8. Testing

- **`state.rs` unit tests**: chunk round-trips, nesting, truncation, bad
  magic/version, the typed reader against hand-written byte vectors.
- **Per-component round-trips**: for each `Persist` impl, save → restore
  into a freshly constructed twin → assert observable equality (registers,
  RAM, `text_screen()`, media bytes, switch states via the existing
  accessors).
- **The determinism gate** (the state analogue of the golden-BMP tests):
  boot DOS 3.3 headless for N frames → save → restore into a fresh machine
  → (a) framebuffer and `text_screen()` identical to the original at the
  save point, and (b) run **both** machines K further frames — still
  identical. (The machine is deterministic modulo host-clock reads and
  input; the gate runs input-free with no Thunderclock dependency.) This is
  the test that catches any state a component forgot to save.
- **CLI end-to-end**: run with `--state`, type `X=42`, quit; restart;
  `PRINT X` → `42`.

---

## 9. Phases (one PR each, into a `state` integration branch or straight to `main` — decide at kickoff)

| Phase | Description | Size | Status |
|---|---|---|---|
| S0 | This plan; `--state` / config `state.path` parsing | S | Done (in S4) |
| S1 | `state.rs` container: Writer/Reader, chunk nesting, atomic file save, error type; unit vectors | S | Done |
| S2 | `Persist` trait; impls for `Cpu` + `Memory` (base RAM, Ram regions) — **no supertrait yet**, devices skipped; round-trip test of a cardless machine | M | Done |
| S3 | The supertrait flip: `Device: Persist` + `AuxCard: Persist` and **all** device impls; full-machine round-trips | L | Done |
| S4 | Lifecycle: startup restore-instead-of-reset, SDL quit save, serve SIGINT/SIGTERM save; fatal-error paths; docs | M | Done |
| S5 | The determinism gate + end-to-end test; REMOTE.md note on suspend/resume | S | Done |


### As built (deviations worth recording)

- **The S0 stub never existed.** All phases landed in one PR, so the
  "state persistence not built yet" placeholder was pointless; the CLI and
  config surface shipped inside S4, where it works.
- **A model seatbelt shipped early.** The `INFO` chunk names the model and
  restore rejects a mismatch — a two-line edge of the backlog fingerprint
  that was too cheap to defer. The full config fingerprint remains backlog.
- **`Clk` and `Pia` save slightly more than planned**: the Thunderclock's
  latched time string mid-read (a suspended ProDOS clock read resumes
  correctly) and the PIA's undrained output queue. Both cost bytes, not
  complexity.
- **The determinism gate saves mid-boot** (motor on, arm seeking) rather
  than at the idle prompt — the harsher variant of §8, and it passes: the
  restored twin stays cycle- and pixel-identical through millions of
  further cycles and both machines finish booting to identical screens.

---

## 10. Backlog (explicitly out of v1)

- **Config fingerprint**: hash the effective machine config (model, slots,
  aux, media identity) into `INFO`; refuse restore on mismatch with a clear
  diff-style message. This converts the v1 precondition into a checked error.
- Skip-unknown chunks / versioned migration (forward compatibility).
- `one` (Apple 1 / Replica 1) through the same trait — `Pia` and TTY are
  tiny; blocked only by prioritization.
- Save-while-running: palette command and/or periodic autosave (the frame
  boundary makes this nearly free once S4 lands).
- Named snapshots (`--state` a directory; save slots).
- Compression (worth revisiting only if RamWorks-sized states annoy).
- Floppy write-back as a feature of its own (today's quit discards floppy
  writes even without `--state` — surprising, and orthogonal to state).
- WozBug: dump/inspect a state file offline.
