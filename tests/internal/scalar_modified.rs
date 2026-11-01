//! Scalar constraints and revision metadata retain independent assessment states.
//! Follows ADR 0021: Explicit modified metadata assessment, and
//! ADR 0022: Scalar constraints and characteristic states.
use super::coverage_support;
use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, Class, EffectiveRequest, Field, Limits,
    Lookup, SourceInput,
};
use serde_json::Value;

const AREA: &str = "modified-date-time/1";
const SCALARS: &str = r#""sdfProperty":{"step":{"multipleOf":0},"count":{"minItems":-1}}"#;

fn source_orders(info: &str) -> [String; 2] {
    [
        format!(r#"{{"info":{info},{SCALARS}}}"#),
        format!(r#"{{{SCALARS},"info":{info}}}"#),
    ]
}

#[test]
fn scalar_errors_and_native_date_status_survive_source_order_and_detail_omission() {
    for text in source_orders(r#"{"modified":"1900-02-29"}"#) {
        for omitted in [false, true] {
            let limits = if omitted {
                Limits::default().diagnostic_bytes(1)
            } else {
                Limits::default()
            };
            let catalogue = Catalogue::with_limits(
                &[SourceInput::supplied("combined", text.as_bytes())],
                limits,
            )
            .unwrap();
            for assessment in [
                Assessment::Initial,
                Assessment::FullBaseline,
                Assessment::ExpandedModified,
            ] {
                let report = catalogue
                    .check(CheckRequest::new().assessment(assessment))
                    .unwrap();
                let expanded = matches!(
                    assessment,
                    Assessment::ExpandedModified | Assessment::FullBaseline
                );
                assert_eq!(report.outcome().errors(), if expanded { 3 } else { 2 });
                coverage_support::assert_requested_counts(&report);
                if assessment == Assessment::ExpandedModified {
                    assert!(matches!(
                        report.assessment_area_status(AREA),
                        Field::Present(AssessmentStatus::Assessed)
                    ));
                    assert_eq!(report.requested_coverage().total(), Some(9));
                    coverage_support::assert_requested_counts(&report);
                    assert_eq!(report.requested_coverage().unassessed(), 0);
                } else if assessment == Assessment::Initial {
                    assert!(matches!(report.assessment_area_status(AREA), Field::Absent));
                    assert_eq!(report.requested_coverage().total(), Some(8));
                    assert_eq!(report.requested_coverage().unassessed(), 0);
                } else {
                    assert!(matches!(report.assessment_area_status(AREA), Field::Absent));
                    assert_eq!(report.requested_coverage().total(), Some(13));
                }
                let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
                assert!(wire["baseline"].get("temporal_knowledge").is_none());
                let rows = wire["diagnostics"]["items"].as_array().unwrap();
                if omitted {
                    assert!(rows.is_empty());
                    if expanded {
                        assert_eq!(
                            wire["coverage"]["rows_omitted"],
                            if assessment == Assessment::ExpandedModified {
                                9
                            } else {
                                13
                            }
                        );
                    }
                } else {
                    for (code, pointer) in [
                        ("parameter.positive_step", "/sdfProperty/step/multipleOf"),
                        ("data.count", "/sdfProperty/count/minItems"),
                    ] {
                        assert_eq!(
                            rows.iter()
                                .filter(|row| row["code"] == code
                                    && row["primary"]["kind"] == "source"
                                    && row["primary"]["pointer"] == pointer)
                                .count(),
                            1
                        );
                    }
                    assert_eq!(
                        rows.iter()
                            .filter(|row| row["code"] == "metadata.modified_calendar")
                            .count(),
                        usize::from(expanded)
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_findings_do_not_erase_omitted_native_uncertainty_or_knowledge_identity() {
    for text in source_orders(r#"{"modified":"2030-06-30T23:59:59.000000000000000000001Z"}"#) {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("combined", text.as_bytes())],
            Limits::default().diagnostic_bytes(1),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 2);
        assert!(matches!(
            report.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        coverage_support::assert_requested_counts(&report);
        assert_eq!(report.requested_coverage().unassessed(), 0);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["baseline"]["temporal_knowledge"]["id"],
            "iers-bulletin-c72/2026-07-06"
        );
        assert_eq!(wire["coverage"]["rows_omitted"], 9);
        assert_eq!(report.diagnostics().conditions().len(), 0);
        assert_eq!(
            catalogue.sources().next().unwrap().raw_bytes(),
            text.as_bytes()
        );
    }
}

#[test]
fn critical_context_gates_date_and_scalar_meanings_but_keeps_independent_findings() {
    for text in source_orders(r#"{"features":["future"],"modified":false}"#) {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("critical", text.as_bytes()),
            SourceInput::supplied(
                "ordinary",
                br#"{"info":{"modified":"2024-02-29"},"sdfProperty":{"value":{"multipleOf":0}}}"#,
            ),
        ])
        .unwrap();
        let critical_id = catalogue.source_id("critical").unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert!(matches!(
            report.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .filter(|condition| condition.code() == "parameter.positive_step")
                .count(),
            1
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "extension.critical_unsupported")
        );
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "data.count"
                    || condition.code().starts_with("metadata."))
        );
        let model = catalogue
            .effective(EffectiveRequest::new(critical_id))
            .unwrap();
        assert!(matches!(model.json(), Field::Unavailable(_)));
        assert!(matches!(
            model.definition(Class::Property, "step"),
            Lookup::Unavailable(_)
        ));
    }
}
