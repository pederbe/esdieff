//! Public component attribution, bounded work and independent query evidence.
//! Follows ADR 0027: Unit names and recorded registry guidance.
use esdieff::{
    Assessment, AssessmentStatus, BasisKind, Catalogue, CheckRequest, Field, Limits,
    RecordedRegistryPolicy, Severity, SourceInput,
};
use serde_json::Value;

#[test]
fn component_attribution_preserves_index_rule_and_qualified_delimiter_advice() {
    for (unit, code, severity, citation, queried) in [
        (
            "urn:ietf:params:unit:m/s",
            "unit.index_syntax",
            Severity::Error,
            "RFC 9880 Section 7.3",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?=",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?+",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?=/leading",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?+?query",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?+r?=",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?+/leading",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?=?query",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?+r?=/leading",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?+?=q",
            "unit.urn_components_syntax",
            Severity::Error,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?ordinary",
            "unit.urn_delimiter_advice",
            Severity::Notice,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?",
            "unit.urn_delimiter_advice",
            Severity::Notice,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?ordinary?+r#?=fragment",
            "unit.urn_delimiter_advice",
            Severity::Notice,
            "RFC 8141 Section 2",
            false,
        ),
        (
            "urn:ietf:params:unit:kg?+r?=q",
            "unit.urn_forbidden",
            Severity::Error,
            "RFC 9880 Section 4.7",
            true,
        ),
        (
            "urn:ietf:params:unit:kg?=q?=more",
            "unit.urn_forbidden",
            Severity::Error,
            "RFC 9880 Section 4.7",
            true,
        ),
        (
            "urn:ietf:params:unit:kg?+r?=q?+more",
            "unit.urn_forbidden",
            Severity::Error,
            "RFC 9880 Section 4.7",
            true,
        ),
    ] {
        for policy in [
            RecordedRegistryPolicy::UseRecorded,
            RecordedRegistryPolicy::WithholdRecorded,
        ] {
            let source = serde_json::json!({"info":{},"sdfData":{"A":{"unit":unit}}}).to_string();
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("fixture", source.as_bytes())])
                    .unwrap();
            let report = catalogue
                .check(
                    CheckRequest::new()
                        .assessment(Assessment::ExpandedUnits)
                        .recorded_registry_policy(policy),
                )
                .unwrap();
            let withheld = policy == RecordedRegistryPolicy::WithholdRecorded;
            assert_eq!(
                report.outcome().errors(),
                u32::from(severity == Severity::Error),
                "{unit}"
            );
            assert_eq!(
                report.outcome().notices(),
                u32::from(severity == Severity::Notice),
                "{unit}"
            );
            let units = report.unit_guidance().require().unwrap();
            assert_eq!(
                (
                    units.known_primary(),
                    units.known_secondary(),
                    units.unknown(),
                    units.unavailable(),
                    units.legacy(),
                    units.uri_names()
                ),
                (
                    u32::from(queried && !withheld),
                    0,
                    0,
                    u32::from(queried && withheld),
                    0,
                    1
                ),
                "{unit}"
            );
            assert_eq!(units.complete(), !queried || !withheld, "{unit}");
            assert!(matches!(
                report.assessment_area_status("unit-spelling/1"),
                Field::Present(AssessmentStatus::Assessed)
            ));
            assert_eq!(
                report
                    .assessment_area_status("unit-registry-guidance/1")
                    .require()
                    .unwrap(),
                if queried && withheld {
                    AssessmentStatus::Undetermined
                } else {
                    AssessmentStatus::Assessed
                }
            );
            let diagnostics = report.diagnostics();
            let condition = diagnostics
                .conditions()
                .find(|row| row.code() == code)
                .unwrap_or_else(|| panic!("missing {code} for {unit}"));
            assert_eq!(condition.severity(), severity);
            assert_eq!(condition.assessment_unit(), Some("unit-spelling/1"));
            assert_eq!(condition.basis().citation(), Some(citation));
            assert_eq!(
                condition.basis().kind(),
                if severity == Severity::Notice {
                    BasisKind::Advice
                } else {
                    BasisKind::Rfc
                }
            );
            assert_eq!(
                condition.basis().id(),
                (severity == Severity::Notice).then_some(code)
            );
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            let row = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["code"] == code)
                .unwrap();
            assert_eq!(row["primary"]["kind"], "effective");
            assert_eq!(row["primary"]["root"], 1);
            assert_eq!(row["primary"]["pointer"], "/sdfData/A/unit");
            assert_eq!(row["related"][0]["kind"], "source");
            assert_eq!(row["related"][0]["source_id"], 1);
            assert_eq!(row["related"][0]["pointer"], "/sdfData/A/unit");
            let token = serde_json::to_string(unit).unwrap();
            let start = source.find(&token).unwrap();
            assert_eq!(
                row["related"][0]["span"],
                serde_json::json!([start, start + token.len()])
            );
            assert_eq!(
                catalogue.sources().next().unwrap().raw_bytes(),
                source.as_bytes()
            );
            assert!(report.to_human().contains(code));
            assert!(matches!(
                report.registry_knowledge("iana.secondary-units"),
                Field::Absent
            ));
            if queried {
                assert_eq!(row["arguments"]["replacement"], "kg");
                assert_eq!(
                    report
                        .registry_knowledge("iana.senml-units")
                        .require()
                        .unwrap()
                        .state(),
                    if withheld { "unavailable" } else { "available" }
                );
            } else {
                assert!(row["arguments"].get("replacement").is_none());
                assert!(matches!(
                    report.registry_knowledge("iana.senml-units"),
                    Field::Absent
                ));
                assert!(matches!(units.source_edition(), Field::Absent));
                assert!(
                    wire["baseline"]["registry_knowledge"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }
}

#[test]
fn valid_question_data_fragments_and_percent_controls_keep_generic_uri_precedence() {
    for unit in [
        "urn:ietf:params:unit:kg?+r?ordinary",
        "urn:ietf:params:unit:kg?=q?ordinary",
        "urn:ietf:params:unit:kg?=q?+r",
        "urn:ietf:params:unit:kg?+r?=q?=data",
        "urn:ietf:params:unit:kg#?ordinary",
        "urn:ietf:params:unit:kg#?=",
        "urn:ietf:params:unit:kg?=q#?ordinary",
        "urn:ietf:params:unit:kg?+%2F",
        "urn:ietf:params:unit:kg?=q%3F%3Ddata",
    ] {
        let source = serde_json::json!({"info":{},"sdfData":{"A":{"unit":unit}}}).to_string();
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("fixture", source.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1, "{unit}");
        let units = report.unit_guidance().require().unwrap();
        assert_eq!(
            (
                units.known_primary(),
                units.unknown(),
                units.unavailable(),
                units.uri_names()
            ),
            (1, 0, 0, 1),
            "{unit}"
        );
        assert!(units.complete());
        let diagnostics = report.diagnostics();
        assert!(
            diagnostics
                .conditions()
                .any(|row| row.code() == "unit.urn_forbidden")
        );
        assert!(!diagnostics.conditions().any(|row| matches!(
            row.code(),
            "unit.urn_components_syntax" | "unit.urn_delimiter_advice"
        )));
    }
    for (unit, replacement) in [
        ("urn:ietf:params:unit:kg%3F", "kg?"),
        ("urn:ietf:params:unit:kg%3F%3Dq", "kg?=q"),
    ] {
        let source = serde_json::json!({"info":{},"sdfData":{"A":{"unit":unit}}}).to_string();
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("fixture", source.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        let units = report.unit_guidance().require().unwrap();
        assert_eq!(
            (units.known_primary(), units.unknown(), units.uri_names()),
            (0, 1, 1)
        );
        assert!(units.complete());
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let row = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "unit.urn_forbidden")
            .unwrap();
        assert_eq!(row["arguments"]["replacement"], replacement);
    }
    for unit in [
        "urn:ietf:params:unit:kg?=%GG",
        "urn:ietf:params:unit:kg?+r%",
        "urn:ietf:params:unit:kg#%XX",
        "urn:ietf:params:unit:kg?=é",
    ] {
        let source = serde_json::json!({"info":{},"sdfData":{"A":{"unit":unit}}}).to_string();
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("fixture", source.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        let units = report.unit_guidance().require().unwrap();
        assert_eq!(
            (
                units.known_primary(),
                units.unknown(),
                units.unavailable(),
                units.uri_names()
            ),
            (0, 0, 0, 0)
        );
        assert!(units.complete());
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|row| row.code() == "unit.uri_syntax")
        );
        assert!(matches!(
            report.registry_knowledge("iana.senml-units"),
            Field::Absent
        ));
    }
}

#[test]
fn mixed_components_preserve_other_queries_with_omitted_optional_details() {
    let bytes = br#"{"info":{},"sdfData":{"A":{"unit":"kg"},"B":{"unit":"urn:ietf:params:unit:kg?="},"C":{"unit":"urn:ietf:params:unit:kg?ordinary"},"D":{"multipleOf":0}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().conditions(1),
        Limits::default().report_bytes(41970),
    ] {
        for policy in [
            RecordedRegistryPolicy::UseRecorded,
            RecordedRegistryPolicy::WithholdRecorded,
        ] {
            let catalogue =
                Catalogue::with_limits(&[SourceInput::supplied("fixture", bytes)], limits.clone())
                    .unwrap();
            let report = catalogue
                .check(
                    CheckRequest::new()
                        .assessment(Assessment::ExpandedUnits)
                        .recorded_registry_policy(policy),
                )
                .unwrap();
            let withheld = policy == RecordedRegistryPolicy::WithholdRecorded;
            assert_eq!(
                (report.outcome().errors(), report.outcome().notices()),
                (2, 1)
            );
            let units = report.unit_guidance().require().unwrap();
            assert_eq!(
                (
                    units.known_primary(),
                    units.known_secondary(),
                    units.unknown(),
                    units.unavailable(),
                    units.uri_names()
                ),
                (u32::from(!withheld), 0, 0, u32::from(withheld), 2)
            );
            assert_eq!(units.complete(), !withheld);
            assert_eq!(
                report
                    .registry_knowledge("iana.senml-units")
                    .require()
                    .unwrap()
                    .state(),
                if withheld { "unavailable" } else { "available" }
            );
            assert!(matches!(
                report.registry_knowledge("iana.secondary-units"),
                Field::Absent
            ));
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(
                wire["baseline"]["registry_knowledge"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(wire["outcome"]["errors"], 2);
            assert_eq!(wire["outcome"]["notices"], 1);
            assert!(report.to_human().contains("Errors: 2;"));
            assert_eq!(catalogue.sources().next().unwrap().raw_bytes(), bytes);
        }
    }
}

#[test]
fn original_budget_stop_retains_earlier_components_without_inventing_queries() {
    let long = "x".repeat(10000);
    let source = format!(
        r#"{{"info":{{}},"sdfData":{{"A":{{"unit":"urn:ietf:params:unit:kg?=","multipleOf":0}},"B":{{"unit":"urn:ietf:params:unit:kg?ordinary"}},"C":{{"unit":"{long}"}}}}}}"#
    );
    for detail in [1, 1024 * 1024] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("fixture", source.as_bytes())],
            Limits::default()
                .decoding_work(100000)
                .diagnostic_bytes(detail),
        )
        .unwrap();
        assert!(
            catalogue
                .sources()
                .next()
                .unwrap()
                .json()
                .require()
                .unwrap()
                .capability_complete()
        );
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        assert_eq!(
            (report.outcome().errors(), report.outcome().notices()),
            (2, 1)
        );
        let units = report.unit_guidance().require().unwrap();
        assert_eq!(
            (
                units.known_primary(),
                units.known_secondary(),
                units.unknown(),
                units.unavailable(),
                units.uri_names()
            ),
            (0, 0, 0, 0, 2)
        );
        assert!(!units.complete());
        assert!(matches!(units.source_edition(), Field::Absent));
        assert!(matches!(
            report.registry_knowledge("iana.senml-units"),
            Field::Absent
        ));
        assert!(matches!(
            report.registry_knowledge("iana.secondary-units"),
            Field::Absent
        ));
        assert!(report.to_human().contains("Errors: 2;"));
    }
}
