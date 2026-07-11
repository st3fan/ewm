//! The JSON machine configuration behind `ewm two --config file.json`.
//!
//! The serde types here mirror `schema/ewm-config.schema.json` — the schema
//! is *derived* from these structs by the `schema_matches_committed` test,
//! so the doc comments double as the schema's `description` fields.
//! `load()` parses, validates semantically, and resolves relative paths
//! against the config file's directory (the property that makes
//! `.ewmachine` bundles portable). See notes/JSON_CONFIG.md.

use std::collections::BTreeMap;
use std::path::Path;

use crate::scr::{MonitorStyle, Scanlines};
use crate::two::TwoType;

/// A complete EWM machine configuration, for `ewm two --config file.json`.
/// Only `machine` is required; every other setting defaults to what a bare
/// `ewm two` would do. Explicitly given command-line flags override the file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Optional reference to the JSON Schema, for editor validation and
    /// autocomplete.
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    /// The machine's physical build: model, aux card, slots, and any extra
    /// memory regions.
    pub machine: Machine,
    /// Monitor and rendering settings.
    #[serde(default)]
    pub display: Display,
    /// CPU speed and emulation-strictness settings.
    #[serde(default)]
    pub cpu: Cpu,
    /// Input-device preferences.
    #[serde(default)]
    pub input: Input,
    /// Boot behavior.
    #[serde(default)]
    pub boot: Boot,
    /// Debugging aids.
    #[serde(default)]
    pub debug: Debug,
}

/// The machine's physical build.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Machine {
    /// Which Apple II model to emulate.
    pub model: Model,
    /// The //e auxiliary-slot card. Only valid with `"model": "2e"`; when
    /// absent the //e gets the standard Extended 80-Column Text Card.
    pub aux: Option<Aux>,
    /// The card in each peripheral slot, keyed `"1"` through `"7"`. When
    /// the whole `slots` object is absent the machine gets the classic
    /// default layout (a Thunderclock in slot 1, a Disk II in slot 6); when
    /// present it is taken literally — an absent slot key means that slot
    /// is empty, and `"empty"` exists to say it explicitly.
    pub slots: Option<BTreeMap<String, SlotCard>>,
    /// Extra RAM or ROM regions loaded from files at startup.
    #[serde(default)]
    pub memory: Vec<MemoryRegion>,
}

/// Which Apple II model to emulate.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum Model {
    /// The Apple ][+.
    #[serde(rename = "2plus")]
    TwoPlus,
    /// The Apple //e.
    #[serde(rename = "2e")]
    TwoE,
}

impl Model {
    pub fn two_type(self) -> TwoType {
        match self {
            Model::TwoPlus => TwoType::Apple2Plus,
            Model::TwoE => TwoType::Apple2E,
        }
    }
}

/// The //e auxiliary-slot card.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Aux {
    /// Which auxiliary card is installed.
    pub card: AuxKind,
    /// Memory size for the RamWorks III, e.g. `"256k"` or `"1m"` — a
    /// multiple of 64K up to 8M. Only valid with `"card": "ramworksiii"`
    /// (which defaults to the full 8M when the size is omitted).
    pub size: Option<String>,
}

/// The auxiliary-card types.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum AuxKind {
    /// The 1K Apple 80-Column Text Card.
    #[serde(rename = "80col")]
    Col80,
    /// The Extended 80-Column Text Card (64K) — the default card.
    #[serde(rename = "ext80col")]
    Ext80Col,
    /// The Applied Engineering RamWorks III (64K–8M, see `size`).
    #[serde(rename = "ramworksiii")]
    RamWorksIII,
}

impl AuxKind {
    /// The card's `--aux` flag token, so config and CLI share one
    /// construction path (`crate::aux::parse`).
    pub fn flag_token(self) -> &'static str {
        match self {
            AuxKind::Col80 => "80col",
            AuxKind::Ext80Col => "ext80col",
            AuxKind::RamWorksIII => "ramworksiii",
        }
    }
}

/// A peripheral card, discriminated by `"card"`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "card", rename_all = "lowercase", deny_unknown_fields)]
pub enum SlotCard {
    /// A Disk ][ controller with up to two drives.
    Diskii {
        /// Floppy image for drive 1 (`.dsk`, `.do`, `.po`, `.nib`, `.woz`).
        drive1: Option<String>,
        /// Floppy image for drive 2.
        drive2: Option<String>,
    },
    /// A ProDOS-compatible hard-drive controller.
    Harddrive {
        /// Block image (`.hdv`, `.po`).
        image: String,
    },
    /// A Thunderclock Plus real-time clock.
    Thunderclock,
    /// Explicitly nothing in this slot.
    Empty,
}

impl SlotCard {
    /// The `"card"` discriminator value, for error messages.
    pub fn card_name(&self) -> &'static str {
        match self {
            SlotCard::Diskii { .. } => "diskii",
            SlotCard::Harddrive { .. } => "harddrive",
            SlotCard::Thunderclock => "thunderclock",
            SlotCard::Empty => "empty",
        }
    }
}

/// An extra RAM or ROM region loaded from a file at startup (the config
/// equivalent of the `--memory` flag).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MemoryRegion {
    /// Whether the region is writable RAM or read-only ROM.
    #[serde(rename = "type")]
    pub kind: MemoryKind,
    /// Load address, hex (`"0xd000"`) or decimal (`"53248"`).
    pub address: String,
    /// File whose contents fill the region.
    pub path: String,
}

/// Whether a memory region is RAM or ROM.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    /// Writable memory.
    Ram,
    /// Read-only memory.
    Rom,
}

impl MemoryRegion {
    /// The `address` string as a 16-bit value; accepts `0x`-prefixed hex
    /// and plain decimal.
    pub fn address_value(&self) -> Result<u16, String> {
        let s = &self.address;
        let parsed = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            u32::from_str_radix(hex, 16).ok()
        } else {
            s.parse::<u32>().ok()
        };
        parsed
            .and_then(|v| u16::try_from(v).ok())
            .ok_or_else(|| format!("bad address {s:?} (expected 0x0000-0xffff)"))
    }
}

/// Monitor and rendering settings.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Display {
    /// Monitor style: a monochrome phosphor or an RGB color monitor.
    pub monitor: Option<Monitor>,
    /// Scanline darkening between the doubled scanlines.
    pub scanlines: Option<ScanlinesSetting>,
    /// Display refresh rate in frames per second.
    pub fps: Option<u32>,
}

/// Monitor styles.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Monitor {
    /// Green phosphor.
    Green,
    /// Amber phosphor.
    Amber,
    /// White phosphor.
    White,
    /// An RGB color monitor.
    Rgb,
}

impl Monitor {
    pub fn style(self) -> MonitorStyle {
        match self {
            Monitor::Green => MonitorStyle::Green,
            Monitor::Amber => MonitorStyle::Amber,
            Monitor::White => MonitorStyle::White,
            Monitor::Rgb => MonitorStyle::Rgb,
        }
    }
}

/// Scanline darkening levels.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ScanlinesSetting {
    /// No scanline effect.
    Off,
    /// Slight darkening.
    Light,
    /// Strong darkening.
    Heavy,
}

impl ScanlinesSetting {
    pub fn scanlines(self) -> Scanlines {
        match self {
            ScanlinesSetting::Off => Scanlines::Off,
            ScanlinesSetting::Light => Scanlines::Light,
            ScanlinesSetting::Heavy => Scanlines::Heavy,
        }
    }
}

/// CPU speed and emulation-strictness settings.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Cpu {
    /// Emulated CPU speed — the classic accelerator steps.
    pub speed: Option<CpuSpeed>,
    /// Treat unimplemented opcodes as fatal.
    pub strict: Option<bool>,
}

/// The classic accelerator speed steps.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum CpuSpeed {
    /// 1.023 MHz — a stock machine.
    #[serde(rename = "normal")]
    Normal,
    /// 3.58 MHz.
    #[serde(rename = "3.58mhz")]
    Fast,
    /// 7.16 MHz.
    #[serde(rename = "7.16mhz")]
    Faster,
}

/// Input-device preferences.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Preferred game controller, by the exact name the Command Palette
    /// lists. Hot-plug still applies when absent or unmatched.
    pub controller: Option<String>,
}

/// Boot behavior.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Boot {
    /// Seconds to hold the machine before it starts executing (the window
    /// is up and rendering) — for debugging and video recording.
    pub delay: Option<f64>,
}

/// Debugging aids.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Debug {
    /// Write a CPU trace to this file.
    pub trace: Option<String>,
    /// Enable the debug overlay.
    pub enabled: Option<bool>,
}

/// Load a machine configuration: read the file, parse it, validate it
/// semantically, and resolve relative paths against the file's directory.
pub fn load(path: &str) -> Result<Config, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read config {path}: {e}"))?;
    let base = Path::new(path).parent().unwrap_or(Path::new("."));
    from_str_resolved(&text, path, base)
}

/// Load a config file as a JSON document: the full typed path (parse,
/// semantic validation, relative paths resolved against the file's
/// directory), then back to JSON — ready to layer with other sources
/// (`merge_documents`, `apply_set`) before the final `from_document`.
pub fn load_document(path: &str) -> Result<serde_json::Value, String> {
    let config = load(path)?;
    serde_json::to_value(config).map_err(|e| format!("{path}: {e}"))
}

/// Deep-merge `overlay` into `doc`, the layering rule for config sources
/// (later `--config` files and `--set` overrides win, key by key):
///
/// - objects merge recursively;
/// - a `null` overlay value is a no-op — a source that doesn't set a field
///   must not clear it (`Option` fields serialize to `null`);
/// - an *empty array* overlay is likewise a no-op (`machine.memory`
///   serializes to `[]` when a file has no regions);
/// - two objects whose `"card"` discriminators differ replace wholesale —
///   merging a diskii's drives into an `"empty"` card would fail
///   validation;
/// - everything else replaces.
pub fn merge_documents(doc: &mut serde_json::Value, overlay: serde_json::Value) {
    use serde_json::Value;
    // A source that doesn't set a field must not clear it: None fields
    // serialize to null, an empty machine.memory to [].
    fn is_noop(value: &Value) -> bool {
        value.is_null() || matches!(value, Value::Array(entries) if entries.is_empty())
    }
    if is_noop(&overlay) {
        return;
    }
    if let (Value::Object(base), Value::Object(overlay_map)) = (&mut *doc, &overlay) {
        let card_differs = matches!(
            (base.get("card"), overlay_map.get("card")),
            (Some(a), Some(b)) if a != b
        );
        if !card_differs {
            let Value::Object(overlay_map) = overlay else {
                unreachable!("matched as an object above");
            };
            for (key, value) in overlay_map {
                match base.get_mut(&key) {
                    Some(slot) => merge_documents(slot, value),
                    None if is_noop(&value) => {}
                    None => {
                        base.insert(key, value);
                    }
                }
            }
            return;
        }
    }
    *doc = overlay;
}

/// The JSON form of the default slot table (`two::default_slots()` is the
/// machine-level equivalent), materialized when a `--set` override enters
/// `machine:slots` on a document that has none — so overrides extend the
/// default machine instead of accidentally creating a literal one-slot
/// table.
fn default_slots_value() -> serde_json::Value {
    serde_json::json!({
        "1": { "card": "thunderclock" },
        "6": { "card": "diskii" },
    })
}

/// Apply one `--set <key>=<value>` override to the document. The key path
/// is colon-separated (`machine:slots:6:drive1`); the value is parsed as
/// JSON when it *is* valid JSON — numbers, booleans, quoted strings, whole
/// objects like `machine:slots:7={"card":"harddrive","image":"x.hdv"}` —
/// and taken as a plain string otherwise.
pub fn apply_set(doc: &mut serde_json::Value, expr: &str) -> Result<(), String> {
    use serde_json::Value;
    let Some((key, value)) = expr.split_once('=') else {
        return Err(format!("--set {expr}: expected <key>=<value>"));
    };
    let segments: Vec<&str> = key.split(':').collect();
    if segments.iter().any(|s| s.is_empty()) {
        return Err(format!("--set {expr}: empty segment in key {key:?}"));
    }

    // Entering machine:slots on a document without one would create a
    // literal (near-empty) table; materialize the default layout first so
    // overrides extend the default machine.
    if segments.first() == Some(&"machine") && segments.get(1) == Some(&"slots") {
        let machine = doc
            .as_object_mut()
            .ok_or_else(|| format!("--set {expr}: the config document is not an object"))?
            .entry("machine")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(machine) = machine.as_object_mut()
            && machine.get("slots").is_none_or(|s| s.is_null())
        {
            machine.insert("slots".to_string(), default_slots_value());
        }
    }

    let mut parsed = Some(
        serde_json::from_str::<Value>(value).unwrap_or_else(|_| Value::String(value.to_string())),
    );
    let mut node = &mut *doc;
    for (i, segment) in segments.iter().enumerate() {
        let map = match node {
            Value::Array(_) => {
                return Err(format!(
                    "--set {expr}: cannot index into the {:?} array (memory regions come from --memory)",
                    segments[..i].join(":")
                ));
            }
            Value::Object(map) => map,
            _ => {
                return Err(format!(
                    "--set {expr}: {:?} is not an object",
                    segments[..i].join(":")
                ));
            }
        };
        if i == segments.len() - 1 {
            let value = parsed.take().expect("value used once");
            // Changing a "card" discriminator invalidates the object's other
            // fields (a harddrive has no drive1) — reset the object to just
            // the new card, mirroring merge_documents' replace rule.
            if *segment == "card" && map.get("card").is_some_and(|card| *card != value) {
                map.clear();
            }
            map.insert(segment.to_string(), value);
            return Ok(());
        }
        node = map
            .entry(segment.to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
    }
    // The key has at least one non-empty segment, so the loop always
    // returns from its last iteration.
    Ok(())
}

/// Convert a layered config document (files + `--set` overrides) into a
/// validated `Config`. No path resolution happens here: file-sourced paths
/// were resolved per file by `load_document`, and `--set` values stay as
/// given (relative to the working directory, like the flags they replace).
pub fn from_document(doc: serde_json::Value) -> Result<Config, String> {
    let config: Config = serde_json::from_value(doc).map_err(|e| format!("config: {e}"))?;
    validate(&config).map_err(|e| format!("config: {e}"))?;
    Ok(config)
}

/// The testable core of `load`: `origin` names the file in error messages,
/// `base` is the directory relative paths resolve against.
fn from_str_resolved(text: &str, origin: &str, base: &Path) -> Result<Config, String> {
    let mut config: Config = serde_json::from_str(text).map_err(|e| format!("{origin}: {e}"))?;
    validate(&config).map_err(|e| format!("{origin}: {e}"))?;
    resolve_paths(&mut config, base);
    Ok(config)
}

/// Semantic validation beyond what serde's typed parse enforces.
fn validate(config: &Config) -> Result<(), String> {
    if let Some(aux) = &config.machine.aux {
        if config.machine.model != Model::TwoE {
            return Err("machine.aux: aux cards are a //e feature (model is \"2plus\")".into());
        }
        if let Some(size) = &aux.size {
            if aux.card != AuxKind::RamWorksIII {
                return Err("machine.aux.size: only valid with the \"ramworksiii\" card".into());
            }
            crate::aux::parse_size(size).map_err(|e| format!("machine.aux.size: {e}"))?;
        }
    }
    if let Some(slots) = &config.machine.slots {
        for key in slots.keys() {
            if !matches!(key.as_str(), "1" | "2" | "3" | "4" | "5" | "6" | "7") {
                return Err(format!(
                    "machine.slots: no such slot {key:?} (slots are \"1\" through \"7\")"
                ));
            }
        }
        // Any card can go in any slot; the multiplicity limits are the
        // classic three-controller maximum and the single clock driver
        // ProDOS installs.
        let count = |wanted: &str| {
            slots
                .values()
                .filter(|card| card.card_name() == wanted)
                .count()
        };
        if count("diskii") > 3 {
            return Err("machine.slots: at most three Disk ][ controllers".into());
        }
        if count("thunderclock") > 1 {
            return Err("machine.slots: at most one Thunderclock".into());
        }
    }
    for (i, region) in config.machine.memory.iter().enumerate() {
        region
            .address_value()
            .map_err(|e| format!("machine.memory[{i}].address: {e}"))?;
    }
    if config.display.fps == Some(0) {
        return Err("display.fps: must be at least 1".into());
    }
    if let Some(delay) = config.boot.delay
        && delay < 0.0
    {
        return Err("boot.delay: must be >= 0".into());
    }
    Ok(())
}

/// Rewrite every relative path-valued field to be relative to `base` — the
/// config file's directory — so a config works regardless of the CWD.
fn resolve_paths(config: &mut Config, base: &Path) {
    for card in config.machine.slots.iter_mut().flat_map(|s| s.values_mut()) {
        match card {
            SlotCard::Diskii { drive1, drive2 } => {
                if let Some(p) = drive1 {
                    resolve(base, p);
                }
                if let Some(p) = drive2 {
                    resolve(base, p);
                }
            }
            SlotCard::Harddrive { image } => resolve(base, image),
            SlotCard::Thunderclock | SlotCard::Empty => {}
        }
    }
    for region in &mut config.machine.memory {
        resolve(base, &mut region.path);
    }
    if let Some(p) = &mut config.debug.trace {
        resolve(base, p);
    }
}

fn resolve(base: &Path, p: &mut String) {
    if Path::new(p).is_relative() {
        *p = base.join(&*p).to_string_lossy().into_owned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse inline JSON as if it came from `/cfg/test.json`.
    fn parse(text: &str) -> Result<Config, String> {
        from_str_resolved(text, "test.json", Path::new("/cfg"))
    }

    /// The committed schema/ewm-config.schema.json is derived from these
    /// structs — this test keeps the two in lockstep, byte for byte.
    /// Regenerate with:
    ///
    ///   EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed
    #[test]
    fn schema_matches_committed() {
        let schema = schemars::generate::SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<Config>();
        let mut generated = serde_json::to_string_pretty(&schema).expect("schema must serialize");
        generated.push('\n');

        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../schema/ewm-config.schema.json"
        );
        if std::env::var_os("EWM_UPDATE_SCHEMA").is_some() {
            std::fs::write(path, &generated).expect("cannot write the schema");
            return;
        }
        let committed = std::fs::read_to_string(path).expect(
            "cannot read schema/ewm-config.schema.json — regenerate with \
             EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed",
        );
        assert_eq!(
            committed, generated,
            "schema drift — regenerate with \
             EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed"
        );
    }

    #[test]
    fn minimal_config_parses() {
        let config = parse(r#"{"machine": {"model": "2plus"}}"#).expect("minimal config");
        assert_eq!(config.machine.model, Model::TwoPlus);
        assert!(config.machine.aux.is_none());
        assert!(config.machine.slots.is_none());
        assert!(config.machine.memory.is_empty());
        assert_eq!(config.display, Display::default());
        assert_eq!(config.cpu, Cpu::default());
        assert_eq!(config.input, Input::default());
        assert_eq!(config.boot, Boot::default());
        assert_eq!(config.debug, Debug::default());
    }

    #[test]
    fn unknown_top_level_key_is_rejected() {
        let err = parse(r#"{"machine": {"model": "2plus"}, "monitr": {}}"#).unwrap_err();
        assert!(err.contains("unknown field `monitr`"), "{err}");
        assert!(err.starts_with("test.json:"), "{err}");
    }

    #[test]
    fn unknown_slot_card_key_is_rejected() {
        // The canary for serde's internally-tagged deny_unknown_fields
        // behavior (see notes/JSON_CONFIG.md).
        let err = parse(
            r#"{"machine": {"model": "2plus",
                "slots": {"6": {"card": "diskii", "driv1": "x.dsk"}}}}"#,
        )
        .unwrap_err();
        assert!(err.contains("driv1"), "{err}");
    }

    #[test]
    fn bad_values_are_rejected_with_expected_lists() {
        let err = parse(r#"{"machine": {"model": "2gs"}}"#).unwrap_err();
        assert!(err.contains("2plus") && err.contains("2e"), "{err}");

        let err = parse(r#"{"machine": {"model": "2plus"}, "display": {"monitor": "blue"}}"#)
            .unwrap_err();
        assert!(err.contains("green") && err.contains("rgb"), "{err}");

        let err =
            parse(r#"{"machine": {"model": "2plus"}, "cpu": {"speed": "2mhz"}}"#).unwrap_err();
        assert!(err.contains("normal") && err.contains("3.58mhz"), "{err}");
    }

    #[test]
    fn slot_rules() {
        let slot = |n: &str, card: &str| {
            parse(&format!(
                r#"{{"machine": {{"model": "2plus", "slots": {{"{n}": {card}}}}}}}"#
            ))
        };

        // Any card in any slot (Phase B): the Phase A layout gate is gone.
        assert!(slot("5", r#"{"card": "diskii", "drive1": "x.dsk"}"#).is_ok());
        assert!(slot("6", r#"{"card": "harddrive", "image": "x.hdv"}"#).is_ok());
        assert!(slot("1", r#"{"card": "empty"}"#).is_ok());
        assert!(slot("2", r#"{"card": "thunderclock"}"#).is_ok());
        assert!(slot("1", r#"{"card": "thunderclock"}"#).is_ok());
        assert!(slot("6", r#"{"card": "diskii"}"#).is_ok());
        assert!(slot("7", r#"{"card": "harddrive", "image": "x.hdv"}"#).is_ok());
        assert!(slot("7", r#"{"card": "empty"}"#).is_ok());
        assert!(slot("3", r#"{"card": "empty"}"#).is_ok());

        // Slot keys stay range-checked.
        let err = slot("8", r#"{"card": "empty"}"#).unwrap_err();
        assert!(err.contains(r#"no such slot "8""#), "{err}");
        let err = slot("01", r#"{"card": "thunderclock"}"#).unwrap_err();
        assert!(err.contains(r#"no such slot "01""#), "{err}");

        // Multiplicity: at most three Disk ][ controllers, one Thunderclock.
        let err = parse(
            r#"{"machine": {"model": "2plus", "slots": {
                "3": {"card": "diskii"}, "4": {"card": "diskii"},
                "5": {"card": "diskii"}, "6": {"card": "diskii"}}}}"#,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "test.json: machine.slots: at most three Disk ][ controllers"
        );
        let err = parse(
            r#"{"machine": {"model": "2plus", "slots": {
                "1": {"card": "thunderclock"}, "2": {"card": "thunderclock"}}}}"#,
        )
        .unwrap_err();
        assert_eq!(err, "test.json: machine.slots: at most one Thunderclock");

        // Three controllers and two hard drives are fine.
        assert!(
            parse(
                r#"{"machine": {"model": "2plus", "slots": {
                    "4": {"card": "diskii"}, "5": {"card": "diskii"},
                    "6": {"card": "diskii"}, "2": {"card": "harddrive", "image": "a.hdv"},
                    "7": {"card": "harddrive", "image": "b.hdv"}}}}"#,
            )
            .is_ok()
        );

        // A present-but-empty table is a bare machine, distinct from an
        // absent one (the default layout).
        let config = parse(r#"{"machine": {"model": "2plus", "slots": {}}}"#).expect("empty");
        assert_eq!(config.machine.slots, Some(BTreeMap::new()));
    }

    #[test]
    fn aux_rules() {
        let aux = |model: &str, aux: &str| {
            parse(&format!(
                r#"{{"machine": {{"model": "{model}", "aux": {aux}}}}}"#
            ))
        };

        let err = aux("2e", r#"{"card": "80col", "size": "1m"}"#).unwrap_err();
        assert!(err.contains("only valid with"), "{err}");
        let err = aux("2plus", r#"{"card": "80col"}"#).unwrap_err();
        assert!(err.contains("//e feature"), "{err}");
        let err = aux("2e", r#"{"card": "ramworksiii", "size": "3k"}"#).unwrap_err();
        assert!(err.contains("multiple of 64k"), "{err}");

        let config = aux("2e", r#"{"card": "ramworksiii", "size": "1m"}"#).expect("valid aux");
        let aux = config.machine.aux.expect("aux present");
        assert_eq!(aux.card, AuxKind::RamWorksIII);
        assert_eq!(aux.size.as_deref(), Some("1m"));
    }

    #[test]
    fn relative_paths_resolve_against_the_config_dir() {
        let config = parse(
            r#"{"machine": {"model": "2plus",
                "slots": {
                    "6": {"card": "diskii", "drive1": "disks/a.dsk", "drive2": "/abs/b.dsk"},
                    "7": {"card": "harddrive", "image": "hd.hdv"}},
                "memory": [{"type": "rom", "address": "0xd000", "path": "roms/x.bin"}]},
                "debug": {"trace": "trace.txt"}}"#,
        )
        .expect("valid config");
        let slots = config.machine.slots.as_ref().expect("slots present");
        let SlotCard::Diskii { drive1, drive2 } = &slots["6"] else {
            panic!("slot 6 should be a diskii");
        };
        assert_eq!(drive1.as_deref(), Some("/cfg/disks/a.dsk"));
        assert_eq!(drive2.as_deref(), Some("/abs/b.dsk"));
        let SlotCard::Harddrive { image } = &slots["7"] else {
            panic!("slot 7 should be a harddrive");
        };
        assert_eq!(image, "/cfg/hd.hdv");
        assert_eq!(config.machine.memory[0].path, "/cfg/roms/x.bin");
        assert_eq!(config.debug.trace.as_deref(), Some("/cfg/trace.txt"));
    }

    #[test]
    fn memory_addresses_accept_hex_and_decimal() {
        let region = |address: &str| MemoryRegion {
            kind: MemoryKind::Rom,
            address: address.to_string(),
            path: "x.bin".to_string(),
        };
        assert_eq!(region("0xd000").address_value(), Ok(0xd000));
        assert_eq!(region("53248").address_value(), Ok(0xd000));
        assert!(region("0x10000").address_value().is_err());
        assert!(region("d000").address_value().is_err());
        assert!(region("").address_value().is_err());
    }

    #[test]
    fn merge_layers_objects_and_skips_nulls() {
        let mut doc = serde_json::json!({
            "machine": {"model": "2plus", "slots": {"6": {"card": "diskii", "drive1": "a.dsk"}}},
            "display": {"monitor": "green"},
        });
        merge_documents(
            &mut doc,
            serde_json::json!({
                "machine": {"model": "2e", "slots": {"6": {"drive2": "b.dsk"}}, "aux": null, "memory": []},
                "display": {"monitor": null, "fps": 30},
            }),
        );
        assert_eq!(
            doc,
            serde_json::json!({
                "machine": {"model": "2e", "slots": {"6": {"card": "diskii", "drive1": "a.dsk", "drive2": "b.dsk"}}},
                "display": {"monitor": "green", "fps": 30},
            })
        );
    }

    #[test]
    fn merge_replaces_an_object_whose_card_changes() {
        let mut doc = serde_json::json!({"card": "diskii", "drive1": "a.dsk"});
        merge_documents(&mut doc, serde_json::json!({"card": "empty"}));
        assert_eq!(doc, serde_json::json!({"card": "empty"}));
    }

    #[test]
    fn set_types_values_as_json_or_string() {
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        apply_set(&mut doc, "display:fps=30").unwrap();
        apply_set(&mut doc, "cpu:strict=true").unwrap();
        apply_set(&mut doc, "display:monitor=amber").unwrap();
        apply_set(&mut doc, r#"input:controller="8BitDo Pro 2""#).unwrap();
        assert_eq!(doc["display"]["fps"], serde_json::json!(30));
        assert_eq!(doc["cpu"]["strict"], serde_json::json!(true));
        assert_eq!(doc["display"]["monitor"], serde_json::json!("amber"));
        assert_eq!(
            doc["input"]["controller"],
            serde_json::json!("8BitDo Pro 2")
        );
    }

    #[test]
    fn set_materializes_the_default_slots_once() {
        // Entering machine:slots on a slotless document brings in the
        // default table, so the override extends the default machine.
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        apply_set(&mut doc, "machine:slots:6:drive1=x.dsk").unwrap();
        assert_eq!(
            doc["machine"]["slots"],
            serde_json::json!({
                "1": {"card": "thunderclock"},
                "6": {"card": "diskii", "drive1": "x.dsk"},
            })
        );
        // A document that already has a slots table is taken literally.
        let mut doc = serde_json::json!({"machine": {"model": "2plus", "slots": {}}});
        apply_set(&mut doc, "machine:slots:6:drive1=x.dsk").unwrap();
        assert_eq!(
            doc["machine"]["slots"],
            serde_json::json!({"6": {"drive1": "x.dsk"}})
        );
    }

    #[test]
    fn set_replaces_a_slot_whose_card_changes() {
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        apply_set(&mut doc, "machine:slots:6:drive1=x.dsk").unwrap();
        apply_set(&mut doc, "machine:slots:6:card=harddrive").unwrap();
        apply_set(&mut doc, "machine:slots:6:image=x.hdv").unwrap();
        assert_eq!(
            doc["machine"]["slots"]["6"],
            serde_json::json!({"card": "harddrive", "image": "x.hdv"})
        );
        // A whole-object value replaces the slot in one go.
        apply_set(
            &mut doc,
            r#"machine:slots:7={"card":"harddrive","image":"y.hdv"}"#,
        )
        .unwrap();
        assert_eq!(
            doc["machine"]["slots"]["7"],
            serde_json::json!({"card": "harddrive", "image": "y.hdv"})
        );
    }

    #[test]
    fn set_rejects_bad_expressions() {
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        let err = apply_set(&mut doc, "display:monitor").unwrap_err();
        assert!(err.contains("expected <key>=<value>"), "{err}");
        let err = apply_set(&mut doc, "display::monitor=amber").unwrap_err();
        assert!(err.contains("empty segment"), "{err}");
        let err = apply_set(&mut doc, "machine:model:x=1").unwrap_err();
        assert!(err.contains(r#""machine:model" is not an object"#), "{err}");
        doc["machine"]["memory"] = serde_json::json!([{"type": "rom"}]);
        let err = apply_set(&mut doc, "machine:memory:0:path=x.bin").unwrap_err();
        assert!(err.contains("cannot index into"), "{err}");
    }

    #[test]
    fn from_document_validates_and_names_unknown_fields() {
        let doc = serde_json::json!({"machine": {"model": "2plus"}, "disply": {}});
        let err = from_document(doc).unwrap_err();
        assert!(err.starts_with("config:"), "{err}");
        assert!(err.contains("unknown field `disply`"), "{err}");

        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        apply_set(&mut doc, "display:fps=0").unwrap();
        let err = from_document(doc).unwrap_err();
        assert!(err.contains("display.fps"), "{err}");
    }

    #[test]
    fn documents_round_trip_through_serialize() {
        // load_document(file) == what from_document accepts: the resolved
        // typed config survives the Value round trip intact.
        let doc = load_document(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/configs/full.json"
        ))
        .expect("full.json must load");
        let via_document = from_document(doc).expect("round trip");
        let direct = load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/configs/full.json"
        ))
        .expect("direct load");
        assert_eq!(via_document, direct);
    }
}
