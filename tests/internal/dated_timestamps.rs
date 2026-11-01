//! Dated instant equality follows ADR 0018 through public catalogue operations.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, JsonBuilder, Limits, Report,
    SampleKind, SampleRequest, SourceInput,
};
use serde_json::{Value, json};

fn catalogue(description: Value, sample: &str, limits: Limits) -> Catalogue {
    let model = json!({"info":{},"sdfProperty":{"p":description}}).to_string();
    Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ],
        limits,
    )
    .unwrap()
}

fn selected(cat: &Catalogue) -> Report<'_> {
    cat.check_sample(
        SampleRequest::new(
            cat.source_id("m").unwrap(),
            SampleKind::Property,
            "/sdfProperty/p",
        )
        .sample(cat.source_id("s").unwrap()),
    )
    .unwrap()
}

fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}

fn const_status(report: &Report<'_>) -> AssessmentStatus {
    report
        .assessment_area_status("sample-const/1")
        .require()
        .unwrap()
}

fn instant() -> Value {
    json!({"type":"string","format":"date-time"})
}

fn instant_items() -> Value {
    json!({"type":"array","uniqueItems":true,"items":{"sdfChoice":{"instant":instant()}}})
}

#[test]
fn full_instant_comparator_needs_no_operands_or_adjustment_consultation() {
    for nullable in [None, Some(true), Some(false)] {
        let mut description = instant();
        if let Some(nullable) = nullable {
            description["nullable"] = json!(nullable);
        }
        let model = json!({"info":{},"sdfData":{"D":{
            "type":"array","uniqueItems":true,"items":{"sdfChoice":{"instant":description}}
        }}})
        .to_string();
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("m", model.as_bytes())]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M052/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
        let value = wire(&report);
        assert!(
            value["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(value["baseline"].get("temporal_knowledge").is_none());
    }
}

#[test]
fn equivalent_offsets_calendar_slots_and_exact_fractions_compare_symmetrically() {
    for (left, right, knowledge) in [
        ("1996-12-19T16:39:57-08:00", "1996-12-20T00:39:57Z", false),
        ("2024-01-01T12:34:56Z", "2024-01-01t12:34:56z", false),
        (
            "2024-01-01T12:34:56+00:00",
            "2024-01-01T12:34:56-00:00",
            false,
        ),
        ("2024-01-01T12:34:56Z", "2024-01-01T12:34:56-00:00", false),
        (
            "2024-01-01T12:34:56.1Z",
            "2024-01-01T12:34:56.100+00:00",
            false,
        ),
        ("2024-01-01T12:34:56Z", "2024-01-01T12:34:56.000Z", false),
        ("2024-03-01T00:00:58+01:00", "2024-02-29T23:00:58Z", false),
        ("1900-03-01T00:00:58+01:00", "1900-02-28T23:00:58Z", false),
        ("0000-03-01T00:00:58+01:00", "0000-02-29T23:00:58Z", false),
        ("2024-12-31T23:30:58-01:00", "2025-01-01T00:30:58Z", false),
        ("2024-03-01T00:00:58+23:59", "2024-02-29T00:01:58Z", false),
        ("2024-03-01T23:59:58-23:59", "2024-03-02T23:58:58Z", false),
        (
            "0000-01-01T00:59:58+23:59",
            "0000-01-01T00:00:58+23:00",
            false,
        ),
        (
            "9999-12-31T23:00:58-23:59",
            "9999-12-31T23:59:58-23:00",
            false,
        ),
        (
            "2016-12-31T23:59:60.10Z",
            "2017-01-01T05:44:60.100+05:45",
            true,
        ),
        (
            "2016-12-31T15:59:60-08:00",
            "2016-12-31T23:59:60-00:00",
            true,
        ),
    ] {
        for (constant, sample) in [(left, right), (right, left)] {
            let cat = catalogue(
                json!({"type":"string","format":"date-time","const":constant}),
                &json!(sample).to_string(),
                Limits::default(),
            );
            let report = selected(&cat);
            assert_eq!(
                const_status(&report),
                AssessmentStatus::Assessed,
                "{constant} / {sample}"
            );
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Admitted,
                "{constant} / {sample}"
            );
            assert!(report.outcome().complete());
            let value = wire(&report);
            assert_eq!(
                value["baseline"].get("temporal_knowledge").is_some(),
                knowledge
            );
            assert!(
                value["result"]["meaning"]["const"]["limitations"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(report.to_human().contains("admitted"));
        }
    }
}

#[test]
fn different_instants_preserve_inserted_slots_and_unlimited_fractional_precision() {
    let long_left = format!("2024-01-01T12:34:56.{}1Z", "0".repeat(4095));
    let long_right = format!("2024-01-01T12:34:56.{}2Z", "0".repeat(4095));
    for (left, right) in [
        (
            "2024-01-01T12:34:56.1000000000000000000000Z",
            "2024-01-01T12:34:56.1000000000000000000001Z",
        ),
        ("2024-01-01T12:34:56.1Z", "2024-01-01T12:34:56.01Z"),
        ("2016-12-31T23:59:59Z", "2016-12-31T23:59:60Z"),
        ("2016-12-31T23:59:60Z", "2017-01-01T00:00:00Z"),
        ("2016-12-31T23:59:60.1Z", "2017-01-01T05:44:60.2+05:45"),
        (long_left.as_str(), long_right.as_str()),
    ] {
        for (constant, sample) in [(left, right), (right, left)] {
            let cat = catalogue(
                json!({"type":"string","format":"date-time","const":constant}),
                &json!(sample).to_string(),
                Limits::default(),
            );
            let report = selected(&cat);
            assert_eq!(const_status(&report), AssessmentStatus::Assessed);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert!(report.outcome().complete());
            assert_eq!(report.outcome().errors(), 1);
            let value = wire(&report);
            assert!(
                value["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "sample.const")
            );
        }
    }
    // Trailing zeros beyond a machine precision still leave the same fraction.
    let cat = catalogue(
        json!({"type":"string","format":"date-time","const":long_left}),
        &json!(format!("2024-01-01T12:34:56.{}1000Z", "0".repeat(4095))).to_string(),
        Limits::default(),
    );
    assert_eq!(
        selected(&cat).sample_result().unwrap().admission(),
        Admission::Admitted
    );
}

#[test]
fn malformed_or_unestablished_operands_never_supply_an_instant_mismatch() {
    for (left, right, knowledge) in [
        ("bad", "2024-01-01T12:34:56Z", false),
        // An invalid first operand still attempts the second operand's knowledge.
        ("bad", "2016-12-31T23:59:60Z", true),
        ("bad", "bad", false),
        ("2024-02-30T00:00:00Z", "2024-03-01T00:00:00Z", false),
        ("2024-01-01T24:00:00Z", "2024-01-02T00:00:00Z", false),
        ("2024-01-01T12:34:56.Z", "2024-01-01T12:34:56Z", false),
        ("2024-01-01T12:34:56Z[UTC]", "2024-01-01T12:34:56Z", false),
        ("2030-06-30T23:59:60Z", "2030-07-01T01:59:60+02:00", true),
        ("2030-06-30T23:59:59Z", "2030-07-01T00:00:00Z", true),
        ("1971-12-31T23:59:59Z", "1972-01-01T00:00:00Z", true),
        ("2024-06-30T23:59:60Z", "2024-07-01T00:00:00Z", true),
    ] {
        for (constant, sample) in [(left, right), (right, left)] {
            let cat = catalogue(
                json!({"type":"string","format":"date-time","const":constant}),
                &json!(sample).to_string(),
                Limits::default(),
            );
            let report = selected(&cat);
            assert_eq!(
                const_status(&report),
                AssessmentStatus::Undetermined,
                "{constant} / {sample}"
            );
            assert!(!report.outcome().complete());
            let value = wire(&report);
            assert_eq!(
                value["baseline"].get("temporal_knowledge").is_some(),
                knowledge
            );
            assert!(
                !value["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "sample.const")
            );
        }
    }
}

#[test]
fn uniqueness_uses_full_instant_meaning_and_preserves_unknown_pairs_and_duplicates() {
    for (sample, admission, complete, duplicate) in [
        (
            r#"["1996-12-19T16:39:57-08:00","1996-12-20T00:39:57Z"]"#,
            Admission::Rejected,
            true,
            true,
        ),
        (
            r#"["1996-12-20T00:39:57Z","1996-12-19T16:39:57-08:00"]"#,
            Admission::Rejected,
            true,
            true,
        ),
        (
            r#"["2024-01-01T12:34:56.1Z","2024-01-01T12:34:56.2Z"]"#,
            Admission::Admitted,
            true,
            false,
        ),
        (
            r#"[null,"2024-01-01T12:34:56Z"]"#,
            Admission::Admitted,
            true,
            false,
        ),
        (
            r#"[null,"2024-01-01T12:34:56Z",null]"#,
            Admission::Rejected,
            true,
            true,
        ),
        (
            r#"["2030-06-30T23:59:59Z","2030-07-01T00:00:00Z"]"#,
            Admission::Undetermined,
            false,
            false,
        ),
        (
            r#"["2030-06-30T23:59:59Z","1996-12-19T16:39:57-08:00","1996-12-20T00:39:57Z"]"#,
            Admission::Rejected,
            false,
            true,
        ),
        ("[]", Admission::Admitted, true, false),
        (
            r#"["2024-01-01T12:34:56Z"]"#,
            Admission::Admitted,
            true,
            false,
        ),
    ] {
        let cat = catalogue(instant_items(), sample, Limits::default());
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            admission,
            "{sample}"
        );
        assert_eq!(report.outcome().complete(), complete, "{sample}");
        let value = wire(&report);
        let finding = value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "sample.unique");
        assert_eq!(finding.is_some(), duplicate, "{sample}");
        if duplicate {
            assert!(
                finding.unwrap()["primary"]["pointer"]
                    .as_str()
                    .unwrap()
                    .starts_with('/')
            );
            assert!(!finding.unwrap()["related"].as_array().unwrap().is_empty());
        }
    }
}

#[test]
fn model_capability_projects_compatible_full_meanings_and_retains_direct_guards() {
    use AssessmentStatus::{Assessed as A, Undetermined as U};
    for (items, expected) in [
        (
            json!({"type":"object","properties":{"stamp":{"sdfRef":"#/sdfData/Instant"}}}),
            A,
        ),
        (
            json!({"sdfChoice":{"array":{"type":"array","items":{"sdfChoice":{"instant":{"sdfRef":"#/sdfData/Instant"}}}}}}),
            A,
        ),
        (
            json!({"sdfChoice":{"a":{"sdfRef":"#/sdfData/Instant","nullable":false},
            "b":{"sdfRef":"#/sdfData/Instant","nullable":true}}}),
            A,
        ),
        (
            json!({"sdfChoice":{"outer":{"sdfChoice":{"inner":{"sdfRef":"#/sdfData/Instant"}}}}}),
            A,
        ),
        (instant(), U),
        (
            json!({"sdfChoice":{"instant":{"type":"string","format":"date-time","nullable":"unknown"}}}),
            U,
        ),
        (
            json!({"sdfChoice":{"instant":{"type":"string","format":"date-time","future:quality":true}}}),
            A,
        ),
        (
            json!({"sdfChoice":{"instant":instant(),"text":{"type":"string"}}}),
            U,
        ),
        (
            json!({"sdfChoice":{"instant":instant(),"date":{"type":"string","format":"date"}}}),
            U,
        ),
        (
            json!({"sdfChoice":{"instant":instant(),"clock":{"type":"string","format":"time"}}}),
            U,
        ),
        (
            json!({"type":"string","nullable":false,"sdfChoice":{"instant":{"format":"date-time"}}}),
            U,
        ),
        (
            json!({"type":"string","nullable":false,"sdfChoice":{"instant":{"format":"date-time","nullable":true}}}),
            A,
        ),
    ] {
        let model = json!({"info":{},"sdfData":{"Instant":instant(),
            "A":{"type":"array","uniqueItems":true,"items":items}}})
        .to_string();
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("m", model.as_bytes())]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M052/1")
                .require()
                .unwrap(),
            expected,
            "{model}"
        );
        // Only undeclared extension vocabulary names a reading, following
        // ADR 0037: Undeclared extension vocabulary.
        let reading: &[&str] = if model.contains("future:quality") {
            &["undeclared-extensions-ignorable/1"]
        } else {
            &[]
        };
        assert_eq!(
            wire(&report)["baseline"]["interpretations"],
            json!(reading),
            "{model}"
        );
        assert!(
            wire(&report)["baseline"]
                .get("temporal_knowledge")
                .is_none()
        );
    }
}

#[test]
fn alternatives_do_not_select_a_preferred_instant_interpretation() {
    for (branches, expected) in [
        (
            json!({"a":{"format":"date-time"},"b":{"format":"date-time"}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"a":{"format":"date-time"},"b":{}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{},"b":{"format":"date-time"}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"format":"date-time"},"b":{"format":"time"}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"format":"time"},"b":{"format":"date-time"}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"format":"date-time"},"b":{"format":null}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"format":"date-time"},"b":{"sdfRef":"#/sdfData/missing"}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"sdfRef":"#/sdfData/missing"},"b":{"format":"date-time"}}),
            AssessmentStatus::Undetermined,
        ),
    ] {
        let cat = catalogue(
            json!({"type":"string","const":"1996-12-19T16:39:57-08:00","sdfChoice":branches}),
            r#""1996-12-20T00:39:57Z""#,
            Limits::default(),
        );
        let report = selected(&cat);
        assert_eq!(const_status(&report), expected);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            if expected == AssessmentStatus::Assessed {
                Admission::Admitted
            } else {
                Admission::Undetermined
            }
        );
    }
}

#[test]
fn compound_instant_comparisons_retain_decisive_nested_locations_and_support() {
    for sample in [
        r#"{"stamp":"1996-12-20T00:39:57Z","other":"2030-06-30T23:59:60Z"}"#,
        r#"{"stamp":"1996-12-20T00:39:58Z","other":"2030-06-30T23:59:60Z"}"#,
    ] {
        let model = json!({"info":{},"sdfData":{"Instant":instant()},"sdfProperty":{"p":{
            "type":"object","const":{"stamp":"1996-12-19T16:39:57-08:00","other":"2030-06-30T23:59:60Z"},
            "properties":{"stamp":{"sdfRef":"#/sdfData/Instant"},"other":instant()}
        }}}).to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        let different = sample.contains("00:39:58");
        assert_eq!(
            const_status(&report),
            if different {
                AssessmentStatus::Assessed
            } else {
                AssessmentStatus::Undetermined
            }
        );
        assert!(!report.outcome().complete());
        let value = wire(&report);
        assert!(value["baseline"].get("temporal_knowledge").is_some());
        if different {
            let finding = value["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["code"] == "sample.const")
                .unwrap();
            assert_eq!(finding["primary"]["pointer"], "/stamp");
            assert_eq!(finding["primary"]["span"], json!([9, 31]));
            assert!(
                finding["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["pointer"] == "/sdfProperty/p/const/stamp")
            );
            assert!(
                value["result"]["meaning"]["const"]["limitations"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                value["result"]["meaning"]["const"]["basis"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|b| b["citation"]
                        .as_str()
                        .is_some_and(|s| s.contains("ADR 0018")))
            );
        }
    }
    for (sample, admission) in [
        (r#"["1996-12-20T00:39:57Z"]"#, Admission::Admitted),
        (r#"["1996-12-20T00:39:58Z"]"#, Admission::Rejected),
    ] {
        let mut description = instant_items();
        description["const"] = json!(["1996-12-19T16:39:57-08:00"]);
        let cat = catalogue(description, sample, Limits::default());
        let report = selected(&cat);
        assert_eq!(report.sample_result().unwrap().admission(), admission);
        assert!(report.outcome().complete());
        if admission == Admission::Rejected {
            let value = wire(&report);
            assert_eq!(
                value["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|d| d["code"] == "sample.const")
                    .unwrap()["primary"]["pointer"],
                "/0"
            );
        }
    }
}

#[test]
fn defaults_and_enum_candidates_share_the_instant_comparator() {
    for (constant, default, warnings, status) in [
        (
            "1996-12-19T16:39:57-08:00",
            "1996-12-20T00:39:57Z",
            0,
            AssessmentStatus::Assessed,
        ),
        (
            "1996-12-20T00:39:57Z",
            "1996-12-19T16:39:57-08:00",
            0,
            AssessmentStatus::Assessed,
        ),
        (
            "1996-12-19T16:39:57-08:00",
            "1996-12-20T00:39:58Z",
            1,
            AssessmentStatus::Assessed,
        ),
        (
            "2030-06-30T23:59:60Z",
            "2030-07-01T01:59:60+02:00",
            0,
            AssessmentStatus::Undetermined,
        ),
    ] {
        let cat = catalogue(
            json!({"type":"string","format":"date-time","const":constant,"default":default}),
            "null",
            Limits::default(),
        );
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        assert_eq!(
            report
                .assessment_area_status("default-consistency/1")
                .require()
                .unwrap(),
            status
        );
        let value = wire(&report);
        let findings: Vec<_> = value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "consistency.default_excluded")
            .collect();
        assert_eq!(findings.len(), warnings);
        if warnings != 0 {
            assert_eq!(findings[0]["basis"]["kind"], "advice");
            assert_eq!(findings[0]["primary"]["pointer"], "/sdfProperty/p/default");
        }
        let full = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert!(
            !wire(&full)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "consistency.default_excluded")
        );
    }
    let cat = catalogue(
        json!({"type":"string","format":"date-time","enum":[
            "2030-06-30T23:59:60Z","1996-12-19T16:39:57-08:00"
        ]}),
        r#""1996-12-20T00:39:57Z""#,
        Limits::default(),
    );
    let report = selected(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert!(report.outcome().complete());
    let value = wire(&report);
    assert!(value["baseline"].get("temporal_knowledge").is_some());
    assert!(
        value["result"]["meaning"]["const"]["limitations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn all_declaring_selectors_preserve_typed_and_original_equivalent_instants() {
    let model = json!({"info":{},"sdfData":{"D":{
        "type":"string","format":"date-time","const":"1996-12-19T16:39:57-08:00"
    }},"sdfProperty":{"p":{"sdfRef":"#/sdfData/D"}},
    "sdfAction":{"a":{"sdfInputData":{"sdfRef":"#/sdfData/D"},"sdfOutputData":{"sdfRef":"#/sdfData/D"}}},
    "sdfEvent":{"e":{"sdfOutputData":{"sdfRef":"#/sdfData/D"}}}}).to_string();
    for typed in [false, true] {
        for (text, token, admission) in [
            (
                "1996-12-20T00:39:57Z",
                r#""1996-12-20\u005400:39:57\u005a""#,
                Admission::Admitted,
            ),
            (
                "1996-12-20T00:39:58Z",
                r#""1996-12-20T00:39:58Z""#,
                Admission::Rejected,
            ),
        ] {
            let sample = if typed {
                SourceInput::typed("s", JsonBuilder::new().text(text).unwrap())
            } else {
                SourceInput::supplied("s", token.as_bytes()).sample()
            };
            let cat =
                Catalogue::from_inputs(&[SourceInput::supplied("m", model.as_bytes()), sample])
                    .unwrap();
            for (kind, pointer) in [
                (SampleKind::Property, "/sdfProperty/p"),
                (SampleKind::Data, "/sdfData/D"),
                (SampleKind::ActionInput, "/sdfAction/a"),
                (SampleKind::ActionOutput, "/sdfAction/a"),
                (SampleKind::EventOutput, "/sdfEvent/e"),
            ] {
                let report = cat
                    .check_sample(
                        SampleRequest::new(cat.source_id("m").unwrap(), kind, pointer)
                            .sample(cat.source_id("s").unwrap()),
                    )
                    .unwrap();
                assert_eq!(report.sample_result().unwrap().admission(), admission);
                assert!(report.outcome().complete());
                assert!(report.to_human().contains(admission.as_str()));
                let value = wire(&report);
                assert_eq!(value["version"], "1.1");
                assert_eq!(value["coverage"]["requested"]["total"], 9);
                assert_eq!(
                    value["result"]["sample_location"]["span"],
                    if typed {
                        Value::Null
                    } else {
                        json!([0, token.len()])
                    }
                );
                if admission == Admission::Rejected {
                    let finding = value["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|d| d["code"] == "sample.const")
                        .unwrap();
                    assert!(
                        finding["related"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|r| r["pointer"] == "/sdfData/D/const")
                    );
                }
            }
        }
    }
}

#[test]
fn actual_constants_and_admission_constraints_keep_independent_meaning() {
    for (constant, expected) in [
        ("0000-01-01T00:59:58+23:59", AssessmentStatus::Assessed),
        ("2016-12-31T23:59:60Z", AssessmentStatus::Assessed),
        ("2030-06-30T23:59:60Z", AssessmentStatus::Undetermined),
        ("bad", AssessmentStatus::Undetermined),
    ] {
        let cat = catalogue(
            json!({"type":"string","format":"date-time","const":constant}),
            "null",
            Limits::default(),
        );
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M049/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M043/1")
                .require()
                .unwrap(),
            expected
        );
    }
    for sample in ["1996-12-20T00:39:57Z", "1996-12-20T00:39:58Z"] {
        for restriction in [json!({"maxLength":1}), json!({"pattern":"^x$"})] {
            let mut description =
                json!({"type":"string","format":"date-time","const":"1996-12-19T16:39:57-08:00"});
            description
                .as_object_mut()
                .unwrap()
                .extend(restriction.as_object().unwrap().clone());
            let cat = catalogue(description, &json!(sample).to_string(), Limits::default());
            let report = selected(&cat);
            assert_eq!(const_status(&report), AssessmentStatus::Assessed);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert!(report.outcome().complete());
            assert_eq!(
                report.outcome().errors(),
                if sample.ends_with("57Z") { 1 } else { 2 }
            );
        }
    }
}

#[test]
fn reference_format_overrides_and_compound_defaults_keep_the_effective_contract() {
    for (consumer, expected) in [
        (
            json!({"sdfRef":"#/sdfData/D","format":"date-time"}),
            Admission::Admitted,
        ),
        (
            json!({"sdfRef":"#/sdfData/D","format":null}),
            Admission::Rejected,
        ),
    ] {
        let model = json!({"info":{},"sdfData":{"D":{"type":"string","format":"date",
            "const":"1996-12-19T16:39:57-08:00"}},"sdfProperty":{"p":consumer}})
        .to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""1996-12-20T00:39:57Z""#).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(const_status(&report), AssessmentStatus::Assessed);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete());
    }
    let cat = catalogue(
        json!({"type":"object",
            "const":{"stamp":"1996-12-19T16:39:57-08:00","later":"bad"},
            "default":{"stamp":"1996-12-20T00:39:58Z","later":"bad"},
            "properties":{"stamp":instant(),"later":{"type":"string","format":"future-format"}}
        }),
        "{}",
        Limits::default(),
    );
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    let value = wire(&report);
    let findings: Vec<_> = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "consistency.default_excluded")
        .collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0]["primary"]["pointer"],
        "/sdfProperty/p/default/stamp"
    );
    assert_eq!(findings[0]["basis"]["kind"], "advice");
    assert!(!report.outcome().complete());
    let cat = catalogue(
        json!({"type":"object","required":["stamp"],"properties":{
            "stamp":{"type":"string","format":"date-time","default":"1996-12-19T16:39:57-08:00"}
        }}),
        "{}",
        Limits::default(),
    );
    assert_eq!(
        selected(&cat).sample_result().unwrap().admission(),
        Admission::Rejected
    );
}

#[test]
fn original_work_and_storage_boundaries_never_supply_unfunded_instant_proofs() {
    for (constant, sample) in [
        ("1996-12-19T16:39:57-08:00", "1996-12-20T00:39:57Z"),
        ("1996-12-19T16:39:57-08:00", "1996-12-20T00:39:58Z"),
    ] {
        let description = json!({"type":"string","format":"date-time","const":constant});
        let sample = json!(sample).to_string();
        for configure in [
            Limits::decoding_work as fn(Limits, usize) -> Limits,
            Limits::effective_allocation_bytes,
        ] {
            let assessed = |remaining| {
                let cat = catalogue(
                    description.clone(),
                    &sample,
                    configure(Limits::default(), remaining),
                );
                const_status(&selected(&cat)) == AssessmentStatus::Assessed
            };
            let (mut low, mut high) = (1, 250_000);
            assert!(assessed(high));
            while low < high {
                let middle = low + (high - low) / 2;
                if assessed(middle) {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            assert!(low > 1);
            assert!(assessed(low));
            let cat = catalogue(
                description.clone(),
                &sample,
                configure(Limits::default(), low - 1),
            );
            for _ in 0..2 {
                let report = selected(&cat);
                assert_eq!(const_status(&report), AssessmentStatus::Undetermined);
                assert!(!report.outcome().complete());
                assert!(
                    !wire(&report)["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["code"] == "sample.const")
                );
            }
            assert_eq!(
                cat.source(cat.source_id("s").unwrap()).unwrap().raw_bytes(),
                sample.as_bytes()
            );
        }
    }
}

#[test]
fn request_pair_limits_and_optional_detail_omission_keep_truthful_outcomes() {
    for sample in [
        r#"["2024-01-01T12:34:56.1Z","2024-01-01T12:34:56.2Z","2024-01-01T12:34:56.3Z"]"#,
        r#"["2024-01-01T12:34:56.3Z","2024-01-01T12:34:56.2Z","2024-01-01T12:34:56.1Z"]"#,
    ] {
        for (pairs, admission) in [(2, Admission::Undetermined), (3, Admission::Admitted)] {
            let cat = catalogue(
                instant_items(),
                sample,
                Limits::default().equality_pairs(pairs),
            );
            for _ in 0..2 {
                let report = selected(&cat);
                assert_eq!(report.sample_result().unwrap().admission(), admission);
                assert_eq!(report.outcome().complete(), pairs == 3);
            }
        }
    }
    for limits in [
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
        Limits::default().pointer_bytes(1).pointer_output_bytes(1),
    ] {
        for (description, sample) in [
            (
                json!({"type":"string","format":"date-time","const":"1996-12-19T16:39:57-08:00"}),
                r#""1996-12-20T00:39:58Z""#,
            ),
            (
                instant_items(),
                r#"["1996-12-19T16:39:57-08:00","1996-12-20T00:39:57Z"]"#,
            ),
        ] {
            let cat = catalogue(description, sample, limits.clone());
            let report = selected(&cat);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert_eq!(report.outcome().errors(), 1);
            assert!(report.outcome().complete());
        }
    }
}
