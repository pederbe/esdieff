//! Const and uniqueness follow ADR0015: Bounded value equality in a resolved data context,
//! and ADR0016: Selected samples with explicit admission and coverage.
use serde_json::Value;
use std::{fs, process::Command};

struct Inputs(std::path::PathBuf);
impl Drop for Inputs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_compound_and_unique_reports_agree_with_library_outcomes_and_locations() {
    let directory =
        Inputs(std::env::temp_dir().join(format!("esdieff-equality-cli-{}", std::process::id())));
    fs::create_dir(&directory.0).unwrap();
    let model = directory.0.join("model.json");
    let sample = directory.0.join("sample.json");
    for (qualities, bytes, admission, complete, exit, code, primary) in [
        (
            r#"{"const":{"n":10,"a":[true,null]}}"#,
            r#"{"a":[true,null],"n":1e1}"#,
            "admitted",
            true,
            0,
            "",
            "",
        ),
        (
            r#"{"type":"array","const":[1,2]}"#,
            "[2,1]",
            "rejected",
            true,
            1,
            "sample.const",
            "/0",
        ),
        (
            r#"{"type":"array","uniqueItems":true}"#,
            "[10,1e1]",
            "rejected",
            true,
            1,
            "sample.unique",
            "/1",
        ),
        (
            r#"{"type":"array","uniqueItems":true}"#,
            "[[1,2],[2,1]]",
            "admitted",
            true,
            0,
            "",
            "",
        ),
        (
            r#"{"uniqueItems":true,"maxItems":1}"#,
            "[1,2]",
            "undetermined",
            false,
            3,
            "",
            "",
        ),
        (
            r#"{"uniqueItems":true,"maxItems":1}"#,
            "[1,1]",
            "rejected",
            false,
            1,
            "sample.unique",
            "/1",
        ),
        (
            r#"{"type":"object","properties":{"stamp":{"type":"string","format":"future-format"}},"const":{"stamp":"not-a-date"}}"#,
            r#"{"stamp":"not-a-date"}"#,
            "undetermined",
            false,
            3,
            "",
            "",
        ),
        (
            r#"{"type":"object","properties":{"stamp":{"type":"string","format":"date-time"}},"const":{"stamp":"not-a-date"}}"#,
            r#"{"stamp":"not-a-date"}"#,
            "rejected",
            false,
            1,
            "sample.temporal_representation",
            "/stamp",
        ),
        (
            r#"{"type":"object","sdfChoice":{"one":{"properties":{"x":{"type":"array","const":[1]}}},"two":{"properties":{"x":{"type":"array","const":[1.0]}}}}}"#,
            r#"{"x":[1e0]}"#,
            "admitted",
            true,
            0,
            "",
            "",
        ),
        (
            r#"{"type":"object","sdfChoice":{"plain":{"properties":{"x":{"type":"object","const":{"stamp":"not-a-date"}}}},"dated":{"properties":{"x":{"type":"object","const":{"stamp":"not-a-date"},"properties":{"stamp":{"type":"string","format":"date-time"}}}}}}}"#,
            r#"{"x":{"stamp":"not-a-date"}}"#,
            "undetermined",
            false,
            3,
            "",
            "",
        ),
        (
            r#"{"type":"object","sdfChoice":{"plain":{"properties":{"x":{"type":"array","uniqueItems":true}}},"ordinary":{"properties":{"x":{"type":"array","uniqueItems":true,"items":{"type":"number"}}}}}}"#,
            r#"{"x":[10,1e1]}"#,
            "rejected",
            true,
            1,
            "sample.unique",
            "/x/1",
        ),
    ] {
        fs::write(&model, format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#)).unwrap();
        fs::write(&sample, bytes).unwrap();
        let mut outputs = Vec::new();
        for format in ["json", "human"] {
            let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
                .arg("sample")
                .arg("--document")
                .arg(&model)
                .args(["--kind", "property", "--pointer", "/sdfProperty/p"])
                .arg("--sample")
                .arg(&sample)
                .args(["--format", format])
                .arg(&model)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(exit), "{qualities} {bytes}");
            assert!(output.stderr.is_empty());
            outputs.push(String::from_utf8(output.stdout).unwrap());
        }
        let wire: Value = serde_json::from_str(&outputs[0]).unwrap();
        assert_eq!(wire["result"]["admission"], admission);
        assert_eq!(wire["outcome"]["complete"], complete);
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        let requested = &wire["coverage"]["requested"];
        assert_eq!(
            ["assessed", "not_applicable", "unassessed", "undetermined"]
                .iter()
                .map(|key| requested[key].as_u64().unwrap())
                .sum::<u64>(),
            9
        );
        assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 9);
        assert!(outputs[1].contains(&format!(": {admission}; context")));
        assert!(outputs[1].contains(&format!("Complete: {complete}")));
        println!(
            "PUBLIC_EQUALITY_CLI qualities={qualities} sample={bytes} exit={exit}\nJSON {}\nHUMAN {}",
            outputs[0], outputs[1]
        );
        if !code.is_empty() {
            let finding = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["code"] == code)
                .unwrap();
            assert_eq!(finding["primary"]["pointer"], primary);
            assert!(finding["primary"]["span"].is_array());
            if code == "sample.unique" {
                assert_eq!(finding["related"][0]["kind"], "effective");
                assert_eq!(finding["related"][1]["kind"], "source");
                assert_eq!(
                    finding["related"][2]["pointer"],
                    format!("{}/0", primary.strip_suffix("/1").unwrap())
                );
                let witness = wire["result"]["witnesses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|w| w["kind"] == "rejection" && w["constraint"] == "uniqueItems")
                    .unwrap();
                assert_eq!(witness["effective"], finding["related"][0]);
                assert_eq!(witness["sample"], finding["primary"]);
            }
        }
    }
}
