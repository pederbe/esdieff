use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn compound_model_meaning_has_consistent_json_and_human_completion() {
    let source = br#"{"info":{},"sdfData":{"D":{
        "type":"array","uniqueItems":true,"items":{"type":"object",
        "required":["free"],"properties":{"optional":{"type":"boolean"}}}
    }}}"#;
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
        child.stdin.take().unwrap().write_all(source).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["outcome"]["complete"], true);
            assert_eq!(wire["coverage"]["broader"]["state"], "complete");
            let rows = wire["coverage"]["requirement_rows"].as_array().unwrap();
            for member in [42, 50, 52, 53, 54] {
                let id = format!("rfc9880.model.M{member:03}/1");
                assert_eq!(
                    rows.iter().find(|row| row["id"] == id).unwrap()["status"],
                    "assessed",
                    "{id}"
                );
            }
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("Complete: true"));
            assert!(text.contains("Broader RFC coverage: complete"));
        }
    }
}
