//! Declared-value advice shares library aggregates, coverage and original locations.
//! Follows ADR 0031: Bounded declared-value consistency.
use esdieff::{Assessment, Catalogue, CheckRequest, SourceInput};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn run(args: &[&str], bytes: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn compound_consistency_matches_rust_json_human_and_strict_policy() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"object","default":{"a":0},"properties":{"a":{"type":"number","minimum":1}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", bytes)]).unwrap();
    for strict in [false, true] {
        let report = catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::ExpandedConsistency)
                    .strict_warnings(strict),
            )
            .unwrap();
        let mut args = vec![
            "check",
            "--assessment",
            "expanded-consistency-v1",
            "--format",
            "json",
        ];
        if strict {
            args.push("--strict-warnings");
        }
        args.push("-");
        let json = run(&args, bytes);
        assert_eq!(
            json.status.code(),
            Some(i32::from(report.outcome().disposition().exit_code()))
        );
        assert!(json.stderr.is_empty());
        let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(
            wire["request"]["inventory"],
            "esdieff.expanded.consistency/1"
        );
        assert_eq!(wire["coverage"]["requested"]["total"], 10);
        assert_eq!(wire["outcome"]["warnings"], report.outcome().warnings());
        assert_eq!(wire["outcome"]["warnings"], 1);
        assert_eq!(wire["outcome"]["errors"], 0);
        assert_eq!(wire["outcome"]["complete"], report.outcome().complete());
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "consistency.default_excluded")
            .unwrap();
        assert_eq!(finding["primary"]["kind"], "source");
        assert_eq!(finding["primary"]["pointer"], "/sdfData/value/default/a");
        assert_eq!(finding["basis"]["kind"], "advice");
        let human_args: Vec<_> = args
            .iter()
            .map(|arg| if *arg == "json" { "human" } else { *arg })
            .collect();
        let human = run(&human_args, bytes);
        assert_eq!(human.status.code(), json.status.code());
        assert!(human.stderr.is_empty());
        let human = String::from_utf8(human.stdout).unwrap();
        for text in [
            "esdieff.expanded.consistency/1",
            "total 10",
            "warnings: 1",
            "Assessment area const-consistency/1: not_applicable",
            "Assessment area default-consistency/1: assessed",
            "consistency.default_excluded",
            "/sdfData/value/default/a",
            "/sdfData/value/properties/a/minimum",
        ] {
            assert!(human.contains(text), "{text}");
        }
    }
}

#[test]
fn unknown_candidate_work_is_explicit_and_initial_membership_is_unchanged() {
    let bytes =
        br#"{"info":{},"sdfData":{"value":{"type":"string","default":"x","pattern":"(?<x>a)|(?<x>b)"}}}"#;
    let expanded = run(
        &[
            "check",
            "--assessment",
            "expanded-consistency-v1",
            "--format",
            "json",
            "-",
        ],
        bytes,
    );
    assert_eq!(expanded.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&expanded.stdout).unwrap();
    assert_eq!(wire["outcome"]["warnings"], 0);
    assert_eq!(wire["outcome"]["complete"], false);
    let row = wire["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "default-consistency/1")
        .unwrap();
    assert_eq!(row["status"], "undetermined");
    assert_eq!(row["basis"]["kind"], "operational");
    let initial = run(&["check", "--format", "json", "-"], bytes);
    assert_eq!(initial.status.code(), Some(0));
    let initial: Value = serde_json::from_slice(&initial.stdout).unwrap();
    assert_eq!(initial["request"]["inventory"], "esdieff.initial/1");
    assert_eq!(initial["coverage"]["requested"]["total"], 8);
    assert_eq!(initial["coverage"]["rows"].as_array().unwrap().len(), 8);
}

#[test]
fn independent_owner_default_advice_keeps_reference_incompleteness_in_every_output() {
    let bytes = br##"{"info":{},"sdfData":{"value":{"type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":{"first":{"properties":{"optional":{"sdfRef":"#/sdfData/Absent"}}},"second":{}}}}}
"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", bytes)]).unwrap();
    for strict in [false, true] {
        let report = catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::ExpandedConsistency)
                    .strict_warnings(strict),
            )
            .unwrap();
        let mut args = vec![
            "check",
            "--assessment",
            "expanded-consistency-v1",
            "--format",
            "json",
        ];
        if strict {
            args.push("--strict-warnings");
        }
        args.push("-");
        let json = run(&args, bytes);
        assert_eq!(json.status.code(), Some(if strict { 1 } else { 3 }));
        assert_eq!(
            json.status.code(),
            Some(i32::from(report.outcome().disposition().exit_code()))
        );
        assert!(json.stderr.is_empty());
        let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(wire["outcome"]["warnings"], report.outcome().warnings());
        assert_eq!(wire["outcome"]["warnings"], 1);
        assert_eq!(wire["outcome"]["errors"], 0);
        assert_eq!(wire["outcome"]["complete"], false);
        assert!(!report.outcome().complete());
        assert_eq!(wire["coverage"]["requested"]["total"], 10);
        let items = wire["diagnostics"]["items"].as_array().unwrap();
        for code in [
            "reference.unavailable",
            "interpretation.unresolved",
            "consistency.default_excluded",
        ] {
            assert!(items.iter().any(|item| item["code"] == code), "{code}");
        }
        let advice: Vec<_> = items
            .iter()
            .filter(|item| item["code"] == "consistency.default_excluded")
            .collect();
        assert_eq!(advice.len(), 1);
        assert_eq!(advice[0]["basis"]["kind"], "advice");
        assert_eq!(advice[0]["primary"]["kind"], "source");
        assert_eq!(advice[0]["primary"]["pointer"], "/sdfData/value/default");
        assert!(advice[0]["related"].as_array().unwrap().iter().any(|row|
            row["kind"] == "source" && row["pointer"] == "/sdfData/value/required/0"));
        let human_args: Vec<_> = args
            .iter()
            .map(|arg| if *arg == "json" { "human" } else { *arg })
            .collect();
        let human = run(&human_args, bytes);
        assert_eq!(human.status.code(), json.status.code());
        assert!(human.stderr.is_empty());
        let human = String::from_utf8(human.stdout).unwrap();
        for text in [
            "total 10",
            "warnings: 1",
            "undetermined",
            "reference.unavailable",
            "interpretation.unresolved",
            "consistency.default_excluded",
            "/sdfData/value/default",
            "/sdfData/value/required/0",
        ] {
            assert!(human.contains(text), "{text}");
        }
    }
}
