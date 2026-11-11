//! CLI producers share declaration and recorded-evidence semantics.
//! Follows ADR 0029: Content-format declarations and recorded registry guidance.
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn run(bytes: &[u8], assessment: &str, format: &str) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
    command.args(["check", "--assessment", assessment, "--format", format]);
    command
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn full_option_checks_content_syntax_while_initial_keeps_its_scope() {
    let bytes = br#"{"info":{"title":"Authored format"},"sdfData":{"Payload":{"contentFormat":"text/plain;"}}}"#;
    let json = run(bytes, "full-baseline", "json");
    assert_eq!(json.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(report["schema"], "esdieff.report");
    assert_eq!(report["operation"], "check");
    assert_eq!(report["request"]["assessment"], "rfc9880/base-1");
    assert_eq!(report["outcome"]["errors"], 1);
    assert_eq!(report["outcome"]["disposition"], "failed");
    assert_eq!(report["coverage"]["requested"]["total"], 13);
    let human = run(bytes, "full-baseline", "human");
    assert_eq!(human.status.code(), json.status.code());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 1"));
    assert!(human.contains("content_format.syntax"));
    assert!(human.contains("content-format-syntax/1"));
    let initial = run(bytes, "initial-v1", "json");
    let initial: Value = serde_json::from_slice(&initial.stdout).unwrap();
    assert_eq!(initial["outcome"]["errors"], 0);
    assert_eq!(initial["coverage"]["requested"]["total"], 8);
    assert!(
        !initial["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "content_format.syntax")
    );
}

#[test]
fn registry_miss_is_incomplete_with_no_syntax_error_in_both_presentations() {
    let bytes = br#"{"info":{"title":"Authored format"},"sdfData":{"Payload":{"contentFormat":"application/unknown-model-format"}}}"#;
    let json = run(bytes, "full-baseline", "json");
    assert_eq!(json.status.code(), Some(3));
    let report: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(report["outcome"]["errors"], 0);
    assert_eq!(report["outcome"]["complete"], false);
    assert_eq!(report["outcome"]["disposition"], "undetermined");
    assert!(
        report["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "content_format.registration_unknown")
    );
    let human = run(bytes, "full-baseline", "human");
    assert_eq!(human.status.code(), json.status.code());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 0"));
    assert!(human.contains("content_format.registration_unknown"));
    // No full message text or diagnostic ordering is an automation contract.
}

#[test]
fn file_scope_and_source_effective_origins_match_the_shared_library_conclusion() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = root.join("tests/fixtures/content-format/base.sdf.json");
    let consumer = root.join("tests/fixtures/content-format/inherited.sdf.json");
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            "--root",
        ])
        .arg(&consumer)
        .arg(&base)
        .arg(&consumer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["outcome"]["errors"], 1);
    let syntax = report["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "content_format.syntax")
        .unwrap();
    assert_eq!(syntax["primary"]["kind"], "effective");
    assert_eq!(
        syntax["primary"]["pointer"],
        "/sdfProperty/Payload/contentFormat"
    );
    assert!(
        syntax["related"].as_array().unwrap().iter().any(
            |loc| loc["kind"] == "source" && loc["pointer"] == "/sdfData/Payload/contentFormat"
        )
    );
}

#[test]
fn full_check_records_registry_evidence_in_both_presentations() {
    let bytes =
        br#"{"info":{"title":"Declared content"},"sdfData":{"Payload":{"contentFormat":"60"}}}"#;
    for format in ["json", "human"] {
        let output = run(bytes, "full-baseline", format);
        assert_eq!(output.status.code(), Some(0));
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["outcome"]["errors"], 0);
            assert_eq!(wire["policy"]["recorded_registry"], "use_recorded");
            assert_eq!(
                wire["baseline"]["registry_knowledge"][0]["state"],
                "available"
            );
            assert!(wire["baseline"]["registry_knowledge"][0]["snapshot"].is_string());
        } else {
            let human = String::from_utf8(output.stdout).unwrap();
            assert!(human.contains("content-format-syntax/1"));
            assert!(human.contains("coap-content-formats"));
        }
    }
}
