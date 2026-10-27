//! Required declaration inspection shares the source and effective report contract.
//! Follows ADR 0026: Required declaration relationships and source evidence.
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn inspect(view: &str, pointer: &str, format: &str, text: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "inspect",
            "--view",
            view,
            "--document",
            "-",
            "--pointer",
            pointer,
            "--required-declarations",
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
fn effective_cli_inspects_required_temperature() {
    let text = include_bytes!("fixtures/temperature-device.sdf.json");
    let pointer = "/sdfThing/Device/sdfObject/ambient";
    let output = inspect("effective", pointer, "json", text);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        wire["result"]["required_declarations"]["state"],
        "available"
    );
    assert_eq!(wire["result"]["required_declarations"]["complete"], true);
    let owners = wire["result"]["required_declarations"]["owners"]
        .as_array()
        .unwrap();
    let ambient = owners
        .iter()
        .find(|owner| owner["class"] == "sdfObject")
        .unwrap();
    assert_eq!(
        ambient["references"][0]["targets"][0]["given_name"],
        "temperature"
    );
    let human = inspect("effective", pointer, "human", text);
    assert_eq!(human.status.code(), Some(0));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Required declarations: available; complete: true"));
    assert!(human.contains("temperature"));
    assert!(human.contains("/sdfObject/Sensor/sdfProperty/temperature"));
    assert!(human.contains("/sdfThing/Device/sdfObject/ambient/sdfRef"));
}

#[test]
fn missing_required_source_selector_is_invocation_error_in_both_formats() {
    let text = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[true]}}}"#;
    for view in ["source", "effective"] {
        for format in ["json", "human"] {
            let missing = inspect(view, "/missing", format, text);
            assert_eq!(missing.status.code(), Some(2), "{view} {format}");
            assert!(missing.stdout.is_empty());
            assert!(
                String::from_utf8(missing.stderr)
                    .unwrap()
                    .contains("unknown inspection selector")
            );
            let valid = inspect(view, "/sdfObject/O", format, text);
            assert_eq!(valid.status.code(), Some(0));
            let unavailable = inspect(view, "/missing", format, b"not JSON");
            assert_eq!(unavailable.status.code(), Some(1));
            assert!(unavailable.stderr.is_empty());
        }
    }
}

#[test]
fn source_and_effective_cli_keep_states_counts_and_exit_agreement_across_formats() {
    type Case<'a> = (&'a [u8], &'a str, &'a str, bool, u64, u64, i32);
    let cases: &[Case<'_>] = &[
        (br#"{"info":{}}"#, "", "available", true, 0, 0, 0),
        (
            br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[]}}}"#,
            "/sdfObject/O",
            "available",
            true,
            0,
            0,
            0,
        ),
        (
            br#"{"info":{},"sdfObject":{"O":{"sdfRequired":false}}}"#,
            "/sdfObject/O",
            "available",
            true,
            1,
            0,
            1,
        ),
        (
            br#"{"info":{},"sdfObject":{"O":{"sdfRequired":["missing"]}}}"#,
            "/sdfObject/O",
            "partial",
            false,
            0,
            1,
            3,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"sdfRequired":[true]}}}"#,
            "/sdfData/D",
            "partial",
            false,
            0,
            0,
            3,
        ),
    ];
    for &(text, pointer, state, complete, errors, notices, exit) in cases {
        for view in ["source", "effective"] {
            let json = inspect(view, pointer, "json", text);
            assert_eq!(json.status.code(), Some(exit), "{view} {pointer}");
            assert!(json.stderr.is_empty());
            let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
            assert_eq!(wire["result"]["required_declarations"]["state"], state);
            assert_eq!(
                wire["result"]["required_declarations"]["complete"],
                complete
            );
            assert_eq!(wire["outcome"]["errors"], errors);
            assert_eq!(wire["outcome"]["notices"], notices);
            assert_eq!(wire["outcome"]["incomplete_checks"], u64::from(!complete));
            assert_eq!(wire["coverage"]["requested"]["total"], 1);
            let human = inspect(view, pointer, "human", text);
            assert_eq!(human.status.code(), Some(exit));
            assert!(human.stderr.is_empty());
            let human = String::from_utf8(human.stdout).unwrap();
            assert!(human.contains(&format!(
                "Required declarations: {state}; complete: {complete}"
            )));
            assert!(human.contains(&format!("Errors: {errors}")));
            assert!(human.contains(&format!("notices: {notices}")));
        }
    }
}
