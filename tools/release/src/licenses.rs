//! Generate per-target dependency notices with cargo-about's maintained scanner.
use crate::Result;
use std::{fs, path::Path, process::Command};

fn command_output(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "{:?} failed with {}: {}",
            command.get_program(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// TARGET OUTPUT: writes the notices before cargo-dist assembles the archive.
pub fn run(args: &[String]) -> Result<()> {
    let [target, output] = args else {
        return Err("usage: esdieff-release licenses TARGET OUTPUT".into());
    };
    if ![
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ]
    .contains(&target.as_str())
    {
        return Err("unsupported release target".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let about = std::env::var_os("CARGO_ABOUT").unwrap_or_else(|| "cargo-about".into());
    let mut report = command_output(
        Command::new(about)
            .args([
                "generate",
                "--locked",
                "--fail",
                "--target",
                target,
                "--config",
                "about.toml",
                "about.hbs",
            ])
            .current_dir(&root),
    )?;
    let sysroot = command_output(Command::new("rustc").args(["--print", "sysroot"]))?;
    // COPYRIGHT-library.html ships with rustc, including its minimal profile.
    let standard_library = fs::read_to_string(
        Path::new(sysroot.trim()).join("share/doc/rust/COPYRIGHT-library.html"),
    )?;
    if !standard_library.contains("Copyright notices for The Rust Standard Library") {
        return Err("Rust toolchain has no standard-library copyright notices".into());
    }
    report.push_str("\n## Rust standard library\n\nThe following copyright notices accompany the Rust standard library.\n\n```html\n");
    report.push_str(&standard_library);
    report.push_str("\n```\n");
    let output = Path::new(output);
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, report)?;
    Ok(())
}
