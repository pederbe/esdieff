use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};
const ORDINARY: &[u8] = include_bytes!("fixtures/model-meaning/ordinary.sdf.json");
#[test]
fn combined_model_has_matching_cli_completion_and_exit_status() {
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
        child.stdin.take().unwrap().write_all(ORDINARY).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.stderr.is_empty());
        assert_eq!(output.status.code(), Some(0));
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["outcome"]["complete"], true);
            assert_eq!(wire["coverage"]["broader"]["state"], "complete");
            assert_eq!(wire["request"]["inventory"], "esdieff.full-model/1");
        } else {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("Complete: true")
            );
        }
    }
}
