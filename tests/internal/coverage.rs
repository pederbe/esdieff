use super::coverage_support;
use coverage_support::{assert_initial_completion, native_catalogue, observe_native_domain};
use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, Limits, SourceInput};
use serde_json::Value;

#[test]
fn native_catalogue_and_both_selected_documents_complete_initial_work() {
    for reverse in [false, true] {
        let catalogue = native_catalogue(reverse);
        observe_native_domain(&catalogue);
        assert_initial_completion(&catalogue, None);
        assert_initial_completion(&catalogue, Some("device"));
        assert_initial_completion(&catalogue, Some("controller"));
    }
}

#[test]
fn full_native_states_bind_all_requirements_and_join_each_area_once() {
    use AssessmentStatus::{Assessed, NotApplicable};
    let assessed = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 29, 31, 33, 34, 36,
        42, 45,
    ];
    for reverse in [false, true] {
        let catalogue = native_catalogue(reverse);
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        coverage_support::assert_full_binding(&report);
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(
            report.requirement_inventory().require().unwrap(),
            "rfc9880.model/1"
        );
        let coverage = report.requirement_coverage().require().unwrap();
        assert_eq!(coverage.count_unit(), "requirement");
        assert_eq!(coverage.total(), Some(54));
        assert_eq!(coverage.assessed(), assessed.len() as u32);
        assert_eq!(coverage.unassessed(), 0);
        assert_eq!(coverage.undetermined(), 0);
        let descriptors = report.requirement_descriptors().require().unwrap();
        assert_eq!(descriptors.len(), 54);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["broader"]["inventory_status"], "available");
        assert_eq!(
            wire["coverage"]["requirement_rows"]
                .as_array()
                .unwrap()
                .len(),
            54
        );
        for (index, descriptor) in descriptors.enumerate() {
            let id = format!("rfc9880.model.M{:03}/1", index + 1);
            assert_eq!(descriptor.id(), id);
            assert!(descriptor.citations().starts_with("RFC 9880"));
            assert!(coverage_support::FULL_AREAS.contains(&descriptor.assessment_area()));
            let expected = if assessed.contains(&(index + 1)) {
                Assessed
            } else {
                NotApplicable
            };
            assert_eq!(
                report.requirement_status(&id).require().unwrap(),
                expected,
                "{id}"
            );
            let row = &wire["coverage"]["requirement_rows"][index];
            assert_eq!(row["id"], id);
            assert_eq!(row["count_unit"], "requirement");
            assert_eq!(row["coverage_owner"], descriptor.assessment_area());
            assert!(!descriptor.planned_check().contains("pending"));
        }
        assert!(matches!(
            report.requirement_status("unknown/1"),
            Field::Absent
        ));
        assert_eq!(report.requested_coverage().total(), Some(13));
        assert_eq!(report.requested_coverage().unassessed(), 0);
        let initial = catalogue.check(CheckRequest::new()).unwrap();
        assert!(matches!(initial.requirement_inventory(), Field::Absent));
        assert!(matches!(
            initial.requirement_status("rfc9880.model.M001/1"),
            Field::Absent
        ));
        assert!(matches!(initial.requirement_descriptors(), Field::Absent));
        assert!(matches!(initial.requirement_coverage(), Field::Absent));
    }
}

#[test]
fn type_and_formal_count_findings_have_context_specific_owners() {
    for (text, limits, member, code, owner, others) in [
        (
            br#"{"info":{},"sdfData":{"D":{"type":42}}}"#.as_slice(),
            Limits::default(),
            34,
            "data.string",
            "full.data-grammar/1",
            [35, 36, 37],
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"items":{"type":42}}}}"#.as_slice(),
            Limits::default(),
            34,
            "data.string",
            "full.data-grammar/1",
            [35, 36, 37],
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"minLength":12345}}}"#.as_slice(),
            Limits::default().number_token_bytes(2),
            37,
            "data.number_unavailable",
            "full.formal-numbers/1",
            [34, 35, 36],
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"items":{"minLength":12345}}}}"#.as_slice(),
            Limits::default().number_token_bytes(2),
            37,
            "data.number_unavailable",
            "full.formal-numbers/1",
            [34, 35, 36],
        ),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("m", text)], limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        let diagnostics = report.diagnostics();
        let condition = diagnostics
            .conditions()
            .find(|c| c.code() == code)
            .expect(code);
        assert_eq!(condition.coverage_owner(), Some(owner));
        assert_eq!(
            report
                .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
                .require()
                .unwrap(),
            if member == 37 {
                AssessmentStatus::Undetermined
            } else {
                AssessmentStatus::Assessed
            }
        );
        for other in others {
            assert_eq!(
                report
                    .requirement_status(&format!("rfc9880.model.M{other:03}/1"))
                    .require()
                    .unwrap(),
                AssessmentStatus::NotApplicable
            );
        }
    }
}

#[test]
fn required_findings_keep_the_inspection_producer_and_full_owner() {
    let text =
        br#"{"info":{},"sdfObject":{"O":{"sdfRequired":["missing"],"sdfProperty":{"p":{}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", text)]).unwrap();
    for profile in [Assessment::Initial, Assessment::FullBaseline] {
        let report = catalogue
            .check(CheckRequest::new().assessment(profile))
            .unwrap();
        let diagnostics = report.diagnostics();
        let condition = diagnostics
            .conditions()
            .find(|c| c.code() == "required.target_missing")
            .unwrap();
        assert_eq!(condition.assessment_unit(), Some("inspection"));
        assert_eq!(
            condition.coverage_owner(),
            if profile == Assessment::FullBaseline {
                Some("full.composition-requiredness/1")
            } else {
                None
            }
        );
        let terse = Catalogue::with_limits(
            &[SourceInput::supplied("m", text)],
            Limits::default().diagnostic_bytes(1),
        )
        .unwrap();
        let terse = terse
            .check(CheckRequest::new().assessment(profile))
            .unwrap();
        assert_eq!(terse.outcome().notices(), report.outcome().notices());
        assert_eq!(terse.outcome().errors(), report.outcome().errors());
        assert_eq!(terse.outcome().complete(), report.outcome().complete());
    }
}

#[test]
fn exclusive_guards_and_needed_failures_do_not_credit_unrelated_requirements() {
    for (text, member, expected) in [
        (
            br#"{"info":{},"sdfProperty":{"p":{"nullable":false}}}"#.as_slice(),
            22,
            AssessmentStatus::NotApplicable,
        ),
        (
            br#"{"info":{},"sdfProperty":{"p":{"nullable":false}}}"#.as_slice(),
            33,
            AssessmentStatus::Assessed,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"example:quality":{"arbitrary":true}}}}"#.as_slice(),
            33,
            AssessmentStatus::Assessed,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"example:quality":{"arbitrary":true}}}}"#.as_slice(),
            28,
            AssessmentStatus::Undetermined,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"minLength":-1}}}"#.as_slice(),
            37,
            AssessmentStatus::Assessed,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"minLength":1}}}"#.as_slice(),
            37,
            AssessmentStatus::Assessed,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"sdfType":"byte-string"}}}"#.as_slice(),
            26,
            AssessmentStatus::Assessed,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"sdfType":"unix-time"}}}"#.as_slice(),
            27,
            AssessmentStatus::Assessed,
        ),
    ] {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", text)]).unwrap();
        let initial = catalogue.check(CheckRequest::new()).unwrap();
        assert!(initial.outcome().complete());
        let full = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            full.requirement_status(&format!("rfc9880.model.M{member:03}/1"))
                .require()
                .unwrap(),
            expected
        );
    }
    for (input, limits) in [
        (SourceInput::failed("m"), Limits::default()),
        (
            SourceInput::supplied("m", b"{} trailing"),
            Limits::default().document_bytes(2),
        ),
        (
            SourceInput::supplied("m", b"{}"),
            Limits::default().decoding_work(1),
        ),
    ] {
        let catalogue = Catalogue::with_limits(&[input], limits).unwrap();
        for profile in [Assessment::Initial, Assessment::FullBaseline] {
            let report = catalogue
                .check(CheckRequest::new().assessment(profile))
                .unwrap();
            assert!(!report.outcome().complete());
            assert_eq!(report.outcome().errors(), 0);
            if profile == Assessment::Initial {
                assert_eq!(
                    report
                        .assessment_area_status("document-envelope/1")
                        .require()
                        .unwrap(),
                    AssessmentStatus::Undetermined
                );
            } else {
                assert_eq!(
                    report
                        .requirement_status("rfc9880.model.M001/1")
                        .require()
                        .unwrap(),
                    AssessmentStatus::Undetermined
                );
            }
        }
    }
}

#[test]
fn all_optional_requirement_detail_can_be_absent_without_erasing_typed_membership() {
    let inputs = [
        SourceInput::supplied("device", coverage_support::DEVICE),
        SourceInput::supplied("controller", coverage_support::CONTROLLER),
        SourceInput::supplied("level", coverage_support::LEVEL),
    ];
    let ordinary = Catalogue::from_inputs(&inputs).unwrap();
    let terse = Catalogue::with_limits(&inputs, Limits::default().diagnostic_bytes(1)).unwrap();
    let full = ordinary
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let limited = terse
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    coverage_support::assert_full_binding(&limited);
    let wire: Value = serde_json::from_str(&limited.to_json()).unwrap();
    assert!(wire["request"].get("requirement_inventory").is_none());
    for field in [
        "requirements",
        "requirement_rows",
        "requirement_rows_retained",
        "requirement_rows_omitted",
        "requirement_omissions",
    ] {
        assert!(wire["coverage"].get(field).is_none());
    }
    assert_eq!(
        limited.requirement_inventory().require().unwrap(),
        "rfc9880.model/1"
    );
    assert_eq!(
        limited.requirement_descriptors().require().unwrap().len(),
        54
    );
    for descriptor in full.requirement_descriptors().require().unwrap() {
        assert_eq!(
            limited
                .requirement_status(descriptor.id())
                .require()
                .unwrap(),
            full.requirement_status(descriptor.id()).require().unwrap()
        );
    }
    assert_eq!(
        limited
            .requirement_coverage()
            .require()
            .unwrap()
            .unassessed(),
        full.requirement_coverage().require().unwrap().unassessed()
    );
}

#[test]
fn full_broader_completion_is_model_work_with_independent_errors() {
    for (text, has_errors) in [
        (br#"{"info":{},"sdfData":{"D":{}}}"#.as_slice(), false),
        (
            br#"{"info":{},"sdfData":{"D":{"unit":"urn:ietf:params:unit:Cel"}}}"#.as_slice(),
            true,
        ),
    ] {
        for terse in [false, true] {
            let catalogue = Catalogue::with_limits(
                &[SourceInput::supplied("m", text)],
                if terse {
                    Limits::default().diagnostic_bytes(1)
                } else {
                    Limits::default()
                },
            )
            .unwrap();
            let report = catalogue
                .check(CheckRequest::new().assessment(Assessment::FullBaseline))
                .unwrap();
            assert!(report.outcome().complete());
            assert_eq!(
                report
                    .requirement_coverage()
                    .require()
                    .unwrap()
                    .unassessed(),
                0
            );
            assert_eq!(
                report
                    .requirement_coverage()
                    .require()
                    .unwrap()
                    .undetermined(),
                0
            );
            for descriptor in report.requirement_descriptors().require().unwrap() {
                assert!(matches!(
                    report
                        .requirement_status(descriptor.id())
                        .require()
                        .unwrap(),
                    AssessmentStatus::Assessed | AssessmentStatus::NotApplicable
                ));
            }
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["coverage"]["broader"]["inventory"], "rfc9880.model/1");
            assert_eq!(wire["coverage"]["broader"]["inventory_status"], "available");
            assert_eq!(wire["coverage"]["broader"]["state"], "complete");
            assert!(wire["coverage"]["broader"].get("omitted_areas").is_none());
            assert_eq!(report.outcome().errors() > 0, has_errors);
            assert_eq!(
                report.outcome().disposition(),
                if has_errors {
                    esdieff::Disposition::Failed
                } else {
                    esdieff::Disposition::Passed
                }
            );
            assert!(report.to_human().contains("Broader RFC coverage: complete"));
            if terse {
                assert!(wire["coverage"].get("requirements").is_none());
            }
        }
    }
}

#[test]
fn malformed_alternative_cannot_establish_model_completion() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "m",
        br#"{"info":{},"sdfData":{"D":{"sdfChoice":{"bad":42}}}}"#,
    )])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_eq!(
        report
            .requirement_status("rfc9880.model.M016/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
    assert_eq!(
        report
            .assessment_area_status("advisory-four/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
    assert!(!report.outcome().complete());
    assert!(report.outcome().incomplete_checks() > 0);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["broader"]["state"], "partial");
    assert!(wire["coverage"]["broader"].get("omitted_areas").is_none());
}

#[test]
fn broader_state_preserves_strict_warning_and_other_operation_meanings() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("m", br#"{"sdfData":{"D":{}}}"#)]).unwrap();
    for strict in [false, true] {
        let report = catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::FullBaseline)
                    .strict_warnings(strict),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(report.outcome().warnings(), 1);
        assert_eq!(
            report.outcome().disposition(),
            if strict {
                esdieff::Disposition::Failed
            } else {
                esdieff::Disposition::Passed
            }
        );
        assert!(report.outcome().complete());
        assert_eq!(
            report
                .requirement_coverage()
                .require()
                .unwrap()
                .unassessed(),
            0
        );
        assert_eq!(
            report
                .requirement_coverage()
                .require()
                .unwrap()
                .undetermined(),
            0
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["broader"]["state"], "complete");
        let diagnostics = report.diagnostics();
        let warning = diagnostics
            .conditions()
            .find(|condition| condition.code() == "metadata.info_absent")
            .unwrap();
        assert_eq!(warning.severity(), esdieff::Severity::Warning);
        assert_eq!(warning.basis().kind(), esdieff::BasisKind::Advice);
    }
    for profile in [
        Assessment::Initial,
        Assessment::ExpandedModified,
        Assessment::ExpandedUnits,
        Assessment::ExpandedContentFormat,
        Assessment::ExpandedPatternSyntax,
    ] {
        let report = catalogue
            .check(CheckRequest::new().assessment(profile))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["broader"]["state"], "partial");
        assert_eq!(
            wire["coverage"]["broader"]["inventory_status"],
            "unavailable"
        );
        assert!(wire["coverage"]["broader"]["inventory"].is_null());
        assert!(wire["coverage"]["broader"]["omitted_areas"].is_array());
        assert!(report.to_human().contains("Broader RFC coverage: partial"));
    }
    let report = catalogue
        .inspect(esdieff::InspectRequest::new(
            catalogue.source_id("m").unwrap(),
            "",
        ))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["broader"]["state"], "partial");
    assert_eq!(
        wire["coverage"]["broader"]["inventory_status"],
        "unavailable"
    );
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", br#"{"info":{},"sdfData":{"D":{"type":"number"}}}"#),
        SourceInput::supplied("s", b"1").sample(),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            esdieff::SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                esdieff::SampleKind::Data,
                "/sdfData/D",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["broader"]["state"], "partial");
    assert_eq!(
        wire["coverage"]["broader"]["inventory_status"],
        "unavailable"
    );
}

#[test]
fn full_missing_capabilities_survive_wholly_omitted_detail() {
    for (text, requirement, status) in [
        (
            br#"{"info":{},"sdfData":{"D":{"sdfType":"ext-type"}}}"#.as_slice(),
            "rfc9880.model.M025/1",
            AssessmentStatus::Unassessed,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"contentFormat":"application/json;p=x"}}}"#.as_slice(),
            "rfc9880.model.M024/1",
            AssessmentStatus::Unassessed,
        ),
    ] {
        for terse in [false, true] {
            let catalogue = Catalogue::with_limits(
                &[SourceInput::supplied("m", text)],
                if terse {
                    Limits::default().diagnostic_bytes(1)
                } else {
                    Limits::default()
                },
            )
            .unwrap();
            let report = catalogue
                .check(CheckRequest::new().assessment(Assessment::FullBaseline))
                .unwrap();
            assert_eq!(
                report.requirement_status(requirement).require().unwrap(),
                status
            );
            assert!(!report.outcome().complete());
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["coverage"]["broader"]["state"], "partial");
            assert!(wire["coverage"]["broader"].get("omitted_areas").is_none());
            if terse {
                assert!(wire["coverage"].get("requirements").is_none());
            }
        }
    }
}

#[test]
fn copied_required_identity_leaves_unit_and_content_format_to_the_composed_content() {
    use AssessmentStatus::{Assessed, NotApplicable};
    // A surviving copied pointer relocates to the importer's copy under ADR 0036:
    // Copied reference relocation, so copying and requiredness complete. An
    // sdfRequired quality still never holds a unit or contentFormat.
    let status = |report: &esdieff::Report<'_>, member: u32| {
        report
            .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
            .require()
            .unwrap()
    };
    for (properties, expected) in [
        (r#"{"p":{"type":"boolean"}}"#, NotApplicable),
        (
            r#"{"p":{"type":"number","unit":"s"},"t":{"type":"string","contentFormat":"application/json"}}"#,
            Assessed,
        ),
    ] {
        for required in [false, true] {
            let required = if required {
                r##","sdfRequired":["#/sdfObject/Base/sdfProperty/p"]"##
            } else {
                ""
            };
            let text = format!(
                r##"{{"info":{{}},"sdfObject":{{"Base":{{"sdfProperty":{properties}{required}}},"Use":{{"sdfRef":"#/sdfObject/Base"}}}}}}"##
            );
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("m", text.as_bytes())]).unwrap();
            let report = catalogue
                .check(CheckRequest::new().assessment(Assessment::FullBaseline))
                .unwrap();
            assert_eq!(report.outcome().errors(), 0, "{text}");
            for member in [23, 24] {
                assert_eq!(status(&report, member), expected, "M{member:03} in {text}");
            }
            for member in [19, 20] {
                assert_eq!(status(&report, member), Assessed, "M{member:03} in {text}");
            }
            for (assessment, areas) in [
                (
                    Assessment::ExpandedUnits,
                    ["unit-spelling/1", "unit-registry-guidance/1"],
                ),
                (
                    Assessment::ExpandedContentFormat,
                    ["content-format-syntax/1", "content-format-registry/1"],
                ),
            ] {
                let report = catalogue
                    .check(CheckRequest::new().assessment(assessment))
                    .unwrap();
                for area in areas {
                    assert_eq!(
                        report.assessment_area_status(area).require().unwrap(),
                        expected,
                        "{area} in {text}"
                    );
                }
            }
        }
    }
}
