use serde_json::Value;
fn vocabulary(wire: &Value) -> Vec<&Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            matches!(
                item["code"].as_str(),
                Some("extension.optional_unsupported" | "quality.unfamiliar")
            )
        })
        .collect()
}
#[test]
fn full_cli_names_the_quality_in_both_formats() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let text = br#"{"info":{},"sdfData":{"D":{"type":"object","properties":{"entry":{"type":"number","writable":true}}}}}"#;
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
        child.stdin.take().unwrap().write_all(text).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(3));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            let items = vocabulary(&wire);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["arguments"]["quality"], "writable");
            assert_eq!(
                items[0]["primary"]["pointer"],
                "/sdfData/D/properties/entry/writable"
            );
        } else {
            let human = String::from_utf8(output.stdout).unwrap();
            assert!(human.contains("Quality: \"writable\""));
            assert!(human.contains("/sdfData/D/properties/entry/writable"));
        }
    }
}
