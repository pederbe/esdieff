use crate::Result;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn create() -> Result<Self> {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "esdieff-installation-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not create installation check directory".into())
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn output(command: &mut Command) -> Result<String> {
    let done = command.output()?;
    if !done.status.success() {
        return Err(format!(
            "installation check failed with {}: {}",
            done.status,
            String::from_utf8_lossy(&done.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(done.stdout)?)
}

/// Exercise the actual installed executable. VERSION may be supplied for a frozen artifact.
pub(crate) fn run(args: &[String]) -> Result<()> {
    let [binary, rest @ ..] = args else {
        return Err("usage: esdieff-release smoke BIN [VERSION]".into());
    };
    if rest.len() > 1 {
        return Err("usage: esdieff-release smoke BIN [VERSION]".into());
    }
    let expected = if let Some(version) = rest.first() {
        weekly_version(version)?
    } else {
        crate::weekly::crate_version(&fs::read_to_string("Cargo.toml")?)?
    };
    let version = output(Command::new(binary).arg("--version"))?;
    if version.trim() != format!("esdieff {expected}") {
        return Err(format!("expected esdieff {expected}, received {:?}", version.trim()).into());
    }
    let directory = Directory::create()?;
    let model = directory.0.join("thermostat.sdf.json");
    fs::write(
        &model,
        r#"{"info":{"title":"Installation check"},"sdfObject":{"Thermostat":{"sdfProperty":{"temperature":{"type":"number","minimum":5,"maximum":30}}}}}"#,
    )?;
    for operation in ["check", "inspect"] {
        let mut command = Command::new(binary);
        command.args([operation, "--format", "json", "--color", "never"]);
        if operation == "inspect" {
            command
                .arg("--document")
                .arg(&model)
                .args(["--pointer", "/sdfObject/Thermostat"]);
        }
        command.arg(&model);
        let report: serde_json::Value = serde_json::from_str(&output(&mut command)?)?;
        if report["operation"] != operation
            || report["outcome"]["complete"] != true
            || report["outcome"]["disposition"] != "passed"
        {
            return Err(
                format!("installed {operation} did not return a complete passing report").into(),
            );
        }
        if operation == "inspect"
            && report["result"]["json"]["sdfProperty"]["temperature"]["maximum"] != 30
        {
            return Err("installed inspection lost the model's temperature constraint".into());
        }
    }
    fs::remove_dir_all(&directory.0)?;
    println!("Esdieff {expected}: installation check passed");
    Ok(())
}

fn weekly_version(version: &str) -> Result<String> {
    crate::weekly::version(version)?;
    Ok(version.strip_prefix('v').unwrap_or(version).to_owned())
}
