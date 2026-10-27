use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, Field, InspectRequest, JsonBuilder, Limits,
    Report, SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;

#[path = "cli_use.rs"]
mod cli_use;
pub use cli_use::{
    BARE, GENERIC, HTTP, HTTPS, OPAQUE, RULES, assert_wire_and_human_use, relative_rule_ids,
};

pub fn assert_sample_use(report: &Report<'_>, wire: &Value, human: &str, used: &[&str]) {
    assert_wire_and_human_use(wire, human, used);
    for (id, rfc) in RULES {
        match report.format_knowledge(id) {
            Field::Present(view) => {
                assert!(used.contains(&id));
                let _: &'static str = view.id();
                let _: &'static str = view.rfc();
                let _: &'static str = view.sections();
                let _: &'static str = view.scope();
                assert_eq!(view.id(), id);
                assert_eq!(view.rfc(), rfc);
                let record = wire["baseline"]["format_knowledge"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["id"].as_str() == Some(id))
                    .unwrap();
                for (key, text) in [
                    ("id", view.id()),
                    ("rfc", view.rfc()),
                    ("sections", view.sections()),
                    ("scope", view.scope()),
                ] {
                    assert_eq!(record.get(key).unwrap().as_str().unwrap(), text);
                }
            }
            Field::Absent => assert!(!used.contains(&id)),
            _ => panic!("established usage owner cannot erase facts: {id}"),
        }
    }
    for id in ["unrecognized-format/1", "rfc3986-uri-syntax/2", ""] {
        assert!(matches!(report.format_knowledge(id), Field::Absent));
    }
}

pub fn assert_non_sample_use(report: &Report<'_>) {
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire.get("baseline")
            .unwrap()
            .as_object()
            .unwrap()
            .get("format_knowledge")
            .is_none()
    );
    for (id, _) in RULES {
        assert!(matches!(report.format_knowledge(id), Field::Absent));
    }
    // Other operations can have their own correctly tagged interpretation or
    // finding facts. The sample-only getter/field does not suppress those.
    assert!(matches!(report.format_knowledge("unknown"), Field::Absent));
}

#[test]
fn ordinary_consultation_is_typed_and_mandatory_in_both_renderers() {
    for (format, sample, used) in [
        (
            "uri",
            br#""https://example.test/""#.as_slice(),
            &[GENERIC, HTTPS][..],
        ),
        (
            "uri",
            br#""http://user@/""#.as_slice(),
            &[GENERIC, HTTP][..],
        ),
        (
            "uri",
            br#""https://example.test/%GG""#.as_slice(),
            &[GENERIC][..],
        ),
        ("uri", br#""custom:device""#.as_slice(), &[GENERIC][..]),
        ("uri-reference", br#""""#.as_slice(), relative_rule_ids()),
        (
            "uri-reference",
            br#""https://example.test/""#.as_slice(),
            &[GENERIC, HTTPS][..],
        ),
        (
            "uuid",
            br#""F81D4FAE-7DEC-F1D0-F765-00A0C91E6BF6""#.as_slice(),
            &[BARE, OPAQUE][..],
        ),
        (
            "uuid",
            br#""{f81d4fae-7dec-11d0-a765-00a0c91e6bf6}""#.as_slice(),
            &[BARE][..],
        ),
    ] {
        let model = format!(
            r#"{{"info":{{}},"sdfData":{{"V":{{"type":"string","nullable":false,"format":"{format}"}}}}}}"#
        );
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = cat
            .check_sample(
                SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                    .sample(cat.source_id("s").unwrap()),
            )
            .unwrap();
        let wire = serde_json::from_str(&report.to_json()).unwrap();
        assert_sample_use(&report, &wire, &report.to_human(), used);
    }
}

#[test]
fn discarded_uri_branch_and_early_uuid_winner_preserve_visited_facts_only() {
    let sample = br#""00000000-0000-0000-0000-000000000000""#;
    // Authored and lexical order coincide on both supported map backends.
    for (choices, used) in [
        (
            r#""AUri":{"format":"uri"},"BUuid":{"format":"uuid"}"#,
            &[GENERIC, BARE, OPAQUE][..],
        ),
        (
            r#""AUuid":{"format":"uuid"},"BUri":{"format":"uri"}"#,
            &[BARE, OPAQUE][..],
        ),
    ] {
        let model = format!(
            r#"{{"info":{{}},"sdfData":{{"V":{{"type":"string","nullable":false,"sdfChoice":{{{choices}}}}}}}}}"#
        );
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = cat
            .check_sample(
                SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                    .sample(cat.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        let wire = serde_json::from_str(&report.to_json()).unwrap();
        assert_sample_use(&report, &wire, &report.to_human(), used);
    }
}

#[test]
fn replaced_unknown_branch_result_does_not_hide_already_consulted_generic_rule() {
    let model = br#"{"info":{},"sdfData":{"V":{"type":"string","nullable":false,"sdfChoice":{"AUri":{"format":"uri"},"BUnknown":{"format":"custom"}}}}}"#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""not-a-uri""#).sample(),
    ])
    .unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    let wire = serde_json::from_str(&report.to_json()).unwrap();
    assert_sample_use(&report, &wire, &report.to_human(), &[GENERIC]);
}

#[test]
fn zero_optional_details_preserve_independent_rejection_and_observed_rules() {
    let model = br#"{"info":{},"sdfData":{"V":{"type":"string","format":"uri","maxLength":1}}}"#;
    let inputs = [
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""custom:device""#).sample(),
    ];
    const CAP: usize = 12 + 2 * 4096 + 16384 + 163930;
    assert!(Catalogue::with_limits(&inputs, Limits::default().report_bytes(CAP - 1)).is_err());
    let cat = Catalogue::with_limits(
        &inputs,
        Limits::default()
            .report_bytes(CAP)
            .conditions(1)
            .diagnostic_bytes(1),
    )
    .unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(!report.outcome().complete() && report.outcome().errors() > 0);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(wire["coverage"]["rows"].as_array().unwrap().is_empty());
    assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    assert_sample_use(&report, &wire, &report.to_human(), &[GENERIC]);
}

#[test]
fn critical_unknown_context_initial_owner_is_unavailable() {
    let model = br#"{"info":{"features":["https://example.test/unknown-critical"]},"sdfData":{"V":{"type":"string","format":"uuid"}}}"#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""00000000-0000-0000-0000-000000000000""#).sample(),
    ])
    .unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    let wire = serde_json::from_str(&report.to_json()).unwrap();
    assert_initial_unavailable(&report, &wire);
}

pub fn assert_initial_unavailable(report: &Report<'_>, wire: &Value) {
    assert!(
        wire.get("baseline")
            .unwrap()
            .get("format_knowledge")
            .unwrap()
            .is_null()
    );
    for (id, _) in RULES {
        assert!(matches!(report.format_knowledge(id), Field::Unavailable(_)));
    }
    assert!(matches!(report.format_knowledge("unknown"), Field::Absent));
}

#[test]
fn other_operations_omit_the_sample_field_and_return_absent() {
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("m", br#"{"info":{}}"#)]).unwrap();
    for assessment in [
        Assessment::Initial,
        Assessment::ExpandedModified,
        Assessment::ExpandedUnits,
        Assessment::ExpandedContentFormat,
        Assessment::FullBaseline,
    ] {
        let report = cat
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        assert_non_sample_use(&report);
    }
    let report = cat
        .inspect(InspectRequest::new(cat.source_id("m").unwrap(), ""))
        .unwrap();
    assert_non_sample_use(&report);
}

#[test]
fn typed_and_escaped_original_uuid_record_the_same_actual_interpretations() {
    let model = br#"{"info":{},"sdfData":{"V":{"type":"string","format":"uuid"}}}"#;
    let builder = JsonBuilder::new();
    for input in [
        SourceInput::typed(
            "s",
            builder
                .text("F81D4FAE-7DEC-F1D0-F765-00A0C91E6BF6")
                .unwrap(),
        ),
        SourceInput::supplied("s", br#""\u004681D4FAE-7DEC-F1D0-F765-00A0C91E6BF6""#).sample(),
    ] {
        let cat = Catalogue::take_inputs(vec![SourceInput::supplied("m", model), input]).unwrap();
        let report = cat
            .check_sample(
                SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                    .sample(cat.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        let wire = serde_json::from_str(&report.to_json()).unwrap();
        assert_sample_use(&report, &wire, &report.to_human(), &[BARE, OPAQUE]);
    }
}
