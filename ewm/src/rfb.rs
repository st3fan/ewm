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
}

impl Channel {
    fn new(width: u16, height: u16, name: String) -> Channel {
        Channel {
            frame: Mutex::new(Frame {
                bytes: vec![0; width as usize * height as usize * 4],
                generation: 0,
            }),
            dirty: Condvar::new(),
            width,
            height,
            name,
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

/// A running RFB server: the listener and its clients live on background
/// threads; the emulator keeps a [`Publisher`] to push frames and this
/// `Server` to drain input between frames.
pub struct Server {
    input: Receiver<InputEvent>,
    port: u16,
}

impl Server {
    /// Bind `bind:port` (port 0 picks an ephemeral port — tests) and start
    /// accepting clients. Returns the `Server` (drain input from it) and a
    /// [`Publisher`] (push frames into it). `view_only` drops all client
    /// input at the source.
    pub fn start(
        bind: &str,
        port: u16,
        width: u16,
        height: u16,
        name: &str,
        view_only: bool,
    ) -> io::Result<(Server, Publisher)> {
        let listener = TcpListener::bind((bind, port))?;
        let port = listener.local_addr()?.port();
        let channel = Arc::new(Channel::new(width, height, name.to_string()));
        let (input_tx, input) = std::sync::mpsc::channel();
        let accept_channel = channel.clone();
        std::thread::spawn(move || accept(listener, accept_channel, input_tx, view_only));
        Ok((Server { input, port }, Publisher { channel }))
    }

    /// The bound port (useful when 0 was passed).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Pop one pending input event, or `None`. The emulator loop drains this
    /// each frame — an idle server costs one `try_recv`.
    pub fn try_recv_input(&self) -> Option<InputEvent> {
        self.input.try_recv().ok()
    }
}

/// Accept connections forever, spawning a handler thread per client so several
/// viewers can watch the same machine at once (RFB shared-desktop).
fn accept(
    listener: TcpListener,
    channel: Arc<Channel>,
    input_tx: Sender<InputEvent>,
    view_only: bool,
) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let channel = channel.clone();
        let input_tx = input_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, channel, input_tx, view_only) {
                // A dropped client is normal; log at debug volume only.
                if e.kind() != io::ErrorKind::UnexpectedEof {
                    eprintln!("[RFB] connection closed: {e}");
                }
            }
        });
    }
}

/// One client: RFB handshake, then split into a reader thread (client
/// messages → input channel + update requests) and this thread as the writer
/// (frames → the socket).
fn handle(
    mut stream: TcpStream,
    channel: Arc<Channel>,
    input_tx: Sender<InputEvent>,
    view_only: bool,
) -> io::Result<()> {
    handshake(&mut stream, &channel)?;

    let reader = stream.try_clone()?;
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

    let result = write_loop(&mut stream, &channel, &pending, &alive);

    // Whatever ended the writer, tear the connection down so the reader's
    // blocked `read` returns and the thread joins.
    alive.store(false, Ordering::Relaxed);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    let _ = reader_thread.join();
    result
}

/// The RFB 3.x handshake through `ServerInit`: ProtocolVersion, security type
/// `None`, `ClientInit`, then our geometry and pixel format.
fn handshake(stream: &mut TcpStream, channel: &Channel) -> io::Result<()> {
    // ProtocolVersion: offer 3.8; read the client's choice.
    stream.write_all(b"RFB 003.008\n")?;
    let mut version = [0u8; 12];
    stream.read_exact(&mut version)?;
    let minor = parse_minor(&version);

    // Security: type None (1). 3.7+ negotiates from a list; 3.3 is dictated.
    if minor >= 7 {
        stream.write_all(&[1u8, 1u8])?; // one type on offer: None
        let mut selected = [0u8; 1];
        stream.read_exact(&mut selected)?;
        // SecurityResult (OK) is a 3.8-and-later addition.
        if minor >= 8 {
            stream.write_all(&0u32.to_be_bytes())?;
        }
    } else {
        stream.write_all(&1u32.to_be_bytes())?; // dictated: None
    }

    // ClientInit: one shared-flag byte we accept and ignore (all viewers
    // share the one machine).
    let mut shared = [0u8; 1];
    stream.read_exact(&mut shared)?;

    // ServerInit: width, height, PIXEL_FORMAT, name.
    let mut init = Vec::with_capacity(24 + channel.name.len());
    init.extend_from_slice(&channel.width.to_be_bytes());
    init.extend_from_slice(&channel.height.to_be_bytes());
    init.extend_from_slice(&PIXEL_FORMAT);
    init.extend_from_slice(&(channel.name.len() as u32).to_be_bytes());
    init.extend_from_slice(channel.name.as_bytes());
    stream.write_all(&init)?;
    Ok(())
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
fn read_loop(
    mut reader: TcpStream,
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
fn handle_message(
    reader: &mut TcpStream,
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
fn skip(reader: &mut TcpStream, n: usize) -> io::Result<()> {
    io::copy(&mut reader.take(n as u64), &mut io::sink())?;
    Ok(())
}

/// Send a full-frame Raw `FramebufferUpdate` whenever the client has a request
/// outstanding: immediately for a non-incremental request, otherwise as soon
/// as the frame generation advances. Blocks on the `dirty` condvar between
/// sends, with a short timeout as a lost-wakeup safety net.
fn write_loop(
    stream: &mut TcpStream,
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
        stream.write_all(&update)?;
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

    /// Drive the full handshake over a loopback socket and assert the first
    /// `FramebufferUpdate` decodes to the advertised geometry — the RFB
    /// analogue of `wozbug`'s `server_round_trip`.
    #[test]
    fn handshake_and_first_update() {
        let (server, publisher) =
            Server::start("127.0.0.1", 0, 280, 192, "EWM test", false).expect("bind");
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
        let (server, _publisher) =
            Server::start("127.0.0.1", 0, 280, 192, "EWM", true).expect("bind");
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
        let (server, _publisher) =
            Server::start("127.0.0.1", 0, 280, 192, "EWM", false).expect("bind");
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
}
