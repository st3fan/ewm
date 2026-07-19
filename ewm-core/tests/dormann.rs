//! The Klaus Dormann functional tests, ported from cpu_test.c. Success is
//! reaching the hardcoded success PC; failure is a branch-to-self deadlock,
//! whose PC localizes the failing test case.

use ewm_core::cpu::{Cpu, Model};
use ewm_core::mem::Memory;

fn run_test(model: Model, start_addr: u16, success_addr: u16, rom_path: &str) {
    let data = std::fs::read(rom_path)
        .unwrap_or_else(|e| panic!("cannot read test binary {rom_path}: {e}"));

    let mut cpu = Cpu::new(model, Memory::new(0x10000));
    cpu.mem.load(0x0000, &data);
    cpu.reset();
    cpu.pc = start_addr;

    let mut last_pc = cpu.pc;

    // The 6502 run needs ~30M instructions; the cap only guards CI against a
    // regression that neither finishes nor deadlocks.
    for _ in 0..200_000_000u64 {
        cpu.step();

        if cpu.pc == success_addr {
            println!("TEST   Success; executed {} cycles", cpu.counter);
            return;
        }

        // A branch-to-self deadlock means a test case failed; the PC tells
        // which one (see the Dormann listing).
        if cpu.pc == last_pc {
            let i = cpu.mem.read(cpu.pc);
            let is_branch = matches!(i, 0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xb0 | 0xd0 | 0xf0);
            if is_branch && cpu.mem.read(cpu.pc.wrapping_add(1)) == 0xfe {
                panic!("functional test failed at {:#06x}", cpu.pc);
            }
        }

        last_pc = cpu.pc;
    }

    panic!("step cap exceeded without reaching the success address");
}

#[test]
fn dormann_6502() {
    run_test(
        Model::M6502,
        0x0400,
        0x3399,
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../roms/6502_functional_test.bin"
        ),
    );
}

#[test]
fn dormann_65c02() {
    run_test(
        Model::M65C02,
        0x0400,
        0x24a8,
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../roms/65C02_extended_opcodes_test.bin"
        ),
    );
}
