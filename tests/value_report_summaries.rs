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
    fn invoke(&self, args: &[&str], json: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
        command
            .current_dir(&self.0)
            .args(["--color", "never"])
            .args(args);
        if json {
            command.args(["--format", "json"]);
        }
        command.output().unwrap()
    }
    fn run(&self, args: &[&str], json: bool) -> Output {
        let output = self.invoke(args, json);
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

#[test]
fn check_and_inspect_lead_with_findings_or_requested_content() {
    let files = Files::new();
    files.write(
        "warning.json",
        r#"{"sdfProperty":{"p":{"type":"integer"}}}"#,
    );
    let output = files.run(&["check", "--strict-warnings", "warning.json"], false);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("Warnings caused this run to fail because --strict-warnings is enabled.")
    );
    assert!(text.find("warning metadata.info_absent:").unwrap() < text.find("Details:\n").unwrap());
    assert!(text.contains("Input: \"warning.json\""));
    let output = files.run(
        &[
            "inspect",
            "--document",
            "model.sdf.json",
            "--pointer",
            "/sdfProperty/p",
            "model.sdf.json",
        ],
        false,
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.find("Inspection source at").unwrap() < text.find("Requested assessment:").unwrap()
    );
    assert!(text.contains("\"maximum\":10"));
}

#[test]
fn plans_put_outcomes_before_details_and_keep_request_errors_readable() {
    let files = Files::new();
    files.write("value.json", "11");
    let plan = serde_json::json!({
        "schema":"esdieff.check-plan", "version":"1",
        "sources":[{"key":"m","path":"model.sdf.json","role":"model"},{"key":"v","path":"value.json","role":"sample"}],
        "cases":[
            {"name":"bad selection","operation":"sample","document":"m","kind":"property","pointer":"/sdfProperty/missing","sample":"v"},
            {"name":"late rejection","operation":"sample","document":"m","kind":"property","pointer":"/sdfProperty/p","sample":"v"}
        ]
    });
    files.write("plan.json", &plan.to_string());
    let output = files.run(&["check", "--plan", "plan.json"], false);
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Esdieff check run: invalid\n"));
    assert!(text.contains("Cases: 0 passed, 1 failed, 0 undetermined, 1 invalid requests"));
    assert!(
        text.find("\"late rejection\" [sample]: failed").unwrap()
            < text.find("Details:\n").unwrap()
    );
    assert!(text.contains("Plan location: /cases/0"));
    assert!(text.contains("Selection: property"));
    assert!(!text.contains("Scope/profile/selector: {"));
    assert!(!text.contains("Case request error: {"));
    let wire: Value =
        serde_json::from_slice(&files.run(&["check", "--plan", "plan.json"], true).stdout).unwrap();
    assert_eq!(wire["summary"]["failed"], 1);
    assert_eq!(wire["summary"]["request_errors"], 1);
}

#[test]
fn delivery_reports_distinguish_written_and_withheld_documents() {
    let files = Files::new();
    files.write("edit.json", r#"{"key":"test","source":"model","generated_key":"changed","edits":[{"owner":"/sdfProperty/p","quality":"maximum","operation":"replace","value":"bad"}]}"#);
    files.write("result.json", "previous document");
    let args = [
        "edit",
        "--plan",
        "edit.json",
        "--source",
        "model",
        "model.sdf.json",
        "--document-output",
        "result.json",
        "--replace-document",
        "--check",
        "full-baseline",
    ];
    let output = files.run(&args, false);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Document not written: \"result.json\"\n"));
    assert!(text.contains("Reason: The requested checks did not pass."));
    assert!(text.contains("Fidelity: established"));
    assert!(text.contains("error data.number:"));
    assert!(text.contains("edit request \"test\" at \"/edits/0/value\""));
    assert_eq!(
        fs::read_to_string(files.0.join("result.json")).unwrap(),
        "previous document"
    );
    let output = files.run(&args[..args.len() - 2], false);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Document written: \"result.json\"\n"));
    assert!(text.contains("Checking: not requested"));
    assert!(
        serde_json::from_slice::<Value>(&fs::read(files.0.join("result.json")).unwrap()).is_ok()
    );
}

#[test]
fn author_findings_keep_original_and_generated_locations() {
    let files = Files::new();
    files.write("base.json", r#"{"info":{},"namespace":{"b":"https://example.test/b"},"defaultNamespace":"b","sdfObject":{"Sensor":{"sdfProperty":{"p":{"type":"integer","maxLength":18446744073709551616}}}}}"#);
    files.write("recipe.json", r#"{"key":"r","generated_key":"product","thing_name":"Product","namespace":{"b":"https://example.test/b","p":"https://example.test/p"},"default_namespace":"p","objects":[{"name":"sensor","source":"base","pointer":"/sdfObject/Sensor","prefix":"b","required":false,"refinement":{}}]}"#);
    let args = [
        "author",
        "--plan",
        "recipe.json",
        "--source",
        "base",
        "base.json",
        "--document-output",
        "product.json",
        "--check",
        "full-baseline",
    ];
    let output = files.run(&args, false);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Document not written: \"product.json\""));
    assert!(text.contains("Input: \"base.json\" (source 1)"));
    assert!(
        text.contains("effective 2 \"/sdfThing/Product/sdfObject/sensor/sdfProperty/p/maxLength\"")
    );
    assert!(text.contains("source 1 \"/sdfObject/Sensor/sdfProperty/p/maxLength\" bytes "));
    assert!(text.find("error formal.uint:").unwrap() < text.find("Details:\n").unwrap());
    assert!(!files.0.join("product.json").exists());
}

#[test]
fn generation_and_trace_explain_retained_incomplete_reasons() {
    let files = Files::new();
    files.write("model.sdf.json", r#"{"info":{},"namespace":{"t":"https://example.test/missing"},"sdfProperty":{"p":{"sdfRef":"t:#/sdfData/T"}}}"#);
    let args = ["generate", "--document", "model.sdf.json", "model.sdf.json"];
    let output = files.run(&args, false);
    assert_eq!(output.status.code(), Some(3));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Reason: supplied global target unavailable"));
    let wire: Value = serde_json::from_slice(&files.run(&args, true).stdout).unwrap();
    assert_eq!(
        wire["result"]["descriptions"][0]["reasons"]["items"][0]["message"],
        "supplied global target unavailable"
    );
    assert_eq!(wire["result"]["descriptions"][0]["reasons"]["omitted"], 0);
    let output = files.trace(
        "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/p\",\"value\":1}\n",
        false,
        &[],
    );
    assert_eq!(output.status.code(), Some(3));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Reason: supplied global target unavailable")
    );

    files.write(
        "model.sdf.json",
        r#"{"info":{},"sdfProperty":{"p":{"type":"string","nullable":false,"pattern":"["}}}"#,
    );
    let wire: Value = serde_json::from_slice(&files.run(&args, true).stdout).unwrap();
    let description = &wire["result"]["descriptions"][0];
    assert!(
        description["reasons"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["value_index"].is_number())
    );
    for value in description["values"].as_array().unwrap() {
        if value["admission"] == "undetermined" {
            assert!(value["codes"].as_array().unwrap().is_empty());
        }
    }
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "m",
        br#"{"info":{},"sdfProperty":{"p":{"type":"string","pattern":"["}}}"#,
    )])
    .unwrap();
    let result = catalogue
        .generate(esdieff::GenerationRequest::new(
            catalogue.source_id("m").unwrap(),
        ))
        .unwrap();
    assert!(
        result
            .descriptions()
            .flat_map(|description| description.values())
            .any(|value| value.reason().is_some())
    );
}

#[test]
fn empty_selection_and_comparison_scope_are_explicit() {
    let files = Files::new();
    files.write("context.json", r#"{"info":{},"sdfObject":{"S":{"sdfContext":{"site":{"type":"string","nullable":false}}}}}"#);
    let output = files.run(
        &["generate", "--document", "context.json", "context.json"],
        false,
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("No eligible value descriptions were found."));
    assert!(text.contains("Context needs --kind context and --pointer."));
    let output = files.run(
        &["diff", "--old", "model.sdf.json", "--new", "model.sdf.json"],
        false,
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Earlier input: \"model.sdf.json\""));
    assert!(text.contains("Later input: \"model.sdf.json\""));
    assert!(text.contains("Standalone top-level affordances and data are excluded."));
}

#[test]
fn selection_errors_and_schema_refusals_identify_the_requested_input() {
    let files = Files::new();
    for command in ["inspect", "reference", "schema"] {
        let mut args = vec![
            command,
            "--document",
            "model.sdf.json",
            "--pointer",
            "/sdfProperty/missing",
            "model.sdf.json",
        ];
        if command == "schema" {
            args.extend(["--kind", "property"]);
        }
        let output = files.invoke(&args, false);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains("selection in \"model.sdf.json\" at \"/sdfProperty/missing\""));
    }
    let output = files.run(
        &[
            "schema",
            "--document",
            "missing.json",
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "missing.json",
        ],
        false,
    );
    assert_eq!(output.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["reason"], "selected input could not be read");
    assert!(wire["schema"].is_null());
}

#[test]
fn comparison_pointers_and_generated_nested_targets_escape_controls() {
    let files = Files::new();
    let name = "p\n\u{1b}[31m";
    for (file, maximum) in [("old.json", 10), ("new.json", 2)] {
        let model = serde_json::json!({"info":{},"sdfObject":{"S":{"sdfProperty":{name:{"type":"integer","nullable":false,"maximum":maximum}}}}});
        files.write(file, &model.to_string());
    }
    let output = files.run(&["diff", "--old", "old.json", "--new", "new.json"], false);
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stdout.contains(&0x1b));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains(r"\n\u001b[31m")
    );
    let model = serde_json::json!({"info":{},"sdfProperty":{"p":{"type":"object","properties":{name:{"type":"integer","minimum":1,"maximum":10}},"required":[name]}}});
    files.write("model.sdf.json", &model.to_string());
    let output = files.run(
        &["generate", "--document", "model.sdf.json", "model.sdf.json"],
        false,
    );
    assert!(!output.stdout.contains(&0x1b));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(r"\n\u001b[31m.minimum"));
    let output = files.run(
        &[
            "reference",
            "--uses",
            "--document",
            "model.sdf.json",
            "--pointer",
            "/sdfProperty/p",
            "model.sdf.json",
        ],
        false,
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("[location 1](#source-1)"));
    assert!(text.contains("<a id=\"source-1\"></a>"));
    assert!(text.contains("Selected model: model\\.sdf\\.json"));
}
