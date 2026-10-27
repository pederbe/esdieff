//! Ordinary value admission follows ADR0016: Selected samples with explicit admission and coverage,
//! and ADR0022: Scalar constraints and characteristic states.
mod location_support;
use esdieff::{
    Admission, AssessmentStatus, Catalogue, JsonBuilder, Limits, Report, SampleKind, SampleRequest,
    SourceInput,
};

fn selected<'a>(catalogue: &'a Catalogue) -> Report<'a> {
    selected_at(catalogue, SampleKind::Property, "/sdfProperty/p")
}

fn selected_at<'a>(catalogue: &'a Catalogue, kind: SampleKind, pointer: &str) -> Report<'a> {
    catalogue
        .check_sample(
            SampleRequest::new(catalogue.source_id("m").unwrap(), kind, pointer)
                .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap()
}

#[test]
fn ordinary_text_and_boolean_values_use_the_shared_sample_admission() {
    for (kind, sample, expected) in [
        ("string", &br#""on""#[..], Admission::Admitted),
        ("string", &b"true"[..], Admission::Rejected),
        ("boolean", &b"true"[..], Admission::Admitted),
        ("boolean", &b"false"[..], Admission::Admitted),
        ("boolean", &br#""false""#[..], Admission::Rejected),
        ("string", &b"null"[..], Admission::Admitted),
        ("boolean", &b"null"[..], Admission::Admitted),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{"type":"{kind}"}}}}}}"#);
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{kind}"
        );
        assert!(report.outcome().complete());
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-type-null/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        assert_eq!(report.requested_coverage().total(), Some(9));
    }
}

#[test]
fn text_lengths_are_inclusive_unicode_scalar_counts_with_exact_bounds() {
    for (lengths, sample, expected) in [
        (
            r#""minLength":1,"maxLength":1"#,
            r#""é""#,
            Admission::Admitted,
        ),
        (
            r#""minLength":1,"maxLength":1"#,
            r#""𝄞""#,
            Admission::Admitted,
        ),
        (
            r#""minLength":2,"maxLength":2"#,
            r#""e\u0301""#,
            Admission::Admitted,
        ),
        (r#""maxLength":1"#, r#""e\u0301""#, Admission::Rejected),
        (
            r#""minLength":4,"maxLength":4"#,
            r#""é𝄞e\u0301""#,
            Admission::Admitted,
        ),
        (
            r#""minLength":2.0,"maxLength":2e0"#,
            r#""on""#,
            Admission::Admitted,
        ),
        (
            r#""minLength":0,"maxLength":0"#,
            r#""""#,
            Admission::Admitted,
        ),
        (r#""minLength":1"#, r#""""#, Admission::Rejected),
        (r#""maxLength":0"#, r#""x""#, Admission::Rejected),
        (r#""maxLength":1e1000000"#, r#""x""#, Admission::Admitted),
        (r#""minLength":1e1000000"#, r#""x""#, Admission::Rejected),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{"type":"string",{lengths}}}}}}}"#);
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{lengths}, {sample}"
        );
        assert!(report.outcome().complete());
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-text-boolean/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
    }
}

#[test]
fn independent_text_rejection_preserves_unfinished_pattern_and_equality_obligations() {
    for (extra, expected, text_status, complete) in [
        (
            r#""pattern":"^z$""#,
            Admission::Rejected,
            AssessmentStatus::Assessed,
            true,
        ),
        (
            r#""pattern":"^z$","maxLength":1"#,
            Admission::Rejected,
            AssessmentStatus::Assessed,
            true,
        ),
        (
            r#""const":"on","maxLength":1"#,
            Admission::Rejected,
            AssessmentStatus::Assessed,
            true,
        ),
        (
            r#""const":"on","title":"t","maxLength":1"#,
            Admission::Rejected,
            AssessmentStatus::Assessed,
            false,
        ),
        // Undeclared extension vocabulary leaves the equality obligation finished,
        // following ADR 0037: Undeclared extension vocabulary.
        (
            r#""const":"on","unknownEqualityQuality":true,"maxLength":1"#,
            Admission::Rejected,
            AssessmentStatus::Assessed,
            true,
        ),
        (
            r#""format":"future-format","maxLength":1"#,
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
            false,
        ),
        (
            r#""sdfChoice":{"wide":{"maxLength":10}},"maxLength":1"#,
            Admission::Admitted,
            AssessmentStatus::Assessed,
            true,
        ),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{"type":"string",{extra}}}}}}}"#);
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""on""#).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{extra}"
        );
        assert_eq!(report.outcome().complete(), complete, "{extra}");
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-text-boolean/1")
                .require()
                .unwrap(),
            text_status,
            "{extra}"
        );
        assert_eq!(
            report.outcome().errors(),
            if extra == r#""pattern":"^z$","maxLength":1"# {
                2
            } else {
                u32::from(expected == Admission::Rejected)
            }
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let text_row = wire["coverage"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "sample-text-boolean/1")
            .unwrap();
        if text_status == AssessmentStatus::Assessed {
            assert_eq!(text_row["basis"]["citation"], "RFC 9880 Appendix C.2");
        } else {
            assert_eq!(
                text_row["basis"]["id"],
                if text_status == AssessmentStatus::Unassessed {
                    "sample-implementation"
                } else {
                    "sample-establishment"
                }
            );
        }
        for condition in report
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "sample.length")
        {
            assert_eq!(condition.basis().citation(), Some("RFC 9880 Appendix C.2"));
        }
    }
}

#[test]
fn nullable_remains_separate_from_const_and_documentary_default() {
    for (qualities, expected, complete) in [
        (
            r#""type":"string","nullable":false"#,
            Admission::Rejected,
            true,
        ),
        (
            r#""type":"boolean","nullable":false"#,
            Admission::Rejected,
            true,
        ),
        (
            r#""type":"string","minLength":1,"default":"on""#,
            Admission::Admitted,
            true,
        ),
        (
            r#""type":"boolean","default":true"#,
            Admission::Admitted,
            true,
        ),
        (
            r#""type":"string","const":"on","default":"on""#,
            Admission::Rejected,
            true,
        ),
        (
            r#""type":"boolean","const":true"#,
            Admission::Rejected,
            true,
        ),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{{qualities}}}}}}}"#);
        for input in [
            SourceInput::supplied("s", b"null").sample(),
            SourceInput::typed("s", JsonBuilder::new().null().unwrap()),
        ] {
            let catalogue =
                Catalogue::take_inputs(vec![SourceInput::supplied("m", model.as_bytes()), input])
                    .unwrap();
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{qualities}"
            );
            assert_eq!(report.outcome().complete(), complete);
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            let type_row = wire["coverage"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == "sample-type-null/1")
                .unwrap();
            assert_eq!(
                type_row["basis"]["citation"],
                "RFC 9880 Section 4.7 and Appendix C"
            );
            if expected == Admission::Admitted {
                let witness = wire["result"]["witnesses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|witness| witness["kind"] == "admission")
                    .unwrap();
                assert_eq!(witness["basis"]["citation"], "RFC 9880 Appendix C");
            }
            assert_eq!(
                report
                    .sample_result()
                    .unwrap()
                    .sample_value()
                    .require()
                    .unwrap()
                    .kind(),
                esdieff::JsonKind::Null
            );
        }
    }
}

#[test]
fn text_findings_preserve_original_and_typed_locations_and_bounded_counts() {
    let model =
        br#"{"sdfProperty":{"p":{"type":"string","minLength":3,"maxLength":1,"pattern":"z"}}}"#;
    for typed in [false, true] {
        let mut totals = Vec::new();
        for limits in [
            Limits::default(),
            Limits::default().conditions(1),
            Limits::default().diagnostic_bytes(1),
            Limits::default().report_bytes(12 + 2 * 4096 + 16384 + 163930),
            Limits::default()
                .conditions(1)
                .diagnostic_bytes(1)
                .pointer_bytes(1)
                .pointer_output_bytes(1)
                .report_bytes(12 + 2 * 4096 + 16384 + 163930),
        ] {
            let sample = if typed {
                SourceInput::typed("s", JsonBuilder::new().text("éx").unwrap())
            } else {
                SourceInput::supplied("s", "\"éx\"".as_bytes()).sample()
            };
            let catalogue = Catalogue::take_inputs_with_limits(
                vec![SourceInput::supplied("m", model), sample],
                limits,
            )
            .unwrap();
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert_eq!(report.outcome().errors(), 3);
            assert!(report.outcome().complete());
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["coverage"]["requested"]["total"], 9);
            totals.push(
                wire["diagnostics"]["retained"].as_u64().unwrap()
                    + wire["diagnostics"]["omitted"].as_u64().unwrap(),
            );
            assert_eq!(
                wire["result"]["sample_location"]["span"],
                if typed {
                    serde_json::Value::Null
                } else {
                    serde_json::json!([0, 5])
                }
            );
            for condition in wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["domain"] == "sample")
            {
                assert!(matches!(
                    condition["code"].as_str(),
                    Some("sample.length" | "sample.pattern")
                ));
                assert_eq!(condition["primary"], wire["result"]["sample_location"]);
                assert_eq!(condition["basis"]["citation"], "RFC 9880 Appendix C.2");
                assert!(condition["related"].as_array().unwrap().iter().any(
                    |related| related["kind"] == "source"
                        && (related["pointer"].as_str().unwrap().ends_with("Length")
                            || related["pointer"].as_str().unwrap().ends_with("pattern"))
                ));
            }
            assert!(report.to_human().contains("rejected"));
        }
        assert!(totals.iter().all(|total| *total == 3));
    }
}

#[test]
fn every_declaring_kind_reuses_effective_text_or_boolean_and_handle_selection() {
    let model = br##"{"sdfData":{"Text":{"type":"string","minLength":2,"maxLength":2},"Flag":{"type":"boolean","nullable":false}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Text","maxLength":1}},"sdfAction":{"a":{"sdfInputData":{"sdfRef":"#/sdfData/Text","maxLength":3},"sdfOutputData":{"sdfRef":"#/sdfData/Flag"}}},"sdfEvent":{"e":{"sdfOutputData":{"sdfRef":"#/sdfData/Flag"}}}}"##;
    for (kind, pointer, sample, expected) in [
        (
            SampleKind::Data,
            "/sdfData/Text",
            "\"éx\"",
            Admission::Admitted,
        ),
        (
            SampleKind::Property,
            "/sdfProperty/p",
            "\"éx\"",
            Admission::Rejected,
        ),
        (
            SampleKind::ActionInput,
            "/sdfAction/a",
            "\"éx\"",
            Admission::Admitted,
        ),
        (
            SampleKind::ActionOutput,
            "/sdfAction/a",
            "true",
            Admission::Admitted,
        ),
        (
            SampleKind::EventOutput,
            "/sdfEvent/e",
            "null",
            Admission::Rejected,
        ),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected_at(&catalogue, kind, pointer);
        let result = report.sample_result().unwrap();
        assert_eq!(result.admission(), expected, "{}", kind.as_str());
        assert!(report.outcome().complete());
        assert_eq!(result.selected_kind(), kind);
        let handle = result.description().require().unwrap();
        let from_handle = catalogue
            .check_sample(
                SampleRequest::from_description(handle)
                    .unwrap()
                    .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(from_handle.sample_result().unwrap().admission(), expected);
        assert_eq!(
            from_handle.sample_result().unwrap().selected_pointer(),
            pointer
        );
        if kind == SampleKind::Property {
            assert_eq!(handle.maximum_length().require().unwrap().lexeme(), "1");
            assert_eq!(handle.minimum_length().require().unwrap().lexeme(), "2");
        }
    }
}

#[test]
fn length_bounds_are_needed_only_for_an_ordinary_nonnull_string() {
    for (qualities, sample, expected) in [
        (
            r#""type":"boolean","minLength":"bad","pattern":false"#,
            "true",
            Admission::Admitted,
        ),
        (
            r#""type":"string","minLength":"bad","pattern":false"#,
            "null",
            Admission::Admitted,
        ),
        (
            r#""type":"string","minLength":"bad","pattern":false"#,
            "true",
            Admission::Rejected,
        ),
        (
            r#""type":"string","nullable":false,"maxLength":-1"#,
            "null",
            Admission::Rejected,
        ),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{{qualities}}}}}}}"#);
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete(), "{qualities}, {sample}");
        assert_eq!(
            report.sample_result().unwrap().context_findings().errors(),
            0
        );
        assert_eq!(
            report.outcome().errors(),
            u32::from(expected == Admission::Rejected)
        );
    }
}

#[test]
fn exact_length_comparisons_share_the_parser_numeric_remainder() {
    for (bounds, expected) in [
        (r#""maxLength":2"#, Admission::Admitted),
        (r#""minLength":2,"maxLength":2"#, Admission::Undetermined),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{"type":"string",{bounds}}}}}}}"#);
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", br#""on""#).sample(),
            ],
            Limits::default().numeric_work(25),
        )
        .unwrap();
        for _ in 0..2 {
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{bounds}"
            );
            assert_eq!(report.outcome().complete(), expected == Admission::Admitted);
            assert_eq!(report.outcome().errors(), 0);
        }
    }
}

#[test]
fn text_scalar_scanning_uses_work_remaining_after_selected_composition() {
    let text = serde_json::to_string(&"x".repeat(1000)).unwrap();
    for (bounds, expected) in [
        ("", Admission::Admitted),
        (r#", "maxLength":1000"#, Admission::Undetermined),
    ] {
        let model = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"string","minimum":0,"maximum":1,"exclusiveMinimum":0,"exclusiveMaximum":1,"multipleOf":1,"nullable":true{bounds}}}}}}}"#
        );
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", text.as_bytes()).sample(),
            ],
            Limits::default().decoding_work(6000),
        )
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .sample_value()
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap()
                .len(),
            1000
        );
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{bounds}"
        );
        assert_eq!(report.outcome().complete(), expected == Admission::Admitted);
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(
            report.sample_result().unwrap().context_state(),
            esdieff::ContextState::Available
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-type-null/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
    }
}

#[test]
fn inherited_overridden_and_deleted_text_bounds_keep_contributor_origins() {
    let root = br##"{"namespace":{"d":"https://example.com/data"},"sdfProperty":{"p":{"sdfRef":"d:#/sdfData/Text"},"override":{"sdfRef":"d:#/sdfData/Text","maxLength":1},"deleted":{"sdfRef":"d:#/sdfData/Text","maxLength":null}}}"##;
    let donor = br#"{"namespace":{"d":"https://example.com/data"},"defaultNamespace":"d","sdfData":{"Text":{"type":"string","minLength":2,"maxLength":3}}}"#;
    for (pointer, sample, expected) in [
        (
            "/sdfProperty/p",
            br#""abcd""#.as_slice(),
            Admission::Rejected,
        ),
        ("/sdfProperty/override", br#""ab""#, Admission::Rejected),
        ("/sdfProperty/deleted", br#""abcd""#, Admission::Admitted),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", root),
            SourceInput::supplied("donor", donor),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = selected_at(&catalogue, SampleKind::Property, pointer);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete());
        let description = report
            .sample_result()
            .unwrap()
            .description()
            .require()
            .unwrap();
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let minimum = wire["result"]["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["effective"]["pointer"] == format!("{pointer}/minLength"))
            .unwrap();
        assert_eq!(
            minimum["sources"][0]["source_id"],
            catalogue.source_id("donor").unwrap().get()
        );
        assert_eq!(minimum["sources"][0]["pointer"], "/sdfData/Text/minLength");
        assert_eq!(minimum["consumer"]["pointer"], format!("{pointer}/sdfRef"));
        if expected == Admission::Rejected {
            let condition = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["code"] == "sample.length")
                .unwrap();
            let source = condition["related"]
                .as_array()
                .unwrap()
                .iter()
                .find(|related| related["kind"] == "source")
                .unwrap();
            assert_eq!(
                source["source_id"],
                catalogue
                    .source_id(if pointer.ends_with("override") {
                        "m"
                    } else {
                        "donor"
                    })
                    .unwrap()
                    .get()
            );
            assert_eq!(
                source["pointer"],
                if pointer.ends_with("override") {
                    "/sdfProperty/override/maxLength"
                } else {
                    "/sdfData/Text/maxLength"
                }
            );
        } else {
            assert!(matches!(
                description.maximum_length(),
                esdieff::Field::Absent
            ));
            assert_eq!(
                description
                    .json()
                    .quality_explanation("maxLength")
                    .require()
                    .unwrap()
                    .kind(),
                esdieff::OriginKind::Deleted
            );
            assert!(
                !wire["result"]["explanations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["effective"]["pointer"] == format!("{pointer}/maxLength"))
            );
        }
    }
}

#[test]
fn unavailable_scalar_decoding_cannot_admit_a_string_without_length_constraints() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", br#"{"sdfProperty":{"p":{"type":"string"}}}"#),
        SourceInput::supplied("s", br#""\uD800""#).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "input.unicode_unavailable"
                && condition.domain() == Some(esdieff::ConditionDomain::Sample)
                && location_support::primary_sample(condition).span() == Some(0..8))
    );
}

#[test]
fn needed_unknown_context_and_restricted_donor_meanings_cannot_become_text_proofs() {
    for (model, sample) in [
        (br##"{"sdfProperty":{"p":{"sdfRef":"missing:#/sdfData/Text"}}}"##.as_slice(), br#""on""#.as_slice()),
        (br#"{"info":{"features":["https://example.com/critical"]},"sdfProperty":{"p":{"type":"string","maxLength":1}}}"#, br#""on""#),
        (br#"{"sdfProperty":{"p":{"type":"vendor:text","maxLength":1}}}"#, br#""on""#),
        (br#"{"sdfProperty":{"p":{"type":"string","sdfType":"byte-string","maxLength":1}}}"#, br#""Zg""#),
        (br##"{"sdfData":{"Rows":{"type":"array","items":{"type":"array","maxLength":1}}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Rows/items"}}}"##, br#""on""#),
    ] {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", model), SourceInput::supplied("s", sample).sample()]).unwrap();
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), Admission::Undetermined);
        assert!(!report.outcome().complete());
        assert!(!report.diagnostics().conditions().any(|condition| condition.domain() == Some(esdieff::ConditionDomain::Sample) && condition.severity() == esdieff::Severity::Error));
    }
}

#[test]
fn malformed_needed_length_keeps_the_other_independent_length_rejection() {
    for bad in ["-1", "1.5", "true", "\"bad\"", "1e1000001"] {
        let model = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"string","minLength":{bad},"maxLength":1}}}}}}"#
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""on""#).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected,
            "{bad}"
        );
        assert!(!report.outcome().complete());
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "sample.length"
                    && condition.domain() == Some(esdieff::ConditionDomain::Sample))
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.domain() != Some(esdieff::ConditionDomain::Sample))
        );
    }
}

#[test]
fn typed_boolean_and_text_admission_preserve_policy_and_projection_distinctions() {
    let builder = JsonBuilder::new();
    for (qualities, value) in [
        (
            r#""type":"boolean","nullable":false"#,
            builder.boolean(false).unwrap(),
        ),
        (
            r#""type":"string","minLength":2,"maxLength":2"#,
            builder.text("e\u{301}").unwrap(),
        ),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{{qualities}}}}}}}"#);
        let catalogue = Catalogue::take_inputs(vec![
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::typed("s", value),
        ])
        .unwrap();
        for strict in [false, true] {
            let report = catalogue
                .check_sample(
                    SampleRequest::new(
                        catalogue.source_id("m").unwrap(),
                        SampleKind::Property,
                        "/sdfProperty/p",
                    )
                    .sample(catalogue.source_id("s").unwrap())
                    .strict_warnings(strict),
                )
                .unwrap();
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Admitted
            );
            assert!(report.outcome().complete());
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["policy"]["strict_warnings"], strict);
            assert_eq!(wire["outcome"]["disposition"], "passed");
            assert_eq!(
                wire["result"]["sample_location"]["span"],
                serde_json::Value::Null
            );
            assert_eq!(
                wire["result"]["witnesses"][0]["sample"]["span"],
                serde_json::Value::Null
            );
            assert!(matches!(
                catalogue
                    .source(catalogue.source_id("s").unwrap())
                    .unwrap()
                    .json(),
                esdieff::Field::Unavailable(_)
            ));
        }
    }
}

#[test]
fn malformed_needed_pattern_shape_is_a_model_finding_with_independent_text_rejection() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "m",
            br#"{"sdfProperty":{"p":{"type":"string","pattern":false,"maxLength":1}}}"#,
        ),
        SourceInput::supplied("s", br#""on""#).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(
        report.sample_result().unwrap().context_findings().errors(),
        1
    );
    assert_eq!(report.outcome().errors(), 2);
    assert!(!report.outcome().complete());
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .area_status("sample-text-boolean/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "data.string"
                && condition.domain() == Some(esdieff::ConditionDomain::Model))
    );
}

#[test]
fn produced_findings_witnesses_and_coverage_cite_text_boolean_and_nullable_work() {
    for (qualities, sample, code, citation, type_citation) in [
        (
            r#""type":"string","maxLength":1"#,
            r#""on""#,
            "sample.length",
            "RFC 9880 Appendix C.2",
            "RFC 9880 Appendix C.2",
        ),
        (
            r#""type":"string""#,
            "true",
            "sample.type",
            "RFC 9880 Appendix C.2",
            "RFC 9880 Appendix C.2",
        ),
        (
            r#""type":"boolean""#,
            r#""on""#,
            "sample.type",
            "RFC 9880 Appendix C.3",
            "RFC 9880 Appendix C.3",
        ),
        (
            r#""type":"number""#,
            r#""on""#,
            "sample.type",
            "RFC 9880 Appendix C.1",
            "RFC 9880 Appendix C.1",
        ),
        (
            r#""type":"number","nullable":false"#,
            "null",
            "sample.null",
            "RFC 9880 Section 4.7 Table 4",
            "RFC 9880 Section 4.7 and Appendix C",
        ),
        (
            r#""type":"string","nullable":false"#,
            "null",
            "sample.null",
            "RFC 9880 Section 4.7 Table 4",
            "RFC 9880 Section 4.7 and Appendix C",
        ),
        (
            r#""type":"boolean","nullable":false"#,
            "null",
            "sample.null",
            "RFC 9880 Section 4.7 Table 4",
            "RFC 9880 Section 4.7 and Appendix C",
        ),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{{qualities}}}}}}}"#);
        let builder = JsonBuilder::new();
        for input in [
            SourceInput::supplied("s", sample.as_bytes()).sample(),
            SourceInput::typed(
                "s",
                match sample {
                    "null" => builder.null().unwrap(),
                    "true" => builder.boolean(true).unwrap(),
                    _ => builder.text("on").unwrap(),
                },
            ),
        ] {
            let catalogue =
                Catalogue::take_inputs(vec![SourceInput::supplied("m", model.as_bytes()), input])
                    .unwrap();
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            let diagnostics = report.diagnostics();
            let condition = diagnostics
                .conditions()
                .find(|condition| condition.code() == code)
                .unwrap();
            assert_eq!(condition.basis().citation(), Some(citation));
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            let finding = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|condition| condition["code"] == code)
                .unwrap();
            let witness = wire["result"]["witnesses"]
                .as_array()
                .unwrap()
                .iter()
                .find(|witness| witness["kind"] == "rejection")
                .unwrap();
            assert_eq!(finding["basis"], witness["basis"]);
            assert!(serde_json::to_string(finding).unwrap().len() < 3072);
            let type_row = wire["coverage"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == "sample-type-null/1")
                .unwrap();
            assert_eq!(type_row["basis"]["citation"], type_citation);
            assert!(serde_json::to_string(type_row).unwrap().len() < 256);
            let text_row = wire["coverage"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == "sample-text-boolean/1")
                .unwrap();
            assert_eq!(
                text_row["basis"]["citation"],
                if sample == "null" || qualities.contains("number") {
                    "RFC 9880 Appendix C.1"
                } else {
                    citation
                }
            );
            assert!(report.to_human().contains("rejected"));
        }
    }
}
