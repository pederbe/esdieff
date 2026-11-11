//! Shared unit outcomes, policy and source/effective evidence through the CLI.
//! Follows ADR 0027: Unit names and recorded registry guidance.
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
fn human_json_agree_on_independent_unit_error_counts_and_reasons() {
    let bytes = include_bytes!("fixtures/units/forbidden-one.sdf.json");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("-", bytes)]).unwrap();

    let typed = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let json = run(
        &[
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            "-",
        ],
        bytes,
    );
    let human = run(
        &[
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "human",
            "-",
        ],
        bytes,
    );
    assert_eq!(
        (json.status.code(), human.status.code()),
        (Some(1), Some(1))
    );
    assert!(json.stderr.is_empty() && human.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    let human = String::from_utf8(human.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 1);
    assert!(human.contains("Errors: 1;") && human.contains("unit.urn_forbidden"));
    assert!(human.contains("Direct symbol if intended: \"kg\""));
    assert_eq!(wire["coverage"]["requested"]["total"], 13);
    assert_eq!(
        wire["coverage"]["requested"]["unassessed"],
        typed.requested_coverage().unassessed()
    );
    assert_eq!(
        wire["coverage"]["requested"]["undetermined"],
        typed.requested_coverage().undetermined()
    );
    assert_eq!(wire["outcome"]["complete"], typed.outcome().complete());
    assert_eq!(
        wire["result"]["unit_guidance"]["counts"]["known_primary"],
        1
    );
    assert!(human.contains("known_primary 1"));
    assert!(human.contains("iana.senml-units: available"));
    let row = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "unit.urn_forbidden")
        .unwrap();
    assert_eq!(row["primary"]["pointer"], "/sdfProperty/Mass/unit");
    assert_eq!(row["arguments"]["replacement"], "kg");
}
#[test]
fn unknown_guidance_is_scoped_advice_and_strict_warning_is_independent() {
    let bytes = br#"{"sdfData":{"A":{"unit":"unfamiliar-unit"}}}"#;
    let output = run(
        &[
            "check",
            "--assessment",
            "full-baseline",
            "--strict-warnings",
            "--format",
            "json",
            "-",
        ],
        bytes,
    );
    assert_eq!(output.status.code(), Some(1));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["result"]["unit_guidance"]["counts"]["unknown"], 1);
    let advice = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "unit.registration_unknown")
        .unwrap();
    assert_eq!(advice["severity"], "notice");
    assert_eq!(advice["basis"]["kind"], "advice");
}

#[test]
fn component_grammar_error_and_delimiter_advice_agree_in_both_formats() {
    let bytes = br#"{"info":{},"sdfData":{"A":{"unit":"urn:ietf:params:unit:kg?="},"B":{"unit":"urn:ietf:params:unit:kg?ordinary"}}}"#;

    let json = run(
        &[
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            "-",
        ],
        bytes,
    );
    let human = run(
        &[
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "human",
            "-",
        ],
        bytes,
    );
    assert_eq!(
        (json.status.code(), human.status.code()),
        (Some(1), Some(1))
    );
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    let text = String::from_utf8(human.stdout).unwrap();
    assert_eq!(
        (
            wire["outcome"]["errors"].as_u64(),
            wire["outcome"]["notices"].as_u64()
        ),
        (Some(1), Some(1))
    );
    assert!(
        text.contains("Errors: 1;")
            && text.contains("unit.urn_components_syntax")
            && text.contains("unit.urn_delimiter_advice")
    );
    for (code, severity, kind, pointer) in [
        (
            "unit.urn_components_syntax",
            "error",
            "rfc",
            "/sdfData/A/unit",
        ),
        (
            "unit.urn_delimiter_advice",
            "notice",
            "advice",
            "/sdfData/B/unit",
        ),
    ] {
        let row = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == code)
            .unwrap();
        assert_eq!(row["severity"], severity);
        assert_eq!(row["basis"]["kind"], kind);
        assert_eq!(row["basis"]["citation"], "RFC 8141 Section 2");
        assert_eq!(row["primary"]["pointer"], pointer);
        assert_eq!(row["related"][0]["pointer"], pointer);
        assert!(row["arguments"].get("replacement").is_none());
    }
    assert_eq!(wire["result"]["unit_guidance"]["counts"]["uri_names"], 2);
    assert_eq!(wire["result"]["unit_guidance"]["counts"]["unavailable"], 0);
    assert!(
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
