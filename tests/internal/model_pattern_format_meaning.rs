use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, Report, SourceInput};

fn model(text: &[u8], limits: Limits) -> Catalogue {
    Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap()
}

fn full(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap()
}

fn status(report: &Report<'_>, member: u16) -> AssessmentStatus {
    report
        .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
        .require()
        .unwrap()
}

#[test]
fn pattern_matching_capability_is_assessed_without_a_sample_or_a_witness() {
    let catalogue = model(
        br#"{"info":{},"sdfData":{
            "empty":{"type":"string","pattern":"(?!)"},
            "unicode":{"type":"string","pattern":"(?<=a)\\p{Letter}+(b)\\1"}
        }}"#,
        Limits::default().pattern_subject_bytes(1),
    );
    let report = full(&catalogue);
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(status(&report, 47), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 48), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
}

#[test]
fn former_runtime_capacities_do_not_limit_supported_patterns() {
    for pattern in [
        format!("(?<{}>a)", "x".repeat(125)),
        "(a)".repeat(255),
        format!("(?<x>a){}", "|a".repeat(256)),
    ] {
        let source =
            serde_json::json!({"info":{},"sdfData":{"D":{"type":"string","pattern":pattern}}})
                .to_string();
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(status(&report, 47), AssessmentStatus::Assessed);
        assert_eq!(status(&report, 48), AssessmentStatus::Assessed);
        assert!(report.outcome().complete());
    }
}

#[test]
fn pattern_capability_preserves_unsupported_syntax_and_operation_limits() {
    for (pattern, limits, syntax) in [
        (
            "(?<x>a)|(?<x>b)",
            Limits::default(),
            AssessmentStatus::Undetermined,
        ),
        (
            "aa",
            Limits::default().pattern_bytes(1),
            AssessmentStatus::Undetermined,
        ),
        ("(", Limits::default(), AssessmentStatus::Assessed),
    ] {
        let source =
            serde_json::json!({"info":{},"sdfData":{"D":{"type":"string","pattern":pattern}}})
                .to_string();
        let catalogue = model(source.as_bytes(), limits);
        let report = full(&catalogue);
        assert_eq!(status(&report, 47), syntax);
        assert_eq!(status(&report, 48), AssessmentStatus::Undetermined);
        assert!(!report.outcome().complete());
    }
}

#[test]
fn known_format_and_special_type_meanings_need_no_test_value() {
    let catalogue = model(
        br#"{"info":{},"sdfData":{
            "date":{"type":"string","format":"date","minLength":1},
            "clock":{"type":"string","format":"time"},
            "instant":{"type":"string","format":"date-time"},
            "identifier":{"type":"string","format":"uri"},
            "reference":{"type":"string","format":"uri-reference"},
            "uuid":{"type":"string","format":"uuid"},
            "bytes":{"type":"string","sdfType":"byte-string"},
            "posix":{"type":"number","sdfType":"unix-time","minimum":0}
        }}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    assert_eq!(report.outcome().errors(), 0);
    for member in [26, 27, 42, 45, 46, 49] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(wire["baseline"].get("temporal_knowledge").is_none());
    assert!(
        wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn named_interpretation_and_conflicting_conventional_context_are_distinct() {
    for (description, member, expected) in [
        (r#"{"format":"date"}"#, 49, AssessmentStatus::Assessed),
        (
            r#"{"sdfType":"byte-string"}"#,
            26,
            AssessmentStatus::Assessed,
        ),
        (r#"{"sdfType":"unix-time"}"#, 27, AssessmentStatus::Assessed),
        (
            r#"{"type":"number","format":"date"}"#,
            49,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"type":"number","sdfType":"byte-string"}"#,
            26,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"type":"string","sdfType":"unix-time"}"#,
            27,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"type":"string","format":"date","sdfType":"byte-string"}"#,
            49,
            AssessmentStatus::Undetermined,
        ),
    ] {
        let source = format!(r#"{{"info":{{}},"sdfData":{{"D":{description}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, member), expected, "{source}");
        if description.contains("type") {
            assert_eq!(status(&report, 42), AssessmentStatus::Assessed, "{source}");
        }
    }
}

#[test]
fn byte_text_qualities_keep_only_their_unsettled_interpretation_unfinished() {
    let catalogue = model(
        br#"{"info":{},"sdfData":{"D":{
            "type":"string","sdfType":"byte-string","minLength":1,"pattern":"a"
        }}}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    assert_eq!(status(&report, 26), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 47), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 46), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 48), AssessmentStatus::Undetermined);
}

#[test]
fn known_const_interpretation_is_separate_from_selector_meaning() {
    for (description, expected) in [
        (
            r#"{"type":"string","format":"date","const":"2024-02-29"}"#,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"string","format":"time","const":"23:59:60Z"}"#,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"type":"string","format":"uri","const":"https://example.com/a"}"#,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"string","format":"uri","const":"future:item"}"#,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"type":"string","sdfType":"byte-string","const":"Zg"}"#,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":0}"#,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":-1}"#,
            AssessmentStatus::Undetermined,
        ),
    ] {
        let source = format!(r#"{{"info":{{}},"sdfData":{{"D":{description}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 43), expected, "{source}");
        for member in [26, 27, 49] {
            assert!(
                matches!(
                    status(&report, member),
                    AssessmentStatus::Assessed | AssessmentStatus::NotApplicable
                ),
                "{source} M{member}"
            );
        }
    }
}

#[test]
fn unknown_base_values_and_restricted_donors_keep_affected_meanings_unfinished() {
    for description in [
        r#"{"type":"string","format":"future","const":"x"}"#,
        r#"{"type":"string","sdfType":"future","const":"x"}"#,
    ] {
        let source = format!(r#"{{"info":{{}},"sdfData":{{"D":{description}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(
            status(&report, 43),
            AssessmentStatus::Undetermined,
            "{source}"
        );
        if description.contains("format") {
            assert_eq!(
                status(&report, 49),
                AssessmentStatus::Undetermined,
                "{source}"
            );
        }
    }
    // Undeclared extension vocabulary leaves the known format and constant intact,
    // following ADR 0037: Undeclared extension vocabulary.
    let catalogue = model(
        br#"{"info":{},"sdfData":{"D":{
            "type":"string","format":"date","future:quality":true,"const":"2024-01-01"
        }}}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    assert_eq!(status(&report, 43), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 49), AssessmentStatus::Assessed);
    let catalogue = model(
        br#"{"info":{},"sdfData":{"D":{"type":"array","items":{
            "type":"string","pattern":"a","format":"date","sdfChoice":{"branch":{}}
        }}}}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    assert_eq!(status(&report, 48), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 49), AssessmentStatus::Undetermined);
}

#[test]
fn byte_and_uuid_comparator_capabilities_need_no_operands() {
    for items in [
        serde_json::json!({"sdfChoice":{"id":{"type":"string","format":"uuid","nullable":false}}}),
        serde_json::json!({"sdfChoice":{"bytes":{"type":"string","sdfType":"byte-string"}}}),
        serde_json::json!({"type":"object","properties":{
            "bytes":{"type":"string","sdfType":"byte-string"},
            "uuid":{"type":"string","format":"uuid","nullable":false}
        }}),
        serde_json::json!({"sdfChoice":{
            "one":{"type":"object","properties":{"key":{"type":"string","sdfType":"byte-string"}}},
            "two":{"type":"object","properties":{"key":{"type":"string","sdfType":"byte-string"}}}
        }}),
    ] {
        let source = serde_json::json!({"info":{},"sdfData":{"D":{"type":"array","uniqueItems":true,"items":items}}}).to_string();
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 52), AssessmentStatus::Assessed, "{source}");
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            wire["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(wire["baseline"].get("temporal_knowledge").is_none());
    }
}

#[test]
fn abstract_comparator_gaps_and_incompatible_choice_projections_stay_unfinished() {
    for items in [
        serde_json::json!({"type":"string","format":"date"}),
        serde_json::json!({"type":"string","format":"uri"}),
        serde_json::json!({"sdfChoice":{"unix":{"type":"number","sdfType":"unix-time"}}}),
        serde_json::json!({"sdfChoice":{
            "bytes":{"type":"string","sdfType":"byte-string"},
            "uuid":{"type":"string","format":"uuid","nullable":false}
        }}),
        serde_json::json!({"sdfChoice":{
            "one":{"type":"object","properties":{"key":{"type":"string","sdfType":"byte-string"}}},
            "two":{"type":"object","properties":{"key":{"type":"string","format":"uuid","nullable":false}}}
        }}),
        serde_json::json!({"sdfChoice":{
            "one":{"type":"object","properties":{"key":{"type":"string","sdfType":"byte-string"}}},
            "two":{"type":"object"}
        }}),
    ] {
        let source = serde_json::json!({"info":{},"sdfData":{"D":{"type":"array","uniqueItems":true,"items":items}}}).to_string();
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(
            status(&report, 52),
            AssessmentStatus::Undetermined,
            "{source}"
        );
    }
}

#[test]
fn nullable_uuid_comparator_capability_includes_absence_true_and_false() {
    for nullable in [
        serde_json::Value::Null,
        serde_json::json!(true),
        serde_json::json!(false),
    ] {
        let mut branch = serde_json::json!({"type":"string","format":"uuid"});
        if !nullable.is_null() {
            branch["nullable"] = nullable;
        }
        let source = serde_json::json!({"info":{},"sdfData":{"D":{"type":"array","uniqueItems":true,"items":{"sdfChoice":{"id":branch}}}}}).to_string();
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 52), AssessmentStatus::Assessed);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            wire["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn content_format_declaration_does_not_erase_known_named_interpretations() {
    for (selector, member) in [
        (
            serde_json::json!({"type":"string","sdfType":"byte-string"}),
            26,
        ),
        (
            serde_json::json!({"type":"number","sdfType":"unix-time"}),
            27,
        ),
        (serde_json::json!({"type":"string","format":"date"}), 49),
    ] {
        for content in ["application/cbor", "application/future"] {
            let mut description = selector.clone();
            description["contentFormat"] = serde_json::json!(content);
            let source = serde_json::json!({"info":{},"sdfData":{"D":description}}).to_string();
            let catalogue = model(source.as_bytes(), Limits::default());
            let report = full(&catalogue);
            assert_eq!(
                status(&report, member),
                AssessmentStatus::Assessed,
                "{source}"
            );
            assert_eq!(status(&report, 42), AssessmentStatus::Assessed, "{source}");
            assert_eq!(
                status(&report, 24),
                if content == "application/cbor" {
                    AssessmentStatus::Assessed
                } else {
                    AssessmentStatus::Undetermined
                },
                "{source}"
            );
        }
    }
}
