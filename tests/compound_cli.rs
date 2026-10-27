//! Compound role checks agree across human and JSON command output.
//! Follows ADR 0011: Machine-readable report contracts.
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
fn recursive_compound_check_has_matching_errors_paths_and_fixed_coverage() {
    let text = br#"{"info":{},"sdfProperty":{"rows":{"type":"array","items":{"type":"object","required":[],"minLength":-1,"format":false,"properties":{"value":{"multipleOf":0}}}},"bad":{"type":"array","items":[1]},"wide":{"type":"array","items":{"type":"array","multipleOf":0,"items":{"multipleOf":0},"default":1,"nullable":"unknown"}}}}"#;
    let json = run(&["check", "--format", "json", "-"], text);
    assert_eq!(json.status.code(), Some(1));
    assert!(json.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 5);
    assert_eq!(wire["outcome"]["complete"], false);
    assert_eq!(
        wire["coverage"]["requested"]["inventory"],
        "esdieff.initial/1"
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert_eq!(wire["coverage"]["requested"]["unassessed"], 0);
    let human = run(&["check", "--format", "human", "-"], text);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 5;") && human.contains("Complete: false"));
    for (code, pointer) in [
        ("data.count", "/sdfProperty/rows/items/minLength"),
        ("data.required", "/sdfProperty/rows/items/required"),
        ("data.string", "/sdfProperty/rows/items/format"),
        (
            "parameter.positive_step",
            "/sdfProperty/rows/items/properties/value/multipleOf",
        ),
        ("data.map_shape", "/sdfProperty/bad/items"),
    ] {
        assert_eq!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["code"] == code
                    && row["primary"]["kind"] == "source"
                    && row["primary"]["pointer"] == pointer)
                .count(),
            1
        );
        assert!(human.contains(code) && human.contains(pointer));
    }
    assert!(human.contains("esdieff.initial/1"));
}

#[test]
fn source_and_effective_inspection_retain_wider_direct_array_content() {
    let text = br#"{"info":{},"sdfProperty":{"rows":{"type":"array","items":{"type":"array","items":{"multipleOf":0},"default":1,"nullable":"unknown"}}}}"#;
    for view in ["source", "effective"] {
        let json = run(
            &[
                "inspect",
                "--view",
                view,
                "--document",
                "-",
                "--pointer",
                "/sdfProperty/rows/items",
                "--format",
                "json",
                "-",
            ],
            text,
        );
        assert_eq!(json.status.code(), Some(0));
        assert!(json.stderr.is_empty());
        let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(wire["outcome"]["complete"], true);
        assert_eq!(wire["result"]["json"]["type"], "array");
        assert_eq!(wire["result"]["json"]["items"]["multipleOf"], 0);
        assert_eq!(wire["result"]["json"]["default"], 1);
        assert_eq!(wire["result"]["json"]["nullable"], "unknown");
        let human = run(
            &[
                "inspect",
                "--view",
                view,
                "--document",
                "-",
                "--pointer",
                "/sdfProperty/rows/items",
                "--format",
                "human",
                "-",
            ],
            text,
        );
        assert_eq!(human.status.code(), Some(0));
        assert!(human.stderr.is_empty());
        let human = String::from_utf8(human.stdout).unwrap();
        assert!(
            human.contains("Complete: true")
                && human.contains("\"array\"")
                && human.contains("\"nullable\":\"unknown\"")
        );
    }
}

#[test]
fn composed_object_items_report_consumer_paths_and_source_origins_in_both_formats() {
    let text = br##"{"info":{},"sdfData":{"Base":{"type":"array","items":{"type":"object","required":[],"properties":{"value":{"multipleOf":0}}}}},"sdfProperty":{"Inherited":{"sdfRef":"#/sdfData/Base"},"Deleted":{"sdfRef":"#/sdfData/Base","items":{"required":null,"properties":{"value":{"multipleOf":null}}}}}}"##;
    let json = run(&["check", "--format", "json", "-"], text);
    assert_eq!(json.status.code(), Some(1));
    assert!(json.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 4);
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    let human = run(&["check", "--format", "human", "-"], text);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 4;") && human.contains("Complete: true"));
    for (code, suffix) in [
        ("data.required", "items/required"),
        (
            "parameter.positive_step",
            "items/properties/value/multipleOf",
        ),
    ] {
        let effective = format!("/sdfProperty/Inherited/{suffix}");
        let source = format!("/sdfData/Base/{suffix}");
        let condition = rows
            .iter()
            .find(|row| {
                row["code"] == code
                    && row["primary"]["kind"] == "effective"
                    && row["primary"]["pointer"] == effective
            })
            .unwrap();
        assert_eq!(condition["basis"]["kind"], "rfc");
        assert_eq!(condition["related"][0]["kind"], "source");
        assert_eq!(condition["related"][0]["pointer"], source);
        assert!(human.contains(&effective) && human.contains(&source));
    }
    assert!(!rows.iter().any(|row| {
        row["code"] == "data.required"
            && row["primary"]["pointer"]
                .as_str()
                .unwrap_or("")
                .starts_with("/sdfProperty/Deleted")
    }));
}
