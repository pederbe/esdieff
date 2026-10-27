// Ordinary action maps follow ADR0016: Selected samples with explicit admission and coverage.
use serde_json::Value;
use std::{fs, process::Command};

#[test]
fn action_map_cli_agrees_with_the_public_domain_observation() {
    let directory = std::env::temp_dir().join(format!("esdieff-map-cli-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let model = directory.join("model.json");
    let sample = directory.join("sample.json");
    fs::write(&model, br#"{"sdfAction":{"configure":{"sdfInputData":{"type":"object","properties":{"mode":{"type":"string"}},"required":["mode"]}}}}"#).unwrap();
    for (bytes, admission, exit, complete) in [
        ("{}", "rejected", 1, true),
        (r#"{"mode":"on"}"#, "admitted", 0, true),
    ] {
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
        assert_eq!(wire["version"], "1.1");
        assert_eq!(wire["operation"], "sample");
        assert_eq!(wire["result"]["admission"], admission);
        assert_eq!(wire["outcome"]["complete"], complete);
        assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
        assert!(outputs[1].contains(&format!(": {admission}; context")));
        assert!(outputs[1].contains(&format!("Complete: {complete}")));
        assert_eq!(wire["request"]["assessment"], "esdieff.sample/1");
        assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 9);
    }
    fs::remove_dir_all(directory).unwrap();
}
