use std::{env, fs, io::Write, path::Path};

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    let command = env::args().next().unwrap();
    let command = Path::new(&command).file_name().unwrap().to_str().unwrap();
    let root = env::var("INSTALL_TEST_ROOT").unwrap();
    let root = Path::new(&root);
    match command {
        "id" => println!("{}", env::var("INSTALL_TEST_UID").unwrap()),
        "uname" => println!("{}", if args == ["-s"] { "Linux" } else { "x86_64" }),
        "curl" => {
            if env::var_os("INSTALL_TEST_DOWNLOAD_FAILURE").is_some() {
                std::process::exit(22);
            }
            assert!(
                args.iter()
                    .any(|a| a == "https://packages.esdieff.com/esdieff.asc")
            );
            let output = &args[args.iter().position(|a| a == "-o").unwrap() + 1];
            let key = if env::var_os("INSTALL_TEST_WRONG_KEY").is_some() {
                b"a different signing key".as_slice()
            } else {
                include_bytes!("../../../../packaging/esdieff.asc").as_slice()
            };
            fs::write(output, key).unwrap();
        }
        _ => {
            let mut log = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(root.join("commands"))
                .unwrap();
            writeln!(log, "{command} {}", args.join(" ")).unwrap();
            if command == "sudo" {
                let status = std::process::Command::new(&args[0])
                    .args(&args[1..])
                    .status()
                    .unwrap();
                std::process::exit(status.code().unwrap_or(1));
            }
            if command == "install" {
                let destination = root.join("installed").join(args[4].trim_start_matches('/'));
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(&args[3], destination).unwrap();
            }
            if command == "apt-get"
                && args == ["update"]
                && env::var_os("INSTALL_TEST_UPDATE_FAILURE").is_some()
            {
                std::process::exit(100);
            }
        }
    }
}
