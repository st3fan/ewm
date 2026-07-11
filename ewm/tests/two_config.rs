//! The public `ewm::config` API, exercised from outside the crate on the
//! committed fixture configs (the same files the in-crate `--config` tests
//! use) — pins the contract that `load` parses, validates, and resolves
//! relative paths against the config file's directory.

use ewm::config::{self, AuxKind, MemoryKind, Model, SlotCard};
use ewm::two::TwoType;

/// A fixture path under ewm/tests/configs/.
macro_rules! fixture {
    ($name:literal) => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/configs/", $name)
    };
}

#[test]
fn minimal_config_loads() {
    let config = config::load(fixture!("minimal.json")).expect("minimal.json must load");
    assert_eq!(config.machine.model, Model::TwoPlus);
    assert_eq!(config.machine.model.two_type(), TwoType::Apple2Plus);
    assert!(config.machine.aux.is_none());
    assert!(config.machine.slots.is_empty());
    assert!(config.machine.memory.is_empty());
}

#[test]
fn full_config_loads_with_resolved_paths() {
    let config = config::load(fixture!("full.json")).expect("full.json must load");
    assert_eq!(config.machine.model, Model::TwoE);

    let aux = config.machine.aux.as_ref().expect("aux card");
    assert_eq!(aux.card, AuxKind::RamWorksIII);
    assert_eq!(aux.size.as_deref(), Some("1m"));

    let SlotCard::Diskii { drive1, drive2 } = &config.machine.slots["6"] else {
        panic!("slot 6 should be a diskii");
    };
    // Relative image paths come back anchored to the config's directory.
    assert_eq!(
        drive1.as_deref(),
        Some(fixture!("../../../disks/DOS33-SystemMaster.dsk"))
    );
    assert_eq!(
        drive2.as_deref(),
        Some(fixture!("../../../disks/DOS33-SamplePrograms.dsk"))
    );
    let SlotCard::Harddrive { image } = &config.machine.slots["7"] else {
        panic!("slot 7 should be a harddrive");
    };
    assert_eq!(image, fixture!("../../../disks/Total Replay v6.0.1.hdv"));
    assert!(std::fs::metadata(drive1.as_deref().unwrap()).is_ok());
    assert!(std::fs::metadata(image).is_ok());

    let region = &config.machine.memory[0];
    assert_eq!(region.kind, MemoryKind::Rom);
    assert_eq!(region.address_value(), Ok(0xd000));
    assert_eq!(region.path, fixture!("custom.bin"));

    assert_eq!(config.debug.trace.as_deref(), Some(fixture!("trace.txt")));
}

#[test]
fn missing_file_and_unsupported_layout_error() {
    let err = config::load(fixture!("does-not-exist.json")).unwrap_err();
    assert!(err.starts_with("cannot read config"), "{err}");

    let dir = std::env::temp_dir().join("ewm-config-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("slot5.json");
    std::fs::write(
        &path,
        r#"{"machine": {"model": "2plus", "slots": {"5": {"card": "diskii"}}}}"#,
    )
    .expect("write temp config");
    let err = config::load(path.to_str().unwrap()).unwrap_err();
    assert!(
        err.ends_with("slot 5 diskii: not supported yet (see notes/JSON_CONFIG.md Phase B)"),
        "{err}"
    );
}
