//! Unit guidance shares model checking with advisory proofs and preserves selected workflows.
//! Follows ADR 0025: Selected numeric samples, ADR 0026: Required declaration
//! relationships and source evidence, and ADR 0027: Unit names and recorded registry guidance.
mod coverage_support;
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, InspectRequest,
    Limits, RecordedRegistryPolicy, SampleKind, SampleRequest, SourceInput, View,
};
use serde_json::Value;

const MODEL: &[u8] = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":["p"],"sdfProperty":{"p":{"type":"number","multipleOf":0.1,"maximum":100,"unit":"urn:ietf:params:unit:Cel"}}}},"sdfData":{"bounds":{"type":"number","nullable":false,"minimum":2,"maximum":1,"unit":"kg"},"text":{"type":"string","nullable":false,"minLength":2,"maxLength":1},"array":{"type":"array","nullable":false,"minItems":2,"maxItems":1},"primitive":{"type":"boolean","const":""}}}"#;

#[test]
fn all_four_advisory_classes_and_units_keep_their_own_basis_and_coverage() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", MODEL)]).unwrap();
    for policy in [
        RecordedRegistryPolicy::UseRecorded,
        RecordedRegistryPolicy::WithholdRecorded,
    ] {
        let report = catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::ExpandedUnits)
                    .recorded_registry_policy(policy),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(report.outcome().warnings(), 4);
        assert_eq!(
            report
                .assessment_area_status("advisory-four/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        assert_eq!(report.requested_coverage().total(), Some(10));
        assert_eq!(report.requested_coverage().unassessed(), 0);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        for code in [
            "proof.empty_numeric_interval",
            "proof.inverted_text_lengths",
            "proof.inverted_array_counts",
            "proof.primitive_const_type",
        ] {
            let condition = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["code"] == code)
                .unwrap();
            assert_eq!(condition["basis"]["kind"], "advice");
            assert!(condition["basis"]["citation"].is_string());
            assert_eq!(condition["assessment_unit"], "advisory-four/1");
        }
        let forbidden = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == "unit.urn_forbidden")
            .unwrap();
        assert_eq!(forbidden["basis"]["kind"], "rfc");
        assert_eq!(forbidden["arguments"]["replacement"], "Cel");
        assert!(report.to_human().contains("proof.empty_numeric_interval"));
        let guidance = report.unit_guidance().require().unwrap();
        if policy == RecordedRegistryPolicy::UseRecorded {
            assert_eq!(guidance.known_primary(), 2);
            coverage_support::assert_requested_counts(&report);
        } else {
            assert_eq!(guidance.unavailable(), 2);
            coverage_support::assert_requested_counts(&report);
        }
    }
}

#[test]
fn numeric_residual_exhaustion_keeps_unit_errors_and_omits_optional_citation() {
    let bytes = br#"{"info":{},"sdfData":{"p":{"type":"number","nullable":false,"minimum":2,"maximum":1,"unit":"urn:ietf:params:unit:Cel"}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", bytes)],
        Limits::default().numeric_work(2),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.outcome().warnings(), 0);
    assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 1);
    assert_eq!(
        report
            .assessment_area_status("advisory-four/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let limit = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["basis"]["id"] == "advisory-numeric-work")
        .unwrap();
    assert_eq!(limit["code"], "operation.limit");
    assert_eq!(
        limit["arguments"]["proof_class"],
        "proof.empty_numeric_interval"
    );
    assert!(limit["arguments"]["operation"].is_string());
    assert_eq!(limit["arguments"]["qualities"].as_array().unwrap().len(), 2);
    assert!(limit["basis"].get("citation").is_none());
}

#[test]
fn required_inspection_and_exact_sample_admission_stay_separate_from_unit_checking() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", MODEL),
        SourceInput::supplied("exact", b"0.3").sample(),
        SourceInput::supplied("inexact", b"0.30000000000000004").sample(),
    ])
    .unwrap();
    let model = catalogue.source_id("model").unwrap();
    let checked = catalogue
        .check(
            CheckRequest::new()
                .assessment(Assessment::ExpandedUnits)
                .root(model),
        )
        .unwrap();
    assert_eq!(checked.outcome().errors(), 1);
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(
                InspectRequest::new(model, "/sdfObject/O")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(report.required_declarations().require().unwrap().complete());
        assert!(matches!(report.unit_guidance(), Field::Absent));
    }
    for (sample, expected) in [
        ("exact", Admission::Admitted),
        ("inexact", Admission::Rejected),
    ] {
        let report = catalogue
            .check_sample(
                SampleRequest::new(model, SampleKind::Property, "/sdfObject/O/sdfProperty/p")
                    .sample(catalogue.source_id(sample).unwrap()),
            )
            .unwrap();
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().warnings(), 0);
        assert!(matches!(report.unit_guidance(), Field::Absent));
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert!(
            wire["baseline"]["registry_knowledge"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn optional_detail_omission_preserves_joint_aggregates_and_registry_observation() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", MODEL)],
        Limits::default().conditions(1).diagnostic_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert_eq!(
        (report.outcome().errors(), report.outcome().warnings()),
        (1, 4)
    );
    assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 2);
    coverage_support::assert_requested_counts(&report);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
    assert_eq!(
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        wire["result"]["unit_guidance"]["counts"]["known_primary"],
        2
    );
}

#[test]
fn unit_basis_omits_absent_citations_and_keeps_present_citations_as_strings() {
    let bytes = br#"{"info":{},"sdfData":{"marker":{"unit":"urn:ietf:Params:unit:kg"},"empty":{"unit":"urn:ietf:params:unit:"},"unknown":{"unit":"not-recorded-here"},"forbidden":{"unit":"urn:ietf:params:unit:kg"}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    for (code, citation) in [
        ("unit.marker_case_unresolved", false),
        ("unit.index_meaning_unavailable", false),
        ("unit.registration_unknown", true),
        ("unit.urn_forbidden", true),
    ] {
        let condition = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|condition| condition["code"] == code)
            .unwrap();
        if citation {
            assert!(condition["basis"]["citation"].is_string());
        } else {
            assert!(condition["basis"].get("citation").is_none(), "{code}");
        }
    }
    let mut found_work = false;
    for work in [128, 256, 512, 1024, 2048, 4096] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied(
                "model",
                br#"{"info":{},"sdfData":{"p":{"unit":"kg"}}}"#,
            )],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        if let Some(condition) = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|condition| condition["basis"]["id"] == "unit-establishment")
        {
            assert!(condition["basis"].get("citation").is_none());
            found_work = true;
        }
    }
    assert!(found_work, "exercise a real unit operational limit");
}
