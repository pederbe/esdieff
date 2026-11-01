//! Null equality in established format contexts follows ADR 0015.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, JsonBuilder, Limits, Report,
    SampleKind, SampleRequest, SourceInput,
};
use serde_json::{Value, json};

const FORMATS: [&str; 6] = ["date", "date-time", "time", "uri", "uri-reference", "uuid"];

fn catalogue(qualities: Value, sample: &str, limits: Limits) -> Catalogue {
    let model = json!({"info":{}, "sdfProperty":{"p": qualities}}).to_string();
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

const VALUES: [(&str, &str, &str); 6] = [
    ("date", "2000-02-29", "1900-02-29"),
    ("date-time", "2000-01-02T03:04:05Z", "not-a-date-time"),
    ("time", "03:04:05Z", "24:00:00Z"),
    ("uri", "https://example.test/device", "../device"),
    ("uri-reference", "../device", "device%GG"),
    ("uuid", "f81d4fae-7dec-11d0-a765-00a0c91e6bf6", "not-a-uuid"),
];

#[test]
fn wrong_conventional_kinds_do_not_supply_formatted_null_differences() {
    for format in FORMATS {
        for nonnull in [json!(true), json!(42), json!([]), json!({})] {
            for null_sample in [false, true] {
                let (constant, sample) = if null_sample {
                    (nonnull.clone(), "null".to_owned())
                } else {
                    (Value::Null, nonnull.to_string())
                };
                let cat = catalogue(
                    json!({"type":"string","format":format,"const":constant}),
                    &sample,
                    Limits::default(),
                );
                let report = selected(&cat);
                assert_eq!(
                    const_status(&report),
                    AssessmentStatus::Undetermined,
                    "{format}: {nonnull}, null_sample={null_sample}"
                );
                assert!(!report.outcome().complete());
                assert!(
                    !wire(&report)["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["code"] == "sample.const")
                );
                if !null_sample {
                    assert_eq!(
                        report.sample_result().unwrap().admission(),
                        Admission::Rejected
                    );
                    assert!(
                        wire(&report)["diagnostics"]["items"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|d| d["code"] == "sample.type")
                    );
                }
            }
        }
    }
}

#[test]
fn null_and_denoting_formatted_values_supply_symmetric_const_differences() {
    for (format, valid, _) in VALUES {
        for null_sample in [false, true] {
            let (constant, sample) = if null_sample {
                (json!(valid), "null".to_owned())
            } else {
                (Value::Null, json!(valid).to_string())
            };
            let cat = catalogue(
                json!({"type":"string", "format":format, "const":constant}),
                &sample,
                Limits::default(),
            );
            let report = selected(&cat);
            assert_eq!(
                const_status(&report),
                AssessmentStatus::Assessed,
                "{format} {null_sample}"
            );
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert!(report.outcome().complete());
            let value = wire(&report);
            let finding = value["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["code"] == "sample.const")
                .unwrap();
            assert_eq!(finding["primary"]["pointer"], "");
            assert_eq!(finding["primary"]["span"], json!([0, sample.len()]));
            assert!(
                value["result"]["meaning"]["const"]["limitations"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn invalid_and_uninterpreted_nonnull_operands_never_prove_const_difference() {
    let cases = VALUES
        .into_iter()
        .map(|(format, _, invalid)| (format, invalid))
        .chain([
            ("time", "23:59:60Z"),
            ("date-time", "2030-06-30T23:59:60Z"),
            ("uri", "custom:device"),
        ]);
    for (format, token) in cases {
        for null_sample in [false, true] {
            let (constant, sample) = if null_sample {
                (json!(token), "null".to_owned())
            } else {
                (Value::Null, json!(token).to_string())
            };
            let cat = catalogue(
                json!({"type":"string", "format":format, "const":constant}),
                &sample,
                Limits::default(),
            );
            let report = selected(&cat);
            assert_eq!(
                const_status(&report),
                AssessmentStatus::Undetermined,
                "{format} {token} {null_sample}"
            );
            assert!(!report.outcome().complete());
            let value = wire(&report);
            assert!(
                !value["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "sample.const")
            );
            assert!(
                !value["result"]["meaning"]["const"]["limitations"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            if format == "date-time" && token.starts_with("2030") {
                assert!(value["baseline"].get("temporal_knowledge").is_some());
            }
        }
    }
}

#[test]
fn restricted_direct_items_do_not_gain_formatted_null_meaning() {
    for format in FORMATS {
        let cat = catalogue(
            json!({"type":"array", "uniqueItems":true, "items":{"type":"string", "format":format}}),
            "[null,null]",
            Limits::default(),
        );
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined,
            "{format}\n{}",
            report.to_json()
        );
        assert!(
            !wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "sample.unique")
        );
    }
}

fn unique_description(format: &str) -> Value {
    json!({"type":"array", "uniqueItems":true, "items":{"sdfChoice":{"value":{"type":"string", "format":format}}}})
}

#[test]
fn full_formatted_items_compare_null_duplicates_and_valid_mixed_pairs() {
    for (format, valid, _) in VALUES {
        for (values, expected) in [
            (json!([null, null]), Admission::Rejected),
            (json!([null, valid]), Admission::Admitted),
            (json!([valid, null]), Admission::Admitted),
        ] {
            let sample = values.to_string();
            let cat = catalogue(unique_description(format), &sample, Limits::default());
            let report = selected(&cat);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{format}\n{}",
                report.to_json()
            );
            assert!(report.outcome().complete());
            if expected == Admission::Rejected {
                let value = wire(&report);
                let finding = value["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|d| d["code"] == "sample.unique")
                    .unwrap();
                assert_eq!(finding["primary"]["pointer"], "/1");
                assert!(
                    finding["related"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|r| r["pointer"] == "/0")
                );
            }
        }
    }
    let a = VALUES[5].1;
    let escaped = format!(r#"[null,"{a}","\u006681d4fae-7dec-11d0-a765-00a0c91e6bf6"]"#);
    let cat = catalogue(unique_description("uuid"), &escaped, Limits::default());
    let report = selected(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    let value = wire(&report);
    let finding = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "sample.unique")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "/2");
    assert!(
        finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["pointer"] == "/1")
    );
    for (values, expected, complete, duplicate) in [
        (
            json!([null, a, a.to_uppercase()]),
            Admission::Rejected,
            true,
            true,
        ),
        (
            json!([null, a, "00000000-0000-0000-0000-000000000000"]),
            Admission::Admitted,
            true,
            false,
        ),
        (json!([null, "bad", null]), Admission::Rejected, true, true),
        (json!([null, "bad"]), Admission::Rejected, false, false),
    ] {
        let cat = catalogue(
            unique_description("uuid"),
            &values.to_string(),
            Limits::default(),
        );
        let report = selected(&cat);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert_eq!(report.outcome().complete(), complete);
        assert_eq!(
            wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "sample.unique"),
            duplicate
        );
    }
    let cat = catalogue(
        json!({"type":"object", "properties":{"ids":unique_description("uuid"), "later":{"type":"string","format":"future-format"}}}),
        r#"{"ids":[null,"bad",null],"later":"x"}"#,
        Limits::default(),
    );
    let report = selected(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(!report.outcome().complete());
}

#[test]
fn choices_and_compounds_keep_common_meaning_and_independent_differences() {
    for branches in [
        json!({"one":{"format":"uuid"},"two":{"sdfChoice":{"inner":{"format":"uuid"}}}}),
        json!({"two":{"sdfChoice":{"inner":{"format":"uuid"}}},"one":{"format":"uuid"}}),
    ] {
        let cat = catalogue(
            json!({"type":"string","const":null,"sdfChoice":branches}),
            "null",
            Limits::default(),
        );
        assert_eq!(const_status(&selected(&cat)), AssessmentStatus::Assessed);
    }
    for branches in [
        json!({"known":{"format":"uuid"},"unknown":{"format":"future-format"}}),
        json!({"unknown":{"format":"future-format"},"known":{"format":"uuid"}}),
        json!({"id":{"format":"uuid"},"address":{"format":"uri"}}),
    ] {
        let cat = catalogue(
            json!({"type":"string","const":null,"sdfChoice":branches}),
            "null",
            Limits::default(),
        );
        assert_eq!(
            const_status(&selected(&cat)),
            AssessmentStatus::Undetermined
        );
    }
    for format in FORMATS {
        let field = json!({"type":"string","format":format});
        let description =
            json!({"type":"object","const":{"value":null},"properties":{"value":field}});
        let cat = catalogue(description, r#"{"value":null}"#, Limits::default());
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        assert_eq!(const_status(&report), AssessmentStatus::Assessed);
    }
    let cat = catalogue(
        json!({"type":"object","const":{"value":null},"properties":{"value":{"type":"string","format":"uuid"}}}),
        r#"{"value":"00000000-0000-0000-0000-000000000000"}"#,
        Limits::default(),
    );
    let report = selected(&cat);
    let value = wire(&report);
    let finding = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "sample.const")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "/value");
    for sample in [
        r#"{"value":"bad","extra":1}"#,
        r#"{"extra":1,"value":"bad"}"#,
    ] {
        let cat = catalogue(
            json!({"type":"object","const":{"value":null},"properties":{"value":{"type":"string","format":"uuid"}}}),
            sample,
            Limits::default(),
        );
        let report = selected(&cat);
        assert_eq!(const_status(&report), AssessmentStatus::Assessed);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
    }
    let cat = catalogue(
        json!({"type":"string","format":"uuid","const":null,"maxLength":1}),
        &json!(VALUES[5].1).to_string(),
        Limits::default(),
    );
    let report = selected(&cat);
    assert_eq!(const_status(&report), AssessmentStatus::Assessed);
    assert!(
        wire(&report)["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "sample.length")
    );
}

#[test]
fn declared_value_advice_uses_null_equality_without_inserting_defaults() {
    for (format, valid, _) in VALUES {
        for (constant, default) in [(Value::Null, json!(valid)), (json!(valid), Value::Null)] {
            let cat = catalogue(
                json!({"type":"string","format":format,"const":constant,"default":default}),
                "null",
                Limits::default(),
            );
            let report = cat
                .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
                .unwrap();
            for area in ["const-consistency/1", "default-consistency/1"] {
                assert_eq!(
                    report.assessment_area_status(area).require().unwrap(),
                    AssessmentStatus::Assessed,
                    "{format}"
                );
            }
            let value = wire(&report);
            let findings = value["diagnostics"]["items"].as_array().unwrap();
            assert_eq!(
                findings
                    .iter()
                    .filter(|d| d["code"] == "consistency.default_excluded")
                    .count(),
                1
            );
            assert!(
                !findings
                    .iter()
                    .any(|d| d["code"] == "consistency.const_excluded")
            );
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
            json!({"type":"string","format":format,"nullable":false,"const":null}),
            "null",
            Limits::default(),
        );
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        assert_eq!(
            wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == "consistency.const_excluded")
                .count(),
            1
        );
    }
    let cat = catalogue(
        json!({"type":"string","format":"uuid","const":"bad","default":null}),
        "null",
        Limits::default(),
    );
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert!(!report.outcome().complete());
    assert!(
        !wire(&report)["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "consistency.default_excluded")
    );
    let cat = catalogue(
        json!({"type":"object","required":["missing"],"properties":{"missing":{"type":"string","format":"uuid","default":null}}}),
        "{}",
        Limits::default(),
    );
    assert_eq!(
        selected(&cat).sample_result().unwrap().admission(),
        Admission::Rejected
    );
}

#[test]
fn typed_operands_and_inherited_constraints_retain_actual_evidence() {
    for format in FORMATS {
        let model =
            json!({"sdfProperty":{"p":{"type":"string","format":format,"const":null}}}).to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::typed("s", JsonBuilder::new().null().unwrap()),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        let value = wire(&report);
        assert!(
            value["baseline"]["format_knowledge"]
                .as_array()
                .is_none_or(Vec::is_empty)
        );
        let meaning = report.sample_result().unwrap().meaning().const_values();
        assert_eq!(
            meaning.basis().len(),
            value["result"]["meaning"]["const"]["basis"]
                .as_array()
                .unwrap()
                .len()
        );
        assert!(meaning.limitations().next().is_none());
    }
    let model = json!({"sdfData":{"U":{"type":"string","format":"uuid","const":null}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/U"}}}).to_string();
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::typed("s", JsonBuilder::new().text(VALUES[5].1).unwrap()),
    ])
    .unwrap();
    let report = selected(&cat);
    let value = wire(&report);
    let finding = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "sample.const")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "");
    assert!(finding["primary"]["span"].is_null());
    assert!(
        finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "source" && r["pointer"] == "/sdfData/U/const")
    );
    for (patch, expected) in [
        (true, AssessmentStatus::Assessed),
        (false, AssessmentStatus::Undetermined),
    ] {
        let property = if patch {
            json!({"sdfRef":"#/sdfData/U", "format":null})
        } else {
            json!({"type":"string", "format":null, "const":null})
        };
        let model = json!({"sdfData":{"U":{"type":"string","format":"uuid","const":null}},"sdfProperty":{"p":property}}).to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", b"null").sample(),
        ])
        .unwrap();
        assert_eq!(const_status(&selected(&cat)), expected);
    }
}

#[test]
fn null_constants_complete_in_all_formats_independently_of_null_admission() {
    for format in FORMATS {
        for nullable in [None, Some(true), Some(false)] {
            let mut description = json!({"type":"string", "format":format, "const":null});
            if let Some(nullable) = nullable {
                description["nullable"] = nullable.into();
            }
            let cat = catalogue(description, "null", Limits::default());
            let report = selected(&cat);
            assert_eq!(
                const_status(&report),
                AssessmentStatus::Assessed,
                "{format}"
            );
            let expected = if nullable == Some(false) {
                Admission::Rejected
            } else {
                Admission::Admitted
            };
            assert_eq!(report.sample_result().unwrap().admission(), expected);
            assert!(report.outcome().complete());
            let value = wire(&report);
            assert_eq!(value["result"]["admission"], expected.as_str());
            assert!(
                value["baseline"]["interpretations"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(value["baseline"].get("temporal_knowledge").is_none());
            assert!(
                report
                    .to_human()
                    .contains("Sample area sample-const/1: assessed")
            );
            let full = cat
                .check(CheckRequest::new().assessment(Assessment::FullBaseline))
                .unwrap();
            assert_eq!(
                full.requirement_status("rfc9880.model.M043/1")
                    .require()
                    .unwrap(),
                AssessmentStatus::Assessed
            );
        }
    }
}

#[test]
fn all_declaring_selectors_use_formatted_null_equality() {
    let description = json!({"type":"string","format":"uuid","const":null});
    let model = json!({"sdfData":{"D":description},"sdfProperty":{"p":description},
        "sdfAction":{"a":{"sdfInputData":description,"sdfOutputData":description}},
        "sdfEvent":{"e":{"sdfOutputData":description}}})
    .to_string();
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", b"null").sample(),
    ])
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
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        assert!(report.outcome().complete());
        assert_eq!(const_status(&report), AssessmentStatus::Assessed);
    }
}

#[test]
fn formatted_null_pairs_share_request_limits_without_reusing_prior_verdicts() {
    let a = VALUES[5].1;
    let b = "00000000-0000-0000-0000-000000000000";
    for values in [json!([null, a, b]), json!([b, a, null])] {
        for (pairs, expected) in [(2, Admission::Undetermined), (3, Admission::Admitted)] {
            let cat = catalogue(
                unique_description("uuid"),
                &values.to_string(),
                Limits::default().equality_pairs(pairs),
            );
            for _ in 0..2 {
                let report = selected(&cat);
                assert_eq!(report.sample_result().unwrap().admission(), expected);
                assert_eq!(report.outcome().complete(), pairs == 3);
                assert!(
                    !wire(&report)["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["code"] == "sample.unique")
                );
            }
        }
    }
    let description = json!({"type":"string","format":"uri","const":null});
    let sample = json!(format!("https://example.test/{}", "a".repeat(1024))).to_string();
    for configure in [
        Limits::decoding_work as fn(Limits, usize) -> Limits,
        Limits::effective_allocation_bytes,
    ] {
        let assessed = |allowance| {
            let cat = catalogue(
                description.clone(),
                &sample,
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
    }
    for limits in [
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
        Limits::default().pointer_bytes(1).pointer_output_bytes(1),
    ] {
        let cat = catalogue(unique_description("uuid"), "[null,null]", limits);
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
fn nullable_uuid_model_capability_projects_final_full_contexts_and_keeps_guards() {
    for (items, expected) in [
        (
            json!({"type":"object","properties":{"id":{"sdfRef":"#/sdfData/U"}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"sdfChoice":{"one":{"sdfRef":"#/sdfData/U","nullable":false},"two":{"sdfRef":"#/sdfData/U","nullable":true}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"sdfChoice":{"outer":{"sdfChoice":{"one":{"sdfRef":"#/sdfData/U"}}}}}),
            AssessmentStatus::Assessed,
        ),
        (
            json!({"type":"string","format":"uuid"}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"sdfChoice":{"id":{"type":"string","format":"uuid","nullable":"unknown"}}}),
            AssessmentStatus::Undetermined,
        ),
        (
            json!({"sdfChoice":{"id":{"type":"string","format":"uuid","future:quality":true}}}),
            AssessmentStatus::Assessed,
        ),
    ] {
        let model = json!({"info":{},"sdfData":{"U":{"type":"string","format":"uuid"},"A":{"type":"array","uniqueItems":true,"items":items}}}).to_string();
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
fn restricted_uuid_nullability_donors_need_an_explicit_full_override() {
    for (branch, expected) in [
        (json!({"format":"uuid"}), AssessmentStatus::Undetermined),
        (
            json!({"format":"uuid","nullable":true}),
            AssessmentStatus::Assessed,
        ),
    ] {
        let model = json!({"info":{},"sdfData":{"A":{"type":"array","uniqueItems":true,
            "items":{"type":"string","nullable":false,"sdfChoice":{"id":branch}}}}})
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
fn final_const_witness_discards_temporal_proof_but_keeps_actual_consultation() {
    let cat = catalogue(
        json!({"type":"object","const":{"stamp":null,"flag":false},
        "properties":{"stamp":{"type":"string","format":"date-time"},"flag":{"type":"boolean"}}}),
        r#"{"stamp":"2030-06-30T23:59:60Z","flag":true}"#,
        Limits::default(),
    );
    let report = selected(&cat);
    assert_eq!(const_status(&report), AssessmentStatus::Assessed);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(!report.outcome().complete());
    let value = wire(&report);
    let meaning = &value["result"]["meaning"]["const"];
    assert!(meaning["limitations"].as_array().unwrap().is_empty());
    assert!(!meaning["basis"].as_array().unwrap().iter().any(|b| {
        b["citation"]
            .as_str()
            .is_some_and(|s| s.contains("ADR 0018"))
    }));
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
    let finding = value["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "sample.const")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "/flag");
    assert!(
        value["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "ADR0018-selected-representation")
    );
    assert!(value["baseline"].get("temporal_knowledge").is_some());
    assert!(
        !value["result"]["meaning"]["abstract_values"]["limitations"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let cat = catalogue(
        json!({"sdfChoice":{
        "a":{"type":"string","format":"date-time","const":"2030-06-30T23:59:60Z"},
        "b":{"type":"string","format":"date-time","const":null}}}),
        "null",
        Limits::default(),
    );
    let report = selected(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert!(report.outcome().complete());
    let value = wire(&report);
    assert!(
        value["result"]["meaning"]["const"]["limitations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !value["result"]["meaning"]["const"]["basis"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["citation"]
                .as_str()
                .is_some_and(|s| s.contains("ADR 0018")))
    );
    assert!(
        value["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "ADR0018-selected-representation")
    );
    assert!(value["baseline"].get("temporal_knowledge").is_some());
    assert!(
        value["baseline"]["format_knowledge"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
