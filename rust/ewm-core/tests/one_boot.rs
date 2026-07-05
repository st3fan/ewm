//! Headless Apple 1 / Replica 1 boot tests (the Phase 3 gate): boot to the
//! Woz monitor prompt, type a memory-dump command through the PIA, and
//! assert the hex dump of the ROM region comes back on the display.

use ewm_core::cpu::Cpu;
use ewm_core::one::{One, OneModel};

struct Machine {
    cpu: Cpu,
    one: One,
    output: Vec<u8>,
}

impl Machine {
    fn boot(model: OneModel) -> Machine {
        let mut one = One::new(model);
        let mut cpu = Cpu::new(one.cpu_model());
        cpu.reset(&mut one);
        Machine {
            cpu,
            one,
            output: Vec::new(),
        }
    }

    fn step(&mut self, cycles: u64) {
        let mut done = 0u64;
        while done < cycles {
            done += self.cpu.step(&mut self.one) as u64;
        }
        self.output.extend(self.one.drain_display());
    }

    /// Feed keys one at a time, giving the monitor time to poll and echo
    /// each one — there is no keyboard queue, just the PIA input register.
    fn type_keys(&mut self, keys: &str) {
        for &b in keys.as_bytes() {
            self.one.key(b);
            self.step(50_000);
        }
    }

    /// Display output as text. The high bit is stripped because the Woz
    /// monitor writes characters with bit 7 set and only the Apple 1 model
    /// masks them on the way out.
    fn text(&self) -> String {
        self.output.iter().map(|&b| (b & 0x7f) as char).collect()
    }
}

/// The Woz monitor dump line for eight bytes starting at `addr`:
/// `ADDR: B0 B1 B2 B3 B4 B5 B6 B7`.
fn dump_line(addr: u16, bytes: &[u8]) -> String {
    format!(
        "{:04X}: {}",
        addr,
        bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn rom(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../src/rom/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("cannot read ROM")
}

#[test]
fn apple1_woz_monitor_dumps_rom() {
    let mut m = Machine::boot(OneModel::Apple1);
    m.step(1_000_000);
    assert!(
        m.text().contains('\\'),
        "no Woz monitor prompt, display was {:?}",
        m.text()
    );

    // Dump the first 16 bytes of the monitor ROM itself.
    m.type_keys("FF00.FF0F\r");
    m.step(1_000_000);

    let rom = rom("apple1.rom");
    let text = m.text();
    assert!(
        text.contains(&dump_line(0xff00, &rom[0..8])),
        "first dump line missing, display was {text:?}"
    );
    assert!(
        text.contains(&dump_line(0xff08, &rom[8..16])),
        "second dump line missing, display was {text:?}"
    );
}

#[test]
fn replica1_woz_monitor_dumps_rom() {
    let mut m = Machine::boot(OneModel::Replica1);
    m.step(1_000_000);
    assert!(
        m.text().contains('\\'),
        "no Woz monitor prompt, display was {:?}",
        m.text()
    );

    // Dump the first 16 bytes of the Krusader ROM at $E000, per the
    // REWRITE.md gate.
    m.type_keys("E000.E00F\r");
    m.step(1_000_000);

    let rom = rom("krusader.rom");
    let text = m.text();
    assert!(
        text.contains(&dump_line(0xe000, &rom[0..8])),
        "first dump line missing, display was {text:?}"
    );
    assert!(
        text.contains(&dump_line(0xe008, &rom[8..16])),
        "second dump line missing, display was {text:?}"
    );
}

#[test]
fn apple1_echoes_typed_characters() {
    // Mirrors the intent of tests/apple1/echo.s: what is typed comes back
    // on the display.
    let mut m = Machine::boot(OneModel::Apple1);
    m.step(1_000_000);
    m.output.clear();

    m.type_keys("A9");
    assert!(
        m.text().contains("A9"),
        "typed characters not echoed, display was {:?}",
        m.text()
    );
}
