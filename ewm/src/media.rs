//! Classify a disk image dropped on (or opened with) the app: floppy images
//! go to slot 6 drive 1, ProDOS block images to the slot 7 hard drive. The
//! one ambiguous extension is `.po` — a 140K ProDOS-ordered floppy and a
//! ProDOS hard-drive volume share it — resolved by file size.

/// A 5.25" floppy is exactly 35 tracks x 16 sectors x 256 bytes.
const FLOPPY_SIZE: u64 = 143_360;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MediaKind {
    Floppy,
    HardDrive,
}

/// What kind of image is this path? `None` for files we don't recognize.
pub fn classify(path: &str) -> Option<MediaKind> {
    let ext = std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_lowercase();
    match ext.as_str() {
        "dsk" | "do" | "nib" | "woz" => Some(MediaKind::Floppy),
        "hdv" => Some(MediaKind::HardDrive),
        // .po is a floppy at exactly floppy size, a hard-drive volume when
        // larger. An unreadable path defaults to floppy; mounting will
        // report the real error.
        "po" => match std::fs::metadata(path) {
            Ok(meta) if meta.len() > FLOPPY_SIZE => Some(MediaKind::HardDrive),
            _ => Some(MediaKind::Floppy),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_classify() {
        for path in ["a.dsk", "b.DO", "c.nib", "d.WoZ"] {
            assert_eq!(classify(path), Some(MediaKind::Floppy), "{path}");
        }
        assert_eq!(classify("e.hdv"), Some(MediaKind::HardDrive));
        assert_eq!(classify("readme.txt"), None);
        assert_eq!(classify("no-extension"), None);
    }

    #[test]
    fn po_is_resolved_by_size() {
        let dir = std::env::temp_dir().join("ewm-media-test");
        std::fs::create_dir_all(&dir).unwrap();
        let floppy = dir.join("floppy.po");
        let volume = dir.join("volume.po");
        std::fs::write(&floppy, vec![0u8; FLOPPY_SIZE as usize]).unwrap();
        std::fs::write(&volume, vec![0u8; FLOPPY_SIZE as usize + 512]).unwrap();
        assert_eq!(
            classify(floppy.to_str().unwrap()),
            Some(MediaKind::Floppy),
            "140K .po is a floppy"
        );
        assert_eq!(
            classify(volume.to_str().unwrap()),
            Some(MediaKind::HardDrive),
            "a larger .po is a hard-drive volume"
        );
        // A missing .po defaults to floppy; the mount reports the error.
        assert_eq!(classify("missing.po"), Some(MediaKind::Floppy));
        std::fs::remove_dir_all(&dir).ok();
    }
}
