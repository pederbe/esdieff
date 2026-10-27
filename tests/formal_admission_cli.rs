use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn formal_results_agree_in_cli_json_human_and_initial_scope() {
    for (source, exit, complete, errors) in [
        (
            r#"{"info":{},"sdfData":{"D":{"type":"number","minimum":0.5,"const":10.0}}}"#,
            0,
            true,
            0,
        ),
        (
            r#"{"info":{},"sdfData":{"D":{"type":"number","minimum":0.1}}}"#,
            0,
            true,
            0,
        ),
        (
            r#"{"info":{},"sdfData":{"D":{"type":"string","maxLength":18446744073709551616}}}"#,
            1,
            true,
            1,
        ),
        (
            r#"{"info":{},"sdfData":{"D":{"const":[null]}}}"#,
            3,
            false,
            0,
        ),
        (
            r#"{"info":{},"sdfObject":{"O":{"minItems":0.5}}}"#,
            3,
            false,
            0,
        ),
    ] {
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
            assert_eq!(output.status.code(), Some(exit), "{source}");
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["complete"], complete);
                assert_eq!(wire["outcome"]["errors"], errors);
                assert_eq!(
                    wire["coverage"]["broader"]["state"],
                    if complete { "complete" } else { "partial" }
                );
                if errors > 0 {
                    assert!(
                        wire["diagnostics"]["items"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|finding| finding["code"] == "formal.uint")
                    );
                }
            } else {
                let text = String::from_utf8(output.stdout).unwrap();
                assert!(text.contains(&format!("Complete: {complete}")));
                assert!(text.contains(&format!("Errors: {errors}")));
            }
        }
    }
    let source = br#"{"info":{},"sdfData":{"D":{"minimum":0.1,"const":0.1}}}"#;
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "check",
            "--assessment",
            "initial-v1",
            "--format",
            "json",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["outcome"]["complete"], true);
    assert!(wire["coverage"].get("requirement_rows").is_none());
}
