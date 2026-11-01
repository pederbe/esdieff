//! Current-consumer reference for the Rust pattern qualification, not a replacement engine.
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, GenerationRequest, SampleRequest, SourceInput,
    TraceOutcome, TraceRequest,
};
use serde_json::{Value, json};
use std::{fs, path::Path, time::Instant};

fn check_models(name: &str, paths: &[std::path::PathBuf], selected: usize) {
    let keys: Vec<_> = (0..paths.len()).map(|i| format!("model{i}")).collect();
    let bytes: Vec<_> = paths.iter().map(|p| fs::read(p).unwrap()).collect();
    let inputs: Vec<_> = keys
        .iter()
        .zip(&bytes)
        .map(|(key, bytes)| SourceInput::supplied(key, bytes))
        .collect();
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let document = catalogue.source_id(&keys[selected]).unwrap();
    let start = Instant::now();
    let model = catalogue
        .check(
            CheckRequest::new()
                .root(document)
                .assessment(Assessment::FullBaseline),
        )
        .unwrap();
    assert!(model.outcome().complete(), "{name}: {}", model.to_human());
    assert_eq!(model.outcome().errors(), 0, "{name}: {}", model.to_human());
    let generated = catalogue
        .generate(GenerationRequest::new(document))
        .unwrap();
    let mut samples = 0;
    for description in generated.descriptions() {
        for value in description
            .values()
            .filter(|v| v.admission() != Admission::Undetermined)
        {
            let mut inputs: Vec<_> = keys
                .iter()
                .zip(&bytes)
                .map(|(key, bytes)| SourceInput::supplied(key, bytes))
                .collect();
            inputs.push(SourceInput::supplied("value", value.to_json().as_bytes()).sample());
            let supplied = Catalogue::from_inputs(&inputs).unwrap();
            let sample = supplied
                .check_sample(
                    SampleRequest::new(
                        supplied.source_id(&keys[selected]).unwrap(),
                        description.kind(),
                        description.pointer(),
                    )
                    .sample(supplied.source_id("value").unwrap()),
                )
                .unwrap();
            assert_eq!(
                sample.sample_result().unwrap().admission(),
                value.admission(),
                "{name}: {}",
                sample.to_human()
            );
            samples += 1;
        }
    }
    let lines = generated.to_json_lines();
    let entries = lines.lines().count();
    assert!(entries > 0, "{name}: no settled vectors");
    assert_eq!(entries, samples);
    let trace = catalogue
        .check_trace(TraceRequest::new(document, lines.as_bytes()))
        .unwrap();
    assert_eq!(
        trace.count(TraceOutcome::Passed),
        entries as u32,
        "{name}: {}",
        trace.to_human()
    );
    assert_eq!(trace.count(TraceOutcome::Failed), 0);
    assert_eq!(trace.count(TraceOutcome::Undetermined), 0);
    println!(
        "{}",
        json!({"workload":"consumer-reference", "model":name,
        "model_errors":0, "sample_checks":samples, "trace_passed":entries,
        "generation_complete":generated.complete(), "elapsed_ns":start.elapsed().as_nanos()})
    );
}

fn traces(entries: u32) {
    // Exact counts come from these authored independent values, not generated vectors.
    for (kind, pattern, valid, invalid) in [
        ("credential", "^[A-Za-z0-9_-]+$", "k00001", "k 0001"),
        (
            "lookbehind",
            "(?<=key-)[0-9]{4}(?:_[A-Z]{2})?$",
            "key-1234_US",
            "key-12 4_US",
        ),
    ] {
        let bytes = json!({"sdfProperty":{"p":{"type":"string","pattern":pattern}}}).to_string();
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", bytes.as_bytes())]).unwrap();
        let mut lines = String::new();
        for i in 0..entries {
            lines.push_str(&json!({"kind":"property","pointer":"/sdfProperty/p", "value":if i%100==99 { invalid } else { valid }}).to_string());
            lines.push('\n');
        }
        let start = Instant::now();
        let result = catalogue
            .check_trace(TraceRequest::new(
                catalogue.source_id("model").unwrap(),
                lines.as_bytes(),
            ))
            .unwrap();
        assert_eq!(result.count(TraceOutcome::Passed), entries - entries / 100);
        assert_eq!(result.count(TraceOutcome::Failed), entries / 100);
        assert_eq!(result.count(TraceOutcome::Undetermined), 0);
        println!(
            "{}",
            json!({"workload":"independent-trace", "kind":kind,"entries":entries,
            "passed":result.count(TraceOutcome::Passed),"failed":result.count(TraceOutcome::Failed),
            "undetermined":0,"elapsed_ns":start.elapsed().as_nanos()})
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for family in [
        "thermostat",
        "rgb-lamp",
        "door-lock",
        "power-strip",
        "irrigation",
        "coffee-machine",
    ] {
        let dir = root.join(format!("examples/{family}-check"));
        let plan: Value =
            serde_json::from_slice(&fs::read(dir.join("rich-plan.json")).unwrap()).unwrap();
        let paths: Vec<_> = plan["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["role"] != "sample")
            .map(|s| dir.join(s["path"].as_str().unwrap()))
            .collect();
        let selected = paths
            .iter()
            .position(|p| p.file_name().unwrap() == "rich.sdf.json")
            .unwrap();
        check_models(family, &paths, selected);
    }
    let context = root.join("examples/context-reuse");
    check_models(
        "context-reuse",
        &[
            context.join("base.sdf.json"),
            context.join("device.sdf.json"),
        ],
        1,
    );
    for path in std::env::args().skip(1) {
        check_models(&path, &[std::path::PathBuf::from(&path)], 0);
    }
    traces(1000);
    traces(100_000);
}
