use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn full_content_meaning_completion_and_limits_agree_in_json_human_and_exit_codes() {
    for (format, code, complete) in [
        ("text/plain;charset=utf-8@gzip", 0, true),
        ("multipart/mixed", 3, false),
    ] {
        let source =
            serde_json::json!({"info":{},"sdfData":{"D":{"contentFormat":format}}}).to_string();
        for output_format in ["json", "human"] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
                .args([
                    "check",
                    "--assessment",
                    "full-baseline",
                    "--format",
                    output_format,
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
            assert_eq!(output.status.code(), Some(code));
            assert!(output.stderr.is_empty());
            if output_format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["complete"], complete);
                let rows = wire["coverage"]["requirement_rows"].as_array().unwrap();
                assert_eq!(
                    rows.iter()
                        .find(|row| row["id"] == "rfc9880.model.M024/1")
                        .unwrap()["status"],
                    if complete { "assessed" } else { "unassessed" }
                );
                assert!(
                    wire["baseline"]["interpretations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|id| id == "content-format-model-meaning/1")
                );
            } else {
                let text = String::from_utf8(output.stdout).unwrap();
                assert!(text.contains(if complete {
                    "Complete: true"
                } else {
                    "Complete: false"
                }));
                assert!(text.contains("Interpretation: content-format-model-meaning/1"));
                if !complete {
                    assert!(text.contains("content_format.model_meaning_unassessed"));
                }
            }
        }
    }
}
