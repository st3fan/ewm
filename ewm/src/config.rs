//! The JSON machine configuration behind `ewm two --config file.json`.
//!
//! The serde types here mirror `schema/ewm-config.schema.json` — the schema
//! is *derived* from these structs by the `schema_matches_committed` test,
//! so the doc comments double as the schema's `description` fields.
//! `load()` parses, validates semantically, and resolves relative paths
//! against the config file's directory (the property that makes
//! `.ewmachine` bundles portable). See notes/JSON_CONFIG.md.
//!
//! The types parse arbitrarily *partial* fragments (`machine` and
//! `machine.model` are `Option`), because overlays layer partial documents
//! onto a base config (plans/20260718-02-config-sources.md C2). Validation
//! splits accordingly: `validate` judges what a lone fragment can be judged
//! on (structure), `validate_complete` judges what only the final layered
//! document can (the model is present, and the model-dependent
//! cross-checks). `load` and `--config` still require completeness per
//! file; `load_document` does not.

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
    /// A one-line human description of this configuration, shown by
    /// `--config builtin:list`.
    #[serde(default)]
    pub description: Option<String>,
    /// The machine's physical build: model, aux card, slots, and any extra
    /// memory regions. Required in a complete config; an overlay may omit
    /// it.
    pub machine: Option<Machine>,
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
    /// Remote-console (VNC) server. When present the machine boots headless
    /// and is reachable over the network instead of opening an SDL window.
    #[serde(default)]
    pub remote: Remote,
    /// Machine-state persistence: restore at startup, save at quit.
    #[serde(default)]
    pub state: State,
}

/// The machine's physical build.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Machine {
    /// Which machine to emulate. Required in a complete config; an
    /// overlay may omit it and inherit the base config's model.
    pub model: Option<Model>,
    /// The //e auxiliary-slot card. Only valid with `"model": "2e"`; when
    /// absent the //e gets the standard Extended 80-Column Text Card.
    pub aux: Option<Aux>,
    /// The card in each peripheral slot, keyed `"0"` through `"7"` (slot 0
    /// is the ][+ language-card socket). When the whole `slots` object is
    /// absent the machine gets the classic default layout (a Language Card
    /// in slot 0, a Thunderclock in slot 1, a Disk II in slot 6); when
    /// present it is taken literally — an absent slot key means that slot
    /// is empty, and `"empty"` exists to say it explicitly. A ][+ table
    /// without `"0"` is therefore a 48K machine.
    pub slots: Option<BTreeMap<String, SlotCard>>,
    /// Extra RAM or ROM regions loaded from files at startup.
    #[serde(default)]
    pub memory: Vec<MemoryRegion>,
}

/// Which machine to emulate. The token also decides the machine
/// *family* — Apple II (`ewm two`) or Apple 1 (`ewm one`) — which the
/// completeness validation and the subcommands use to keep configs
/// honest (plans/20260719-02-one-config.md).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum Model {
    /// The Apple ][+.
    #[serde(rename = "2plus")]
    TwoPlus,
    /// The Apple //e.
    #[serde(rename = "2e")]
    TwoE,
    /// The classic Apple 1 (6502, 8KB RAM, Woz Monitor).
    #[serde(rename = "apple1")]
    Apple1,
    /// The Replica 1 (65C02, 32KB RAM, KRUSADER).
    #[serde(rename = "replica1")]
    Replica1,
}

/// The machine family a model belongs to: which subcommand runs it and
/// which config keys apply to it (see `validate_complete`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// `ewm two`: the Apple ][+ / //e — slots, aux, display, the works.
    Apple2,
    /// `ewm one`: the Apple 1 / Replica 1 — model, memory, debugging.
    Apple1,
}

impl Model {
    pub fn family(self) -> Family {
        match self {
            Model::TwoPlus | Model::TwoE => Family::Apple2,
            Model::Apple1 | Model::Replica1 => Family::Apple1,
        }
    }

    /// The schema token, for error messages.
    pub fn token(self) -> &'static str {
        match self {
            Model::TwoPlus => "2plus",
            Model::TwoE => "2e",
            Model::Apple1 => "apple1",
            Model::Replica1 => "replica1",
        }
    }

    /// The `ewm two` machine type; `None` for the one family (callers
    /// turn that into the cross-subcommand error).
    pub fn two_type(self) -> Option<TwoType> {
        match self {
            Model::TwoPlus => Some(TwoType::Apple2Plus),
            Model::TwoE => Some(TwoType::Apple2E),
            Model::Apple1 | Model::Replica1 => None,
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
    /// The card's aux token, so config and the power-on path share one
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
    /// A UniDisk 3.5 Controller ("Liron") with up to two SmartPort 3.5"
    /// drives taking .2mg images of 400K or 800K.
    Liron {
        /// .2mg image for drive 1.
        drive1: Option<String>,
        /// .2mg image for drive 2.
        drive2: Option<String>,
    },
    /// A Thunderclock Plus real-time clock.
    Thunderclock,
    /// The 16K Apple Language Card. Slot 0 only, ][+ only — it turns the
    /// 48K machine into the classic 64K build. Omitting slot 0 from an
    /// explicit `slots` table leaves the socket empty (a 48K machine).
    Language,
    /// The Saturn Systems 128K RAM Board: eight 16K banks at $D000-$FFFF,
    /// bank 1 speaking the exact Language Card protocol. Slot 0 only,
    /// ][+ only.
    Saturn128,
    /// Explicitly nothing in this slot.
    Empty,
}

impl SlotCard {
    /// The `"card"` discriminator value, for error messages.
    pub fn card_name(&self) -> &'static str {
        match self {
            SlotCard::Diskii { .. } => "diskii",
            SlotCard::Harddrive { .. } => "harddrive",
            SlotCard::Liron { .. } => "liron",
            SlotCard::Thunderclock => "thunderclock",
            SlotCard::Language => "language",
            SlotCard::Saturn128 => "saturn128",
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

/// Remote-console (VNC) server settings. Presence of a `protocol` or `port`
/// makes `ewm two` boot headless and serve the screen over RFB (VNC) rather
/// than opening a local window. See notes/REMOTE.md.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Remote {
    /// Remote-console protocol. Only `"vnc"` is implemented; `"rdp"` is
    /// reserved for a later optional track (notes/REMOTE.md Track B).
    pub protocol: Option<RemoteProtocol>,
    /// Address to bind. Defaults to `127.0.0.1` (localhost only); set
    /// `"0.0.0.0"` to expose the machine to the network.
    pub bind: Option<String>,
    /// Plain-TCP RFB (VNC) port. Defaults to 5901.
    pub port: Option<u16>,
    /// RFB-over-WebSocket port for browser clients: noVNC connects straight
    /// to it, no websockify bridge. Absent means no WebSocket listener.
    pub websocket: Option<u16>,
    /// Serve the embedded web console (a vendored noVNC page) for plain HTTP
    /// requests on the WebSocket port, so `http://host:port/` is a live
    /// console with no external tooling. Implies a WebSocket listener (on
    /// 5701 when `websocket` is absent). Off by default.
    pub web: Option<bool>,
    /// Serve the console read-only: ignore all keyboard and pointer input.
    pub view_only: Option<bool>,
    /// VNC-auth password. When set, clients must authenticate with the RFB DES
    /// challenge (only the first 8 characters are significant); this is what
    /// lets clients that refuse the "None" security type — notably macOS
    /// Screen Sharing — connect. The scheme is weak (notes/REMOTE.md §10):
    /// keep the machine on localhost or behind a tunnel/proxy regardless.
    pub password: Option<String>,
}

/// Machine-state persistence (notes/STATE.md): with a path set, the
/// machine restores from it at startup (when the file exists) and saves
/// back to it at quit — suspend/resume. Requires the same hardware
/// configuration across runs (mismatch detection is backlog).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct State {
    /// The state file, resolved relative to the config file.
    pub path: Option<String>,
}

/// The remote-console protocol (`remote.protocol`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum RemoteProtocol {
    /// RFB / VNC (RFC 6143) — the implemented v1 protocol.
    Vnc,
    /// RDP — reserved, not implemented.
    Rdp,
}

/// The built-in machine configurations: the files under `configs/`,
/// embedded at compile time so `--config builtin:<name>` works from any
/// installed binary. Name = the file's stem, sorted, so errors and
/// `builtin:list` read predictably. Built-ins must be self-contained —
/// enforced by `load_builtin` and pinned by the
/// `builtins_load_and_are_self_contained` test. See
/// plans/20260718-02-config-sources.md (C1).
const BUILTINS: &[(&str, &str)] = &[
    ("2e", include_str!("../../configs/2e.json")),
    ("2plus", include_str!("../../configs/2plus.json")),
    ("apple1", include_str!("../../configs/apple1.json")),
    ("replica1", include_str!("../../configs/replica1.json")),
];

/// Name and description of every built-in config, for `builtin:list`.
pub fn builtin_list() -> Vec<(&'static str, Option<String>)> {
    BUILTINS
        .iter()
        .map(|(name, _)| {
            let config = load_builtin(name).expect("builtins are pinned valid by test");
            (*name, config.description)
        })
        .collect()
}

/// Load the named built-in configuration. Built-ins carry no file
/// references (there is no directory to resolve relative paths against),
/// so no path resolution happens; a builtin that references a file is a
/// bug, caught here and by the self-containment test.
pub fn load_builtin(name: &str) -> Result<Config, String> {
    let Some((_, text)) = BUILTINS.iter().find(|(n, _)| *n == name) else {
        let names: Vec<&str> = BUILTINS.iter().map(|(n, _)| *n).collect();
        return Err(format!(
            "no built-in config {name:?} (available: {})",
            names.join(", ")
        ));
    };
    let origin = format!("builtin:{name}");
    let config: Config = serde_json::from_str(text).map_err(|e| format!("{origin}: {e}"))?;
    validate(&config).map_err(|e| format!("{origin}: {e}"))?;
    validate_complete(&config, "built-in configs must be complete")
        .map_err(|e| format!("{origin}: {e}"))?;
    let files = referenced_files(&config);
    if !files.is_empty() {
        return Err(format!(
            "{origin}: built-in configs cannot reference files ({})",
            files.join(", ")
        ));
    }
    Ok(config)
}

/// Every file path a config references — drive images, memory-region
/// files, the trace and state paths. The set that must be empty for a
/// built-in config.
fn referenced_files(config: &Config) -> Vec<&str> {
    let mut files: Vec<&str> = Vec::new();
    if let Some(machine) = &config.machine {
        for card in machine.slots.iter().flat_map(|s| s.values()) {
            match card {
                SlotCard::Diskii { drive1, drive2 } | SlotCard::Liron { drive1, drive2 } => {
                    files.extend(drive1.as_deref());
                    files.extend(drive2.as_deref());
                }
                SlotCard::Harddrive { image } => files.push(image),
                SlotCard::Thunderclock
                | SlotCard::Language
                | SlotCard::Saturn128
                | SlotCard::Empty => {}
            }
        }
        files.extend(machine.memory.iter().map(|r| r.path.as_str()));
    }
    files.extend(config.debug.trace.as_deref());
    files.extend(config.state.path.as_deref());
    files
}

/// Resolve a `--config` source to its JSON-document form, ready for
/// layering (`merge_documents`, `apply_set`): `builtin:<name>` loads an
/// embedded config, anything else is a file path (a literal file named
/// `builtin:…` is reachable as `./builtin:…`). A `--config` source is a
/// *complete* machine, so completeness is required per file here — a
/// partial fragment belongs to `--config-overlay`.
pub fn load_source_document(source: &str) -> Result<serde_json::Value, String> {
    match source.strip_prefix("builtin:") {
        Some(name) => {
            let config = load_builtin(name)?;
            serde_json::to_value(config).map_err(|e| format!("builtin:{name}: {e}"))
        }
        None => {
            let config = load(source)?;
            serde_json::to_value(config).map_err(|e| format!("{source}: {e}"))
        }
    }
}

/// Load a *complete* machine configuration: read the file, parse it,
/// validate it structurally and for completeness, and resolve relative
/// paths against the file's directory.
pub fn load(path: &str) -> Result<Config, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read config {path}: {e}"))?;
    let base = Path::new(path).parent().unwrap_or(Path::new("."));
    from_str_resolved(&text, path, base)
}

/// Resolve a `--config-overlay` source to its JSON-document form. The
/// `builtin:` scheme is shared with `--config` (a built-in is a complete
/// config, which is a valid overlay); anything else is a file loaded
/// through the structural-only path — an overlay may be arbitrarily
/// partial, and its relative paths resolve against the overlay file's
/// directory, the same portability property config files have.
pub fn load_overlay_document(source: &str) -> Result<serde_json::Value, String> {
    match source.strip_prefix("builtin:") {
        Some(name) => {
            let config = load_builtin(name)?;
            serde_json::to_value(config).map_err(|e| format!("builtin:{name}: {e}"))
        }
        None => load_document(source),
    }
}

/// Load a config file as a JSON document: the typed *structural* path
/// (parse, per-file validation with the file named in errors, relative
/// paths resolved against the file's directory), then back to JSON —
/// ready to layer with other sources (`merge_documents`, `apply_set`)
/// before the final `from_document`. Completeness (`machine.model`) is
/// *not* required here: partial overlay fragments load through this path;
/// the complete-config path is `load`.
pub fn load_document(path: &str) -> Result<serde_json::Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read config {path}: {e}"))?;
    let base = Path::new(path).parent().unwrap_or(Path::new("."));
    let config = from_str_partial(&text, path, base)?;
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
        "0": { "card": "language" },
        "1": { "card": "thunderclock" },
        "6": { "card": "diskii" },
    })
}

/// Merge a `--config-overlay` document into the base: `merge_documents`
/// plus the slots-materialization rule `--set` already has — an overlay
/// carrying a `machine.slots` table onto a document without one
/// materializes the default table first, so the overlay *extends* the
/// default machine ("plus a hard drive in slot 7") instead of producing a
/// literal one-slot table. A base whose explicit table came from
/// `--config` stays literal, as today: materialization only fills a
/// missing table, never touches a present one.
pub fn merge_overlay_document(doc: &mut serde_json::Value, overlay: serde_json::Value) {
    use serde_json::Value;
    if matches!(overlay.pointer("/machine/slots"), Some(Value::Object(_)))
        && let Some(base) = doc.as_object_mut()
    {
        let machine = base
            .entry("machine")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(machine) = machine.as_object_mut()
            && machine.get("slots").is_none_or(|s| s.is_null())
        {
            machine.insert("slots".to_string(), default_slots_value());
        }
    }
    merge_documents(doc, overlay);
}

/// Compact a serialized config document for human consumption
/// (`--print-config`, the future "save current setup" — JSON_CONFIG
/// Phase C): drop `null` members (absent options), empty arrays (no
/// memory regions), and objects that *become* empty once their nulls are
/// gone (untouched sections) — while keeping objects that were genuinely
/// empty in the typed config, like an explicit bare `"slots": {}` table,
/// which means "no cards", not "default layout".
pub fn compact_document(doc: &mut serde_json::Value) {
    use serde_json::Value;
    fn keep(value: &mut Value) -> bool {
        match value {
            Value::Null => false,
            Value::Array(entries) => !entries.is_empty(),
            Value::Object(map) if map.is_empty() => true,
            Value::Object(map) => {
                map.retain(|_, member| keep(member));
                !map.is_empty()
            }
            _ => true,
        }
    }
    keep(doc);
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
                    "--set {expr}: cannot index into the {:?} array (memory regions come from a config or overlay file)",
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
    validate_complete(&config, "start from --config, e.g. --config builtin:2plus")
        .map_err(|e| format!("config: {e}"))?;
    Ok(config)
}

/// The testable core of `load`: `origin` names the file in error messages,
/// `base` is the directory relative paths resolve against.
fn from_str_resolved(text: &str, origin: &str, base: &Path) -> Result<Config, String> {
    let config = from_str_partial(text, origin, base)?;
    validate_complete(&config, "is this an overlay? use --config-overlay")
        .map_err(|e| format!("{origin}: {e}"))?;
    Ok(config)
}

/// The fragment-friendly core shared by `load` and `load_document`: parse,
/// structural validation, path resolution — no completeness check, so a
/// partial overlay loads.
fn from_str_partial(text: &str, origin: &str, base: &Path) -> Result<Config, String> {
    let mut config: Config = serde_json::from_str(text).map_err(|e| format!("{origin}: {e}"))?;
    validate(&config).map_err(|e| format!("{origin}: {e}"))?;
    resolve_paths(&mut config, base);
    Ok(config)
}

/// Structural validation beyond what serde's typed parse enforces:
/// everything that can be judged on a lone (possibly partial) fragment.
/// Cross-checks that need `machine.model` live in `validate_complete` —
/// a fragment adding `machine.aux` can't be judged until the merged
/// document says what the model is.
fn validate(config: &Config) -> Result<(), String> {
    let machine = config.machine.as_ref();
    if let Some(aux) = machine.and_then(|m| m.aux.as_ref())
        && let Some(size) = &aux.size
    {
        if aux.card != AuxKind::RamWorksIII {
            return Err("machine.aux.size: only valid with the \"ramworksiii\" card".into());
        }
        crate::aux::parse_size(size).map_err(|e| format!("machine.aux.size: {e}"))?;
    }
    if let Some(slots) = machine.and_then(|m| m.slots.as_ref()) {
        for (key, card) in slots {
            match key.as_str() {
                // Slot 0 is the ][+ memory-expansion socket: no $Cn00
                // firmware space, so only bankable-RAM cards (or nothing)
                // fit. (That the //e has no slot 0 at all is a model
                // cross-check, judged in validate_complete.)
                "0" => {
                    if !matches!(
                        card,
                        SlotCard::Language | SlotCard::Saturn128 | SlotCard::Empty
                    ) {
                        return Err(format!(
                            "machine.slots: slot \"0\" takes only \"language\", \"saturn128\" or \"empty\" (not \"{}\")",
                            card.card_name()
                        ));
                    }
                }
                "1" | "2" | "3" | "4" | "5" | "6" | "7" => {
                    if matches!(card, SlotCard::Language | SlotCard::Saturn128) {
                        return Err(format!(
                            "machine.slots: the {} card only fits slot \"0\" (not slot {key:?})",
                            card.card_name()
                        ));
                    }
                }
                _ => {
                    return Err(format!(
                        "machine.slots: no such slot {key:?} (slots are \"0\" through \"7\")"
                    ));
                }
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
    for (i, region) in machine
        .map(|m| m.memory.as_slice())
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
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
    if config.remote.protocol == Some(RemoteProtocol::Rdp) {
        return Err("remote.protocol: \"rdp\" is not implemented yet (VNC only)".into());
    }
    if config.remote.port == Some(0) {
        return Err("remote.port: must be at least 1".into());
    }
    if config.remote.websocket == Some(0) {
        return Err("remote.websocket: must be at least 1".into());
    }
    if config.remote.websocket.is_some()
        && config.remote.websocket == config.remote.port.or(Some(5901))
    {
        return Err("remote.websocket: must differ from remote.port".into());
    }
    Ok(())
}

/// Completeness validation: the checks only the final layered document can
/// pass — `machine.model` must be present, plus the cross-checks that need
/// the model: the //e-only rules within the apple2 family, and the family
/// table that keeps apple2-only keys off the Apple 1 / Replica 1 (rejected,
/// not ignored — the TTY is fixed green, the clock fixed 1.023 MHz; remote
/// and state for the one family are recorded backlog). `hint` finishes the
/// missing-model message with where the model should have come from (the
/// fix differs per calling path).
fn validate_complete(config: &Config, hint: &str) -> Result<(), String> {
    let Some(model) = config.machine.as_ref().and_then(|m| m.model) else {
        return Err(format!("machine.model is required ({hint})"));
    };
    let machine = config.machine.as_ref().expect("model implies machine");
    match model.family() {
        Family::Apple2 => {
            if machine.aux.is_some() && model != Model::TwoE {
                return Err("machine.aux: aux cards are a //e feature (model is \"2plus\")".into());
            }
            if model == Model::TwoE && machine.slots.as_ref().is_some_and(|s| s.contains_key("0")) {
                return Err(
                    "machine.slots: the //e has no slot 0 (its language card is built in)".into(),
                );
            }
        }
        Family::Apple1 => {
            let token = model.token();
            if machine.slots.is_some() {
                return Err(format!("machine.slots: {token:?} has no peripheral slots"));
            }
            if machine.aux.is_some() {
                return Err(format!("machine.aux: {token:?} has no auxiliary slot"));
            }
            if config.display != Display::default() {
                return Err(format!(
                    "display: not configurable for {token:?} (the TTY is fixed)"
                ));
            }
            if config.cpu.speed.is_some() {
                return Err(format!(
                    "cpu.speed: not configurable for {token:?} (the clock is fixed)"
                ));
            }
            if config.input != Input::default() {
                return Err(format!("input: not configurable for {token:?}"));
            }
            if config.boot != Boot::default() {
                return Err(format!("boot: not configurable for {token:?}"));
            }
            if config.remote != Remote::default() {
                return Err(format!(
                    "remote: not configurable for {token:?} yet (notes/REMOTE.md Phase 7)"
                ));
            }
            if config.state != State::default() {
                return Err(format!("state: not configurable for {token:?} yet"));
            }
        }
    }
    Ok(())
}

/// Rewrite every relative path-valued field to be relative to `base` — the
/// config file's directory — so a config works regardless of the CWD.
fn resolve_paths(config: &mut Config, base: &Path) {
    if let Some(machine) = &mut config.machine {
        for card in machine.slots.iter_mut().flat_map(|s| s.values_mut()) {
            match card {
                SlotCard::Diskii { drive1, drive2 } | SlotCard::Liron { drive1, drive2 } => {
                    if let Some(p) = drive1 {
                        resolve(base, p);
                    }
                    if let Some(p) = drive2 {
                        resolve(base, p);
                    }
                }
                SlotCard::Harddrive { image } => resolve(base, image),
                SlotCard::Thunderclock
                | SlotCard::Language
                | SlotCard::Saturn128
                | SlotCard::Empty => {}
            }
        }
        for region in &mut machine.memory {
            resolve(base, &mut region.path);
        }
    }
    if let Some(p) = &mut config.debug.trace {
        resolve(base, p);
    }
    if let Some(p) = &mut config.state.path {
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

    /// The committed schemas are derived from these structs — this test
    /// keeps them in lockstep, byte for byte. Since C2 the serde types are
    /// partial-friendly, so the raw generated schema is *overlay*-shaped
    /// (nothing required); it is committed as
    /// schema/ewm-config-overlay.schema.json with its own title and
    /// description, while the complete-config schema gets the requiredness
    /// (`machine`, `machine.model`) post-processed back in for editors.
    /// Regenerate both with:
    ///
    ///   EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed
    #[test]
    fn schema_matches_committed() {
        let schema = schemars::generate::SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<Config>();
        let mut full = serde_json::to_value(&schema).expect("schema must serialize");
        let mut overlay = full.clone();
        full["required"] = serde_json::json!(["machine"]);
        full["$defs"]["Machine"]["required"] = serde_json::json!(["model"]);
        overlay["title"] = serde_json::json!("ConfigOverlay");
        overlay["description"] = serde_json::json!(
            "A partial EWM machine configuration, for `ewm two --config-overlay \
             file.json`: the same shape as ewm-config.schema.json with nothing \
             required, deep-merged onto the base config."
        );

        for (schema, file) in [
            (full, "/../schema/ewm-config.schema.json"),
            (overlay, "/../schema/ewm-config-overlay.schema.json"),
        ] {
            // Back through schemars::Schema: its Serialize impl is what
            // orders the keywords canonically ($schema, title, … last
            // $defs) instead of alphabetically.
            let schema = schemars::Schema::try_from(schema).expect("still a schema");
            let mut generated =
                serde_json::to_string_pretty(&schema).expect("schema must serialize");
            generated.push('\n');
            let path = format!("{}{file}", env!("CARGO_MANIFEST_DIR"));
            if std::env::var_os("EWM_UPDATE_SCHEMA").is_some() {
                std::fs::write(&path, &generated).expect("cannot write the schema");
                continue;
            }
            let committed = std::fs::read_to_string(&path).expect(
                "cannot read the committed schema — regenerate with \
                 EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed",
            );
            assert_eq!(
                committed, generated,
                "schema drift in {file} — regenerate with \
                 EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed"
            );
        }
    }

    #[test]
    fn builtins_load_and_are_self_contained() {
        for (name, text) in BUILTINS {
            let config = load_builtin(name).expect("every builtin must load");
            // Self-containment, stated directly: the loader would have
            // rejected any file reference, but pin the property on the
            // parsed text too so a future loader change can't lose it.
            let parsed: Config = serde_json::from_str(text).expect("builtin parses");
            assert_eq!(
                referenced_files(&parsed),
                Vec::<&str>::new(),
                "builtin:{name}"
            );
            // Every builtin describes itself for `builtin:list`.
            assert!(
                config.description.is_some(),
                "builtin:{name} needs a description"
            );
        }
        // The table stays sorted so listings and error text read predictably.
        assert!(BUILTINS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn builtin_names_match_their_model() {
        // The naming convention: builtin names are the schema's model
        // tokens (builtin:2plus, builtin:apple1, …), both families.
        let models: Vec<&str> = builtin_list().iter().map(|(n, _)| *n).collect();
        assert_eq!(models, vec!["2e", "2plus", "apple1", "replica1"]);
        let model = |name| load_builtin(name).unwrap().machine.unwrap().model;
        assert_eq!(model("2plus"), Some(Model::TwoPlus));
        assert_eq!(model("2e"), Some(Model::TwoE));
        assert_eq!(model("apple1"), Some(Model::Apple1));
        assert_eq!(model("replica1"), Some(Model::Replica1));
    }

    #[test]
    fn unknown_builtin_lists_the_available_names() {
        let err = load_builtin("foo").unwrap_err();
        assert_eq!(
            err,
            r#"no built-in config "foo" (available: 2e, 2plus, apple1, replica1)"#
        );
    }

    #[test]
    fn referenced_files_finds_every_path_field() {
        let config = parse(
            r#"{"machine": {"model": "2plus",
                "slots": {
                    "5": {"card": "liron", "drive1": "/a.2mg"},
                    "6": {"card": "diskii", "drive1": "/b.dsk", "drive2": "/c.dsk"},
                    "7": {"card": "harddrive", "image": "/d.hdv"}},
                "memory": [{"type": "rom", "address": "0xd000", "path": "/e.bin"}]},
                "debug": {"trace": "/f.txt"},
                "state": {"path": "/g.state"}}"#,
        )
        .expect("valid config");
        let mut files = referenced_files(&config);
        files.sort();
        assert_eq!(
            files,
            vec![
                "/a.2mg", "/b.dsk", "/c.dsk", "/d.hdv", "/e.bin", "/f.txt", "/g.state"
            ]
        );
    }

    #[test]
    fn source_documents_resolve_builtins_and_paths() {
        let builtin = load_source_document("builtin:2plus").expect("builtin source");
        let file = load_document(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../configs/2plus.json"
        ))
        .expect("file source");
        // The embedded copy and the committed file are the same config.
        assert_eq!(builtin, file);
        let err = load_source_document("builtin:nope").unwrap_err();
        assert!(err.starts_with("no built-in config"), "{err}");
    }

    #[test]
    fn minimal_config_parses() {
        let config = parse(r#"{"machine": {"model": "2plus"}}"#).expect("minimal config");
        let machine = config.machine.expect("machine present");
        assert_eq!(machine.model, Some(Model::TwoPlus));
        assert!(machine.aux.is_none());
        assert!(machine.slots.is_none());
        assert!(machine.memory.is_empty());
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

        // The Liron takes up to two .2mg drives, any slot but 0.
        assert!(slot("5", r#"{"card": "liron"}"#).is_ok());
        assert!(
            slot(
                "5",
                r#"{"card": "liron", "drive1": "a.2mg", "drive2": "b.2mg"}"#
            )
            .is_ok()
        );
        let err = slot("0", r#"{"card": "liron"}"#).unwrap_err();
        assert!(
            err.contains(r#"slot "0" takes only "language", "saturn128" or "empty""#),
            "{err}"
        );

        // Slot 0 is the ][+ memory-expansion socket: bankable-RAM cards or
        // empty only, and those cards fit nowhere else.
        assert!(slot("0", r#"{"card": "language"}"#).is_ok());
        assert!(slot("0", r#"{"card": "saturn128"}"#).is_ok());
        assert!(slot("0", r#"{"card": "empty"}"#).is_ok());
        let err = slot("0", r#"{"card": "diskii"}"#).unwrap_err();
        assert!(
            err.contains(r#"slot "0" takes only "language", "saturn128" or "empty""#),
            "{err}"
        );
        let err = slot("3", r#"{"card": "language"}"#).unwrap_err();
        assert!(
            err.contains(r#"the language card only fits slot "0""#),
            "{err}"
        );
        let err = slot("3", r#"{"card": "saturn128"}"#).unwrap_err();
        assert!(
            err.contains(r#"the saturn128 card only fits slot "0""#),
            "{err}"
        );
        let err = parse(r#"{"machine": {"model": "2e", "slots": {"0": {"card": "language"}}}}"#)
            .unwrap_err();
        assert!(
            err.contains("the //e has no slot 0 (its language card is built in)"),
            "{err}"
        );

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
        assert_eq!(config.machine.unwrap().slots, Some(BTreeMap::new()));
    }

    #[test]
    fn model_families_and_tokens() {
        assert_eq!(Model::TwoPlus.family(), Family::Apple2);
        assert_eq!(Model::TwoE.family(), Family::Apple2);
        assert_eq!(Model::Apple1.family(), Family::Apple1);
        assert_eq!(Model::Replica1.family(), Family::Apple1);
        assert_eq!(Model::TwoPlus.two_type(), Some(TwoType::Apple2Plus));
        assert_eq!(Model::TwoE.two_type(), Some(TwoType::Apple2E));
        assert_eq!(Model::Apple1.two_type(), None);
        assert_eq!(Model::Replica1.two_type(), None);
        assert_eq!(Model::Apple1.token(), "apple1");
        assert_eq!(Model::Replica1.token(), "replica1");
    }

    #[test]
    fn one_family_models_parse_with_their_keys() {
        // Minimal one-family configs are complete and valid.
        let config = parse(r#"{"machine": {"model": "apple1"}}"#).expect("apple1");
        assert_eq!(config.machine.unwrap().model, Some(Model::Apple1));
        // The keys the one family does have: memory, strict, trace.
        let config = parse(
            r#"{"machine": {"model": "replica1",
                "memory": [{"type": "rom", "address": "0xe000", "path": "basic.rom"}]},
                "cpu": {"strict": true},
                "debug": {"trace": "trace.txt"}}"#,
        )
        .expect("replica1 with memory/strict/trace");
        let machine = config.machine.expect("machine");
        assert_eq!(machine.memory.len(), 1);
        // Paths resolve against the config's directory, as for two.
        assert_eq!(machine.memory[0].path, "/cfg/basic.rom");
        assert_eq!(config.debug.trace.as_deref(), Some("/cfg/trace.txt"));
    }

    #[test]
    fn one_family_models_reject_apple2_keys() {
        // The family cross-checks: rejected, not ignored, naming the
        // offending key and the model.
        let case = |json: &str, key: &str| {
            let err = parse(json).unwrap_err();
            assert!(err.contains(key), "{key}: {err}");
            assert!(
                err.contains("apple1") || err.contains("replica1"),
                "{key}: {err}"
            );
        };
        case(
            r#"{"machine": {"model": "replica1", "slots": {}}}"#,
            "machine.slots",
        );
        case(
            r#"{"machine": {"model": "apple1", "aux": {"card": "80col"}}}"#,
            "machine.aux",
        );
        case(
            r#"{"machine": {"model": "apple1"}, "display": {"monitor": "green"}}"#,
            "display",
        );
        case(
            r#"{"machine": {"model": "replica1"}, "cpu": {"speed": "normal"}}"#,
            "cpu.speed",
        );
        case(
            r#"{"machine": {"model": "apple1"}, "input": {"controller": "Pad"}}"#,
            "input",
        );
        case(
            r#"{"machine": {"model": "apple1"}, "boot": {"delay": 1.5}}"#,
            "boot",
        );
        case(
            r#"{"machine": {"model": "apple1"}, "remote": {"port": 5901}}"#,
            "remote",
        );
        case(
            r#"{"machine": {"model": "apple1"}, "state": {"path": "m.state"}}"#,
            "state",
        );
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
        let aux = config
            .machine
            .expect("machine present")
            .aux
            .expect("aux present");
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
        let machine = config.machine.as_ref().expect("machine present");
        let slots = machine.slots.as_ref().expect("slots present");
        let SlotCard::Diskii { drive1, drive2 } = &slots["6"] else {
            panic!("slot 6 should be a diskii");
        };
        assert_eq!(drive1.as_deref(), Some("/cfg/disks/a.dsk"));
        assert_eq!(drive2.as_deref(), Some("/abs/b.dsk"));
        let SlotCard::Harddrive { image } = &slots["7"] else {
            panic!("slot 7 should be a harddrive");
        };
        assert_eq!(image, "/cfg/hd.hdv");
        assert_eq!(machine.memory[0].path, "/cfg/roms/x.bin");
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
    fn overlay_merge_materializes_the_default_slots() {
        let hdd7 = serde_json::json!({"machine": {"slots": {"7": {"card": "harddrive", "image": "tr.hdv"}}}});

        // The four base × overlay slots combinations (the plan's hazard).
        // 1. Slotless base + overlay with slots: materialize, then extend —
        //    "the default machine plus a hard drive in slot 7".
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        merge_overlay_document(&mut doc, hdd7.clone());
        assert_eq!(
            doc["machine"]["slots"],
            serde_json::json!({
                "0": {"card": "language"},
                "1": {"card": "thunderclock"},
                "6": {"card": "diskii"},
                "7": {"card": "harddrive", "image": "tr.hdv"},
            })
        );

        // 2. A base with an explicit table stays literal; the overlay
        //    merges into it key by key.
        let mut doc =
            serde_json::json!({"machine": {"model": "2plus", "slots": {"6": {"card": "diskii"}}}});
        merge_overlay_document(&mut doc, hdd7);
        assert_eq!(
            doc["machine"]["slots"],
            serde_json::json!({
                "6": {"card": "diskii"},
                "7": {"card": "harddrive", "image": "tr.hdv"},
            })
        );

        // 3. An overlay without slots never materializes: a slotless base
        //    stays slotless (the default machine at build time)...
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        merge_overlay_document(
            &mut doc,
            serde_json::json!({"display": {"monitor": "amber"}}),
        );
        assert_eq!(doc["machine"], serde_json::json!({"model": "2plus"}));
        assert_eq!(doc["display"]["monitor"], serde_json::json!("amber"));

        // 4. ...and a base's explicit table is untouched.
        let mut doc = serde_json::json!({"machine": {"model": "2plus", "slots": {}}});
        merge_overlay_document(&mut doc, serde_json::json!({"cpu": {"strict": true}}));
        assert_eq!(doc["machine"]["slots"], serde_json::json!({}));

        // A null machine.slots (how a fragment that never mentioned slots
        // serializes) is a merge no-op, not a table.
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        merge_overlay_document(
            &mut doc,
            serde_json::json!({"machine": {"model": "2e", "slots": null}}),
        );
        assert_eq!(doc["machine"], serde_json::json!({"model": "2e"}));
    }

    #[test]
    fn compact_document_drops_noise_but_keeps_bare_tables() {
        let mut doc = serde_json::json!({
            "machine": {
                "model": "2plus",
                "aux": null,
                "slots": {"6": {"card": "diskii", "drive1": "a.dsk", "drive2": null}},
                "memory": [],
            },
            "display": {"monitor": "green", "scanlines": null, "fps": null},
            "input": {"controller": null},
        });
        compact_document(&mut doc);
        assert_eq!(
            doc,
            serde_json::json!({
                "machine": {
                    "model": "2plus",
                    "slots": {"6": {"card": "diskii", "drive1": "a.dsk"}},
                },
                "display": {"monitor": "green"},
            })
        );

        // An explicit bare slots table survives: {} means "no cards",
        // where an absent table would mean the default layout.
        let mut doc = serde_json::json!({
            "machine": {"model": "2plus", "slots": {}},
            "cpu": {"speed": null, "strict": null},
        });
        compact_document(&mut doc);
        assert_eq!(
            doc,
            serde_json::json!({"machine": {"model": "2plus", "slots": {}}})
        );
    }

    #[test]
    fn overlay_sources_resolve_builtins_and_fragments() {
        // The builtin: scheme is shared with --config...
        let builtin = load_overlay_document("builtin:2plus").expect("builtin overlay");
        assert_eq!(builtin["machine"]["model"], serde_json::json!("2plus"));
        let err = load_overlay_document("builtin:nope").unwrap_err();
        assert!(err.starts_with("no built-in config"), "{err}");
        // ...while a file may be arbitrarily partial.
        let path = scratch("frag.json", r#"{"display": {"monitor": "amber"}}"#);
        let doc = load_overlay_document(path.to_str().unwrap()).expect("fragment loads");
        assert_eq!(doc["display"]["monitor"], serde_json::json!("amber"));
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
                "0": {"card": "language"},
                "1": {"card": "thunderclock"},
                "6": {"card": "diskii", "drive1": "x.dsk"},
            })
        );
        // Opting out of the language card keeps the rest of the default
        // layout: the classic 48K machine.
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        apply_set(&mut doc, "machine:slots:0:card=empty").unwrap();
        assert_eq!(
            doc["machine"]["slots"],
            serde_json::json!({
                "0": {"card": "empty"},
                "1": {"card": "thunderclock"},
                "6": {"card": "diskii"},
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

    /// A scratch file under the OS temp dir, for exercising the file-based
    /// loaders on inline JSON.
    fn scratch(name: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("ewm-config-c2-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(name);
        std::fs::write(&path, text).expect("write scratch config");
        path
    }

    #[test]
    fn load_document_accepts_partial_fragments() {
        // A slots-only fragment — a whole valid overlay — loads as a
        // document, with relative paths resolved against the file's dir.
        let path = scratch(
            "overlay.json",
            r#"{"machine": {"slots": {"7": {"card": "harddrive", "image": "tr.hdv"}}}}"#,
        );
        let doc = load_document(path.to_str().unwrap()).expect("fragment loads");
        assert!(doc["machine"]["model"].is_null());
        let image = doc["machine"]["slots"]["7"]["image"].as_str().unwrap();
        assert!(
            image.ends_with("tr.hdv") && Path::new(image).is_absolute(),
            "{image}"
        );

        // The empty fragment is the degenerate overlay.
        let path = scratch("empty.json", "{}");
        let doc = load_document(path.to_str().unwrap()).expect("empty fragment loads");
        assert!(doc["machine"].is_null());

        // Structural errors still name the file.
        let path = scratch(
            "bad.json",
            r#"{"machine": {"slots": {"9": {"card": "empty"}}}}"#,
        );
        let err = load_document(path.to_str().unwrap()).unwrap_err();
        assert!(
            err.contains("bad.json") && err.contains(r#"no such slot "9""#),
            "{err}"
        );
    }

    #[test]
    fn load_requires_a_complete_config() {
        // The complete-config path (--config) rejects a fragment per file,
        // pointing at the overlay flag.
        let path = scratch("partial.json", r#"{"machine": {"slots": {}}}"#);
        let err = load(path.to_str().unwrap()).unwrap_err();
        assert!(
            err.ends_with(
                "partial.json: machine.model is required (is this an overlay? use --config-overlay)"
            ),
            "{err}"
        );
        // load_source_document is the actual --config path; same contract.
        let err = load_source_document(path.to_str().unwrap()).unwrap_err();
        assert!(err.contains("machine.model is required"), "{err}");
    }

    #[test]
    fn from_document_requires_machine_model() {
        let message = "config: machine.model is required \
                       (start from --config, e.g. --config builtin:2plus)";
        let err = from_document(serde_json::json!({})).unwrap_err();
        assert_eq!(err, message);
        let err = from_document(serde_json::json!({"machine": {}})).unwrap_err();
        assert_eq!(err, message);
    }

    #[test]
    fn model_cross_checks_run_on_the_final_document() {
        // An aux card is structurally fine on a modelless fragment...
        let fragment = serde_json::json!({"machine": {"aux": {"card": "80col"}}});
        let config: Config = serde_json::from_value(fragment.clone()).expect("fragment parses");
        assert!(validate(&config).is_ok());

        // ...and judged against the model once the document is complete.
        let mut doc = serde_json::json!({"machine": {"model": "2plus"}});
        merge_documents(&mut doc, fragment.clone());
        let err = from_document(doc).unwrap_err();
        assert!(err.contains("//e feature"), "{err}");
        let mut doc = serde_json::json!({"machine": {"model": "2e"}});
        merge_documents(&mut doc, fragment);
        assert!(from_document(doc).is_ok());

        // Same for the //e's missing slot 0.
        let err = from_document(serde_json::json!(
            {"machine": {"model": "2e", "slots": {"0": {"card": "language"}}}}
        ))
        .unwrap_err();
        assert!(err.contains("the //e has no slot 0"), "{err}");
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
