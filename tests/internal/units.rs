//! Observable unit spelling and recorded evidence are independent obligations.
//! Follows ADR 0027: Unit names and recorded registry guidance.
use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, SourceInput};

fn assert_unit_inventory_aggregates(report: &esdieff::Report<'_>) {
    let mut counts = [0u32; 4];
    for id in [
        "document-envelope/1",
        "base-roles-names/1",
        "metadata-namespaces/1",
        "established-composition/1",
        "basic-parameters/1",
        "alternatives-requiredness/1",
        "extensions-policies/1",
        "advisory-four/1",
        "unit-spelling/1",
        "unit-registry-guidance/1",
    ] {
        let status = report.assessment_area_status(id).require().unwrap();
        counts[match status {
            AssessmentStatus::Assessed => 0,
            AssessmentStatus::NotApplicable => 1,
            AssessmentStatus::Unassessed => 2,
            AssessmentStatus::Undetermined => 3,
            _ => panic!("unexpected assessment status"),
        }] += 1;
    }
    let coverage = report.requested_coverage();
    assert_eq!(coverage.total(), Some(10));
    assert_eq!(
        [
            coverage.assessed(),
            coverage.not_applicable(),
            coverage.unassessed(),
            coverage.undetermined(),
        ],
        counts
    );
    assert_eq!(report.outcome().incomplete_checks(), counts[2] + counts[3]);
    assert_eq!(report.outcome().complete(), counts[2] + counts[3] == 0);
}
#[test]
fn forbidden_urn_has_independent_error_and_positive_represented_membership() {
    let bytes =
        br#"{"sdfData":{"Temperature":{"type":"number","unit":"urn:ietf:params:unit:Cel"}}}"#;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 1);
    assert_eq!(report.unit_guidance().require().unwrap().uri_names(), 1);
    assert!(matches!(
        report.assessment_area_status("unit-spelling/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(matches!(
        report.assessment_area_status("unit-registry-guidance/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert_unit_inventory_aggregates(&report);
}

fn check_unit(text: &str) -> (u32, [u32; 6], bool, serde_json::Value) {
    let source = serde_json::json!({"info":{},"sdfData":{"Value":{"unit":text}}}).to_string();
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", source.as_bytes())]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    let units = report.unit_guidance().require().unwrap();
    (
        report.outcome().errors(),
        [
            units.known_primary(),
            units.known_secondary(),
            units.unknown(),
            units.unavailable(),
            units.legacy(),
            units.uri_names(),
        ],
        units.complete(),
        serde_json::from_str(&report.to_json()).unwrap(),
    )
}
#[test]
fn exact_spelling_exception_percent_mapping_suffixes_and_distinct_uncertainty() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/units/spelling-cases.json")).unwrap();
    for row in fixtures.as_array().unwrap() {
        let unit = row["unit"].as_str().unwrap();
        let (errors, counts, complete, wire) = check_unit(unit);
        let spelling = row["spelling"].as_str().unwrap();
        let code = match spelling {
            "urn_forbidden" => Some("unit.urn_forbidden"),
            "uri_syntax_error" => Some("unit.uri_syntax"),
            "unit_index_spelling_error" => Some("unit.index_syntax"),
            "index_meaning_unavailable" | "assigned_symbol_unavailable" => {
                Some("unit.index_meaning_unavailable")
            }
            "interpretation_unresolved" => Some(if unit.contains('%') {
                "unit.marker_encoding_unresolved"
            } else {
                "unit.marker_case_unresolved"
            }),
            _ => None,
        };
        assert_eq!(
            errors,
            u32::from(matches!(
                spelling,
                "urn_forbidden" | "uri_syntax_error" | "unit_index_spelling_error"
            )),
            "{unit}"
        );
        assert_eq!(
            complete,
            !matches!(
                spelling,
                "index_meaning_unavailable"
                    | "assigned_symbol_unavailable"
                    | "interpretation_unresolved"
            ),
            "{unit}"
        );
        if let Some(code) = code {
            assert!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["code"] == code),
                "{unit}: {wire}"
            );
        }
        if spelling == "colon_exception" {
            assert_eq!(counts, [0, 0, 1, 0, 0, 1], "{unit}");
        }
        if spelling == "generic_uri" {
            assert_eq!(counts, [0, 0, 0, 0, 0, 1], "{unit}");
            assert!(
                wire["baseline"]["registry_knowledge"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        if spelling == "unit_index_spelling_error" {
            assert_eq!(counts, [0, 0, 0, 0, 0, 1]);
        }
        if spelling == "interpretation_unresolved" {
            assert_eq!(counts, [0, 0, 0, 0, 0, 1]);
        }
    }
}
#[test]
fn whole_recorded_positive_tables_retain_exact_names_secondary_and_legacy_flags() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/units/spelling-cases.json")).unwrap();
    for text in ["kg", "Cel", "ms", "/100", "g", "%", "cel", " kg ", "%6Bg"] {
        let (errors, counts, complete, wire) = check_unit(text);
        assert_eq!(errors, 0);
        assert!(complete);
        let expected = match text {
            "kg" | "Cel" => [1, 0, 0, 0, 0, 0],
            "g" | "%" => [1, 0, 0, 0, 1, 0],
            "ms" | "/100" => [0, 1, 0, 0, 0, 0],
            _ => [0, 0, 1, 0, 0, 0],
        };
        assert_eq!(counts, expected, "{text}");
        let records = wire["baseline"]["registry_knowledge"].as_array().unwrap();
        assert_eq!(records.len(), if expected[0] == 1 { 1 } else { 2 });
        assert_eq!(records[0]["entries_count"], 67);
        assert_eq!(records[0]["state"], "available");
    }
    assert_eq!(fixtures.as_array().unwrap().len(), 28);
}

#[test]
fn all_full_roles_and_refined_alternatives_have_distinct_final_coordinates() {
    let bytes = include_bytes!("../fixtures/units/full-positions.sdf.json");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("fixture", bytes)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    let units = report.unit_guidance().require().unwrap();
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(
        (
            units.known_primary(),
            units.known_secondary(),
            units.unknown(),
            units.unavailable(),
            units.uri_names()
        ),
        (12, 0, 0, 0, 2)
    );
    assert!(units.complete());
    assert_unit_inventory_aggregates(&report);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let mut paths: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["code"] == "unit.urn_forbidden")
        .map(|row| row["primary"]["pointer"].as_str().unwrap())
        .collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "/sdfData/ObjectItems/items/properties/Mass/unit",
            "/sdfData/Root/properties/Mass/unit"
        ]
    );
}
#[test]
fn withheld_request_retains_spelling_error_and_real_unavailable_needed_record() {
    use esdieff::RecordedRegistryPolicy;
    let bytes = include_bytes!("../fixtures/units/forbidden-one.sdf.json");
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let request = CheckRequest::new()
        .assessment(Assessment::ExpandedUnits)
        .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded);
    let report = cat.check(request).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let summary = report.unit_guidance().require().unwrap();
    assert_eq!(
        (
            summary.known_primary(),
            summary.unknown(),
            summary.unavailable(),
            summary.uri_names()
        ),
        (0, 0, 1, 1)
    );
    assert!(!summary.complete());
    assert!(matches!(summary.source_edition(), Field::Unavailable(_)));
    let record = report
        .registry_knowledge("iana.senml-units")
        .require()
        .unwrap();
    assert_eq!(
        (record.state(), record.reason(), record.snapshot()),
        ("unavailable", Some("caller_withheld"), None)
    );
    assert!(matches!(
        report.registry_knowledge("iana.secondary-units"),
        Field::Absent
    ));
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["policy"]["recorded_registry"], "withhold_recorded");
    assert!(wire["request"].get("registry_knowledge").is_none());
    assert_eq!(
        wire["baseline"]["registry_knowledge"][0]
            .as_object()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(wire["result"]["unit_guidance"]["counts"]["unavailable"], 1);
    assert_unit_inventory_aggregates(&report);
}
#[test]
fn retained_sources_effective_overrides_deletions_and_copied_coordinates_count_once() {
    let bytes = include_bytes!("../fixtures/units/local-overrides.sdf.json");
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert_eq!(cat.sources().next().unwrap().raw_bytes(), bytes);
    let units = report.unit_guidance().require().unwrap();
    assert_eq!(units.known_primary(), 2);
    assert_eq!(report.outcome().errors(), 1);
}
#[test]
fn initial_modified_and_full_inventory_getter_meanings_are_preserved() {
    let cat = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br#"{"sdfData":{"A":{"unit":"urn:ietf:params:unit:kg"}}}"#,
    )])
    .unwrap();
    for assessment in [Assessment::Initial, Assessment::ExpandedModified] {
        let report = cat
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        assert!(matches!(report.unit_guidance(), Field::Absent));
        assert!(matches!(
            report.assessment_area_status("unit-spelling/1"),
            Field::Absent
        ));
        assert_eq!(report.outcome().errors(), 0);
        assert!(
            serde_json::from_str::<serde_json::Value>(&report.to_json()).unwrap()["policy"]
                .get("recorded_registry")
                .is_none()
        );
    }
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    for id in ["unit-spelling/1", "document-envelope/1", "never-known"] {
        assert!(matches!(report.assessment_area_status(id), Field::Absent));
    }
    assert_eq!(report.requested_coverage().total(), Some(13));
}
#[test]
fn reliable_absence_shape_direct_and_critical_guards_are_distinct() {
    for bytes in [
        b"{}".as_slice(),
        include_bytes!("../fixtures/units/direct-items.sdf.json"),
    ] {
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        let units = report.unit_guidance().require().unwrap();
        if bytes == b"{}" {
            assert!(units.complete());
        }
        let explicit_full = u32::from(bytes != b"{}");
        assert_eq!(units.known_primary(), explicit_full);
        assert_eq!(units.uri_names(), explicit_full);
        if bytes == b"{}" {
            assert!(matches!(units.source_edition(), Field::Absent));
        }
    }
    let bytes = include_bytes!("../fixtures/units/critical.sdf.json");
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert!(!report.unit_guidance().require().unwrap().complete());
    assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 0);
    assert!(matches!(
        report.registry_knowledge("iana.senml-units"),
        Field::Absent
    ));
    let bytes = br#"{"sdfData":{"A":{"unit":false}}}"#;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let units = report.unit_guidance().require().unwrap();
    assert!(units.complete());
    assert_eq!(units.unavailable(), 0);
}

#[test]
fn exact_report_admission_preserves_mandatory_counts_with_all_optional_detail_omitted() {
    use esdieff::Limits;
    let bytes = br#"{"info":{},"sdfData":{"A":{"unit":"urn:ietf:params:unit:Cel"}}}"#;
    for limit in [41969, 41970] {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("fixture", bytes)],
            Limits::default()
                .report_bytes(limit)
                .diagnostic_bytes(1)
                .conditions(1),
        )
        .unwrap();
        let result = cat.check(CheckRequest::new().assessment(Assessment::ExpandedUnits));
        if limit == 41969 {
            assert!(result.is_err());
            continue;
        }
        let report = result.unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 1);
        let json = report.to_json();
        let human = report.to_human();
        assert!(json.len() <= limit && human.len() <= limit);
        let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["coverage"]["rows_omitted"], 10);
        assert_eq!(wire["diagnostics"]["retained"], 0);
        assert_eq!(
            wire["result"]["unit_guidance"]["counts"]["known_primary"],
            1
        );
        assert_eq!(
            wire["coverage"]["requested"]["assessed"],
            report.requested_coverage().assessed()
        );
        assert_unit_inventory_aggregates(&report);
        assert!(human.contains("known_primary 1") && human.contains("unit-spelling/1: assessed"));
        println!(
            "exact report admission {limit}: JSON {}, human {}; optional rows10/conditions0 omitted",
            json.len(),
            human.len()
        );
    }
}
#[test]
fn owner_failure_is_null_and_not_a_completed_empty_summary() {
    use esdieff::Limits;
    let cat = Catalogue::with_limits(
        &[SourceInput::supplied(
            "model",
            br#"{"sdfData":{"Bad":{"multipleOf":0}}}"#,
        )],
        Limits::default().effective_allocation_bytes(32),
    )
    .unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert!(matches!(report.unit_guidance(), Field::Unavailable(_)));
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status("unit-spelling/1"),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(matches!(
        report.registry_knowledge("iana.senml-units"),
        Field::Unavailable(_)
    ));
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["result"]["unit_guidance"]["state"], "unavailable");
    assert!(wire["result"]["unit_guidance"]["counts"].is_null());
    assert!(
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(report.to_human().contains("Unit counts: unavailable"));
}
#[test]
fn lookup_work_before_and_after_first_comparison_has_truthful_consultation() {
    use esdieff::Limits;
    let bytes = br#"{"info":{},"sdfData":{"A":{"unit":"unknown-name"}}}"#;
    let mut before = None;
    let mut after = None;
    let mut completed = None;
    for work in 1..5000 {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("model", bytes)],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        if !cat
            .sources()
            .next()
            .unwrap()
            .json()
            .present()
            .is_some_and(|json| json.capability_complete())
        {
            continue;
        }
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        let summary = report.unit_guidance().require().unwrap();
        if summary.unavailable() == 1 {
            match report.registry_knowledge("iana.senml-units") {
                Field::Absent => {
                    before.get_or_insert(work);
                    assert!(matches!(summary.source_edition(), Field::Absent));
                }
                Field::Present(record) => {
                    after.get_or_insert(work);
                    assert_eq!(record.state(), "available");
                    assert!(matches!(summary.source_edition(), Field::Present(_)));
                }
                Field::Unavailable(_) => panic!("no owner failure in this control"),
                _ => panic!("unknown field state"),
            }
        }
        if summary.unknown() == 1 {
            completed = Some(work);
            break;
        }
    }
    assert!(
        before.is_some() && after.is_some() && completed.is_some(),
        "{before:?}/{after:?}/{completed:?}"
    );
    println!(
        "lookup calibration shared work before_first={before:?} after_first={after:?} completed={completed:?}"
    );
}
#[test]
fn exact_decoded_replacements_are_optional_and_do_not_rewrite_sources_or_scale() {
    for (text, expected) in [
        ("urn:ietf:params:unit:%6Bg", "kg"),
        ("urn:ietf:params:unit:m%2Fs", "m/s"),
        ("urn:ietf:params:unit:%25", "%"),
        ("urn:ietf:params:unit:example%253Aratio", "example%3Aratio"),
        ("urn:ietf:params:unit:%C3%A9", "é"),
    ] {
        let (_, counts, _, wire) = check_unit(text);
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "unit.urn_forbidden")
            .unwrap();
        assert_eq!(finding["arguments"]["replacement"], expected);
        assert_eq!(counts[5], 1);
    }
}
#[test]
fn selected_unused_providers_reverse_order_sample_exclusion_and_inherited_origins() {
    let provider = include_bytes!("../fixtures/units/provider.sdf.json");
    let consumer = include_bytes!("../fixtures/units/consumer-inherits.sdf.json");
    for reverse in [false, true] {
        let inputs = if reverse {
            [
                SourceInput::supplied("consumer", consumer),
                SourceInput::supplied("provider", provider),
            ]
        } else {
            [
                SourceInput::supplied("provider", provider),
                SourceInput::supplied("consumer", consumer),
            ]
        };
        let cat = Catalogue::from_inputs(&inputs).unwrap();
        let root = cat
            .sources()
            .find(|source| source.key() == "consumer")
            .unwrap()
            .id();
        let report = cat
            .check(
                CheckRequest::new()
                    .root(root)
                    .assessment(Assessment::ExpandedUnits),
            )
            .unwrap();
        assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 1);
        assert_eq!(report.outcome().errors(), 1);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "unit.urn_forbidden")
            .unwrap();
        assert_eq!(finding["primary"]["kind"], "effective");
        assert_eq!(finding["primary"]["root"], root.get());
        assert_eq!(finding["related"][0]["kind"], "source");
        assert_eq!(finding["related"][0]["pointer"], "/sdfData/Mass/unit");
        let all = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        assert_eq!(all.unit_guidance().require().unwrap().known_primary(), 2);
    }
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "sample",
            br#"{"sdfData":{"A":{"unit":"urn:ietf:params:unit:kg"}}}"#,
        )
        .sample(),
        SourceInput::supplied("model", b"{}"),
    ])
    .unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 0);
}

#[test]
fn unavailable_alternative_meaning_keeps_independent_full_unit_error() {
    let bytes =
        br#"{"info":{},"sdfData":{"A":{"enum":"not-an-array","unit":"urn:ietf:params:unit:kg"}}}"#;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("fixture", bytes)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "unit.urn_forbidden")
    );
}

#[test]
fn later_work_limit_and_detail_omission_preserve_earlier_errors_and_coordinate_counts() {
    use esdieff::Limits;
    let long = "x".repeat(10000);
    let bytes = format!(
        r#"{{"info":{{}},"sdfData":{{"A":{{"unit":"urn:ietf:params:unit:kg","multipleOf":0}},"B":{{"unit":"{long}"}}}}}}"#
    );
    for detail in [1, 1024 * 1024] {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("fixture", bytes.as_bytes())],
            Limits::default()
                .decoding_work(100000)
                .diagnostic_bytes(detail),
        )
        .unwrap();
        assert!(
            cat.sources()
                .next()
                .unwrap()
                .json()
                .require()
                .unwrap()
                .capability_complete()
        );
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
            .unwrap();
        let units = report.unit_guidance().require().unwrap();
        assert_eq!(report.outcome().errors(), 2);
        assert_eq!(
            (
                units.known_primary(),
                units.uri_names(),
                units.unavailable()
            ),
            (1, 1, 0)
        );
        assert!(!units.complete());
        assert!(matches!(units.source_edition(), Field::Present(_)));
        assert_eq!(
            report
                .registry_knowledge("iana.senml-units")
                .require()
                .unwrap()
                .state(),
            "available"
        );
    }
}
#[test]
fn withheld_unused_evidence_and_unresolved_scope_do_not_invent_lookup_counts() {
    use esdieff::RecordedRegistryPolicy;
    let bytes = br#"{"info":{},"sdfData":{"A":{"unit":"https://example.test/units/length"}}}"#;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("fixture", bytes)]).unwrap();
    let report = cat
        .check(
            CheckRequest::new()
                .assessment(Assessment::ExpandedUnits)
                .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded),
        )
        .unwrap();
    let units = report.unit_guidance().require().unwrap();
    assert!(units.complete());
    assert_eq!((units.uri_names(), units.unavailable()), (1, 0));
    assert!(matches!(units.source_edition(), Field::Absent));
    assert!(matches!(
        report.registry_knowledge("iana.senml-units"),
        Field::Absent
    ));
    let bytes =
        br#"{"info":{},"sdfData":{"A":{"unit":"kg"},"B":{"sdfRef":"missing:#/sdfData/Base"}}}"#;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("fixture", bytes)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedUnits))
        .unwrap();
    let units = report.unit_guidance().require().unwrap();
    assert!(!units.complete());
    assert_eq!((units.known_primary(), units.unavailable()), (1, 0));
}
#[test]
fn explicit_library_policy_refuses_unrelated_profiles_and_full_exact_cap_is_independent() {
    use esdieff::{Limits, RecordedRegistryPolicy};
    let bytes = br#"{"info":{},"sdfData":{"A":{"unit":"kg"}}}"#;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("fixture", bytes)]).unwrap();
    for assessment in [
        Assessment::Initial,
        Assessment::ExpandedModified,
        Assessment::ExpandedPatternSyntax,
    ] {
        for policy in [
            RecordedRegistryPolicy::UseRecorded,
            RecordedRegistryPolicy::WithholdRecorded,
        ] {
            assert!(
                cat.check(
                    CheckRequest::new()
                        .assessment(assessment)
                        .recorded_registry_policy(policy)
                )
                .is_err()
            );
        }
    }
    for cap in [52140, 52141] {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("fixture", bytes)],
            Limits::default().report_bytes(cap),
        )
        .unwrap();
        let result = cat.check(CheckRequest::new().assessment(Assessment::FullBaseline));
        if cap == 52140 {
            assert!(result.is_err());
            continue;
        }
        let report = result.unwrap();
        assert!(report.to_json().len() <= cap && report.to_human().len() <= cap);
        assert!(matches!(
            report.assessment_area_status("unit-spelling/1"),
            Field::Absent
        ));
        assert_eq!(report.unit_guidance().require().unwrap().known_primary(), 1);
    }
}
#[test]
fn suffix_components_keep_pchar_start_and_encoded_marker_boundaries_separate() {
    for unit in [
        "urn:ietf:params:unit:kg?=q?=more",
        "urn:ietf:params:unit:kg?+r?=q?+more",
    ] {
        let (errors, counts, complete, _) = check_unit(unit);
        assert_eq!(errors, 1);
        assert_eq!(counts[0], 1);
        assert!(complete);
    }
    for unit in ["urn:ietf:params%3Aunit:kg", "urn:ietf:%70arams:unit:kg"] {
        let (errors, counts, complete, wire) = check_unit(unit);
        assert_eq!(errors, 0);
        assert_eq!(counts, [0, 0, 0, 0, 0, 1]);
        assert!(!complete);
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "unit.marker_encoding_unresolved")
        );
    }
}
