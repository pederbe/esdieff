//! Array command observations follow ADR0016: Selected samples with explicit admission and coverage.
use serde_json::Value;
use std::{fs, process::Command};

#[test]
fn action_array_cli_retains_shared_admission_coverage_locations_and_exit_codes() {
    let directory = std::env::temp_dir().join(format!("esdieff-array-cli-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let model = directory.join("model.json");
    let sample = directory.join("sample.json");
    for (qualities, bytes, admission, complete, exit, code, pointer) in [
        (
            r#"{"type":"array","maxItems":1,"items":{"type":"boolean"}}"#,
            "[true]",
            "admitted",
            true,
            0,
            "",
            "",
        ),
        (
            r#"{"type":"array","maxItems":1,"items":{"type":"boolean"}}"#,
            "[true,false]",
            "rejected",
            true,
            1,
            "sample.count",
            "",
        ),
        (
            r#"{"type":"array","items":{"type":"boolean"}}"#,
            "[true,0]",
            "rejected",
            true,
            1,
            "sample.type",
            "/1",
        ),
        (
            r#"{"type":"array","items":{"type":"number","default":1}}"#,
            "[1]",
            "undetermined",
            false,
            3,
            "",
            "",
        ),
        (
            r#"{"type":"array","maxItems":0,"items":{"type":"number","default":1}}"#,
            "[1]",
            "rejected",
            false,
            1,
            "sample.count",
            "",
        ),
    ] {
        fs::write(
            &model,
            format!(r#"{{"sdfAction":{{"configure":{{"sdfInputData":{qualities}}}}}}}"#),
        )
        .unwrap();
        fs::write(&sample, bytes).unwrap();
        let mut outputs = Vec::new();
        for format in ["json", "human"] {
            let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
                .arg("sample")
                .arg("--document")
                .arg(&model)
                .args([
                    "--kind",
                    "action_input",
                    "--pointer",
                    "/sdfAction/configure",
                ])
                .arg("--sample")
                .arg(&sample)
                .args(["--format", format])
                .arg(&model)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(exit));
            assert!(output.stderr.is_empty());
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.ends_with('\n'));
            outputs.push(text);
        }
        let wire: Value = serde_json::from_str(&outputs[0]).unwrap();
        assert_eq!(wire["operation"], "sample");
        assert_eq!(wire["version"], "1.1");
        assert_eq!(wire["result"]["admission"], admission);
        assert_eq!(wire["outcome"]["complete"], complete);
        assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
        assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 9);
        assert!(outputs[1].contains(&format!(": {admission}; context")));
        assert!(outputs[1].contains(&format!("Complete: {complete}")));
        if !code.is_empty() {
            let finding = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["code"] == code)
                .unwrap();
            assert_eq!(finding["primary"]["pointer"], pointer);
            assert!(finding["primary"]["span"].is_array());
        }
    }
    fs::remove_dir_all(directory).unwrap();
}
