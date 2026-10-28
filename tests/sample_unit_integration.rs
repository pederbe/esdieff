//! Selected values preserve independent model findings and contributor evidence.
//! Follows ADR 0016: Selected samples with explicit admission and coverage,
//! ADR 0026: Required declaration relationships and source evidence, and
//! ADR 0027: Unit names and recorded registry guidance.
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, Field, InspectRequest, JsonBuilder, Limits,
    SampleKind, SampleRequest, SourceInput, View,
};
use serde_json::Value;

const ROOT: &[u8] = br##"{"info":{},"namespace":{"d":"https://example.com/data"},"sdfObject":{"O":{"sdfRequired":["text","flag","number"],"sdfProperty":{"text":{"sdfRef":"d:#/sdfData/Text"},"flag":{"type":"boolean","nullable":false,"unit":"urn:ietf:params:unit:Cel"},"number":{"type":"number","multipleOf":0.1,"unit":"kg"}}}},"sdfData":{"numeric":{"type":"number","nullable":false,"minimum":2,"maximum":1},"array":{"type":"array","nullable":false,"minItems":2,"maxItems":1},"primitive":{"type":"boolean","const":""}}}"##;
const DONOR: &[u8] = br#"{"namespace":{"d":"https://example.com/data"},"defaultNamespace":"d","sdfData":{"Text":{"type":"string","nullable":false,"minLength":2,"maxLength":1,"unit":"kg"}}}"#;

fn sample<'a>(catalogue: &'a Catalogue, pointer: &str, key: &str) -> esdieff::Report<'a> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                pointer,
            )
            .sample(catalogue.source_id(key).unwrap()),
        )
        .unwrap()
}

#[test]
fn selected_values_preserve_independent_unit_advisory_and_required_results() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", ROOT),
        SourceInput::supplied("donor", DONOR),
        SourceInput::supplied("text", "\"éx\"".as_bytes()).sample(),
        SourceInput::supplied("flag", b"true").sample(),
        SourceInput::supplied("exact", b"0.3").sample(),
        SourceInput::supplied("inexact", b"0.30000000000000004").sample(),
        SourceInput::supplied("sample-model", DONOR).sample(),
    ])
    .unwrap();
    let model = catalogue.source_id("m").unwrap();
    let checked = catalogue
        .check(
            CheckRequest::new()
                .root(model)
                .assessment(Assessment::ExpandedUnits),
        )
        .unwrap();
    assert_eq!(
        (checked.outcome().errors(), checked.outcome().warnings()),
        (1, 4)
    );
    assert_eq!(checked.requested_coverage().total(), Some(10));
    assert!(checked.unit_guidance().require().unwrap().known_primary() >= 3);
    let before = checked.to_json();
    for code in [
        "unit.urn_forbidden",
        "proof.empty_numeric_interval",
        "proof.inverted_text_lengths",
        "proof.inverted_array_counts",
        "proof.primitive_const_type",
    ] {
        assert!(
            checked
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == code)
        );
    }
    let required = catalogue
        .inspect(
            InspectRequest::new(model, "/sdfObject/O")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert!(
        required
            .required_declarations()
            .require()
            .unwrap()
            .complete()
    );
    let required_wire: Value = serde_json::from_str(&required.to_json()).unwrap();
    assert_eq!(
        required_wire["request"]["scope"]["dependencies"],
        serde_json::json!([catalogue.source_id("donor").unwrap().get()])
    );
    for (pointer, key, expected) in [
        ("text", "text", Admission::Rejected),
        ("flag", "flag", Admission::Admitted),
        ("number", "exact", Admission::Admitted),
        ("number", "inexact", Admission::Rejected),
    ] {
        let report = sample(
            &catalogue,
            &format!("/sdfObject/O/sdfProperty/{pointer}"),
            key,
        );
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().warnings(), 0);
        assert_eq!(report.requested_coverage().total(), Some(9));
        assert!(matches!(report.unit_guidance(), Field::Absent));
        assert!(matches!(report.required_declarations(), Field::Absent));
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            wire["baseline"]["registry_knowledge"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(report.to_human().contains(expected.as_str()));
    }
    assert_eq!(checked.to_json(), before);
}

#[test]
fn future_formats_and_nullable_rules_keep_model_units_independent() {
    for (qualities, bytes, expected, complete) in [
        (
            r#""type":"string","format":"future-format","maxLength":1"#,
            &br#""on""#[..],
            Admission::Undetermined,
            false,
        ),
        (
            r#""type":"boolean","format":"vendor""#,
            &b"true"[..],
            Admission::Undetermined,
            false,
        ),
        (
            r#""type":"string","pattern":"z","maxLength":1"#,
            &br#""on""#[..],
            Admission::Rejected,
            true,
        ),
        (
            r#""type":"string","pattern":"(?<x>a)|(?<x>b)","maxLength":1"#,
            &br#""on""#[..],
            Admission::Rejected,
            false,
        ),
        (
            r#""type":"string","nullable":false,"minLength":true"#,
            &b"null"[..],
            Admission::Rejected,
            true,
        ),
        (
            r#""type":"boolean","minLength":true,"pattern":false"#,
            &b"true"[..],
            Admission::Admitted,
            true,
        ),
    ] {
        let model = format!(
            r#"{{"info":{{}},"sdfProperty":{{"p":{{{qualities},"unit":"urn:ietf:params:unit:Cel"}}}}}}"#
        );
        // This checks independent findings, not the wall-clock pattern limit.
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", bytes).sample(),
            ],
            Limits::default().pattern_deadline(std::time::Duration::from_secs(1)),
        )
        .unwrap();
        let checked = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        assert!(
            checked
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "unit.urn_forbidden")
        );
        let report = sample(&catalogue, "/sdfProperty/p", "s");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{qualities}"
        );
        assert_eq!(
            report.outcome().complete(),
            complete,
            "{qualities}: {}",
            report.to_json()
        );
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code().starts_with("unit."))
        );
        assert_eq!(
            report.sample_result().unwrap().context_findings().errors(),
            0
        );
    }
}

#[test]
fn inherited_sample_findings_keep_span_and_inline_source_omission() {
    for typed in [false, true] {
        let mut totals = Vec::new();
        for tiny_details in [false, true] {
            let mut limits = Limits::default().pointer_bytes(1).pointer_output_bytes(1);
            if tiny_details {
                limits = limits.conditions(1).diagnostic_bytes(1);
            }
            let input = if typed {
                SourceInput::typed("s", JsonBuilder::new().text("éx").unwrap())
            } else {
                SourceInput::supplied("s", "\"éx\"".as_bytes()).sample()
            };
            let catalogue = Catalogue::take_inputs_with_limits(
                vec![
                    SourceInput::supplied("m", ROOT),
                    SourceInput::supplied("donor", DONOR),
                    input,
                ],
                limits,
            )
            .unwrap();
            let report = sample(&catalogue, "/sdfObject/O/sdfProperty/text", "s");
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert_eq!(report.outcome().errors(), 1);
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            totals.push(
                wire["diagnostics"]["retained"].as_u64().unwrap()
                    + wire["diagnostics"]["omitted"].as_u64().unwrap(),
            );
            assert_eq!(
                wire["result"]["sample_location"]["span"],
                if typed {
                    Value::Null
                } else {
                    serde_json::json!([0, 5])
                }
            );
            if tiny_details {
                assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
            } else {
                let finding = wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|finding| finding["code"] == "sample.length")
                    .unwrap();
                let source = finding["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|location| location["kind"] == "source")
                    .unwrap();
                assert_eq!(
                    source["source_id"],
                    catalogue.source_id("donor").unwrap().get()
                );
                assert!(source["pointer"].is_null());
                assert_eq!(source["omission"]["reason"], "location_limit");
                assert!(source["span"].is_array());
                let witness = wire["result"]["witnesses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|witness| witness["kind"] == "rejection")
                    .unwrap();
                assert_eq!(finding["basis"], witness["basis"]);
            }
            assert_eq!(wire["coverage"]["requested"]["total"], 9);
        }
        assert_eq!(totals[0], totals[1]);
    }
}

#[test]
fn new_values_fit_the_exact_mandatory_envelope_on_current_owners() {
    const CAP: usize = 12 + 2 * 4096 + 16384 + 163930;
    let name = "\u{1}".repeat(8192 - "/sdfProperty/".len());
    let pointer = format!("/sdfProperty/{name}");
    for (qualities, bytes) in [
        (
            serde_json::json!({"type":"string","maxLength":1}),
            &br#""on""#[..],
        ),
        (
            serde_json::json!({"type":"boolean","nullable":false}),
            &b"null"[..],
        ),
    ] {
        let model = serde_json::json!({"sdfProperty":{&name:qualities}}).to_string();
        let inputs = [
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", bytes).sample(),
        ];
        assert_eq!(
            Catalogue::with_limits(&inputs, Limits::default().report_bytes(CAP - 1)).err(),
            Some("report cannot retain its mandatory envelope")
        );
        let catalogue =
            Catalogue::with_limits(&inputs, Limits::default().report_bytes(CAP)).unwrap();
        let report = sample(&catalogue, &pointer, "s");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(report.outcome().complete());
        let json = report.to_json();
        assert!(json.len() <= CAP);
        assert!(report.to_human().len() <= CAP);
        let wire: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["request"]["selector"]["pointer"], pointer);
        assert_eq!(wire["result"]["selector"]["pointer"], pointer);
        assert_eq!(wire["result"]["description"]["pointer"], pointer);
        assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
    }
}

#[test]
fn prior_unit_check_does_not_refill_selected_exact_work() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"type":"string","minLength":2,"maxLength":2,"unit":"urn:ietf:params:unit:Cel"}}}"#;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", br#""on""#).sample(),
        ],
        Limits::default().numeric_work(25),
    )
    .unwrap();
    for _ in 0..2 {
        let checked = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        assert!(
            checked
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "unit.urn_forbidden")
        );
        let report = sample(&catalogue, "/sdfProperty/p", "s");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
    }
}
