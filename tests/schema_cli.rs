use serde_json::Value;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(model: &str, extra: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
    command.args([
        "schema",
        "--document",
        "-",
        "--kind",
        "property",
        "--pointer",
        "/sdfProperty/value",
    ]);
    command
        .args(extra)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let written = child.stdin.take().unwrap().write_all(model.as_bytes());
    let output = child.wait_with_output().unwrap();
    if let Err(error) = written {
        assert!(
            error.kind() == std::io::ErrorKind::BrokenPipe && output.status.code() == Some(2),
            "stdin write failed: {error}"
        );
    }
    output
}

#[test]
fn json_report_and_standalone_schema_preserve_numbers_and_ignore_color() {
    let model = r#"{"sdfProperty":{"value":{"type":"number","nullable":false,"minimum":0.10000000000000001}}}"#;
    let report = run(model, &["--color=always"]);
    assert_eq!(
        report.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    assert!(report.stderr.is_empty());
    assert!(!report.stdout.contains(&0x1b));
    assert!(String::from_utf8_lossy(&report.stdout).contains("0.10000000000000001"));
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["fidelity"], "json-values");
    assert_eq!(report["contract"], "esdieff.schema/1");
    let raw = run(model, &["--format", "schema"]);
    assert_eq!(raw.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&raw.stdout).unwrap(),
        report["schema"]
    );
}

#[test]
fn refusals_are_machine_readable_and_raw_mode_writes_no_schema() {
    let model = r#"{"sdfProperty":{"value":{"type":"string","format":"date-time"}}}"#;
    let report = run(model, &[]);
    assert_eq!(report.status.code(), Some(3));
    assert!(report.stderr.is_empty());
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["fidelity"], "refused");
    assert!(report["schema"].is_null());
    let raw = run(model, &["--format", "schema"]);
    assert_eq!(raw.status.code(), Some(3));
    assert!(raw.stdout.is_empty());
    assert!(String::from_utf8_lossy(&raw.stderr).contains("format"));
}

#[test]
fn invalid_selectors_and_duplicate_or_unknown_options_are_invocation_errors() {
    let model = r#"{"sdfProperty":{"value":{"type":"boolean"}}}"#;
    for extra in [
        &["--kind", "data"][..],
        &["--format", "human"],
        &["--bad"],
        &["--pointer", "bad"],
        &["--document", "-"],
    ] {
        let output = run(model, extra);
        assert_eq!(output.status.code(), Some(2), "{extra:?}");
        assert!(output.stdout.is_empty());
    }
    // Larger than the pipe buffer, so an early invocation rejection necessarily
    // closes stdin while the helper is still writing.
    let output = run(&" ".repeat(2 * 1024 * 1024), &["--bad"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["schema", "--help"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("json|schema"));
}
