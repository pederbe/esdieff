//! Check a door-lock model and fixed sample values without simulating a device.
mod checks;

use std::{io, process::ExitCode};

fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        eprintln!("usage: cargo run --example lock-demo");
        return ExitCode::from(2);
    }
    match checks::run(&mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lock-demo: {error}");
            ExitCode::FAILURE
        }
    }
}
