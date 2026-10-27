//! CLI acquisition and reports share supplied composition and scope outcomes.
mod coverage_support;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

const CONTROLLER: &[u8] = br##"{"namespace":{"level":"https://example.com/level"},"sdfObject":{"Controller":{"sdfProperty":{"level":{"sdfRef":"level:#/sdfData/Level","maximum":80,"description":null}}}}}"##;
const LEVEL: &[u8] = br##"{"namespace":{"data":"https://example.com/level"},"defaultNamespace":"data","sdfData":{"Level":{"type":"number","minimum":0,"maximum":100,"description":"Reusable level"}}}"##;
struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(content: &[u8]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-cross-cli-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(content).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn run(args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn supplied_controller_inspection_agrees_in_human_json_and_reordered_inputs() {
    let level = InputFile::new(LEVEL);
    for reverse in [false, true] {
        for format in ["json", "human"] {
            let mut args = vec![
                "inspect",
                "--view",
                "effective",
                "--document",
                "-",
                "--pointer",
                "/sdfObject/Controller/sdfProperty/level",
                "--format",
                format,
            ];
            args.extend(if reverse {
                [level.path(), "-"]
            } else {
                ["-", level.path()]
            });
            let output = run(&args, CONTROLLER);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["complete"], true);
                assert_eq!(wire["result"]["json"]["minimum"], 0);
                assert_eq!(wire["result"]["json"]["maximum"], 80);
                assert!(wire["result"]["json"].get("description").is_none());
                let root = if reverse { 2 } else { 1 };
                assert_eq!(wire["request"]["scope"]["roots"], serde_json::json!([root]));
                assert_eq!(
                    wire["request"]["scope"]["dependencies"],
                    serde_json::json!([if reverse { 1 } else { 2 }])
                );
                for row in wire["result"]["explanations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|row| row.get("consumer").is_some())
                {
                    assert_eq!(row["consumer"]["source_id"], root);
                    assert_eq!(
                        row["consumer"]["pointer"],
                        "/sdfObject/Controller/sdfProperty/level/sdfRef"
                    );
                }
            } else {
                let human = String::from_utf8(output.stdout).unwrap();
                assert!(human.contains("Complete: true"));
                assert!(human.contains("\"minimum\":0") && human.contains("\"maximum\":80"));
                assert!(human.contains("imported at") && human.contains("/sdfData/Level/minimum"));
            }
        }
    }
}
#[test]
fn explicit_root_check_and_dependency_modified_profile_keep_matching_exit_and_scope() {
    let bad = String::from_utf8(LEVEL.to_vec())
        .unwrap()
        .replace(
            "\"namespace\"",
            "\"info\":{\"title\":7,\"modified\":7},\"namespace\"",
        )
        .replace("\"Level\":", "\"Unused\":{\"description\":7},\"Level\":");
    let level = InputFile::new(bad.as_bytes());
    for format in ["json", "human"] {
        let selected = run(
            &[
                "check",
                "--root",
                "-",
                "--format",
                format,
                "-",
                level.path(),
            ],
            CONTROLLER,
        );
        assert_eq!(selected.status.code(), Some(0));
        assert!(selected.stderr.is_empty());
        let all = run(
            &["check", "--format", format, "-", level.path()],
            CONTROLLER,
        );
        assert_eq!(all.status.code(), Some(1));
        let expanded = run(
            &[
                "check",
                "--root",
                "-",
                "--assessment",
                "expanded-modified-v1",
                "--format",
                format,
                "-",
                level.path(),
            ],
            CONTROLLER,
        );
        assert_eq!(expanded.status.code(), Some(1));
        assert!(expanded.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&expanded.stdout).unwrap();
            assert_eq!(wire["outcome"]["errors"], 1);
            assert_eq!(
                wire["request"]["scope"]["dependencies"],
                serde_json::json!([2])
            );
            assert_eq!(wire["coverage"]["requested"]["total"], 9);
            coverage_support::assert_wire_counts(&wire);
            coverage_support::assert_wire_counts(&wire);
            let advisory = wire["coverage"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == "advisory-four/1")
                .unwrap();
            assert_eq!(advisory["status"], "not_applicable");
        } else {
            let human = String::from_utf8(expanded.stdout).unwrap();
            assert!(human.contains("Errors: 1"));
            assert!(human.contains("Assessment area modified-date-time/1: assessed"));
            assert!(
                human.contains(
                    "total 9; assessed 8; not applicable 1; unassessed 0; undetermined 0"
                )
            );
            assert!(human.contains("/info/modified"));
        }
    }
}

#[test]
fn imported_boolean_defaults_report_values_and_consumer_boundaries_in_both_formats() {
    let base=InputFile::new(br##"{"info":{},"namespace":{"d":"https://example.com/base"},"defaultNamespace":"d","sdfProperty":{"Base":{"type":"number","nullable":false,"writable":false,"minimum":0}}}"##);
    let root=br##"{"info":{},"namespace":{"d":"https://example.com/base"},"sdfProperty":{"Use":{"sdfRef":"d:#/sdfProperty/Base","nullable":null,"writable":null}}}"##;
    for reverse in [false, true] {
        for format in ["json", "human"] {
            let mut args = vec![
                "inspect",
                "--view",
                "effective",
                "--document",
                "-",
                "--pointer",
                "/sdfProperty/Use",
                "--format",
                format,
            ];
            args.extend(if reverse {
                [base.path(), "-"]
            } else {
                ["-", base.path()]
            });
            let output = run(&args, root);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                for quality in ["nullable", "writable"] {
                    assert!(wire["result"]["json"].get(quality).is_none());
                    let path = format!("/sdfProperty/Use/{quality}");
                    let rows = wire["result"]["explanations"].as_array().unwrap();
                    let default = rows
                        .iter()
                        .find(|row| row["kind"] == "default" && row["effective"]["pointer"] == path)
                        .unwrap();
                    assert_eq!(default["default_value"], true);
                    assert_eq!(default["sources"], serde_json::json!([]));
                    assert_eq!(
                        default["consumer"]["source_id"],
                        if reverse { 2 } else { 1 }
                    );
                    assert_eq!(default["consumer"]["pointer"], "/sdfProperty/Use/sdfRef");
                    let deletion = rows
                        .iter()
                        .find(|row| row["kind"] == "deleted" && row["effective"]["pointer"] == path)
                        .unwrap();
                    assert!(deletion.get("default_value").is_none());
                    assert_eq!(deletion["consumer"]["pointer"], "/sdfProperty/Use/sdfRef");
                }
            } else {
                let human = String::from_utf8(output.stdout).unwrap();
                assert!(human.contains("default true at"));
                assert!(human.contains("imported at"));
                assert!(human.contains("/sdfProperty/Use/sdfRef"));
                assert!(human.contains("/sdfProperty/Use/writable"));
                assert!(human.contains("/sdfProperty/Use/nullable"));
                assert!(human.contains("Complete: true"));
            }
        }
    }
}

#[test]
fn imported_compound_findings_keep_native_uncertainty_and_matching_cli_outcomes() {
    let base=InputFile::new(br##"{"info":{"title":7,"modified":"2030-12-31T23:59:60Z"},"namespace":{"d":"https://example.com/base"},"defaultNamespace":"d","sdfProperty":{"Base":{"type":"string","required":[],"multipleOf":0,"items":{"type":"string","minLength":-1,"minItems":-1,"nullable":[],"multipleOf":0}},"Unused":{"description":7}}}"##);
    let root=br##"{"info":{},"namespace":{"d":"https://example.com/base"},"sdfProperty":{"Use":{"sdfRef":"d:#/sdfProperty/Base","type":"object"}}}"##;
    for format in ["json", "human"] {
        let output = run(
            &[
                "check",
                "--root",
                "-",
                "--assessment",
                "expanded-modified-v1",
                "--format",
                format,
                "-",
                base.path(),
            ],
            root,
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["outcome"]["errors"], 3);
            assert_eq!(wire["outcome"]["complete"], false);
            assert_eq!(wire["outcome"]["disposition"], "failed");
            assert_eq!(wire["coverage"]["requested"]["total"], 9);
            coverage_support::assert_wire_counts(&wire);
            assert_eq!(
                wire["request"]["scope"]["dependencies"],
                serde_json::json!([2])
            );
            assert!(wire["baseline"]["temporal_knowledge"].is_object());
            for (code, path) in [
                ("data.required", "/sdfProperty/Use/required"),
                ("parameter.positive_step", "/sdfProperty/Use/multipleOf"),
                ("data.count", "/sdfProperty/Use/items/minLength"),
            ] {
                assert!(wire["diagnostics"]["items"].as_array().unwrap().iter().any(
                    |row| row["code"] == code
                        && row["primary"]["kind"] == "effective"
                        && row["primary"]["pointer"] == path
                ));
            }
            assert!(
                !wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["code"] == "metadata.string")
            );
        } else {
            let human = String::from_utf8(output.stdout).unwrap();
            assert!(human.contains("Errors: 3;"));
            assert!(human.contains("Complete: false"));
            assert!(human.contains("Assessment area modified-date-time/1: undetermined"));
            for code in ["data.required", "parameter.positive_step", "data.count"] {
                assert!(human.contains(code));
            }
            assert!(!human.contains("metadata.string"));
        }
    }
}

#[test]
fn undecodable_mandatory_reference_reports_unknown_date_scope_in_both_formats() {
    let level=InputFile::new(br##"{"info":{},"namespace":{"own":"https://example.com/level"},"defaultNamespace":"own","sdfData":{"Level":{"sdfRef":"\uD800","type":"number"}}}"##);
    let root=br##"{"info":{},"namespace":{"level":"https://example.com/level"},"sdfProperty":{"Controller":{"sdfRef":"level:#/sdfData/Level"}}}"##;
    for reverse in [false, true] {
        for format in ["json", "human"] {
            let mut args = vec![
                "check",
                "--root",
                "-",
                "--assessment",
                "expanded-modified-v1",
                "--format",
                format,
            ];
            args.extend(if reverse {
                [level.path(), "-"]
            } else {
                ["-", level.path()]
            });
            let output = run(&args, root);
            assert_eq!(output.status.code(), Some(3));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["errors"], 0);
                assert_eq!(wire["outcome"]["complete"], false);
                assert_eq!(wire["outcome"]["disposition"], "undetermined");
                assert_eq!(wire["coverage"]["requested"]["total"], 9);
                coverage_support::assert_wire_counts(&wire);
                coverage_support::assert_wire_counts(&wire);
                assert_eq!(
                    wire["request"]["scope"]["dependencies"],
                    serde_json::json!([if reverse { 1 } else { 2 }])
                );
                assert!(wire["baseline"].get("temporal_knowledge").is_none());
            } else {
                let human = String::from_utf8(output.stdout).unwrap();
                assert!(human.contains("Errors: 0;"));
                assert!(human.contains("Complete: false"));
                assert!(human.contains("Assessment area modified-date-time/1: undetermined"));
            }
        }
    }
}
