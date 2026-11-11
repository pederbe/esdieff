//! Human summaries preserve independent value admissions, run outcomes and completeness.
use esdieff::{Catalogue, SampleKind, SampleRequest, SourceInput};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-value-summary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let files = Self(path);
        files.write("model.sdf.json", r#"{"info":{},"sdfProperty":{"p":{"type":"integer","nullable":false,"minimum":1,"maximum":10}}}"#);
        files
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn run(&self, args: &[&str], json: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
        command
            .current_dir(&self.0)
            .args(["--color", "never"])
            .args(args);
        if json {
            command.args(["--format", "json"]);
        }
        let output = command.output().unwrap();
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
    fn sample(&self, value: &str, json: bool) -> Output {
        self.write("value.json", value);
        self.run(
            &[
                "sample",
                "--document",
                "model.sdf.json",
                "--kind",
                "property",
                "--pointer",
                "/sdfProperty/p",
                "--sample",
                "value.json",
                "model.sdf.json",
            ],
            json,
        )
    }
    fn trace(&self, entries: &str, json: bool, dependencies: &[&str]) -> Output {
        self.write("readings.jsonl", entries);
        let mut args = vec![
            "trace",
            "--document",
            "model.sdf.json",
            "--trace",
            "readings.jsonl",
            "model.sdf.json",
        ];
        args.extend_from_slice(dependencies);
        self.run(&args, json)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn sample_admission_inputs_and_causes_precede_technical_details() {
    let files = Files::new();
    for (value, exit, admission, outcome) in [
        ("5", 0, "admitted", "passed"),
        ("11", 1, "rejected", "failed"),
    ] {
        let output = files.sample(value, false);
        assert_eq!(output.status.code(), Some(exit));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.starts_with(&format!("Sample 2: {admission}; context available\n")));
        let run = text.find(&format!("Run: {outcome}\n")).unwrap();
        for input in [
            "Sample input: \"value.json\" (source 2)",
            "Model: \"model.sdf.json\" (source 1)",
        ] {
            assert!(text.find(input).unwrap() < run);
        }
        assert!(text.contains("Complete: true\n"));
        if exit != 0 {
            assert!(text.find("sample.bound:").unwrap() < text.find("Details:\n").unwrap());
            assert!(
                text.find("sample.bound:").unwrap() < text.find("Requested assessment:").unwrap()
            );
        }
        let wire: Value = serde_json::from_slice(&files.sample(value, true).stdout).unwrap();
        assert_eq!(wire["result"]["admission"], admission);
        assert_eq!(wire["outcome"]["disposition"], outcome);
        assert_eq!(wire["outcome"]["complete"], true);
    }
}

#[test]
fn invalid_sample_and_incomplete_rejection_keep_their_independent_results() {
    let files = Files::new();
    let invalid = files.sample(r#"{"x":1,"x":2}"#, false);
    assert_eq!(invalid.status.code(), Some(1));
    let text = String::from_utf8(invalid.stdout).unwrap();
    assert!(text.starts_with("Sample 2: undetermined;"));
    assert!(text.contains("Run: failed\nComplete: false\n"));
    assert!(text.find("input.duplicate_name:").unwrap() < text.find("Details:\n").unwrap());
    let wire: Value =
        serde_json::from_slice(&files.sample(r#"{"x":1,"x":2}"#, true).stdout).unwrap();
    assert_eq!(wire["result"]["admission"], "undetermined");
    assert_eq!(wire["outcome"]["errors"], 1);

    // Existing scalar semantics permit definite rejection alongside unfinished pattern work.
    files.write(
        "model.sdf.json",
        r#"{"info":{},"sdfProperty":{"p":{"type":"string","pattern":false,"maxLength":1}}}"#,
    );
    let partial = files.sample(r#""on""#, false);
    assert_eq!(partial.status.code(), Some(1));
    let text = String::from_utf8(partial.stdout).unwrap();
    assert!(text.starts_with("Sample 2: rejected;"));
    assert!(text.contains("Run: failed\nComplete: false\n"));
    let wire: Value = serde_json::from_slice(&files.sample(r#""on""#, true).stdout).unwrap();
    assert_eq!(wire["result"]["admission"], "rejected");
    assert_eq!(wire["outcome"]["complete"], false);
}

#[test]
fn trace_expected_rejection_and_unchecked_values_are_distinguished() {
    let files = Files::new();
    let expected = "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/p\",\"value\":11,\"expected\":\"rejected\"}\n";
    let output = files.trace(expected, false, &[]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Recorded trace esdieff.trace/1: passed\n"));
    assert!(text.find("Trace input: \"readings.jsonl\"").unwrap() < text.find("Entries:").unwrap());
    assert!(text.find("Model: \"model.sdf.json\"").unwrap() < text.find("Entries:").unwrap());
    assert!(text.contains("Admissions: 0 admitted, 1 rejected, 0 undetermined\n"));
    assert!(text.contains("only when rejection was expected"));
    assert!(!text.contains("Values not checked:"));
    let wire: Value = serde_json::from_slice(&files.trace(expected, true, &[]).stdout).unwrap();
    assert_eq!(wire["result"]["summary"]["passed"], 1);
    assert_eq!(wire["result"]["summary"]["admissions"]["rejected"], 1);
    assert!(wire["result"]["entries"].as_array().unwrap().is_empty());

    let malformed =
        "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/p\",\"value\":5}\nnot-json\n";
    let output = files.trace(malformed, false, &[]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Entries: 1 passed, 1 failed, 0 undetermined\n"));
    assert!(
        text.contains(
            "Admissions: 1 admitted, 0 rejected, 0 undetermined\nValues not checked: 1\n"
        )
    );
    assert!(text.contains("line 2: failed\n  value not checked"));
    let wire: Value = serde_json::from_slice(&files.trace(malformed, true, &[]).stdout).unwrap();
    assert!(wire["result"]["entries"][0]["admission"].is_null());
    assert!(wire["result"]["summary"].get("not_checked").is_none());

    files.write(
        "model.sdf.json",
        r#"{"info":{},"sdfProperty":{"p":{"type":"string","pattern":"["}}}"#,
    );
    let uncertain = "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/p\",\"value\":\"a\",\"expected\":\"rejected\"}\n";
    let output = files.trace(uncertain, false, &[]);
    assert_eq!(output.status.code(), Some(3));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Admissions: 0 admitted, 0 rejected, 1 undetermined"));
    assert!(!text.contains("Values not checked:"));
    assert!(!text.contains("value not checked"));
}

#[test]
fn trace_rules_and_original_model_locations_are_visible_for_dependencies() {
    let files = Files::new();
    files.write("model.sdf.json", r#"{"info":{},"namespace":{"t":"https://example.com/types"},"sdfProperty":{"p":{"sdfRef":"t:#/sdfData/Limit"}}}"#);
    files.write("types.sdf.json", r#"{"info":{},"namespace":{"t":"https://example.com/types"},"defaultNamespace":"t","sdfData":{"Limit":{"type":"integer","maximum":10}}}"#);
    let entries = "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/p\",\"value\":11}\n";
    let output = files.trace(entries, false, &["types.sdf.json"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Model rule: \"maximum\""));
    assert!(text.contains("related effective 1 \"/sdfProperty/p/maximum\""));
    assert!(text.contains("related source 2 \"types.sdf.json\" \"/sdfData/Limit/maximum\" bytes "));
    let wire: Value =
        serde_json::from_slice(&files.trace(entries, true, &["types.sdf.json"]).stdout).unwrap();
    let condition = &wire["result"]["entries"][0]["conditions"][0];
    assert_eq!(condition["code"], "sample.bound");
    assert_eq!(condition["arguments"]["constraint"], "maximum");
    assert_eq!(condition["related"][1]["source_id"], 2);
    assert_eq!(condition["related"][1]["pointer"], "/sdfData/Limit/maximum");
}

#[test]
fn summary_labels_escape_controls_and_fall_back_to_source_keys() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"type":"integer","maximum":10}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model-key", model).omit_label(),
        SourceInput::supplied("sample-key", b"11")
            .sample()
            .label("value\n\u{1b}[31m.json"),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("model-key").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("sample-key").unwrap()),
        )
        .unwrap();
    let text = report.to_human();
    assert!(text.contains("Model: \"model-key\" (source 1)"));
    assert!(text.contains("Sample input: \"value\\n\\u001b[31m.json\" (source 2)"));
    assert!(!text.contains('\u{1b}'));
}
