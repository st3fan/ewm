//! Disk images over HTTP: a config may point a floppy or hard-drive
//! image at an `http(s)://` URL instead of a local path. The image is
//! downloaded once into a per-URL cache directory and revalidated with
//! `ETag` / `Last-Modified` on later runs, so booting the same machine
//! twice costs one conditional request (usually a 304), not a re-download.
//!
//! Cache layout — `$XDG_CACHE_HOME/ewm` (else `~/.cache/ewm`):
//!
//! ```text
//! <root>/<sha1 of the URL>/Total Replay v6.0.1.hdv
//! <root>/<sha1 of the URL>/meta.json     # etag / last-modified
//! ```
//!
//! The digest keys the directory (URLs are not filesystem-safe), while the
//! file keeps its own name so the cache is browsable and error messages
//! read normally. Offline is not fatal: if revalidation fails but a copy
//! is cached, the cached copy is used.

use std::path::{Path, PathBuf};

/// Cap on a downloaded image: a ProDOS volume tops out at 32MB, so this
/// is generous while still refusing to fill the disk with a bad URL.
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Is this source an HTTP(S) URL rather than a local path?
pub fn is_url(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

/// The local file a source names: a path is itself, a URL is its cached
/// download (fetched now if the cache is cold, revalidated if warm).
pub fn local_path(src: &str) -> Result<String, String> {
    if !is_url(src) {
        return Ok(src.to_string());
    }
    let root = cache_root()?;
    cached_download(&root, src)
}

/// Where downloads live: `$XDG_CACHE_HOME/ewm`, else `~/.cache/ewm`.
fn cache_root() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME")
        && !dir.is_empty()
    {
        return Ok(PathBuf::from(dir).join("ewm"));
    }
    let home = std::env::var_os("HOME")
        .ok_or("cannot cache downloads: neither XDG_CACHE_HOME nor HOME is set")?;
    Ok(PathBuf::from(home).join(".cache").join("ewm"))
}

/// The cache directory a URL downloads into. Exposed so tests can clean
/// up after themselves without guessing the layout.
#[cfg(test)]
pub(crate) fn cache_dir_for(url: &str) -> Result<PathBuf, String> {
    Ok(cache_root()?.join(url_digest(url)))
}

/// The URL's cache directory name: our own SHA-1, hex — stable across
/// runs and filesystem-safe, where the URL is neither.
fn url_digest(url: &str) -> String {
    crate::ws::sha1(url.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The file name inside the cache directory: the URL's last path
/// segment, minus any query or fragment, with separators refused. The
/// digest directory already guarantees uniqueness, so this only has to
/// be a sane, recognizable name.
fn url_filename(url: &str) -> String {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    // Drop the scheme, then everything up to the first slash — the
    // authority is not a file name (a URL with no path at all must not
    // end up named after its host).
    let after_scheme = without_query
        .split_once("://")
        .map_or(without_query, |(_, rest)| rest);
    let path = after_scheme
        .split_once('/')
        .map_or("", |(_authority, path)| path)
        .trim_end_matches('/');
    let name = path.rsplit('/').next().unwrap_or("");
    let name = percent_decode(name);
    let safe: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0'))
        .collect();
    if safe.is_empty() || safe == "." || safe == ".." {
        "image".to_string()
    } else {
        safe
    }
}

/// Decode `%20`-style escapes so a cached file reads like its name.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The cache validators kept beside a download.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Meta {
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
    /// The URL, for a human browsing the cache.
    #[serde(default)]
    url: Option<String>,
}

/// What a conditional request came back with.
enum Fetched {
    /// The server confirmed the cached copy is current.
    NotModified,
    Body {
        bytes: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
}

/// Download `url` into `root`, reusing (and revalidating) a cached copy.
/// Returns the path to the local file.
fn cached_download(root: &Path, url: &str) -> Result<String, String> {
    let dir = root.join(url_digest(url));
    let file = dir.join(url_filename(url));
    let meta_path = dir.join("meta.json");
    let cached = file.is_file();
    let meta = if cached {
        std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Meta>(&text).ok())
            .unwrap_or_default()
    } else {
        Meta::default()
    };

    let name = || file.to_string_lossy().into_owned();
    match http_get(url, &meta) {
        Ok(Fetched::NotModified) if cached => Ok(name()),
        // A 304 with nothing cached would mean we sent validators we do
        // not have; treat it as a failed fetch rather than a phantom hit.
        Ok(Fetched::NotModified) => Err(format!("{url}: server said 304 with nothing cached")),
        Ok(Fetched::Body {
            bytes,
            etag,
            last_modified,
        }) => {
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            std::fs::write(&file, &bytes)
                .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
            let meta = Meta {
                etag,
                last_modified,
                url: Some(url.to_string()),
            };
            if let Ok(text) = serde_json::to_string_pretty(&meta) {
                let _ = std::fs::write(&meta_path, text);
            }
            eprintln!("[EWM] downloaded {url} ({} bytes)", bytes.len());
            Ok(name())
        }
        // Offline (or the server is down) with a copy in hand: use it.
        Err(e) if cached => {
            eprintln!("[EWM] {e}; using the cached copy of {url}");
            Ok(name())
        }
        Err(e) => Err(e),
    }
}

/// One conditional GET: sends whatever validators the cache holds and
/// reports either "still current" or the fresh body.
fn http_get(url: &str, meta: &Meta) -> Result<Fetched, String> {
    // Statuses are data here, not errors: a 304 is the good case.
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut request = agent.get(url);
    if let Some(etag) = &meta.etag {
        request = request.header("If-None-Match", etag);
    }
    if let Some(modified) = &meta.last_modified {
        request = request.header("If-Modified-Since", modified);
    }
    let mut response = request
        .call()
        .map_err(|e| format!("cannot fetch {url}: {e}"))?;

    let status = response.status().as_u16();
    if status == 304 {
        return Ok(Fetched::NotModified);
    }
    if !(200..300).contains(&status) {
        return Err(format!("cannot fetch {url}: HTTP {status}"));
    }
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let etag = header("etag");
    let last_modified = header("last-modified");
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_IMAGE_BYTES)
        .read_to_vec()
        .map_err(|e| format!("cannot read {url}: {e}"))?;
    Ok(Fetched::Body {
        bytes,
        etag,
        last_modified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    #[test]
    fn urls_are_recognized_and_paths_pass_through() {
        assert!(is_url("http://example.com/a.dsk"));
        assert!(is_url("https://example.com/a.dsk"));
        assert!(!is_url("/disks/a.dsk"));
        assert!(!is_url("builtin:WozMon"));
        assert!(!is_url("ftp://example.com/a.dsk"));
        // A local path is returned untouched, with no network involved.
        assert_eq!(local_path("/disks/a.dsk").unwrap(), "/disks/a.dsk");
    }

    #[test]
    fn cache_names_are_stable_and_filesystem_safe() {
        // The directory is the URL's digest: stable, and unaffected by
        // characters no filesystem would take.
        let a = url_digest("https://example.com/Total Replay.hdv");
        assert_eq!(a, url_digest("https://example.com/Total Replay.hdv"));
        assert_ne!(a, url_digest("https://example.com/other.hdv"));
        assert_eq!(a.len(), 40);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));

        // The file keeps a recognizable name.
        assert_eq!(
            url_filename("https://x.test/games/Frogger.dsk"),
            "Frogger.dsk"
        );
        assert_eq!(
            url_filename("https://x.test/Total%20Replay.hdv"),
            "Total Replay.hdv"
        );
        // Query strings and fragments are not part of the name.
        assert_eq!(url_filename("https://x.test/a.dsk?token=1#x"), "a.dsk");
        // Degenerate URLs still yield something safe.
        assert_eq!(url_filename("https://x.test/"), "image");
        assert_eq!(url_filename("https://x.test"), "image");
    }

    /// A one-shot HTTP server: answers `If-None-Match: <etag>` with 304,
    /// anything else with the body. Returns its port and a handle whose
    /// join yields how many requests carried a conditional header.
    fn serve(
        body: Vec<u8>,
        etag: &'static str,
        requests: usize,
    ) -> (u16, std::thread::JoinHandle<usize>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            let mut conditional = 0;
            for _ in 0..requests {
                let Ok((stream, _)) = listener.accept() else {
                    break;
                };
                let mut reader = BufReader::new(&stream);
                let mut matched = false;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if line.to_ascii_lowercase().starts_with("if-none-match:") {
                        conditional += 1;
                        matched = line.contains(etag);
                    }
                }
                let mut stream = &stream;
                if matched {
                    let _ =
                        stream.write_all(b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n");
                } else {
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: {etag}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&body);
                }
                let _ = stream.flush();
            }
            conditional
        });
        (port, handle)
    }

    #[test]
    fn downloads_once_then_revalidates_then_survives_offline() {
        let body = b"APPLE DISK IMAGE".to_vec();
        let (port, server) = serve(body.clone(), "\"v1\"", 2);
        let url = format!("http://127.0.0.1:{port}/games/Frogger.dsk");

        let root = std::env::temp_dir().join(format!("ewm-fetch-test-{port}"));
        let _ = std::fs::remove_dir_all(&root);

        // Cold cache: downloads, and the file keeps its name.
        let first = cached_download(&root, &url).expect("first fetch");
        assert!(first.ends_with("Frogger.dsk"), "{first}");
        assert_eq!(std::fs::read(&first).unwrap(), body);

        // Warm cache: revalidates with the stored ETag and reuses the
        // file (the server answers 304).
        let second = cached_download(&root, &url).expect("second fetch");
        assert_eq!(second, first);
        assert_eq!(std::fs::read(&second).unwrap(), body);
        assert_eq!(
            server.join().unwrap(),
            1,
            "second request was not conditional"
        );

        // Server gone: the cached copy still boots the machine.
        let offline = cached_download(&root, &url).expect("offline falls back to cache");
        assert_eq!(offline, first);
        assert_eq!(std::fs::read(&offline).unwrap(), body);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_cold_cache_with_no_server_is_an_error() {
        // Nothing cached and nothing answering: that must fail loudly
        // rather than hand back a path to a file that does not exist.
        let root = std::env::temp_dir().join("ewm-fetch-test-cold");
        let _ = std::fs::remove_dir_all(&root);
        // Port 1 on localhost: nothing listens there.
        let err = cached_download(&root, "http://127.0.0.1:1/nope.dsk").unwrap_err();
        assert!(err.contains("cannot fetch"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
