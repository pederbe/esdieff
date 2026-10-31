//! `esdieff generate` writes Generated values as a human report, a JSON report or JSON Lines
//! Trace entries. Follows ADR 0041: Generated values settled by Sample-value checking.
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

const THERMOSTAT: &str = include_str!("../examples/thermostat-check/rich.sdf.json");
const LEVEL: &str = r#"{"sdfProperty":{"level":{"type":"integer","minimum":10,"maximum":20}}}"#;

/// A model file that is removed again when the test ends.
struct Model(PathBuf);
impl Model {
    fn new(text: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-generate-{}-{}.sdf.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, text).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .output()
        .unwrap()
}

fn report(output: &Output, exit: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The target, value and admission of each value of the report's first description.
fn rows(report: &Value) -> Vec<(String, Value, String)> {
    report["result"]["descriptions"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            (
                value["target"].as_str().unwrap().to_owned(),
                value["value"].clone(),
                value["admission"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn a_selected_property_lists_each_value_with_its_target_and_admission() {
    let model = Model::new(LEVEL);
    let output = run(&[
        "generate",
        "--document",
        model.path(),
        "--kind",
        "property",
        "--pointer",
        "/sdfProperty/level",
        "--format",
        "json",
        model.path(),
    ]);
    let report = report(&output, 0);
    assert_eq!(report["operation"], "generate");
    let rows = rows(&report);
    assert!(rows.contains(&("minimum".into(), 10.into(), "admitted".into())));
    assert!(rows.contains(&("below_minimum".into(), 9.into(), "rejected".into())));
    assert!(rows.contains(&("above_maximum".into(), 21.into(), "rejected".into())));
}

#[test]
fn patterned_identifiers_agree_across_reports_and_replay_as_trace_entries() {
    let model = Model::new(
        r#"{"sdfProperty":{"id":{"type":"string","nullable":false,
        "pattern":"^ID-[0-9]+$","minLength":5,"maxLength":7}}}"#,
    );
    let path = model.path();
    let json = report(
        &run(&["generate", "--document", path, "--format", "json", path]),
        0,
    );
    assert_eq!(
        json["request"]["generation"]["contract"],
        "esdieff.generation/2"
    );
    assert!(rows(&json).contains(&("type".into(), "ID-00".into(), "admitted".into())));
    let human = run(&["generate", "--document", path, path]);
    assert_eq!(human.status.code(), Some(0));
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .lines()
            .any(|line| line.contains("admitted") && line.contains("\"ID-00\""))
    );
    let vectors = run(&["generate", "--document", path, "--format", "jsonl", path]);
    assert_eq!(vectors.status.code(), Some(0));
    assert_eq!(
        vectors.stdout,
        run(&["generate", "--document", path, "--format", "jsonl", path]).stdout
    );
    let mut replay = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "trace",
            "--document",
            path,
            "--trace",
            "-",
            "--format",
            "json",
            path,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    replay
        .stdin
        .take()
        .unwrap()
        .write_all(&vectors.stdout)
        .unwrap();
    let replay = report(&replay.wait_with_output().unwrap(), 0);
    assert_eq!(replay["outcome"]["disposition"], "passed");
}

#[test]
fn the_human_report_shows_targets_values_and_admissions() {
    let model = Model::new(LEVEL);
    let output = run(&[
        "generate",
        "--document",
        model.path(),
        "--kind",
        "property",
        "--pointer",
        "/sdfProperty/level",
        model.path(),
    ]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.starts_with("Generated values esdieff.generation/2: passed\n"),
        "{text}"
    );
    assert!(
        text.contains("property /sdfProperty/level: complete\n"),
        "{text}"
    );
    let below = text
        .lines()
        .find(|line| line.contains("below_minimum"))
        .unwrap();
    assert!(
        below.contains("rejected") && below.contains(" 9 (sample.bound)"),
        "{below}"
    );
}

#[test]
fn corpus_evidence_explains_blocked_boundaries_in_human_and_json_reports() {
    let model = Model::new(
        r#"{"sdfProperty":{
            "identifier":{"type":"string","nullable":false,"pattern":"^(foo|bar)$","minLength":3,"maxLength":3},
            "level":{"type":"integer","nullable":false,"minimum":10,"maximum":20}
        }}"#,
    );
    let path = model.path();
    let json = report(
        &run(&["generate", "--document", path, "--format", "json", path]),
        0,
    );
    let descriptions = json["result"]["descriptions"].as_array().unwrap();
    let blocked = &descriptions[0];
    assert_eq!(blocked["state"], "complete");
    assert_eq!(blocked["evidence"]["admitted_boundary"], 0);
    assert_eq!(blocked["evidence"]["admitted_example"], 0);
    let boundary = blocked["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["target"] == "min_length")
        .unwrap();
    assert_eq!(boundary["evidence"], "rejected_candidate");
    assert_eq!(boundary["codes"], serde_json::json!(["sample.pattern"]));
    assert_eq!(
        descriptions[1]["evidence"],
        serde_json::json!({
            "admitted_example":0,"admitted_boundary":2,"rejected_outside":6,
            "rejected_candidate":1,"undetermined":0
        })
    );
    let human = run(&["generate", "--document", path, path]);
    assert_eq!(human.status.code(), Some(0));
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(
        text.contains(
            "No admitted example was retained. This does not prove the description admits no value."
        ),
        "{text}"
    );
    assert!(text.contains("0 other admitted examples, 2 admitted boundary candidates, 6 rejected outside candidates, 1 other rejected candidates, 0 undetermined"), "{text}");
    let boundary = text
        .lines()
        .find(|line| line.contains("min_length"))
        .unwrap();
    assert!(
        boundary.contains("(sample.pattern) [rejected_candidate]"),
        "{boundary}"
    );
    assert!(text.contains("Complete generation does not imply complete constraint coverage."));

    // Evidence is report detail. Recorded traces retain their existing schema.
    let vectors = run(&["generate", "--document", path, "--format", "jsonl", path]);
    assert_eq!(vectors.status.code(), Some(0));
    for line in String::from_utf8(vectors.stdout).unwrap().lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        assert!(entry.get("evidence").is_none());
        assert_eq!(entry.as_object().unwrap().len(), 5);
    }
}

#[test]
fn json_lines_give_one_trace_entry_per_settled_value() {
    let model = Model::new(THERMOSTAT);
    let output = run(&[
        "generate",
        "--document",
        model.path(),
        "--format",
        "jsonl",
        model.path(),
    ]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.ends_with('\n'));
    let entries: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let json = report(
        &run(&[
            "generate",
            "--document",
            model.path(),
            "--format",
            "json",
            model.path(),
        ]),
        0,
    );
    let summary = &json["result"]["summary"];
    let settled = summary["admitted"].as_u64().unwrap() + summary["rejected"].as_u64().unwrap();
    assert_eq!(entries.len() as u64, settled);
    let first = &entries[0];
    assert_eq!(first["kind"], "property");
    assert_eq!(
        first["pointer"],
        "/sdfObject/Thermostat/sdfProperty/currentTemperature"
    );
    let members: Vec<_> = first.as_object().unwrap().keys().cloned().collect();
    assert_eq!(members, ["expected", "kind", "label", "pointer", "value"]);
    assert!(
        entries
            .iter()
            .any(|entry| entry["label"] == "below_minimum" && entry["expected"] == "rejected")
    );
}

#[test]
fn json_lines_without_settled_values_write_nothing() {
    // A document with no affordances selects no description, so it has no entries.
    let model = Model::new(r#"{"sdfData":{"level":{"type":"number"}}}"#);
    let path = model.path();
    let output = run(&["generate", "--document", path, "--format", "jsonl", path]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert!(output.stderr.is_empty());
}

#[test]
fn undetermined_values_are_left_out_of_json_lines_and_exit_3() {
    let model = Model::new(r#"{"sdfProperty":{"code":{"type":"string","pattern":"["}}}"#);
    let output = run(&[
        "generate",
        "--document",
        model.path(),
        "--format",
        "jsonl",
        model.path(),
    ]);
    assert_eq!(output.status.code(), Some(3));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("undetermined"));
    let output = run(&[
        "generate",
        "--document",
        model.path(),
        "--format",
        "json",
        model.path(),
    ]);
    let report = report(&output, 3);
    assert_eq!(report["outcome"]["disposition"], "undetermined");
    assert_eq!(report["result"]["descriptions"][0]["state"], "partial");
}

#[test]
fn repeated_runs_give_byte_identical_reports() {
    let model = Model::new(THERMOSTAT);
    for format in ["json", "human", "jsonl"] {
        let args = [
            "generate",
            "--document",
            model.path(),
            "--format",
            format,
            model.path(),
        ];
        assert_eq!(run(&args).stdout, run(&args).stdout, "{format}");
    }
}

#[test]
fn a_model_on_standard_input_can_be_selected() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["generate", "--document", "-", "--format", "json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(LEVEL.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let report = report(&output, 0);
    assert_eq!(
        report["result"]["descriptions"][0]["selector"]["pointer"],
        "/sdfProperty/level"
    );
}

#[test]
fn invocation_errors_exit_2_with_empty_stdout() {
    let model = Model::new(THERMOSTAT);
    let path = model.path();
    let cases: &[&[&str]] = &[
        &[
            "generate",
            "--document",
            path,
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/missing",
            path,
        ],
        &["generate", "--document", path, "--kind", "property", path],
        &[
            "generate",
            "--document",
            path,
            "--pointer",
            "/sdfObject/Thermostat",
            path,
        ],
        &[
            "generate",
            "--document",
            path,
            "--kind",
            "grouping",
            "--pointer",
            "/x",
            path,
        ],
        &["generate", "--document", path, "--format", "sarif", path],
        &["generate", "--document", path, "--unknown", path],
        &["generate", "--document", path, "--sample", path, path],
        &["generate", path],
        &["generate", "--document", path],
        &["generate", "--document", "elsewhere.sdf.json", path],
        &["generate", "--document", "-", "-", "-"],
    ];
    for args in cases {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}

/// The JSON report for every description of `model`, which must exit 3.
fn undetermined(model: &str) -> Value {
    let model = Model::new(model);
    let path = model.path();
    report(
        &run(&["generate", "--document", path, "--format", "json", path]),
        3,
    )
}

#[test]
fn the_description_ceiling_exits_3_and_counts_the_omitted_descriptions() {
    let properties: Vec<String> = (0..1030)
        .map(|index| format!(r#""p{index}":{{"type":"boolean"}}"#))
        .collect();
    let report = undetermined(&format!(
        r#"{{"sdfProperty":{{{}}}}}"#,
        properties.join(",")
    ));
    assert_eq!(report["result"]["descriptions_omitted"], 6);
    assert_eq!(report["coverage"]["requested"]["unassessed"], 6);
    assert_eq!(
        report["result"]["omissions"][0],
        serde_json::json!({"field":"result.descriptions","reason":"count_limit","omitted_count":6})
    );
}

#[test]
fn the_value_ceiling_exits_3_and_counts_the_omitted_values() {
    let entries: Vec<String> = (0..12)
        .map(|index| format!(r#""e{index}":{{"type":"boolean"}}"#))
        .collect();
    let report = undetermined(&format!(
        r#"{{"sdfProperty":{{"flags":{{"type":"object","properties":{{{}}}}}}}}}"#,
        entries.join(",")
    ));
    let description = &report["result"]["descriptions"][0];
    assert_eq!(description["state"], "partial");
    assert_eq!(description["values"].as_array().unwrap().len(), 64);
    // Seven targets of the map itself and six of each of its twelve entries, 79 in all.
    assert_eq!(description["values_omitted"], 79 - 64);
}

#[test]
fn an_exhausted_check_exits_3_with_an_undetermined_value() {
    // Five hundred unique items need more equality pairs than one Sample-value checking
    // request receives, so the maximum-count value stays undetermined.
    let report = undetermined(
        r#"{"sdfProperty":{"readings":{"type":"array","uniqueItems":true,"maxItems":500,
            "items":{"type":"integer","minimum":0}}}}"#,
    );
    let description = &report["result"]["descriptions"][0];
    assert_eq!(description["state"], "partial");
    let values = description["values"].as_array().unwrap();
    let max_items = values
        .iter()
        .find(|value| value["target"] == "max_items")
        .unwrap();
    assert_eq!(max_items["admission"], "undetermined");
    assert_eq!(values[0]["admission"], "admitted");
}

#[test]
fn choice_admission_evidence_agrees_in_human_json_and_replayable_trace() {
    for alternatives in [
        r#""low":{"const":1},"high":{"const":9}"#,
        r#""first":{"type":"integer","minimum":0,"maximum":10},"second":{"type":"integer","minimum":0,"maximum":5}"#,
    ] {
        let model = Model::new(&format!(
            r#"{{"sdfProperty":{{"setting":{{"nullable":false,"sdfChoice":{{{alternatives}}}}}}}}}"#
        ));
        let wire = report(
            &run(&[
                "generate",
                "--document",
                model.path(),
                "--format",
                "json",
                model.path(),
            ]),
            0,
        );
        let evidence = &wire["result"]["descriptions"][0]["choice_evidence"];
        assert_eq!(evidence["state"], "complete");
        let human = run(&["generate", "--document", model.path(), model.path()]);
        assert!(human.status.success());
        let human = String::from_utf8(human.stdout).unwrap();
        assert!(human.contains("Collection: complete"));
        assert!(human.contains("Overlapping alternatives may be skipped"));
        for row in evidence["alternatives"].as_array().unwrap() {
            assert!(human.contains(&format!(
                "\"{}\": {}",
                row["alternative"].as_str().unwrap(),
                row["status"].as_str().unwrap()
            )));
        }
        let trace = run(&[
            "generate",
            "--document",
            model.path(),
            "--format",
            "jsonl",
            model.path(),
        ]);
        assert!(trace.status.success());
        for line in String::from_utf8(trace.stdout).unwrap().lines() {
            let entry: Value = serde_json::from_str(line).unwrap();
            assert!(entry.get("choice_evidence").is_none());
            assert!(entry.get("expected").is_some());
        }
    }
}

#[test]
fn nested_choice_witnesses_report_actual_sample_pointers() {
    let model = Model::new(
        r#"{"sdfProperty":{"setting":{"type":"object","nullable":false,"required":["mode","level"],"properties":{"mode":{"nullable":false,"sdfChoice":{"off":{"const":"off"},"on":{"const":"on"}}},"level":{"type":"integer","nullable":false,"minimum":5}}}}}"#,
    );
    let output = run(&[
        "generate",
        "--document",
        model.path(),
        "--format",
        "json",
        model.path(),
    ]);
    let wire = report(&output, 0);
    let rows = wire["result"]["descriptions"][0]["choice_evidence"]["alternatives"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|row| row["status"] == "witnessed" && row["witness"]["sample_pointer"] == "/mode")
    );
    let human = run(&["generate", "--document", model.path(), model.path()]);
    assert!(human.status.success());
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("sample \"/mode\"")
    );
}
