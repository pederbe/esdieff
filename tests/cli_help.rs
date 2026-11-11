//! Exercise the examples printed by help with independent, complete input fixtures.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const COMMANDS: &[&str] = &[
    "check",
    "inspect",
    "sample",
    "generate",
    "trace",
    "diff",
    "reference",
    "schema",
    "author",
    "edit",
];

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-help-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let model = br#"{"info":{"title":"Help example"},"sdfData":{"Count":{"type":"integer","minimum":0}},"sdfProperty":{"temperature":{"type":"number","nullable":false,"minimum":0,"maximum":100}},"sdfObject":{"Sensor":{"sdfProperty":{"reading":{"type":"number","nullable":false}}}}}"#;
        for file in ["model.sdf.json", "old.sdf.json", "new.sdf.json"] {
            fs::write(path.join(file), model).unwrap();
        }
        for (file, content) in [
            (
                "device.sdf.json",
                include_str!("../examples/directory-check/device.sdf.json"),
            ),
            (
                "types.sdf.json",
                include_str!("../examples/directory-check/types.sdf.json"),
            ),
            (
                "base.sdf.json",
                include_str!("../examples/context-authoring/base.sdf.json"),
            ),
            (
                "recipe.json",
                include_str!("../examples/context-authoring/recipe.json"),
            ),
            (
                "edits.json",
                include_str!("../examples/context-authoring/edits.json"),
            ),
            ("value.json", "21.5"),
            (
                "readings.jsonl",
                "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/temperature\",\"value\":21.5}\n",
            ),
        ] {
            fs::write(path.join(file), content).unwrap();
        }
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["--color", "never"])
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn help(&self, scope: Option<&str>) -> String {
        let mut args = Vec::new();
        args.extend(scope);
        args.push("--help");
        let output = self.run(&args);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn help_is_task_specific_with_examples_before_options_and_a_guide_link() {
    let files = Files::new();
    let global = files.help(None);
    assert!(global.starts_with("Esdieff checks connected-device models"));
    assert!(!global.contains("Directory selection:"));
    assert!(!global.contains("10,000"));
    for command in COMMANDS {
        let help = files.help(Some(command));
        assert_ne!(help, global);
        assert!(help.contains(&format!("esdieff {command} ")));
        assert!(help.find("Examples:").unwrap() < help.find("Options:").unwrap());
        assert!(help.ends_with(&format!(
            "Guide: https://esdieff.com/docs/command-line#{command}\n"
        )));
        assert!(!help.contains("Commands:"));
        assert_eq!(
            help.contains("Directory selection:"),
            matches!(*command, "check" | "diff")
        );
    }
    let inspect = files.help(Some("inspect"));
    assert!(inspect.contains("optional, default: source"));
    assert!(inspect.contains("The trailing paths supply"));
    for command in ["author", "edit"] {
        let help = files.help(Some(command));
        assert!(help.contains("--source KEY PATH [--source KEY PATH ...]"));
        assert!(help.contains("Repeat the complete --source KEY PATH option"));
    }
    assert!(files.help(Some("schema")).contains("default: json"));
}

#[test]
fn every_printed_help_example_runs_with_the_files_and_definitions_it_names() {
    for scope in std::iter::once(None).chain(COMMANDS.iter().copied().map(Some)) {
        let files = Files::new();
        let help = files.help(scope);
        let examples = help
            .split_once("Examples:\n")
            .unwrap()
            .1
            .split_once("\nUsage:\n")
            .unwrap()
            .0;
        let commands: Vec<_> = examples
            .lines()
            .filter_map(|line| line.strip_prefix("    esdieff "))
            .collect();
        assert!((1..=2).contains(&commands.len()), "{scope:?}: {help}");
        for command in commands {
            let output = files.run(&command.split_whitespace().collect::<Vec<_>>());
            assert_eq!(
                output.status.code(),
                Some(0),
                "{command}\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert!(!output.stdout.is_empty());
        }
    }
}
