//! `esdieff trace` checks a Recorded trace against one selected model document. Follows
//! ADR 0042: Recorded traces with independent entries.
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

const THERMOSTAT: &str = include_str!("../examples/thermostat-check/thermostat.sdf.json");
const TARGET: &str = "/sdfObject/Thermostat/sdfProperty/targetTemperature";

/// A temporary file that is removed again when the test ends.
struct TempFile(PathBuf);
impl TempFile {
    fn new(bytes: impl AsRef<[u8]>) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-trace-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn target(value: &str, rest: &str) -> String {
    format!("{{\"kind\":\"property\",\"pointer\":\"{TARGET}\",\"value\":{value}{rest}}}\n")
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .output()
        .unwrap()
}

/// `esdieff trace` of `trace` against the thermostat model, with `extra` options.
fn trace(trace: &str, extra: &[&str]) -> Output {
    let model = TempFile::new(THERMOSTAT);
    let file = TempFile::new(trace);
    let mut args = vec!["trace", "--document", model.path(), "--trace", file.path()];
    args.extend(extra);
    args.push(model.path());
    run(&args)
}

fn exits(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_rejected_entry_fails_the_run_with_its_line_and_label() {
    let entries = [target("21", ""), target("35", r#","label":"too hot""#)].concat();
    let output = trace(&entries, &[]);
    exits(&output, 1);
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.starts_with("Recorded trace esdieff.trace/1: failed\n"),
        "{text}"
    );
    assert!(
        text.contains("Entries: 1 passed, 1 failed, 0 undetermined\n"),
        "{text}"
    );
    assert!(
        text.contains(&format!("\nline 2 property \"{TARGET}\": failed\n")),
        "{text}"
    );
    assert!(text.contains("  label \"too hot\"\n"), "{text}");
    let output = trace(&entries, &["--format", "json"]);
    exits(&output, 1);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["operation"], "trace");
    assert_eq!(report["result"]["entries"][0]["line"], 2);
    assert_eq!(report["result"]["entries"][0]["label"], "too hot");
}

#[test]
fn passing_entries_exit_0_and_only_undetermined_entries_exit_3() {
    exits(&trace(&target("21", ""), &[]), 0);
    let model = TempFile::new(r#"{"sdfProperty":{"code":{"type":"string","pattern":"["}}}"#);
    let file = TempFile::new(
        "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/code\",\"value\":\"a\",\"expected\":\"rejected\"}\n",
    );
    let output = run(&[
        "trace",
        "--document",
        model.path(),
        "--trace",
        file.path(),
        model.path(),
    ]);
    exits(&output, 3);
}

#[test]
fn unavailable_direct_item_meaning_is_explained_in_json_and_human_reports() {
    let model = TempFile::new(
        r#"{"sdfProperty":{"readings":{"type":"array","items":{"type":"string","pattern":"^a$"}}}}"#,
    );
    let file = TempFile::new(concat!(
        "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/readings\",\"value\":[\"a\"]}\n",
        "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/readings\",\"value\":[\"b\"]}\n",
    ));
    for format in ["json", "human"] {
        let output = run(&[
            "trace",
            "--document",
            model.path(),
            "--trace",
            file.path(),
            "--format",
            format,
            "--color",
            "never",
            model.path(),
        ]);
        exits(&output, 3);
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        if format == "json" {
            let report: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(report["version"], "1.5");
            assert_eq!(report["result"]["summary"]["undetermined"], 2);
            for entry in report["result"]["entries"].as_array().unwrap() {
                assert_eq!(entry["admission"], "undetermined");
                let condition = &entry["conditions"][0];
                assert_eq!(condition["code"], "sample.coverage_unavailable");
                assert_eq!(condition["severity"], "incomplete");
                assert_eq!(condition["primary"]["pointer"], "/value");
                assert_eq!(condition["basis"]["id"], "sample-establishment");
                assert_eq!(entry["conditions_omitted"], 0);
            }
        } else {
            assert!(
                text.contains("0 passed, 0 failed, 2 undetermined"),
                "{text}"
            );
            assert_eq!(text.matches("sample.coverage_unavailable").count(), 2);
            assert!(text.contains("Sample admission requires checks that could not be completed"));
        }
    }
}

#[test]
fn the_trace_can_come_from_standard_input() {
    let model = TempFile::new(THERMOSTAT);
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "trace",
            "--document",
            model.path(),
            "--trace",
            "-",
            "--format",
            "json",
            model.path(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(target("40", "").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    exits(&output, 1);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["result"]["summary"]["failed"], 1);
}

/// The run exits 2 with empty stdout and a reason on stderr.
fn refused(output: &Output) {
    exits(output, 2);
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn an_unreadable_or_oversized_trace_is_an_invocation_error() {
    let model = TempFile::new(THERMOSTAT);
    let missing =
        std::env::temp_dir().join(format!("esdieff-trace-missing-{}", std::process::id()));
    let directory = std::env::temp_dir();
    for path in [missing.to_str().unwrap(), directory.to_str().unwrap()] {
        refused(&run(&[
            "trace",
            "--document",
            model.path(),
            "--trace",
            path,
            model.path(),
        ]));
    }
    refused(&trace(&" ".repeat(64 * 1024 * 1024 + 1), &[]));
    refused(&trace(&" ".repeat(1024 * 1024 + 1), &[]));
    refused(&trace(&"{}\n".repeat(100_001), &[]));
}

#[test]
fn malformed_invocations_exit_2() {
    let model = TempFile::new(THERMOSTAT);
    let file = TempFile::new(target("21", ""));
    for args in [
        vec!["trace", "--document", model.path(), model.path()],
        vec!["trace", "--trace", file.path(), model.path()],
        vec![
            "trace",
            "--document",
            model.path(),
            "--trace",
            file.path(),
            "--format",
            "jsonl",
            model.path(),
        ],
        vec!["trace", "--document", "-", "--trace", "-", "-"],
        vec![
            "trace",
            "--document",
            "absent.sdf.json",
            "--trace",
            file.path(),
            model.path(),
        ],
    ] {
        refused(&run(&args));
    }
}
