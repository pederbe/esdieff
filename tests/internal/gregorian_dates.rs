//! Gregorian date equality follows ADR 0018 through the catalogue operations.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, JsonBuilder, Limits, Report,
    SampleKind, SampleRequest, SourceInput,
};
use serde_json::{Value, json};

fn catalogue(description: Value, sample: &str, limits: Limits) -> Catalogue {
    let model = json!({"info":{}, "sdfProperty":{"p":description}}).to_string();
    Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ],
        limits,
    )
    .unwrap()
}

fn selected(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
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

#[test]
fn established_dates_complete_equal_and_different_constants_in_both_orders() {
    for (left, right) in [
        ("0000-02-29", "0000-03-01"),
        ("2000-02-29", "2000-03-01"),
        ("2024-04-30", "2024-05-01"),
        ("2024-12-31", "2025-01-01"),
        ("9999-12-30", "9999-12-31"),
    ] {
        for (constant, sample) in [(left, right), (right, left), (left, left)] {
            let cat = catalogue(
                json!({"type":"string","format":"date","nullable":false,"const":constant}),
                &json!(sample).to_string(),
                Limits::default(),
            );
            let report = selected(&cat);
            assert_eq!(const_status(&report), AssessmentStatus::Assessed);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                if constant == sample {
                    Admission::Admitted
                } else {
                    Admission::Rejected
                }
            );
            assert!(report.outcome().complete());
            let value = wire(&report);
            assert!(value["baseline"].get("temporal_knowledge").is_none());
            assert!(
                value["baseline"]["interpretations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|id| id == "ADR0018-selected-representation")
            );
        }
    }
}

#[test]
fn full_date_comparator_capability_needs_no_sample_and_includes_nullability() {
    for nullable in [None, Some(true), Some(false)] {
        let mut date = json!({"type":"string","format":"date"});
        if let Some(nullable) = nullable {
            date["nullable"] = json!(nullable);
        }
        let model = json!({"info":{},"sdfData":{"D":{
            "type":"array","uniqueItems":true,"items":{"sdfChoice":{"date":date}}
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
fn invalid_dates_never_supply_calendar_equality_or_difference() {
    for invalid in [
        json!("1900-02-29"),
        json!("2024-04-31"),
        json!("2024-00-01"),
        json!("2024-13-01"),
        json!("2024-01-00"),
        json!("2024-01-32"),
        json!("2024-1-01"),
        json!("10000-01-01"),
        json!("-0001-01-01"),
        json!("2024-01-01Z"),
        json!("2024-01-01T00:00:00Z"),
        json!("not-a-date"),
    ] {
        for (constant, sample) in [
            (json!("2000-02-29"), invalid.clone()),
            (invalid.clone(), json!("2000-02-29")),
            (invalid.clone(), invalid.clone()),
            (invalid.clone(), json!("not-a-date")),
        ] {
            let cat = catalogue(
                json!({"type":"string","format":"date","const":constant}),
                &sample.to_string(),
                Limits::default(),
            );
            let report = selected(&cat);
            assert_eq!(
                const_status(&report),
                AssessmentStatus::Undetermined,
                "{constant} / {sample}"
            );
            assert!(
                !wire(&report)["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "sample.const")
            );
        }
    }
}

#[test]
fn wrong_kinds_keep_independent_type_differences_without_calendar_proof() {
    for value in [json!(true), json!(42), json!([]), json!({})] {
        let cat = catalogue(
            json!({"type":"string","format":"date","const":"2000-02-29"}),
            &value.to_string(),
            Limits::default(),
        );
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert_eq!(const_status(&report), AssessmentStatus::Assessed);
        assert!(
            !wire(&report)["result"]["meaning"]["const"]["basis"]
                .as_array()
                .unwrap()
                .iter()
                .any(|b| b["citation"]
                    .as_str()
                    .is_some_and(|s| s.contains("ADR 0018")))
        );
    }
}

fn date_items() -> Value {
    json!({"type":"array","uniqueItems":true,"items":{
        "sdfChoice":{"date":{"type":"string","format":"date"}}
    }})
}

#[test]
fn date_uniqueness_proves_distinct_values_and_retains_independent_duplicates() {
    for (sample, admission, complete, duplicate) in [
        (
            r#"["2000-02-29","2000-03-01"]"#,
            Admission::Admitted,
            true,
            false,
        ),
        (
            r#"["2000-03-01","2000-02-29"]"#,
            Admission::Admitted,
            true,
            false,
        ),
        (
            r#"["2000-02-29","\u0032000-02-29"]"#,
            Admission::Rejected,
            true,
            true,
        ),
        (
            r#"[null,"2000-02-29","2000-03-01"]"#,
            Admission::Admitted,
            true,
            false,
        ),
        (
            r#"[null,"2000-02-29",null]"#,
            Admission::Rejected,
            true,
            true,
        ),
        (r#"["2000-02-29","bad"]"#, Admission::Rejected, false, false),
        (r#"["bad","2000-02-29"]"#, Admission::Rejected, false, false),
        (
            r#"["2000-02-29","bad","2000-02-29"]"#,
            Admission::Rejected,
            true,
            true,
        ),
        ("[]", Admission::Admitted, true, false),
        (r#"["2000-02-29"]"#, Admission::Admitted, true, false),
    ] {
        let cat = catalogue(date_items(), sample, Limits::default());
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            admission,
            "{sample}"
        );
        assert_eq!(report.outcome().complete(), complete, "{sample}");
        assert_eq!(
            wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "sample.unique"),
            duplicate,
            "{sample}"
        );
    }
}

#[test]
fn model_date_capability_projects_full_contexts_and_keeps_other_meanings_unfinished() {
    for (items, expected) in [
        (
            json!({"type":"object","properties":{"stamp":{"sdfRef":"#/sdfData/Date"}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"sdfChoice":{"a":{"sdfRef":"#/sdfData/Date","nullable":false},
            "b":{"sdfRef":"#/sdfData/Date","nullable":true}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"sdfChoice":{"outer":{"sdfChoice":{"date":{"sdfRef":"#/sdfData/Date"}}}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"type":"string","format":"date"}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"sdfChoice":{"date":{"type":"string","format":"date","nullable":"unknown"}}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"sdfChoice":{"date":{"type":"string","format":"date","future:quality":true}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"sdfChoice":{"date":{"type":"string","format":"date"},
            "text":{"type":"string"}}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"sdfChoice":{"instant":{"type":"string","format":"date-time"}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"sdfChoice":{"clock":{"type":"string","format":"time"}}}),
            AssessmentStatus::Undetermined,
        ),
    ] {
        let model = json!({"info":{},"sdfData":{
            "Date":{"type":"string","format":"date"},
            "A":{"type":"array","uniqueItems":true,"items":items}
        }})
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
    }
}

#[test]
fn date_advice_uses_the_shared_comparator_without_inserting_defaults() {
    for (constant, default, warnings, status) in [
        ("2000-02-29", "2000-02-29", 0, AssessmentStatus::Assessed),
        ("2000-02-29", "2000-03-01", 1, AssessmentStatus::Assessed),
        ("2000-03-01", "2000-02-29", 1, AssessmentStatus::Assessed),
        (
            "1900-02-29",
            "2000-02-29",
            0,
            AssessmentStatus::Undetermined,
        ),
    ] {
        let cat = catalogue(
            json!({"type":"string","format":"date","const":constant,
            "default":default}),
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
        if let Some(finding) = findings.first() {
            assert_eq!(finding["severity"], "warning");
            assert_eq!(finding["basis"]["kind"], "advice");
            assert_eq!(finding["primary"]["pointer"], "/sdfProperty/p/default");
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
        json!({"type":"object","required":["stamp"],"properties":{
            "stamp":{"type":"string","format":"date","default":"2000-02-29"}
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
fn all_declaring_selectors_compare_original_and_typed_date_samples() {
    let date = json!({"type":"string","format":"date","const":"2000-02-29"});
    let model = json!({"info":{},"sdfData":{"D":date},"sdfProperty":{"p":date},
        "sdfAction":{"a":{"sdfInputData":date,"sdfOutputData":date}},
        "sdfEvent":{"e":{"sdfOutputData":date}}})
    .to_string();
    for typed in [false, true] {
        for (text, expected) in [
            ("2000-02-29", Admission::Admitted),
            ("2000-03-01", Admission::Rejected),
        ] {
            let token = json!(text).to_string();
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
                assert_eq!(report.sample_result().unwrap().admission(), expected);
                assert!(report.outcome().complete());
                assert!(report.to_human().contains(expected.as_str()));
                let value = wire(&report);
                assert_eq!(value["version"], "1.1");
                assert_eq!(value["coverage"]["requested"]["total"], 9);
                if expected == Admission::Rejected {
                    let finding = value["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|d| d["code"] == "sample.const")
                        .unwrap();
                    assert_eq!(finding["primary"]["pointer"], "");
                    assert_eq!(finding["primary"]["span"].is_null(), typed);
                }
            }
        }
    }
}

#[test]
fn enum_dates_reduce_to_a_complete_match_or_established_exclusion() {
    for (sample, expected) in [
        (r#""\u0032000-03-01""#, Admission::Admitted),
        (r#""2000-03-02""#, Admission::Rejected),
        (r#""1900-02-29""#, Admission::Rejected),
    ] {
        let cat = catalogue(
            json!({"type":"string","format":"date",
            "enum":["2000-02-29","2000-03-01"]}),
            sample,
            Limits::default(),
        );
        let report = selected(&cat);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert_eq!(
            const_status(&report),
            if sample.contains("1900") {
                AssessmentStatus::Undetermined
            } else {
                AssessmentStatus::Assessed
            }
        );
        if expected == Admission::Admitted {
            assert!(
                report
                    .sample_result()
                    .unwrap()
                    .meaning()
                    .const_values()
                    .limitations()
                    .next()
                    .is_none()
            );
            assert!(report.outcome().complete());
        }
    }
}

#[test]
fn date_alternatives_and_format_patches_preserve_common_interpretation() {
    for (branches, expected) in [
        (
            json!({"a":{"format":"date"},"b":{"format":"date"}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"a":{"format":"date"},"b":{}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{},"b":{"format":"date"}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"format":"date"},"b":{"format":"time"}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"format":"time"},"b":{"format":"date"}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"a":{"format":"date"},"b":{"format":null}}),
            AssessmentStatus::Undetermined,
        ),
    ] {
        let cat = catalogue(
            json!({"type":"string","const":"2000-02-29","sdfChoice":branches}),
            r#""2000-03-01""#,
            Limits::default(),
        );
        assert_eq!(const_status(&selected(&cat)), expected);
    }
    for (consumer, expected) in [
        (
            json!({"sdfRef":"#/sdfData/D","format":null}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"type":"string","format":null,"const":"2000-02-29"}),
            AssessmentStatus::Undetermined,
        ),
    ] {
        let model = json!({"info":{},"sdfData":{"D":{
            "type":"string","format":"date","const":"2000-02-29"
        }},"sdfProperty":{"p":consumer}})
        .to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""not-a-date""#).sample(),
        ])
        .unwrap();
        assert_eq!(const_status(&selected(&cat)), expected);
    }
}

#[test]
fn independent_text_exclusions_do_not_remove_date_difference() {
    for restriction in [json!({"maxLength":1}), json!({"pattern":"^x$"})] {
        let mut description = json!({"type":"string","format":"date","const":"2000-02-29"});
        description
            .as_object_mut()
            .unwrap()
            .extend(restriction.as_object().unwrap().clone());
        let cat = catalogue(description, r#""2000-03-01""#, Limits::default());
        let report = selected(&cat);
        assert_eq!(const_status(&report), AssessmentStatus::Assessed);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(report.outcome().complete());
        assert!(report.outcome().errors() >= 2);
    }
}

#[test]
fn nested_date_difference_retains_its_decisive_leaf_and_inherited_origin() {
    let model = json!({"info":{},"sdfData":{"Date":{"type":"string","format":"date"}},
        "sdfProperty":{"p":{"type":"object","const":{"stamp":"2000-02-29","other":"2030-06-30T23:59:60Z"},
            "properties":{"stamp":{"sdfRef":"#/sdfData/Date"},
                "other":{"type":"string","format":"date-time"}}}}}).to_string();
    let sample = br#"{"stamp":"2000-03-01","other":"2030-06-30T23:59:60Z"}"#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", sample).sample(),
    ])
    .unwrap();
    let report = selected(&cat);
    assert_eq!(const_status(&report), AssessmentStatus::Assessed);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(!report.outcome().complete());
    let value = wire(&report);
    let finding = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "sample.const")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "/stamp");
    assert_eq!(finding["primary"]["span"], json!([9, 21]));
    assert!(
        finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["pointer"] == "/sdfProperty/p/const/stamp")
    );
    let proof = &value["result"]["meaning"]["const"];
    assert!(proof["limitations"].as_array().unwrap().is_empty());
    assert!(proof["basis"].as_array().unwrap().iter().any(|b| {
        b["citation"]
            .as_str()
            .is_some_and(|s| s.contains("ADR 0018"))
    }));
}

#[test]
fn date_comparisons_share_pair_and_work_allowances_and_preserve_omitted_details() {
    for sample in [
        r#"["2000-02-29","2000-03-01","2000-03-02"]"#,
        r#"["2000-03-02","2000-03-01","2000-02-29"]"#,
    ] {
        for (pairs, expected) in [(2, Admission::Undetermined), (3, Admission::Admitted)] {
            let cat = catalogue(
                date_items(),
                sample,
                Limits::default().equality_pairs(pairs),
            );
            for _ in 0..2 {
                let report = selected(&cat);
                assert_eq!(report.sample_result().unwrap().admission(), expected);
                assert_eq!(report.outcome().complete(), pairs == 3);
            }
        }
    }
    let description = json!({"type":"string","format":"date","const":"2000-02-29"});
    for configure in [
        Limits::decoding_work as fn(Limits, usize) -> Limits,
        Limits::effective_allocation_bytes,
    ] {
        let assessed = |allowance| {
            let cat = catalogue(
                description.clone(),
                r#""2000-03-01""#,
                configure(Limits::default(), allowance),
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
        let cat = catalogue(
            description.clone(),
            r#""2000-03-01""#,
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
    }
    for limits in [
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
        Limits::default().pointer_bytes(1).pointer_output_bytes(1),
    ] {
        let cat = catalogue(description.clone(), r#""2000-03-01""#, limits);
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert_eq!(report.outcome().errors(), 1);
        assert!(report.outcome().complete());
    }
}

#[test]
fn restricted_date_nullability_donors_need_an_establishing_full_override() {
    for (branch, expected) in [
        (json!({"format":"date"}), AssessmentStatus::Undetermined),
        (
            json!({"format":"date","nullable":true}),
            AssessmentStatus::Assessed,
        ),
    ] {
        let model = json!({"info":{},"sdfData":{"A":{"type":"array","uniqueItems":true,
            "items":{"type":"string","nullable":false,"sdfChoice":{"date":branch}}
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
            expected
        );
    }
}

#[test]
fn compound_date_defaults_produce_one_decisive_advisory_exclusion() {
    let cat = catalogue(
        json!({"type":"object",
            "const":{"stamp":"2000-02-29","later":"bad"},
            "default":{"stamp":"2000-03-01","later":"bad"},
            "properties":{"stamp":{"type":"string","format":"date"},
                "later":{"type":"string","format":"future-format"}}
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
}

#[test]
fn nested_array_date_constants_keep_the_actual_mismatch_location() {
    let cat = catalogue(
        json!({"type":"array","const":["2000-02-29","2000-03-01"],
            "items":{"sdfChoice":{"date":{"type":"string","format":"date"}}}
        }),
        r#"["2000-02-29","2000-03-02"]"#,
        Limits::default(),
    );
    let report = selected(&cat);
    assert_eq!(const_status(&report), AssessmentStatus::Assessed);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    let value = wire(&report);
    let finding = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "sample.const")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "/1");
    assert!(
        finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["pointer"] == "/sdfProperty/p/const/1")
    );
}
