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
    /// The CPU, for the Apple 1 family (`"6502"` or `"65C02"`); when
    /// absent the model decides (Apple 1: 6502, Replica 1: 65C02). The
    /// apple2 family's CPU is a model property and rejects this key.
    pub cpu: Option<CpuModel>,
    /// The //e auxiliary-slot card. Only valid with `"model": "apple2e"`; when
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
    /// The original Apple ][ (6502, Integer BASIC, non-autostart Monitor).
    #[serde(rename = "apple2")]
    Two,
    /// The Apple ][+.
    #[serde(rename = "apple2plus")]
    TwoPlus,
    /// The Apple //e.
    #[serde(rename = "apple2e")]
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
            Model::Two | Model::TwoPlus | Model::TwoE => Family::Apple2,
            Model::Apple1 | Model::Replica1 => Family::Apple1,
        }
    }

    /// The schema token, for error messages.
    pub fn token(self) -> &'static str {
        match self {
            Model::Two => "apple2",
            Model::TwoPlus => "apple2plus",
            Model::TwoE => "apple2e",
            Model::Apple1 => "apple1",
            Model::Replica1 => "replica1",
        }
    }

    /// The `ewm two` machine type; `None` for the one family (callers
    /// turn that into the cross-subcommand error).
    pub fn two_type(self) -> Option<TwoType> {
        match self {
            Model::Two => Some(TwoType::Apple2),
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
        /// May be an `http(s)://` URL, downloaded once into the image
        /// cache and revalidated with ETag / Last-Modified thereafter.
        drive1: Option<String>,
        /// Floppy image for drive 2.
        drive2: Option<String>,
    },
    /// A ProDOS-compatible hard-drive controller.
    Harddrive {
        /// Block image (`.hdv`, `.po`). May be an `http(s)://` URL —
        /// a downloaded volume mounts memory-only, so ProDOS writes do
        /// not persist into the cache.
        image: String,
    },
    /// A UniDisk 3.5 Controller ("Liron") with up to two SmartPort 3.5"
    /// drives taking .2mg images of 400K or 800K.
    Liron {
        /// .2mg image for drive 1. May be an `http(s)://` URL.
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
    /// File whose contents fill the region, or `builtin:<name>` for one
    /// of the embedded ROM images under `roms/` (e.g. `builtin:WozMon`,
    /// `builtin:apple1-basic`, `builtin:Krusader-1.3-65C02`). A region
    /// takes exactly one of `path` or `size`.
    pub path: Option<String>,
    /// Size of an *empty* RAM bank (`"4k"`, `"32k"`, or decimal bytes) —
    /// the Apple 1 family's RAM boards. Only valid with `"type": "ram"`;
    /// a region takes exactly one of `path` or `size`.
    pub size: Option<String>,
}

/// The Apple 1 family's CPU choice (`machine.cpu`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum CpuModel {
    /// The MOS 6502.
    #[serde(rename = "6502")]
    M6502,
    /// The WDC 65C02.
    #[serde(rename = "65C02")]
    M65C02,
}

/// Parse a RAM-bank size: `"4k"` / `"32K"` (KiB) or plain decimal bytes.
pub fn parse_memory_size(s: &str) -> Result<u32, String> {
    let (digits, unit) = match s.strip_suffix(['k', 'K']) {
        Some(digits) => (digits, 1024),
        None => (s, 1),
    };
    let n: u32 = digits
        .parse()
        .map_err(|_| format!("bad size {s:?} (expected e.g. \"4k\", \"32k\", or bytes)"))?;
    let bytes = n
        .checked_mul(unit)
        .filter(|&b| b > 0 && b <= 0x10000)
        .ok_or_else(|| format!("bad size {s:?} (1 byte to 64k)"))?;
    Ok(bytes)
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
    ("apple1", include_str!("../../configs/apple1.json")),
    ("apple2e", include_str!("../../configs/apple2e.json")),
    ("apple2plus", include_str!("../../configs/apple2plus.json")),
    ("replica1", include_str!("../../configs/replica1.json")),
];

/// The mountable one-family ROM images: the files under `roms/` a memory
/// region can name as `"path": "builtin:<name>"` — embedded so built-in
/// configs stay self-contained. Name = the file's stem, sorted, like
/// `BUILTINS`. Provenance and layout: notes/APPLE1.md.
const ROM_BUILTINS: &[(&str, &[u8])] = &[
    (
        "Krusader-1.3-6502",
        include_bytes!("../../roms/Krusader-1.3-6502.rom"),
    ),
    (
        "Krusader-1.3-65C02",
        include_bytes!("../../roms/Krusader-1.3-65C02.rom"),
    ),
    ("WozMon", include_bytes!("../../roms/WozMon.rom")),
    (
        "apple1-basic",
        include_bytes!("../../roms/apple1-basic.rom"),
    ),
];

/// Look up an embedded ROM image by its `builtin:` name.
pub fn rom_builtin(name: &str) -> Result<&'static [u8], String> {
    match ROM_BUILTINS.iter().find(|(n, _)| *n == name) {
        Some((_, data)) => Ok(data),
        None => {
            let names: Vec<&str> = ROM_BUILTINS.iter().map(|(n, _)| *n).collect();
            Err(format!(
                "no built-in ROM {name:?} (available: {})",
                names.join(", ")
            ))
        }
    }
}

/// The bytes a memory region's `path` names: `builtin:<name>` resolves
/// against the embedded ROM images and never touches the filesystem (a
/// literal file named `builtin:x` is reachable as `./builtin:x`, the
/// same escape hatch `--config` has); anything else is a file path.
pub fn read_memory_image(path: &str) -> Result<Vec<u8>, String> {
    match path.strip_prefix("builtin:") {
        Some(name) => Ok(rom_builtin(name)?.to_vec()),
        None => std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}")),
    }
}

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
        // builtin: images are embedded, not files — a config carrying
        // them stays self-contained (size banks have no path at all).
        files.extend(
            machine
                .memory
                .iter()
                .filter_map(|r| r.path.as_deref())
                .filter(|p| !p.starts_with("builtin:")),
        );
    }
    files.extend(config.debug.trace.as_deref());
    files.extend(config.state.path.as_deref());
    files
}

/// The outcome of collecting the config document from a command line's
/// source flags — pass 1 of a subcommand's option parsing, shared by
/// `two` and `one` (plans/20260719-02-one-config.md O3).
pub enum Collected {
    /// The layered document; `None` when the command line had no sources.
    Document(Option<serde_json::Value>),
    /// `builtin:list` was answered on stdout; exit 0, like `--help`.
    Listed,
    /// A source failed to load or apply; the message went to stderr;
    /// exit 1.
    Failed,
    /// A source flag was missing its value; the caller shows its usage.
    MissingValue,
}

/// Collect the config document from `--config`, `--config-overlay`, and
/// `--set`, applied strictly in command-line order through the merge.
/// `seed_model` is the machine the document starts from when overlays or
/// sets appear without a `--config` base (`"apple2plus"` for two,
/// `"replica1"` for one). `materialize_slots` enables the overlay
/// slots-materialization rule — a `two` behavior; a one-family document
/// must never grow the ][+ default table.
pub fn collect_document(args: &[String], seed_model: &str, materialize_slots: bool) -> Collected {
    let seed = || serde_json::json!({"machine": {"model": seed_model}});
    let mut doc: Option<serde_json::Value> = None;
    let mut config_seen = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--config" => {
                let Some(source) = it.next() else {
                    return Collected::MissingValue;
                };
                // `builtin:list` is a query, not a machine: print the
                // embedded configs and exit like --help does.
                if source == "builtin:list" {
                    for (name, description) in builtin_list() {
                        match description {
                            Some(description) => println!("{name:<12}{description}"),
                            None => println!("{name}"),
                        }
                    }
                    return Collected::Listed;
                }
                // One complete machine per command line: two --config files
                // deep-merging reads as an accident now that partial layers
                // have their own flag.
                if config_seen {
                    eprintln!(
                        "only one --config allowed; use --config-overlay for additional layers"
                    );
                    return Collected::Failed;
                }
                config_seen = true;
                match load_source_document(source) {
                    Ok(value) => match doc.as_mut() {
                        Some(doc) => merge_documents(doc, value),
                        None => doc = Some(value),
                    },
                    Err(e) => {
                        eprintln!("{e}");
                        return Collected::Failed;
                    }
                }
            }
            "--config-overlay" => {
                let Some(source) = it.next() else {
                    return Collected::MissingValue;
                };
                match load_overlay_document(source) {
                    Ok(value) => {
                        // Without a --config the document starts from the
                        // default machine, like bare --set does.
                        let doc = doc.get_or_insert_with(seed);
                        if materialize_slots {
                            merge_overlay_document(doc, value);
                        } else {
                            merge_documents(doc, value);
                        }
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        return Collected::Failed;
                    }
                }
            }
            "--set" => {
                let Some(expr) = it.next() else {
                    return Collected::MissingValue;
                };
                let doc = doc.get_or_insert_with(seed);
                if let Err(e) = apply_set(doc, expr) {
                    eprintln!("{e}");
                    return Collected::Failed;
                }
            }
            _ => {}
        }
    }
    Collected::Document(doc)
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
            Value::Array(entries) => {
                // Compact the members (memory regions carry null for the
                // unused path/size half) but never drop one — positions
                // are meaningful.
                for entry in entries.iter_mut() {
                    keep(entry);
                }
                !entries.is_empty()
            }
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
    validate_complete(
        &config,
        "start from --config, e.g. --config builtin:apple2plus",
    )
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
        match (&region.path, &region.size) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(format!(
                    "machine.memory[{i}]: a region takes exactly one of \"path\" or \"size\""
                ));
            }
            (Some(path), None) => {
                // A builtin: image must exist — judged here, per fragment,
                // so a typo'd name fails before any machine is built.
                if let Some(name) = path.strip_prefix("builtin:") {
                    rom_builtin(name).map_err(|e| format!("machine.memory[{i}].path: {e}"))?;
                }
            }
            (None, Some(size)) => {
                if region.kind != MemoryKind::Ram {
                    return Err(format!(
                        "machine.memory[{i}].size: only RAM banks take a size (ROM comes from an image)"
                    ));
                }
                parse_memory_size(size).map_err(|e| format!("machine.memory[{i}].size: {e}"))?;
            }
        }
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
            if machine.cpu.is_some() {
                return Err(format!(
                    "machine.cpu: not configurable for {:?} (the model decides)",
                    model.token()
                ));
            }
            if machine.memory.iter().any(|r| r.size.is_some()) {
                return Err(
                    "machine.memory: RAM banks (size) are an Apple 1 family concept (the ][+ / //e board RAM is fixed)"
                        .into(),
                );
            }
            if machine.aux.is_some() && model != Model::TwoE {
                return Err(
                    "machine.aux: aux cards are a //e feature (model is \"apple2plus\")".into(),
                );
            }
            if model == Model::TwoE && machine.slots.as_ref().is_some_and(|s| s.contains_key("0")) {
                return Err(
                    "machine.slots: the //e has no slot 0 (its language card is built in)".into(),
                );
            }
            // The original Apple ][ had the slot-0 memory-expansion socket
            // too, but wiring a Language Card / Saturn to bank the *Integer*
            // ROM is not done yet — a `2` machine is 48K for now. An
            // explicit "empty" is fine (it is a 48K machine regardless).
            if model == Model::Two
                && let Some(slots) = machine.slots.as_ref()
                && let Some(card) = slots.get("0")
                && !matches!(card, SlotCard::Empty)
            {
                return Err(format!(
                    "machine.slots: slot \"0\" on the original Apple ][ (a memory-expansion \
                     card) is not supported yet — it is a 48K machine (got \"{}\")",
                    card.card_name()
                ));
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
            // debug.trace is fine (one has --trace's machinery); the debug
            // *overlay* is a two frontend feature.
            if config.debug.enabled.is_some() {
                return Err(format!("debug.enabled: not configurable for {token:?}"));
            }
            validate_one_memory_layout(&machine.memory)?;
        }
    }
    Ok(())
}

/// The Apple 1 family's PIA — keyboard in, display out — the one fixed
/// piece of hardware (notes/APPLE1.md).
const ONE_PIA_RANGE: (u32, u32) = (0xd010, 0xd013);

/// Layout checks for Apple 1 family memory regions — which, when
/// present, describe the *whole board* (an absent/empty list means the
/// model's built-in board): regions with a known extent (a `size` bank
/// or a `builtin:` image) must fit the 64K address space and must not
/// overlap each other or the PIA, and something must cover the reset
/// vector or the machine cannot boot. A file image's length is unknown
/// until it is read, so it is judged by its start address only (and
/// given the benefit of the doubt on the vector).
fn validate_one_memory_layout(memory: &[MemoryRegion]) -> Result<(), String> {
    // (index, start, exclusive end when known)
    let mut extents: Vec<(usize, u32, Option<u32>)> = Vec::new();
    for (i, region) in memory.iter().enumerate() {
        let start = u32::from(region.address_value().expect("validated structurally"));
        let len = match (&region.size, &region.path) {
            (Some(size), _) => Some(parse_memory_size(size).expect("validated structurally")),
            (None, Some(path)) => path
                .strip_prefix("builtin:")
                .map(|name| rom_builtin(name).expect("validated structurally").len() as u32),
            (None, None) => unreachable!("validated structurally"),
        };
        let end = match len {
            Some(len) => {
                let end = start + len;
                if end > 0x10000 {
                    return Err(format!(
                        "machine.memory[{i}]: region ${start:04X}+{len} runs past the 64K address space"
                    ));
                }
                Some(end)
            }
            None => None,
        };
        let (pia_start, pia_end) = ONE_PIA_RANGE;
        if start <= pia_end && end.unwrap_or(start + 1) > pia_start {
            return Err(format!(
                "machine.memory[{i}]: region overlaps the PIA at $D010-$D013 (the fixed keyboard/display hardware)"
            ));
        }
        for (j, other_start, other_end) in &extents {
            let overlaps = match (end, other_end) {
                (Some(end), Some(other_end)) => start < *other_end && *other_start < end,
                // Unknown extents: only identical starts are judgeable.
                _ => start == *other_start,
            };
            if overlaps {
                return Err(format!(
                    "machine.memory[{i}]: region overlaps machine.memory[{j}]"
                ));
            }
        }
        extents.push((i, start, end));
    }
    // The board must hold the 6502's reset vector ($FFFC-$FFFD) — a
    // known extent covering it, or a file image that *could* (its length
    // is unknown, so a start at or below $FFFC gets the benefit of the
    // doubt).
    if !memory.is_empty() {
        let covered = extents.iter().any(|(_, start, end)| match end {
            Some(end) => *start <= 0xfffc && *end >= 0xfffe,
            None => *start <= 0xfffc,
        });
        if !covered {
            return Err(
                "machine.memory: nothing covers the reset vector ($FFFC-$FFFD) — the machine \
                 cannot boot; the regions describe the whole board, so include a monitor ROM \
                 (e.g. builtin:WozMon at 0xff00)"
                    .into(),
            );
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
            // builtin: is a scheme, not a relative path.
            if let Some(path) = &mut region.path
                && !path.starts_with("builtin:")
            {
                resolve(base, path);
            }
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
    // An http(s) URL is a source, not a relative path (like builtin:) —
    // it is fetched into the cache when the machine is built.
    if crate::fetch::is_url(p) {
        return;
    }
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
        // tokens (builtin:apple2plus, builtin:apple1, …), both families.
        let models: Vec<&str> = builtin_list().iter().map(|(n, _)| *n).collect();
        assert_eq!(models, vec!["apple1", "apple2e", "apple2plus", "replica1"]);
        let model = |name| load_builtin(name).unwrap().machine.unwrap().model;
        assert_eq!(model("apple2plus"), Some(Model::TwoPlus));
        assert_eq!(model("apple2e"), Some(Model::TwoE));
        assert_eq!(model("apple1"), Some(Model::Apple1));
        assert_eq!(model("replica1"), Some(Model::Replica1));
    }

    #[test]
    fn unknown_builtin_lists_the_available_names() {
        let err = load_builtin("foo").unwrap_err();
        assert_eq!(
            err,
            r#"no built-in config "foo" (available: apple1, apple2e, apple2plus, replica1)"#
        );
    }

    #[test]
    fn memory_regions_take_exactly_path_or_size() {
        // Structural, family-independent rules (R2).
        let err = parse(
            r#"{"machine": {"model": "apple2plus", "memory": [{"type": "ram", "address": "0x4000"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("exactly one of"), "{err}");
        let err = parse(
            r#"{"machine": {"model": "apple2plus",
                "memory": [{"type": "ram", "address": "0x4000", "path": "x.bin", "size": "4k"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("exactly one of"), "{err}");
        let err = parse(
            r#"{"machine": {"model": "apple1",
                "memory": [{"type": "rom", "address": "0x4000", "size": "4k"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("only RAM banks take a size"), "{err}");
        let err = parse(
            r#"{"machine": {"model": "apple1",
                "memory": [{"type": "ram", "address": "0x4000", "size": "huge"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("machine.memory[0].size"), "{err}");
        // A typo'd builtin image fails at parse time, naming the options.
        let err = parse(
            r#"{"machine": {"model": "apple1",
                "memory": [{"type": "rom", "address": "0xff00", "path": "builtin:WozMan"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("no built-in ROM \"WozMan\""), "{err}");
    }

    #[test]
    fn memory_sizes_parse_kib_and_bytes() {
        assert_eq!(parse_memory_size("4k"), Ok(4096));
        assert_eq!(parse_memory_size("32K"), Ok(32768));
        assert_eq!(parse_memory_size("256"), Ok(256));
        assert_eq!(parse_memory_size("64k"), Ok(65536));
        assert!(parse_memory_size("0").is_err());
        assert!(parse_memory_size("65k").is_err());
        assert!(parse_memory_size("lots").is_err());
    }

    #[test]
    fn cpu_and_banks_are_one_family_keys() {
        // machine.cpu picks the Apple 1 family CPU...
        let config = parse(r#"{"machine": {"model": "apple1", "cpu": "65C02"}}"#).expect("cpu");
        assert_eq!(config.machine.unwrap().cpu, Some(CpuModel::M65C02));
        // ...and is rejected for the apple2 family, whose model decides.
        let err = parse(r#"{"machine": {"model": "apple2e", "cpu": "6502"}}"#).unwrap_err();
        assert!(
            err.contains("machine.cpu") && err.contains("apple2e"),
            "{err}"
        );
        // Size banks are Apple 1 family boards.
        let err = parse(
            r#"{"machine": {"model": "apple2plus",
                "memory": [{"type": "ram", "address": "0x4000", "size": "4k"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("RAM banks"), "{err}");
    }

    #[test]
    fn one_memory_layouts_reject_overlaps() {
        // Known extents (banks and builtin images) must not collide with
        // each other, the PIA, or the end of the address space.
        let err = parse(
            r#"{"machine": {"model": "apple1", "memory": [
                {"type": "ram", "address": "0x0000", "size": "8k"},
                {"type": "ram", "address": "0x1000", "size": "4k"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("overlaps machine.memory[0]"), "{err}");
        let err = parse(
            r#"{"machine": {"model": "apple1",
                "memory": [{"type": "ram", "address": "0xd000", "size": "4k"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("overlaps the PIA"), "{err}");
        let err = parse(
            r#"{"machine": {"model": "apple1",
                "memory": [{"type": "rom", "address": "0xff80", "path": "builtin:WozMon"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("64K address space"), "{err}");
        // Two builtin images side by side are fine — the Replica 1 layout.
        assert!(
            parse(
                r#"{"machine": {"model": "replica1", "memory": [
                    {"type": "ram", "address": "0x0000", "size": "32k"},
                    {"type": "rom", "address": "0xe000", "path": "builtin:apple1-basic"},
                    {"type": "rom", "address": "0xf000", "path": "builtin:Krusader-1.3-65C02"}]}}"#,
            )
            .is_ok()
        );
        // File images have unknown extents: only identical starts are
        // judged (the rest is checked when the machine is built).
        let err = parse(
            r#"{"machine": {"model": "apple1", "memory": [
                {"type": "rom", "address": "0xc000", "path": "a.bin"},
                {"type": "rom", "address": "0xc000", "path": "b.bin"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("overlaps machine.memory[0]"), "{err}");
        // The apple2 family keeps its regions unchecked (extras on a
        // fixed board — the machine builders own that layout).
        assert!(
            parse(
                r#"{"machine": {"model": "apple2plus", "memory": [
                    {"type": "rom", "address": "0xd000", "path": "a.bin"},
                    {"type": "rom", "address": "0xd000", "path": "b.bin"}]}}"#,
            )
            .is_ok()
        );
    }

    #[test]
    fn rom_builtins_resolve_and_list() {
        assert_eq!(rom_builtin("WozMon").unwrap().len(), 256);
        assert_eq!(rom_builtin("apple1-basic").unwrap().len(), 4096);
        assert_eq!(rom_builtin("Krusader-1.3-6502").unwrap().len(), 4096);
        assert_eq!(rom_builtin("Krusader-1.3-65C02").unwrap().len(), 4096);
        // Names are exact (= the file stems), and unknowns list them all.
        let err = rom_builtin("wozmon").unwrap_err();
        assert_eq!(
            err,
            "no built-in ROM \"wozmon\" (available: Krusader-1.3-6502, \
             Krusader-1.3-65C02, WozMon, apple1-basic)"
        );
        // The table stays sorted so the listing reads predictably.
        assert!(ROM_BUILTINS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn rom_decomposition_matches_the_historical_images() {
        // Provenance (notes/APPLE1.md): BASIC + the 6502 Krusader slice
        // reassemble the historical 8KB krusader.rom; WozMon is the
        // historical apple1.rom; BASIC + the 65C02 slice reassemble the
        // Krusader 1.3 65C02 distribution — all pinned by SHA-1 (the
        // crate's own implementation, RFC-vector-tested in ws.rs).
        fn hex(digest: [u8; 20]) -> String {
            digest.iter().map(|b| format!("{b:02x}")).collect()
        }
        let sha1 = |data: &[u8]| hex(crate::ws::sha1(data));

        let mut image = rom_builtin("apple1-basic").unwrap().to_vec();
        image.extend_from_slice(rom_builtin("Krusader-1.3-6502").unwrap());
        assert_eq!(sha1(&image), "5e5ca9d94bc83a79e06806a9df180aa29d8e1a0a");

        assert_eq!(
            sha1(rom_builtin("WozMon").unwrap()),
            "224767aa499dc98767e042f375ced1359be8a35f"
        );

        let mut image = rom_builtin("apple1-basic").unwrap().to_vec();
        image.extend_from_slice(rom_builtin("Krusader-1.3-65C02").unwrap());
        assert_eq!(sha1(&image), "f038b2d8761171ff770ce032ce0a22918cc96872");
    }

    #[test]
    fn memory_images_resolve_builtins_without_the_filesystem() {
        // builtin: resolves against the embedded table, never the
        // filesystem: a valid name yields the image, an unknown name gets
        // the registry error — not a "cannot read" from a file probe.
        assert_eq!(read_memory_image("builtin:WozMon").unwrap().len(), 256);
        let err = read_memory_image("builtin:nope").unwrap_err();
        assert!(err.starts_with("no built-in ROM"), "{err}");

        // The escape hatch: a path *containing* a directory component is
        // a file, even if the file is literally named builtin:WozMon.
        let junk = scratch("builtin:WozMon", "junk");
        let data = read_memory_image(junk.to_str().unwrap()).expect("literal file loads");
        assert_eq!(data, b"junk");

        // A builtin: region path is a scheme, not a relative path: it
        // survives per-file path resolution untouched...
        let config = parse(
            r#"{"machine": {"model": "apple2plus",
                "memory": [{"type": "rom", "address": "0xd000", "path": "builtin:WozMon"}]}}"#,
        )
        .expect("builtin memory path parses");
        let machine = config.machine.as_ref().expect("machine");
        assert_eq!(machine.memory[0].path.as_deref(), Some("builtin:WozMon"));
        // ...and does not count as a file reference (self-containment).
        assert_eq!(referenced_files(&config), Vec::<&str>::new());
    }

    #[test]
    fn referenced_files_finds_every_path_field() {
        let config = parse(
            r#"{"machine": {"model": "apple2plus",
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
        let builtin = load_source_document("builtin:apple2plus").expect("builtin source");
        let file = load_document(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../configs/apple2plus.json"
        ))
        .expect("file source");
        // The embedded copy and the committed file are the same config.
        assert_eq!(builtin, file);
        let err = load_source_document("builtin:nope").unwrap_err();
        assert!(err.starts_with("no built-in config"), "{err}");
    }

    #[test]
    fn minimal_config_parses() {
        let config = parse(r#"{"machine": {"model": "apple2plus"}}"#).expect("minimal config");
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
        let err = parse(r#"{"machine": {"model": "apple2plus"}, "monitr": {}}"#).unwrap_err();
        assert!(err.contains("unknown field `monitr`"), "{err}");
        assert!(err.starts_with("test.json:"), "{err}");
    }

    #[test]
    fn unknown_slot_card_key_is_rejected() {
        // The canary for serde's internally-tagged deny_unknown_fields
        // behavior (see notes/JSON_CONFIG.md).
        let err = parse(
            r#"{"machine": {"model": "apple2plus",
                "slots": {"6": {"card": "diskii", "driv1": "x.dsk"}}}}"#,
        )
        .unwrap_err();
        assert!(err.contains("driv1"), "{err}");
    }

    #[test]
    fn bad_values_are_rejected_with_expected_lists() {
        let err = parse(r#"{"machine": {"model": "2gs"}}"#).unwrap_err();
        assert!(
            err.contains("apple2plus") && err.contains("apple2e"),
            "{err}"
        );

        let err = parse(r#"{"machine": {"model": "apple2plus"}, "display": {"monitor": "blue"}}"#)
            .unwrap_err();
        assert!(err.contains("green") && err.contains("rgb"), "{err}");

        let err =
            parse(r#"{"machine": {"model": "apple2plus"}, "cpu": {"speed": "2mhz"}}"#).unwrap_err();
        assert!(err.contains("normal") && err.contains("3.58mhz"), "{err}");
    }

    #[test]
    fn slot_rules() {
        let slot = |n: &str, card: &str| {
            parse(&format!(
                r#"{{"machine": {{"model": "apple2plus", "slots": {{"{n}": {card}}}}}}}"#
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
        let err =
            parse(r#"{"machine": {"model": "apple2e", "slots": {"0": {"card": "language"}}}}"#)
                .unwrap_err();
        assert!(
            err.contains("the //e has no slot 0 (its language card is built in)"),
            "{err}"
        );

        // Multiplicity: at most three Disk ][ controllers, one Thunderclock.
        let err = parse(
            r#"{"machine": {"model": "apple2plus", "slots": {
                "3": {"card": "diskii"}, "4": {"card": "diskii"},
                "5": {"card": "diskii"}, "6": {"card": "diskii"}}}}"#,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "test.json: machine.slots: at most three Disk ][ controllers"
        );
        let err = parse(
            r#"{"machine": {"model": "apple2plus", "slots": {
                "1": {"card": "thunderclock"}, "2": {"card": "thunderclock"}}}}"#,
        )
        .unwrap_err();
        assert_eq!(err, "test.json: machine.slots: at most one Thunderclock");

        // Three controllers and two hard drives are fine.
        assert!(
            parse(
                r#"{"machine": {"model": "apple2plus", "slots": {
                    "4": {"card": "diskii"}, "5": {"card": "diskii"},
                    "6": {"card": "diskii"}, "2": {"card": "harddrive", "image": "a.hdv"},
                    "7": {"card": "harddrive", "image": "b.hdv"}}}}"#,
            )
            .is_ok()
        );

        // A present-but-empty table is a bare machine, distinct from an
        // absent one (the default layout).
        let config = parse(r#"{"machine": {"model": "apple2plus", "slots": {}}}"#).expect("empty");
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
        assert_eq!(machine.memory[0].path.as_deref(), Some("/cfg/basic.rom"));
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
        // debug.trace is valid for the family; the debug *overlay* is a
        // two frontend feature (O3).
        case(
            r#"{"machine": {"model": "apple1"}, "debug": {"enabled": true}}"#,
            "debug.enabled",
        );
        assert!(parse(r#"{"machine": {"model": "apple1"}, "debug": {"trace": "t.txt"}}"#).is_ok());
    }

    #[test]
    fn aux_rules() {
        let aux = |model: &str, aux: &str| {
            parse(&format!(
                r#"{{"machine": {{"model": "{model}", "aux": {aux}}}}}"#
            ))
        };

        let err = aux("apple2e", r#"{"card": "80col", "size": "1m"}"#).unwrap_err();
        assert!(err.contains("only valid with"), "{err}");
        let err = aux("apple2plus", r#"{"card": "80col"}"#).unwrap_err();
        assert!(err.contains("//e feature"), "{err}");
        let err = aux("apple2e", r#"{"card": "ramworksiii", "size": "3k"}"#).unwrap_err();
        assert!(err.contains("multiple of 64k"), "{err}");

        let config = aux("apple2e", r#"{"card": "ramworksiii", "size": "1m"}"#).expect("valid aux");
        let aux = config
            .machine
            .expect("machine present")
            .aux
            .expect("aux present");
        assert_eq!(aux.card, AuxKind::RamWorksIII);
        assert_eq!(aux.size.as_deref(), Some("1m"));
    }

    #[test]
    fn http_urls_are_sources_not_relative_paths() {
        // A disk image may be an http(s) URL: it must survive per-file
        // path resolution untouched (like builtin:), to be downloaded
        // into the cache when the machine is built.
        let config = parse(
            r#"{"machine": {"model": "apple2plus", "slots": {
                "6": {"card": "diskii", "drive1": "https://x.test/games/Frogger.dsk"},
                "7": {"card": "harddrive", "image": "http://x.test/Total%20Replay.hdv"}}}}"#,
        )
        .expect("URL media parses");
        let slots = config.machine.as_ref().unwrap().slots.as_ref().unwrap();
        let SlotCard::Diskii { drive1, .. } = &slots["6"] else {
            panic!("slot 6 should be a diskii");
        };
        assert_eq!(drive1.as_deref(), Some("https://x.test/games/Frogger.dsk"));
        let SlotCard::Harddrive { image } = &slots["7"] else {
            panic!("slot 7 should be a harddrive");
        };
        assert_eq!(image, "http://x.test/Total%20Replay.hdv");
        // A URL *is* an external reference, so a built-in config may not
        // carry one (builtins must run offline).
        assert_eq!(referenced_files(&config).len(), 2);
    }

    #[test]
    fn relative_paths_resolve_against_the_config_dir() {
        let config = parse(
            r#"{"machine": {"model": "apple2plus",
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
        assert_eq!(machine.memory[0].path.as_deref(), Some("/cfg/roms/x.bin"));
        assert_eq!(config.debug.trace.as_deref(), Some("/cfg/trace.txt"));
    }

    #[test]
    fn memory_addresses_accept_hex_and_decimal() {
        let region = |address: &str| MemoryRegion {
            kind: MemoryKind::Rom,
            address: address.to_string(),
            path: Some("x.bin".to_string()),
            size: None,
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
            "machine": {"model": "apple2plus", "slots": {"6": {"card": "diskii", "drive1": "a.dsk"}}},
            "display": {"monitor": "green"},
        });
        merge_documents(
            &mut doc,
            serde_json::json!({
                "machine": {"model": "apple2e", "slots": {"6": {"drive2": "b.dsk"}}, "aux": null, "memory": []},
                "display": {"monitor": null, "fps": 30},
            }),
        );
        assert_eq!(
            doc,
            serde_json::json!({
                "machine": {"model": "apple2e", "slots": {"6": {"card": "diskii", "drive1": "a.dsk", "drive2": "b.dsk"}}},
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus", "slots": {"6": {"card": "diskii"}}}});
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
        merge_overlay_document(
            &mut doc,
            serde_json::json!({"display": {"monitor": "amber"}}),
        );
        assert_eq!(doc["machine"], serde_json::json!({"model": "apple2plus"}));
        assert_eq!(doc["display"]["monitor"], serde_json::json!("amber"));

        // 4. ...and a base's explicit table is untouched.
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus", "slots": {}}});
        merge_overlay_document(&mut doc, serde_json::json!({"cpu": {"strict": true}}));
        assert_eq!(doc["machine"]["slots"], serde_json::json!({}));

        // A null machine.slots (how a fragment that never mentioned slots
        // serializes) is a merge no-op, not a table.
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
        merge_overlay_document(
            &mut doc,
            serde_json::json!({"machine": {"model": "apple2e", "slots": null}}),
        );
        assert_eq!(doc["machine"], serde_json::json!({"model": "apple2e"}));
    }

    #[test]
    fn compact_document_drops_noise_but_keeps_bare_tables() {
        let mut doc = serde_json::json!({
            "machine": {
                "model": "apple2plus",
                "aux": null,
                "slots": {"6": {"card": "diskii", "drive1": "a.dsk", "drive2": null}},
                "memory": [{"type": "ram", "address": "0x0000", "path": null, "size": "4k"}],
            },
            "display": {"monitor": "green", "scanlines": null, "fps": null},
            "input": {"controller": null},
        });
        compact_document(&mut doc);
        assert_eq!(
            doc,
            serde_json::json!({
                "machine": {
                    "model": "apple2plus",
                    "slots": {"6": {"card": "diskii", "drive1": "a.dsk"}},
                    "memory": [{"type": "ram", "address": "0x0000", "size": "4k"}],
                },
                "display": {"monitor": "green"},
            })
        );
        // An empty memory list still compacts away entirely.
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus", "memory": []}});
        compact_document(&mut doc);
        assert_eq!(doc, serde_json::json!({"machine": {"model": "apple2plus"}}));

        // An explicit bare slots table survives: {} means "no cards",
        // where an absent table would mean the default layout.
        let mut doc = serde_json::json!({
            "machine": {"model": "apple2plus", "slots": {}},
            "cpu": {"speed": null, "strict": null},
        });
        compact_document(&mut doc);
        assert_eq!(
            doc,
            serde_json::json!({"machine": {"model": "apple2plus", "slots": {}}})
        );
    }

    #[test]
    fn overlay_sources_resolve_builtins_and_fragments() {
        // The builtin: scheme is shared with --config...
        let builtin = load_overlay_document("builtin:apple2plus").expect("builtin overlay");
        assert_eq!(builtin["machine"]["model"], serde_json::json!("apple2plus"));
        let err = load_overlay_document("builtin:nope").unwrap_err();
        assert!(err.starts_with("no built-in config"), "{err}");
        // ...while a file may be arbitrarily partial.
        let path = scratch("frag.json", r#"{"display": {"monitor": "amber"}}"#);
        let doc = load_overlay_document(path.to_str().unwrap()).expect("fragment loads");
        assert_eq!(doc["display"]["monitor"], serde_json::json!("amber"));
    }

    #[test]
    fn set_types_values_as_json_or_string() {
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus", "slots": {}}});
        apply_set(&mut doc, "machine:slots:6:drive1=x.dsk").unwrap();
        assert_eq!(
            doc["machine"]["slots"],
            serde_json::json!({"6": {"drive1": "x.dsk"}})
        );
    }

    #[test]
    fn set_replaces_a_slot_whose_card_changes() {
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
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
        let doc = serde_json::json!({"machine": {"model": "apple2plus"}, "disply": {}});
        let err = from_document(doc).unwrap_err();
        assert!(err.starts_with("config:"), "{err}");
        assert!(err.contains("unknown field `disply`"), "{err}");

        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
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
                       (start from --config, e.g. --config builtin:apple2plus)";
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
        let mut doc = serde_json::json!({"machine": {"model": "apple2plus"}});
        merge_documents(&mut doc, fragment.clone());
        let err = from_document(doc).unwrap_err();
        assert!(err.contains("//e feature"), "{err}");
        let mut doc = serde_json::json!({"machine": {"model": "apple2e"}});
        merge_documents(&mut doc, fragment);
        assert!(from_document(doc).is_ok());

        // Same for the //e's missing slot 0.
        let err = from_document(serde_json::json!(
            {"machine": {"model": "apple2e", "slots": {"0": {"card": "language"}}}}
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
