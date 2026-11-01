//! Ordinary map boundaries follow ADR0016: Selected samples with explicit admission and coverage,
//! and ADR0024: Effective alternatives and nullable characteristics.
use super::map_support;
use esdieff::{Admission, AssessmentStatus, Catalogue, Limits, Report, SourceInput};
use map_support::selected;
use serde_json::Value;

fn supplied(qualities: &str, sample: &str, limits: Limits) -> Catalogue {
    let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
    Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ],
        limits,
    )
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
    serde_json::from_str(&report.to_json()).unwrap()
}
#[test]
fn null_and_duplicate_presence_do_not_insert_or_repeat_missing_values() {
    for (qualities, sample, admission, errors) in [
        (
            r#"{"type":"object","properties":{"x":{"type":"string"}},"required":["x"]}"#,
            r#"{"x":null}"#,
            Admission::Admitted,
            0,
        ),
        (
            r#"{"type":"object","required":["x"]}"#,
            "null",
            Admission::Admitted,
            0,
        ),
        (
            r#"{"type":"object","required":["x","x"]}"#,
            "{}",
            Admission::Rejected,
            1,
        ),
    ] {
        let catalogue = supplied(qualities, sample, Limits::default());
        let report = selected(&catalogue);
        expect(&report, admission, true);
        assert_eq!(report.outcome().errors(), errors);
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
    }
}
#[test]
fn wrong_object_kind_uses_the_map_basis_without_entry_descent() {
    for sample in ["1", r#""text""#, "[]"] {
        let catalogue = supplied(
            r#"{"type":"object","properties":{"x":{"sdfRef":"missing:#/sdfData/Never"}},"required":["x"]}"#,
            sample,
            Limits::default(),
        );
        let report = selected(&catalogue);
        let value = expect(&report, Admission::Rejected, true);
        assert_eq!(report.outcome().errors(), 1);
        let condition = value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "sample.type")
            .unwrap();
        assert_eq!(condition["basis"]["citation"], "RFC 9880 Appendix C.5");
        assert!(
            !value["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"]
                    .as_str()
                    .is_some_and(|code| code.starts_with("reference.")))
        );
    }
}
#[test]
fn unfinished_child_meaning_keeps_an_independent_outer_missing_entry() {
    for (child, pending) in [
        (
            r#"{"type":"array","items":{"type":"string","format":"future-format"}}"#,
            r#"["2020-01-01"]"#,
        ),
        (
            r#"{"type":"string","format":"future-format"}"#,
            r#""2020-01-01""#,
        ),
        (r#"{"sdfType":"vendor:point"}"#, "2"),
        (
            r#"{"sdfChoice":{"known":{"const":1},"unknown":{"sdfRef":false}}}"#,
            "2",
        ),
    ] {
        let qualities = format!(
            r#"{{"type":"object","properties":{{"pending":{child}}},"required":["missing"]}}"#
        );
        let sample = format!(r#"{{"pending":{pending}}}"#);
        let catalogue = supplied(&qualities, &sample, Limits::default());
        let report = selected(&catalogue);
        expect(&report, Admission::Rejected, false);
        assert_eq!(report.outcome().errors(), 1);
    }
}
#[test]
fn different_kind_and_compound_constants_follow_ordinary_equality() {
    for (constant, admission, complete) in [
        ("1", Admission::Rejected, true),
        (r#"{"x":1}"#, Admission::Admitted, true),
    ] {
        let qualities = format!(
            r#"{{"type":"object","const":{constant},"properties":{{"x":{{"type":"number"}}}}}}"#
        );
        let catalogue = supplied(&qualities, r#"{"x":1}"#, Limits::default());
        expect(&selected(&catalogue), admission, complete);
    }
}
#[test]
fn a_present_referenced_child_uses_its_final_override_and_actual_origin() {
    let model=br##"{"sdfData":{"Base":{"type":"number","maximum":10}},"sdfProperty":{"p":{"type":"object","properties":{"value":{"sdfRef":"#/sdfData/Base","maximum":5}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#"{"value":6}"#).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    let value = expect(&report, Admission::Rejected, true);
    let condition = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "sample.bound")
        .unwrap();
    assert_eq!(condition["primary"]["pointer"], "/value");
    assert!(
        condition["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "effective"
                && row["pointer"] == "/sdfProperty/p/properties/value/maximum")
    );
    assert!(
        condition["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "source"
                && row["pointer"] == "/sdfProperty/p/properties/value/maximum")
    );
    let root = catalogue
        .source(catalogue.source_id("m").unwrap())
        .unwrap()
        .json()
        .require()
        .unwrap();
    assert_eq!(
        root.get("sdfData")
            .require()
            .unwrap()
            .get("Base")
            .require()
            .unwrap()
            .get("maximum")
            .require()
            .unwrap()
            .raw_json(),
        "10"
    );
}
#[test]
fn deleted_structural_type_does_not_infer_an_object() {
    let model=br##"{"sdfData":{"Base":{"type":"object","required":["x"]}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Base","type":null}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", b"{}").sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    expect(&report, Admission::Undetermined, false);
    assert_eq!(report.outcome().errors(), 0);
}
#[test]
fn two_numeric_children_share_the_original_remainder_and_repeated_calls_are_stable() {
    let qualities = r#"{"type":"object","properties":{"a":{"type":"number","maximum":100},"b":{"type":"number","maximum":100}}}"#;
    let mut found = None;
    for quota in 1..80 {
        let catalogue = supplied(
            qualities,
            r#"{"a":6,"b":6}"#,
            Limits::default().numeric_work(quota),
        );
        let sample = catalogue
            .source(catalogue.source_id("s").unwrap())
            .unwrap()
            .sample_value()
            .require()
            .unwrap();
        let properties = catalogue
            .source(catalogue.source_id("m").unwrap())
            .unwrap()
            .json()
            .require()
            .unwrap()
            .get("sdfProperty")
            .require()
            .unwrap()
            .get("p")
            .require()
            .unwrap()
            .get("properties")
            .require()
            .unwrap();
        if ["a", "b"].into_iter().any(|key| {
            sample
                .get(key)
                .require()
                .unwrap()
                .number()
                .present()
                .is_none()
                || properties
                    .get(key)
                    .require()
                    .unwrap()
                    .get("maximum")
                    .require()
                    .unwrap()
                    .number()
                    .present()
                    .is_none()
        }) {
            continue;
        }
        let report = selected(&catalogue);
        let one = supplied(
            qualities,
            r#"{"a":6}"#,
            Limits::default().numeric_work(quota),
        );
        if selected(&one).sample_result().unwrap().admission() == Admission::Admitted
            && report.sample_result().unwrap().admission() == Admission::Undetermined
        {
            assert_eq!(report.outcome().errors(), 0);
            assert_eq!(
                report
                    .assessment_area_status("sample-numeric/1")
                    .require()
                    .unwrap(),
                AssessmentStatus::Undetermined
            );
            expect(&selected(&catalogue), Admission::Undetermined, false);
            found = Some(quota);
            break;
        }
    }
    println!(
        "PUBLIC_SHARED_NUMERIC_QUOTA={}",
        found.expect("source-bound shared numeric cutoff")
    );
}
#[test]
fn supported_reference_and_effective_depth_failures_remain_unknown() {
    let model=br##"{"sdfData":{"A":{"sdfRef":"#/sdfData/B"},"B":{"type":"number"}},"sdfProperty":{"p":{"type":"object","properties":{"x":{"sdfRef":"#/sdfData/A"}}}}}"##;
    for limits in [
        Limits::default().reference_applications(1),
        Limits::default().reference_depth(1),
        Limits::default().effective_nodes(1),
        Limits::default().effective_allocation_bytes(1),
        Limits::default().depth(2),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model),
                SourceInput::supplied("s", br#"{"x":1}"#).sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue);
        expect(&report, Admission::Undetermined, false);
    }
}
#[test]
fn original_semantic_work_limits_preserve_a_known_outer_rejection() {
    let qualities = r#"{"type":"object","required":["missing"],"properties":{"later":{"type":"number","maximum":0}}}"#;
    let mut found = None;
    for quota in 500..10_000 {
        let catalogue = supplied(
            qualities,
            r#"{"later":1}"#,
            Limits::default().decoding_work(quota),
        );
        let report = selected(&catalogue);
        if report.sample_result().unwrap().admission() == Admission::Rejected
            && !report.outcome().complete()
        {
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            if wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.required")
            {
                expect(&selected(&catalogue), Admission::Rejected, false);
                found = Some(quota);
                break;
            }
        }
    }
    println!(
        "PUBLIC_MAP_SEMANTIC_WORK_QUOTA={}",
        found.expect("public original-work cutoff after required proof")
    );
}
#[test]
fn large_escaped_required_names_keep_real_parent_provenance_under_omission() {
    let entry = "\u{1}".repeat(8192);
    let qualities = serde_json::json!({"type":"object","required":[entry]}).to_string();
    for limits in [
        Limits::default(),
        Limits::default().pointer_bytes(1),
        Limits::default().diagnostic_bytes(1),
    ] {
        let catalogue = supplied(&qualities, "{}", limits);
        let report = selected(&catalogue);
        let value = expect(&report, Admission::Rejected, true);
        assert_eq!(report.outcome().errors(), 1);
        for condition in value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["code"] == "sample.required")
        {
            assert_eq!(condition["primary"]["pointer"], "");
            assert_eq!(condition["primary"]["span"], serde_json::json!([0, 2]));
            if let Some(name) = condition["arguments"]["entry"].as_str() {
                assert_eq!(name, entry);
            }
        }
    }
}
