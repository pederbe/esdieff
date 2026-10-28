//! Generated values written as JSON Lines replay through `esdieff trace` as regression
//! vectors. Follows ADR 0041: Generated values settled by Sample-value checking, and ADR 0042:
//! Recorded traces with independent entries.
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const THERMOSTAT: &str = include_str!("../examples/thermostat-check/rich.sdf.json");
const LEVEL: &str = r#"{"sdfProperty":{"level":{"type":"integer","minimum":10,"maximum":20}}}"#;

/// A temporary file that is removed again when the test ends.
struct TempFile(PathBuf);
impl TempFile {
    fn new(bytes: impl AsRef<[u8]>) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-replay-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for TempFile {
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

/// The JSON Lines Trace entries `esdieff generate` writes for every description of `model`,
/// and its exit code.
fn generate(model: &TempFile) -> (Vec<u8>, i32) {
    let output = run(&[
        "generate",
        "--document",
        model.path(),
        "--format",
        "jsonl",
        model.path(),
    ]);
    assert!(output.stderr.is_empty());
    (output.stdout, output.status.code().unwrap())
}

/// The JSON report of `esdieff trace` replaying `vectors` against `model`, and its exit code.
fn replay(model: &TempFile, vectors: &[u8]) -> (Value, i32) {
    let trace = TempFile::new(vectors);
    let output = run(&[
        "trace",
        "--document",
        model.path(),
        "--trace",
        trace.path(),
        "--format",
        "json",
        model.path(),
    ]);
    assert!(output.stderr.is_empty());
    let report = serde_json::from_slice(&output.stdout).unwrap();
    (report, output.status.code().unwrap())
}

fn summary(report: &Value) -> (u64, u64, u64) {
    let summary = &report["result"]["summary"];
    let count = |name: &str| summary[name].as_u64().unwrap();
    (count("passed"), count("failed"), count("undetermined"))
}

#[test]
fn generated_values_replay_against_the_same_model_and_every_entry_passes() {
    let model = TempFile::new(THERMOSTAT);
    let (vectors, code) = generate(&model);
    assert_eq!(code, 0);
    // Repeated runs write byte-identical vectors.
    assert_eq!(generate(&model).0, vectors);
    let lines = vectors.iter().filter(|byte| **byte == b'\n').count() as u64;
    assert!(lines > 20, "{lines} vectors");
    let (report, code) = replay(&model, &vectors);
    assert_eq!(code, 0);
    assert_eq!(summary(&report), (lines, 0, 0));
    assert_eq!(report["outcome"]["disposition"], "passed");
}

#[test]
fn undetermined_values_are_left_out_and_every_written_entry_replays() {
    let model = TempFile::new(
        r#"{"sdfProperty":{"code":{"type":"string","pattern":"[","maxLength":4},"level":{"type":"integer","maximum":3}}}"#,
    );
    let (vectors, code) = generate(&model);
    // Generation left values undetermined, and the exit code reports them.
    assert_eq!(code, 3);
    let text = String::from_utf8(vectors.clone()).unwrap();
    let mut lines = 0;
    for line in text.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        assert!(
            entry["expected"] == "admitted" || entry["expected"] == "rejected",
            "{line}"
        );
        lines += 1;
    }
    assert!(text.contains("/sdfProperty/code"));
    let (report, code) = replay(&model, &vectors);
    assert_eq!(code, 0);
    assert_eq!(summary(&report), (lines, 0, 0));
}

#[test]
fn a_value_too_long_for_a_trace_line_is_left_out_so_every_written_entry_replays() {
    use esdieff::{
        Catalogue, Disposition, GenerationRequest, GenerationState, SourceInput, TraceOutcome,
        TraceRequest,
    };
    const LINE: usize = 1024 * 1024;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br#"{"sdfProperty":{"note":{"type":"string","minLength":1100000}}}"#,
    )])
    .unwrap();
    let document = catalogue.source_id("model").unwrap();
    let generated = catalogue
        .generate(GenerationRequest::new(document))
        .unwrap();
    // The `min_length` and `too_short` strings are each over 1 MiB long.
    let note = generated.descriptions().next().unwrap();
    assert_eq!(note.state(), GenerationState::Partial);
    assert_eq!(note.values_omitted(), 2);
    assert_eq!(generated.disposition(), Disposition::Undetermined);
    let report: Value = serde_json::from_str(&generated.to_json()).unwrap();
    let omission = serde_json::json!({"field":"result.descriptions.values",
        "reason":"byte_limit","omitted_count":2});
    assert_eq!(report["result"]["omissions"], serde_json::json!([omission]));
    let vectors = generated.to_json_lines();
    assert!(vectors.lines().all(|line| line.len() <= LINE));
    let lines = vectors.lines().count() as u32;
    assert!(lines > 0);
    let result = catalogue
        .check_trace(TraceRequest::new(document, vectors.as_bytes()))
        .unwrap();
    assert_eq!(result.count(TraceOutcome::Passed), lines);
}

#[test]
fn replaying_against_a_narrowed_maximum_fails_exactly_the_changed_admissions() {
    let earlier = TempFile::new(LEVEL);
    let (vectors, code) = generate(&earlier);
    assert_eq!(code, 0);
    // Only the admitted maximum of 20 lies beyond the narrowed maximum of 15. Every other
    // value keeps its admission: the minimum 10 stays admitted, while 9, 21, null and the
    // values of other types stay rejected.
    let later = TempFile::new(LEVEL.replace("20", "15"));
    let (report, code) = replay(&later, &vectors);
    assert_eq!(code, 1);
    let lines = vectors.iter().filter(|byte| **byte == b'\n').count() as u64;
    assert_eq!(summary(&report), (lines - 1, 1, 0));
    let entries = report["result"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry["label"], "maximum");
    assert_eq!(entry["expected"], "admitted");
    assert_eq!(entry["admission"], "rejected");
    let codes: Vec<_> = entry["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|condition| condition["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["trace.expected", "sample.bound"]);
    let line = entry["line"].as_u64().unwrap() as usize;
    let vector = String::from_utf8(vectors).unwrap();
    let vector: Value = serde_json::from_str(vector.lines().nth(line - 1).unwrap()).unwrap();
    assert_eq!(vector["value"], 20);
}

const CONTEXT: &str = "/sdfObject/Device/sdfContext/level";
const CONTEXT_MODEL: &str = r#"{"sdfObject":{"Device":{"sdfContext":{"level":{"type":"integer","minimum":10,"maximum":20,"nullable":false}}}}}"#;

fn generate_context(model: &TempFile, format: &str) -> Output {
    run(&[
        "generate",
        "--document",
        model.path(),
        "--kind",
        "context",
        "--pointer",
        CONTEXT,
        "--format",
        format,
        model.path(),
    ])
}

#[test]
fn cli_context_vectors_are_deterministic_and_replay_changed_admissions() {
    let model = TempFile::new(CONTEXT_MODEL);
    let output = generate_context(&model, "jsonl");
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    let vectors = output.stdout;
    assert_eq!(generate_context(&model, "jsonl").stdout, vectors);
    let text = std::str::from_utf8(&vectors).unwrap();
    for line in text.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        assert_eq!(entry["kind"], "context");
        assert_eq!(entry["pointer"], CONTEXT);
    }
    let count = text.lines().count() as u64;
    let (report, code) = replay(&model, &vectors);
    assert_eq!(code, 0);
    assert_eq!(summary(&report), (count, 0, 0));

    let narrowed = TempFile::new(CONTEXT_MODEL.replace("20", "15"));
    let (report, code) = replay(&narrowed, &vectors);
    assert_eq!(code, 1);
    assert_eq!(summary(&report), (count - 1, 1, 0));
    let changed = &report["result"]["entries"][0];
    assert_eq!(changed["selector"]["kind"], "context");
    assert_eq!(changed["label"], "maximum");
    assert_eq!(changed["admission"], "rejected");

    let mut rows: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let minimum = rows
        .iter()
        .position(|row| row["label"] == "minimum")
        .unwrap();
    rows[minimum]["value"] = Value::from(9);
    let edited = rows
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let (report, code) = replay(&model, edited.as_bytes());
    assert_eq!(code, 1);
    assert_eq!(summary(&report), (count - 1, 1, 0));
    assert_eq!(report["result"]["entries"][0]["line"], minimum + 1);
    assert_eq!(report["result"]["entries"][0]["label"], "minimum");

    let json = generate_context(&model, "json");
    assert!(json.status.success());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["request"]["generation"]["selector"]["kind"], "context");
    let human = generate_context(&model, "human");
    assert!(human.status.success());
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains(&format!("context {CONTEXT}"))
    );
}

#[test]
fn cli_context_generation_and_replay_keep_incomplete_and_missing_results_visible() {
    let model = TempFile::new(CONTEXT_MODEL.replace(",\"nullable\":false", ""));
    let json = generate_context(&model, "json");
    assert_eq!(json.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["result"]["descriptions"][0]["state"], "partial");
    assert!(wire["result"]["summary"]["undetermined"].as_u64().unwrap() > 0);
    let output = generate_context(&model, "jsonl");
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    assert!(
        std::str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .all(|line| {
                let entry: Value = serde_json::from_str(line).unwrap();
                !entry["value"].is_null()
            })
    );
    let (report, code) = replay(&model, &output.stdout);
    assert_eq!(code, 0);
    assert_eq!(
        summary(&report),
        (
            std::str::from_utf8(&output.stdout).unwrap().lines().count() as u64,
            0,
            0
        )
    );

    let trace =
        format!(r#"{{"kind":"context","pointer":"{CONTEXT}","value":null,"expected":"rejected"}}"#);
    let (report, code) = replay(&model, trace.as_bytes());
    assert_eq!(code, 3);
    assert_eq!(summary(&report), (0, 0, 1));
    assert_eq!(report["result"]["entries"][0]["admission"], "undetermined");
    let removed = TempFile::new(r#"{"sdfObject":{"Device":{"sdfContext":{}}}}"#);
    let (report, code) = replay(&removed, trace.as_bytes());
    assert_eq!(code, 1);
    assert_eq!(
        report["result"]["entries"][0]["conditions"][0]["code"],
        "trace.unselectable"
    );
}

#[test]
fn cli_generates_and_replays_compound_context_from_supplied_references() {
    let base = TempFile::new(include_bytes!("../examples/context-reuse/base.sdf.json"));
    let device = TempFile::new(include_bytes!("../examples/context-reuse/device.sdf.json"));
    let pointer = "/sdfThing/Installation/sdfObject/sensor/sdfContext/location";
    let output = run(&[
        "generate",
        "--document",
        device.path(),
        "--kind",
        "context",
        "--pointer",
        pointer,
        "--format",
        "jsonl",
        base.path(),
        device.path(),
    ]);
    assert!(output.status.success(), "{:?}", output);
    let text = std::str::from_utf8(&output.stdout).unwrap();
    assert!(text.lines().any(|line| {
        let entry: Value = serde_json::from_str(line).unwrap();
        entry["value"] == serde_json::json!({"room":"a","floor":2})
            && entry["expected"] == "admitted"
    }));
    let trace = TempFile::new(&output.stdout);
    let replay = run(&[
        "trace",
        "--document",
        device.path(),
        "--trace",
        trace.path(),
        "--format",
        "json",
        base.path(),
        device.path(),
    ]);
    assert!(replay.status.success(), "{:?}", replay);
    let wire: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(summary(&wire), (text.lines().count() as u64, 0, 0));
}
