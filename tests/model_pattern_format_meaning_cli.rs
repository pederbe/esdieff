use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn nullable_formatted_capability_completes_full_cli_without_operand_consultation() {
    for meaning in ["uuid", "date", "date-time"] {
        let source =
            serde_json::json!({"info":{},"sdfData":{"A":{"type":"array","uniqueItems":true,
        "items":{"sdfChoice":{"id":{"type":"string","format":meaning}}}}}})
            .to_string();
        for format in ["json", "human"] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
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
            child
                .stdin
                .take()
                .unwrap()
                .write_all(source.as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["complete"], true);
                let row = wire["coverage"]["requirement_rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["id"] == "rfc9880.model.M052/1")
                    .unwrap();
                assert_eq!(row["status"], "assessed");
                assert!(
                    wire["baseline"]["interpretations"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
            } else {
                assert!(
                    String::from_utf8(output.stdout)
                        .unwrap()
                        .contains("Broader RFC coverage: complete")
                );
            }
        }
    }
}

#[test]
fn known_model_meaning_agrees_across_cli_formats_and_preserves_initial_scope() {
    let source = br#"{"info":{},"sdfData":{
        "pattern":{"type":"string","pattern":"(?!)"},
        "date":{"type":"string","format":"date-time"},
        "uri":{"type":"string","format":"uri"},
        "uuid":{"type":"string","format":"uuid"},
        "bytes":{"type":"string","sdfType":"byte-string"},
        "time":{"type":"number","sdfType":"unix-time"}
    }}"#;
    for assessment in ["full-baseline", "initial-v1"] {
        for format in ["json", "human"] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
                .args(["check", "--assessment", assessment, "--format", format, "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(source).unwrap();
            let output = child.wait_with_output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["complete"], true);
                assert_eq!(wire["outcome"]["disposition"], "passed");
                if assessment == "full-baseline" {
                    for member in [26, 27, 42, 47, 48, 49] {
                        let id = format!("rfc9880.model.M{member:03}/1");
                        let row = wire["coverage"]["requirement_rows"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|row| row["id"] == id)
                            .unwrap();
                        assert_eq!(row["status"], "assessed", "{id}");
                    }
                    assert_eq!(wire["coverage"]["broader"]["state"], "complete");
                } else {
                    assert_eq!(wire["request"]["inventory"], "esdieff.initial/1");
                    assert!(wire["coverage"].get("requirement_rows").is_none());
                }
            } else {
                let text = String::from_utf8(output.stdout).unwrap();
                assert!(text.contains("Complete: true"));
                assert!(text.contains(if assessment == "full-baseline" {
                    "Broader RFC coverage: complete"
                } else {
                    "Broader RFC coverage: partial"
                }));
            }
        }
    }
}
