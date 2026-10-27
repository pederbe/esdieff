//! Scalar admission follows ADR0015: Bounded value equality in a resolved data context,
//! and ADR0016: Selected samples with explicit admission and coverage.
use serde_json::{Value, json};
use std::{io::Write, process::Command};

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-scalar-sample-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn execute(
    model: &InputFile,
    sample: &InputFile,
    pointer: &str,
    format: &str,
    strict: bool,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
    command.args([
        "sample",
        "--document",
        model.path(),
        "--kind",
        "property",
        "--pointer",
        pointer,
        "--sample",
        sample.path(),
        "--format",
        format,
    ]);
    if strict {
        command.arg("--strict-warnings");
    }
    command.arg(model.path()).output().unwrap()
}

#[test]
fn cli_human_and_json_agree_on_exact_scalars_enums_and_whole_branch_witnesses() {
    let model = InputFile::new(include_bytes!(
        "fixtures/scalar-sample-constraints/ordinary.sdf.json"
    ));
    for (property, bytes, admission, exit) in [
        ("number", "1e1", "admitted", 0),
        ("number", "10.0", "admitted", 0),
        ("number", "11", "rejected", 1),
        ("wideInteger", "9007199254740993", "rejected", 1),
        ("text", r#""\u00e9""#, "admitted", 0),
        ("text", r#""e\u0301""#, "rejected", 1),
        ("boolean", "true", "admitted", 0),
        ("boolean", "1", "rejected", 1),
        ("null", "null", "admitted", 0),
        ("null", "0", "rejected", 1),
        ("enum", r#""\u006fn""#, "admitted", 0),
        ("enum", r#""other""#, "rejected", 1),
        ("override", "1e1", "admitted", 0),
        ("override", "30", "rejected", 1),
        ("nested", "10", "admitted", 0),
    ] {
        let sample = InputFile::new(bytes.as_bytes());
        let pointer = format!("/sdfProperty/{property}");
        let output = execute(&model, &sample, &pointer, "json", false);
        assert_eq!(output.status.code(), Some(exit), "{property} {bytes}");
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout.last(), Some(&b'\n'));
        let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(wire["schema"], "esdieff.report");
        assert_eq!(wire["version"], "1.1");
        assert_eq!(wire["operation"], "sample");
        assert_eq!(wire["result"]["admission"], admission);
        assert_eq!(wire["outcome"]["complete"], true);
        assert_eq!(
            wire["outcome"]["disposition"],
            if exit == 0 { "passed" } else { "failed" }
        );
        assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
        assert_eq!(wire["result"]["selector"]["pointer"], pointer);
        assert_eq!(wire["result"]["sample"], wire["request"]["sample"]);
        assert_eq!(
            wire["result"]["sample_location"]["span"],
            json!([0, bytes.len()])
        );
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert_eq!(wire["coverage"]["requested"]["count_unit"], "sample_area");
        assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 9);
        assert!(
            wire["result"]["witnesses"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["kind"] == if exit == 0 { "admission" } else { "rejection" })
        );
        if property == "enum" && exit == 0 {
            assert_eq!(
                wire["result"]["witnesses"][0]["alternative"]["pointer"],
                "/sdfProperty/enum/enum/0"
            );
        }
        if property == "override" && exit == 0 {
            assert_eq!(
                wire["result"]["witnesses"][0]["alternative"]["pointer"],
                "/sdfProperty/override/sdfChoice/wide"
            );
            assert!(
                wire["result"]["explanations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["sources"]
                        .as_array()
                        .is_some_and(|sources| sources.iter().any(|source| source["pointer"]
                            == "/sdfProperty/override/sdfChoice/wide/const")))
            );
        }
        let human = execute(&model, &sample, &pointer, "human", false);
        assert_eq!(human.status.code(), Some(exit));
        assert!(human.stderr.is_empty());
        let text = String::from_utf8(human.stdout).unwrap();
        assert!(text.contains(admission));
        assert!(text.contains(&pointer));
        assert!(text.contains("complete"));
    }
}

#[test]
fn cli_keeps_unknown_interpretation_separate_from_a_complete_non_equality_witness_and_policy() {
    let model = InputFile::new(include_bytes!(
        "fixtures/scalar-sample-constraints/unresolved.sdf.json"
    ));
    for (property, sample, expected) in [
        ("unknown", "10", "undetermined"),
        ("unknown", "30", "undetermined"),
        ("noEquality", "10", "admitted"),
        ("textMeaning", r#""x""#, "undetermined"),
        // Undeclared extension vocabulary keeps its warning without stopping admission,
        // following ADR 0037: Undeclared extension vocabulary.
        ("warning", "10", "admitted"),
    ] {
        let sample = InputFile::new(sample.as_bytes());
        let pointer = format!("/sdfProperty/{property}");
        for strict in [false, true] {
            let output = execute(&model, &sample, &pointer, "json", strict);
            assert!(output.stderr.is_empty());
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["result"]["admission"], expected);
            assert_eq!(wire["outcome"]["complete"], expected == "admitted");
            assert_eq!(wire["outcome"]["errors"], 0);
            assert_eq!(wire["policy"]["strict_warnings"], strict);
            let warnings = wire["outcome"]["warnings"].as_u64().unwrap();
            assert_eq!(
                output.status.code(),
                Some(if strict && warnings > 0 {
                    1
                } else if expected == "admitted" {
                    0
                } else {
                    3
                })
            );
            if expected == "undetermined" {
                assert!(wire["result"]["witnesses"].as_array().unwrap().is_empty());
            }
        }
    }
}
