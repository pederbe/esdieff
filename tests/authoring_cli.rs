use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn setup() -> std::path::PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "esdieff-author-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    fs::write(path.join("base.json"), r#"{"namespace":{"b":"https://example.com/base"},"defaultNamespace":"b","sdfObject":{"Sensor":{"sdfProperty":{"temperature":{"type":"number","multipleOf":0.10,"maxLength":18446744073709551616}}}}}"#).unwrap();
    fs::write(path.join("plan.json"), r#"{"key":"recipe","generated_key":"product","thing_name":"Room","namespace":{"reuse":"https://example.com/base","p":"https://example.com/product"},"default_namespace":"p","objects":[{"name":"sensor","source":"base","pointer":"/sdfObject/Sensor","prefix":"reuse","refinement":{},"required":true}]}"#).unwrap();
    path
}
fn command(path: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
    command.current_dir(path).args([
        "author",
        "--plan",
        "plan.json",
        "--source",
        "base",
        "base.json",
        "--format",
        "json",
    ]);
    command
}
#[test]
fn author_delivers_separate_document_and_report_and_honors_check_policy() {
    let path = setup();
    let output = command(&path)
        .args(["--document-output", "product.json", "--check", "initial-v1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["operation"], "author");
    assert_eq!(report["outcome"]["disposition"], "passed");
    assert!(
        fs::read_to_string(path.join("product.json"))
            .unwrap()
            .contains("reuse:#/sdfObject/Sensor")
    );
    let output = command(&path)
        .args([
            "--document-output",
            "invalid.json",
            "--check",
            "full-baseline",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!path.join("invalid.json").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("formal.uint"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["result"]["checking"]
            ["state"],
        "requested"
    );
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn author_refuses_collisions_and_input_aliases_without_changing_sources() {
    let path = setup();
    let original = fs::read(path.join("base.json")).unwrap();
    for arguments in [
        vec!["--document-output", "-"],
        vec!["--document-output", "base.json", "--replace-document"],
        vec![
            "--document-output",
            "same.json",
            "--report-output",
            "same.json",
        ],
    ] {
        let output = command(&path).args(arguments).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::hard_link(path.join("base.json"), path.join("alias.json")).unwrap();
    let output = command(&path)
        .args(["--document-output", "alias.json", "--replace-document"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(path.join("base.json")).unwrap(), original);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn document_stdout_replacement_and_missing_acquisition_follow_output_policy() {
    let path = setup();
    let output = command(&path)
        .args(["--document-output", "-", "--report-output", "report.json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .unwrap()
            .get("sdfThing")
            .is_some()
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(path.join("report.json")).unwrap())
            .unwrap()["operation"],
        "author"
    );
    fs::write(path.join("product.json"), "old document").unwrap();
    let output = command(&path)
        .args(["--document-output", "product.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let output = command(&path)
        .args(["--document-output", "product.json", "--replace-document"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        fs::read_to_string(path.join("product.json"))
            .unwrap()
            .contains("sdfThing")
    );
    fs::remove_file(path.join("base.json")).unwrap();
    let output = command(&path)
        .args(["--document-output", "missing.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(!path.join("missing.json").exists());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["sources"][0]["acquisition"], "failed");
    assert!(
        report["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition["code"] == "reference.unavailable")
    );
    fs::remove_dir_all(path).unwrap();
}

#[cfg(unix)]
#[test]
fn symlinks_are_refused_and_report_failure_names_the_delivered_document() {
    let path = setup();
    std::os::unix::fs::symlink("base.json", path.join("link.json")).unwrap();
    let output = command(&path)
        .args(["--document-output", "link.json", "--replace-document"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let (failed_stdout, reader) = std::os::unix::net::UnixStream::pair().unwrap();
    drop(reader);
    let output = command(&path)
        .args(["--document-output", "product.json"])
        .stdout(std::process::Stdio::from(std::os::fd::OwnedFd::from(
            failed_stdout,
        )))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(4));
    assert!(path.join("product.json").exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already delivered"));
    assert!(!fs::read_dir(&path).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".esdieff-stage-")
    }));
    fs::remove_dir_all(path).unwrap();
}
