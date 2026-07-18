//! Machine-state persistence: the chunk container (notes/STATE.md §4,
//! plans/20260718-01-machine-state.md). Hand-rolled in the house style — a
//! tagged-chunk binary format, everything little-endian, no dependencies,
//! and a parser that is total: corrupt or truncated input is an [`Error`],
//! never a panic.
//!
//! ```text
//! file   := magic "EWMS" | u32 version (=1) | chunk*
//! chunk  := tag [u8;4] | u32 length | payload (length bytes)
//! ```
//!
//! Chunks nest freely: a payload may itself be a sequence of chunks. The
//! convention (notes/STATE.md §3.3): a component reads and writes its
//! *payload* only; the **owner** frames its children in chunks and fixes the
//! order, so tags and sequencing live at the level that owns the structure.

use std::fmt;

/// A four-byte chunk tag, written literally: `*b"CPU "`.
pub type Tag = [u8; 4];

pub const MAGIC: Tag = *b"EWMS";
pub const VERSION: u32 = 1;

/// Why a state file could not be read or written. One flat type: state
/// files are local and trusted, so errors are for humans, not for
/// programmatic dispatch.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error(e.to_string())
    }
}

fn corrupt(what: &str) -> Error {
    Error(format!("corrupt state: {what}"))
}

pub type Result<T> = std::result::Result<T, Error>;

/// Component-local machine-state persistence (notes/STATE.md §3). Save and
/// restore must agree exactly: only *runtime* state is written — anything
/// reconstructible from config or ROMs is not — and `restore` overwrites
/// every field `save` recorded. Impls read and write their payload only;
/// the owner frames children in chunks and fixes the order. On `Err` the
/// machine is unusable: restore is all-or-nothing at the top level.
pub trait Persist {
    /// Append this component's payload to the writer.
    fn save(&self, w: &mut Writer);

    /// Restore from a reader holding exactly this component's payload.
    fn restore(&mut self, r: &mut Reader) -> Result<()>;
}

/// Builds a chunk stream in memory. All integers little-endian.
#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer::default()
    }

    pub fn put_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn put_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn put_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn put_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn put_bool(&mut self, v: bool) {
        self.put_u8(v as u8);
    }

    /// Raw bytes, no length prefix (the surrounding chunk bounds them).
    pub fn put_bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }

    /// Length-prefixed bytes (u32 length, then the bytes).
    pub fn put_blob(&mut self, v: &[u8]) {
        self.put_u32(v.len() as u32);
        self.put_bytes(v);
    }

    /// Length-prefixed UTF-8 string.
    pub fn put_str(&mut self, v: &str) {
        self.put_blob(v.as_bytes());
    }

    /// Write one chunk: tag, length (patched after the fact), and whatever
    /// `body` appends as the payload.
    pub fn chunk(&mut self, tag: Tag, body: impl FnOnce(&mut Writer)) {
        self.buf.extend_from_slice(&tag);
        let at = self.buf.len();
        self.put_u32(0); // patched below
        body(self);
        let length = (self.buf.len() - at - 4) as u32;
        self.buf[at..at + 4].copy_from_slice(&length.to_le_bytes());
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

/// A bounds-checked cursor over a chunk payload. Every accessor errors on
/// truncation instead of panicking.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        if self.data.len() - self.pos < n {
            return Err(corrupt(&format!(
                "unexpected end reading {what} ({n} bytes wanted, {} left)",
                self.data.len() - self.pos
            )));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    pub fn get_u8(&mut self) -> Result<u8> {
        Ok(self.take(1, "u8")?[0])
    }

    pub fn get_u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.take(2, "u16")?.try_into().expect("2 bytes"),
        ))
    }

    pub fn get_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4, "u32")?.try_into().expect("4 bytes"),
        ))
    }

    pub fn get_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8, "u64")?.try_into().expect("8 bytes"),
        ))
    }

    pub fn get_bool(&mut self) -> Result<bool> {
        Ok(self.get_u8()? != 0)
    }

    /// Raw bytes of a known length.
    pub fn get_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.take(n, "bytes")
    }

    /// Length-prefixed bytes written by [`Writer::put_blob`].
    pub fn get_blob(&mut self) -> Result<&'a [u8]> {
        let n = self.get_u32()? as usize;
        self.take(n, "blob")
    }

    /// Length-prefixed UTF-8 string written by [`Writer::put_str`].
    pub fn get_str(&mut self) -> Result<String> {
        let bytes = self.get_blob()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| corrupt("string is not UTF-8"))
    }

    /// Open the next chunk, which must carry `expected` (strict v1: chunk
    /// order is fixed by the owner, notes/STATE.md §4). Returns a reader
    /// over the payload; the cursor advances past the whole chunk.
    pub fn chunk(&mut self, expected: Tag) -> Result<Reader<'a>> {
        let tag: Tag = self.take(4, "chunk tag")?.try_into().expect("4 bytes");
        if tag != expected {
            return Err(corrupt(&format!(
                "expected chunk {:?}, found {:?}",
                String::from_utf8_lossy(&expected),
                String::from_utf8_lossy(&tag)
            )));
        }
        let length = self.get_u32()? as usize;
        Ok(Reader::new(self.take(length, "chunk payload")?))
    }

    /// Strictness check: a component must consume its payload exactly, so
    /// format drift fails loudly instead of silently misaligning.
    pub fn done(&self) -> Result<()> {
        if self.pos != self.data.len() {
            return Err(corrupt(&format!(
                "{} unread bytes at end of chunk",
                self.data.len() - self.pos
            )));
        }
        Ok(())
    }
}

/// Write a state file atomically: magic + version + the writer's chunks to
/// `path.tmp`, then rename over `path`. A crash mid-save leaves any previous
/// state intact.
pub fn write_file(path: &str, writer: Writer) -> Result<()> {
    let mut bytes = Vec::with_capacity(writer.buf.len() + 8);
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&writer.buf);
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| Error(format!("cannot write {tmp}: {e}")))?;
    std::fs::rename(&tmp, path).map_err(|e| Error(format!("cannot rename {tmp} to {path}: {e}")))
}

/// Read a state file's chunk payload (magic and version checked, strict).
pub fn read_file(path: &str) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path).map_err(|e| Error(format!("cannot read {path}: {e}")))?;
    let mut r = Reader::new(&bytes);
    let magic: Tag = r.get_bytes(4)?.try_into().expect("4 bytes");
    if magic != MAGIC {
        return Err(corrupt("not an EWM state file (bad magic)"));
    }
    let version = r.get_u32()?;
    if version != VERSION {
        return Err(Error(format!(
            "state file version {version} (this build reads version {VERSION})"
        )));
    }
    Ok(bytes[r.pos..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_values_round_trip() {
        let mut w = Writer::new();
        w.put_u8(0x12);
        w.put_u16(0x3456);
        w.put_u32(0x789a_bcde);
        w.put_u64(0x0123_4567_89ab_cdef);
        w.put_bool(true);
        w.put_bytes(&[1, 2, 3]);
        w.put_blob(&[4, 5]);
        w.put_str("héllo");
        let bytes = w.into_bytes();

        let mut r = Reader::new(&bytes);
        assert_eq!(r.get_u8().unwrap(), 0x12);
        assert_eq!(r.get_u16().unwrap(), 0x3456);
        assert_eq!(r.get_u32().unwrap(), 0x789a_bcde);
        assert_eq!(r.get_u64().unwrap(), 0x0123_4567_89ab_cdef);
        assert!(r.get_bool().unwrap());
        assert_eq!(r.get_bytes(3).unwrap(), &[1, 2, 3]);
        assert_eq!(r.get_blob().unwrap(), &[4, 5]);
        assert_eq!(r.get_str().unwrap(), "héllo");
        r.done().expect("fully consumed");
    }

    #[test]
    fn integers_are_little_endian_on_the_wire() {
        let mut w = Writer::new();
        w.put_u32(0x0403_0201);
        assert_eq!(w.into_bytes(), [1, 2, 3, 4]);
    }

    #[test]
    fn chunks_nest_and_bound_their_payloads() {
        let mut w = Writer::new();
        w.chunk(*b"OUTR", |w| {
            w.put_u8(1);
            w.chunk(*b"INNR", |w| w.put_str("nested"));
            w.put_u8(2);
        });
        w.chunk(*b"NEXT", |w| w.put_u16(7));
        let bytes = w.into_bytes();

        let mut r = Reader::new(&bytes);
        let mut outer = r.chunk(*b"OUTR").expect("outer");
        assert_eq!(outer.get_u8().unwrap(), 1);
        let mut inner = outer.chunk(*b"INNR").expect("inner");
        assert_eq!(inner.get_str().unwrap(), "nested");
        inner.done().expect("inner consumed");
        assert_eq!(outer.get_u8().unwrap(), 2);
        outer.done().expect("outer consumed");
        let mut next = r.chunk(*b"NEXT").expect("next");
        assert_eq!(next.get_u16().unwrap(), 7);
        r.done().expect("stream consumed");
    }

    #[test]
    fn wrong_tag_truncation_and_leftovers_are_errors_not_panics() {
        let mut w = Writer::new();
        w.chunk(*b"GOOD", |w| w.put_u32(1));
        let bytes = w.into_bytes();

        // Wrong tag.
        assert!(Reader::new(&bytes).chunk(*b"EVIL").is_err());
        // Truncated payload.
        assert!(
            Reader::new(&bytes[..bytes.len() - 2])
                .chunk(*b"GOOD")
                .is_err()
        );
        // Truncated header.
        assert!(Reader::new(&bytes[..3]).chunk(*b"GOOD").is_err());
        // Unread payload bytes are a loud error, not silent drift.
        let mut r = Reader::new(&bytes);
        let payload = r.chunk(*b"GOOD").unwrap();
        assert!(payload.done().is_err());
        // A blob whose length outruns the data is an error.
        let mut w = Writer::new();
        w.put_u32(1000);
        let bad = w.into_bytes();
        assert!(Reader::new(&bad).get_blob().is_err());
    }

    #[test]
    fn file_round_trip_is_atomic_and_validated() {
        let dir = std::env::temp_dir().join(format!("ewm-state-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test dir");
        let path = dir.join("machine.state");
        let path = path.to_str().expect("utf-8 path");

        let mut w = Writer::new();
        w.chunk(*b"TEST", |w| w.put_str("payload"));
        write_file(path, w).expect("write");
        assert!(
            !std::fs::exists(format!("{path}.tmp")).unwrap_or(true),
            "temp file renamed away"
        );

        let payload = read_file(path).expect("read");
        let mut r = Reader::new(&payload);
        let mut t = r.chunk(*b"TEST").expect("chunk");
        assert_eq!(t.get_str().unwrap(), "payload");

        // Bad magic and bad version are clear errors.
        std::fs::write(path, b"NOPE\x01\x00\x00\x00").expect("clobber");
        assert!(read_file(path).is_err());
        let mut bad = MAGIC.to_vec();
        bad.extend_from_slice(&99u32.to_le_bytes());
        std::fs::write(path, &bad).expect("clobber");
        let err = read_file(path).expect_err("version rejected").to_string();
        assert!(err.contains("version 99"), "{err}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
