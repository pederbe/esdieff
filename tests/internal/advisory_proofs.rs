//! Bounded advice uses effective constraints without claiming general satisfiability.
//! Follows ADR 0003: Check findings and completeness, and ADR 0013: Shared workflow and report guarantees.
use super::coverage_support;
use esdieff::{AssessmentStatus, BasisKind, Catalogue, CheckRequest, Field, Severity, SourceInput};

#[test]
fn proof_basis_omits_absent_citations_and_retains_warning_citations() {
    type Case<'a> = (&'a [u8], &'a str, &'a str, &'a [(usize, &'a str)]);
    let cases: [Case<'_>; 4] = [
        (
            br#"{"info":{},"sdfData":{"value":{"type":"number","nullable":false,"minimum":2,"maximum":1}}}"#,
            "proof.empty_numeric_interval",
            "RFC 9880 Appendix C.1",
            &[(2, "compare")],
        ),
        (
            br#"{"info":{},"sdfData":{"value":{"type":"string","nullable":false,"minLength":2,"maxLength":1}}}"#,
            "proof.inverted_text_lengths",
            "RFC 9880 Appendix C.2",
            &[(3, "integrality"), (4, "compare")],
        ),
        (
            br#"{"info":{},"sdfData":{"value":{"type":"array","nullable":false,"minItems":2,"maxItems":1}}}"#,
            "proof.inverted_array_counts",
            "RFC 9880 Appendix C.4",
            &[(3, "integrality"), (4, "compare")],
        ),
        (
            br#"{"info":{},"sdfData":{"value":{"type":"integer","const":0.5}}}"#,
            "proof.primitive_const_type",
            "RFC 9880 Appendix C; Section 4.7",
            &[(3, "integrality")],
        ),
    ];
    let mut unexpected_citations = Vec::new();
    for (bytes, code, citation, failures) in cases {
        let catalogue =
            Catalogue::take_inputs(vec![SourceInput::supplied("model", bytes)]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(), 1);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let warning = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|condition| condition["code"] == code)
            .unwrap();
        assert_eq!(warning["basis"]["kind"], "advice");
        assert_eq!(warning["basis"]["id"], code);
        assert_eq!(warning["basis"]["citation"].as_str(), Some(citation));

        for &(work, operation) in failures {
            let catalogue = Catalogue::take_inputs_with_limits(
                vec![SourceInput::supplied("model", bytes)],
                esdieff::Limits::default().numeric_work(work),
            )
            .unwrap();
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(report.outcome().warnings(), 0);
            coverage_support::assert_requested_counts(&report);
            let limits = numeric_limits(&report);
            assert_eq!(limits.len(), 1);
            assert_eq!(limits[0]["arguments"]["proof_class"], code);
            assert_eq!(limits[0]["arguments"]["operation"], operation);
            assert_eq!(limits[0]["basis"]["kind"], "operational");
            assert_eq!(limits[0]["basis"]["id"], "advisory-numeric-work");
            if limits[0]["basis"]
                .as_object()
                .unwrap()
                .contains_key("citation")
            {
                unexpected_citations.push((code, operation));
            }
        }
    }
    assert!(
        unexpected_citations.is_empty(),
        "absent optional citations must be omitted: {unexpected_citations:?}"
    );
}

#[test]
fn exclusive_upper_equality_explains_an_empty_numeric_interval() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"number","nullable":false,"minimum":2,"exclusiveMaximum":2}}}"#;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("model", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 1);
    coverage_support::assert_requested_counts(&report);
    assert!(matches!(
        report.assessment_area_status("advisory-four/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    let diagnostics = report.diagnostics();
    let proof = diagnostics
        .conditions()
        .find(|condition| condition.code() == "proof.empty_numeric_interval")
        .unwrap();
    assert_eq!(proof.severity(), Severity::Warning);
    assert_eq!(proof.basis().kind(), BasisKind::Advice);
    assert_eq!(proof.assessment_unit(), Some("advisory-four/1"));
    assert_eq!(catalogue.sources().next().unwrap().raw_bytes(), bytes);
}

#[test]
fn known_string_formats_preserve_inverted_unicode_scalar_lengths() {
    let bytes = br#"{"info":{},"sdfData":{
        "plain":{"type":"string","nullable":false,"minLength":3,"maxLength":2},
        "uri":{"type":"string","format":"uri","nullable":false,"minLength":3,"maxLength":2},
        "reference":{"type":"string","format":"uri-reference","nullable":false,"minLength":3,"maxLength":2},
        "uuid":{"type":"string","format":"uuid","nullable":false,"minLength":3,"maxLength":2},
        "date":{"type":"string","format":"date","nullable":false,"minLength":3,"maxLength":2},
        "time":{"type":"string","format":"time","nullable":false,"minLength":3,"maxLength":2},
        "datetime":{"type":"string","format":"date-time","nullable":false,"minLength":3,"maxLength":2},
        "equal":{"type":"string","nullable":false,"minLength":2.0,"maxLength":2},
        "nullable":{"type":"string","minLength":3,"maxLength":2}
    }}"#;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("strings", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 7);
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "proof.inverted_text_lengths")
            .count(),
        7
    );
    assert!(matches!(
        report.assessment_area_status("advisory-four/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
}

#[test]
fn array_count_proofs_do_not_interpret_items_or_uniqueness() {
    let bytes = br#"{"info":{},"sdfData":{
        "bad":{"type":"array","nullable":false,"minItems":3,"maxItems":2,"items":{"type":"integer"},"uniqueItems":true},
        "equal":{"type":"array","nullable":false,"minItems":2.0,"maxItems":2},
        "nullable":{"type":"array","minItems":3,"maxItems":2},
        "group":{"type":"object","nullable":false,"minItems":3,"maxItems":2}
    }}"#;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("counts", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 1);
    let diagnostics = report.diagnostics();
    let proof = diagnostics
        .conditions()
        .find(|condition| condition.code() == "proof.inverted_array_counts")
        .unwrap();
    assert_eq!(proof.basis().citation(), Some("RFC 9880 Appendix C.4"));
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "proof.inverted_array_counts")
            .count(),
        1
    );
    assert!(matches!(
        report.assessment_area_status("advisory-four/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
}

#[test]
fn primitive_constants_use_mathematical_integrality_and_independent_null_admission() {
    let bytes = br#"{"info":{},"sdfData":{
        "fraction":{"type":"integer","nullable":"unknown","const":0.5},
        "text":{"type":"string","const":false},
        "boolean":{"type":"boolean","const":""},
        "number":{"type":"number","const":true},
        "null":{"type":"string","nullable":false,"format":"date","const":null},
        "ten":{"type":"integer","const":10.0},
        "hundred":{"type":"integer","const":1e2},
        "nullable":{"type":"integer","const":null},
        "complex":{"type":"integer","const":[0.5]},
        "map":{"type":"object","const":false}
    }}"#;
    let catalogue =
        Catalogue::take_inputs(vec![SourceInput::supplied("constants", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 5);
    let diagnostics = report.diagnostics();
    let proof = diagnostics
        .conditions()
        .find(|condition| condition.code() == "proof.primitive_const_type")
        .unwrap();
    assert_eq!(
        proof.basis().citation(),
        Some("RFC 9880 Appendix C; Section 4.7")
    );
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "proof.primitive_const_type")
            .count(),
        5
    );
    assert!(matches!(
        report.assessment_area_status("advisory-four/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    let value: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let fraction = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|condition| {
            condition["primary"]["pointer"] == "/sdfData/fraction"
                && condition["code"] == "proof.primitive_const_type"
        })
        .unwrap();
    assert_eq!(fraction["arguments"]["const"], "0.5");
    assert!(
        fraction["related"]
            .as_array()
            .unwrap()
            .iter()
            .all(|location| location["pointer"] != "/sdfData/fraction/nullable")
    );
}

fn status(report: &esdieff::Report<'_>) -> AssessmentStatus {
    match report.assessment_area_status("advisory-four/1") {
        Field::Present(status) => status,
        _ => panic!("known initial area"),
    }
}
fn conditions(report: &esdieff::Report<'_>) -> Vec<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|condition| condition["code"].as_str().unwrap().starts_with("proof."))
        .cloned()
        .collect()
}
#[test]
fn all_present_numeric_pairs_share_the_acquisition_remainder() {
    let bytes = include_bytes!("../fixtures/advisory/shared-pairs.json");
    for (limit, warnings, expected) in [
        (12, 4, AssessmentStatus::Assessed),
        (6, 1, AssessmentStatus::Undetermined),
    ] {
        let catalogue = Catalogue::take_inputs_with_limits(
            vec![SourceInput::supplied("pairs", bytes)],
            esdieff::Limits::default().numeric_work(limit),
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(), warnings);
        assert_eq!(status(&report), expected);
        assert_eq!(
            report.to_json(),
            catalogue.check(CheckRequest::new()).unwrap().to_json(),
            "a new check seeds the same immutable remainder"
        );
        if warnings == 4 {
            let pairs: Vec<_> = conditions(&report)
                .into_iter()
                .map(|condition| {
                    (
                        condition["arguments"]["minimum"]
                            .as_str()
                            .map(str::to_owned),
                        condition["arguments"]["exclusiveMinimum"]
                            .as_str()
                            .map(str::to_owned),
                        condition["arguments"]["maximum"]
                            .as_str()
                            .map(str::to_owned),
                        condition["arguments"]["exclusiveMaximum"]
                            .as_str()
                            .map(str::to_owned),
                    )
                })
                .collect();
            assert_eq!(
                pairs,
                vec![
                    (Some("2".into()), None, Some("1".into()), None),
                    (Some("2".into()), None, None, Some("2".into())),
                    (None, Some("3".into()), Some("1".into()), None),
                    (None, Some("3".into()), None, Some("2".into()))
                ]
            );
        }
    }
}

#[test]
fn nullable_exclusions_do_not_make_guarded_classes_applicable() {
    for (primitive, constraints) in [
        ("number", "\"minimum\":2,\"maximum\":1"),
        ("integer", "\"minimum\":2,\"exclusiveMaximum\":2"),
        (
            "string",
            "\"minLength\":2,\"maxLength\":1,\"format\":\"uri\"",
        ),
        ("array", "\"minItems\":2,\"maxItems\":1"),
    ] {
        for nullable in ["", ",\"nullable\":true"] {
            let bytes = format!(
                "{{\"info\":{{}},\"sdfData\":{{\"candidate\":{{\"type\":\"{primitive}\",{constraints}{nullable}}}}}}}"
            );
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("excluded", bytes.as_bytes())])
                    .unwrap();
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(
                status(&report),
                AssessmentStatus::NotApplicable,
                "{primitive}: {nullable}"
            );
            assert_eq!(report.outcome().warnings(), 0);
            coverage_support::assert_requested_counts(&report);
        }
    }
    for (nullable, expected) in [
        ("false", AssessmentStatus::Assessed),
        ("\"unknown\"", AssessmentStatus::Undetermined),
    ] {
        let bytes = format!(
            "{{\"info\":{{}},\"sdfData\":{{\"ordered\":{{\"type\":\"number\",\"minimum\":1,\"maximum\":2,\"nullable\":{nullable}}}}}}}"
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("ordered", bytes.as_bytes())]).unwrap();
        assert_eq!(
            status(&catalogue.check(CheckRequest::new()).unwrap()),
            expected
        );
    }
    let bytes = br#"{"info":{},"sdfData":{"excluded":{"type":"number","nullable":true,"minimum":2,"maximum":1},"independent":{"type":"boolean","nullable":true,"const":""}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("mixed", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(status(&report), AssessmentStatus::Assessed);
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(conditions(&report)[0]["code"], "proof.primitive_const_type");
}

fn numeric_limits(report: &esdieff::Report<'_>) -> Vec<serde_json::Value> {
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            row["code"] == "operation.limit" && row["assessment_unit"] == "advisory-four/1"
        })
        .cloned()
        .collect()
}

#[test]
fn numeric_limit_witnesses_identify_pairs_without_multiplying_incomplete_units() {
    let bytes = include_bytes!("../fixtures/advisory/shared-pairs.json");
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![SourceInput::supplied("pairs", bytes)],
        esdieff::Limits::default().numeric_work(6),
    )
    .unwrap();
    for assessment in [
        esdieff::Assessment::Initial,
        esdieff::Assessment::ExpandedModified,
        esdieff::Assessment::FullBaseline,
    ] {
        let report = catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        assert_eq!(report.outcome().warnings(), 1);
        coverage_support::assert_requested_counts(&report);
        let limits = numeric_limits(&report);
        assert_eq!(limits.len(), 3);
        for (condition, expected) in limits.iter().zip([
            ["minimum", "exclusiveMaximum"],
            ["exclusiveMinimum", "maximum"],
            ["exclusiveMinimum", "exclusiveMaximum"],
        ]) {
            assert_eq!(condition["severity"], "incomplete");
            assert_eq!(condition["basis"]["kind"], "operational");
            assert_eq!(condition["basis"]["id"], "advisory-numeric-work");
            assert_eq!(
                condition["arguments"]["proof_class"],
                "proof.empty_numeric_interval"
            );
            assert_eq!(condition["arguments"]["operation"], "compare");
            assert_eq!(
                condition["arguments"]["qualities"],
                serde_json::json!(expected)
            );
            assert_eq!(condition["arguments"]["type"], "number");
            assert_eq!(condition["primary"]["kind"], "effective");
            assert_eq!(condition["primary"]["pointer"], "/sdfData/value");
            assert!(
                condition["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["kind"] == "source"
                        && location["pointer"]
                            .as_str()
                            .unwrap()
                            .starts_with("/sdfData/value/"))
            );
        }
        if assessment == esdieff::Assessment::FullBaseline {
            assert_eq!(report.requested_coverage().total(), Some(13));
            assert!(matches!(
                report.assessment_area_status("advisory-four/1"),
                Field::Present(AssessmentStatus::Undetermined)
            ));
        } else {
            assert_eq!(status(&report), AssessmentStatus::Undetermined);
        }
    }
}

#[test]
fn failed_integrality_and_count_comparison_have_distinct_numeric_operations() {
    for (primitive, low, high, class) in [
        (
            "string",
            "minLength",
            "maxLength",
            "proof.inverted_text_lengths",
        ),
        (
            "array",
            "minItems",
            "maxItems",
            "proof.inverted_array_counts",
        ),
    ] {
        let bytes = format!(
            "{{\"info\":{{}},\"sdfData\":{{\"count\":{{\"type\":\"{primitive}\",\"nullable\":false,\"{low}\":2,\"{high}\":1}}}}}}"
        );
        for (work, operation) in [(3, "integrality"), (4, "compare")] {
            let catalogue = Catalogue::take_inputs_with_limits(
                vec![SourceInput::supplied("count", bytes.as_bytes())],
                esdieff::Limits::default().numeric_work(work),
            )
            .unwrap();
            let report = catalogue.check(CheckRequest::new()).unwrap();
            let limits = numeric_limits(&report);
            assert_eq!(limits.len(), 1);
            assert_eq!(limits[0]["arguments"]["proof_class"], class);
            assert_eq!(limits[0]["arguments"]["operation"], operation);
            assert_eq!(
                limits[0]["arguments"]["qualities"],
                serde_json::json!([low, high])
            );
            coverage_support::assert_requested_counts(&report);
        }
    }
    let bytes = br#"{"info":{},"sdfData":{"fraction":{"type":"integer","const":0.5}}}"#;
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![SourceInput::supplied("const", bytes)],
        esdieff::Limits::default().numeric_work(3),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let limits = numeric_limits(&report);
    assert_eq!(limits.len(), 1);
    assert_eq!(
        limits[0]["arguments"]["proof_class"],
        "proof.primitive_const_type"
    );
    assert_eq!(limits[0]["arguments"]["operation"], "integrality");
    assert_eq!(
        limits[0]["arguments"]["qualities"],
        serde_json::json!(["const"])
    );
    assert_eq!(report.outcome().warnings(), 0);
    coverage_support::assert_requested_counts(&report);
    let unavailable = br#"{"info":{},"sdfData":{"integer":{"type":"integer","const":12}}}"#;
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![SourceInput::supplied("unavailable", unavailable)],
        esdieff::Limits::default()
            .numeric_work(2)
            .coefficient_digits(1),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
    assert!(
        numeric_limits(&report).is_empty(),
        "unavailable mathematical values are not failed derived numeric operations"
    );
}

#[test]
fn numeric_limits_keep_inherited_origins_and_aggregate_outcomes_under_omission() {
    let inherited = br##"{"info":{},"sdfData":{"Base":{"type":"number","nullable":false,"minimum":2,"maximum":1,"exclusiveMinimum":3,"exclusiveMaximum":2},"Copy":{"sdfRef":"#/sdfData/Base"}}}"##;
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![SourceInput::supplied("inherited", inherited)],
        esdieff::Limits::default().numeric_work(6),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let limits = numeric_limits(&report);
    assert_eq!(limits.len(), 7);
    let copy = limits
        .iter()
        .find(|row| row["primary"]["pointer"] == "/sdfData/Copy")
        .unwrap();
    let related = copy["related"].as_array().unwrap();
    assert!(
        related
            .iter()
            .any(|row| row["kind"] == "source" && row["pointer"] == "/sdfData/Base/minimum")
    );
    assert!(
        related
            .iter()
            .any(|row| row["kind"] == "source" && row["pointer"] == "/sdfData/Copy/sdfRef")
    );
    coverage_support::assert_requested_counts(&report);

    let mixed = br#"{"info":{},"sdfData":{"early":{"type":"boolean","const":""},"pairs":{"type":"number","nullable":false,"minimum":2,"maximum":1,"exclusiveMinimum":3,"exclusiveMaximum":2},"late":{"type":"boolean","const":""}}}"#;
    for limits in [
        esdieff::Limits::default(),
        esdieff::Limits::default().conditions(1),
        esdieff::Limits::default().diagnostic_bytes(1),
        esdieff::Limits::default().pointer_bytes(1),
        esdieff::Limits::default().pointer_output_bytes(1),
        esdieff::Limits::default().report_bytes(22000),
    ] {
        let catalogue = Catalogue::take_inputs_with_limits(
            vec![SourceInput::supplied("mixed", mixed)],
            limits.numeric_work(6),
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(), 3);
        coverage_support::assert_requested_counts(&report);
        assert_eq!(status(&report), AssessmentStatus::Undetermined);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["outcome"]["warnings"], 3);
        assert_eq!(
            wire["diagnostics"]["retained"].as_u64().unwrap()
                + wire["diagnostics"]["omitted"].as_u64().unwrap(),
            7
        );
        assert!(report.to_human().contains("warnings: 3"));
    }
}
#[test]
fn shared_numeric_exhaustion_preserves_independent_wrong_kind_constants() {
    let first = include_bytes!("../fixtures/advisory/budget-first.json");
    let second = include_bytes!("../fixtures/advisory/budget-second.json");
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![
            SourceInput::supplied("first", first),
            SourceInput::supplied("second", second),
        ],
        esdieff::Limits::default().numeric_work(6),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 2);
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
    assert_eq!(
        conditions(&report)
            .iter()
            .filter(|condition| condition["code"] == "proof.empty_numeric_interval")
            .count(),
        1
    );
    assert_eq!(
        conditions(&report)
            .iter()
            .filter(|condition| condition["code"] == "proof.primitive_const_type")
            .count(),
        1
    );
    let selected = catalogue
        .check(CheckRequest::new().root(catalogue.sources().nth(1).unwrap().id()))
        .unwrap();
    assert_eq!(
        selected.outcome().warnings(),
        2,
        "selected checks still share acquisition work from every supplied source"
    );
}
#[test]
fn normalized_powers_do_not_reapply_raw_exponent_limits_or_allocate_gaps() {
    let bytes = include_bytes!("../fixtures/advisory/normalized-power.json");
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![SourceInput::supplied("powers", bytes)],
        esdieff::Limits::default().numeric_work(21),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(status(&report), AssessmentStatus::Assessed);
    assert_eq!(conditions(&report)[0]["arguments"]["minimum"], "10e1000000");
}
#[test]
fn unavailable_numeric_values_do_not_hide_known_kind_mismatches() {
    let bytes = include_bytes!("../fixtures/advisory/unavailable-number.json");
    let catalogue =
        Catalogue::take_inputs(vec![SourceInput::supplied("unavailable", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 2);
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
    assert!(
        conditions(&report)
            .iter()
            .all(|condition| condition["code"] == "proof.primitive_const_type")
    );
}
#[test]
fn effective_repairs_deletions_alternatives_and_restricted_donors_remain_distinct() {
    let bytes = include_bytes!("../fixtures/advisory/composition.json");
    let catalogue =
        Catalogue::take_inputs(vec![SourceInput::supplied("composition", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let proofs = conditions(&report);
    assert_eq!(
        report.outcome().warnings(),
        7,
        "Base, Inherited, bad choice, fraction, two Full item branches, and nested property"
    );
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
    assert!(proofs.iter().all(
        |condition| condition["primary"]["pointer"] != "/sdfData/Choice"
            && condition["primary"]["pointer"] != "/sdfData/Rows/items"
            && condition["primary"]["pointer"] != "/sdfProperty/Repaired"
            && condition["primary"]["pointer"] != "/sdfProperty/Deleted"
    ));
    let imported = proofs
        .iter()
        .find(|condition| condition["primary"]["pointer"] == "/sdfProperty/Inherited")
        .unwrap();
    let related = imported["related"].as_array().unwrap();
    assert_eq!(related.len(), 7);
    assert!(
        related.iter().any(|location| location["kind"] == "source"
            && location["pointer"] == "/sdfData/Base/minimum")
    );
    assert!(related.iter().any(|location| location["kind"] == "source"
        && location["pointer"] == "/sdfProperty/Inherited/sdfRef"));
    assert!(
        proofs
            .iter()
            .any(|condition| condition["arguments"]["alternative"]["name"] == "bad")
    );
    assert_eq!(catalogue.sources().next().unwrap().raw_bytes(), bytes);
}
#[test]
fn relevant_unavailable_roots_and_named_entries_defer_only_the_advisory_area() {
    for bytes in [
        include_bytes!("../fixtures/advisory/critical.json").as_slice(),
        include_bytes!("../fixtures/advisory/missing-reference.json").as_slice(),
    ] {
        let catalogue =
            Catalogue::take_inputs(vec![SourceInput::supplied("limited", bytes)]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(status(&report), AssessmentStatus::Undetermined);
        coverage_support::assert_requested_counts(&report);
    }
    let bytes = br##"{"info":{},"sdfData":{"unknown":{"sdfRef":"#/sdfData/missing"},"known":{"type":"boolean","const":""}}}"##;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("entries", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
}
#[test]
fn per_proof_guards_preserve_independence_and_defer_abstract_dependencies() {
    let bytes = br#"{"info":{},"sdfData":{
      "numeric":{"type":"number","format":"unknown","unit":"v:u","contentFormat":"v:f","nullable":false,"minimum":2,"maximum":1,"default":{},"required":[false]},
      "bytes":{"type":"string","sdfType":"byte-string","nullable":false,"minLength":3,"maxLength":2},
      "epoch":{"type":"number","sdfType":"unix-time","nullable":false,"minimum":2,"maximum":1},
      "unknown":{"type":"string","format":"v:unknown","const":false},
      "dateconst":{"type":"string","format":"date","const":false},
      "null":{"type":"string","format":"v:unknown","nullable":false,"const":null},
      "unknowncollection":{"type":"string","sdfChoice":false},
      "gap":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9},
      "equality":{"type":"integer","nullable":false,"minimum":2,"maximum":2}
    }}"#;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("guards", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 2);
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
}
#[test]
fn narrow_coverage_and_full_baseline_supplemental_advice_stay_honest() {
    let bytes = br#"{"info":{},"sdfData":{"empty":{"type":"number","nullable":false,"minimum":2,"maximum":1}}}"#;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("coverage", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let coverage = report.requested_coverage();
    assert_eq!(
        (
            coverage.total(),
            coverage.assessed(),
            coverage.unassessed(),
            coverage.undetermined()
        ),
        (Some(8), 7, 0, 0)
    );
    assert!(report.outcome().complete());
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let row = json["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "advisory-four/1")
        .unwrap();
    assert_eq!(row["basis"]["kind"], "advice");
    assert_eq!(row["basis"]["id"], "advisory-four/1");
    assert!(
        !json["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition["code"] == "coverage.unimplemented"
                && condition["assessment_unit"] == "advisory-four/1")
    );
    let full = catalogue
        .check(CheckRequest::new().assessment(esdieff::Assessment::FullBaseline))
        .unwrap();
    coverage_support::assert_requested_counts(&full);
    assert_eq!(full.outcome().warnings(), 1);
    assert_eq!(full.requested_coverage().total(), Some(13));
    assert!(matches!(
        full.assessment_area_status("advisory-four/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    let no_candidate = Catalogue::take_inputs(vec![SourceInput::supplied(
        "none",
        br#"{"info":{},"sdfData":{"plain":{"type":"string"}}}"#,
    )])
    .unwrap();
    assert_eq!(
        status(&no_candidate.check(CheckRequest::new()).unwrap()),
        AssessmentStatus::NotApplicable
    );
}

#[test]
fn aggregate_warnings_survive_detail_pointer_and_report_omissions() {
    let bytes = include_bytes!("../fixtures/advisory/shared-pairs.json");
    for limits in [
        esdieff::Limits::default().conditions(1),
        esdieff::Limits::default().diagnostic_bytes(1),
        esdieff::Limits::default().pointer_bytes(1),
        esdieff::Limits::default().pointer_output_bytes(1),
        esdieff::Limits::default().report_bytes(22000),
    ] {
        let catalogue = Catalogue::take_inputs_with_limits(
            vec![SourceInput::supplied("omissions", bytes)],
            limits,
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(), 4);
        coverage_support::assert_requested_counts(&report);
        assert_eq!(status(&report), AssessmentStatus::Assessed);
        let json: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(json["outcome"]["warnings"], 4);
        assert_eq!(json["diagnostics"]["truncated"], true);
        assert!(report.to_json().len() <= 8 * 1024 * 1024);
        assert!(report.to_human().contains("warnings: 4"));
    }
}
#[test]
fn unknown_collections_and_traversal_exhaustion_do_not_claim_not_applicable() {
    let bytes = br#"{"info":{},"sdfData":{"owner":{"type":"string","enum":["same","same"]},"choice":{"sdfChoice":{"unknown":false,"known":{"type":"boolean","const":""}}}}}"#;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("branches", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(status(&report), AssessmentStatus::Undetermined);
    assert_eq!(report.outcome().warnings(), 1);
    let byte_limited = Catalogue::take_inputs_with_limits(
        vec![SourceInput::supplied(
            "partial",
            br#"{"info":{},"sdfData":{"value":{"type":"boolean","const":""}}}"#,
        )],
        esdieff::Limits::default().document_bytes(20),
    )
    .unwrap();
    assert_eq!(
        status(&byte_limited.check(CheckRequest::new()).unwrap()),
        AssessmentStatus::Undetermined
    );
    let work_limited = Catalogue::take_inputs_with_limits(
        vec![SourceInput::supplied(
            "work",
            include_bytes!("../fixtures/advisory/classes.json"),
        )],
        esdieff::Limits::default().decoding_work(5000),
    )
    .unwrap();
    assert_eq!(
        status(&work_limited.check(CheckRequest::new()).unwrap()),
        AssessmentStatus::Undetermined
    );
}
#[test]
fn enum_occurrences_retain_real_const_origins() {
    let bytes = br#"{"info":{},"sdfData":{"owner":{"type":"number","enum":["same","same"]}}}"#;
    let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("enum", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 2);
    let proofs = conditions(&report);
    for (index, proof) in proofs.iter().enumerate() {
        assert_eq!(proof["arguments"]["alternative"]["kind"], "enum");
        assert_eq!(proof["arguments"]["alternative"]["index"], index);
        assert!(
            proof["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|location| location["kind"] == "source"
                    && location["pointer"] == format!("/sdfData/owner/enum/{index}"))
        );
        assert!(
            proof["related"]
                .as_array()
                .unwrap()
                .iter()
                .all(|location| location["kind"] != "source"
                    || location["pointer"] != "/sdfData/owner/const")
        );
    }
}

#[test]
fn numeric_work_is_shared_with_count_and_integrality_premises() {
    let interval = br#"{"info":{},"sdfData":{"first":{"type":"number","nullable":false,"minimum":2,"maximum":1}}}"#;
    let counts = br#"{"info":{},"sdfData":{"second":{"type":"string","nullable":false,"minLength":3,"maxLength":2},"independent":{"type":"boolean","const":""}}}"#;
    for (work, warnings, expected) in [
        (8, 2, AssessmentStatus::Undetermined),
        (10, 3, AssessmentStatus::Assessed),
    ] {
        let catalogue = Catalogue::take_inputs_with_limits(
            vec![
                SourceInput::supplied("interval", interval),
                SourceInput::supplied("counts", counts),
            ],
            esdieff::Limits::default().numeric_work(work),
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(), warnings);
        assert_eq!(status(&report), expected);
    }
    let constant = br#"{"info":{},"sdfData":{"value":{"type":"integer","const":0.5}}}"#;
    for (work, warnings, expected) in [
        (3, 0, AssessmentStatus::Undetermined),
        (4, 1, AssessmentStatus::Assessed),
    ] {
        let catalogue = Catalogue::take_inputs_with_limits(
            vec![SourceInput::supplied("integrality", constant)],
            esdieff::Limits::default().numeric_work(work),
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(), warnings);
        assert_eq!(status(&report), expected);
    }
}
#[test]
fn direct_item_defaults_and_established_null_admission_preserve_independence() {
    let bytes = br#"{"info":{},"sdfData":{"rows":{"type":"array","items":{"type":"number","minimum":2,"maximum":1,"sdfChoice":{"allowsNull":{},"excludesNull":{"nullable":false}}}},"unavailable":{"type":"number","nullable":true,"minimum":1e1000001,"maximum":0},"null":{"type":"string","nullable":false,"format":"v:unknown","const":null}}}"#;
    let catalogue =
        Catalogue::take_inputs(vec![SourceInput::supplied("nullability", bytes)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().warnings(), 2);
    assert_eq!(status(&report), AssessmentStatus::Assessed);
    let proofs = conditions(&report);
    assert!(
        proofs
            .iter()
            .any(|condition| condition["arguments"]["alternative"]["name"] == "excludesNull")
    );
    assert!(
        proofs
            .iter()
            .all(|condition| condition["arguments"]["alternative"]["name"] != "allowsNull")
    );
}

#[test]
fn eight_omitted_witness_locations_fit_the_wire_charge_with_short_numbers() {
    let name = "\0/~\"\\".repeat(200);
    let consumer = format!("{name}consumer");
    let reference = format!("#/sdfData/{}", name.replace('~', "~0").replace('/', "~1"));
    let value = serde_json::json!({"info":{},"sdfData":{name:{"type":"number","nullable":false,"minimum":2,"maximum":1,"exclusiveMinimum":3,"exclusiveMaximum":2}},"sdfProperty":{consumer:{"sdfRef":reference}}});
    let mut bytes = vec![b' '; 1_000_000];
    bytes.extend(serde_json::to_vec(&value).unwrap());
    let keys: Vec<_> = (0..255).map(|index| format!("failed-{index}")).collect();
    let mut inputs: Vec<_> = keys.iter().map(|key| SourceInput::failed(key)).collect();
    inputs.push(SourceInput::supplied("selected", &bytes));
    for (work, warnings, expected) in [
        (20, 8, AssessmentStatus::Assessed),
        (4, 0, AssessmentStatus::Undetermined),
    ] {
        let catalogue = Catalogue::with_limits(
            &inputs,
            esdieff::Limits::default()
                .pointer_bytes(1)
                .numeric_work(work),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().root(catalogue.source_id("selected").unwrap()))
            .unwrap();
        assert_eq!(report.outcome().warnings(), warnings);
        assert_eq!(status(&report), expected);
        let witnesses = if warnings == 8 {
            conditions(&report)
        } else {
            numeric_limits(&report)
        };
        assert_eq!(witnesses.len(), 8);
        assert_eq!(
            witnesses
                .iter()
                .filter(|condition| condition["related"].as_array().unwrap().len() == 7)
                .count(),
            4
        );
        for witness in witnesses {
            assert!(witness["primary"]["pointer"].is_null());
            assert!(witness["primary"]["omission"].is_object());
            assert!(
                witness["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|location| location["pointer"].is_null()
                        && location["omission"].is_object())
            );
        }
    }
}

#[test]
fn failed_alternative_establishment_does_not_promote_raw_branch_descriptions() {
    let bytes = br#"{"info":{},"sdfData":{"owner":{"enum":["x"],"sdfChoice":{"raw":{"type":"number","nullable":false,"minimum":2,"maximum":1}}},"rows":{"type":"array","items":{"enum":["x"],"sdfChoice":{"raw":{"type":"number","nullable":false,"minimum":2,"maximum":1}}}},"independent":{"type":"boolean","const":""}}}"#;
    let limits = std::iter::once(esdieff::Limits::default()).chain(
        [256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536]
            .into_iter()
            .map(|bytes| esdieff::Limits::default().effective_allocation_bytes(bytes)),
    );
    for (index, limits) in limits.enumerate() {
        let catalogue = Catalogue::take_inputs_with_limits(
            vec![SourceInput::supplied("unestablished", bytes)],
            limits,
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        if index == 0 {
            assert_eq!(report.outcome().warnings(), 1);
        }
        assert_eq!(status(&report), AssessmentStatus::Undetermined);
        assert!(
            conditions(&report)
                .iter()
                .all(|condition| condition["code"] == "proof.primitive_const_type")
        );
    }
}

#[test]
fn unavailable_direct_item_alternative_collections_defer_full_branch_work() {
    for bytes in [
        br#"{"info":{},"sdfData":{"rows":{"type":"array","items":{"enum":["x"],"sdfChoice":{"raw":{"type":"number","nullable":false,"minimum":2,"maximum":1}}}}}}"#.as_slice(),
        br#"{"info":{},"sdfData":{"rows":{"type":"array","items":{"enum":false}}}}"#.as_slice(),
        br#"{"info":{},"sdfData":{"rows":{"type":"array","items":{"sdfChoice":false}}}}"#.as_slice(),
    ] {
        let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied("direct-collection",bytes)]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().warnings(),0);
        assert_eq!(status(&report),AssessmentStatus::Undetermined);
    }
}
