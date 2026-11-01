//! Metadata and vocabulary observations through the supplied-catalogue boundary.
//! Follows ADR 0003: Check findings and completeness, and
//! ADR 0008: Reject the historical Action input quality.
use super::coverage_support;
use super::location_support;
use esdieff::{
    BasisKind, Catalogue, CheckRequest, Disposition, Field, InspectRequest, Limits, ReportLocation,
    Severity, SourceInput,
};

#[test]
fn document_metadata_strings_are_checked_without_imposing_content_formats() {
    for quality in [
        "title",
        "description",
        "version",
        "modified",
        "copyright",
        "license",
        "$comment",
    ] {
        let text = format!(r#"{{"info":{{"{quality}":false}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("document", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1, "{quality}");
        let diagnostics = report.diagnostics();
        let condition = diagnostics
            .conditions()
            .find(|condition| condition.code() == "metadata.string")
            .unwrap();
        assert_eq!(condition.severity(), Severity::Error);
        assert_eq!(condition.basis().kind(), BasisKind::Rfc);
        assert!(
            matches!(condition.primary(), Field::Present(ReportLocation::Source(location))
            if matches!(location.pointer(), Field::Present(pointer) if pointer == format!("/info/{quality}")))
        );
        assert_eq!(
            catalogue
                .source(catalogue.source_id("document").unwrap())
                .unwrap()
                .raw_bytes(),
            text.as_bytes()
        );
    }
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "strings",
        br#"{"info":{"title":"","description":"text","version":"revision alpha","modified":"calendar semantics are separate","copyright":"text","license":"custom license terms","$comment":"source"}}"#,
    )])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 0);
    assert_eq!(report.outcome().notices(), 0);
    assert!(report.outcome().complete());
}

#[test]
fn common_metadata_follows_all_six_definition_classes_and_established_data_roles() {
    let text = br#"{
        "info":{}, "namespace":{"sdfRequiredInputData":"urn:example:base"},
        "sdfThing":{"Room":{"description":false,"label":0,"$comment":{},
            "sdfObject":{"Sensor":{"description":false,"label":0,"$comment":{},
                "sdfProperty":{"state":{"description":false,"label":0,"$comment":{},"type":"object","properties":{"sdfRequiredInputData":{"description":7}}}},
                "sdfAction":{"invoke":{"description":false,"label":0,"$comment":{},"sdfInputData":{"description":7},"sdfOutputData":{"$comment":false}}},
                "sdfEvent":{"changed":{"description":false,"label":0,"$comment":{},"sdfOutputData":{"label":0}}},
                "sdfData":{"Local":{"description":false,"label":0,"$comment":{},"type":"array","items":{"description":7,"$comment":false},"sdfChoice":{"sdfRequiredInputData":{"label":false}}}}
            }}
        }}
    }"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("document", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 27);
    assert_eq!(report.outcome().warnings(), 0);
    assert_eq!(report.outcome().notices(), 0);
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|c| c.code() == "metadata.string")
            .count(),
        27
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .filter(|row| row["code"] == "metadata.string" && row["primary"]["kind"] == "source")
            .count(),
        25
    );
    for quality in ["description", "$comment"] {
        assert_eq!(rows.iter().filter(|row| row["code"] == "metadata.string" && row["primary"]["kind"] == "effective" && row["primary"]["pointer"] == format!("/sdfThing/Room/sdfObject/Sensor/sdfData/Local/sdfChoice/sdfRequiredInputData/{quality}")).count(), 1);
    }
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "policy.historical_action_input")
    );
}

#[test]
fn optional_summary_and_late_policy_aggregates_survive_detail_limits() {
    let entries: Vec<_> = (0..200)
        .map(|index| format!(r#""example:extra{index}":{{"sdfRequiredInputData":true}}"#))
        .collect();
    let text = format!(
        r#"{{"info":{{}},"descriptino":true,{},"sdfRequiredInputData":true}}"#,
        entries.join(",")
    );
    let ample =
        Catalogue::from_inputs(&[SourceInput::supplied("document", text.as_bytes())]).unwrap();
    let report = ample.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(report.outcome().notices(), 1);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let summary = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "extension.optional_unsupported")
        .unwrap();
    assert_eq!(summary["arguments"]["observed_occurrences"], 200);
    for (limits, reason) in [
        (Limits::default().conditions(1), "count_limit"),
        (Limits::default().diagnostic_bytes(1), "byte_limit"),
        (Limits::default().pointer_bytes(1), "location_limit"),
    ] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("document", text.as_bytes())],
            limits,
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(report.outcome().warnings(), 1);
        assert_eq!(report.outcome().notices(), 1);
        coverage_support::assert_requested_counts(&report);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            wire["diagnostics"]["reasons"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(reason))
        );
        assert!(
            report
                .to_human()
                .contains("Errors: 1; warnings: 1; notices: 1; incomplete checks: 0")
        );
    }
}

#[test]
fn optional_selector_work_exhaustion_preserves_independent_policy_and_source_view() {
    let selector = "x".repeat(1500);
    let text = format!(
        r#"{{"info":{{}},"sdfRequiredInputData":true,"sdfData":{{"value":{{"type":"{selector}","description":7}}}}}}"#
    );
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("document", text.as_bytes())],
        Limits::default().decoding_work(6000),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "operation.limit")
    );
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "metadata.string")
    );
    let inspection = catalogue
        .inspect(InspectRequest::new(
            catalogue.source_id("document").unwrap(),
            "",
        ))
        .unwrap();
    assert!(inspection.outcome().complete());
    assert_eq!(inspection.inspection_json().unwrap().raw_json(), text);
    let ample =
        Catalogue::from_inputs(&[SourceInput::supplied("document", text.as_bytes())]).unwrap();
    let report = ample.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(report.outcome().notices(), 1);
}

#[test]
fn information_metadata_does_not_import_reference_patch_semantics_from_unsupported_spelling() {
    for reference in [
        r##""#/unused""##,
        "null",
        r#"{"sdfRequiredInputData":true,"title":false}"#,
    ] {
        let text = format!(
            r#"{{"info":{{"features":[],"sdfRef":{reference},"title":null,"sdfRequiredInputData":null}}}}"#
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("document", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 2, "{reference}");
        assert_eq!(report.outcome().notices(), 1);
        let diagnostics = report.diagnostics();
        assert!(
            diagnostics
                .conditions()
                .any(|c| c.code() == "metadata.string"
                    && location_support::primary_source(c).pointer().present()
                        == Some("/info/title"))
        );
        assert!(
            diagnostics
                .conditions()
                .any(|c| c.code() == "policy.historical_action_input")
        );
        let inspection = catalogue
            .inspect(InspectRequest::new(
                catalogue.source_id("document").unwrap(),
                "",
            ))
            .unwrap();
        assert!(inspection.outcome().complete());
        assert_eq!(inspection.inspection_json().unwrap().raw_json(), text);
    }
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "document",
        br#"{"info":{},"sdfRef":null,"sdfRequiredInputData":null}"#,
    )])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.outcome().notices(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "policy.historical_action_input")
    );
}

#[test]
fn wider_reference_forms_cannot_establish_null_metadata_or_policy_survival() {
    for reference in ["false", "null", "{}", "0", "[]"] {
        for (metadata, historical, errors) in [("null", "null", 0), ("7", "true", 2)] {
            let quality = format!(
                r#"{{"sdfRef":{reference},"description":{metadata},"sdfRequiredInputData":{historical}}}"#
            );
            for body in [
                format!(r#""sdfData":{{"value":{quality}}}"#),
                format!(r#""sdfAction":{{"invoke":{{"sdfInputData":{quality}}}}}"#),
                format!(r#""sdfData":{{"list":{{"type":"array","items":{quality}}}}}"#),
            ] {
                let text = format!(r#"{{"info":{{}},{body}}}"#);
                let catalogue =
                    Catalogue::from_inputs(&[SourceInput::supplied("document", text.as_bytes())])
                        .unwrap();
                let report = catalogue.check(CheckRequest::new()).unwrap();
                assert_eq!(report.outcome().errors(), errors, "{text}");
                assert!(!report.outcome().complete());
                let diagnostics = report.diagnostics();
                assert_eq!(
                    diagnostics
                        .conditions()
                        .filter(|c| matches!(
                            c.code(),
                            "metadata.string" | "policy.historical_action_input"
                        ))
                        .count(),
                    errors as usize
                );
                let inspection = catalogue
                    .inspect(InspectRequest::new(
                        catalogue.source_id("document").unwrap(),
                        "",
                    ))
                    .unwrap();
                assert!(inspection.outcome().complete());
                assert_eq!(inspection.inspection_json().unwrap().raw_json(), text);
            }
        }
    }
}

#[test]
fn readable_critical_lists_and_wider_declarations_defer_dependent_vocabulary() {
    for (features, detail) in [
        (
            r#"["future","urn:example:meaning"]"#,
            "extension.critical_unsupported",
        ),
        (
            r#"[{"future":true}]"#,
            "extension.feature_declaration_unavailable",
        ),
        (
            r#"["future",1]"#,
            "extension.feature_declaration_unavailable",
        ),
        (r#"["\uD800"]"#, "extension.feature_declaration_unavailable"),
        ("null", "extension.feature_declaration_unavailable"),
        ("true", "extension.feature_declaration_unavailable"),
        (r#""future""#, "extension.feature_declaration_unavailable"),
        ("{}", "extension.feature_declaration_unavailable"),
    ] {
        for info_first in [true, false] {
            let info = format!(
                r#""info":{{"features":{features},"title":false,"versoin":"typo","sdfRequiredInputData":true}}"#
            );
            let body = r#""sdfRequiredInputData":true,"sdfData":{"bad:Name":{"description":false,"sdfRequiredInputData":true}}"#;
            let text = if info_first {
                format!("{{{info},{body}}}")
            } else {
                format!("{{{body},{info}}}")
            };
            let catalogue = Catalogue::from_inputs(&[
                SourceInput::supplied("critical", text.as_bytes()),
                SourceInput::supplied(
                    "independent",
                    br#"{"info":{"title":false},"sdfRequiredInputData":true}"#,
                ),
            ])
            .unwrap();
            let selected = catalogue
                .check(CheckRequest::new().root(catalogue.source_id("critical").unwrap()))
                .unwrap();
            assert_eq!(selected.outcome().errors(), 0, "{features}");
            assert_eq!(selected.outcome().warnings(), 0);
            let diagnostics = selected.diagnostics();
            assert!(
                diagnostics
                    .conditions()
                    .any(|c| c.code() == "structure.feature_context_unavailable")
            );
            let condition = diagnostics
                .conditions()
                .find(|c| c.code() == detail)
                .unwrap();
            assert_eq!(condition.severity(), Severity::Incomplete);
            assert_eq!(condition.basis().kind(), BasisKind::Operational);
            assert_eq!(
                location_support::primary_source(condition)
                    .pointer()
                    .require()
                    .unwrap(),
                "/info/features"
            );
            assert_eq!(
                catalogue
                    .check(CheckRequest::new())
                    .unwrap()
                    .outcome()
                    .errors(),
                2
            );
            let source = catalogue
                .source(catalogue.source_id("critical").unwrap())
                .unwrap();
            assert_eq!(source.raw_bytes(), text.as_bytes());
        }
    }
}

#[test]
fn recognized_optional_spellings_are_summarized_without_registration_or_meaning_claims() {
    let text = br#"{
        "info":{"example:description":{"sdfRequiredInputData":true}},
        "version":"root extension content",
        "sdfData":{
            "value":{"type":"future-kind","example:opaque":{"descriptino":1,"sdfRequiredInputData":true}},
            "list":{"type":"array","items":{"type":"number","nullable":false,"label":false,"items":{"descriptino":1,"sdfRequiredInputData":true}}}
        }
    }"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("document", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 0);
    assert_eq!(report.outcome().notices(), 1);
    let diagnostics = report.diagnostics();
    let notice = diagnostics
        .conditions()
        .find(|c| c.code() == "extension.optional_unsupported")
        .unwrap();
    assert_eq!(notice.severity(), Severity::Notice);
    assert_eq!(notice.basis().kind(), BasisKind::Operational);
    assert_eq!(
        location_support::primary_source(notice).source_id(),
        catalogue.source_id("document").unwrap()
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let notice = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "extension.optional_unsupported")
        .unwrap();
    assert_eq!(notice["arguments"]["observed_occurrences"], 7);
    assert_eq!(
        wire["baseline"]["registry_knowledge"],
        serde_json::json!([])
    );
    assert_eq!(
        catalogue
            .source(catalogue.source_id("document").unwrap())
            .unwrap()
            .raw_bytes(),
        text
    );
}

#[test]
fn unfamiliar_quality_warnings_exclude_given_names_values_and_opaque_subtrees() {
    let text = br##"{
        "info":{"versoin":"author supplied"},
        "sdfObjekt":{},
        "sdfData":{
            "sdfRequiredInputData":{}, "descriptino":{},
            "record":{
                "type":"object", "descriptino":"typo",
                "properties":{"sdfRequiredInputData":{},"descriptino":{}},
                "sdfChoice":{"sdfRequiredInputData":{},"descriptino":{}},
                "const":{"sdfRequiredInputData":true,"descriptino":1},
                "default":{"versoin":1}, "enum":["sdfRequiredInputData","descriptino"],
                "custom":{"sdfRequiredInputData":true,"descriptino":1},
                "example:extra":{"sdfRequiredInputData":true,"descriptino":1}
            },
            "list":{"type":"array","items":{"descriptino":"typo"}},
            "patch":{"sdfRef":"#/sdfData/record","deletionTypo":null}
        }
    }"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("document", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(report.outcome().warnings(), 7);
    let diagnostics = report.diagnostics();
    let warnings: Vec<_> = diagnostics
        .conditions()
        .filter(|c| c.code() == "quality.unfamiliar")
        .collect();
    assert_eq!(warnings.len(), 7);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let conflicts: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|condition| condition["code"] == "data.enum_choice")
        .collect();
    assert_eq!(conflicts.len(), 2);
    for (kind, pointer) in [
        ("source", "/sdfData/record/enum"),
        ("effective", "/sdfData/patch/enum"),
    ] {
        assert!(conflicts.iter().any(|condition| {
            condition["primary"]["kind"] == kind && condition["primary"]["pointer"] == pointer
        }));
    }
    assert!(wire["diagnostics"]["items"].as_array().unwrap().iter().any(
        |condition| condition["code"] == "quality.unfamiliar"
            && condition["primary"]["kind"] == "effective"
            && condition["primary"]["pointer"] == "/sdfData/patch/descriptino"
    ));
    assert!(wire["diagnostics"]["items"].as_array().unwrap().iter().any(
        |condition| condition["code"] == "quality.unfamiliar"
            && condition["primary"]["kind"] == "effective"
            && condition["primary"]["pointer"] == "/sdfData/patch/custom"
    ));
    for condition in wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["code"] == "quality.unfamiliar")
    {
        assert_eq!(condition["basis"]["kind"], "advice");
        assert_eq!(condition["basis"]["id"], "quality.unfamiliar");
        assert_eq!(
            condition["basis"]["citation"],
            "RFC 9880 Sections 2.3.1 and 2.3.3"
        );
    }
    for condition in warnings {
        assert_eq!(condition.severity(), Severity::Warning);
        assert_eq!(condition.basis().kind(), BasisKind::Advice);
        assert_eq!(condition.basis().id(), Some("quality.unfamiliar"));
    }
    assert!(
        !diagnostics
            .conditions()
            .any(|c| c.code() == "policy.historical_action_input")
    );
    assert_eq!(
        catalogue
            .source(catalogue.source_id("document").unwrap())
            .unwrap()
            .raw_bytes(),
        text
    );
}

#[test]
fn surviving_patch_metadata_and_historical_qualities_have_target_independent_findings() {
    let text = br##"{
        "info":{},
        "sdfData":{
            "base":{"description":"source"},
            "number":{"sdfRef":"#/sdfData/base","description":7},
            "map":{"sdfRef":"#/sdfData/base","description":{}},
            "legacy":{"sdfRef":"#/sdfData/base","sdfRequiredInputData":true},
            "deletions":{"sdfRef":"#/sdfData/base","description":null,"label":null,"sdfRequiredInputData":null},
            "ordinary":{"label":false},
            "opaque":{"type":"object","const":{"description":7,"sdfRequiredInputData":true},"default":{"sdfRequiredInputData":true},"example:vocabulary":{"description":7,"sdfRequiredInputData":true}},
            "uncertain":{"sdfRef":"#/sdfData/base","\uD800":1,"description":7,"sdfRequiredInputData":true}
        }
    }"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("document", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 7);
    let diagnostics = report.diagnostics();
    let metadata: Vec<_> = diagnostics
        .conditions()
        .filter(|c| c.code() == "metadata.string")
        .collect();
    assert_eq!(metadata.len(), 5);
    for pointer in [
        "/sdfData/number/description",
        "/sdfData/map/description",
        "/sdfData/ordinary/label",
    ] {
        assert!(metadata.iter().any(|c| matches!(c.primary(), Field::Present(ReportLocation::Source(location)) if location.pointer().present() == Some(pointer))));
    }
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    for pointer in [
        "/sdfData/number/description",
        "/sdfData/map/description",
        "/sdfData/legacy/sdfRequiredInputData",
    ] {
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["primary"]["kind"] == "effective"
                    && finding["primary"]["pointer"] == pointer)
        );
    }
    let policy = diagnostics
        .conditions()
        .find(|c| c.code() == "policy.historical_action_input")
        .unwrap();
    assert_eq!(
        location_support::primary_source(policy)
            .pointer()
            .require()
            .unwrap(),
        "/sdfData/legacy/sdfRequiredInputData"
    );
    assert_eq!(
        catalogue
            .source(catalogue.source_id("document").unwrap())
            .unwrap()
            .raw_bytes(),
        text
    );
    assert!(!report.outcome().complete());
}

#[test]
fn historical_quality_rejection_has_acceptance_basis_current_guidance_and_raw_inspection() {
    let text = br#"{
        "sdfRequiredInputData":true,
        "info":{"sdfRequiredInputData":[]},
        "sdfAction":{"invoke":{"sdfRequiredInputData":{},"sdfInputData":{"sdfRequiredInputData":"field"}}},
        "sdfData":{"input":{"sdfRequiredInputData":null}}
    }"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("document", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 5);
    let diagnostics = report.diagnostics();
    let policies: Vec<_> = diagnostics
        .conditions()
        .filter(|c| c.code() == "policy.historical_action_input")
        .collect();
    assert_eq!(policies.len(), 5);
    for condition in policies {
        assert_eq!(condition.severity(), Severity::Error);
        assert_eq!(condition.basis().kind(), BasisKind::Acceptance);
        assert_eq!(condition.basis().id(), Some("A001"));
        assert_eq!(condition.basis().citation(), None);
        for guidance in ["sdfInputData", "object", "properties", "required"] {
            assert!(condition.message().contains(guidance));
        }
    }
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["policy"]["restrictions"],
        serde_json::json!(["A001", "A002"])
    );
    let id = catalogue.source_id("document").unwrap();
    let inspection = catalogue.inspect(InspectRequest::new(id, "")).unwrap();
    assert!(inspection.outcome().complete());
    assert_eq!(inspection.outcome().errors(), 0);
    assert_eq!(
        inspection.inspection_json().unwrap().raw_json(),
        std::str::from_utf8(text).unwrap()
    );
    let wire: serde_json::Value = serde_json::from_str(&inspection.to_json()).unwrap();
    assert_eq!(wire["policy"]["restrictions"], serde_json::json!(["A002"]));
}

#[test]
fn metadata_warning_details_cannot_hide_later_errors_or_change_fixed_coverage() {
    let input = br#"{"sdfThing":{"bad:Name":{"minItems":-1}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("document", input)],
        Limits::default().conditions(1),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(report.outcome().errors(), 2);
    coverage_support::assert_requested_counts(&report);
    assert_eq!(report.outcome().disposition(), Disposition::Failed);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["diagnostics"]["retained"], 1);
    assert_eq!(
        wire["diagnostics"]["items"][0]["code"],
        "metadata.info_absent"
    );
    assert_eq!(wire["diagnostics"]["omitted"], 2);
    assert_eq!(
        wire["diagnostics"]["reasons"],
        serde_json::json!(["count_limit"])
    );
    assert_eq!(wire["request"]["assessment"], "esdieff.initial/1");
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    let tiny = Catalogue::with_limits(
        &[SourceInput::supplied("document", input)],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let limited = tiny.check(CheckRequest::new()).unwrap();
    assert_eq!(limited.outcome().errors(), 2);
    assert_eq!(limited.outcome().warnings(), 1);
    let wire: serde_json::Value = serde_json::from_str(&limited.to_json()).unwrap();
    assert!(
        wire["diagnostics"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("byte_limit"))
    );
}

#[test]
fn absent_information_warns_and_strict_policy_changes_only_the_disposition() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("document", b"{}")]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
    let diagnostics = report.diagnostics();
    let condition = diagnostics
        .conditions()
        .find(|c| c.code() == "metadata.info_absent")
        .unwrap();
    assert_eq!(condition.severity(), Severity::Warning);
    assert_eq!(condition.basis().kind(), BasisKind::Advice);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let warning = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "metadata.info_absent")
        .unwrap();
    assert_eq!(warning["basis"]["kind"], "advice");
    assert_eq!(warning["basis"]["id"], "metadata.info_absent");
    assert_eq!(warning["basis"]["citation"], "RFC 9880 Section 3.1");
    assert_eq!(condition.basis().id(), Some("metadata.info_absent"));
    let strict = catalogue
        .check(CheckRequest::new().strict_warnings(true))
        .unwrap();
    assert_eq!(strict.outcome().disposition(), Disposition::Failed);
    assert_eq!(strict.outcome().warnings(), report.outcome().warnings());
    coverage_support::assert_requested_counts(&strict);
    for info in [r#"{"info":{}}"#, r#"{"info":{"features":[]}}"#] {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("document", info.as_bytes())]).unwrap();
        assert_eq!(
            catalogue
                .check(CheckRequest::new())
                .unwrap()
                .outcome()
                .warnings(),
            0
        );
    }
    let envelope = Catalogue::from_inputs(&[SourceInput::supplied("array", b"[]")]).unwrap();
    assert_eq!(
        envelope
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .warnings(),
        0
    );
}
