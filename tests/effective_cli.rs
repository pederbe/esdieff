//! Effective inspection shares outcome, locations and provenance across formats.
//! Follows ADR 0020: Owned effective views and nearest origins.
mod coverage_support;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn inspect(pointer: &str, format: &str, text: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "inspect",
            "--view",
            "effective",
            "--document",
            "-",
            "--pointer",
            pointer,
            "--format",
            format,
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(text).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn effective_fixture_cli_reports_matching_values_and_nearest_origins_in_both_formats() {
    let pointer = "/sdfThing/Device/sdfObject/ambient";
    let text = include_bytes!("fixtures/temperature-device.sdf.json");
    let json = inspect(pointer, "json", text);
    assert_eq!(json.status.code(), Some(0));
    assert!(json.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(wire["coverage"]["requested"]["count_unit"], "inspection");
    assert_eq!(
        wire["result"]["json"]["sdfProperty"]["temperature"]["maximum"],
        100
    );
    let minimum = wire["result"]["explanations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| {
            value["effective"]["pointer"] == format!("{pointer}/sdfProperty/temperature/minimum")
        })
        .unwrap();
    assert_eq!(minimum["kind"], "inherited");
    assert_eq!(
        minimum["sources"][0]["pointer"],
        "/sdfData/Temperature/minimum"
    );
    let human = inspect(pointer, "human", text);
    assert_eq!(human.status.code(), Some(0));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Complete: true"));
    assert!(human.contains("\"maximum\":100"));
    assert!(human.contains("/sdfData/Temperature/minimum"));
    assert!(human.contains("inherited") && human.contains("override") && human.contains("deleted"));
}

#[test]
fn effective_cli_reports_missing_and_cycle_paths_with_undetermined_exit() {
    let text=br##"{"sdfData":{"Missing":{"sdfRef":"#/sdfData/Absent"},"Cycle":{"sdfRef":"#/sdfData/Cycle"}}}"##;
    let json = inspect("", "json", text);
    assert_eq!(json.status.code(), Some(3));
    assert!(json.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    coverage_support::assert_wire_counts(&wire);
    for (code, pointer) in [
        ("reference.unavailable", "/sdfData/Missing"),
        ("reference.effective_cycle", "/sdfData/Cycle"),
    ] {
        let condition = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|condition| condition["code"] == code)
            .unwrap();
        assert_eq!(condition["primary"]["kind"], "effective");
        assert_eq!(condition["primary"]["pointer"], pointer);
    }
    let human = inspect("", "human", text);
    assert_eq!(human.status.code(), Some(3));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Complete: false"));
    assert!(human.contains("/sdfData/Missing") && human.contains("/sdfData/Cycle"));
}
