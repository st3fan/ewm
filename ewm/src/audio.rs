//! The WebAudio side-channel (notes/VNC.md §4): stream the speaker's PCM to
//! browser clients over a WebSocket, since RFB itself has no audio. The
//! emulator loop publishes each frame's samples into a [`Hub`]; every
//! connected `/audio` client has a small bounded queue drained by its own
//! writer thread. The wire is deliberately boring: one text frame of JSON
//! (`{"format":"s16le","rate":44100,"channels":1}`) and then binary frames
//! of raw little-endian mono i16 — ~88 KB/s per listener, no compression.
//!
//! Backpressure is drop-*oldest*: a slow or backgrounded tab loses the
//! stalest chunks and resumes near-live, instead of accumulating a permanent
//! latency the size of its queue. The `Wave` decay model makes the resulting
//! gap sound like at most a click (notes/VNC.md §4.3).

use std::collections::VecDeque;
use std::io::{self, Read};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::snd::SND_SAMPLE_RATE;

/// Most PCM chunks a client may have queued (~200 ms at 40 fps) before the
/// oldest are dropped.
const CLIENT_QUEUE_MAX: usize = 8;

/// The header every client receives first, as a WebSocket text frame.
fn header() -> String {
    format!("{{\"format\":\"s16le\",\"rate\":{SND_SAMPLE_RATE},\"channels\":1}}")
}

/// One connected audio listener: its chunk queue and the wakeup for the
/// writer thread that drains it.
struct Client {
    queue: Mutex<VecDeque<Arc<Vec<u8>>>>,
    ready: Condvar,
    closed: AtomicBool,
}

/// The broadcast point between the emulator loop (one `publish` per frame)
/// and the `/audio` WebSocket connections.
#[derive(Default)]
pub struct Hub {
    clients: Mutex<Vec<Arc<Client>>>,
}

impl Hub {
    pub fn new() -> Arc<Hub> {
        Arc::new(Hub::default())
    }

    /// Broadcast one frame's samples to every listener. With no listeners
    /// this is one short-held lock; encoding happens once, shared by `Arc`.
    pub fn publish(&self, samples: &[i16]) {
        let mut clients = self.clients.lock().expect("audio clients mutex");
        clients.retain(|client| !client.closed.load(Ordering::Relaxed));
        if clients.is_empty() || samples.is_empty() {
            return;
        }
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for &sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let chunk = Arc::new(bytes);
        for client in clients.iter() {
            let mut queue = client.queue.lock().expect("audio queue mutex");
            enqueue(&mut queue, chunk.clone());
            drop(queue);
            client.ready.notify_one();
        }
    }

    /// Serve one already-upgraded `/audio` connection on the current thread:
    /// send the format header, then chunks as they arrive, until the client
    /// goes away. A drain thread consumes the client's incoming frames
    /// (answering pings, echoing close) so the socket stays healthy.
    pub fn attach(&self, stream: TcpStream) -> io::Result<()> {
        let (mut reader, mut writer) = crate::ws::split(&stream)?;
        let client = Arc::new(Client {
            queue: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            closed: AtomicBool::new(false),
        });
        self.clients
            .lock()
            .expect("audio clients mutex")
            .push(client.clone());

        // Incoming side: we expect nothing, but WsReader answers pings and
        // turns a Close into EOF; EOF or error ends the connection.
        let drain_client = client.clone();
        let drain_stream = stream.try_clone()?;
        let drain = std::thread::spawn(move || {
            let mut sink = [0u8; 256];
            loop {
                match reader.read(&mut sink) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            drain_client.closed.store(true, Ordering::Relaxed);
            drain_client.ready.notify_one();
        });

        let result = write_loop(&client, &mut writer);

        client.closed.store(true, Ordering::Relaxed);
        let _ = drain_stream.shutdown(std::net::Shutdown::Both);
        let _ = drain.join();
        result
    }
}

/// Push a chunk, dropping the oldest past the cap.
fn enqueue(queue: &mut VecDeque<Arc<Vec<u8>>>, chunk: Arc<Vec<u8>>) {
    queue.push_back(chunk);
    while queue.len() > CLIENT_QUEUE_MAX {
        queue.pop_front();
    }
}

/// The writer half of one connection: header first, then queued chunks as
/// binary frames, blocking on the condvar between publishes.
fn write_loop(client: &Client, writer: &mut crate::ws::WsWriter) -> io::Result<()> {
    use std::io::Write;
    writer.write_text(&header())?;
    loop {
        let mut queue = client.queue.lock().expect("audio queue mutex");
        let chunk = loop {
            if client.closed.load(Ordering::Relaxed) {
                return Ok(());
            }
            if let Some(chunk) = queue.pop_front() {
                break chunk;
            }
            let (guard, _) = client
                .ready
                .wait_timeout(queue, Duration::from_millis(200))
                .expect("audio queue mutex");
            queue = guard;
        };
        drop(queue);
        writer.write_all(&chunk)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_drops_the_oldest_past_the_cap() {
        let mut queue = VecDeque::new();
        for i in 0..(CLIENT_QUEUE_MAX + 3) {
            enqueue(&mut queue, Arc::new(vec![i as u8]));
        }
        assert_eq!(queue.len(), CLIENT_QUEUE_MAX);
        // The three oldest chunks are gone; the newest survives.
        assert_eq!(queue.front().expect("front")[0], 3);
        assert_eq!(queue.back().expect("back")[0], (CLIENT_QUEUE_MAX + 2) as u8);
    }

    #[test]
    fn header_names_the_wave_format() {
        assert_eq!(
            header(),
            "{\"format\":\"s16le\",\"rate\":44100,\"channels\":1}"
        );
    }

    /// End to end over a loopback socket: upgrade, header text frame, then a
    /// published chunk arrives as little-endian bytes in a binary frame.
    #[test]
    fn attach_streams_header_then_pcm() {
        use std::io::Write;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let hub = Hub::new();

        let serve_hub = hub.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = crate::ws::read_http_request(&mut stream).expect("request");
            crate::ws::accept_upgrade(&mut stream, &request).expect("upgrade");
            let _ = serve_hub.attach(stream);
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        client
            .write_all(
                b"GET /audio HTTP/1.1\r\n\
                  Host: localhost\r\n\
                  Upgrade: websocket\r\n\
                  Connection: Upgrade\r\n\
                  Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                  Sec-WebSocket-Version: 13\r\n\r\n",
            )
            .expect("upgrade request");
        let mut byte = [0u8; 1];
        let mut response = String::new();
        while !response.ends_with("\r\n\r\n") {
            client.read_exact(&mut byte).expect("response");
            response.push(byte[0] as char);
        }
        assert!(response.starts_with("HTTP/1.1 101"), "{response}");

        // The header text frame announces the format.
        let (_, opcode, payload) = crate::ws::read_frame(&mut client, false).expect("header");
        assert_eq!(opcode, crate::ws::OP_TEXT);
        assert_eq!(payload, header().as_bytes());

        // The header is sent only after the client is registered with the
        // hub, so having read it, a publish is guaranteed to reach us.
        let samples: Vec<i16> = vec![0x1234, -2, 0, 257];
        hub.publish(&samples);
        let (_, opcode, payload) = crate::ws::read_frame(&mut client, false).expect("pcm frame");
        assert_eq!(opcode, crate::ws::OP_BINARY);
        let mut expected = Vec::new();
        for &sample in &samples {
            expected.extend_from_slice(&sample.to_le_bytes());
        }
        assert_eq!(payload, expected);

        drop(client); // disconnect: attach() returns, the server thread ends
        server.join().expect("server thread");
    }
}
