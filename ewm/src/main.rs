//! Subcommand dispatch, port of `ewm.c`: `one`, `two`, `boo`, and no
//! arguments runs the bootloader menu.

use std::process::ExitCode;

use ewm::media::MediaKind;
use ewm::{boo, one, two};

fn usage() {
    eprintln!("Usage: ewm [--help|-h] [<command> [--help|-h] [args]]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  one     Run the Apple 1 / Replica 1 emulator");
    eprintln!("  two     Run the Apple ][+ or Enhanced //e (--model 2e) emulator");
    eprintln!("  boo     Run the 'bootloader' (default)");
    eprintln!();
    eprintln!("If no command is specified, the 'bootloader' will be run, which");
    eprintln!("allows the user to interactively select what emulator to start.");
    eprintln!("\nSuggestion: to get started, try 'ewm two --color --drive1 <disk file>'");
}

fn run_boo(args: &[String]) -> i32 {
    match boo::main(args) {
        boo::BooChoice::BootApple1 => one::main(&["--model".to_string(), "apple1".to_string()]),
        boo::BooChoice::BootReplica1 => one::main(&["--model".to_string(), "replica1".to_string()]),
        boo::BooChoice::BootApple2Plus => two::main(&[]),
        // A dropped or Finder-opened disk image boots the ][+ with it (the
        // default model until machine configs exist; see notes/MAC_APP.md).
        boo::BooChoice::BootDroppedMedia(path, MediaKind::Floppy) => {
            two::main(&["--drive1".to_string(), path])
        }
        boo::BooChoice::BootDroppedMedia(path, MediaKind::HardDrive) => {
            two::main(&["--hdd".to_string(), path])
        }
        boo::BooChoice::Quit => 0,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let code = match args.first().map(String::as_str) {
        None => run_boo(&[]),
        Some("--help") | Some("-h") => {
            usage();
            0
        }
        Some("one") => one::main(&args[1..]),
        Some("two") => two::main(&args[1..]),
        Some("boo") => run_boo(&args[1..]),
        _ => {
            usage();
            1
        }
    };
    ExitCode::from(code as u8)
}
