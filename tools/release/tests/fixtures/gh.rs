#![forbid(unsafe_code)]

use std::{env, fs, io::Write, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fake gh: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    let root = PathBuf::from(env::var_os("FAKE_GH_ROOT").ok_or("missing fixture root")?);
    writeln!(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("calls"))?,
        "{}",
        args.join("\t")
    )?;
    if args.first().map(String::as_str) != Some("release") {
        return Err("only release operations are supported".into());
    }
    let option = |name: &str| {
        args.windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].as_str())
            .ok_or_else(|| format!("missing {name}"))
    };
    let remote = root.join("assets");
    match args.get(1).map(String::as_str) {
        Some("view") => {
            let state = fs::read_to_string(root.join("state"))?;
            if state == "missing" {
                return Err("release does not exist".into());
            }
            match option("--json")? {
                "isDraft" => println!("{state}"),
                "assets" => {
                    let mut names = fs::read_dir(remote)?
                        .map(|entry| entry.map(|entry| entry.file_name()))
                        .collect::<Result<Vec<_>, _>>()?;
                    names.sort();
                    for name in names {
                        println!("{}", name.to_string_lossy());
                    }
                }
                _ => return Err("unexpected release query".into()),
            }
        }
        Some("create") => {
            if !args.iter().any(|arg| arg == "--verify-tag") {
                return Err("fixture refuses creating a release without an existing tag".into());
            }
            fs::write(root.join("state"), "true")?;
        }
        Some("upload") => {
            let source = PathBuf::from(args.get(3).ok_or("missing upload")?);
            let destination = remote.join(source.file_name().ok_or("missing file name")?);
            fs::copy(source, &destination)?;
            if root.join("corrupt-upload").exists() {
                fs::write(destination, b"corrupted upload")?;
            }
        }
        Some("download") => {
            let name = option("--pattern")?;
            fs::copy(
                remote.join(name),
                PathBuf::from(option("--dir")?).join(name),
            )?;
        }
        _ => return Err("unexpected release operation".into()),
    }
    Ok(())
}
