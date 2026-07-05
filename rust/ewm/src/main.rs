use std::process::ExitCode;

fn usage() {
    eprintln!("Usage: ewm [--help|-h] [<command> [--help|-h] [args]]");
}

fn main() -> ExitCode {
    usage();
    ExitCode::FAILURE
}
