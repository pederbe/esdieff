//! Array admission follows ADR0016: Selected samples with explicit admission and coverage.
mod map_support;
use esdieff::{Admission, AssessmentStatus, Catalogue, Limits, Report, SourceInput};
use map_support::selected;
use serde_json::Value;

fn supplied(qualities: &str, sample: &str) -> Catalogue {
    let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
    Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", sample.as_bytes()).sample(),
    ])
    .unwrap()
}
fn expect(report: &Report<'_>, admission: Admission, complete: bool) -> Value {
    assert_eq!(report.sample_result().unwrap().admission(), admission);
    assert_eq!(report.outcome().complete(), complete);
    let rows = report.requested_coverage();
    assert_eq!(rows.total(), Some(9));
    assert_eq!(
        rows.assessed() + rows.not_applicable() + rows.unassessed() + rows.undetermined(),
        9
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["request"]["assessment"], "esdieff.sample/1");
    assert_eq!(wire["result"]["admission"], admission.as_str());
    assert_eq!(wire["outcome"]["complete"], complete);
    assert!(
        report
            .to_human()
            .contains(&format!(": {}; context", admission.as_str()))
    );
    wire
}

#[test]
fn array_counts_are_inclusive_exact_nonnegative_mathematical_counts() {
    for (qualities, sample, expected) in [
        (
            r#"{"type":"array","minItems":1,"maxItems":2}"#,
            "[]",
            Admission::Rejected,
        ),
        (
            r#"{"type":"array","minItems":1,"maxItems":2}"#,
            "[1]",
            Admission::Admitted,
        ),
        (
            r#"{"type":"array","minItems":1,"maxItems":2}"#,
            "[1,2]",
            Admission::Admitted,
        ),
        (
            r#"{"type":"array","minItems":1,"maxItems":2}"#,
            "[1,2,3]",
            Admission::Rejected,
        ),
        (
            r#"{"type":"array","minItems":-0,"maxItems":0e3}"#,
            "[]",
            Admission::Admitted,
        ),
        (
            r#"{"type":"array","minItems":2.0,"maxItems":2e0}"#,
            "[1,2]",
            Admission::Admitted,
        ),
        (
            r#"{"type":"array","maxItems":18446744073709551616}"#,
            "[1,2]",
            Admission::Admitted,
        ),
        (
            r#"{"type":"array","minItems":9007199254740993}"#,
            "[1,2]",
            Admission::Rejected,
        ),
    ] {
        let catalogue = supplied(qualities, sample);
        let report = selected(&catalogue);
        let wire = expect(&report, expected, true);
        assert_eq!(
            report
                .assessment_area_status("sample-array/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .sample_value()
                .require()
                .unwrap()
                .raw_json(),
            sample
        );
        if expected == Admission::Rejected {
            let finding = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["code"] == "sample.count")
                .unwrap();
            assert_eq!(finding["primary"]["pointer"], "");
            assert_eq!(finding["basis"]["citation"], "RFC 9880 Appendix C.4");
        }
    }
}

#[test]
fn primitive_elements_reuse_numeric_text_boolean_and_integrality_rules() {
    for (item, sample, expected) in [
        (
            r#"{"type":"number","minimum":0,"maximum":9007199254740992}"#,
            "[0,9007199254740992]",
            Admission::Admitted,
        ),
        (
            r#"{"type":"number","maximum":9007199254740992}"#,
            "[0,9007199254740993]",
            Admission::Rejected,
        ),
        (r#"{"type":"integer"}"#, "[10.0,1e1]", Admission::Admitted),
        (r#"{"type":"integer"}"#, "[10,0.1]", Admission::Rejected),
        (
            r#"{"type":"string","minLength":2,"maxLength":2}"#,
            r#"["é𝄞","e\u0301"]"#,
            Admission::Admitted,
        ),
        (
            r#"{"type":"string","minLength":2}"#,
            r#"["ok","é"]"#,
            Admission::Rejected,
        ),
        (r#"{"type":"boolean"}"#, "[true,false]", Admission::Admitted),
        (r#"{"type":"boolean"}"#, "[true,0]", Admission::Rejected),
    ] {
        let qualities = format!(r#"{{"type":"array","items":{item}}}"#);
        let catalogue = supplied(&qualities, sample);
        let wire = expect(&selected(&catalogue), expected, true);
        if expected == Admission::Rejected {
            assert!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["domain"] == "sample" && row["primary"]["pointer"] == "/1")
            );
        }
    }
}

#[test]
fn object_elements_accumulate_present_optional_interests_without_inserting_values() {
    let model = br##"{"sdfData":{"N":{"type":"number","maximum":1}},"sdfProperty":{"p":{"type":"array","items":{"type":"object","properties":{"x":{"sdfRef":"#/sdfData/N"},"unused":{"sdfRef":"missing:#/sdfData/Never"}}}}}}"##;
    for (sample, expected) in [
        (r#"[{}, {"x":1}]"#, Admission::Admitted),
        (r#"[{}, {"x":2}]"#, Admission::Rejected),
        (r#"[{"x":2}, {}]"#, Admission::Rejected),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        expect(&report, expected, true);
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .sample_value()
                .require()
                .unwrap()
                .raw_json(),
            sample
        );
        assert!(!report.to_json().contains("missing:#"));
    }
}

#[test]
fn object_item_required_presence_and_scalar_constraints_conjoin() {
    let qualities = r#"{"type":"array","items":{"type":"object","properties":{"x":{"type":"number","maximum":1}},"required":["x"]}}"#;
    for (sample, expected) in [
        (r#"[{"x":1},{"x":1}]"#, Admission::Admitted),
        (r#"[{}, {"x":2}]"#, Admission::Rejected),
    ] {
        let catalogue = supplied(qualities, sample);
        let wire = expect(&selected(&catalogue), expected, true);
        if expected == Admission::Rejected {
            let rows = wire["diagnostics"]["items"].as_array().unwrap();
            assert!(
                rows.iter().any(
                    |row| row["code"] == "sample.required" && row["primary"]["pointer"] == "/0"
                )
            );
            assert!(rows.iter().any(|row| row["code"] == "sample.bound" && row["primary"]["pointer"] == "/1/x"));
        }
    }
}

#[test]
fn item_choices_allow_distinct_element_witnesses_and_return_to_array_constraints() {
    let qualities = r#"{"type":"array","maxItems":2,"items":{"sdfChoice":{"on":{"type":"boolean","const":true},"off":{"type":"number","const":0}}}}"#;
    for (sample, expected) in [
        ("[true,0]", Admission::Admitted),
        ("[true,1]", Admission::Rejected),
        ("[true,0,0]", Admission::Rejected),
    ] {
        let catalogue = supplied(qualities, sample);
        expect(&selected(&catalogue), expected, true);
    }
}

#[test]
fn whole_array_choices_require_a_single_whole_array_witness() {
    let qualities = r#"{"type":"array","sdfChoice":{"all_boolean":{"items":{"type":"boolean"}},"all_number":{"items":{"type":"number"}}}}"#;
    for (sample, expected) in [
        ("[true,false]", Admission::Admitted),
        ("[1,2]", Admission::Admitted),
        ("[true,1]", Admission::Rejected),
    ] {
        let catalogue = supplied(qualities, sample);
        expect(&selected(&catalogue), expected, true);
    }
}

#[test]
fn full_properties_and_choice_alternatives_support_nested_arrays() {
    for qualities in [
        r#"{"type":"array","items":{"type":"object","properties":{"x":{"type":"array","minItems":1,"items":{"type":"number","maximum":1}}},"required":["x"]}}"#,
        r#"{"type":"array","items":{"sdfChoice":{"object":{"type":"object","properties":{"x":{"type":"array","minItems":1,"items":{"type":"number","maximum":1}}},"required":["x"]}}}}"#,
    ] {
        for (sample, expected) in [
            (r#"[{"x":[1]}]"#, Admission::Admitted),
            (r#"[{"x":[]}]"#, Admission::Rejected),
            (r#"[{"x":[2]}]"#, Admission::Rejected),
        ] {
            let catalogue = supplied(qualities, sample);
            expect(&selected(&catalogue), expected, true);
        }
    }
    let catalogue = supplied(
        r#"{"type":"array","items":{"sdfChoice":{"nested":{"type":"array","items":{"type":"boolean"}}}}}"#,
        "[[true],[false]]",
    );
    expect(&selected(&catalogue), Admission::Admitted, true);
}

#[test]
fn direct_item_wider_meanings_remain_opaque_and_needed_work_is_undetermined() {
    for (item, sample) in [
        (r#"{"type":"number","default":1}"#, "[1]"),
        (r##"{"sdfRef":"#/sdfData/N"}"##, "[1]"),
        (r#"{"type":"number","nullable":true}"#, "[null]"),
        (r#"{"type":"array","items":{"type":"boolean"}}"#, "[[true]]"),
        (r#"{"properties":{"x":{"type":"number"}}}"#, r#"[{"x":1}]"#),
    ] {
        let qualities = format!(r#"{{"type":"array","items":{item}}}"#);
        let catalogue = supplied(&qualities, sample);
        let report = selected(&catalogue);
        expect(&report, Admission::Undetermined, false);
        let effective = report
            .sample_result()
            .unwrap()
            .description()
            .require()
            .unwrap()
            .json();
        assert_eq!(
            effective
                .source_json()
                .get("items")
                .require()
                .unwrap()
                .raw_json(),
            item
        );
    }
    // Undeclared extension vocabulary is not a wider meaning, following ADR 0037:
    // Undeclared extension vocabulary.
    let catalogue = supplied(
        r#"{"type":"array","items":{"type":"number","x:meaning":1}}"#,
        "[1]",
    );
    expect(&selected(&catalogue), Admission::Admitted, true);
}

#[test]
fn known_count_rejection_survives_unknown_item_meaning() {
    let catalogue = supplied(
        r#"{"type":"array","maxItems":0,"items":{"type":"number","default":1}}"#,
        "[1]",
    );
    let report = selected(&catalogue);
    let wire = expect(&report, Admission::Rejected, false);
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "sample.count")
    );
}

#[test]
fn empty_and_singleton_arrays_keep_other_applicable_obligations() {
    for (qualities, sample, expected, complete) in [
        (
            r#"{"type":"array","minItems":1,"items":{"type":"number"}}"#,
            "[]",
            Admission::Rejected,
            true,
        ),
        (
            r#"{"type":"array","items":{"type":"boolean"}}"#,
            "[]",
            Admission::Admitted,
            true,
        ),
        (
            r#"{"type":"array","items":{"type":"boolean"}}"#,
            "[1]",
            Admission::Rejected,
            true,
        ),
        (
            r#"{"type":"array","const":[]}"#,
            "[]",
            Admission::Admitted,
            true,
        ),
        (
            r#"{"type":"array","items":{"type":"number"}}"#,
            "[null]",
            Admission::Undetermined,
            false,
        ),
        (
            r#"{"type":"array","nullable":false}"#,
            "null",
            Admission::Rejected,
            true,
        ),
    ] {
        let catalogue = supplied(qualities, sample);
        expect(&selected(&catalogue), expected, complete);
    }
}

#[test]
fn wrong_array_kind_rejects_without_requesting_item_imports() {
    for sample in ["1", r#""text""#, "{}"] {
        let catalogue = supplied(
            r#"{"type":"array","items":{"sdfRef":"missing:#/sdfData/Never"}}"#,
            sample,
        );
        let report = selected(&catalogue);
        let wire = expect(&report, Admission::Rejected, true);
        assert_eq!(report.outcome().errors(), 1);
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "sample.type")
            .unwrap();
        assert_eq!(finding["basis"]["citation"], "RFC 9880 Appendix C.4");
    }
}

#[test]
fn omitted_detail_preserves_counts_and_independent_element_errors() {
    let model = br#"{"sdfProperty":{"p":{"type":"array","maxItems":1,"items":{"type":"number","maximum":0}}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
        Limits::default().pointer_bytes(1),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model),
                SourceInput::supplied("s", b"[1,2]").sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue);
        expect(&report, Admission::Rejected, true);
        assert_eq!(report.outcome().errors(), 3);
    }
}
