//! Offline Context vectors use the existing deterministic generation and trace contracts.
use esdieff::{
    Admission, Catalogue, Disposition, GenerationRequest, GenerationState, Limits, SampleKind,
    SourceInput, TraceOutcome, TraceRequest,
};
use serde_json::{Value, json};

const BASE: &[u8] = include_bytes!("../../examples/context-reuse/base.sdf.json");
const DEVICE: &[u8] = include_bytes!("../../examples/context-reuse/device.sdf.json");
const CALIBRATION: &str = "/sdfThing/Installation/sdfObject/sensor/sdfContext/calibration";
const VALUE: &str = "/sdfObject/Device/sdfContext/value";

fn inline(qualities: Value, limits: Limits) -> Catalogue {
    let model = json!({
        "namespace":{"missing":"https://example.invalid/unavailable"},
        "sdfObject":{"Device":{"sdfContext":{"value":qualities}}}
    });
    Catalogue::with_limits(
        &[SourceInput::supplied("model", model.to_string().as_bytes())],
        limits,
    )
    .unwrap()
}

#[test]
fn selected_context_generates_deterministic_final_bounds_and_outside_values() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", DEVICE),
    ])
    .unwrap();
    let document = catalogue.source_id("device").unwrap();
    let generate = || {
        catalogue
            .generate(GenerationRequest::new(document).select(SampleKind::Context, CALIBRATION))
            .unwrap()
    };
    let result = generate();
    assert!(result.complete());
    assert_eq!(result.to_json_lines(), generate().to_json_lines());
    assert_eq!(result.to_json(), generate().to_json());
    assert_eq!(result.selector(), Some((SampleKind::Context, CALIBRATION)));
    let wire: Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(
        wire["baseline"]["extensions"][0]["id"],
        "draft-ietf-asdf-sdf-nonaffordance-06"
    );
    let description = result.descriptions().next().unwrap();
    assert_eq!(description.kind(), SampleKind::Context);
    assert_eq!(description.pointer(), CALIBRATION);
    assert_eq!(description.state(), GenerationState::Complete);
    let values: Vec<_> = description
        .values()
        .map(|value| (value.target(), value.to_json(), value.admission()))
        .collect();
    for expected in [
        ("minimum", "-10", Admission::Admitted),
        ("maximum", "5", Admission::Admitted),
        ("below_minimum", "-11", Admission::Rejected),
        ("above_maximum", "6", Admission::Rejected),
        ("null", "null", Admission::Rejected),
    ] {
        assert!(values.contains(&expected), "{values:?}");
    }
    for line in result.to_json_lines().lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        assert_eq!(entry["kind"], "context");
        assert_eq!(entry["pointer"], CALIBRATION);
        assert!(entry["label"].is_string());
    }
}

#[test]
fn default_traversal_preserves_affordance_vectors_and_context_requires_its_own_kind() {
    let model = json!({"sdfThing":{"Site":{
        "sdfContext":{"value":{"type":"boolean","nullable":false}},
        "sdfObject":{"Device":{
            "sdfProperty":{"value":{"type":"integer","minimum":10}},
            "sdfContext":{"value":{"type":"string","nullable":false}}
        }}
    }}});
    let mut without = model.clone();
    without["sdfThing"]["Site"]
        .as_object_mut()
        .unwrap()
        .remove("sdfContext");
    without["sdfThing"]["Site"]["sdfObject"]["Device"]
        .as_object_mut()
        .unwrap()
        .remove("sdfContext");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model.to_string().as_bytes()),
        SourceInput::supplied("without", without.to_string().as_bytes()),
    ])
    .unwrap();
    let document = catalogue.source_id("model").unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(document))
        .unwrap();
    assert_eq!(result.descriptions().len(), 1);
    assert_eq!(
        result.descriptions().next().unwrap().kind(),
        SampleKind::Property
    );
    let before = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("without").unwrap(),
        ))
        .unwrap();
    assert_eq!(result.to_json_lines(), before.to_json_lines());
    let property = "/sdfThing/Site/sdfObject/Device/sdfProperty/value";
    let context = "/sdfThing/Site/sdfObject/Device/sdfContext/value";
    for (kind, pointer) in [
        (SampleKind::Property, context),
        (SampleKind::Context, property),
        (SampleKind::Context, "/sdfThing/Site/sdfContext/missing"),
    ] {
        assert!(
            catalogue
                .generate(GenerationRequest::new(document).select(kind, pointer))
                .is_err()
        );
    }
}

#[test]
fn replay_keeps_rejection_invalid_selection_and_unavailable_meaning_distinct() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br##"{
        "namespace":{"missing":"https://example.invalid/unavailable"},
        "sdfObject":{"Device":{
            "sdfContext":{
                "value":{"type":"integer","minimum":5,"nullable":false},
                "unknownDefault":{"type":"number"},
                "link":{"type":"string","sdfType":"link","nullable":false},
                "unavailable":{"sdfRef":"missing:#/sdfData/Value"}
            },
            "sdfProperty":{"value":{"type":"integer"}}
        }}
    }"##,
    )])
    .unwrap();
    let rows = [
        json!({"kind":"context","pointer":VALUE,"value":5}),
        json!({"kind":"context","pointer":VALUE,"value":4,"expected":"admitted"}),
        json!({"kind":"context","pointer":"/sdfObject/Device/sdfProperty/value","value":5}),
        json!({"kind":"context","pointer":"/sdfObject/Device/sdfContext/missing","value":5}),
        json!({"kind":"context","pointer":"/sdfObject/Device/sdfContext/unknownDefault","value":null,"expected":"rejected"}),
        json!({"kind":"context","pointer":"/sdfObject/Device/sdfContext/link","value":"https://example.com","expected":"admitted"}),
        json!({"kind":"context","pointer":"/sdfObject/Device/sdfContext/unavailable","value":5,"expected":"rejected"}),
    ];
    let trace = rows
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n";
    let replay = catalogue
        .check_trace(TraceRequest::new(
            catalogue.source_id("model").unwrap(),
            trace.as_bytes(),
        ))
        .unwrap();
    assert_eq!(replay.count(TraceOutcome::Passed), 1);
    assert_eq!(replay.count(TraceOutcome::Failed), 3);
    assert_eq!(replay.count(TraceOutcome::Undetermined), 3);
    let wire: Value = serde_json::from_str(&replay.to_json()).unwrap();
    let entries = wire["result"]["entries"].as_array().unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|e| e["line"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [2, 3, 4, 5, 6, 7]
    );
    let bound = entries[0]["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "sample.bound")
        .unwrap();
    assert_eq!(bound["primary"]["pointer"], "/value");
    assert_eq!(bound["primary"]["source_id"], 2);
    let span = bound["primary"]["span"].as_array().unwrap();
    let span = span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize;
    assert_eq!(&trace[span], "4");
    assert!(
        bound["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["pointer"] == format!("{VALUE}/minimum"))
    );
    for entry in &entries[1..3] {
        assert_eq!(entry["conditions"][0]["code"], "trace.unselectable");
        assert_eq!(entry["conditions"][0]["primary"]["pointer"], "/pointer");
        assert!(entry["admission"].is_null());
    }
    for entry in &entries[3..] {
        assert_eq!(entry["admission"], "undetermined");
        assert!(
            entry["conditions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["severity"] == "incomplete")
        );
    }
}

#[test]
fn unresolved_defaults_and_unsupported_meaning_leave_generation_partial() {
    for qualities in [
        json!({"type":"number","minimum":5}),
        json!({"type":"string","sdfType":"link","nullable":false}),
        json!({"type":"string","pattern":"[","nullable":false}),
    ] {
        let catalogue = inline(qualities, Limits::default());
        let document = catalogue.source_id("model").unwrap();
        let result = catalogue
            .generate(GenerationRequest::new(document).select(SampleKind::Context, VALUE))
            .unwrap();
        assert_eq!(
            result.descriptions().next().unwrap().state(),
            GenerationState::Partial
        );
        assert!(result.count(Admission::Undetermined) > 0);
        assert_eq!(result.disposition(), Disposition::Undetermined);
        let vectors = result.to_json_lines();
        for row in vectors.lines() {
            let entry: Value = serde_json::from_str(row).unwrap();
            assert_ne!(entry["expected"], "undetermined");
        }
        let replay = catalogue
            .check_trace(TraceRequest::new(document, vectors.as_bytes()))
            .unwrap();
        assert_eq!(
            replay.count(TraceOutcome::Passed),
            vectors.lines().count() as u32
        );
    }
    let catalogue = inline(
        json!({"sdfRef":"missing:#/sdfData/Value"}),
        Limits::default(),
    );
    let result = catalogue
        .generate(
            GenerationRequest::new(catalogue.source_id("model").unwrap())
                .select(SampleKind::Context, VALUE),
        )
        .unwrap();
    assert_eq!(
        result.descriptions().next().unwrap().state(),
        GenerationState::Unavailable
    );
    assert!(!result.complete());
    assert_eq!(result.to_json_lines(), "");
}

#[test]
fn context_value_omissions_and_limits_remain_visible() {
    let values: Vec<_> = (0..70).map(|n| format!("value{n}")).collect();
    let catalogue = inline(json!({"enum":values,"nullable":false}), Limits::default());
    let result = catalogue
        .generate(
            GenerationRequest::new(catalogue.source_id("model").unwrap())
                .select(SampleKind::Context, VALUE),
        )
        .unwrap();
    let description = result.descriptions().next().unwrap();
    assert_eq!(description.values().len(), 64);
    assert!(description.values_omitted() >= 6);
    assert!(!result.complete());
    let wire: Value = serde_json::from_str(&result.to_json()).unwrap();
    assert!(
        wire["result"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["reason"] == "count_limit")
    );

    let catalogue = inline(
        json!({"type":"integer","nullable":false}),
        Limits::default().effective_nodes(1),
    );
    let result = catalogue
        .generate(
            GenerationRequest::new(catalogue.source_id("model").unwrap())
                .select(SampleKind::Context, VALUE),
        )
        .unwrap();
    assert!(!result.complete());
    assert_eq!(result.disposition(), Disposition::Undetermined);
}

#[test]
fn context_reports_keep_draft_metadata_and_totals_when_details_do_not_fit() {
    let model = br#"{"sdfObject":{"Device":{"sdfContext":{"value":{"type":"integer","minimum":5,"nullable":false}}}}}"#;
    let trace = format!(r#"{{"kind":"context","pointer":"{VALUE}","value":4}}"#);
    for generation in [true, false] {
        let catalogue = |limit| {
            Catalogue::with_limits(
                &[SourceInput::supplied("model", model)],
                Limits::default().report_bytes(limit),
            )
        };
        let accepted = |limit| {
            catalogue(limit).is_ok_and(|catalogue| {
                let document = catalogue.source_id("model").unwrap();
                if generation {
                    catalogue
                        .generate(
                            GenerationRequest::new(document).select(SampleKind::Context, VALUE),
                        )
                        .is_ok()
                } else {
                    catalogue
                        .check_trace(TraceRequest::new(document, trace.as_bytes()))
                        .is_ok()
                }
            })
        };
        let (mut low, mut high) = (1, 1024 * 1024);
        while low < high {
            let mid = low + (high - low) / 2;
            if accepted(mid) {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        let catalogue = catalogue(low).unwrap();
        let document = catalogue.source_id("model").unwrap();
        let (json, human) = if generation {
            let result = catalogue
                .generate(GenerationRequest::new(document).select(SampleKind::Context, VALUE))
                .unwrap();
            assert_eq!(result.descriptions_omitted(), 1);
            assert!(!result.complete());
            (result.to_json(), result.to_human())
        } else {
            let result = catalogue
                .check_trace(TraceRequest::new(document, trace.as_bytes()))
                .unwrap();
            assert_eq!(result.entries_omitted(), 1);
            assert_eq!(result.count(TraceOutcome::Failed), 1);
            assert_eq!(result.admissions(Admission::Rejected), 1);
            (result.to_json(), result.to_human())
        };
        assert!(json.len() <= low, "{} > {low}", json.len());
        assert!(human.len() <= low);
        let wire: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            wire["baseline"]["extensions"][0]["id"],
            "draft-ietf-asdf-sdf-nonaffordance-06"
        );
        assert_eq!(wire["diagnostics"]["truncated"], true);
    }
}

#[test]
fn generated_context_vectors_replay_against_the_same_model() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", DEVICE),
    ])
    .unwrap();
    let document = catalogue.source_id("device").unwrap();
    for pointer in [
        CALIBRATION,
        "/sdfThing/Installation/sdfContext/siteName",
        "/sdfThing/Installation/sdfObject/sensor/sdfContext/location",
        "/sdfThing/Installation/sdfObject/sensor/sdfContext/assetTag",
    ] {
        let generated = catalogue
            .generate(GenerationRequest::new(document).select(SampleKind::Context, pointer))
            .unwrap();
        assert!(generated.complete(), "{}", generated.to_human());
        let vectors = generated.to_json_lines();
        let count = vectors.lines().count() as u32;
        assert!(count > 0);
        let replay = catalogue
            .check_trace(TraceRequest::new(document, vectors.as_bytes()))
            .unwrap();
        assert_eq!(
            replay.count(TraceOutcome::Passed),
            count,
            "{}",
            replay.to_json()
        );
        assert_eq!(replay.count(TraceOutcome::Failed), 0);
        assert_eq!(replay.count(TraceOutcome::Undetermined), 0);
        let wire: Value = serde_json::from_str(&replay.to_json()).unwrap();
        assert_eq!(
            wire["baseline"]["extensions"][0]["id"],
            "draft-ietf-asdf-sdf-nonaffordance-06"
        );
    }
}
