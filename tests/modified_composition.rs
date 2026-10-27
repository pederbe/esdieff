//! Source revision metadata and composed model observations keep independent meanings.
//! Follows ADR 0021: Explicit modified metadata assessment, and
//! ADR 0020: Owned effective views and nearest origins.
mod coverage_support;
mod location_support;
use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, EffectiveRequest, Field, InspectRequest,
    Limits, ReportLocation, SourceInput, View,
};
use serde_json::Value;

const AREA: &str = "modified-date-time/1";
const KNOWLEDGE: &str = "iers-bulletin-c72/2026-07-06";
const EFFECTIVE_DESCRIPTION: &str = "/sdfData/Use/items/properties/field/description";
const COMPOSITION: &str = r##""sdfData":{"Base":{"type":"array","items":{"type":"object"}},"Use":{"sdfRef":"#/sdfData/Base","items":{"properties":{"field":{"description":7}}}}}"##;

fn source_orders(info: &str, definitions: &str) -> [String; 2] {
    [
        format!(r#"{{"info":{info},{definitions}}}"#),
        format!(r#"{{{definitions},"info":{info}}}"#),
    ]
}

#[test]
fn date_and_inherited_metadata_findings_keep_source_then_effective_locations() {
    for text in source_orders(r#"{"modified":"1900-02-29"}"#, COMPOSITION) {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("combined", text.as_bytes())]).unwrap();
        let id = catalogue.source_id("combined").unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 2);
        assert!(matches!(
            report.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Assessed)
        ));
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let metadata: Vec<_> = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["code"].as_str().unwrap().starts_with("metadata."))
            .collect();
        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[0]["code"], "metadata.modified_calendar");
        assert_eq!(metadata[0]["primary"]["kind"], "source");
        assert_eq!(metadata[0]["primary"]["pointer"], "/info/modified");
        assert_eq!(metadata[0]["assessment_unit"], AREA);
        assert_eq!(metadata[1]["code"], "metadata.string");
        assert_eq!(metadata[1]["primary"]["kind"], "effective");
        assert_eq!(metadata[1]["primary"]["pointer"], EFFECTIVE_DESCRIPTION);
        assert_eq!(catalogue.source(id).unwrap().raw_bytes(), text.as_bytes());
        let inspected = catalogue
            .inspect(InspectRequest::new(id, "/sdfData/Use/items").view(View::Effective))
            .unwrap();
        assert!(inspected.outcome().complete());
        let composed: Value = serde_json::from_str(&inspected.to_json()).unwrap();
        assert_eq!(composed["result"]["json"]["type"], "object");
        assert_eq!(
            composed["result"]["json"]["properties"]["field"]["description"],
            7
        );
    }
}
#[test]
fn early_information_and_completed_named_findings_do_not_replay_or_depend_on_detail() {
    let definitions = COMPOSITION.replacen(r#""sdfData":{"#, r#""sdfData":{"bad:Name":{},"#, 1);
    for text in source_orders(r#"{"modified":false}"#, &definitions) {
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
            let report = catalogue
                .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
                .unwrap();
            assert_eq!(report.outcome().errors(), 3);
            assert!(matches!(
                report.assessment_area_status(AREA),
                Field::Present(AssessmentStatus::Assessed)
            ));
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["outcome"]["errors"], 3);
            coverage_support::assert_wire_counts(&wire);
            let items = wire["diagnostics"]["items"].as_array().unwrap();
            if omitted {
                assert!(items.is_empty());
                assert_eq!(wire["coverage"]["rows_omitted"], 9);
                assert_eq!(report.diagnostics().conditions().len(), 0);
            } else {
                let information: Vec<_> = items
                    .iter()
                    .filter(|c| {
                        c["code"] == "metadata.string"
                            && c["primary"]["pointer"] == "/info/modified"
                    })
                    .collect();
                assert_eq!(information.len(), 1);
                assert_eq!(information[0]["primary"]["kind"], "source");
                assert_eq!(
                    items.iter().filter(|c| c["code"] == "name.colon").count(),
                    1
                );
                assert_eq!(
                    items
                        .iter()
                        .filter(|c| c["code"] == "metadata.string")
                        .count(),
                    2
                );
                assert!(!items.iter().any(|c| {
                    c["code"]
                        .as_str()
                        .unwrap()
                        .starts_with("metadata.modified_")
                }));
            }
        }
    }
}

#[test]
fn composition_and_omission_do_not_erase_outside_interval_consultation() {
    let label = "2030-06-30T23:59:59.000000000000000000001Z";
    for text in source_orders(&format!(r#"{{"modified":"{label}"}}"#), COMPOSITION) {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("combined", text.as_bytes())],
            Limits::default().diagnostic_bytes(1),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert!(matches!(
            report.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        coverage_support::assert_requested_counts(&report);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["baseline"]["temporal_knowledge"]["id"], KNOWLEDGE);
        assert_eq!(wire["coverage"]["rows_omitted"], 9);
        assert_eq!(report.diagnostics().conditions().len(), 0);
        assert_eq!(
            catalogue.sources().next().unwrap().raw_bytes(),
            text.as_bytes()
        );
        let json = catalogue
            .sources()
            .next()
            .unwrap()
            .json()
            .require()
            .unwrap();
        assert_eq!(
            json.get("info")
                .require()
                .unwrap()
                .get("modified")
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap(),
            label
        );
    }
}

#[test]
fn standalone_effective_and_inspection_do_not_acquire_modified_assessment() {
    let text = br##"{"info":{"modified":"2030-06-30T23:59:60Z","license":7},"sdfData":{"Base":{"type":"number"},"Use":{"sdfRef":"#/sdfData/Base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("inspection", text)]).unwrap();
    let id = catalogue.source_id("inspection").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(model.complete());
    assert!(matches!(model.json(), Field::Present(_)));
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(InspectRequest::new(id, "").view(view))
            .unwrap();
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(report.outcome().warnings(), 0);
        assert!(matches!(report.assessment_area_status(AREA), Field::Absent));
        assert_eq!(report.requested_coverage().count_unit(), "inspection");
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(wire["baseline"].get("temporal_knowledge").is_none());
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|c| c.code().starts_with("metadata."))
        );
        assert_eq!(
            wire["result"]["json"]["info"]["modified"],
            "2030-06-30T23:59:60Z"
        );
    }
}

#[test]
fn composed_checks_keep_initial_and_unknown_full_membership_unchanged() {
    let text = source_orders(r#"{"modified":"1900-02-29"}"#, COMPOSITION)
        .into_iter()
        .next()
        .unwrap();
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("combined", text.as_bytes())]).unwrap();
    for assessment in [
        Assessment::Initial,
        Assessment::FullBaseline,
        Assessment::ExpandedModified,
    ] {
        let report = catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        if matches!(
            assessment,
            Assessment::ExpandedModified | Assessment::FullBaseline
        ) {
            assert_eq!(report.outcome().errors(), 2);
            assert_eq!(
                report.requested_coverage().total(),
                Some(if assessment == Assessment::ExpandedModified {
                    9
                } else {
                    13
                })
            );
            assert_eq!(
                report.requested_coverage().inventory(),
                Some(if assessment == Assessment::ExpandedModified {
                    "esdieff.expanded.modified/1"
                } else {
                    "esdieff.full-model/1"
                })
            );
            assert!(matches!(
                report.assessment_area_status(if assessment == Assessment::ExpandedModified {
                    AREA
                } else {
                    "full.metadata/1"
                }),
                Field::Present(AssessmentStatus::Assessed)
            ));
        } else {
            assert_eq!(report.outcome().errors(), 1);
            assert!(
                !report
                    .diagnostics()
                    .conditions()
                    .any(|c| c.code().starts_with("metadata.modified_"))
            );
            if assessment == Assessment::Initial {
                assert_eq!(report.requested_coverage().total(), Some(8));
                assert_eq!(
                    report.requested_coverage().inventory(),
                    Some("esdieff.initial/1")
                );
                assert!(matches!(report.assessment_area_status(AREA), Field::Absent));
            } else {
                assert_eq!(report.requested_coverage().total(), Some(13));
                assert_eq!(
                    report.requested_coverage().inventory(),
                    Some("esdieff.full-model/1")
                );
                assert!(matches!(report.assessment_area_status(AREA), Field::Absent));
            }
        }
        assert!(wire["baseline"].get("temporal_knowledge").is_none());
    }
}

#[test]
fn multiple_composed_sources_share_modified_work_and_preserve_limit_attribution() {
    let fraction = "0".repeat(1000);
    let text = format!(
        r##"{{"info":{{"modified":"2024-02-29T12:34:56.{fraction}Z"}},"sdfData":{{"Base":{{"type":"number"}},"Use":{{"sdfRef":"#/sdfData/Base"}}}}}}"##
    );
    let inputs = [
        SourceInput::supplied("first", text.as_bytes()),
        SourceInput::supplied("second", text.as_bytes()),
    ];
    let catalogue = Catalogue::with_limits(&inputs, Limits::default().decoding_work(4608)).unwrap();
    let first = catalogue.source_id("first").unwrap();
    let second = catalogue.source_id("second").unwrap();
    for id in [first, second] {
        let inspected = catalogue.inspect(InspectRequest::new(id, "")).unwrap();
        assert!(inspected.outcome().complete());
        let checked = catalogue
            .check(
                CheckRequest::new()
                    .root(id)
                    .assessment(Assessment::ExpandedModified),
            )
            .unwrap();
        assert!(matches!(
            checked.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Assessed)
        ));
    }
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    let diagnostics = report.diagnostics();
    let limit = diagnostics.conditions().find(|c| {
        c.code() == "operation.limit" && c.assessment_unit() == Some(AREA)
            && matches!(c.primary(), Field::Present(ReportLocation::Source(location)) if location.source_id() == second)
    }).expect("combined work ceiling reached the second modified value");
    assert!(matches!(
        report.assessment_area_status(AREA),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(matches!(
        location_support::primary_source(limit).pointer(),
        Field::Present("/info/modified")
    ));
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "operation.limit"
                && c["assessment_unit"] == AREA
                && c["primary"]["pointer"] == "/info/modified")
    );
    println!(
        "cumulative_modified_work_witness=4608; each_root_assessed=true; combined=undetermined; second_source_limit_attributed=true"
    );
}

#[test]
fn critical_feature_deferral_still_gates_metadata_and_composition_together() {
    for text in source_orders(r#"{"features":["future"],"modified":false}"#, COMPOSITION) {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("critical", text.as_bytes()),
            SourceInput::supplied("independent", br#"{"info":{"modified":"1900-02-29"}}"#),
        ])
        .unwrap();
        let id = catalogue.source_id("critical").unwrap();
        let selected = catalogue
            .check(
                CheckRequest::new()
                    .root(id)
                    .assessment(Assessment::ExpandedModified),
            )
            .unwrap();
        assert_eq!(selected.outcome().errors(), 0);
        assert!(matches!(
            selected.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        assert!(
            !selected
                .diagnostics()
                .conditions()
                .any(|c| c.code().starts_with("metadata."))
        );
        assert!(
            selected
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "extension.critical_unsupported")
        );
        let wire: Value = serde_json::from_str(&selected.to_json()).unwrap();
        assert!(wire["baseline"].get("temporal_knowledge").is_none());
        assert!(matches!(
            catalogue
                .effective(EffectiveRequest::new(id))
                .unwrap()
                .json(),
            Field::Unavailable(_)
        ));
        let combined = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(combined.outcome().errors(), 1);
        assert!(matches!(
            combined.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        coverage_support::assert_requested_counts(&combined);
    }
}

#[test]
fn missing_same_document_reference_does_not_defer_established_modified_semantics() {
    let definitions = r##""sdfData":{"Use":{"sdfRef":"#/sdfData/Absent"}}"##;
    for text in source_orders(r#"{"modified":"1900-02-29"}"#, definitions) {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("missing", text.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert!(matches!(
            report.assessment_area_status(AREA),
            Field::Present(AssessmentStatus::Assessed)
        ));
        coverage_support::assert_requested_counts(&report);
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "reference.unavailable")
        );
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .filter(|c| c.code() == "metadata.modified_calendar")
                .count(),
            1
        );
    }
}
