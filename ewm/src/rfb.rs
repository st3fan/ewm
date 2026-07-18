//! A minimal RFB (VNC) server, hand-rolled on `std::net` in the spirit of
//! `wozbug::Server`: a background listener thread, one handler thread per
//! connection, and `mpsc`/shared-buffer channels to the emulator loop. No
//! async runtime, no external crates (notes/REMOTE.md, Phase 2).
//!
//! The server speaks RFB 3.8 (falling back to 3.7 / 3.3), security type
//! `None`, and the **Raw** encoding — no compression. Our frames are tiny
//! (280×192 or 560×192 × 4 bytes) so bandwidth is a non-issue and Raw is
//! trivially correct. Pixels ship big-endian RGBA: the `ServerInit`
//! `PIXEL_FORMAT` advertises 32 bpp, depth 24, big-endian, with red/green/
//! blue shifts 24/16/8, so a `Scr` built with `PixelLayout::Rgba8888` maps
//! straight to the wire with `u32::to_be_bytes` and no per-pixel conversion.
//!
//! Concurrency mirrors WozBug: the emulator thread owns the machine and never
//! shares it. Each frame it calls [`Publisher::publish`] with the rendered
//! `&[u32]`; each connection's writer wakes and sends a full-frame update in
//! response to the client's `FramebufferUpdateRequest`. Client keyboard and
//! pointer events arrive back on an `mpsc` channel the emulator drains between
//! frames — exactly the `wozbug::Server::commands` shape.
//!
//! Two transports, one protocol (Phase 4): the plain-TCP port serves native
//! VNC clients, and an optional second port serves the same RFB byte stream
//! inside WebSocket frames so a browser's noVNC connects directly (`ws.rs`
//! supplies `Read`/`Write` adapters; the state machine here is shared
//! verbatim). Both feed one machine: same framebuffer, same input channel.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// An input event decoded from a client, handed to the emulator loop. The
/// keysym→byte translation and modifier tracking live on the emulator side
/// (it owns the machine and its model), exactly as the SDL loop owns its
/// keyboard state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// An RFB `KeyEvent`: an X11 keysym and whether it went down or up.
    Key { down: bool, keysym: u32 },
    /// An RFB `PointerEvent`: the button mask and framebuffer coordinates.
    Pointer { mask: u8, x: u16, y: u16 },
}

/// The shared framebuffer the emulator publishes into and every connection's
/// writer reads from. `bytes` is already in wire format (big-endian RGBA);
/// `generation` bumps on every publish so an incremental request only sends
/// when something actually changed.
struct Frame {
    bytes: Vec<u8>,
    generation: u64,
}

/// The immutable-after-start channel between the emulator and its clients:
/// the current frame behind a `Mutex`, a `Condvar` to wake writers, and the
/// fixed geometry advertised in `ServerInit`.
struct Channel {
    frame: Mutex<Frame>,
    dirty: Condvar,
    width: u16,
    height: u16,
    name: String,
    /// VNC-auth password. `Some` offers security type VNC (DES challenge);
    /// `None` offers `None` — see notes/REMOTE.md §10.
    password: Option<String>,
}

impl Channel {
    fn new(width: u16, height: u16, name: String, password: Option<String>) -> Channel {
        Channel {
            frame: Mutex::new(Frame {
                bytes: vec![0; width as usize * height as usize * 4],
                generation: 0,
            }),
            dirty: Condvar::new(),
            width,
            height,
            name,
            password,
        }
    }
}

/// The emulator's handle for pushing frames. Cheap to hold; `publish` is the
/// only per-frame cost (one buffer fill under a short-held lock).
pub struct Publisher {
    channel: Arc<Channel>,
}

impl Publisher {
    /// Replace the current frame with `pixels` (row-major, `width * height`
    /// long, packed as `PixelLayout::Rgba8888`) and wake every waiting client.
    /// A pixel count that does not match the advertised geometry is ignored
    /// rather than panicking mid-run.
    pub fn publish(&self, pixels: &[u32]) {
        let expected = self.channel.width as usize * self.channel.height as usize;
        if pixels.len() != expected {
            return;
        }
        let mut frame = self.channel.frame.lock().expect("frame mutex");
        let bytes = &mut frame.bytes;
        bytes.clear();
        for &p in pixels {
            bytes.extend_from_slice(&p.to_be_bytes());
        }
        frame.generation = frame.generation.wrapping_add(1);
        drop(frame);
        self.channel.dirty.notify_all();
    }
}

/// How and where [`Server::start`] listens — the RFB half of the machine's
/// `remote` config.
pub struct Options {
    /// Address to bind (both listeners). Default posture is localhost.
    pub bind: String,
    /// Plain-TCP RFB port for native VNC clients (0 = ephemeral, tests).
    pub port: u16,
    /// Optional RFB-over-WebSocket port for browser clients (noVNC connects
    /// straight here, no websockify — Phase 4).
    pub websocket: Option<u16>,
    /// Serve the embedded noVNC console (`web.rs`) for plain HTTP requests on
    /// the WebSocket port (Phase 5). Off: plain HTTP gets `426`.
    pub web: bool,
    /// The desktop name sent in `ServerInit`.
    pub name: String,
    /// Drop all client input at the source.
    pub view_only: bool,
    /// When set, require VNC authentication (the DES challenge) instead of
    /// offering the `None` security type.
    pub password: Option<String>,
}

/// A running RFB server: the listeners and their clients live on background
/// threads; the emulator keeps a [`Publisher`] to push frames and this
/// `Server` to drain input between frames.
pub struct Server {
    input: Receiver<InputEvent>,
    port: u16,
    websocket_port: Option<u16>,
}

impl Server {
    /// Bind the plain-TCP port (and the WebSocket port, when configured) and
    /// start accepting clients. Returns the `Server` (drain input from it)
    /// and a [`Publisher`] (push frames into it). Both transports feed the
    /// same machine: the same framebuffer out, the same input channel back.
    pub fn start(options: Options, width: u16, height: u16) -> io::Result<(Server, Publisher)> {
        let listener = TcpListener::bind((options.bind.as_str(), options.port))?;
        let port = listener.local_addr()?.port();
        let ws_listener = match options.websocket {
            Some(ws_port) => Some(
                TcpListener::bind((options.bind.as_str(), ws_port))
                    .map_err(|e| io::Error::new(e.kind(), format!("websocket port: {e}")))?,
            ),
            None => None,
        };
        let websocket_port = match &ws_listener {
            Some(listener) => Some(listener.local_addr()?.port()),
            None => None,
        };

        let channel = Arc::new(Channel::new(
            width,
            height,
            options.name.clone(),
            options.password,
        ));
        let (input_tx, input) = std::sync::mpsc::channel();
        let view_only = options.view_only;
        let web = options.web;

        let tcp_channel = channel.clone();
        let tcp_input = input_tx.clone();
        std::thread::spawn(move || {
            accept(listener, tcp_channel, tcp_input, view_only, Transport::Tcp)
        });
        if let Some(ws_listener) = ws_listener {
            let ws_channel = channel.clone();
            std::thread::spawn(move || {
                accept(
                    ws_listener,
                    ws_channel,
                    input_tx,
                    view_only,
                    Transport::WebSocket { web },
                )
            });
        }

        Ok((
            Server {
                input,
                port,
                websocket_port,
            },
            Publisher { channel },
        ))
    }

    /// The bound plain-TCP port (useful when 0 was passed).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The bound WebSocket port, when one was configured.
    pub fn websocket_port(&self) -> Option<u16> {
        self.websocket_port
    }

    /// Pop one pending input event, or `None`. The emulator loop drains this
    /// each frame — an idle server costs one `try_recv`.
    pub fn try_recv_input(&self) -> Option<InputEvent> {
        self.input.try_recv().ok()
    }
}

/// What a listener speaks: raw RFB, or RFB inside WebSocket frames — the
/// latter optionally doubling as the web-console HTTP endpoint (Phase 5).
#[derive(Clone, Copy)]
enum Transport {
    Tcp,
    WebSocket { web: bool },
}

/// Accept connections forever, spawning a handler thread per client so several
/// viewers can watch the same machine at once (RFB shared-desktop).
fn accept(
    listener: TcpListener,
    channel: Arc<Channel>,
    input_tx: Sender<InputEvent>,
    view_only: bool,
    transport: Transport,
) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let channel = channel.clone();
        let input_tx = input_tx.clone();
        std::thread::spawn(move || {
            let result = match transport {
                Transport::Tcp => handle_tcp(stream, channel, input_tx, view_only),
                Transport::WebSocket { web } => {
                    handle_ws(stream, channel, input_tx, view_only, web)
                }
            };
            if let Err(e) = result {
                // A dropped client is normal; log at debug volume only.
                if e.kind() != io::ErrorKind::UnexpectedEof {
                    eprintln!("[RFB] connection closed: {e}");
                }
            }
        });
    }
}

/// One native client: the RFB byte stream straight over TCP.
fn handle_tcp(
    stream: TcpStream,
    channel: Arc<Channel>,
    input_tx: Sender<InputEvent>,
    view_only: bool,
) -> io::Result<()> {
    let reader = stream.try_clone()?;
    let writer = stream.try_clone()?;
    run(reader, writer, stream, channel, input_tx, view_only)
}

/// One browser connection. A WebSocket upgrade becomes the identical RFB
/// byte stream framed in WebSocket binary messages (`ws::WsReader`/`WsWriter`
/// implement `Read`/`Write`, so the state machine below is shared verbatim).
/// A plain HTTP request is the web console (Phase 5) when enabled — the same
/// port serves the page and its RFB stream, the Proxmox shape — or `426`.
fn handle_ws(
    mut stream: TcpStream,
    channel: Arc<Channel>,
    input_tx: Sender<InputEvent>,
    view_only: bool,
    web: bool,
) -> io::Result<()> {
    let request = crate::ws::read_http_request(&mut stream)?;
    if !request.is_upgrade() {
        return if web {
            crate::web::respond(&mut stream, &request.path)
        } else {
            crate::ws::refuse_plain_http(&mut stream)?;
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "plain HTTP on the WebSocket port (no web console enabled)",
            ))
        };
    }
    crate::ws::accept_upgrade(&mut stream, &request)?;
    let (reader, writer) = crate::ws::split(&stream)?;
    run(reader, writer, stream, channel, input_tx, view_only)
}

/// One client, transport-agnostic: RFB handshake, then a reader thread
/// (client messages → input channel + update requests) while this thread
/// writes frames. `raw` is the underlying socket, kept for the shutdown that
/// unblocks the reader when the writer stops.
fn run<R, W>(
    mut reader: R,
    mut writer: W,
    raw: TcpStream,
    channel: Arc<Channel>,
    input_tx: Sender<InputEvent>,
    view_only: bool,
) -> io::Result<()>
where
    R: Read + Send + 'static,
    W: Write,
{
    handshake(&mut reader, &mut writer, &channel)?;

    let pending = Arc::new(Mutex::new(Pending::default()));
    let alive = Arc::new(AtomicBool::new(true));

    let read_pending = pending.clone();
    let read_alive = alive.clone();
    let read_channel = channel.clone();
    let reader_thread = std::thread::spawn(move || {
        read_loop(
            reader,
            read_pending,
            read_alive,
            read_channel,
            input_tx,
            view_only,
        );
    });

    let result = write_loop(&mut writer, &channel, &pending, &alive);

    // Whatever ended the writer, tear the connection down so the reader's
    // blocked `read` returns and the thread joins.
    alive.store(false, Ordering::Relaxed);
    let _ = raw.shutdown(std::net::Shutdown::Both);
    let _ = reader_thread.join();
    result
}

/// RFB security type: no authentication.
const SEC_NONE: u8 = 1;
/// RFB security type: VNC authentication (the DES challenge, RFC 6143 §7.2.2).
const SEC_VNC: u8 = 2;

/// The RFB 3.x handshake through `ServerInit`: ProtocolVersion, security
/// negotiation (`None`, or VNC auth when a password is set), `ClientInit`,
/// then our geometry and pixel format.
fn handshake<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    channel: &Channel,
) -> io::Result<()> {
    // ProtocolVersion: offer 3.8; read the client's choice.
    writer.write_all(b"RFB 003.008\n")?;
    let mut version = [0u8; 12];
    reader.read_exact(&mut version)?;
    let minor = parse_minor(&version);

    negotiate_security(reader, writer, minor, channel)?;

    // ClientInit: one shared-flag byte we accept and ignore (all viewers
    // share the one machine).
    let mut shared = [0u8; 1];
    reader.read_exact(&mut shared)?;

    // ServerInit: width, height, PIXEL_FORMAT, name.
    let mut init = Vec::with_capacity(24 + channel.name.len());
    init.extend_from_slice(&channel.width.to_be_bytes());
    init.extend_from_slice(&channel.height.to_be_bytes());
    init.extend_from_slice(&PIXEL_FORMAT);
    init.extend_from_slice(&(channel.name.len() as u32).to_be_bytes());
    init.extend_from_slice(channel.name.as_bytes());
    writer.write_all(&init)?;
    Ok(())
}

/// Negotiate the security type and authenticate. Offers exactly one type —
/// VNC auth when a password is configured, otherwise `None` — via the 3.7+
/// list or the 3.3 dictated word, then runs the DES challenge (VNC) and sends
/// `SecurityResult`. Returns `Err` if the client picks an unoffered type or
/// authentication fails, which closes the connection.
fn negotiate_security<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    minor: u32,
    channel: &Channel,
) -> io::Result<()> {
    let sec_type = if channel.password.is_some() {
        SEC_VNC
    } else {
        SEC_NONE
    };

    // Announce the type: 3.7+ offers a list the client selects from; 3.3 is
    // dictated as a single 32-bit word.
    let selected = if minor >= 7 {
        writer.write_all(&[1u8, sec_type])?;
        let mut selection = [0u8; 1];
        reader.read_exact(&mut selection)?;
        selection[0]
    } else {
        writer.write_all(&(sec_type as u32).to_be_bytes())?;
        sec_type
    };
    if selected != sec_type {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "client chose an unoffered security type",
        ));
    }

    if sec_type == SEC_VNC {
        let password = channel.password.as_deref().unwrap_or_default();
        let ok = vnc_auth(reader, writer, password)?;
        writer.write_all(&(if ok { 0u32 } else { 1u32 }).to_be_bytes())?;
        if !ok {
            // 3.8 carries a reason string with the failure.
            if minor >= 8 {
                let reason = b"Authentication failed";
                writer.write_all(&(reason.len() as u32).to_be_bytes())?;
                writer.write_all(reason)?;
            }
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "VNC authentication failed",
            ));
        }
    } else if minor >= 8 {
        // `None`: `SecurityResult` (OK) is a 3.8-and-later addition; 3.3/3.7
        // proceed straight to `ClientInit`.
        writer.write_all(&0u32.to_be_bytes())?;
    }
    Ok(())
}

/// Run the VNC DES challenge: send 16 random bytes, read the client's DES-
/// encrypted reply, and compare it to our own encryption under the password
/// key. Returns whether they match.
fn vnc_auth<R: Read, W: Write>(reader: &mut R, writer: &mut W, password: &str) -> io::Result<bool> {
    let challenge = random_challenge();
    writer.write_all(&challenge)?;
    let mut response = [0u8; 16];
    reader.read_exact(&mut response)?;

    let key = des_key(password);
    let mut expected = [0u8; 16];
    for (chunk, out) in challenge.chunks_exact(8).zip(expected.chunks_exact_mut(8)) {
        let block = u64::from_be_bytes(chunk.try_into().expect("8-byte chunk"));
        out.copy_from_slice(&crate::des::encrypt_block(key, block).to_be_bytes());
    }
    Ok(constant_time_eq(&response, &expected))
}

/// Derive the DES key from a VNC password: the first 8 bytes (NUL-padded),
/// each with its bit order reversed — the historical VNC quirk.
fn des_key(password: &str) -> u64 {
    let mut key = [0u8; 8];
    for (dst, byte) in key.iter_mut().zip(password.bytes()) {
        *dst = byte.reverse_bits();
    }
    u64::from_be_bytes(key)
}

/// Sixteen random bytes for the auth challenge. `/dev/urandom` when available;
/// a time-seeded fallback otherwise (VNC auth is weak by design regardless —
/// notes/REMOTE.md §10).
fn random_challenge() -> [u8; 16] {
    let mut buf = [0u8; 16];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_ok()
    {
        return buf;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    buf.copy_from_slice(&nanos.to_le_bytes());
    buf
}

/// Compare two 16-byte arrays without an early-out, so a wrong password does
/// not leak a timing signal.
fn constant_time_eq(a: &[u8; 16], b: &[u8; 16]) -> bool {
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Parse `RFB 003.00X\n` → the minor version, defaulting to 3 (the oldest,
/// most conservative handshake) if the banner is malformed.
fn parse_minor(version: &[u8; 12]) -> u32 {
    // Bytes 8..11 are the zero-padded minor number, e.g. "008".
    std::str::from_utf8(&version[8..11])
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(3)
}

/// Our advertised server pixel format: 32 bpp, depth 24, big-endian,
/// true-colour, RGB in the top three bytes (shifts 24/16/8). Matches
/// `PixelLayout::Rgba8888` shipped with `u32::to_be_bytes`.
const PIXEL_FORMAT: [u8; 16] = [
    32, // bits-per-pixel
    24, // depth
    1,  // big-endian-flag
    1,  // true-colour-flag
    0, 255, // red-max   (U16)
    0, 255, // green-max (U16)
    0, 255, // blue-max  (U16)
    24,  // red-shift
    16,  // green-shift
    8,   // blue-shift
    0, 0, 0, // padding
];

/// What the reader has asked the writer to do: whether an update is pending
/// and whether the client wanted it incremental (only on change) or a full
/// immediate refresh.
#[derive(Default, Clone, Copy)]
struct Pending {
    requested: bool,
    incremental: bool,
}

/// Read client messages until the socket closes: decode each fully (so the
/// stream stays framed), forward input, and record framebuffer-update
/// requests for the writer.
fn read_loop<R: Read>(
    mut reader: R,
    pending: Arc<Mutex<Pending>>,
    alive: Arc<AtomicBool>,
    channel: Arc<Channel>,
    input_tx: Sender<InputEvent>,
    view_only: bool,
) {
    while alive.load(Ordering::Relaxed) {
        if handle_message(&mut reader, &pending, &channel, &input_tx, view_only).is_err() {
            break;
        }
    }
    alive.store(false, Ordering::Relaxed);
    channel.dirty.notify_all();
}

/// Read and act on one client message. Every branch consumes exactly the
/// message's bytes so the next read starts on a message boundary.
fn handle_message<R: Read>(
    reader: &mut R,
    pending: &Mutex<Pending>,
    channel: &Channel,
    input_tx: &Sender<InputEvent>,
    view_only: bool,
) -> io::Result<()> {
    let mut kind = [0u8; 1];
    reader.read_exact(&mut kind)?;
    match kind[0] {
        // SetPixelFormat: 3 padding + 16-byte format. We keep our advertised
        // format, so read and discard (clients accept the server's default).
        0 => skip(reader, 19)?,
        // SetEncodings: 1 padding + count + count·S32. We only do Raw (which
        // every client supports unconditionally), so note the count and skip.
        2 => {
            let mut head = [0u8; 3];
            reader.read_exact(&mut head)?;
            let count = u16::from_be_bytes([head[1], head[2]]) as usize;
            skip(reader, count * 4)?;
        }
        // FramebufferUpdateRequest: incremental flag + x/y/w/h (ignored — we
        // always send the whole frame).
        3 => {
            let mut body = [0u8; 9];
            reader.read_exact(&mut body)?;
            let incremental = body[0] != 0;
            let mut p = pending.lock().expect("pending mutex");
            p.requested = true;
            // A non-incremental request forces an immediate full refresh even
            // if nothing changed; keep that stickier flag if already set.
            p.incremental = p.incremental && incremental;
            drop(p);
            channel.dirty.notify_all();
        }
        // KeyEvent: down flag + 2 padding + U32 keysym.
        4 => {
            let mut body = [0u8; 7];
            reader.read_exact(&mut body)?;
            if !view_only {
                let down = body[0] != 0;
                let keysym = u32::from_be_bytes([body[3], body[4], body[5], body[6]]);
                let _ = input_tx.send(InputEvent::Key { down, keysym });
            }
        }
        // PointerEvent: button mask + U16 x + U16 y.
        5 => {
            let mut body = [0u8; 5];
            reader.read_exact(&mut body)?;
            if !view_only {
                let mask = body[0];
                let x = u16::from_be_bytes([body[1], body[2]]);
                let y = u16::from_be_bytes([body[3], body[4]]);
                let _ = input_tx.send(InputEvent::Pointer { mask, x, y });
            }
        }
        // ClientCutText: 3 padding + U32 length + text. Deferred past v1;
        // consume it so the stream stays framed.
        6 => {
            let mut head = [0u8; 7];
            reader.read_exact(&mut head)?;
            let length = u32::from_be_bytes([head[3], head[4], head[5], head[6]]) as usize;
            skip(reader, length)?;
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported RFB client message {other}"),
            ));
        }
    }
    Ok(())
}

/// Discard exactly `n` bytes from the stream.
fn skip<R: Read>(reader: &mut R, n: usize) -> io::Result<()> {
    io::copy(&mut reader.take(n as u64), &mut io::sink())?;
    Ok(())
}

/// Send a full-frame Raw `FramebufferUpdate` whenever the client has a request
/// outstanding: immediately for a non-incremental request, otherwise as soon
/// as the frame generation advances. Blocks on the `dirty` condvar between
/// sends, with a short timeout as a lost-wakeup safety net.
fn write_loop<W: Write>(
    writer: &mut W,
    channel: &Channel,
    pending: &Mutex<Pending>,
    alive: &AtomicBool,
) -> io::Result<()> {
    let mut last_sent = 0u64;
    while alive.load(Ordering::Relaxed) {
        let mut frame = channel.frame.lock().expect("frame mutex");
        loop {
            if !alive.load(Ordering::Relaxed) {
                return Ok(());
            }
            let mut p = pending.lock().expect("pending mutex");
            let ready = p.requested && (!p.incremental || frame.generation != last_sent);
            if ready {
                p.requested = false;
                p.incremental = true;
                break;
            }
            drop(p);
            let (guard, _) = channel
                .dirty
                .wait_timeout(frame, Duration::from_millis(50))
                .expect("frame mutex");
            frame = guard;
        }
        last_sent = frame.generation;
        let update = frame_update(channel.width, channel.height, &frame.bytes);
        drop(frame);
        writer.write_all(&update)?;
    }
    Ok(())
}

/// Build a `FramebufferUpdate` message: one Raw rectangle covering the whole
/// framebuffer.
fn frame_update(width: u16, height: u16, pixels: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(16 + pixels.len());
    msg.push(0); // message-type: FramebufferUpdate
    msg.push(0); // padding
    msg.extend_from_slice(&1u16.to_be_bytes()); // one rectangle
    msg.extend_from_slice(&0u16.to_be_bytes()); // x
    msg.extend_from_slice(&0u16.to_be_bytes()); // y
    msg.extend_from_slice(&width.to_be_bytes());
    msg.extend_from_slice(&height.to_be_bytes());
    msg.extend_from_slice(&0i32.to_be_bytes()); // encoding: Raw
    msg.extend_from_slice(pixels);
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Read, Write};

    /// Start a 280×192 test server on ephemeral ports.
    fn start_server(
        name: &str,
        view_only: bool,
        password: Option<String>,
        websocket: bool,
    ) -> (Server, Publisher) {
        Server::start(
            Options {
                bind: "127.0.0.1".into(),
                port: 0,
                websocket: websocket.then_some(0),
                web: websocket, // tests exercise the console alongside WS
                name: name.into(),
                view_only,
                password,
            },
            280,
            192,
        )
        .expect("bind")
    }

    /// Drive the full handshake over a loopback socket and assert the first
    /// `FramebufferUpdate` decodes to the advertised geometry — the RFB
    /// analogue of `wozbug`'s `server_round_trip`.
    #[test]
    fn handshake_and_first_update() {
        let (server, publisher) = start_server("EWM test", false, None, false);
        let mut client =
            TcpStream::connect(("127.0.0.1", server.port())).expect("connect to server");

        // ProtocolVersion.
        let mut version = [0u8; 12];
        client.read_exact(&mut version).expect("server version");
        assert_eq!(&version, b"RFB 003.008\n");
        client.write_all(b"RFB 003.008\n").expect("client version");

        // Security: one type on offer (None), select it, read SecurityResult.
        let mut sec = [0u8; 2];
        client.read_exact(&mut sec).expect("security list");
        assert_eq!(sec, [1, 1]);
        client.write_all(&[1u8]).expect("select None");
        let mut result = [0u8; 4];
        client.read_exact(&mut result).expect("security result");
        assert_eq!(u32::from_be_bytes(result), 0);

        // ClientInit → ServerInit.
        client.write_all(&[1u8]).expect("shared flag");
        let mut init = [0u8; 24];
        client.read_exact(&mut init).expect("server init");
        let width = u16::from_be_bytes([init[0], init[1]]);
        let height = u16::from_be_bytes([init[2], init[3]]);
        assert_eq!((width, height), (280, 192));
        assert_eq!(init[4], 32, "bits-per-pixel");
        assert_eq!(init[6], 1, "big-endian-flag");
        let name_len = u32::from_be_bytes([init[20], init[21], init[22], init[23]]) as usize;
        let mut name = vec![0u8; name_len];
        client.read_exact(&mut name).expect("name");
        assert_eq!(name, b"EWM test");

        // Publish a distinctive frame, then ask for it.
        let pixels = vec![0x11223344u32; 280 * 192];
        publisher.publish(&pixels);

        let mut reader = BufReader::new(client);
        // A non-incremental FramebufferUpdateRequest for the whole 280×192
        // frame: type 3, incremental 0, x/y 0, w 0x0118 (280), h 0x00C0 (192).
        reader
            .get_mut()
            .write_all(&[3u8, 0, 0, 0, 0, 0, 0x01, 0x18, 0x00, 0xC0])
            .expect("framebuffer update request");

        let mut header = [0u8; 16];
        reader.read_exact(&mut header).expect("update header");
        assert_eq!(header[0], 0, "FramebufferUpdate");
        let rects = u16::from_be_bytes([header[2], header[3]]);
        assert_eq!(rects, 1);
        let rw = u16::from_be_bytes([header[8], header[9]]);
        let rh = u16::from_be_bytes([header[10], header[11]]);
        assert_eq!((rw, rh), (280, 192));
        let encoding = i32::from_be_bytes([header[12], header[13], header[14], header[15]]);
        assert_eq!(encoding, 0, "Raw");

        // The pixel payload is big-endian RGBA of what we published.
        let mut payload = vec![0u8; rw as usize * rh as usize * 4];
        reader.read_exact(&mut payload).expect("pixels");
        assert_eq!(&payload[0..4], &0x11223344u32.to_be_bytes());
    }

    #[test]
    fn view_only_drops_input() {
        // A view-only server must not surface client key/pointer events.
        let (server, _publisher) = start_server("EWM", true, None, false);
        let mut client = TcpStream::connect(("127.0.0.1", server.port())).expect("connect");
        do_handshake(&mut client);
        client
            .write_all(&[4u8, 1, 0, 0, 0, 0, 0, 0x41])
            .expect("key event");
        std::thread::sleep(Duration::from_millis(100));
        assert!(server.try_recv_input().is_none());
    }

    #[test]
    fn key_and_pointer_events_reach_the_emulator() {
        let (server, _publisher) = start_server("EWM", false, None, false);
        let mut client = TcpStream::connect(("127.0.0.1", server.port())).expect("connect");
        do_handshake(&mut client);

        // KeyEvent: down, keysym 0x41 ('A').
        client
            .write_all(&[4u8, 1, 0, 0, 0, 0, 0, 0x41])
            .expect("key event");
        // PointerEvent: mask 1, x=0x0102, y=0x0304.
        client
            .write_all(&[5u8, 1, 0x01, 0x02, 0x03, 0x04])
            .expect("pointer event");

        let key = recv(&server);
        assert_eq!(
            key,
            InputEvent::Key {
                down: true,
                keysym: 0x41
            }
        );
        let pointer = recv(&server);
        assert_eq!(
            pointer,
            InputEvent::Pointer {
                mask: 1,
                x: 0x0102,
                y: 0x0304
            }
        );
    }

    /// Run the client half of the handshake up to and including `ServerInit`.
    fn do_handshake(client: &mut TcpStream) {
        let mut version = [0u8; 12];
        client.read_exact(&mut version).expect("version");
        client.write_all(b"RFB 003.008\n").expect("version");
        let mut sec = [0u8; 2];
        client.read_exact(&mut sec).expect("security");
        client.write_all(&[1u8]).expect("select");
        let mut result = [0u8; 4];
        client.read_exact(&mut result).expect("result");
        client.write_all(&[1u8]).expect("shared");
        let mut init = [0u8; 24];
        client.read_exact(&mut init).expect("init");
        let name_len = u32::from_be_bytes([init[20], init[21], init[22], init[23]]) as usize;
        let mut name = vec![0u8; name_len];
        client.read_exact(&mut name).expect("name");
    }

    /// Block briefly for one input event.
    fn recv(server: &Server) -> InputEvent {
        for _ in 0..50 {
            if let Some(event) = server.try_recv_input() {
                return event;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("no input event arrived");
    }

    /// Run the client half of a VNC-auth handshake up to `SecurityResult`,
    /// answering the DES challenge with `password`. Returns the result word.
    fn vnc_auth_result(port: u16, password: &str) -> (TcpStream, u32) {
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let mut version = [0u8; 12];
        client.read_exact(&mut version).expect("version");
        client.write_all(b"RFB 003.008\n").expect("version");
        let mut list = [0u8; 2];
        client.read_exact(&mut list).expect("security list");
        assert_eq!(list, [1, SEC_VNC], "server must offer VNC auth");
        client.write_all(&[SEC_VNC]).expect("select VNC");
        let mut challenge = [0u8; 16];
        client.read_exact(&mut challenge).expect("challenge");
        let key = des_key(password);
        let mut response = [0u8; 16];
        for (chunk, out) in challenge.chunks_exact(8).zip(response.chunks_exact_mut(8)) {
            let block = u64::from_be_bytes(chunk.try_into().unwrap());
            out.copy_from_slice(&crate::des::encrypt_block(key, block).to_be_bytes());
        }
        client.write_all(&response).expect("response");
        let mut result = [0u8; 4];
        client.read_exact(&mut result).expect("security result");
        (client, u32::from_be_bytes(result))
    }

    #[test]
    fn vnc_auth_accepts_the_right_password() {
        let (server, _publisher) = start_server("EWM", false, Some("sekret".into()), false);
        let (mut client, result) = vnc_auth_result(server.port(), "sekret");
        assert_eq!(result, 0, "correct password should authenticate");
        // The handshake continues into ClientInit → ServerInit.
        client.write_all(&[1u8]).expect("shared flag");
        let mut init = [0u8; 24];
        client.read_exact(&mut init).expect("server init");
        assert_eq!(u16::from_be_bytes([init[0], init[1]]), 280);
    }

    #[test]
    fn vnc_auth_rejects_a_wrong_password() {
        let (server, _publisher) = start_server("EWM", false, Some("sekret".into()), false);
        let (_client, result) = vnc_auth_result(server.port(), "not-it");
        assert_eq!(result, 1, "wrong password should fail authentication");
    }

    /// A mock browser: masks its frames like a real WebSocket client and
    /// treats server frames as a byte stream, mirroring noVNC.
    struct WsClient {
        stream: TcpStream,
        buf: Vec<u8>,
        pos: usize,
    }

    impl WsClient {
        /// Connect and complete the HTTP→WebSocket upgrade.
        fn connect(port: u16) -> WsClient {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect ws");
            stream
                .write_all(
                    b"GET /websockify HTTP/1.1\r\n\
                      Host: localhost\r\n\
                      Upgrade: websocket\r\n\
                      Connection: Upgrade\r\n\
                      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                      Sec-WebSocket-Version: 13\r\n\r\n",
                )
                .expect("upgrade request");
            let mut response = String::new();
            let mut byte = [0u8; 1];
            while !response.ends_with("\r\n\r\n") {
                stream.read_exact(&mut byte).expect("upgrade response");
                response.push(byte[0] as char);
            }
            assert!(response.starts_with("HTTP/1.1 101"), "{response}");
            assert!(
                response.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
                "{response}"
            );
            WsClient {
                stream,
                buf: Vec::new(),
                pos: 0,
            }
        }

        /// Send RFB bytes in one masked binary frame.
        fn send(&mut self, payload: &[u8]) {
            crate::ws::write_masked_frame(&mut self.stream, crate::ws::OP_BINARY, payload)
                .expect("client frame");
        }

        /// Read exactly `n` RFB bytes, de-framing server messages as needed.
        fn read_bytes(&mut self, n: usize) -> Vec<u8> {
            let mut out = Vec::with_capacity(n);
            while out.len() < n {
                if self.pos == self.buf.len() {
                    let (_, opcode, payload) =
                        crate::ws::read_frame(&mut self.stream, false).expect("server frame");
                    assert_eq!(opcode, crate::ws::OP_BINARY, "server sends binary frames");
                    self.buf = payload;
                    self.pos = 0;
                }
                let take = (self.buf.len() - self.pos).min(n - out.len());
                out.extend_from_slice(&self.buf[self.pos..self.pos + take]);
                self.pos += take;
            }
            out
        }
    }

    /// The Phase 4 wire gate: the identical RFB session — handshake, input,
    /// framebuffer — through the WebSocket transport, alongside plain TCP.
    #[test]
    fn websocket_transport_serves_the_same_rfb_session() {
        let (server, publisher) = start_server("EWM ws", false, None, true);
        let ws_port = server.websocket_port().expect("ws port bound");
        let mut client = WsClient::connect(ws_port);

        // RFB handshake, byte-identical to the TCP path, inside WS frames.
        assert_eq!(client.read_bytes(12), b"RFB 003.008\n");
        client.send(b"RFB 003.008\n");
        assert_eq!(client.read_bytes(2), [1, SEC_NONE]);
        client.send(&[SEC_NONE]);
        assert_eq!(client.read_bytes(4), 0u32.to_be_bytes());
        client.send(&[1u8]); // ClientInit: shared
        let init = client.read_bytes(24);
        let width = u16::from_be_bytes([init[0], init[1]]);
        let height = u16::from_be_bytes([init[2], init[3]]);
        assert_eq!((width, height), (280, 192));
        let name_len = u32::from_be_bytes([init[20], init[21], init[22], init[23]]) as usize;
        assert_eq!(client.read_bytes(name_len), b"EWM ws");

        // Input events cross the transport into the emulator channel.
        client.send(&[4u8, 1, 0, 0, 0, 0, 0, 0x42]);
        assert_eq!(
            recv(&server),
            InputEvent::Key {
                down: true,
                keysym: 0x42
            }
        );

        // Publish a frame and request it: one Raw FramebufferUpdate.
        publisher.publish(&vec![0xAABBCCDDu32; 280 * 192]);
        client.send(&[3u8, 0, 0, 0, 0, 0, 0x01, 0x18, 0x00, 0xC0]);
        let header = client.read_bytes(16);
        assert_eq!(header[0], 0, "FramebufferUpdate");
        let encoding = i32::from_be_bytes([header[12], header[13], header[14], header[15]]);
        assert_eq!(encoding, 0, "Raw");
        let pixels = client.read_bytes(280 * 192 * 4);
        assert_eq!(&pixels[0..4], &0xAABBCCDDu32.to_be_bytes());

        // The plain-TCP listener still serves native clients in parallel.
        let mut tcp = TcpStream::connect(("127.0.0.1", server.port())).expect("tcp connect");
        do_handshake(&mut tcp);
    }

    /// Fetch one URL from the web-console port with a plain HTTP GET and
    /// return the full response (headers + body as lossy text).
    fn http_get(port: u16, path: &str) -> String {
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        client
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .expect("request");
        let mut response = Vec::new();
        client.read_to_end(&mut response).expect("response");
        String::from_utf8_lossy(&response).into_owned()
    }

    /// Phase 5: the WebSocket port doubles as the web console — the page,
    /// the noVNC engine modules, a 404 — while the upgrade keeps working.
    #[test]
    fn web_console_serves_novnc_beside_the_upgrade() {
        let (server, _publisher) = start_server("EWM web", false, None, true);
        let port = server.websocket_port().expect("ws port");

        let page = http_get(port, "/");
        assert!(page.starts_with("HTTP/1.1 200 OK"), "{page}");
        assert!(page.contains("Content-Type: text/html"), "{page}");
        assert!(
            page.contains("core/rfb.js"),
            "console page loads the engine"
        );

        let engine = http_get(port, "/core/rfb.js");
        assert!(engine.starts_with("HTTP/1.1 200 OK"), "engine served");
        assert!(engine.contains("Content-Type: text/javascript"), "{}", {
            engine.lines().take(6).collect::<Vec<_>>().join(" | ")
        });

        assert!(http_get(port, "/no-such-file").starts_with("HTTP/1.1 404"));

        // The same port still speaks RFB-over-WebSocket.
        let mut ws = WsClient::connect(port);
        assert_eq!(ws.read_bytes(12), b"RFB 003.008\n");
    }

    /// Without the web console the WebSocket port answers plain HTTP with
    /// `426 Upgrade Required`, as in Phase 4.
    #[test]
    fn websocket_port_without_web_console_refuses_plain_http() {
        let (server, _publisher) = Server::start(
            Options {
                bind: "127.0.0.1".into(),
                port: 0,
                websocket: Some(0),
                web: false,
                name: "EWM".into(),
                view_only: false,
                password: None,
            },
            280,
            192,
        )
        .expect("bind");
        let response = http_get(server.websocket_port().expect("ws port"), "/");
        assert!(response.starts_with("HTTP/1.1 426"), "{response}");
    }
}
