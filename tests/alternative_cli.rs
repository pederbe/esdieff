//! Inspect effective alternatives without rewriting authored enum or choice JSON.
//! Follows ADR 0005: Nullability and presence.
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn inspect(format: &str, text: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "inspect",
            "--view",
            "effective",
            "--document",
            "-",
            "--pointer",
            "/sdfData/mode",
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
fn enum_inspection_exposes_const_alternatives_beside_authored_json() {
    let text =
        br#"{"info":{},"sdfData":{"mode":{"type":"string","enum":["on","off"],"default":"off"}}}"#;
    let output = inspect("json", text);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        wire["result"]["json"]["enum"],
        serde_json::json!(["on", "off"])
    );
    assert!(
        wire["result"].get("alternatives").is_some(),
        "effective const alternatives must be inspectable separately"
    );
}

#[test]
fn enum_const_alternatives_and_nullable_default_are_shared_by_human_and_json() {
    let text =
        br#"{"info":{},"sdfData":{"mode":{"type":"string","enum":["on","off"],"default":"off"}}}"#;
    let output = inspect("json", text);
    assert_eq!(output.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    let owner = &wire["result"]["alternatives"][0];
    assert_eq!(owner["owner"]["pointer"], "/sdfData/mode");
    assert_eq!(owner["state"], "present");
    assert_eq!(owner["branches"][0]["json"]["const"], "on");
    assert_eq!(owner["branches"][1]["json"]["const"], "off");
    assert_eq!(owner["branches"][0]["json"]["default"], "off");
    assert!(owner["branches"][0]["json"].get("nullable").is_none());
    assert!(
        owner["branches"][0]["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "default"
                && row["default_value"] == true
                && row["effective"]["pointer"] == "/sdfData/mode/enum/0/nullable")
    );
    let human = inspect("human", text);
    assert_eq!(human.status.code(), Some(0));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Alternative enum 0"));
    assert!(human.contains("\"const\":\"on\""));
    assert!(human.contains("default true") && human.contains("/sdfData/mode/enum/0/nullable"));
}

#[test]
fn partial_donor_meanings_are_explicit_in_both_formats_with_known_bounds_retained() {
    let text = br#"{"info":{},"sdfData":{"mode":{"items":{"type":"array","nullable":false,"minimum":1,"maximum":2,"sdfChoice":{"partial":{},"known":{"type":"string","nullable":true}}}}}}"#;
    let output = inspect("json", text);
    assert_eq!(output.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    let owner = wire["result"]["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["owner"]["pointer"] == "/sdfData/mode/items")
        .unwrap();
    let partial = owner["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"]["name"] == "partial")
        .unwrap();
    assert_eq!(partial["state"], "partial");
    assert_eq!(partial["complete"], false);
    assert_eq!(partial["json"]["minimum"], 1);
    assert_eq!(partial["json"]["maximum"], 2);
    assert!(
        partial["unavailable_qualities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["name"] == "nullable")
    );
    let known = owner["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"]["name"] == "known")
        .unwrap();
    assert_eq!(known["state"], "available");
    let human = inspect("human", text);
    assert_eq!(human.status.code(), Some(3));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Alternative choice \"partial\": partial"));
    assert!(human.contains("Unavailable quality nullable"));
    assert!(human.contains("\"minimum\":1") && human.contains("\"maximum\":2"));
}
