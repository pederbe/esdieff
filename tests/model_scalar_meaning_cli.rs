use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn model_scalar_completion_agrees_in_library_json_human_and_initial_scope() {
    let source = br#"{"info":{},"sdfData":{"D":{"type":"string","const":"\ud83d\ude00"}}}"#;
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
                assert_eq!(wire["outcome"]["errors"], 0);
                assert_eq!(wire["outcome"]["disposition"], "passed");
                if assessment == "full-baseline" {
                    for id in ["rfc9880.model.M042/1", "rfc9880.model.M043/1"] {
                        let row = wire["coverage"]["requirement_rows"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|row| row["id"] == id)
                            .unwrap();
                        assert_eq!(row["status"], "assessed");
                    }
                    assert_eq!(wire["coverage"]["broader"]["state"], "complete");
                } else {
                    assert_eq!(wire["request"]["inventory"], "esdieff.initial/1");
                    assert!(wire["coverage"].get("requirement_rows").is_none());
                    assert_eq!(wire["coverage"]["broader"]["state"], "partial");
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
