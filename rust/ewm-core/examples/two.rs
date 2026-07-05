//! Minimal interactive console for the headless Apple ][+ — type AppleSoft
//! BASIC at the real ROMs before the SDL frontend lands in Phase 7. Input is
//! line-based: type a line, press enter, and the 40×24 text screen is
//! redrawn.
//!
//!     cargo run -p ewm-core --example two [disk.dsk]
//!
//! Try:
//!
//!     PRINT 2+2
//!     10 FOR I = 1 TO 5
//!     20 PRINT "HELLO NUMBER "; I
//!     30 NEXT I
//!     RUN
//!
//! Or boot DOS 3.3 and explore the disk:
//!
//!     cargo run -p ewm-core --example two -- ../disks/DOS33-SystemMaster.dsk
//!     CATALOG
//!
//! Quit with ctrl-C or ctrl-D.

use std::io::BufRead;

use ewm_core::cpu::Cpu;
use ewm_core::two::{Two, TwoType};

fn step(cpu: &mut Cpu, two: &mut Two, cycles: u64) {
    let mut done = 0;
    while done < cycles {
        two.cycles = cpu.counter;
        done += cpu.step(two) as u64;
    }
}

/// Step until the key strobe is consumed, so keys are not dropped.
fn step_until_key_taken(cpu: &mut Cpu, two: &mut Two) {
    let mut spent = 0u64;
    while two.key & 0x80 != 0 && spent < 4_000_000 {
        step(cpu, two, 50_000);
        spent += 50_000;
    }
}

fn print_screen(two: &Two) {
    let text = two.text_screen();
    println!("+{}+", "-".repeat(40));
    for line in text.lines() {
        println!("|{line}|");
    }
    println!("+{}+", "-".repeat(40));
}

fn main() {
    let mut two = Two::new(TwoType::Apple2Plus).expect("apple2plus");

    if let Some(path) = std::env::args().nth(1) {
        if let Err(e) = two.load_disk(0, &path) {
            eprintln!("cannot load disk: {e}");
            std::process::exit(1);
        }
        eprintln!("[drive 1: {path}]");
    }

    let mut cpu = Cpu::new(two.cpu_model());
    cpu.reset(&mut two);

    eprintln!("[Apple ][+ — type BASIC, enter sends CR]");
    // Boot until the AppleSoft prompt appears (via DOS if a disk is in),
    // then let the boot settle — a transient prompt shows while the DOS
    // HELLO program is still running.
    let mut spent = 0u64;
    while !two.text_screen().contains(']') && spent < 400_000_000 {
        step(&mut cpu, &mut two, 1_000_000);
        spent += 1_000_000;
    }
    step(&mut cpu, &mut two, 30_000_000);
    print_screen(&two);

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        for b in line.to_uppercase().into_bytes() {
            two.key(b);
            step_until_key_taken(&mut cpu, &mut two);
        }
        two.key(0x0d);
        step_until_key_taken(&mut cpu, &mut two);
        // Give programs some time to run (RUN, FOR loops, ...).
        step(&mut cpu, &mut two, 4_000_000);
        print_screen(&two);
    }
}
