//! The CLI translates explicit pattern assessments into the shared report.
mod coverage_support;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn run(args: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn syntax_findings_inventory_and_exit_policy_agree_in_both_formats() {
    for (pattern, errors, code, status) in [
        ("[", 1, Some("pattern.syntax"), "assessed"),
        ("a", 0, None, "assessed"),
        (
            "(?<x>a)|(?<x>b)",
            0,
            Some("pattern.unsupported"),
            "undetermined",
        ),
    ] {
        let source =
            serde_json::json!({"info":{},"sdfProperty":{"p":{"pattern":pattern}}}).to_string();
        for profile in ["expanded-pattern-syntax-v1", "full-baseline"] {
            for format in ["json", "human"] {
                let output = run(
                    &["check", "--assessment", profile, "--format", format, "-"],
                    source.as_bytes(),
                );
                assert_eq!(
                    output.status.code(),
                    Some(if errors > 0 {
                        1
                    } else if status == "undetermined" {
                        3
                    } else {
                        0
                    })
                );
                assert!(output.stderr.is_empty());
                if format == "json" {
                    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(wire["version"], "1.0");
                    assert_eq!(wire["outcome"]["errors"], errors);
                    assert_eq!(
                        wire["outcome"]["complete"],
                        status == "assessed"
                            && (profile == "expanded-pattern-syntax-v1" || errors == 0)
                    );
                    if profile == "expanded-pattern-syntax-v1" {
                        assert_eq!(
                            wire["request"]["assessment"],
                            "esdieff.expanded.pattern-syntax/1"
                        );
                        assert_eq!(wire["coverage"]["requested"]["total"], 9);
                        assert!(
                            wire["coverage"]["rows"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|row| row["id"] == "pattern-syntax/1"
                                    && row["status"] == status)
                        );
                    } else {
                        assert_eq!(
                            wire["coverage"]["requested"]["inventory"],
                            "esdieff.full-model/1"
                        );
                        assert_eq!(wire["coverage"]["requested"]["total"], 13);
                        coverage_support::assert_wire_counts(&wire);
                    }
                    if let Some(code) = code {
                        let condition = wire["diagnostics"]["items"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|item| {
                                item["code"] == code
                                    && item["assessment_unit"] == "pattern-syntax/1"
                            })
                            .unwrap();
                        assert_eq!(condition["assessment_unit"], "pattern-syntax/1");
                        assert_eq!(condition["primary"]["pointer"], "/sdfProperty/p/pattern");
                        assert_eq!(condition["related"][0]["pointer"], "/sdfProperty/p/pattern");
                    }
                } else {
                    let human = String::from_utf8(output.stdout).unwrap();
                    assert!(human.contains(&format!("Errors: {errors};")));
                    if let Some(code) = code {
                        assert!(human.contains(code));
                    }
                    if profile == "expanded-pattern-syntax-v1" {
                        assert!(
                            human.contains(&format!("Assessment area pattern-syntax/1: {status}"))
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn existing_purposes_and_invocation_errors_remain_distinct() {
    let source = br#"{"info":{},"sdfProperty":{"p":{"pattern":"["}}}"#;
    for profile in ["initial-v1", "expanded-modified-v1", "expanded-units-v1"] {
        let output = run(
            &["check", "--assessment", profile, "--format", "json", "-"],
            source,
        );
        assert_eq!(output.status.code(), Some(0));
        let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(wire["outcome"]["errors"], 0);
        assert!(
            !wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"].as_str().unwrap().starts_with("pattern."))
        );
    }
    let inspected = run(
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            "-",
            "--pointer",
            "/sdfProperty/p",
            "--format",
            "json",
            "-",
        ],
        source,
    );
    assert_eq!(inspected.status.code(), Some(0));
    let output = run(
        &[
            "check",
            "--assessment",
            "expanded-pattern-syntax-v1",
            "--registry-knowledge",
            "recorded",
            "-",
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("registry knowledge policy requires a registry assessment")
    );
}
