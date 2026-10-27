mod distribution;
mod licenses;
mod serve;
mod smoke;
mod weekly;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run(args: &[String]) -> Result<()> {
    let Some((command, rest)) = args.split_first() else {
        return Err(
            "expected smoke, licenses, prepare-weekly, release-notes or a distribution command"
                .into(),
        );
    };
    match command.as_str() {
        "smoke" => smoke::run(rest),
        "licenses" => licenses::run(rest),
        "serve-artifacts" => serve::run(rest),
        "prepare-weekly" => weekly::prepare(rest),
        "release-notes" => weekly::notes(rest),
        "package-linux" | "repository" | "homebrew" | "winget" => distribution::run(args),
        _ => Err(format!("unknown release command: {command}").into()),
    }
}

fn main() -> std::process::ExitCode {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("esdieff-release: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
