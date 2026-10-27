//! Shared scalar outcomes and specified defaults in human and JSON CLI output.
//! Follows ADR 0011: Machine-readable report contracts, and
//! ADR 0020: Owned effective views and nearest origins.
mod coverage_support;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn run(args: &[&str], text: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(text).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn effective_inspection_reports_deleted_and_specified_boolean_default_values_in_both_formats() {
    let text = br##"{"info":{},"sdfProperty":{"Base":{"writable":false},"value":{"sdfRef":"#/sdfProperty/Base","writable":null}}}"##;
    let json = run(
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            "-",
            "--pointer",
            "/sdfProperty/value",
            "--format",
            "json",
            "-",
        ],
        text,
    );
    assert_eq!(json.status.code(), Some(0));
    assert!(json.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert!(wire["result"]["json"].get("writable").is_none());
    let rows = wire["result"]["explanations"].as_array().unwrap();
    let default = rows
        .iter()
        .find(|row| {
            row["kind"] == "default" && row["effective"]["pointer"] == "/sdfProperty/value/writable"
        })
        .unwrap();
    assert_eq!(default["default_value"], true);
    assert_eq!(default["sources"], serde_json::json!([]));
    let deleted = rows
        .iter()
        .find(|row| {
            row["kind"] == "deleted" && row["effective"]["pointer"] == "/sdfProperty/value/writable"
        })
        .unwrap();
    assert!(deleted.get("default_value").is_none());
    assert_eq!(
        deleted["sources"][0]["pointer"],
        "/sdfProperty/value/writable"
    );
    for row in rows.iter().filter(|row| row["kind"] != "default") {
        assert!(row.get("default_value").is_none());
    }
    let human = run(
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            "-",
            "--pointer",
            "/sdfProperty/value",
            "--format",
            "human",
            "-",
        ],
        text,
    );
    assert_eq!(human.status.code(), Some(0));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("default true at") && human.contains("/sdfProperty/value/writable"));
    assert!(human.contains("deleted at"));
    assert!(human.contains("Complete: true"));
}

#[test]
fn scalar_parameter_check_reports_the_same_error_paths_totals_and_coverage_in_both_formats() {
    let text = br##"{"info":{},"sdfData":{"Base":{"multipleOf":0}},"sdfProperty":{"Inherited":{"sdfRef":"#/sdfData/Base"},"Deleted":{"sdfRef":"#/sdfData/Base","multipleOf":null}}}"##;
    let json = run(&["check", "--format", "json", "-"], text);
    assert_eq!(json.status.code(), Some(1));
    assert!(json.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 2);
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(wire["outcome"]["disposition"], "failed");
    assert_eq!(
        wire["coverage"]["requested"]["inventory"],
        "esdieff.initial/1"
    );
    assert_eq!(wire["coverage"]["requested"]["unassessed"], 0);
    let steps: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["code"] == "parameter.positive_step")
        .collect();
    assert_eq!(steps.len(), 2);
    assert!(steps.iter().any(|row| row["primary"]["kind"] == "source"
        && row["primary"]["pointer"] == "/sdfData/Base/multipleOf"));
    assert!(steps.iter().any(|row| row["primary"]["kind"] == "effective"
        && row["primary"]["pointer"] == "/sdfProperty/Inherited/multipleOf"));
    let human = run(&["check", "--format", "human", "-"], text);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 2;") && human.contains("Complete: true"));
    assert_eq!(human.matches("parameter.positive_step").count(), 2);
    assert!(
        human.contains("/sdfData/Base/multipleOf")
            && human.contains("/sdfProperty/Inherited/multipleOf")
    );
    assert!(human.contains("esdieff.initial/1"));
}

#[test]
fn expanded_date_and_scalar_check_keeps_matching_cli_errors_and_native_coverage() {
    let text = br#"{"info":{"modified":"1900-02-29"},"sdfProperty":{"value":{"multipleOf":0}}}"#;
    let args = ["check", "--assessment", "expanded-modified-v1", "--format"];
    let json = run(&[args[0], args[1], args[2], args[3], "json", "-"], text);
    assert_eq!(json.status.code(), Some(1));
    assert!(json.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 2);
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(
        wire["coverage"]["requested"]["inventory"],
        "esdieff.expanded.modified/1"
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    coverage_support::assert_wire_counts(&wire);
    assert_eq!(wire["coverage"]["requested"]["unassessed"], 0);
    for (code, pointer) in [
        ("metadata.modified_calendar", "/info/modified"),
        ("parameter.positive_step", "/sdfProperty/value/multipleOf"),
    ] {
        assert_eq!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["code"] == code && row["primary"]["pointer"] == pointer)
                .count(),
            1
        );
    }
    let human = run(&[args[0], args[1], args[2], args[3], "human", "-"], text);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 2;") && human.contains("Complete: true"));
    assert!(
        human.contains("metadata.modified_calendar") && human.contains("parameter.positive_step")
    );
    assert!(human.contains("esdieff.expanded.modified/1"));
    assert!(human.contains("Assessment area modified-date-time/1: assessed"));
}
