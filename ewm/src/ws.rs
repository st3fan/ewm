//! A minimal WebSocket (RFC 6455) server transport, hand-rolled on `std::net`
//! like `rfb.rs` and `des.rs` — no crates, no async runtime. Just enough for
//! RFB-over-WebSocket so a browser's noVNC connects **directly** to EWM with
//! no websockify sidecar (notes/REMOTE.md, Phase 4).
//!
//! Scope: server side only, binary frames, no extensions (permessage-deflate
//! offers are declined by simply not echoing them), no TLS (terminate `wss://`
//! at a reverse proxy — notes/REMOTE.md §10). The handshake needs SHA-1 and
//! base64, both implemented here and tested against the RFC vectors; SHA-1 is
//! long broken for signatures but the WebSocket `Sec-WebSocket-Accept` proof
//! is not a security boundary, just protocol plumbing.
//!
//! The transport is exposed as [`WsReader`] / [`WsWriter`], which implement
//! `io::Read` / `io::Write` by de-framing and framing WebSocket messages, so
//! the RFB state machine in `rfb.rs` runs over either transport unchanged.
//! RFB is a byte stream, so incoming data frames are simply appended to a
//! queue — message boundaries (and fragmentation) carry no meaning here, and
//! noVNC likewise treats received frames as a byte stream.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// The magic GUID every WebSocket accept key is salted with (RFC 6455 §1.3).
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Longest client frame we accept. RFB client messages are tiny (the largest
/// is `ClientCutText`, which we cap the same way); anything bigger is a
/// misbehaving peer, not a VNC client.
const MAX_FRAME: u64 = 1 << 20;

/// Longest HTTP upgrade request we read before giving up.
const MAX_REQUEST: usize = 16 * 1024;

pub const OP_CONTINUATION: u8 = 0x0;
pub const OP_TEXT: u8 = 0x1;
pub const OP_BINARY: u8 = 0x2;
pub const OP_CLOSE: u8 = 0x8;
pub const OP_PING: u8 = 0x9;
pub const OP_PONG: u8 = 0xA;

/// SHA-1 (FIPS 180-4) of `data`. Only used for the WebSocket accept key.
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    // Pad: 0x80, zeros to 56 mod 64, then the bit length as a big-endian u64.
    let mut msg = data.to_vec();
    let bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (word, bytes) in w.iter_mut().zip(chunk.chunks_exact(4)) {
            *word = u32::from_be_bytes(bytes.try_into().expect("4-byte chunk"));
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (dst, word) in out.chunks_exact_mut(4).zip(h.iter()) {
        dst.copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Standard base64 (RFC 4648, with padding). Only used for the accept key.
pub fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | (*chunk.get(2).unwrap_or(&0) as u32);
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// The `Sec-WebSocket-Accept` value for a client's `Sec-WebSocket-Key`.
pub fn accept_key(key: &str) -> String {
    base64(&sha1(format!("{key}{GUID}").as_bytes()))
}

/// One parsed HTTP request on the WebSocket port: either a WebSocket upgrade
/// (→ RFB) or a plain request (→ the Phase 5 web console, or `426`).
pub struct Request {
    /// The request path (`GET <path> HTTP/1.1`).
    pub path: String,
    /// `Some(Sec-WebSocket-Key)` when this is a well-formed GET + upgrade.
    key: Option<String>,
    /// The client offered the "binary" subprotocol (older noVNC), which we
    /// must echo or the browser drops the connection client-side.
    binary: bool,
}

impl Request {
    /// Whether this request asks for the WebSocket upgrade.
    pub fn is_upgrade(&self) -> bool {
        self.key.is_some()
    }
}

/// Read and parse one HTTP request from the socket (headers only).
pub fn read_http_request(stream: &mut TcpStream) -> io::Result<Request> {
    let request = read_request(stream)?;
    let text = String::from_utf8_lossy(&request);

    let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
    let upgrade_ok = header(&text, "upgrade")
        .is_some_and(|v| v.to_ascii_lowercase().contains("websocket"))
        && text.starts_with("GET ");
    let key = match header(&text, "sec-websocket-key") {
        Some(key) if upgrade_ok => Some(key.to_string()),
        _ => None,
    };
    let binary = header(&text, "sec-websocket-protocol")
        .is_some_and(|v| v.split(',').any(|p| p.trim() == "binary"));
    Ok(Request { path, key, binary })
}

/// Answer a WebSocket upgrade with `101 Switching Protocols`. The request
/// path is ignored, so noVNC's default `/websockify` path works unmodified.
pub fn accept_upgrade(stream: &mut TcpStream, request: &Request) -> io::Result<()> {
    let key = request.key.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "not a WebSocket upgrade request",
        )
    })?;
    let mut response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n",
        accept_key(key)
    );
    if request.binary {
        response.push_str("Sec-WebSocket-Protocol: binary\r\n");
    }
    response.push_str("\r\n");
    stream.write_all(response.as_bytes())
}

/// Answer a plain HTTP request on a WebSocket-only port (no web console):
/// `426 Upgrade Required`, with a hint.
pub fn refuse_plain_http(stream: &mut TcpStream) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 426 Upgrade Required\r\n\
          Connection: close\r\n\
          Content-Type: text/plain\r\n\r\n\
          This is EWM's RFB-over-WebSocket port; connect with noVNC,\n\
          or start the machine with the web console (--serve ...?web=PORT).\n",
    )
}

/// Read the HTTP request up to and including the blank line. Byte-at-a-time,
/// so nothing past the header block (the first WebSocket frame) is consumed.
fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while !request.ends_with(b"\r\n\r\n") {
        if request.len() >= MAX_REQUEST {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversized HTTP request",
            ));
        }
        stream.read_exact(&mut byte)?;
        request.push(byte[0]);
    }
    Ok(request)
}

/// The value of a header (case-insensitive name) in a raw HTTP request.
fn header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request.lines().skip(1).find_map(|line| {
        let (header, value) = line.split_once(':')?;
        header
            .trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim())
    })
}

/// Write one unmasked frame (servers must not mask, RFC 6455 §5.1), FIN set.
pub fn write_frame<W: Write>(w: &mut W, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let mut header = Vec::with_capacity(10);
    header.push(0x80 | opcode);
    match payload.len() {
        len if len < 126 => header.push(len as u8),
        len if len <= 0xFFFF => {
            header.push(126);
            header.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            header.push(127);
            header.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    w.write_all(&header)?;
    w.write_all(payload)
}

/// Read one frame: `(fin, opcode, payload)`, unmasked. `require_masked`
/// enforces the client-to-server masking rule (RFC 6455 §5.1); the tests'
/// mock client reads our unmasked server frames with it off.
pub fn read_frame<R: Read>(r: &mut R, require_masked: bool) -> io::Result<(bool, u8, Vec<u8>)> {
    let mut head = [0u8; 2];
    r.read_exact(&mut head)?;
    let fin = head[0] & 0x80 != 0;
    if head[0] & 0x70 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "RSV bits set (no extensions negotiated)",
        ));
    }
    let opcode = head[0] & 0x0F;
    let masked = head[1] & 0x80 != 0;
    let len = match head[1] & 0x7F {
        126 => {
            let mut ext = [0u8; 2];
            r.read_exact(&mut ext)?;
            u16::from_be_bytes(ext) as u64
        }
        127 => {
            let mut ext = [0u8; 8];
            r.read_exact(&mut ext)?;
            u64::from_be_bytes(ext)
        }
        len => len as u64,
    };
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized WebSocket frame",
        ));
    }
    if require_masked && !masked {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unmasked client frame",
        ));
    }
    let mut key = [0u8; 4];
    if masked {
        r.read_exact(&mut key)?;
    }
    let mut payload = vec![0u8; len as usize];
    r.read_exact(&mut payload)?;
    if masked {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
    }
    Ok((fin, opcode, payload))
}

/// Write one masked client-to-server frame — the test mock client's half of
/// the protocol (a real browser does this for us).
#[cfg(test)]
pub fn write_masked_frame<W: Write>(w: &mut W, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let key = [0x12u8, 0x34, 0x56, 0x78]; // any mask works; tests need no entropy
    let mut header = Vec::with_capacity(14);
    header.push(0x80 | opcode);
    match payload.len() {
        len if len < 126 => header.push(0x80 | len as u8),
        len if len <= 0xFFFF => {
            header.push(0x80 | 126);
            header.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            header.push(0x80 | 127);
            header.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    header.extend_from_slice(&key);
    let masked: Vec<u8> = payload
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % 4])
        .collect();
    w.write_all(&header)?;
    w.write_all(&masked)
}

/// The read half of an upgraded connection: de-frames incoming data into a
/// byte queue (`io::Read`), answers pings, and turns a Close frame into EOF.
/// Holds the shared write half so pong/close replies interleave safely with
/// the writer thread's frames.
pub struct WsReader {
    stream: TcpStream,
    writer: Arc<Mutex<TcpStream>>,
    buf: Vec<u8>,
    pos: usize,
}

/// The write half: every `write` becomes one binary frame, serialized through
/// the shared stream so the reader's pong replies never split a frame.
pub struct WsWriter {
    writer: Arc<Mutex<TcpStream>>,
}

/// Split an upgraded connection into the reader/writer pair `rfb::run` needs.
pub fn split(stream: &TcpStream) -> io::Result<(WsReader, WsWriter)> {
    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    let reader = WsReader {
        stream: stream.try_clone()?,
        writer: writer.clone(),
        buf: Vec::new(),
        pos: 0,
    };
    Ok((reader, WsWriter { writer }))
}

impl Read for WsReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.pos == self.buf.len() {
            let (_fin, opcode, payload) = read_frame(&mut self.stream, true)?;
            match opcode {
                // RFB is a byte stream: data frames (fragmented or not) just
                // append; boundaries carry no meaning.
                OP_BINARY | OP_TEXT | OP_CONTINUATION => {
                    self.buf = payload;
                    self.pos = 0;
                }
                OP_PING => {
                    let mut w = self.writer.lock().expect("ws writer mutex");
                    write_frame(&mut *w, OP_PONG, &payload)?;
                }
                OP_PONG => {}
                OP_CLOSE => {
                    // Echo the status code (best effort) and report EOF.
                    let code = &payload[..payload.len().min(2)];
                    if let Ok(mut w) = self.writer.lock() {
                        let _ = write_frame(&mut *w, OP_CLOSE, code);
                    }
                    return Ok(0);
                }
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unsupported WebSocket opcode {other}"),
                    ));
                }
            }
        }
        let n = (self.buf.len() - self.pos).min(out.len());
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Write for WsWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut w = self.writer.lock().expect("ws writer mutex");
        write_frame(&mut *w, OP_BINARY, buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.lock().expect("ws writer mutex").flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha1_vectors() {
        // FIPS 180 / classic vectors.
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha1(b"The quick brown fox jumps over the lazy dog")),
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        );
        // Cross the one-block boundary (padding path for len % 64 >= 56).
        assert_eq!(
            hex(&sha1(&[b'a'; 64])),
            "0098ba824b5c16427bd7a1122a5a442a25ec644d"
        );
    }

    #[test]
    fn base64_vectors() {
        // RFC 4648 §10.
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }

    #[test]
    fn accept_key_rfc6455_example() {
        // The worked example in RFC 6455 §1.3.
        assert_eq!(
            accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn frame_roundtrip_across_length_encodings() {
        // 7-bit, 16-bit, and 64-bit payload-length encodings.
        for len in [0usize, 1, 125, 126, 0xFFFF, 0x10000] {
            let payload: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let mut wire = Vec::new();
            write_masked_frame(&mut wire, OP_BINARY, &payload).expect("write");
            let (fin, opcode, decoded) =
                read_frame(&mut Cursor::new(&wire), true).expect("read masked frame");
            assert!(fin);
            assert_eq!(opcode, OP_BINARY);
            assert_eq!(decoded, payload, "length {len}");
        }
    }

    #[test]
    fn server_frames_are_unmasked_and_rejected_as_client_frames() {
        let mut wire = Vec::new();
        write_frame(&mut wire, OP_BINARY, b"hello").expect("write");
        assert_eq!(wire[0], 0x80 | OP_BINARY);
        assert_eq!(wire[1], 5, "no mask bit, 7-bit length");
        // A server-style (unmasked) frame must be rejected on the server side.
        assert!(read_frame(&mut Cursor::new(&wire), true).is_err());
        // ... but reads fine with the rule off (the test client's view).
        let (_, _, payload) = read_frame(&mut Cursor::new(&wire), false).expect("read");
        assert_eq!(payload, b"hello");
    }

    #[test]
    fn upgrade_answers_101_with_the_accept_key() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_request(&mut stream)?;
            assert_eq!(request.path, "/websockify");
            assert!(request.is_upgrade());
            accept_upgrade(&mut stream, &request)
        });
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        client
            .write_all(
                b"GET /websockify HTTP/1.1\r\n\
                  Host: localhost\r\n\
                  Upgrade: websocket\r\n\
                  Connection: Upgrade\r\n\
                  Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                  Sec-WebSocket-Version: 13\r\n\r\n",
            )
            .expect("request");
        let mut response = String::new();
        // The server keeps the socket open for frames; read until the blank
        // line rather than to EOF.
        let mut byte = [0u8; 1];
        while !response.ends_with("\r\n\r\n") {
            client.read_exact(&mut byte).expect("response");
            response.push(byte[0] as char);
        }
        assert!(response.starts_with("HTTP/1.1 101"), "{response}");
        assert!(
            response.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
            "{response}"
        );
        server.join().expect("join").expect("upgrade ok");
    }

    #[test]
    fn plain_http_request_parses_and_gets_426() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_request(&mut stream).expect("parse");
            assert_eq!(request.path, "/");
            assert!(!request.is_upgrade());
            refuse_plain_http(&mut stream).expect("426");
        });
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("request");
        let mut response = String::new();
        client.read_to_string(&mut response).expect("response");
        assert!(response.starts_with("HTTP/1.1 426"), "{response}");
        server.join().expect("join");
    }

    #[test]
    fn reader_appends_data_frames_answers_pings_and_eofs_on_close() {
        // Loopback pair: the "client" end sends raw frames; the WsReader end
        // is read through the io::Read interface.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let (server_stream, _) = listener.accept().expect("accept");
        let (mut reader, _writer) = split(&server_stream).expect("split");

        write_masked_frame(&mut client, OP_BINARY, b"abc").expect("data");
        write_masked_frame(&mut client, OP_PING, b"hi").expect("ping");
        write_masked_frame(&mut client, OP_BINARY, b"def").expect("data");
        write_masked_frame(&mut client, OP_CLOSE, &[0x03, 0xE8]).expect("close");

        let mut all = [0u8; 6];
        reader.read_exact(&mut all).expect("stream bytes");
        assert_eq!(&all, b"abcdef", "frames concatenate into one byte stream");

        // The ping got a pong (sent before the second data frame was served).
        let (_, opcode, payload) = read_frame(&mut client, false).expect("pong");
        assert_eq!((opcode, payload.as_slice()), (OP_PONG, b"hi".as_slice()));

        // Close → EOF on the Read side, and the close is echoed back.
        assert_eq!(reader.read(&mut [0u8; 1]).expect("eof"), 0);
        let (_, opcode, payload) = read_frame(&mut client, false).expect("close echo");
        assert_eq!(
            (opcode, payload.as_slice()),
            (OP_CLOSE, [0x03, 0xE8].as_slice())
        );
    }
}
