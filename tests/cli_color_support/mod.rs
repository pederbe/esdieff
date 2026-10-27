//! CLI capture shared by presentation tests, including actual terminal destinations.
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn command(args: &[&str], environment: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
    command.args(args);
    configure_environment(&mut command, environment);
    command
}

fn configure_environment(command: &mut Command, environment: &[(&str, &str)]) {
    command
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR")
        .env_remove("CLICOLOR")
        .env_remove("CLICOLOR_FORCE")
        .env("TERM", "xterm-256color")
        .envs(environment.iter().copied());
}

pub fn run(args: &[&str], input: &[u8], environment: &[(&str, &str)]) -> Output {
    let mut child = command(args, environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

pub fn closed_stdout(args: &[&str], input: &[u8]) -> Output {
    let (reader, writer) = std::io::pipe().unwrap();
    // Close the reader before the child can emit even its shortest output.
    drop(reader);
    let mut child = command(args, &[])
        .stdin(Stdio::piped())
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

pub fn plain(bytes: &[u8]) -> String {
    let mut text = String::from_utf8(bytes.to_vec()).unwrap();
    for sequence in [
        "\x1b[0m", "\x1b[1m", "\x1b[31m", "\x1b[32m", "\x1b[33m", "\x1b[36m",
    ] {
        text = text.replace(sequence, "");
    }
    text.replace("\r\n", "\n")
}

/// util-linux script creates a real PTY without unsafe Rust or a runtime dependency.
#[cfg(target_os = "linux")]
pub fn terminal(
    args: &[&str],
    environment: &[(&str, &str)],
    redirect_stdout: bool,
    redirect_stderr: bool,
) -> Output {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "esdieff-color-terminal-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let mut shell = format!("exec {}", quote(env!("CARGO_BIN_EXE_esdieff")));
    for argument in args {
        shell.push(' ');
        shell.push_str(&quote(argument));
    }
    let stdout_path = directory.join("stdout");
    let stderr_path = directory.join("stderr");
    if redirect_stdout {
        shell.push_str(&format!(" >{}", quote(stdout_path.to_str().unwrap())));
    }
    if redirect_stderr {
        shell.push_str(&format!(" 2>{}", quote(stderr_path.to_str().unwrap())));
    }
    let mut command = Command::new("script");
    command
        .args(["-qefc", &shell, "/dev/null"])
        .stdin(Stdio::null());
    configure_environment(&mut command, environment);
    let mut capture = command
        .output()
        .expect("terminal presentation tests require util-linux script");
    if redirect_stdout {
        let terminal_output = std::mem::take(&mut capture.stdout);
        capture.stdout = std::fs::read(stdout_path).unwrap();
        capture.stderr = terminal_output;
    }
    if redirect_stderr {
        capture.stderr = std::fs::read(stderr_path).unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
    capture
}
