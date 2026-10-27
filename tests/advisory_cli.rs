//! Initial advice retains shared CLI disposition and source inspection semantics.
//! Follows ADR 0013: Shared workflow and report guarantees.
mod coverage_support;
use std::io::Write;
use std::process::{Command, Stdio};
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
fn warning_advice_keeps_severity_counts_and_incomplete_coverage_under_strict_mode() {
    let bytes = br#"{"info":{},"sdfData":{"bad":{"type":"number","nullable":false,"minimum":2,"maximum":1}}}"#;
    let plain = run(&["check", "--format", "json", "-"], bytes);
    assert_eq!(plain.status.code(), Some(0));
    assert!(plain.stderr.is_empty());
    let strict = run(
        &["check", "--strict-warnings", "--format", "json", "-"],
        bytes,
    );
    assert_eq!(strict.status.code(), Some(1));
    let plain: serde_json::Value = serde_json::from_slice(&plain.stdout).unwrap();
    let strict: serde_json::Value = serde_json::from_slice(&strict.stdout).unwrap();
    for wire in [&plain, &strict] {
        assert_eq!(wire["outcome"]["errors"], 0);
        assert_eq!(wire["outcome"]["warnings"], 1);
        coverage_support::assert_wire_counts(wire);
        assert_eq!(wire["outcome"]["complete"], true);
        let proof = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|condition| condition["code"] == "proof.empty_numeric_interval")
            .unwrap();
        assert_eq!(proof["severity"], "warning");
        assert_eq!(proof["basis"]["kind"], "advice");
    }
    let human = run(&["check", "-"], bytes);
    assert_eq!(human.status.code(), Some(0));
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("warnings: 1"));
    assert!(text.contains("incomplete checks: 0"));
    assert!(text.contains("proof.empty_numeric_interval"));
}
