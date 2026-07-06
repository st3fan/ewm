//! Minimal interactive console for the headless Apple 1 / Replica 1 — a way
//! to poke at the Woz monitor before the SDL frontend lands in Phase 4.
//! Input is line-based: type a monitor command and press enter.
//!
//!     cargo run -p ewm-core --example one -- [apple1|replica1]
//!
//! Try `E000.E00F` (Replica 1) or `FF00.FFFF` (Apple 1) to dump memory, or
//! deposit and run a tiny program that prints HI! through the monitor's
//! ECHO routine at $FFEF:
//!
//!     280: A9 C8 20 EF FF A9 C9 20 EF FF A9 A1 20 EF FF A9 8D 20 EF FF 60
//!     280R
//!
//! Quit with ctrl-C or ctrl-D.

use std::io::{BufRead, Write};

use ewm::one::{One, OneModel};

fn pump(one: &mut One, cycles: u64) {
    let mut done = 0;
    while done < cycles {
        done += one.cpu.step() as u64;
    }
    let mut out = std::io::stdout().lock();
    for b in one.drain_display() {
        match b & 0x7f {
            0x0d => writeln!(out).unwrap(),
            c @ 0x20..=0x7e => write!(out, "{}", c as char).unwrap(),
            _ => {}
        }
    }
    out.flush().unwrap();
}

fn main() {
    // The C default model is the Replica 1 (EWM_ONE_MODEL_DEFAULT).
    let model = match std::env::args().nth(1).as_deref() {
        None | Some("replica1") => OneModel::Replica1,
        Some("apple1") => OneModel::Apple1,
        Some(other) => {
            eprintln!("unknown model '{other}' (expected apple1 or replica1)");
            std::process::exit(1);
        }
    };

    let mut one = One::new(model);
    one.cpu.reset();

    eprintln!("[{model:?} at the Woz monitor — type commands, enter sends CR]");
    pump(&mut one, 1_000_000);

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        // The C frontend upper-cases typed characters (the Apple 1 keyboard
        // had no lower case); do the same.
        for b in line.to_uppercase().into_bytes() {
            one.key(b);
            pump(&mut one, 50_000);
        }
        one.key(0x0d);
        pump(&mut one, 1_000_000);
    }
}
