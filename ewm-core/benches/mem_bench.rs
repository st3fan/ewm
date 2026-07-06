//! Port of `mem_bench.c`: time the stack primitives, raw bus access, and
//! every addressing-mode helper, 100M iterations each, printing
//! milliseconds in the same format as C.
//!
//!     cargo bench -p ewm-core --bench mem_bench

use std::time::Instant;

use ewm_core::cpu::{Cpu, Model};
use ewm_core::ins;
use ewm_core::mem::Memory;

const MEM_BENCH_ITERATIONS: u64 = 100 * 1000 * 1000;

fn test(name: &str, mut run: impl FnMut()) {
    let start = Instant::now();
    run();
    println!("{:<32} {:8}", name, start.elapsed().as_millis());
}

fn main() {
    let mut cpu = Cpu::new(Model::M6502, Memory::new(0x10000));
    cpu.reset();

    println!("-------------------------------- --------");
    test("_cpu_push_byte", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            cpu.push_byte(0xaa);
        }
    });
    test("_cpu_pull_byte", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = cpu.pull_byte();
        }
    });
    test("_cpu_push_word", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            cpu.push_word(0xaeae);
        }
    });
    test("_cpu_pull_word", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = cpu.pull_word();
        }
    });

    println!("-------------------------------- --------");
    test("mem_get_byte", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = cpu.mem.read(0x1234);
        }
    });
    test("mem_get_byte_abs", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_abs(&mut cpu, 0x1234);
        }
    });
    test("mem_get_byte_absx", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_absx(&mut cpu, 0x1234);
        }
    });
    test("mem_get_byte_absy", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_absy(&mut cpu, 0x1234);
        }
    });
    test("mem_get_byte_zpg", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_zpg(&mut cpu, 0x12);
        }
    });
    test("mem_get_byte_zpgx", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_zpgx(&mut cpu, 0x12);
        }
    });
    test("mem_get_byte_zpgy", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_zpgy(&mut cpu, 0x12);
        }
    });
    test("mem_get_byte_ind", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_ind(&mut cpu, 0x12);
        }
    });
    test("mem_get_byte_indx", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_indx(&mut cpu, 0x12);
        }
    });
    test("mem_get_byte_indy", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            let _ = ins::mem_get_byte_indy(&mut cpu, 0x12);
        }
    });

    println!("-------------------------------- --------");
    test("mem_set_byte", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            cpu.mem.write(0x1234, 0xaa);
        }
    });
    test("mem_set_byte_abs", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_abs(&mut cpu, 0x1234, 0xaa);
        }
    });
    test("mem_set_byte_absx", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_absx(&mut cpu, 0x1234, 0xaa);
        }
    });
    test("mem_set_byte_absy", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_absy(&mut cpu, 0x1234, 0xaa);
        }
    });
    test("mem_set_byte_zpg", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_zpg(&mut cpu, 0x12, 0xaa);
        }
    });
    test("mem_set_byte_zpgx", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_zpgx(&mut cpu, 0x12, 0xaa);
        }
    });
    test("mem_set_byte_zpgy", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_zpgy(&mut cpu, 0x12, 0xaa);
        }
    });
    test("mem_set_byte_ind", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_ind(&mut cpu, 0x12, 0xaa);
        }
    });
    test("mem_set_byte_indx", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_indx(&mut cpu, 0x12, 0xaa);
        }
    });
    test("mem_set_byte_indy", || {
        for _ in 0..MEM_BENCH_ITERATIONS {
            ins::mem_set_byte_indy(&mut cpu, 0x12, 0xaa);
        }
    });
}
