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
    let machine = config.machine.as_ref().expect("machine section");
    assert_eq!(machine.model, Some(Model::TwoPlus));
    assert_eq!(machine.model.unwrap().two_type(), Some(TwoType::Apple2Plus));
    assert!(machine.aux.is_none());
    assert!(machine.slots.is_none());
    assert!(machine.memory.is_empty());
}

#[test]
fn full_config_loads_with_resolved_paths() {
    let config = config::load(fixture!("full.json")).expect("full.json must load");
    let machine = config.machine.as_ref().expect("machine section");
    assert_eq!(machine.model, Some(Model::TwoE));

    let aux = machine.aux.as_ref().expect("aux card");
    assert_eq!(aux.card, AuxKind::RamWorksIII);
    assert_eq!(aux.size.as_deref(), Some("1m"));

    let slots = machine.slots.as_ref().expect("slots present");
    let SlotCard::Diskii { drive1, drive2 } = &slots["6"] else {
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
    let SlotCard::Harddrive { image } = &slots["7"] else {
        panic!("slot 7 should be a harddrive");
    };
    assert_eq!(image, fixture!("../../../disks/ProDOS_2_4_3.po"));
    assert!(std::fs::metadata(drive1.as_deref().unwrap()).is_ok());
    assert!(std::fs::metadata(image).is_ok());

    let region = &machine.memory[0];
    assert_eq!(region.kind, MemoryKind::Rom);
    assert_eq!(region.address_value(), Ok(0xd000));
    assert_eq!(region.path.as_deref(), Some(fixture!("custom.bin")));

    assert_eq!(config.debug.trace.as_deref(), Some(fixture!("trace.txt")));
}

#[test]
fn two_controller_config_loads() {
    // Phase B: arbitrary slot layouts load — two Disk ][ controllers here.
    let config = config::load(fixture!("two-controllers.json")).expect("must load");
    let machine = config.machine.as_ref().expect("machine section");
    let slots = machine.slots.as_ref().expect("slots present");
    assert_eq!(slots.len(), 3);
    assert!(matches!(slots["5"], SlotCard::Diskii { .. }));
    assert!(matches!(slots["6"], SlotCard::Diskii { .. }));
}

#[test]
fn builtin_configs_load_and_match_the_committed_files() {
    // The `builtin:` source scheme resolves against the embedded copies of
    // the configs/ files; both spellings must describe the same machine.
    for name in ["apple1", "apple2", "apple2e", "apple2plus", "replica1"] {
        let config = config::load_builtin(name).expect("builtin loads");
        assert!(config.description.is_some(), "builtin:{name}");

        let builtin =
            config::load_source_document(&format!("builtin:{name}")).expect("builtin source loads");
        let committed = format!("{}/../configs/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let file = config::load_document(&committed).expect("committed file loads");
        assert_eq!(builtin, file, "builtin:{name} != {committed}");
    }

    let err = config::load_builtin("apple2gs").unwrap_err();
    assert_eq!(
        err,
        r#"no built-in config "apple2gs" (available: apple1, apple2, apple2e, apple2plus, replica1)"#
    );
}

#[test]
fn partial_fragment_loads_as_document_but_not_as_config() {
    // C2: a partial fragment (here: slots only) loads through the
    // document path — the shape overlays use — but the complete-config
    // path (`load`, behind --config) rejects it per file.
    let dir = std::env::temp_dir().join("ewm-config-partial-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("hdd7.json");
    std::fs::write(
        &path,
        r#"{"machine": {"slots": {"7": {"card": "harddrive", "image": "tr.hdv"}}}}"#,
    )
    .expect("write temp config");

    let doc = config::load_document(path.to_str().unwrap()).expect("fragment loads as document");
    assert!(doc["machine"]["model"].is_null());

    let err = config::load(path.to_str().unwrap()).unwrap_err();
    assert!(
        err.ends_with("machine.model is required (is this an overlay? use --config-overlay)"),
        "{err}"
    );
}

#[test]
fn missing_file_and_bad_multiplicity_error() {
    let err = config::load(fixture!("does-not-exist.json")).unwrap_err();
    assert!(err.starts_with("cannot read config"), "{err}");

    let dir = std::env::temp_dir().join("ewm-config-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("four-controllers.json");
    std::fs::write(
        &path,
        r#"{"machine": {"model": "apple2plus", "slots": {
            "3": {"card": "diskii"}, "4": {"card": "diskii"},
            "5": {"card": "diskii"}, "6": {"card": "diskii"}}}}"#,
    )
    .expect("write temp config");
    let err = config::load(path.to_str().unwrap()).unwrap_err();
    assert!(
        err.ends_with("machine.slots: at most three Disk ][ controllers"),
        "{err}"
    );
}
