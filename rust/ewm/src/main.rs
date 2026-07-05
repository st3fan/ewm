//! Subcommand dispatch, port of `ewm.c`.

mod one;
mod sdl;
mod tty;

use std::process::ExitCode;

fn usage() {
    eprintln!("Usage: ewm [--help|-h] [<command> [--help|-h] [args]]");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--help") | Some("-h") => {
            usage();
            ExitCode::SUCCESS
        }
        Some("one") => ExitCode::from(one::main(&args[1..]) as u8),
        Some("two") | Some("boo") => {
            eprintln!("ewm: {} is not ported yet (see REWRITE.md)", args[0]);
            ExitCode::FAILURE
        }
        _ => {
            usage();
            ExitCode::FAILURE
        }
    }
}
