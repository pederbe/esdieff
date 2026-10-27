//! Generate per-target dependency notices with cargo-about's maintained scanner.
use crate::Result;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

const NATIVE: &[(&str, &str)] = &[
    ("curl-sys", "curl/COPYING"),
    ("libz-sys", "src/zlib/LICENSE"),
    ("rquickjs-sys", "quickjs/LICENSE"),
    ("openssl-src", "openssl/LICENSE.txt"),
];

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

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_owned()
}

fn check_native_notices(metadata: &Value, target: &str, report: &str) -> Result<()> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("Cargo metadata has no packages")?;
    let report = normalize(report);
    for (name, relative) in NATIVE {
        // curl uses the Windows platform TLS implementation, not openssl-src.
        if *name == "openssl-src" && target.ends_with("windows-msvc") {
            continue;
        }
        let package = packages
            .iter()
            .find(|package| package["name"] == *name)
            .ok_or_else(|| format!("locked target metadata has no {name}"))?;
        let manifest = package["manifest_path"]
            .as_str()
            .ok_or("package has no manifest path")?;
        let source = Path::new(manifest)
            .parent()
            .ok_or("package manifest has no parent")?
            .join(relative);
        let required = normalize(&fs::read_to_string(&source)?);
        if required.is_empty() || !report.contains(&required) {
            return Err(format!("dependency notices omit the complete {name} {relative}").into());
        }
    }
    Ok(())
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
    let metadata = command_output(
        Command::new("cargo")
            .args([
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--filter-platform",
                target,
            ])
            .current_dir(&root),
    )?;
    check_native_notices(&serde_json::from_str(&metadata)?, target, &report)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_copyright_notices_cannot_be_replaced_by_a_wrapper_license() {
        let directory = std::env::temp_dir().join(format!(
            "esdieff-native-notices-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut packages = Vec::new();
        let mut report = String::new();
        for (name, relative) in NATIVE {
            let package = directory.join(name);
            let file = package.join(relative);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            let notice = format!("Copyright actual {name} authors.\nPermission for {name}.\n");
            fs::write(file, &notice).unwrap();
            report.push_str(&notice);
            packages
                .push(serde_json::json!({"name":name,"manifest_path":package.join("Cargo.toml")}));
        }
        let metadata = serde_json::json!({"packages":packages});
        check_native_notices(&metadata, "x86_64-unknown-linux-gnu", &report).unwrap();
        for (name, _) in NATIVE {
            let missing = report.replace(
                &format!("Copyright actual {name} authors."),
                "MIT wrapper license",
            );
            assert!(check_native_notices(&metadata, "x86_64-unknown-linux-gnu", &missing).is_err());
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
