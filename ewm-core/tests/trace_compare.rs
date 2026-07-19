//! Replay the checked-in golden trace captured from the C emulator
//! (scripts/gen-golden-trace.sh) and diff the Rust CPU against it step by
//! step. The Dormann tests are the pass/fail gate; this test exists to
//! *localize* a divergence — it reports the exact instruction where the two
//! emulators first disagree.

use std::io::Read;

use ewm_core::cpu::{Cpu, Model};
use ewm_core::fmt::format_instruction;
use ewm_core::mem::Memory;

fn state_line(cpu: &Cpu) -> String {
    format!(
        "{:04X} {:02X} {:02X} {:02X} {:02X} {:02X}",
        cpu.pc,
        cpu.a,
        cpu.x,
        cpu.y,
        cpu.sp,
        cpu.status()
    )
}

#[test]
fn trace_compare_6502() {
    let golden_gz = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/6502_functional_trace.txt.gz"
    ))
    .expect("cannot read golden trace (regenerate with scripts/gen-golden-trace.sh)");
    let mut golden = String::new();
    flate2::read::GzDecoder::new(&golden_gz[..])
        .read_to_string(&mut golden)
        .expect("cannot decompress golden trace");

    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../roms/6502_functional_test.bin"
    ))
    .expect("cannot read test binary");

    let mut cpu = Cpu::new(Model::M6502, Memory::new(0x10000));
    cpu.mem.load(0x0000, &data);
    cpu.reset();
    cpu.pc = 0x0400;

    let mut prev: Option<(usize, String)> = None;
    for (lineno, expected) in golden.lines().enumerate() {
        let actual = state_line(&cpu);
        if actual != expected {
            let context = prev
                .map(|(n, l)| format!("\n  previous (step {n}):  {l}"))
                .unwrap_or_default();
            panic!(
                "trace diverges at step {lineno}:\
                 \n  expected (C):       {expected}\
                 \n  actual   (Rust):    {actual}\
                 \n  at instruction:     {}{context}",
                format_instruction(&mut cpu)
            );
        }
        prev = Some((lineno, actual));
        cpu.step();
    }
}
