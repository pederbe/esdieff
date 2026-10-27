//! Cumulative assessment allowances follow ADR 0009: Bounded processing and reporting,
//! and ADR 0013: Shared workflow and report guarantees.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, SampleKind,
    SampleRequest, SourceInput,
};
use serde_json::Value;

#[test]
fn absent_and_inherited_candidates_leave_only_remaining_allowances_for_later_defaults() {
    let model = br#"{"info":{},"sdfData":{
        "empty":{},
        "value":{"type":"object","default":{},"required":["missing"],
            "sdfChoice":{"inherited":{},"later":{"default":{}}}}
    }}"#;
    let check = |limits| {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("m", model)], limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        let status = report
            .assessment_area_status("default-consistency/1")
            .require()
            .unwrap();
        (report.outcome().warnings(), status, report.to_json())
    };
    let (warnings, status, _) = check(Limits::default());
    assert_eq!(warnings, 2);
    assert_eq!(status, AssessmentStatus::Assessed);

    // Find public cutoffs after the owner's exclusion, before the later branch
    // finishes. Do not tie the test to private checker sizes or counters.
    for (limit, mut quotas) in allowance_cutoffs() {
        let partial = quotas.find_map(|quota| {
            let (warnings, status, wire) = check(limit(Limits::default(), quota));
            (warnings == 1 && status == AssessmentStatus::Undetermined).then_some(wire)
        });
        let wire: Value = serde_json::from_str(&partial.expect("late candidate cutoff")).unwrap();
        assert_eq!(wire["coverage"]["requested"]["total"], 10);
        assert_eq!(wire["outcome"]["complete"], false);
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "consistency.default_excluded"
                    && row["primary"]["pointer"] == "/sdfData/value/default")
                || wire["diagnostics"]["omitted"].as_u64().unwrap() > 0
        );
    }
}

type AllowanceCutoff = (
    fn(Limits, usize) -> Limits,
    std::iter::StepBy<std::ops::RangeInclusive<usize>>,
);

fn allowance_cutoffs() -> [AllowanceCutoff; 2] {
    [
        (Limits::decoding_work, (500..=20_000).step_by(25)),
        (
            Limits::effective_allocation_bytes,
            (1024..=256 * 1024).step_by(256),
        ),
    ]
}

#[test]
fn needed_sample_pairs_keep_late_errors_when_parameter_detail_storage_runs_out() {
    let mut properties = serde_json::Map::new();
    let mut values = serde_json::Map::new();
    for index in 0..40 {
        let name = format!("p{index:02}");
        properties.insert(
            name.clone(),
            serde_json::json!({"type":"number", "maximum":"bad"}),
        );
        values.insert(name, serde_json::json!(1));
    }
    // This description is outside the sample's interest and must not be checked.
    properties.insert(
        "absent".into(),
        serde_json::json!({"type":"number", "maximum":"also bad"}),
    );
    let model = serde_json::json!({"sdfProperty":{"p":{
        "type":"object", "properties":properties
    }}})
    .to_string();
    let sample = Value::Object(values).to_string();
    for detail in [4096, 1024 * 1024] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", sample.as_bytes()).sample(),
            ],
            Limits::default().diagnostic_bytes(detail),
        )
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 40);
        assert!(!report.outcome().complete());
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert_eq!(report.requested_coverage().total(), Some(9));
        let diagnostics = report.diagnostics();
        let retained = diagnostics
            .conditions()
            .filter(|row| row.code() == "data.number")
            .count();
        if detail == 4096 {
            assert!(retained < 40);
            assert!(diagnostics.truncated());
        } else {
            assert_eq!(retained, 40);
        }
    }
}

#[test]
fn late_full_meaning_refusal_keeps_earlier_formal_errors_and_independent_metadata() {
    let model = br#"{"info":{},"sdfData":{
        "later":{"type":"object","properties":{
            "nested":{"type":"array","items":{"type":"string"}}
        }},
        "early":{"type":"string","maxLength":18446744073709551616}
    }}"#;
    let check = |limits| {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("m", model)], limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        (
            report.outcome().errors(),
            report.outcome().complete(),
            report
                .requirement_status("rfc9880.model.M009/1")
                .require()
                .unwrap(),
            report
                .requirement_status("rfc9880.model.M053/1")
                .require()
                .unwrap(),
            report.to_json(),
        )
    };
    let (errors, complete, _, properties, _) = check(Limits::default());
    assert_eq!(errors, 1);
    assert!(complete);
    assert_eq!(properties, AssessmentStatus::Assessed);
    for (limit, mut quotas) in allowance_cutoffs() {
        let quota = quotas
            .find(|quota| {
                let (errors, complete, metadata, properties, wire) =
                    check(limit(Limits::default(), *quota));
                if errors != 1
                    || complete
                    || metadata != AssessmentStatus::Assessed
                    || properties != AssessmentStatus::Undetermined
                {
                    return false;
                }
                let wire: Value = serde_json::from_str(&wire).unwrap();
                let items = wire["diagnostics"]["items"].as_array().unwrap();
                items.iter().any(|row| row["code"] == "formal.uint")
                    && items.iter().any(|row| {
                        row["code"] == "operation.limit"
                            && row["primary"]["pointer"]
                                .as_str()
                                .is_some_and(|pointer| pointer.starts_with("/sdfData/later"))
                    })
            })
            .expect("Full meaning cutoff after the earlier formal error");
        let (errors, complete, metadata, properties, wire) =
            check(limit(Limits::default(), quota).diagnostic_bytes(1));
        assert_eq!(errors, 1);
        assert!(!complete);
        assert_eq!(metadata, AssessmentStatus::Assessed);
        assert_eq!(properties, AssessmentStatus::Undetermined);
        let wire: Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(wire["coverage"]["requested"]["total"], 13);
        assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
    }
}
