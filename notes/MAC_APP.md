# EWM as a Native Mac Application — Implementation Plan

A working document for packaging EWM as a double-clickable macOS
application, growing toward a VMware-Fusion-style "emulator library". In the
house style of `APPLE_IIE_ENHANCED.md` / `WOZ1.md`: re-read at the start of
every session, update as phases land. **The tree must pass all verification
gates (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`) after every phase.**

> **Branch strategy:** each phase is independently useful and lands as its
> own branch + PR into `master` (no long-lived integration branch needed).

## The two shapes (owner's framing)

1. **`EWM.app`** — double-click and the app as-is starts (the boo menu).
2. **VMware Fusion style** — a launcher / "emulator library" app that starts
   packaged machines, each running as its own application.

**Decision: build 1 first** — it produces exactly the artifacts (bundle,
launch plumbing, file associations, machine configs) that 2 orchestrates.

## Status

| Phase | Description | Size | Status |
|---|---|---|---|
| 1 | `EWM.app`: self-contained bundle, icon, file associations, drag-drop | M | **Done** (PR pending) |
| 2 | Distribution: Developer ID signing, notarization, DMG, CI artifacts | M | Not started |
| 3 | Machine documents: `--config` TOML + the `.ewmachine` bundle | M | Not started |
| 4 | The library app: browse machines, spawn each as its own app instance | L | Not started |

## Grounded facts (verified against the tree)

- The binary currently links **Homebrew's `libSDL3.0.dylib` dynamically**
  (`use-pkg-config` feature; confirmed with `otool -L`) — an app bundled
  as-is only runs on Macs with Homebrew SDL3.
- The sdl3 crate (0.18) offers **`build-from-source-static`**: SDL3 compiled
  from source and linked statically → a fully self-contained binary. Needs
  CMake at build time.
- The rewrite already made the binary **Finder-launch-safe**: every ROM is
  `include_bytes!`, nothing depends on the working directory (Finder
  launches with `cwd=/`). Zero-arg `ewm` boots the boo menu — the natural
  app entry point.
- macOS delivers Finder-opened documents as **Apple Events, not argv**;
  SDL3 surfaces them as drop events (`Event::DropFile`), the same mechanism
  as dragging a file onto the window.

## Phase 1 — `EWM.app` (M)

**Goal:** a double-clickable, self-contained app; double-clicking a disk
image boots it.

**Scope:**
- **Static SDL for bundle builds.** A build profile/feature so bundle builds
  use `sdl3/build-from-source-static` while dev builds keep the fast
  Homebrew path. Gate: `otool -L` on the bundled binary shows no
  `/opt/homebrew` references.
- **Hand-rolled bundling script** (`scripts/make-app.sh`) — house style over
  `cargo-bundle`/`cargo-packager`: build release, assemble
  `EWM.app/Contents/{MacOS,Resources}`, write `Info.plist`, copy the icon,
  **ad-hoc codesign** (`codesign -s -`; runs locally without a paid
  identity).
- **`Info.plist`:** bundle id **`ca.arentz.ewm`**, `CFBundleName` EWM,
  `NSHighResolutionCapable`, and `CFBundleDocumentTypes` for
  `.dsk`/`.do`/`.po`/`.nib`/`.woz` (floppy images) and `.hdv` (hard-drive
  image), registered at **Alternate** rank so EWM does not steal
  associations from other emulators uninvited.
- **Icon rendered by the emulator itself:** generate the artwork with the
  `chr` character generator (green-phosphor `][` glyphs on a dark bezel),
  PNG set → `iconutil` → a committed `EWM.icns` plus the generator so it is
  reproducible.
- **Open-with + drag-and-drop:** handle `Event::DropFile` in the boo and
  two loops. A floppy image boots the **][+** with it in drive 1 (today's
  default model; Phase 3 configs make this choosable), `.hdv` mounts on
  slot 7. Dropping onto a *running* machine swaps drive 1 — the first
  sliver of the disk-management backlog item.

**Gate:** the script produces an app that boots the boo menu from Finder;
`open disks/DOS33-SystemMaster.dsk` (or a double-click) boots DOS 3.3;
`otool -L` is Homebrew-free; the full test suite is untouched.

## Phase 2 — Distribution polish (M)

**Goal:** an app safe to hand to strangers.

**Scope:** Developer ID signing (script takes the identity as an optional
parameter — requires the owner's Apple Developer membership), notarization
via `notarytool` + stapling, a DMG via `hdiutil`, and a GitHub Release
artifact built on a macOS CI runner. Auto-update (Sparkle et al.) is out of
scope.

**Gate:** a notarized DMG downloads and opens cleanly on a Mac that has
never seen Homebrew, with no Gatekeeper override.

## Phase 3 — Machine documents (M) — the bridge to the library

**Goal:** a machine as a file you can double-click.

**Scope:**
- The backlog's **config-file idea**: `ewm two --config machine.json` —
  **JSON, not TOML** (owner's decision; full plan in
  `notes/JSON_CONFIG.md`, including the JSON Schema and configurable
  slots), populating the same `Options` the flags do.
- An **`.ewmachine` document**: a folder bundle holding the JSON config plus
  its disk images (and, once save states exist, the machine state) — the
  analog of a VMware VM bundle. Registered in `CFBundleDocumentTypes`
  (Owner rank: this type is ours); double-clicking one boots that machine.

**Gate:** a checked-in example `.ewmachine` boots the //e with RamWorks and
a mounted disk from a double-click.

## Phase 4 — The library app (L) — option 2 realized

**Goal:** the VMware-Fusion experience.

**Scope (sketch — planned in detail when reached):** a launcher that lists
`.ewmachine` documents (a grown-up boo menu, or a small native shell),
creates/edits them, and starts each machine as **its own app instance**
(`open -n EWM.app --args --config …` → separate windows and Dock presence
per running machine). By this point it is pure orchestration over Phases
1 + 3.

## Defaults taken (veto anytime)

- Bundle identifier: `ca.arentz.ewm`.
- Phase 1 signs **ad-hoc**; real signing waits for Phase 2.
- A bare double-clicked floppy boots the **][+** until machine configs
  exist.

## Risks & open questions

- **Static SDL build time and licensing:** SDL3 is zlib-licensed (fine to
  link statically); the source build adds minutes to bundle builds only.
- **Drop-event timing at launch:** the odoc event may arrive before the boo
  loop polls; verify SDL queues it (it should — delivered post-init) and
  handle it in the first frames. *(Phase 1 note: handled in the boo loop's
  normal poll; verified by the owner's double-click test.)*
- **Phase 1 deviation:** no `EWM.icns` binary is committed — the script
  regenerates it from `examples/icon.rs` on every build (same
  reproducibility, no binary artifact in the repo).
- **`.ewmachine` layout** (Phase 3): folder bundle vs single TOML with
  relative paths — decide when reached; folder bundle is the working
  assumption.
