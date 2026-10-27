//! CLI acquisition, selected roots and requested coverage.
mod coverage_support;
use coverage_support::INITIAL_AREAS;
use serde_json::Value;
use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn initial_wire(wire: &Value, root: Option<&str>) {
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["request"]["inventory"], "esdieff.initial/1");
    assert_eq!(
        wire["coverage"]["requested"]["count_unit"],
        "assessment_area"
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert_eq!(wire["coverage"]["requested"]["unassessed"], 0);
    coverage_support::assert_wire_counts(wire);
    assert_eq!(wire["coverage"]["broader"]["state"], "partial");
    let rows = wire["coverage"]["rows"].as_array().unwrap();
    for area in INITIAL_AREAS {
        let row = rows.iter().find(|row| row["id"] == area).unwrap();
        assert!(matches!(
            row["status"].as_str(),
            Some("assessed" | "not_applicable")
        ));
    }
    assert_eq!(
        wire["request"]["scope"]["kind"],
        if root.is_some() {
            "selected"
        } else {
            "catalogue"
        }
    );
    if let Some(key) = root {
        let id = &wire["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["key"] == key)
            .unwrap()["id"];
        assert_eq!(wire["request"]["scope"]["roots"], serde_json::json!([id]));
    }
}

fn run(binary: &str, args: &[&str]) -> Output {
    Command::new(binary)
        .args(args)
        .output()
        .expect("esdieff CLI")
}
fn stdin_check(binary: &str, strict: bool) -> Output {
    let mut command = Command::new(binary);
    command.args(["check", "--format", "json"]);
    if strict {
        command.arg("--strict-warnings");
    }
    let mut child = command
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("esdieff CLI");
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn full_completed_model_has_matching_cli_presentations_and_disposition() {
    let binary = env!("CARGO_BIN_EXE_esdieff");
    for (input, exit, errors) in [
        (br#"{"info":{},"sdfData":{"D":{}}}"#.as_slice(), 0, false),
        (
            br#"{"info":{},"sdfData":{"D":{"unit":"urn:ietf:params:unit:Cel"}}}"#.as_slice(),
            1,
            true,
        ),
    ] {
        for format in ["json", "human"] {
            let mut child = Command::new(binary)
                .args([
                    "check",
                    "--assessment",
                    "full-baseline",
                    "--format",
                    format,
                    "-",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(input).unwrap();
            let output = child.wait_with_output().unwrap();
            assert_eq!(output.status.code(), Some(exit));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["complete"], true);
                assert_eq!(wire["outcome"]["errors"].as_u64().unwrap() > 0, errors);
                assert_eq!(
                    wire["outcome"]["disposition"],
                    if errors { "failed" } else { "passed" }
                );
                assert_eq!(wire["coverage"]["broader"]["state"], "complete");
                assert!(wire["coverage"]["broader"].get("omitted_areas").is_none());
            } else {
                let human = String::from_utf8(output.stdout).unwrap();
                assert!(human.contains("Complete: true"));
                assert!(human.contains("Broader RFC coverage: complete"));
            }
        }
    }
}
#[test]
fn cli() {
    let binary = env!("CARGO_BIN_EXE_esdieff");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/coverage");
    let paths: Vec<_> = (1..=3)
        .map(|n| fixture.join(format!("native-{n}.json")))
        .collect();
    let names: Vec<_> = paths.iter().map(|p| p.to_str().unwrap()).collect();
    let json = run(
        binary,
        &["check", "--format", "json", names[0], names[1], names[2]],
    );
    assert_eq!(json.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    initial_wire(&wire, None);
    let human = run(binary, &["check", names[0], names[1], names[2]]);
    assert_eq!(human.status.code(), json.status.code());
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("Complete: true")
    );
    let device = run(
        binary,
        &[
            "check", "--root", names[0], "--format", "json", names[2], names[1], names[0],
        ],
    );
    assert_eq!(device.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&device.stdout).unwrap();
    initial_wire(&wire, Some("input3"));
    assert_eq!(
        wire["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["key"] == "input3")
            .unwrap()["label"],
        names[0]
    );
    let device_human = run(
        binary,
        &["check", "--root", names[0], names[2], names[1], names[0]],
    );
    assert_eq!(device_human.status.code(), device.status.code());
    let device_human = String::from_utf8(device_human.stdout).unwrap();
    assert!(device_human.contains("Complete: true"));
    assert!(device_human.contains("Broader RFC coverage: partial"));
    let selected = run(
        binary,
        &[
            "check", "--root", names[1], "--format", "json", names[2], names[0], names[1],
        ],
    );
    assert_eq!(selected.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&selected.stdout).unwrap();
    initial_wire(&wire, Some("input3"));
    assert_eq!(
        wire["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["key"] == "input3")
            .unwrap()["label"],
        names[1]
    );
    let full = run(
        binary,
        &[
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            names[0],
            names[1],
            names[2],
        ],
    );
    assert_eq!(full.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&full.stdout).unwrap();
    assert_eq!(wire["request"]["assessment"], "rfc9880/base-1");
    assert_eq!(wire["baseline"]["id"], "rfc9880/base-1");
    assert_eq!(wire["outcome"]["complete"], true);
    let ambient = run(
        binary,
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            names[0],
            "--pointer",
            "/sdfThing/Device/sdfObject/ambient",
            "--format",
            "json",
            names[0],
        ],
    );
    assert_eq!(ambient.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&ambient.stdout).unwrap();
    let ambient = &wire["result"]["json"];
    assert!(ambient.get("description").is_none());
    assert_eq!(ambient["sdfProperty"]["temperature"]["minimum"], -50);
    assert_eq!(ambient["sdfProperty"]["temperature"]["maximum"], 100);
    assert_eq!(ambient["sdfProperty"]["temperature"]["writable"], false);
    assert_eq!(
        ambient["sdfEvent"]["changed"]["sdfOutputData"]["maximum"],
        150
    );
    let controller = run(
        binary,
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            names[1],
            "--pointer",
            "/sdfObject/Controller/sdfProperty/level",
            "--format",
            "json",
            names[1],
            names[2],
        ],
    );
    assert_eq!(controller.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&controller.stdout).unwrap();
    assert_eq!(wire["result"]["json"]["minimum"], 0);
    assert_eq!(wire["result"]["json"]["maximum"], 80);
    assert!(wire["result"]["json"].get("description").is_none());
    let original = run(
        binary,
        &[
            "inspect",
            "--view",
            "source",
            "--document",
            names[2],
            "--pointer",
            "/sdfData/Level",
            "--format",
            "json",
            names[2],
        ],
    );
    assert_eq!(original.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&original.stdout).unwrap();
    assert_eq!(wire["result"]["json"]["maximum"], 100);
    assert_eq!(wire["result"]["json"]["description"], "Reusable level");
    for strict in [false, true] {
        let output = stdin_check(binary, strict);
        assert_eq!(output.status.code(), Some(if strict { 1 } else { 0 }));
        let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(wire["outcome"]["complete"], true);
        let warning = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["code"] == "metadata.info_absent")
            .unwrap();
        assert_eq!(warning["severity"], "warning");
        assert_eq!(warning["basis"]["kind"], "advice");
    }
}
