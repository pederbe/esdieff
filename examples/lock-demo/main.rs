//! Replay the lock-demo/1 door-lock demonstration with native Sample-value checking.
//! Exits 0 only when the model checks, native sample expectations and authored behavior
//! assertions all match. See README.md beside this file.
mod behavior;
mod demo;
mod expected;
mod expected_native;
mod matching;
mod native;
mod oracle;
mod scenario;
mod simulator;
mod trace;
mod wording;

use std::{
    io::{Write, stderr, stdout},
    process::ExitCode,
};

fn main() -> ExitCode {
    let seed = match demo::arguments(std::env::args_os().skip(1)) {
        Ok(seed) => seed,
        Err(reason) => {
            let _ = writeln!(stderr(), "lock-demo: {reason}");
            return ExitCode::from(2);
        }
    };
    let demonstration = demo::demonstrate(demo::RICH_MODEL, seed);
    let mut out = stdout().lock();
    if demo::render(&mut out, &demonstration)
        .and_then(|()| out.flush())
        .is_err()
    {
        let _ = writeln!(stderr(), "lock-demo: demonstration output failed");
        return ExitCode::from(4);
    }
    if demonstration.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
