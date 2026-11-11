//! Error guidance through the command boundary, preserving report and exit contracts.
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-error-guidance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, content).unwrap();
        path
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&self.0)
            .args(args)
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn usage(output: Output) -> String {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn missing_and_repeated_options_name_the_option_across_all_parsers() {
    let files = Files::new();
    for args in [
        vec!["check", "--format"],
        vec!["inspect", "--pointer"],
        vec!["sample", "--kind"],
        vec!["generate", "--document"],
        vec!["schema", "--kind"],
        vec!["reference", "--pointer"],
        vec!["trace", "--trace"],
        vec!["author", "--plan"],
        vec!["edit", "--document-output"],
        vec!["diff", "--old"],
    ] {
        let message = usage(files.run(&args));
        assert!(message.contains(args[1]), "{args:?}: {message}");
        assert!(message.contains("requires a value"));
    }
    for (args, flag) in [
        (vec!["check", "--plan", "a", "--plan", "b"], "--plan"),
        (
            vec!["inspect", "--document", "a", "--document", "b"],
            "--document",
        ),
        (vec!["sample", "--sample", "a", "--sample", "b"], "--sample"),
        (
            vec!["generate", "--document", "a", "--document", "b"],
            "--document",
        ),
        (
            vec!["schema", "--pointer", "a", "--pointer", "b"],
            "--pointer",
        ),
        (vec!["reference", "--uses", "--uses"], "--uses"),
        (vec!["trace", "--trace", "a", "--trace", "b"], "--trace"),
        (vec!["author", "--plan", "a", "--plan", "b"], "--plan"),
        (
            vec!["edit", "--replace-document", "--replace-document"],
            "--replace-document",
        ),
        (vec!["diff", "--old", "a", "--old", "b"], "--old"),
    ] {
        let message = usage(files.run(&args));
        assert!(message.contains(flag), "{args:?}: {message}");
        assert!(message.contains("only once"));
    }
    let invalid = usage(files.run(&["schema", "--kind", "unknown"]));
    for kind in [
        "--kind",
        "property",
        "data",
        "action_input",
        "action_output",
        "event_output",
        "context",
    ] {
        assert!(invalid.contains(kind));
    }
    let unknown = usage(files.run(&["check", "--formt", "json"]));
    assert!(unknown.contains("--formt"));
    assert!(unknown.contains("esdieff --help"));
    assert!(unknown.contains("Use -- before a path"));
    for command in ["author", "edit", "diff"] {
        let message = usage(files.run(&[command, "--unknown"]));
        assert!(message.contains("--unknown"));
        assert!(message.contains("esdieff --help"));
        assert!(!message.contains("Use --"));
    }
    let wrong_command = usage(files.run(&["check", "--document", "model"]));
    assert!(wrong_command.contains("--document does not apply to check"));
}

#[test]
fn human_reports_show_file_labels_and_spans_and_group_coverage_notices() {
    let files = Files::new();
    files.write("broken.sdf.json", b"{\"sdfProperty\":");
    let human = files.run(&["check", "broken.sdf.json"]);
    assert_eq!(human.status.code(), Some(1));
    let human = String::from_utf8(human.stdout).unwrap();
    let wire: Value = serde_json::from_slice(
        &files
            .run(&["check", "broken.sdf.json", "--format", "json"])
            .stdout,
    )
    .unwrap();
    assert!(human.contains("\"broken.sdf.json\""));
    let span = &wire["diagnostics"]["items"][0]["primary"]["span"];
    assert!(human.contains(&format!("bytes {}..{}", span[0], span[1])));
    assert!(!human.contains("source 1 null"));
    let count = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["code"] == "coverage.unavailable")
        .count();
    assert!(count > 1);
    assert_eq!(human.matches("coverage.unavailable:").count(), 1);
    assert!(human.contains(&format!("({count} assessment areas)")));
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["outcome"]["complete"], false);
}

#[test]
fn file_read_failures_retain_the_os_cause_and_differ_from_plan_size_limits() {
    let files = Files::new();
    let missing = files.0.join("missing.sdf.json");
    let cause = fs::read(&missing).unwrap_err().to_string();
    let model = files.run(&["check", "missing.sdf.json", "--format", "json"]);
    assert_eq!(model.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&model.stdout).unwrap();
    let message = wire["diagnostics"]["items"][0]["message"].as_str().unwrap();
    assert!(message.contains(&cause));
    assert!(message.contains("Check the path and read permissions"));
    assert_eq!(wire["diagnostics"]["items"][0]["code"], "input.acquisition");
    let read_directory = files.run(&["check", ".", "--format", "json"]);
    assert_eq!(read_directory.status.code(), Some(3));
    let directory: Value = serde_json::from_slice(&read_directory.stdout).unwrap();
    assert!(
        directory["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["message"]
                .as_str()
                .unwrap()
                .contains(&fs::read(&files.0).unwrap_err().to_string()))
    );
    let plan = usage(files.run(&["check", "--plan", "missing.sdf.json"]));
    assert!(plan.contains("missing.sdf.json"));
    assert!(plan.contains(&cause));
    assert!(!plan.contains("1 MiB"));
    files.write("large-plan.json", vec![b' '; 1024 * 1024 + 1]);
    let large = usage(files.run(&["check", "--plan", "large-plan.json"]));
    assert!(large.contains("exceeds 1 MiB"));
    assert!(large.contains("Split"));
}

#[test]
fn plan_errors_locate_the_case_field_and_source_key() {
    let files = Files::new();
    let mut plan = json!({"schema":"esdieff.check-plan","version":"1", "sources":[{"key":"device","path":"missing","role":"model"}], "cases":[{"name":"sensor","operation":"check","scope":{"kind":"catalogue"},"assessmnt":"full-baseline"}]});
    files.write("plan.json", serde_json::to_vec(&plan).unwrap());
    let message = usage(files.run(&["check", "--plan", "plan.json"]));
    for fragment in [
        "plan.json",
        "sensor",
        "/cases/0/assessmnt",
        "unknown plan field",
        "assessment",
    ] {
        assert!(message.contains(fragment), "{message}");
    }
    plan["cases"][0] = json!({"name":"sensor","operation":"sample","document":"unknown-device","sample":"device","kind":"property","pointer":"/sdfProperty/p"});
    files.write("plan.json", serde_json::to_vec(&plan).unwrap());
    let message = usage(files.run(&["check", "--plan", "plan.json"]));
    for fragment in [
        "sensor",
        "/cases/0/document",
        "unknown-device",
        "declared in sources",
    ] {
        assert!(message.contains(fragment), "{message}");
    }
    plan["sources"] = json!([]);
    files.write("plan.json", serde_json::to_vec(&plan).unwrap());
    let message = usage(files.run(&["check", "--plan", "plan.json"]));
    assert!(message.contains("/sources"));
    assert!(message.contains("1 through 256"));
}

#[test]
fn destination_guidance_names_the_correct_replacement_flag_without_recommending_it_for_directories()
{
    let files = Files::new();
    files.write("model.json", b"{}");
    files.write("existing.json", b"unchanged");
    fs::create_dir(files.0.join("directory")).unwrap();
    for command in ["author", "edit"] {
        let common = [
            command,
            "--plan",
            "missing-plan.json",
            "--source",
            "model",
            "model.json",
        ];
        for (options, flag) in [
            (
                vec!["--document-output", "existing.json"],
                "--replace-document",
            ),
            (
                vec![
                    "--document-output",
                    "new.json",
                    "--report-output",
                    "existing.json",
                ],
                "--replace-report",
            ),
        ] {
            let args: Vec<_> = common.iter().chain(&options).copied().collect();
            let message = usage(files.run(&args));
            assert!(message.contains("existing.json"));
            assert!(message.contains(flag));
        }
        for extra in [vec![], vec!["--replace-document"]] {
            let options = ["--document-output", "directory"];
            let args: Vec<_> = common
                .iter()
                .chain(&options)
                .chain(&extra)
                .copied()
                .collect();
            let message = usage(files.run(&args));
            assert!(message.contains("directory"));
            assert!(message.contains("Choose another destination"));
            assert!(!message.contains("--replace-document"));
        }
        let message = usage(files.run(&[
            command,
            "--plan",
            "missing-plan.json",
            "--source",
            "model",
            "model.json",
            "--document-output",
            "model.json",
        ]));
        assert!(message.contains("protected source or plan input"));
        assert!(!message.contains("--replace-document"));
        #[cfg(unix)]
        {
            let link = files.0.join(format!("{command}-link.json"));
            std::os::unix::fs::symlink("existing.json", &link).unwrap();
            let options = [
                "--document-output",
                link.to_str().unwrap(),
                "--replace-document",
            ];
            let args: Vec<_> = common.iter().chain(&options).copied().collect();
            let message = usage(files.run(&args));
            assert!(message.contains("Choose another destination"));
            assert!(!message.contains("--replace-document"));
        }
    }
    let message = usage(files.run(&[
        "diff",
        "--old",
        "model.json",
        "--new",
        "model.json",
        "--acknowledgements-output",
        "existing.json",
    ]));
    assert!(message.contains("--replace-acknowledgements"));
    assert_eq!(
        fs::read(files.0.join("existing.json")).unwrap(),
        b"unchanged"
    );
    assert!(!files.0.join("new.json").exists());
}
