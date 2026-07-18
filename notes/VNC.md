# The Browser Console (noVNC) — As Built, and the WebAudio Plan

A working document for the browser side of the remote console: how the
embedded noVNC setup works today (REMOTE.md Phases 4–5, both landed on
`remote-console`), and the plan for **speaker audio in the browser** via a
WebAudio side-channel (REMOTE.md §9 / Track B2, planned here).

Companion to `notes/REMOTE.md`, which owns the overall remote-console plan
and status; this file goes deep on the web tier only.

---

## 1. The console as built

One process, one machine, up to three listeners — every tier **opt-in on the
command line**, with `bind` (default `127.0.0.1`) and every port explicit:

| Invocation | Listeners |
|---|---|
| `--serve vnc://BIND:PORT` | plain-TCP RFB for native VNC clients |
| `…?ws=PORT` | + raw RFB-over-WebSocket (bring-your-own noVNC; plain HTTP → `426`) |
| `…?web=PORT` | + the embedded console page on that same port |

Config equivalents: `remote.bind` / `remote.port` / `remote.websocket` /
`remote.web` / `remote.password` / `remote.view_only`.

**One port, two jobs.** The WebSocket port routes per request (`ws.rs`
parses; `rfb.rs` dispatches): a WebSocket `Upgrade` becomes the RFB byte
stream framed in binary WS messages — the same hand-rolled RFB state machine
as the TCP port, shared verbatim through `io::Read`/`io::Write` adapters — and
a plain `GET` serves the console assets (when `web` is on). This is the
Proxmox/QEMU shape: the page connects back to exactly the host and port that
served it, so the page needs no configuration.

**What's embedded** (`ewm/novnc/`, baked in by `ewm/build.rs`, served by
`web.rs`):

- The **noVNC engine**: `core/` (42 ES modules, `rfb.js` at the root) plus
  its one dependency `vendor/pako`, vendored unmodified from the pinned
  release **v1.6.0** (MPL-2.0; license vendored alongside).
- **EWM's own console page** — `index.html`, ~70 lines. *Not* noVNC's
  `vnc.html`/`app/` (that ~800 KB of settings-panel chrome is deliberately
  not vendored).

**Input**: keys translate through the SDL keymap table and are **paced
through a queue** — the Apple II keyboard is a one-byte latch and a browser
delivers a whole typed word within one frame, so each byte feeds only after
the ROM consumed the previous one (strobe via `$C010`). Fast typing and
(future) paste are safe; type-ahead during boot works.

**Auth**: the VNC password (DES challenge) works in the page — noVNC raises
`credentialsrequired`, our page prompts. TLS is out of scope; terminate
`wss://`/`https://` at a reverse proxy (REMOTE.md §10).

**Known silence**: RFB has no audio channel, so the speaker is dropped for
*every* VNC client, browser included. Fixing that for the browser is the rest
of this document. (Native VNC clients stay silent — that is a protocol
limitation; the native-audio path would be RDP's `rdpsnd`, REMOTE.md
Track B1.)

---

## 2. The embedding model — why no fork is needed

noVNC is two things in one repo, and the distinction is the whole answer:

1. **An application** — `vnc.html` + `app/`: the full client UI with the
   side drawer, settings, clipboard panel. This is what people "deploy".
2. **A library** — `core/rfb.js`: an ES-module `RFB` class you instantiate
   against any DOM element and any WebSocket URL. This is a supported,
   documented API surface (`docs/API.md` upstream), intended precisely for
   embedding in a page the host application owns.

**EWM already uses noVNC only as a library.** Our `index.html` *is* "a page
that we own": it creates `new RFB(screenDiv, url)` and wires a handful of
events (`connect`, `desktopname`, `credentialsrequired`, `disconnect`).
Buttons, status UI, and audio all belong to *our* page, next to — not inside —
the RFB canvas. Nothing about that requires touching a single vendored file,
now or later:

- **Buttons** (built): Reset, Reboot, Pause sit in the page's top bar and
  `POST /control/<action>` on the same port — a tiny EWM-side HTTP endpoint
  (`rfb.rs` routes `/control/*` to an `InputEvent::Control` on the input
  channel the serve loop already drains), no noVNC change. Reset is
  Ctrl-Reset, Reboot is a cold boot (`power_on_machine`), Pause freezes CPU
  stepping (screen keeps publishing, FLASH blink frozen). View-only servers
  drop control like all input. Disk swap and more slot in the same way.
- **Audio**: a second WebSocket + Web Audio in our page. noVNC never sees it.

A fork would only become necessary if we wanted to change the *RFB wire
behaviour itself* (e.g. teach noVNC the QEMU in-band audio extension). We
don't — see §5. **Verdict: the hoped-for direction ("embed noVNC in our page,
WebAudio beside it") is not just viable, it is the architecture already in
place.** The only cost of ownership is keeping our small page working across
noVNC version bumps, which the pin (`ewm/novnc/README.md`) already manages.

---

## 3. The audio source we already have

`snd.rs` split the sound path in two long before this plan:

- **`Wave`** — the pure synthesis half: cycle-stamped `$C030` toggles in,
  **44.1 kHz mono i16 PCM** out (`render(&toggles, cpu_counter) -> &[i16]`),
  with the AC-coupled decay model (level relaxes toward zero ≈ 4 ms time
  constant, so silence is genuinely silent and underruns during silence are
  inaudible — a property that will matter again at the network boundary).
  SDL-free and unit-tested.
- **`Snd`** — the SDL half that queues `Wave`'s samples to a device.

The headless serve loop already calls `two.drain_speaker_toggles()` every
frame — and throws the toggles away. The entire server-side job is: stop
throwing them away.

`Wave` also scales with the emulated CPU speed (`cpu_frequency`): an
accelerated machine pitches up rather than overrunning, exactly like real
accelerator cards. That behaviour transfers to the network stream for free.

---

## 4. The WebAudio plan

```
  serve loop (per frame)                         browser (our index.html)
  ─────────────────────                          ────────────────────────
  toggles = two.drain_speaker_toggles()
  pcm = wave.render(&toggles, counter)           ┌──────────────────────┐
  audio.publish(pcm) ──► per-client queues ──►   │ ws://host:PORT/audio │
                         (bounded, drop-oldest)  └──────────┬───────────┘
                                                            ▼
  same port as the console page and the RFB WS;   Int16 → Float32 → ring
  routed by request path, exactly like GET vs     buffer → AudioWorklet →
  Upgrade today                                   speakers (44.1 kHz ctx)
```

### 4.1 Transport: a path on the port we already have

The WebSocket listener gains one route: an upgrade whose request path is
**`/audio`** streams PCM instead of RFB. (`ws::Request` already carries the
path; today it is ignored.) No new port, no new config block, no change for
native clients. Anything that can open a WebSocket can consume it — the
console page will, but so could a recorder script.

### 4.2 Wire format: raw and boring

- First message (text): `{"format":"s16le","rate":44100,"channels":1}` —
  self-describing so the rate can change later without breaking clients.
- Then binary messages: one frame-loop tick's worth of raw little-endian
  mono i16 samples (~1102 samples ≈ 2.2 KB at 40 fps; ≈ 88 KB/s per
  listener). No compression — the bandwidth is trivial and the dependency
  budget stays at zero. Messages are self-contained sample runs; a dropped
  message is a momentary silence, not corruption.

### 4.3 Server side (`rfb.rs` + a small `AudioPublisher`)

- `snd::Wave` goes `pub` (struct, `render`, `set_cpu_frequency`) — the SDL
  half is untouched.
- An `AudioPublisher` mirrors the framebuffer `Publisher` pattern: the serve
  loop publishes each frame's PCM; each `/audio` connection has a **bounded**
  queue (say 8 chunks ≈ 200 ms) drained by its writer thread. A slow or
  backgrounded tab gets **drop-oldest**, not unbounded memory; the decay
  model means the gap sounds like a click at worst, silence at best.
- Pure-silence chunks are still sent for v1 (constant cadence keeps the
  client's buffer clock honest and the code simpler); an idle machine with
  no audio clients costs one `is_empty()` check per frame.
- View-only affects input, not output: audio streams to view-only clients.

### 4.4 Client side (our page, ~60 lines + a worklet)

- `ewm/novnc/audio.js` (ours, like `index.html`): open the `/audio` socket,
  parse the header, convert i16 → Float32, push into a ring buffer feeding an
  **`AudioWorklet`** (the supported low-latency path; served as its own tiny
  module file, which `web.rs` serves like any other asset).
  `new AudioContext({sampleRate: 44100})` lets the browser own any device
  resampling.
- **Autoplay policy**: browsers refuse audio before a user gesture. The
  console already demands a click to focus the canvas — that same first
  gesture resumes the `AudioContext`. A small 🔊/🔇 toggle in the status bar
  makes it explicit; it is also, deliberately, the **first of the page
  buttons** this architecture was chosen for.
- **Latency**: target a 50–150 ms client buffer. Beeps need no lip-sync;
  underrun plays silence (which, thanks to the decay model, is what the
  signal was converging to anyway) and refills.

### 4.5 What this does *not* do

- No audio for native VNC clients (protocol has none; that's Track B1/RDP).
- No microphone/upstream audio, no Opus, no MP3 — raw PCM only.
- No noVNC changes of any kind.

---

## 5. Alternatives considered and rejected

- **QEMU's RFB audio extension** (in-band, non-standard): would require
  teaching the vendored noVNC the extension — i.e. an actual fork, the thing
  we are avoiding — and no other client speaks it either. Rejected.
- **Forking noVNC to add an audio panel**: solves nothing the side-channel
  doesn't, and buys a permanent merge burden. Rejected.
- **WebRTC**: real jitter buffers and echo cancellation we don't need, at the
  cost of an enormous protocol surface (ICE/DTLS/SRTP) that is unbuildable
  from scratch in this codebase's style. Rejected for a beep.
- **RDP `rdpsnd`**: the native-client audio answer, unchanged as optional
  Track B1 in REMOTE.md.

---

## 6. Phases

| Phase | Description | Gate | Status |
|---|---|---|---|
| A1 | `Wave` public; audio `Hub`; `/audio` route on the WS port; header + PCM framing | A scripted WS client receives the header and non-zero PCM while the machine beeps (`PRINT CHR$(7)`); unit tests for framing and drop-oldest backpressure | **Prototype** ✅ (gate run: peak 8000 — the full speaker rail — during the bell) |
| A2 | `audio.js` + AudioWorklet in the console page; gesture arming; 🔊/🔇 toggle | **Hear the beep in a browser tab** served entirely by the EWM binary | **Prototype** ✅ (in-page stats confirmed the bell's PCM reached the worklet; audible check is the human half) |
| A3 | Polish: reconnect with the page, level indicator, `?audio=0` opt-out if wanted | Manual | Not started |

Each phase is one PR into `remote-console`, gates green
(`cargo fmt --check`, `clippy -D warnings`, `cargo test`), SDL path untouched.

---

## 7. Open questions

- **Chunk cadence vs. fps**: one message per frame tick couples audio cadence
  to `display.fps`. Fine at 30–60 fps; if a config ever runs single-digit
  fps, chunk-split inside the publisher instead. Decide in A1.
- **Multiple machines, one page**: when the Phase 6 hub exists, does the hub
  page play audio for every machine at once, or only the focused console?
  (Leaning: only the focused one; browsers dislike N contexts.)
- **Recording**: the `/audio` stream is trivially recordable (`websocat >
  file`); worth documenting as a feature once A1 lands.
