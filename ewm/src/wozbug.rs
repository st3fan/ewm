//! WozBug: the minimal always-present monitor (notes/DEBUGGING_TOOLS.md).
//! MicroBug in role — the debugger you can count on being there (TN1136) —
//! and Woz Monitor in dialect: `280.29F` dumps, `300:A9 20` deposits,
//! `300G` goes, and unknown input answers `?`.
//!
//! One command core with three frontends: tests call `WozBug::execute`
//! directly, the `--wozbug` line server feeds it over TCP, and a
//! breakpoint hit drops the SDL loop into it. `execute` never runs the
//! machine on its own beyond `S` stepping — `G` clears the stopped state
//! and leaves the running to the caller.
//!
//! Memory access goes through the bus, exactly like the real Monitor: a
//! dump of `C000.C0FF` reads the soft switches *and trips them*. That is
//! authentic and occasionally what you want, but know it before you dump
//! I/O space.

use ewm_core::fmt::{format_instruction, format_state};

use crate::two::Two;

/// The built-in symbol table: where this machine's software famously
/// lives. MicroBug scope — Monitor, DOS 3.3 and ProDOS landmarks, not
/// per-program symbols. Sorted by address.
const SYMBOLS: &[(u16, &str)] = &[
    (0x03d0, "DOSWARM"), // DOS 3.3 warm-start vector
    (0x03d3, "DOSCOLD"), // DOS 3.3 cold-start vector
    (0xb7e8, "IOB"),     // DOS 3.3 RWTS I/O block
    (0xbd00, "RWTS"),    // DOS 3.3 read/write track-sector
    (0xbf00, "MLI"),     // ProDOS machine-language interface
    (0xe000, "BASIC"),   // AppleSoft cold entry
    (0xfa62, "RESET"),   // Monitor reset handler
    (0xfc58, "HOME"),    // Monitor clear screen
    (0xfca8, "WAIT"),    // Monitor delay loop
    (0xfd0c, "RDKEY"),   // Monitor read key
    (0xfd6a, "GETLN"),   // Monitor read line
    (0xfded, "COUT"),    // Monitor character out
    (0xff3a, "BELL"),    // Monitor beep
    (0xff69, "MON"),     // the Monitor itself (CALL -151)
];

/// `BD25` → `Some("RWTS+$25")`; exact hits drop the offset. Only names
/// within $FF of a landmark — past that the guess is worse than silence.
pub fn symbolize(addr: u16) -> Option<String> {
    let idx = SYMBOLS.partition_point(|&(a, _)| a <= addr);
    if idx == 0 {
        return None;
    }
    let (base, name) = SYMBOLS[idx - 1];
    match addr - base {
        0 => Some(name.to_string()),
        off if off < 0x100 => Some(format!("{name}+${off:02X}")),
        _ => None,
    }
}

/// Parse an address: hex (no `$`/`0x` needed, both tolerated) or a symbol
/// name from the built-in table. Public so `--break RWTS` works.
pub fn parse_addr(s: &str) -> Option<u16> {
    let s = s.trim();
    if let Some((_, name)) = SYMBOLS
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(s))
    {
        return SYMBOLS
            .iter()
            .find(|(_, n)| n == name)
            .map(|&(addr, _)| addr);
    }
    let hex = s
        .strip_prefix('$')
        .or_else(|| s.strip_prefix("0x"))
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u16::from_str_radix(hex, 16).ok()
}

/// How many bytes a bare Return continues dumping (the MicroBug trick:
/// the dot address remembers where the last dump stopped).
const CONTINUE_BYTES: u16 = 64;

pub struct WozBug {
    /// The dot address: one past the last byte dumped or deposited.
    dot: u16,
    /// Where a bare `L` continues disassembling.
    lst: Option<u16>,
}

impl Default for WozBug {
    fn default() -> WozBug {
        WozBug::new()
    }
}

impl WozBug {
    pub fn new() -> WozBug {
        WozBug { dot: 0, lst: None }
    }

    /// Execute one command line against the machine; the reply is the
    /// text to show (no trailing newline guaranteed; may be empty).
    pub fn execute(&mut self, two: &mut Two, line: &str) -> String {
        let line = line.trim();

        // Bare Return: continue the last dump.
        if line.is_empty() {
            let start = self.dot;
            return self.dump(two, start, start.saturating_add(CONTINUE_BYTES - 1));
        }

        // Deposit: ADDR:BB BB … (a bare ":BB" continues at the dot).
        if let Some((addr, bytes)) = line.split_once(':') {
            return self.deposit(two, addr, bytes);
        }

        let upper = line.to_ascii_uppercase();

        // Keyword commands.
        if upper == "R" {
            return registers(two);
        }
        if upper == "G" {
            two.cpu.resume();
            return String::new();
        }
        if let Some(rest) = upper.strip_prefix("S") {
            let rest = rest.trim();
            if rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit()) {
                let n = rest.parse::<u32>().unwrap_or(1).clamp(1, 0x10000);
                return step(two, n);
            }
        }
        match upper.as_str() {
            "DSK" => return dsk(two),
            "SW" => return switches(two),
            "TEXT" => return text(two),
            "SLOTS" => return slots(two),
            "T" => {
                two.cpu.trace = Some(Box::new(std::io::stderr()));
                return "trace on (stderr)".to_string();
            }
            "T-" => {
                two.cpu.trace = None;
                return "trace off".to_string();
            }
            "?" | "HELP" => return HELP.trim().to_string(),
            _ => {}
        }
        // B/W/L take their argument after a space or '-' — never glued,
        // because `B` is a hex digit and `BD00` must examine memory, not
        // set a breakpoint at $D00.
        if let Some(rest) = command_arg(&upper, 'B') {
            return self.breakpoints(two, rest);
        }
        if let Some(rest) = command_arg(&upper, 'W') {
            return self.watchpoints(two, rest);
        }
        if let Some(rest) = command_arg(&upper, 'L') {
            return self.list(two, rest);
        }

        // Register set: PC=BD00, A=FF, X=, Y=, SP=, P=.
        if let Some((reg, value)) = line.split_once('=') {
            return set_register(two, reg.trim(), value.trim());
        }

        // ADDRG: go from an address.
        if let Some(addr) = upper.strip_suffix('G').and_then(parse_addr) {
            two.cpu.pc = addr;
            two.cpu.resume();
            return String::new();
        }

        // ADDR.ADDR: dump a range.
        if let Some((from, to)) = line.split_once('.')
            && let (Some(from), Some(to)) = (parse_addr(from), parse_addr(to))
        {
            return self.dump(two, from, to.max(from));
        }

        // ADDR: examine one byte.
        if let Some(addr) = parse_addr(line) {
            return self.dump(two, addr, addr);
        }

        "?".to_string() // the Monitor's own error message
    }

    /// Woz Monitor dump: `0280- A9 20 8D 45 03`, eight bytes per line.
    /// Reads go through the bus (soft switches included). Sets the dot.
    fn dump(&mut self, two: &mut Two, from: u16, to: u16) -> String {
        let mut out = String::new();
        let mut addr = from;
        loop {
            if addr == from || addr.is_multiple_of(8) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&format!("{addr:04X}-"));
            }
            out.push_str(&format!(" {:02X}", two.cpu.mem.read(addr)));
            if addr == to {
                break;
            }
            addr = addr.wrapping_add(1);
            if addr == 0 {
                break; // wrapped past $FFFF
            }
        }
        self.dot = to.wrapping_add(1);
        out
    }

    /// Deposit `ADDR:BB BB …`; an empty ADDR continues at the dot. Echoes
    /// a dump of what was written.
    fn deposit(&mut self, two: &mut Two, addr: &str, bytes: &str) -> String {
        let start = if addr.trim().is_empty() {
            self.dot
        } else {
            match parse_addr(addr) {
                Some(addr) => addr,
                None => return "?".to_string(),
            }
        };
        let mut values = Vec::new();
        for token in bytes.split_whitespace() {
            match u8::from_str_radix(token, 16) {
                Ok(b) => values.push(b),
                Err(_) => return "?".to_string(),
            }
        }
        if values.is_empty() {
            return "?".to_string();
        }
        for (i, &b) in values.iter().enumerate() {
            two.cpu.mem.write(start.wrapping_add(i as u16), b);
        }
        self.dump(two, start, start.wrapping_add(values.len() as u16 - 1))
    }

    /// `W` list, `W ADDR[.ADDR]` watch a range, `W-ADDR[.ADDR]` clear,
    /// `W-` clear all. The CPU stops *after* the instruction that touches
    /// a watched address (fetches count — executing watched code stops
    /// too).
    fn watchpoints(&mut self, two: &mut Two, rest: &str) -> String {
        fn parse_range(s: &str) -> Option<(u16, u16)> {
            match s.split_once('.') {
                Some((from, to)) => Some((parse_addr(from)?, parse_addr(to)?)),
                None => parse_addr(s).map(|addr| (addr, addr)),
            }
        }
        fn range_name((from, to): (u16, u16)) -> String {
            if from == to {
                name_of(from)
            } else {
                format!("{from:04X}.{to:04X}")
            }
        }
        if let Some(rest) = rest.strip_prefix('-') {
            let rest = rest.trim();
            if rest.is_empty() {
                two.cpu.mem.clear_watchpoints();
                return "all watchpoints cleared".to_string();
            }
            return match parse_range(rest) {
                Some((from, to)) => {
                    two.cpu.mem.remove_watchpoint(from, to);
                    format!("watchpoint cleared at {}", range_name((from, to)))
                }
                None => "?".to_string(),
            };
        }
        if rest.is_empty() {
            let list = two.cpu.mem.watchpoints();
            if list.is_empty() {
                return "no watchpoints".to_string();
            }
            return list
                .iter()
                .map(|&range| range_name(range))
                .collect::<Vec<_>>()
                .join("\n");
        }
        match parse_range(rest) {
            Some((from, to)) => {
                two.cpu.mem.add_watchpoint(from, to);
                format!("watchpoint set at {}", range_name((from, to)))
            }
            None => "?".to_string(),
        }
    }

    /// `L [addr]`: disassemble sixteen instructions; a bare `L` continues.
    fn list(&mut self, two: &mut Two, rest: &str) -> String {
        let mut addr = match parse_addr(rest) {
            Some(addr) => addr,
            None => self.lst.unwrap_or(two.cpu.pc),
        };
        let mut out = String::new();
        for _ in 0..16 {
            let (line, next) = ewm_core::fmt::disassemble(&mut two.cpu, addr);
            out.push_str(&format!("{addr:04X}: {line}\n"));
            addr = next;
        }
        self.lst = Some(addr);
        out.trim_end().to_string()
    }

    /// `B` list, `B ADDR` set, `B-ADDR` clear, `B-` clear all. Addresses
    /// may be symbols (`B RWTS`).
    fn breakpoints(&mut self, two: &mut Two, rest: &str) -> String {
        if let Some(rest) = rest.strip_prefix('-') {
            let rest = rest.trim();
            if rest.is_empty() {
                for addr in two.cpu.breakpoints().to_vec() {
                    two.cpu.remove_breakpoint(addr);
                }
                return "all breakpoints cleared".to_string();
            }
            return match parse_addr(rest) {
                Some(addr) => {
                    two.cpu.remove_breakpoint(addr);
                    format!("breakpoint cleared at {}", name_of(addr))
                }
                None => "?".to_string(),
            };
        }
        if rest.is_empty() {
            let list = two.cpu.breakpoints();
            if list.is_empty() {
                return "no breakpoints".to_string();
            }
            return list
                .iter()
                .map(|&addr| name_of(addr))
                .collect::<Vec<_>>()
                .join("\n");
        }
        match parse_addr(rest) {
            Some(addr) => {
                two.cpu.add_breakpoint(addr);
                format!("breakpoint set at {}", name_of(addr))
            }
            None => "?".to_string(),
        }
    }
}

/// Match a one-letter command: bare (`B`), with a clear-marker (`B-…`),
/// or space-separated (`B 300`). Returns the argument (the `-` kept).
fn command_arg(upper: &str, letter: char) -> Option<&str> {
    let rest = upper.strip_prefix(letter)?;
    if rest.is_empty() || rest.starts_with('-') {
        Some(rest)
    } else {
        rest.strip_prefix(' ').map(str::trim)
    }
}

/// `BD25 (RWTS+$25)` or plain `BD25`.
fn name_of(addr: u16) -> String {
    match symbolize(addr) {
        Some(name) => format!("{addr:04X} ({name})"),
        None => format!("{addr:04X}"),
    }
}

/// What the frontends announce when the machine halts on a breakpoint or
/// watchpoint.
pub fn stopped_banner(two: &mut Two) -> String {
    let mut out = String::new();
    if let Some(hit) = two.cpu.watch_stop() {
        out.push_str(&format!(
            "watch: {} {} = {:02X}\n",
            if hit.write { "write" } else { "read" },
            name_of(hit.addr),
            hit.value,
        ));
    }
    out.push_str(&format!(
        "stopped at {}\n{}",
        name_of(two.cpu.pc),
        registers(two)
    ));
    out
}

/// The MicroBug `TD` analogue, in this machine's terms.
fn registers(two: &mut Two) -> String {
    let pc = two.cpu.pc;
    let stopped = if two.cpu.stopped() { "  [stopped]" } else { "" };
    format!("PC={} {}{}", name_of(pc), format_state(&two.cpu), stopped)
}

/// Step n instructions, one trace-style line each. Resumes first if
/// stopped, and reports a breakpoint hit that cuts the run short.
fn step(two: &mut Two, n: u32) -> String {
    let mut out = String::new();
    if two.cpu.stopped() {
        two.cpu.resume();
    }
    for _ in 0..n {
        let pc = two.cpu.pc;
        let instruction = format_instruction(&mut two.cpu);
        if two.cpu.step() == 0 {
            out.push_str(&format!("stopped at {}", name_of(two.cpu.pc)));
            return out;
        }
        out.push_str(&format!(
            "{pc:04X}: {instruction:<24} {}\n",
            format_state(&two.cpu)
        ));
    }
    out.push_str(&registers(two));
    out
}

fn set_register(two: &mut Two, reg: &str, value: &str) -> String {
    let Some(addr) = parse_addr(value) else {
        return "?".to_string();
    };
    match reg.to_ascii_uppercase().as_str() {
        "PC" => two.cpu.pc = addr,
        "A" => two.cpu.a = addr as u8,
        "X" => two.cpu.x = addr as u8,
        "Y" => two.cpu.y = addr as u8,
        "SP" => two.cpu.sp = addr as u8,
        "P" => two.cpu.set_status(addr as u8),
        _ => return "?".to_string(),
    }
    registers(two)
}

/// Every Disk II controller's state — the command PR #253 needed.
fn dsk(two: &mut Two) -> String {
    let cycles = two.cpu.counter;
    let mut out = Vec::new();
    for slot in 1..=7u8 {
        if let Some(dsk) = two.dsk_at(slot) {
            let drive = dsk.active_drive();
            let half = dsk.half_track();
            out.push(format!(
                "S{slot}: drive {} selected, track {}{}, motor {}, D1 {}, D2 {}{}",
                drive + 1,
                half / 2,
                if half % 2 == 1 { ".5" } else { "" },
                if dsk.motor_lit(cycles) { "on" } else { "off" },
                if dsk.drive_loaded(0) {
                    "loaded"
                } else {
                    "empty"
                },
                if dsk.drive_loaded(1) {
                    "loaded"
                } else {
                    "empty"
                },
                if two.boot_disk_slot() == Some(slot) {
                    "  [boot]"
                } else {
                    ""
                },
            ));
        }
    }
    if out.is_empty() {
        return "no Disk II controllers".to_string();
    }
    out.join("\n")
}

/// The display soft switches, decoded.
fn switches(two: &mut Two) -> String {
    format!(
        "mode={:?} graphics={:?}/{:?} page={:?} 80col={} altchar={} dhires={} key=${:02X}",
        two.screen_mode(),
        two.screen_graphics_mode(),
        two.screen_graphics_style(),
        two.screen_page(),
        two.col80(),
        two.alt_charset(),
        two.dhires(),
        two.key_register(),
    )
}

/// The rendered text screen (80 columns when the switch is on).
fn text(two: &mut Two) -> String {
    if two.col80() {
        two.text_screen_80()
    } else {
        two.text_screen()
    }
}

/// The machine's slot table.
fn slots(two: &mut Two) -> String {
    let mut out = Vec::new();
    // Slot 0 is the ][+ language-card socket; the //e's card is built into
    // the motherboard, so no slot 0 line there.
    if two.model() == crate::two::TwoType::Apple2Plus {
        let what = if two.language_card() {
            "language card (16K)"
        } else {
            "-"
        };
        out.push(format!("S0: {what}"));
    }
    for slot in 1..=7u8 {
        let what = if two.dsk_at(slot).is_some() {
            "Disk II".to_string()
        } else if let Some(hdd) = two.hdd_at(slot) {
            format!("hard drive ({} blocks)", hdd.blocks())
        } else if two.clock_slot() == Some(slot) {
            "Thunderclock".to_string()
        } else {
            "-".to_string()
        };
        out.push(format!("S{slot}: {what}"));
    }
    out.join("\n")
}

/// The `--wozbug` line server: one TCP client at a time on 127.0.0.1,
/// newline-delimited commands in, replies out, `nc`-friendly. The server
/// threads only move strings — the emulator loop drains `commands` and
/// executes them between frames, so the machine is never touched off its
/// own thread and an idle server costs nothing per frame beyond one
/// `try_recv`.
pub struct Server {
    pub commands: std::sync::mpsc::Receiver<String>,
    replies: std::sync::mpsc::Sender<String>,
    port: u16,
}

impl Server {
    /// Bind 127.0.0.1:`port` (0 picks an ephemeral port — tests) and start
    /// the connection threads.
    pub fn start(port: u16) -> std::io::Result<Server> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
        let port = listener.local_addr()?.port();
        let (cmd_tx, commands) = std::sync::mpsc::channel();
        let (replies, resp_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || serve(listener, cmd_tx, resp_rx));
        Ok(Server {
            commands,
            replies,
            port,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Send a reply (or an unsolicited announcement) followed by the
    /// Monitor's `*` prompt. Dropped silently when no client is listening.
    pub fn reply(&self, text: &str) {
        let framed = if text.is_empty() {
            "* ".to_string()
        } else {
            format!("{text}\n* ")
        };
        let _ = self.replies.send(framed);
    }
}

/// One client at a time: a reader thread feeds the command channel; this
/// thread writes replies until the client goes away, then accepts the
/// next connection. `recv_timeout` lets a silent disconnect (reader saw
/// EOF, no replies pending) release the writer to accept again.
fn serve(
    listener: std::net::TcpListener,
    commands: std::sync::mpsc::Sender<String>,
    replies: std::sync::mpsc::Receiver<String>,
) {
    use std::io::{BufRead, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        if stream
            .write_all(b"WozBug \xE2\x80\x94 ? for help\n* ")
            .is_err()
        {
            continue;
        }
        let Ok(reader) = stream.try_clone() else {
            continue;
        };
        let alive = Arc::new(AtomicBool::new(true));
        let reader_alive = alive.clone();
        let tx = commands.clone();
        let reader_thread = std::thread::spawn(move || {
            for line in std::io::BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
            reader_alive.store(false, Ordering::Relaxed);
        });
        loop {
            match replies.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(msg) => {
                    if stream.write_all(msg.as_bytes()).is_err() {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !alive.load(Ordering::Relaxed) {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        let _ = stream.shutdown(std::net::Shutdown::Both);
        let _ = reader_thread.join();
    }
}

const HELP: &str = "
WozBug (see notes/DEBUGGING_TOOLS.md)
  280.29F      dump a range          280        examine one byte
  (return)     continue the dump     300:A9 20  deposit bytes
  R            registers             PC=BD00    set a register (A X Y SP P)
  S [n]        step n instructions   G / 300G   resume / go from address
  B [addr]     set/list breakpoints  B-addr     clear one (B- clears all)
  W [a[.b]]    watch memory access   W-a        clear (stops after the
                                     touching instruction; fetches count)
  L [addr]     disassemble 16 (bare L continues)
  T / T-       CPU trace to stderr on/off
  DSK SW TEXT SLOTS                  machine state (controllers, switches,
                                     screen, slot table)
Addresses are hex; Monitor/DOS/ProDOS symbols work too (B RWTS).
Dumps read through the bus: dumping $C0xx trips soft switches.
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::two::TwoType;

    fn machine() -> (WozBug, Two) {
        (
            WozBug::new(),
            Two::new(TwoType::Apple2Plus).expect("machine must construct"),
        )
    }

    #[test]
    fn deposit_dump_and_the_dot_address() {
        let (mut wb, mut two) = machine();
        assert_eq!(
            wb.execute(&mut two, "300:A9 8D 20 ED FD"),
            "0300- A9 8D 20 ED FD"
        );
        assert_eq!(wb.execute(&mut two, "300.304"), "0300- A9 8D 20 ED FD");
        assert_eq!(wb.execute(&mut two, "300"), "0300- A9");
        // The dot advanced past the examined byte; a bare Return continues.
        let cont = wb.execute(&mut two, "");
        assert!(cont.starts_with("0301- 8D 20 ED FD"), "{cont}");
        // A dotted deposit continues where the dump stopped.
        let (mut wb, mut two) = machine();
        wb.execute(&mut two, "300:AA");
        assert_eq!(wb.execute(&mut two, ":BB"), "0301- BB");
        assert_eq!(wb.execute(&mut two, "300.301"), "0300- AA BB");
    }

    #[test]
    fn dump_aligns_to_eight_byte_lines() {
        let (mut wb, mut two) = machine();
        wb.execute(&mut two, "3FE:01 02 03 04");
        let out = wb.execute(&mut two, "3FE.401");
        assert_eq!(out, "03FE- 01 02\n0400- 03 04");
    }

    #[test]
    fn registers_and_symbols() {
        let (mut wb, mut two) = machine();
        two.cpu.pc = 0xbd25;
        let r = wb.execute(&mut two, "R");
        assert!(r.contains("PC=BD25 (RWTS+$25)"), "{r}");
        let r = wb.execute(&mut two, "PC=FDED");
        assert!(r.contains("PC=FDED (COUT)"), "{r}");
        let r = wb.execute(&mut two, "A=7F");
        assert!(r.contains("A=7F"), "{r}");
        assert_eq!(symbolize(0xbd00).as_deref(), Some("RWTS"));
        assert_eq!(symbolize(0xbe00), None, "past the +$FF window");
        assert_eq!(symbolize(0x0100), None);
    }

    #[test]
    fn breakpoints_by_symbol_and_stepping() {
        let (mut wb, mut two) = machine();
        // A tiny program in RAM: LDA #$42, NOP, NOP.
        wb.execute(&mut two, "300:A9 42 EA EA");
        two.cpu.pc = 0x0300;
        assert_eq!(wb.execute(&mut two, "B 302"), "breakpoint set at 0302");
        assert_eq!(
            wb.execute(&mut two, "B RWTS"),
            "breakpoint set at BD00 (RWTS)"
        );
        assert_eq!(wb.execute(&mut two, "B"), "0302\nBD00 (RWTS)");

        // Step runs into the breakpoint and reports it.
        let out = wb.execute(&mut two, "S 10");
        assert!(out.contains("0300: LDA  #$42"), "{out}");
        assert!(out.contains("stopped at 0302"), "{out}");
        assert!(two.cpu.stopped());
        assert_eq!(two.cpu.a, 0x42);

        // S resumes past the breakpoint and executes the instruction there.
        let out = wb.execute(&mut two, "S");
        assert!(out.contains("0302: NOP"), "{out}");
        assert_eq!(two.cpu.pc, 0x0303);

        assert_eq!(
            wb.execute(&mut two, "B-RWTS"),
            "breakpoint cleared at BD00 (RWTS)"
        );
        assert_eq!(wb.execute(&mut two, "B-"), "all breakpoints cleared");
        assert_eq!(wb.execute(&mut two, "B"), "no breakpoints");
    }

    #[test]
    fn go_resumes_and_sets_the_pc() {
        let (mut wb, mut two) = machine();
        two.cpu.stop();
        assert_eq!(wb.execute(&mut two, "G"), "");
        assert!(!two.cpu.stopped());
        two.cpu.stop();
        assert_eq!(wb.execute(&mut two, "300G"), "");
        assert!(!two.cpu.stopped());
        assert_eq!(two.cpu.pc, 0x0300);
    }

    #[test]
    fn hex_addresses_starting_with_b_and_w_symbols_still_examine() {
        let (mut wb, mut two) = machine();
        // BD00 examines memory (RWTS ROM space is empty RAM-side here);
        // it must not become a breakpoint at $D00.
        let out = wb.execute(&mut two, "BD00");
        assert!(out.starts_with("BD00-"), "{out}");
        assert!(two.cpu.breakpoints().is_empty());
        // The WAIT symbol examines $FCA8 rather than becoming a watchpoint.
        let out = wb.execute(&mut two, "WAIT");
        assert!(out.starts_with("FCA8-"), "{out}");
        assert!(two.cpu.mem.watchpoints().is_empty());
    }

    #[test]
    fn watchpoints_and_the_stop_banner() {
        let (mut wb, mut two) = machine();
        assert_eq!(
            wb.execute(&mut two, "W 2000.200F"),
            "watchpoint set at 2000.200F"
        );
        assert_eq!(wb.execute(&mut two, "W"), "2000.200F");

        // STA $2000 from a little program: the CPU stops after the store
        // and the banner names the access.
        wb.execute(&mut two, "300:A9 5A 8D 00 20 EA");
        two.cpu.pc = 0x0300;
        let out = wb.execute(&mut two, "S 5");
        assert!(out.contains("stopped at"), "{out}");
        let banner = stopped_banner(&mut two);
        assert!(banner.contains("watch: write 2000 = 5A"), "{banner}");

        wb.execute(&mut two, "G");
        assert_eq!(
            wb.execute(&mut two, "W-2000.200F"),
            "watchpoint cleared at 2000.200F"
        );
        assert_eq!(wb.execute(&mut two, "W"), "no watchpoints");
        wb.execute(&mut two, "W 1234");
        assert_eq!(wb.execute(&mut two, "W-"), "all watchpoints cleared");
    }

    #[test]
    fn trace_toggle_and_disassembly() {
        let (mut wb, mut two) = machine();
        assert!(two.cpu.trace.is_none());
        assert_eq!(wb.execute(&mut two, "T"), "trace on (stderr)");
        assert!(two.cpu.trace.is_some());
        assert_eq!(wb.execute(&mut two, "T-"), "trace off");
        assert!(two.cpu.trace.is_none());

        wb.execute(&mut two, "300:A9 42 8D 00 20 4C 00 03");
        let out = wb.execute(&mut two, "L 300");
        assert!(out.starts_with("0300: LDA  #$42"), "{out}");
        assert!(out.contains("0302: STA  $2000"), "{out}");
        assert!(out.contains("0305: JMP  $0300"), "{out}");
        assert_eq!(out.lines().count(), 16);
        // A bare L continues where the last one stopped.
        let next = wb.execute(&mut two, "L");
        assert!(!next.contains("0300: LDA"), "{next}");
    }

    #[test]
    fn server_serves_sequential_connections() {
        use std::io::{BufRead, BufReader, Write};

        let (mut wb, mut two) = machine();
        let server = Server::start(0).expect("bind an ephemeral port");

        for attempt in 0..3 {
            let mut client = std::net::TcpStream::connect(("127.0.0.1", server.port()))
                .unwrap_or_else(|e| panic!("connect #{attempt}: {e}"));
            client.write_all(b"R\n").expect("send");
            let line = server
                .commands
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap_or_else(|e| panic!("command #{attempt}: {e}"));
            server.reply(&wb.execute(&mut two, &line));
            let mut reader = BufReader::new(client);
            let mut banner = String::new();
            reader.read_line(&mut banner).expect("banner");
            let mut reply = String::new();
            reader.read_line(&mut reply).expect("reply");
            assert!(reply.contains("PC="), "attempt {attempt}: {reply}");
            // Dropping the client here must free the server to accept the
            // next connection.
        }
    }

    #[test]
    fn server_round_trip() {
        use std::io::{BufRead, BufReader, Write};

        let (mut wb, mut two) = machine();
        let server = Server::start(0).expect("bind an ephemeral port");
        let mut client =
            std::net::TcpStream::connect(("127.0.0.1", server.port())).expect("connect");
        client.write_all(b"R\n").expect("send command");

        // Pump the emulator side once, as the frame loop does.
        let line = server
            .commands
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("command arrives");
        server.reply(&wb.execute(&mut two, &line));

        let mut reader = BufReader::new(client);
        let mut banner = String::new();
        reader.read_line(&mut banner).expect("banner");
        assert!(banner.contains("WozBug"), "{banner}");
        let mut reply = String::new();
        reader.read_line(&mut reply).expect("reply");
        assert!(reply.contains("PC="), "{reply}");
    }

    #[test]
    fn machine_state_commands() {
        let (mut wb, mut two) = machine();
        let dsk = wb.execute(&mut two, "DSK");
        assert!(
            dsk.contains("S6: drive 1 selected, track 0, motor off, D1 empty, D2 empty  [boot]"),
            "{dsk}"
        );
        let slots = wb.execute(&mut two, "SLOTS");
        assert!(slots.contains("S0: language card (16K)"), "{slots}");
        assert!(slots.contains("S1: Thunderclock"), "{slots}");
        assert!(slots.contains("S6: Disk II"), "{slots}");
        assert!(slots.contains("S7: -"), "{slots}");
        let sw = wb.execute(&mut two, "SW");
        assert!(sw.contains("80col=false"), "{sw}");
        assert!(wb.execute(&mut two, "TEXT").len() > 100);
        assert_eq!(wb.execute(&mut two, "XYZZY"), "?");
        assert!(wb.execute(&mut two, "?").contains("WozBug"));
    }
}
