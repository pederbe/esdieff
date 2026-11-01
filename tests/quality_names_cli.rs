use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn full_quality_errors_fail_both_cli_report_formats() {
    for format in ["human", "json"] {
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
            .write_all(br#"{"info":{},"sdfData":{"D":{"type":"number","Bad:quality":1}}}"#)
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["outcome"]["errors"], 1);
            assert!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["code"] == "quality.name_syntax" && row["severity"] == "error")
            );
        } else {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("quality.name_syntax")
            );
        }
    }
}
