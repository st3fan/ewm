//! Port of `cpu_bench.c`: time each 65C02 instruction handler directly,
//! 10M calls × 3 runs, printing milliseconds in the same format as C so the
//! numbers are directly comparable. Optional args filter by mnemonic:
//!
//!     cargo bench -p ewm-core --bench cpu_bench            # all 256 opcodes
//!     cargo bench -p ewm-core --bench cpu_bench -- LDA JSR

use std::time::Instant;

use ewm_core::bus::TestBus;
use ewm_core::cpu::{Cpu, Model};
use ewm_core::ins::{Handler, Instruction, instructions_65c02};

const CPU_BENCH_ITERATIONS: u64 = 10 * 1000 * 1000;

fn test(cpu: &mut Cpu, bus: &mut TestBus, ins: &Instruction) {
    let mut runs = [0u64; 3];

    for run in &mut runs {
        let start = Instant::now();

        match ins.handler {
            Handler::Implied(f) => {
                for _ in 0..CPU_BENCH_ITERATIONS {
                    f(cpu, bus);
                }
            }
            Handler::Byte(f) => {
                for _ in 0..CPU_BENCH_ITERATIONS {
                    f(cpu, bus, 0x12);
                }
            }
            Handler::Word(f) => {
                for _ in 0..CPU_BENCH_ITERATIONS {
                    f(cpu, bus, 0x1234);
                }
            }
        }

        *run = start.elapsed().as_millis() as u64;
    }

    println!(
        "${:02X} {} {:8} {:8} {:8} -> {:8}",
        ins.opcode,
        ins.name,
        runs[0],
        runs[1],
        runs[2],
        (runs[0] + runs[1] + runs[2]) / 3
    );
}

fn main() {
    let mut bus = TestBus::new();
    let mut cpu = Cpu::new(Model::M65C02);
    cpu.reset(&mut bus);

    // cargo bench passes a --bench flag through; ignore flag-like args.
    let names: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();

    let table = instructions_65c02();
    if !names.is_empty() {
        for name in &names {
            for ins in table.iter() {
                if ins.name == name {
                    test(&mut cpu, &mut bus, ins);
                }
            }
        }
    } else {
        for ins in table.iter() {
            test(&mut cpu, &mut bus, ins);
        }
    }
}
