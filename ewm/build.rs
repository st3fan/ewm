//! Embed the vendored noVNC console (`novnc/`, see novnc/README.md) into the
//! binary: walk the directory and generate a `(path, bytes)` table of
//! `include_bytes!` entries for `web.rs` to serve. Keeps the "single
//! self-contained binary" property (notes/REMOTE.md §1) without adding an
//! embed crate.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("novnc");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");

    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();

    let mut code = String::from(
        "/// The embedded noVNC console assets: URL path (no leading slash) to contents.\n\
         pub static NOVNC_ASSETS: &[(&str, &[u8])] = &[\n",
    );
    for (key, path) in &files {
        println!("cargo:rerun-if-changed={path}");
        writeln!(code, "    ({key:?}, include_bytes!({path:?})),").expect("write table row");
    }
    code.push_str("];\n");

    std::fs::write(Path::new(&out).join("novnc_assets.rs"), code).expect("write asset table");
    println!("cargo:rerun-if-changed={}", root.display());
}

/// Collect every file under `dir` as `(key, absolute path)`, where the key is
/// the /-separated path relative to `root`.
fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("read novnc dir") {
        let path = entry.expect("dir entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, files);
        } else {
            let key = path
                .strip_prefix(root)
                .expect("under root")
                .components()
                .map(|c| c.as_os_str().to_str().expect("utf-8 path"))
                .collect::<Vec<_>>()
                .join("/");
            files.push((key, path.to_str().expect("utf-8 path").to_string()));
        }
    }
}
