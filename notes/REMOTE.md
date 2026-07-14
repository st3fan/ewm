# Remote Console (VNC / RDP) — Implementation Plan

A working document for making EWM machines bootable as **headless "VMs"**
that you connect to over the network — with a native VNC/RDP client *and*
from a web page, the way the Proxmox console works. The end state: start any
number of machines, each listening on its own port, and reach every one of
them remotely, including from a browser.

Like `REWRITE.md` and `APPLE_IIE_ENHANCED.md`, this is meant to be re-read at
the start of every session and updated as phases land. **The tree must build
and pass all verification gates (`cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings`, `cargo test`) after every phase.** The
existing SDL frontends must stay **byte-for-byte** unchanged in behaviour —
the golden-BMP tests are the tripwire.

> **Branch:** All remote-console work happens on the long-lived
> **`claude/remote-console`** integration branch, kept separate from `master`
> until the feature is complete. Each sub-phase is cut *from* that branch and
> opened as a PR *into* it (never into `master`). One final PR promotes the
> integration branch to `master`. **One sub-phase = one PR.**

Scope note: this is a **Linux-first** feature and may lean on a Linux/X11
host for deployment tooling, exactly as the task allows. Nothing in the core
design is Linux-specific (it is pure Rust + `std::net`), but the orchestration
recipes (systemd, reverse proxy) target Linux.

---

## 1. Goals and non-goals

**Goals**

1. Boot an EWM machine with **no local window** — no SDL, no GPU, no X
   server required on the host.
2. Serve that machine's screen and keyboard/paddle input over a **standard
   remote-desktop protocol** so an off-the-shelf client can connect.
3. Run **N machines on N ports** simultaneously, each independent.
4. Connect **from a web page** — a self-contained console like Proxmox's,
   no plug-in, no installed client.
5. Keep it a **single self-contained binary** where practical (the Proxmox
   feel), and keep the dependency budget small and blocking-I/O-based, in the
   spirit of the existing WozBug TCP server.

**Non-goals (v1)**

- **Audio.** The RFB/VNC protocol has no audio channel; the speaker beep is
  dropped over VNC. Section 9 sketches a later path (WebAudio side-channel /
  RDP `rdpsnd`), but v1 is silent.
- **Multi-user collaboration semantics**, session recording, file transfer.
- **RDP as the primary protocol** — it is a large, separable optional track
  (Section 12, Track B). VNC/RFB is the recommended v1.

---

## 2. Decision: VNC (RFB) is the primary protocol; RDP is optional Track B

The task says "either VNC or RDP, whatever is more feasible." **RFB (the VNC
wire protocol, RFC 6143) wins decisively** for EWM:

| | **VNC / RFB** | **RDP** |
|---|---|---|
| Minimal server size | A few hundred lines (Raw encoding) | Large: T.128, capability negotiation, bitmap/RemoteFX codecs, virtual channels |
| Security stack required | Optional VNC auth; TLS via reverse proxy | TLS + CredSSP/NLA effectively expected by modern clients |
| Rust libraries | `rfb` (Oxide, trait-based), `rustvncserver`, or hand-roll | `ironrdp-server` — an *extendable skeleton*, Tokio-based, much more to wire up |
| Runtime model | Blocking `std::net` threads (matches WozBug) | Requires the Tokio async runtime |
| Framebuffer fit | We already produce exactly `&[u32]`; Raw is trivial at 280×192 / 560×192 | Expects codecs and larger desktops; overkill for an Apple II frame |
| Browser client | **noVNC** — mature, canvas-based, HTML5 only | `ironrdp-web` (WASM) — real (Cloudflare/Devolutions ship it) but heavier to embed |
| Input fit | `KeyEvent` (X11 keysyms) + `PointerEvent` map straight onto `two.key()` / paddle | PS/2 scancodes; more translation |

RFB's Raw encoding is genuinely tiny, and our frames are tiny (280×192×4 =
210 KB worst case, redrawn only when `screen_dirty`). Bandwidth is a
non-issue, so we do not even *need* compression to start. RFB input maps
one-to-one onto EWM's existing input surface. And the browser story — noVNC —
is the exact thing Proxmox uses.

RDP buys us Windows' built-in `mstsc` and a real audio channel, but at a cost
that is not justified for an 8-bit framebuffer. It stays as an optional later
track for people who specifically want native RDP.

**Building the RFB server:** hand-roll a minimal server in `std::net`
(blocking, thread-per-connection, like `wozbug::Server`), rather than pull in
a Tokio-based crate. This matches EWM's culture — the CPU, the BMP writer, the
PNG writer, the TTY, the palette, and the sound synthesis are all
implemented from scratch and unit-tested, with a deliberately small dependency
list. The Oxide `rfb` crate (consumer implements a `Server` trait that
supplies the framebuffer) and `rustvncserver` are excellent **references**,
and remain fallbacks if the hand-rolled path stalls, but the default is a
small `rfb.rs` we own and test the way `snd.rs`/`scr.rs` are tested.

---

## 3. Why EWM is unusually well-suited to this

Three facts about the current codebase make the embedded-server approach the
natural one — and make the "run the real app under a virtual display" approach
the *unnatural* one.

**3.1 The renderer is already pure and headless.** `scr::Scr::update()` is
SDL-free: it fills a `Vec<u32>` (`pixels`, 280-wide ][+ / `wide`, 560-wide
//e) in the renderer's pixel layout, and `scr.frame(model)` returns the
buffer. The Apple 1 / Replica 1 path is the same shape: `tty::Tty` renders
character cells into `tty.pixels: Vec<u32>`. This is already exercised
headlessly by the hidden `--screenshot` flag, the golden-BMP tests, and
`encode_bmp()`. **A VNC server needs precisely this buffer and nothing more.**
The overlay renderers (`render_status_bar`, `render_led_strip`,
`palette::Palette`, the pause `Tty`) are *also* pure `Vec<u32>` producers, so
the remote frontend can composite the same overlays into one framebuffer if we
want them later.

**3.2 EWM already embeds a network server in the frame loop.** `wozbug::Server`
(the `--wozbug` line server, `notes/DEBUGGING_TOOLS.md`) binds a
`TcpListener` on a background thread, talks to the machine over `mpsc`
channels, and the frame loop drains it between frames. **The VNC server is the
same pattern**: a listener thread per port, input events posted to the driver
over a channel, framebuffer snapshots handed back. No new concurrency model.

**3.3 The "lean on existing X11 tools" path is actively blocked by our own
code.** `sdl::check_renderer()` **rejects the software renderer** and requires
an accelerated one. Under `Xvfb` (a headless X server with no GPU) SDL3 falls
back to software → **EWM refuses to start**. So "run the normal SDL binary on
a virtual `:99` display and point `x11vnc` at it" fights the existing renderer
gate, drags a full X server into every VM, and still needs all the input
plumbing. The embedded server needs no X server, no GPU, no display — it
reuses the pure renderer directly. (Xvfb+x11vnc is written up and rejected in
Section 11.)

The input surface we must drive is small and already factored:

- `two.key(u8)` — a single ASCII entry point (the SDL loop's whole keyboard
  handler ultimately funnels here).
- `two.set_button(n, state)` / `two.set_joystick(Option<(i16,i16)>)` — paddles.
- `two.cpu.reset()`, `two.load_disk(drive, path)` — control.
- `one` has the analogous `one.key()` / PIA feed.

The SDL `KeyDown` handler in `two::main` (CR→`0x0d`, arrows→`0x08/0x15/0x0b/
0x0a`, ESC→`0x1b`, Ctrl-A..Z → 1..26, Alt-1..4 → paddle buttons) **is the
translation table** we re-target from SDL keycodes to RFB/X11 keysyms.

---

## 4. Target architecture

```
   ┌──────────────────────── one host, one process per machine ────────────────────────┐
   │                                                                                     │
   │  ewm two --serve vnc://0.0.0.0:5901 --config configs/enhanced.json                  │
   │  ┌───────────────────────────────────────────────────────────────────────────┐    │
   │  │  Machine driver (shared, frontend-agnostic — Phase 1)                       │    │
   │  │    step CPU (speed/fps cycles) → Scr/Tty.update() → frame(): &[u32]         │    │
   │  │        ▲ input (key/button/joystick/reset)        │ framebuffer            │    │
   │  └────────┼───────────────────────────────────────────┼───────────────────────┘   │
   │           │                                            ▼                            │
   │  ┌────────┴────────────────────────────────────────────────────────────────┐      │
   │  │  rfb.rs — embedded RFB server (background thread, std::net, like WozBug)  │      │
   │  │    RFB handshake · ServerInit · Raw FramebufferUpdate · KeyEvent/Pointer  │      │
   │  │    ├─ plain TCP  :5901   ── native VNC clients (TigerVNC, RealVNC, …)     │      │
   │  │    └─ WebSocket  :5701   ── noVNC in a browser  (tungstenite upgrade)     │      │
   │  └──────────────────────────────────────────────────────────────────────────┘      │
   └─────────────────────────────────────────────────────────────────────────────────────┘

   Web tier (Proxmox-console equivalent)
   ┌───────────────────────────────────────────────────────────────────────────────────┐
   │  Browser ──HTTP──> static noVNC page (vendored, self-contained)                     │
   │          ──WSS──>  ws://host:5701  (RFB-over-WebSocket, straight into rfb.rs)        │
   │                                                                                     │
   │  "Hub" index page lists running machines and links to each console (like the        │
   │  Proxmox node view). Optional reverse proxy (nginx/Caddy) terminates TLS + auth.    │
   └───────────────────────────────────────────────────────────────────────────────────┘

   Fleet:  ewm-vnc@enhanced  → :5901/:5701      ewm-vnc@plus → :5902/:5702   …
           (systemd template unit or scripts/ewm-farm.sh; config name → port)
```

**Process model:** one machine = one process = one VNC port (+ one WebSocket
port). This is the QEMU/Propolis/Proxmox model and it makes "N VMs on N ports"
fall out for free — no shared state, crash isolation, trivial orchestration.

---

## 5. Web access — three ways, and which we pick

The web-console requirement (noVNC-style, like Proxmox) has three viable
shapes. We can support more than one; the recommendation is **(A) as the
built-in default, with (C) documented for people who already run a gateway.**

**(A) Embedded WebSocket + vendored noVNC (recommended default).** Put a
WebSocket upgrade (sync `tungstenite`) *in front of* the RFB state machine in
`rfb.rs`, so a browser's noVNC connects **directly to EWM** with no sidecar —
exactly how LibVNCServer/QEMU expose their built-in web consoles. Ship a tiny
static page bundling noVNC, served by a small built-in HTTP handler on the
same port. This is the true "single binary, Proxmox feel" experience:
`http://host:5701/` → live Apple II in a tab.

**(B) websockify in front of a plain-TCP VNC port (zero code).** The stock
noVNC deployment: `websockify` bridges browser WebSocket ↔ our plain
`:5901`. Requires no EWM code at all, so it works from **Phase 2** as an
interim browser path before (A) lands. Documented as the fallback.

**(C) Apache Guacamole gateway (optional, best for fleets + auth).** Guacamole
(`guacd` + web app) is a protocol-agnostic HTML5 gateway: point it at our VNC
ports and it gives a browser console **plus** authentication, a connection
list, and TLS for free — a ready-made multi-VM front door. It is a
*deployment* recommendation, **not** a substitute for EWM speaking a protocol:
EWM still serves VNC; Guacamole is one possible web tier over it. Recommended
for anyone running many machines who wants auth without building it.

We do **not** need a browser RDP client for v1. If Track B (RDP) is ever
built, its browser story is `ironrdp-web` (WASM), which Cloudflare and
Devolutions ship in production.

---

## 6. Config and CLI surface

A new top-level `remote` block in the JSON config (`config.rs` already has
`Display`, `Machine`, `Input`, `Boot`, `Debug`; this slots alongside), plus a
`--serve` CLI shorthand:

```jsonc
{
  "machine": { "model": "2e", "slots": { "6": { "card": "diskii" } } },
  "display": { "monitor": "rgb" },
  "remote": {
    "protocol": "vnc",          // "vnc" (v1). "rdp" reserved for Track B.
    "bind": "127.0.0.1",        // default localhost; opt in to 0.0.0.0
    "port": 5901,               // plain-TCP RFB
    "websocket": 5701,          // optional; enables browser noVNC (Phase 4)
    "web": true,                // serve the vendored noVNC page (Phase 5)
    "password": null,           // null → VNC "None" auth (localhost/tunnel only)
    "view_only": false
  }
}
```

```
ewm two --serve vnc://0.0.0.0:5901 --config configs/enhanced.json
ewm two --serve vnc://0.0.0.0:5901?ws=5701&web=1 --config configs/plus.json
```

`--serve` parses to the same `remote` struct and is mutually exclusive with
opening an SDL window. When `remote` is present (or `--serve` given), `main`
takes the **headless driver + RFB frontend** path instead of the SDL path.

---

## 7. Input mapping (RFB → machine)

RFB `KeyEvent` carries an X11 keysym + down flag; `PointerEvent` carries
button mask + x/y. The translation is a direct re-targeting of the existing
SDL `KeyDown`/`KeyUp` logic:

| RFB keysym | Machine action |
|---|---|
| ASCII `0x20`–`0x7E` (down) | `two.key(byte)` |
| `XK_Return` `0xFF0D` | `two.key(0x0D)` |
| `XK_BackSpace 0xFF08` / `XK_Left 0xFF51` | `two.key(0x08)` |
| `XK_Right 0xFF53` | `two.key(0x15)` |
| `XK_Up 0xFF52` / `XK_Down 0xFF54` | `two.key(0x0B)` / `0x0A` |
| `XK_Escape 0xFF1B` | `two.key(0x1B)` |
| Ctrl held + `A`–`Z` | `two.key(letter - 'A' + 1)` (1–26) |
| `XK_Tab 0xFF09` | `two.key(0x09)` then `0x7F` (mirrors the SDL quirk) |
| `XK_Delete 0xFFFF` | `two.key(0x7F)` |
| Reset | a chosen combo (e.g. Ctrl+F12) → `two.cpu.reset()` |

Modifier state (Ctrl/Alt) is tracked from keysym up/down, exactly as
`keymod` is today. Paddle buttons reuse the Alt-1..4 mapping, or move to
`PointerEvent` buttons once the AppleMouse card lands (see `IDEAS.md`). The
`//e` vs `][+` keyboard differences (lower case, Open/Solid-Apple) are the
same branch the SDL path already takes on `two.model()`.

RFB clipboard (`ClientCutText` / the ExtendedClipboard pseudo-encoding) is an
optional nicety for paste-in; deferred past v1.

---

## 8. Rendering / RFB details

- **Pixel format:** advertise a `PIXEL_FORMAT` in `ServerInit` that matches
  our `PixelLayout` (32-bit, 8-8-8), so the framebuffer ships with **no
  per-pixel conversion**. If a client requests a different `SetPixelFormat`,
  convert in the server (rare; most clients accept the server's).
- **Frame dims:** read straight from the driver — `frame_width(model)` × 192
  for `two` (280 or 560), `TTY_PIXEL_WIDTH × TTY_PIXEL_HEIGHT` for `one`.
- **Update cadence:** the RFB client drives with `FramebufferUpdateRequest`
  (usually incremental). Serve a frame when the machine is `screen_dirty` or
  the text-flash phase flips — reusing the existing `screen_dirty()` +
  `phase` signals. Coalesce to the machine's fps.
- **Encoding:** **Raw** to start (tiny frames, trivially correct). Add
  `CopyRect` and then a compressed encoding (`Tight`/`ZRLE`, both noVNC
  supports) only if we ever want to cut bandwidth — not needed for this frame
  size, so it stays a later optimisation, not a v1 requirement.
- **Scaling:** the client scales; we send native pixels (RFB has no server
  scaling). noVNC and native clients both upscale the low-res frame.

---

## 9. Audio (deferred, path documented)

RFB has no audio. The speaker beep is dropped over VNC in v1 — acceptable, and
called out in docs. Two later paths, neither blocking v1:

1. **WebAudio side-channel.** `snd::Wave` already renders the speaker to i16
   PCM **SDL-free**. Stream those samples over a second WebSocket and play
   them via the Web Audio API in the noVNC page. Self-contained, browser-only.
2. **RDP `rdpsnd`.** If Track B (RDP) is built, it carries audio natively.

---

## 10. Security

- **Default bind is `127.0.0.1`.** Exposing a machine is an explicit
  `bind: 0.0.0.0` (or `--serve vnc://0.0.0.0:…`).
- **VNC auth:** the RFB DES challenge (`password`) is weak on its own; the
  intended posture is **VNC "None" behind a boundary** — an SSH tunnel, or a
  TLS-terminating reverse proxy / Guacamole that does real auth. This mirrors
  Proxmox, which fronts noVNC with a ticketed, authenticated web tier rather
  than trusting VNC auth.
- **WebSocket/TLS:** terminate `wss://` at nginx/Caddy (or Guacamole); EWM
  speaks plaintext RFB/WS on localhost behind it. A ticket/token scheme like
  Proxmox's `vncproxy` is a possible later addition for the built-in web tier.
- Document all of this loudly; the failure mode (an open VNC port on
  `0.0.0.0` with no auth) should never be the default.

---

## 11. Alternatives considered and rejected (as the primary path)

- **Xvfb + x11vnc + the real SDL binary.** Blocked by
  `sdl::check_renderer()`'s accelerated-renderer requirement (software
  renderer under Xvfb → EWM exits); one full X server per VM; still needs
  input plumbing; heavier and more fragile than reusing the pure renderer.
  Could be *made* to work with a GL virtual display (Mesa `llvmpipe` /
  VirtualGL), but that is more moving parts than the embedded server, for no
  benefit. Rejected.
- **`xrdp` + an X session.** Same X-server-per-VM weight, plus RDP's security
  stack, to reach a Windows client we do not need. Rejected as primary (see
  Track B for native RDP done properly).
- **Guacamole as a *substitute* for a protocol.** Guacamole is a gateway, not
  a display server — EWM still has to speak VNC/RDP underneath it. So it is a
  recommended **web tier** (Section 5C), not an alternative to the embedded
  server.
- **A Tokio-based VNC crate (`rustvncserver`).** Capable and current, but
  pulls the async runtime into a project that is deliberately
  blocking-threads + tiny-deps. Kept as a reference/fallback, not the default.

---

## 12. Status

Each row is one PR. `two` (][+ / //e, bitmap) is the lead target; `one`
(Apple 1 / Replica 1, TTY) reuses the same frontend since its display is also
a pure `Vec<u32>`.

| Phase | Description | Size | Status |
|---|---|---|---|
| 0 | This plan; `remote` config + `--serve` parsing (validates, errors "not built") | S | Not started |
| 1 | Extract a frontend-agnostic **machine driver** from `two::main`; SDL path reuses it, behaviour unchanged | M | Not started |
| 2 | `rfb.rs`: minimal RFB server (handshake, ServerInit, Raw update, KeyEvent/Pointer) on a background thread; `--serve vnc://…` serves `two` to a **native** VNC client | L | Not started |
| 3 | Input completeness: control keys, Ctrl/Alt, arrows, reset, paddle buttons; `//e` vs `][+` keyboard parity with the SDL path | M | Not started |
| 4 | Embedded **WebSocket** transport (`tungstenite`) → noVNC connects directly, no websockify | M | Not started |
| 5 | Vendored **noVNC web page** + tiny built-in HTTP handler; per-machine console + a "hub" index (Proxmox-console feel) | M | Not started |
| 6 | **Multi-VM orchestration**: `ewm-vnc@.service` systemd template / `scripts/ewm-farm.sh`; config→port; docs | M | Not started |
| 7 | `one` (Apple 1 / Replica 1) served through the same frontend | S | Not started |
| B1 | *(optional Track B)* `ironrdp-server` RDP frontend | L | Optional |
| B2 | *(optional)* Audio side-channel (WebAudio) and/or Guacamole deployment recipe | M | Optional |

---

## 13. Phases in detail

### Phase 0 — Plan + config/CLI surface (no server yet)
This document, plus the `remote` config struct in `config.rs` (with schema +
`apply_set` support and validation, following the `Display`/`fps` pattern) and
`--serve vnc://…` parsing. With no server compiled in yet, `--serve` prints a
clear "remote console not built (Phase 2)" and exits. **Gate:** config
round-trips through the schema test; SDL path untouched; fmt/clippy/test green.

### Phase 1 — Frontend-agnostic machine driver
Lift the machine-stepping + rendering out of `two::main` into a `Driver` that
owns the `Two` + `Scr`, steps `speed/fps` cycles per `tick()`, and exposes:
`frame() -> &[u32]`, `dims() -> (w, h)`, and input methods (`key`, `set_button`,
`set_joystick`, `reset`, `load_disk`). Re-express today's SDL loop as a thin
`SdlFrontend` over the driver. **This is a pure refactor with no VNC** — its
whole job is to prove the SDL frontend is unchanged. **Gate:** the golden-BMP
tests (`boot_screen_matches_golden_bmp`, the //e goldens) still pass;
`--screenshot` still works; fmt/clippy/test green.

### Phase 2 — Minimal RFB server, native client
New `rfb.rs`: RFB 3.8 (and 3.3) handshake, security type None (auth is
Phase 10-ish / Section 10), `ServerInit` advertising our pixel format and
`frame dims`, `FramebufferUpdate` with **Raw** encoding on
`FramebufferUpdateRequest`, and decode of `KeyEvent` + `PointerEvent`. Runs on
a background listener thread (thread-per-connection, `mpsc` to the driver,
drained per frame — the `wozbug::Server` shape). Wire `--serve vnc://…` to run
the driver headless and pump the server each tick. Port the ASCII/printable
subset of the keymap (Section 7). **Gate:** connect with TigerVNC/RealVNC,
watch it boot a disk, type into AppleSoft BASIC. Add a headless unit test that
drives the RFB handshake over a loopback socket and asserts the first
`FramebufferUpdate` decodes to the expected dims (mirrors `wozbug`'s
`server_round_trip`).

### Phase 3 — Input completeness
Full keysym table: control keys, Ctrl-letter → 1–26, arrows, ESC, Tab/Delete
quirks, a reset combo, paddle buttons, and the `//e`/`][+` keyboard branch.
Track modifier state from key up/down. **Gate:** a keysym→byte unit test
table; manual parity check against the SDL keyboard.

### Phase 4 — Embedded WebSocket transport
Detect a WebSocket `Upgrade` handshake (sync `tungstenite`) on the configured
`websocket` port and frame RFB inside WS messages, so **noVNC connects
directly** — no websockify. Keep plain-TCP RFB working in parallel. **Gate:**
noVNC (pointed straight at `ws://host:5701`) boots and drives a machine in a
browser.

### Phase 5 — Vendored web console (the Proxmox-console piece)
Bundle a pinned, self-contained noVNC build and serve it from a small built-in
HTTP handler on the WS port: `GET /` returns the console page wired to this
machine's WebSocket. Add a minimal **hub** index (static or a tiny endpoint)
that lists running machines and links to each console — the Proxmox node-view
feel. **Gate:** `http://host:5701/` shows a live, interactive Apple II with no
external tooling.

### Phase 6 — Multi-VM orchestration
A `systemd` template unit `ewm-vnc@.service` (instance name = config file →
deterministic port pair) and/or `scripts/ewm-farm.sh` that launches a set of
configs on a port range. Document the reverse-proxy + Guacamole options
(Section 5/10). **Gate:** boot `configs/plus.json` and `configs/enhanced.json`
(and a third) on distinct ports; reach each from a browser tab.

### Phase 7 — Apple 1 / Replica 1 over the same frontend
Give `one` a `Driver` the same way (its `Tty.pixels` is already a `Vec<u32>`),
so the Woz Monitor / KRUSADER machines are reachable remotely too. **Gate:**
`ewm one --model replica1 --serve vnc://…` reachable from noVNC.

### Track B (optional) — native RDP, audio, gateways
- **B1:** an `ironrdp-server` frontend (Tokio) reusing the same `Driver`
  boundary; browser client via `ironrdp-web`. Kept isolated behind a Cargo
  feature so the default build stays Tokio-free.
- **B2:** the WebAudio speaker side-channel (Section 9) and a documented
  Guacamole deployment for authenticated fleet access.

---

## 14. Dependencies (kept deliberately small)

- **v1 core:** nothing beyond `std::net` + threads (the `wozbug` model) for
  the RFB server itself.
- **Phase 4:** a sync WebSocket crate (`tungstenite`) — small, no async
  runtime.
- **Phase 5:** noVNC is a vendored static asset, not a Cargo dependency.
- **Track B only:** `ironrdp-server` + Tokio, behind a Cargo feature.
- **References/fallbacks (not linked by default):** Oxide `rfb`,
  `rustvncserver`.

Everything in v1 stays consistent with the current lean dep list
(`sdl3`, `serde`, `serde_json`, `fontdue`, `chrono`) and the blocking-I/O
style already used by WozBug.

---

## 15. Open questions

- **Reset gesture over RFB.** No standard "send Ctrl-Reset" — pick a combo
  (Ctrl+F12?) or expose it via the web page UI / a Guacamole menu.
- **Auth for the built-in web tier.** Ship "None + bind localhost + document a
  proxy," or build a Proxmox-style ticket now? Leaning: document the proxy for
  v1, revisit a token scheme in Phase 6.
- **Overlays remote-side.** Do we composite the status bar / drive LEDs /
  command palette into the remote framebuffer (they are pure `Vec<u32>`
  renderers, so it is cheap), or keep the remote view clean? Probably a config
  toggle, default clean.
- **Per-connection vs shared view.** v1 = one machine, many viewers see the
  same screen (RFB shared-flag). Fine for a personal fleet.

---

## 16. References

VNC / RFB servers in Rust:
- [Oxide `rfb` — trait-based server-side RFB](https://github.com/oxidecomputer/rfb)
- [`rustvncserver` (RFC 6143, 11 encodings, Tokio)](https://crates.io/crates/rustvncserver)
- [whitequark `rust-vnc`](https://github.com/whitequark/rust-vnc)

RDP in Rust:
- [Devolutions IronRDP (incl. `ironrdp-server`)](https://github.com/Devolutions/IronRDP)
- [`ironrdp-web` browser/WASM client](https://github.com/Devolutions/IronRDP/tree/master/web-client)

Web consoles:
- [noVNC — HTML5 VNC client](https://novnc.com/noVNC/)
- [websockify — WebSocket↔TCP bridge](https://github.com/novnc/websockify)
- [Apache Guacamole architecture (`guacd`)](https://guacamole.apache.org/doc/gug/guacamole-architecture.html)
- Proxmox `vncproxy` / `vncwebsocket` ticketed console (the reference UX).

Protocol:
- [RFC 6143 — The Remote Framebuffer Protocol](https://datatracker.ietf.org/doc/html/rfc6143)

In-tree precedents this design leans on:
- `ewm/src/wozbug.rs` — the embedded `TcpListener` + `mpsc` + frame-loop-drain
  server pattern the RFB server copies.
- `ewm/src/scr.rs` / `ewm/src/tty.rs` — the pure `Vec<u32>` renderers the
  server reads (already headless via `--screenshot` and the golden tests).
- `ewm/src/config.rs` — the `Display`/`Machine` config + schema + `apply_set`
  the `remote` block extends.
