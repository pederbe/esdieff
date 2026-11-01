//! Reporting preserves sample causes and coverage under ADR0025: Selected numeric samples.
use super::coverage_support;
use super::location_support;
use esdieff::{
    Admission, AssessmentStatus, Catalogue, CheckRequest, ConditionDomain, InspectRequest,
    JsonBuilder, Limits, Report, SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;

const TEMPERATURE: &[u8] = include_bytes!("../fixtures/temperature-device.sdf.json");
const TEMPERATURE_POINTER: &str = "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature";

fn selected<'a>(catalogue: &'a Catalogue, pointer: &str) -> Report<'a> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                pointer,
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap()
}

#[test]
fn sample_coverage_bases_describe_the_work_for_all_statuses_and_input_kinds() {
    let builder = JsonBuilder::new();
    let mut statuses = std::collections::BTreeSet::new();
    for (qualities, bytes, typed, limits) in [
        (
            r#""type":"number","maximum":100"#,
            &b"1"[..],
            builder.number("1").unwrap(),
            Limits::default(),
        ),
        (
            r#""type":"number","maximum":100,"const":1"#,
            &b"1"[..],
            builder.number("1").unwrap(),
            Limits::default(),
        ),
        (
            r#""type":"string","pattern":"hello""#,
            &br#""hello""#[..],
            builder.text("hello").unwrap(),
            Limits::default(),
        ),
        (
            r#""type":"string","sdfType":"byte-string","minLength":1"#,
            &br#""Zg""#[..],
            builder.text("Zg").unwrap(),
            Limits::default(),
        ),
        (
            r#""type":"number","nullable":true"#,
            &b"null"[..],
            builder.null().unwrap(),
            Limits::default(),
        ),
        (
            r#""type":"string"#,
            &br#""hello""#[..],
            builder.text("hello").unwrap(),
            Limits::default(),
        ),
        (
            r#""type":"number","maximum":100"#,
            &b"1e101"[..],
            builder.number("1e101").unwrap(),
            Limits::default().exponent_magnitude(100),
        ),
    ] {
        let model = format!("{{\"sdfProperty\":{{\"p\":{{{qualities}}}}}}}");
        let inputs = vec![
            SourceInput::supplied("s", bytes).sample(),
            SourceInput::typed("s", typed),
        ];
        for input in inputs {
            let catalogue = Catalogue::take_inputs_with_limits(
                vec![SourceInput::supplied("m", model.as_bytes()), input],
                limits.clone(),
            )
            .unwrap();
            let report = selected(&catalogue, "/sdfProperty/p");
            if qualities.contains("sdfType") {
                assert_eq!(
                    report
                        .assessment_area_status("sample-text-boolean/1")
                        .require()
                        .unwrap(),
                    AssessmentStatus::Unassessed
                );
            } else if qualities.contains("pattern") {
                assert_eq!(
                    report
                        .assessment_area_status("sample-text-boolean/1")
                        .require()
                        .unwrap(),
                    AssessmentStatus::Assessed
                );
            }
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["version"], "1.1");
            let rows = wire["coverage"]["rows"].as_array().unwrap();
            assert_eq!(rows.len(), 9);
            assert_eq!(wire["coverage"]["rows_omitted"], 0);
            let human = report.to_human();
            for row in rows {
                let area = row["id"].as_str().unwrap();
                let status = report.assessment_area_status(area).require().unwrap();
                assert_eq!(row["status"], status.as_str());
                assert!(human.contains(&format!("Sample area {area}: {}", status.as_str())));
                statuses.insert(status.as_str());
                match status {
                    AssessmentStatus::Unassessed => {
                        assert_eq!(row["basis"]["kind"], "operational");
                        assert_eq!(row["basis"]["id"], "sample-implementation");
                    }
                    AssessmentStatus::Undetermined => {
                        assert_eq!(row["basis"]["kind"], "operational");
                        assert_eq!(row["basis"]["id"], "sample-establishment");
                    }
                    _ if area == "sample-context/1" => {
                        assert_eq!(row["basis"]["kind"], "operational");
                        assert_eq!(row["basis"]["id"], "sample-establishment");
                    }
                    _ if area == "sample-abstract-values/1" && qualities.contains("sdfType") => {
                        assert_eq!(row["basis"]["kind"], "interpretation");
                        assert_eq!(row["basis"]["id"], "byte-string-json/1");
                        assert_eq!(row["basis"]["citation"], "RFC 9880 Section 4.7.1 Table 5");
                    }
                    _ => {
                        assert_eq!(row["basis"]["kind"], "rfc");
                        let citation = match (area, bytes) {
                            ("sample-map/1", _) => "RFC 9880 Appendix C.5",
                            ("sample-array/1", _) => "RFC 9880 Appendix C.4",
                            ("sample-const/1", _) => "RFC 9880 Appendix C",
                            ("sample-alternatives/1", _) => "RFC 9880 Section 4.7.2",
                            ("sample-type-null/1", b"null") => {
                                "RFC 9880 Section 4.7 and Appendix C"
                            }
                            (
                                "sample-type-null/1" | "sample-text-boolean/1",
                                b"\"hello\"" | b"\"Zg\"",
                            ) => "RFC 9880 Appendix C.2",
                            _ => "RFC 9880 Appendix C.1",
                        };
                        assert_eq!(row["basis"]["citation"], citation);
                    }
                }
                assert!(serde_json::to_string(row).unwrap().len() < 256);
            }
            let coverage = report.requested_coverage();
            assert_eq!(coverage.total(), Some(9));
            assert_eq!(
                coverage.assessed()
                    + coverage.not_applicable()
                    + coverage.unassessed()
                    + coverage.undetermined(),
                9
            );
            for condition in report
                .diagnostics()
                .conditions()
                .filter(|c| c.code() == "sample.coverage_unavailable")
            {
                let status = report
                    .assessment_area_status(condition.assessment_unit().unwrap())
                    .require()
                    .unwrap();
                assert_eq!(
                    condition.basis().id(),
                    Some(if status == AssessmentStatus::Unassessed {
                        "sample-implementation"
                    } else {
                        "sample-establishment"
                    })
                );
            }
        }
    }
    assert_eq!(
        statuses,
        ["assessed", "not_applicable", "unassessed", "undetermined"]
            .into_iter()
            .collect()
    );
}

#[test]
fn sample_reporting_changes_preserve_check_and_inspect_profiles() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", TEMPERATURE)]).unwrap();
    let checked = catalogue.check(CheckRequest::new()).unwrap();
    let wire: Value = serde_json::from_str(&checked.to_json()).unwrap();
    assert_eq!(wire["version"], "1.0");
    let rows = wire["coverage"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 8);
    assert_eq!(
        rows.iter()
            .filter(|row| row["id"] == "advisory-four/1")
            .count(),
        1
    );
    coverage_support::assert_requested_counts(&checked);
    assert_eq!(checked.requested_coverage().unassessed(), 0);
    for row in rows {
        if row["id"] == "advisory-four/1" {
            assert_eq!(row["status"], "assessed");
            assert_eq!(row["basis"]["kind"], "advice");
            assert_eq!(row["basis"]["id"], "advisory-four/1");
            assert_eq!(row["basis"]["citation"], "RFC 9880 Appendix C");
        } else {
            assert_eq!(row["status"], "assessed");
            assert_eq!(row["basis"]["kind"], "rfc");
            assert!(row["basis"]["citation"].is_string());
        }
    }
    let inspected = catalogue
        .inspect(InspectRequest::new(catalogue.source_id("m").unwrap(), ""))
        .unwrap();
    let wire: Value = serde_json::from_str(&inspected.to_json()).unwrap();
    assert_eq!(wire["version"], "1.0");
    assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 0);
}

#[test]
fn needed_sample_decoding_cause_keeps_its_original_location() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", TEMPERATURE),
        SourceInput::supplied("s", b"1e1000001").sample(),
    ])
    .unwrap();
    let report = selected(&catalogue, TEMPERATURE_POINTER);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert_eq!(report.outcome().errors(), 0);
    assert!(!report.outcome().complete());
    let diagnostics = report.diagnostics();
    let causes: Vec<_> = diagnostics
        .conditions()
        .filter(|c| c.code() == "operation.limit" && c.message() == "lexical exponent capability")
        .collect();
    assert_eq!(causes.len(), 1);
    assert_eq!(causes[0].domain(), Some(ConditionDomain::Operation));
    let location = location_support::primary_sample(causes[0]);
    assert_eq!(location.source_id(), catalogue.source_id("s").unwrap());
    assert_eq!(location.span(), Some(0..9));
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["diagnostics"]["omitted"], 0);
    assert_eq!(
        wire["diagnostics"]["retained"].as_u64().unwrap() as usize,
        wire["diagnostics"]["items"].as_array().unwrap().len()
    );
    assert!(report.to_human().contains("lexical exponent capability"));
}

#[test]
fn needed_typed_sample_decoding_cause_has_no_original_span() {
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![
            SourceInput::supplied("m", br#"{"sdfProperty":{"p":{"type":"number"}}}"#),
            SourceInput::typed("s", JsonBuilder::new().number("1234").unwrap()),
        ],
        Limits::default().number_token_bytes(3),
    )
    .unwrap();
    let report = selected(&catalogue, "/sdfProperty/p");
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    let diagnostics = report.diagnostics();
    let causes: Vec<_> = diagnostics
        .conditions()
        .filter(|c| c.code() == "operation.limit")
        .collect();
    assert_eq!(causes.len(), 1);
    assert_eq!(causes[0].domain(), Some(ConditionDomain::Operation));
    assert_eq!(location_support::primary_sample(causes[0]).span(), None);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let cause = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "operation.limit")
        .unwrap();
    assert_eq!(cause["primary"]["span"], Value::Null);
}

#[test]
fn unrelated_model_leaf_decoding_limit_is_excluded_from_sample_work() {
    let model =
        br#"{"sdfProperty":{"p":{"type":"number","maximum":100},"unused":{"default":1e1000001}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", b"101").sample(),
    ])
    .unwrap();
    let report = selected(&catalogue, "/sdfProperty/p");
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "operation.limit")
    );
}

#[test]
fn needed_sample_cause_counts_survive_optional_detail_limits() {
    let mut totals = Vec::new();
    for limits in [
        Limits::default(),
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", TEMPERATURE),
                SourceInput::supplied("s", b"1e1000001").sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue, TEMPERATURE_POINTER);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let items = wire["diagnostics"]["items"].as_array().unwrap();
        let omitted = wire["diagnostics"]["omitted"].as_u64().unwrap();
        let retained = wire["diagnostics"]["retained"].as_u64().unwrap();
        assert_eq!(retained, items.len() as u64);
        let total = retained + omitted;
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert_eq!(report.outcome().errors(), 0);
        assert!(!report.outcome().complete());
        if totals.len() == 1 {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["code"], "operation.limit");
            assert_eq!(items[0]["primary"]["span"], serde_json::json!([0, 9]));
            assert!(wire["diagnostics"]["truncated"].as_bool().unwrap());
        } else if totals.len() == 2 {
            assert!(items.is_empty());
            assert!(wire["diagnostics"]["truncated"].as_bool().unwrap());
        }
        totals.push(total);
    }
    assert_eq!(totals[0], 4);
    assert!(totals.iter().all(|n| *n == totals[0]));
}
