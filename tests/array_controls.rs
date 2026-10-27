//! Finite array work follows ADR0009: Bounded processing and reporting,
//! and ADR0016: Selected samples with explicit admission and coverage.
mod map_support;
use esdieff::{Admission, AssessmentStatus, Catalogue, Field, Limits, SourceInput};
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

#[test]
fn uniqueness_and_compound_constants_preserve_independent_count_results() {
    for (qualities, sample, expected, complete) in [
        (
            r#"{"type":"array","uniqueItems":true}"#,
            "[1,1]",
            Admission::Rejected,
            true,
        ),
        (
            r#"{"type":"array","uniqueItems":false}"#,
            "[1,1]",
            Admission::Admitted,
            true,
        ),
        (
            r#"{"type":"array","maxItems":0,"uniqueItems":true}"#,
            "[1]",
            Admission::Rejected,
            true,
        ),
        (
            r#"{"type":"array","maxItems":0,"const":[1]}"#,
            "[1]",
            Admission::Rejected,
            true,
        ),
    ] {
        let catalogue = supplied(qualities, sample, Limits::default());
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert_eq!(report.outcome().complete(), complete);
        assert_eq!(report.requested_coverage().total(), Some(9));
    }
}

#[test]
fn invalid_needed_count_parameters_are_model_errors_and_keep_independent_valid_bounds() {
    for value in ["-1", "0.5", "true", r#""bad""#] {
        let qualities = format!(r#"{{"type":"array","minItems":{value},"maxItems":0}}"#);
        let catalogue = supplied(&qualities, "[1]", Limits::default());
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(!report.outcome().complete());
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let rows = wire["diagnostics"]["items"].as_array().unwrap();
        assert!(
            rows.iter()
                .any(|row| row["domain"] == "model" && row["severity"] == "error")
        );
        assert!(rows.iter().any(
            |row| row["code"] == "sample.count" && row["arguments"]["constraint"] == "maxItems"
        ));
    }
}

#[test]
fn shared_numeric_remainder_keeps_count_rejection_when_later_element_arithmetic_exhausts() {
    let qualities = r#"{"type":"array","maxItems":1,"items":{"type":"number","maximum":0}}"#;
    let mut found = None;
    for quota in 1..256 {
        let catalogue = supplied(qualities, "[1,2]", Limits::default().numeric_work(quota));
        let model = catalogue
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
            .unwrap();
        let sample = catalogue
            .source(catalogue.source_id("s").unwrap())
            .unwrap()
            .sample_value()
            .require()
            .unwrap();
        if !matches!(
            model.get("maxItems").require().unwrap().number(),
            Field::Present(_)
        ) || !matches!(
            model
                .get("items")
                .require()
                .unwrap()
                .get("maximum")
                .require()
                .unwrap()
                .number(),
            Field::Present(_)
        ) || sample
            .children()
            .any(|item| !matches!(item.number(), Field::Present(_)))
        {
            continue;
        }
        let report = selected(&catalogue);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        if report.sample_result().unwrap().admission() == Admission::Rejected
            && report
                .assessment_area_status("sample-numeric/1")
                .require()
                .unwrap()
                == AssessmentStatus::Undetermined
            && wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.count")
        {
            assert!(!report.outcome().complete());
            assert!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["code"] == "sample.count")
            );
            assert_eq!(selected(&catalogue).to_json(), report.to_json());
            found = Some(quota);
            break;
        }
    }
    assert!(
        found.is_some(),
        "a shared original numeric cutoff must retain its prior count proof"
    );
    println!("Public array numeric cutoff: {}", found.unwrap());
}

#[test]
fn original_depth_work_nodes_allocation_and_reference_limits_leave_array_work_incomplete() {
    let mut qualities = r#"{"type":"boolean"}"#.to_owned();
    let mut sample = "true".to_owned();
    for _ in 0..12 {
        qualities =
            format!(r#"{{"type":"array","items":{{"sdfChoice":{{"nested":{qualities}}}}}}}"#);
        sample = format!("[{sample}]");
    }
    let full = supplied(&qualities, &sample, Limits::default());
    assert_eq!(
        selected(&full).sample_result().unwrap().admission(),
        Admission::Admitted
    );
    for limits in [
        Limits::default().depth(8),
        Limits::default().decoding_work(1),
        Limits::default().effective_nodes(8),
        Limits::default().effective_allocation_bytes(1),
    ] {
        let catalogue = supplied(&qualities, &sample, limits);
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
        assert_eq!(report.requested_coverage().total(), Some(9));
    }
    let model = br##"{"sdfData":{"A":{"sdfRef":"#/sdfData/B"},"B":{"type":"number"}},"sdfProperty":{"p":{"type":"array","items":{"type":"object","properties":{"x":{"sdfRef":"#/sdfData/A"}}}}}}"##;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", br#"[{"x":1}]"#).sample(),
        ],
        Limits::default().reference_applications(1),
    )
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
}

#[test]
fn unusable_nested_sample_decoding_never_becomes_empty_or_singleton_admission() {
    for sample in [r#"["\ud800"]"#, r#"[{"a":1,"\u0061":2}]"#, "[", "[] null"] {
        let catalogue = supplied(
            r#"{"type":"array","items":{"type":"string"}}"#,
            sample,
            Limits::default(),
        );
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
    }
}

#[test]
fn repeated_and_alternating_elements_report_common_model_conditions_once() {
    for sample in ["[true,false,true]", "[true,1,\"x\",false]", "[1,2,3]"] {
        for limits in [
            Limits::default(),
            Limits::default().conditions(1),
            Limits::default().diagnostic_bytes(1),
        ] {
            let catalogue = supplied(r#"{"type":"array","items":{"type":17}}"#, sample, limits);
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Undetermined
            );
            assert_eq!(
                report.outcome().errors(),
                1,
                "common type shape once for {sample}"
            );
            assert!(!report.outcome().complete());
        }
    }
}

#[test]
fn later_numeric_interest_checks_a_malformed_bound_once() {
    for sample in ["[true,1,2,false]", "[1,true,2]"] {
        let catalogue = supplied(
            r#"{"type":"array","items":{"maximum":"bad"}}"#,
            sample,
            Limits::default(),
        );
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert_eq!(report.outcome().errors(), 1);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["domain"] == "model" && row["severity"] == "error")
                .count(),
            1
        );
    }
}

#[test]
fn alternative_copies_reset_model_interest_and_honor_explicit_overrides() {
    for sample in ["[true,false,true]", "[false,true]"] {
        for (qualities, errors) in [
            (
                r#"{"type":"array","items":{"type":17,"sdfChoice":{"inherited":{},"boolean":{"type":"boolean"}}}}"#,
                0,
            ),
            (
                r#"{"type":"array","items":{"sdfChoice":{"malformed":{"type":17},"boolean":{"type":"boolean"}}}}"#,
                1,
            ),
            (
                r#"{"type":"array","items":{"sdfChoice":{"one":{"type":17},"two":{"type":17},"boolean":{"type":"boolean"}}}}"#,
                2,
            ),
        ] {
            let catalogue = supplied(qualities, sample, Limits::default());
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Admitted
            );
            assert_eq!(report.outcome().errors(), errors);
            assert!(report.outcome().complete());
        }
    }
}

#[test]
fn independent_item_rejection_preserves_count_limits_and_vacuous_uniqueness() {
    for (qualities, complete) in [
        (
            r#"{"type":"array","minItems":"bad","items":{"type":"number","maximum":0}}"#,
            false,
        ),
        (
            r#"{"type":"array","uniqueItems":true,"items":{"type":"number","maximum":0}}"#,
            true,
        ),
    ] {
        let catalogue = supplied(qualities, "[1]", Limits::default());
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert_eq!(report.outcome().complete(), complete);
        assert!(report.to_json().contains("sample.bound"));
    }
}
