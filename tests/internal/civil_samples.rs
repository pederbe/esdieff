//! Selected civil samples follow ADR 0018: Civil date and time sample conventions.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, JsonBuilder, Limits, Report,
    SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;

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
fn model(format: &str, extra: &str) -> String {
    format!(
        r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":false,"format":"{format}"{extra}}}}}}}"#
    )
}
fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}
fn invariant(report: &Report<'_>) {
    let wire = wire(report);
    assert_eq!(wire["schema"], "esdieff.report");
    assert_eq!(wire["version"], "1.1");
    assert_eq!(wire["operation"], "sample");
    assert_eq!(wire["request"]["assessment"], "esdieff.sample/1");
    assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
    assert_eq!(wire["request"]["sample"], wire["result"]["sample"]);
    let coverage = &wire["coverage"]["requested"];
    assert_eq!(coverage["total"], 9);
    assert_eq!(coverage["count_unit"], "sample_area");
    assert_eq!(
        ["assessed", "not_applicable", "unassessed", "undetermined"]
            .into_iter()
            .map(|key| coverage[key].as_u64().unwrap())
            .sum::<u64>(),
        9
    );
    assert_eq!(wire["result"]["context"]["model_assessment"], Value::Null);
    assert!(
        report
            .to_human()
            .contains(report.sample_result().unwrap().admission().as_str())
    );
}

#[test]
fn all_three_formats_have_original_and_typed_public_cases() {
    use Admission::{Admitted as A, Rejected as R, Undetermined as U};
    let cases = [
        ("date", "0000-02-29", A),
        ("date", "2000-02-29", A),
        ("date", "9999-12-31", A),
        ("date", "1900-02-29", R),
        ("date", "2024-04-31", R),
        ("date", "20240229", R),
        ("date-time", "1996-12-19T16:39:57-08:00", A),
        ("date-time", "1996-12-20t00:39:57z", A),
        ("date-time", "2024-03-01T00:00:58+23:59", A),
        ("date-time", "2024-03-01T23:59:58-23:59", A),
        ("date-time", "0000-01-01T00:00:58+23:59", A),
        ("date-time", "9999-12-31T23:59:58-23:59", A),
        ("date-time", "2024-01-01T12:34:56-00:00", A),
        ("date-time", "2017-01-01T05:44:60+05:45", A),
        ("date-time", "2016-12-31T15:59:60-08:00", A),
        ("date-time", "2016-12-31T23:59:60.000000000000000000001Z", A),
        ("date-time", "2024-06-30T23:59:60Z", R),
        ("date-time", "2026-12-31T23:59:60Z", R),
        ("date-time", "2030-06-30T23:59:60Z", U),
        ("date-time", "2030-06-30T23:59:59.9Z", U),
        ("date-time", "1971-12-31T23:59:60Z", U),
        ("date-time", "2030-09-30T23:59:60Z", U),
        ("date-time", "2030-06-30T12:34:60Z", R),
        ("date-time", "2030-06-29T23:59:60Z", R),
        ("date-time", "2030-06-30T23:59:58Z", A),
        ("date-time", "2024-01-01 12:00:00Z", R),
        ("date-time", "2024-01-01T24:00:00Z", R),
        ("date-time", "2024-01-01T12:00:00Z[UTC]", R),
        ("date-time", "2024-01-01T12:00:00+24:00", R),
        ("date-time", "2024-01-01T12:00:00+05:45:00", R),
        ("time", "12:34:56.000000000000000000001+05:45", A),
        ("time", "23:59:59Z", A),
        ("time", "12:34:56-00:00", A),
        ("time", "12:34:56", R),
        ("time", "12:34:60Z", R),
        ("time", "23:59:60Z", U),
        ("time", "05:44:60+05:45", U),
        ("time", "24:00:00Z", R),
        ("time", "12:34:56,1Z", R),
    ];
    for (format, text, expected) in cases {
        let model = model(format, "");
        let token = serde_json::to_string(text).unwrap();
        for typed in [false, true] {
            let sample = if typed {
                SourceInput::typed("s", JsonBuilder::new().text(text).unwrap())
            } else {
                SourceInput::supplied("s", token.as_bytes()).sample()
            };
            let catalogue =
                Catalogue::take_inputs(vec![SourceInput::supplied("m", model.as_bytes()), sample])
                    .unwrap();
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{format}: {text}"
            );
            assert_eq!(report.outcome().complete(), expected != U);
            assert_eq!(report.outcome().errors(), u32::from(expected == R));
            assert_eq!(
                report
                    .sample_result()
                    .unwrap()
                    .area_status("sample-abstract-values/1")
                    .require()
                    .unwrap(),
                if expected == U {
                    AssessmentStatus::Undetermined
                } else {
                    AssessmentStatus::Assessed
                }
            );
            let wire = wire(&report);
            assert_eq!(wire["result"]["sample_location"]["pointer"], "");
            assert_eq!(
                wire["result"]["sample_location"]["span"],
                if typed {
                    Value::Null
                } else {
                    serde_json::json!([0, token.len()])
                }
            );
            assert_eq!(
                wire["coverage"]["requested"]["assessed"],
                if expected == U { 3 } else { 4 }
            );
            assert_eq!(wire["coverage"]["requested"]["not_applicable"], 5);
            invariant(&report);
        }
    }
}

#[test]
fn date_capability_and_long_fraction_controls_remain_exact() {
    // This deliberately exhausts prerequisites before temporal evaluation.
    let date_model = model("date", "");
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", date_model.as_bytes()),
            SourceInput::supplied("s", br#""2024-02-29""#).sample(),
        ],
        Limits::default().decoding_work(1),
    )
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    let clock = format!("12:34:56.{}+05:45", "0".repeat(4095) + "1");
    let token = serde_json::to_string(&clock).unwrap();
    let time_model = model("time", "");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", time_model.as_bytes()),
        SourceInput::supplied("s", token.as_bytes()).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .sample_value()
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        clock
    );
    assert_eq!(
        catalogue
            .source(catalogue.source_id("s").unwrap())
            .unwrap()
            .raw_bytes(),
        token.as_bytes()
    );
}

#[test]
fn identical_valid_and_invalid_temporal_const_keep_denotation_boundary() {
    for (format, value, constant, admission, complete, const_status) in [
        (
            "date",
            "2000-02-29",
            "2000-02-29",
            Admission::Admitted,
            true,
            AssessmentStatus::Assessed,
        ),
        (
            "date",
            "not-a-date",
            "not-a-date",
            Admission::Rejected,
            false,
            AssessmentStatus::Undetermined,
        ),
        (
            "time",
            "23:59:60Z",
            "23:59:60Z",
            Admission::Undetermined,
            false,
            AssessmentStatus::Undetermined,
        ),
        (
            "date",
            "2000-02-29",
            "2000-03-01",
            Admission::Rejected,
            true,
            AssessmentStatus::Assessed,
        ),
        (
            "date-time",
            "1996-12-19T16:39:57-08:00",
            "1996-12-20T00:39:57Z",
            Admission::Admitted,
            true,
            AssessmentStatus::Assessed,
        ),
    ] {
        let extra = format!(",\"const\":{}", serde_json::to_string(constant).unwrap());
        let model = model(format, &extra);
        let token = serde_json::to_string(value).unwrap();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", token.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), admission);
        assert_eq!(report.outcome().complete(), complete);
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-const/1")
                .require()
                .unwrap(),
            const_status
        );
        assert_eq!(
            wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.const"),
            admission == Admission::Rejected && const_status == AssessmentStatus::Assessed
        );
        invariant(&report);
    }
}

#[test]
fn alternatives_require_one_complete_witness_and_keep_unknown_branches() {
    let model = br##"{"sdfProperty":{"p":{"type":"string","nullable":false,"format":"date","sdfChoice":{"known":{},"unknown":{"sdfRef":"#/sdfData/missing"}}}}}"##;
    for (token, expected) in [
        (br#""2000-02-29""#.as_slice(), Admission::Admitted),
        (br#""not-a-date""#.as_slice(), Admission::Undetermined),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", token).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        let wire = wire(&report);
        if expected == Admission::Admitted {
            assert!(
                wire["result"]["witnesses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["kind"] == "admission"
                        && row["alternative"]["pointer"] == "/sdfProperty/p/sdfChoice/known")
            );
        } else {
            assert!(
                !wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["domain"] == "sample" && row["severity"] == "error")
            );
        }
        invariant(&report);
    }
    let model = br#"{"sdfProperty":{"p":{"type":"string","nullable":false,"sdfChoice":{"temporal":{"format":"date"},"long":{"minLength":100}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""not-a-date""#).sample(),
    ])
    .unwrap();
    assert_eq!(
        selected(&catalogue).sample_result().unwrap().admission(),
        Admission::Rejected
    );
}

#[test]
fn independent_lengths_and_optional_omission_preserve_outcomes_and_knowledge() {
    let date_model = model("date", ",\"maxLength\":1");
    let date = Catalogue::from_inputs(&[
        SourceInput::supplied("m", date_model.as_bytes()),
        SourceInput::supplied("s", br#""2000-02-29""#).sample(),
    ])
    .unwrap();
    let date_report = selected(&date);
    assert_eq!(
        date_report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(date_report.outcome().complete());
    assert_eq!(date_report.outcome().errors(), 1);
    assert_eq!(
        date_report
            .sample_result()
            .unwrap()
            .area_status("sample-abstract-values/1")
            .require()
            .unwrap(),
        AssessmentStatus::Assessed
    );
    let model = model("date-time", ",\"maxLength\":1");
    let token = br#""2030-06-30T23:59:60Z""#;
    let mut summaries = Vec::new();
    for limits in [
        Limits::default(),
        Limits::default().diagnostic_bytes(1),
        Limits::default()
            .conditions(1)
            .pointer_bytes(1)
            .pointer_output_bytes(1),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", token).sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(!report.outcome().complete());
        let wire = wire(&report);
        assert_eq!(
            wire["baseline"]["temporal_knowledge"]["id"],
            "iers-bulletin-c72/2026-07-06"
        );
        assert_eq!(
            wire["result"]["sample_location"]["span"],
            serde_json::json!([0, token.len()])
        );
        summaries.push((
            wire["outcome"].clone(),
            wire["coverage"]["requested"].clone(),
            wire["result"]["admission"].clone(),
        ));
        invariant(&report);
    }
    assert!(summaries.iter().all(|row| row == &summaries[0]));
}

#[test]
fn actual_use_survives_failed_branch_and_metadata_grammar_remains_separate() {
    let model = br#"{"sdfProperty":{"p":{"type":"string","nullable":false,"sdfChoice":{"aDated":{"format":"date-time"},"zText":{}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""2024-06-30T23:59:60Z""#).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert_eq!(
        wire(&report)["baseline"]["temporal_knowledge"]["id"],
        "iers-bulletin-c72/2026-07-06"
    );
    let model = br#"{"info":{"modified":"2024-01-01T12:00:00+00:00"},"sdfProperty":{"p":{"type":"string","nullable":false,"format":"date-time"}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""2024-01-01T12:00:00+00:00""#).sample(),
    ])
    .unwrap();
    assert_eq!(
        selected(&catalogue).sample_result().unwrap().admission(),
        Admission::Admitted
    );
    let check = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert!(
        wire(&check)["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "metadata.modified_syntax")
    );
}

#[test]
fn original_escaped_token_and_opposing_type_null_controls_keep_their_domains() {
    let model = model("date-time", "");
    let token = br#""1996-12-20\u005400:39:57\u005a""#;
    for typed in [false, true] {
        let sample = if typed {
            SourceInput::typed(
                "s",
                JsonBuilder::new().text("1996-12-20T00:39:57Z").unwrap(),
            )
        } else {
            SourceInput::supplied("s", token).sample()
        };
        let catalogue =
            Catalogue::take_inputs(vec![SourceInput::supplied("m", model.as_bytes()), sample])
                .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        assert_eq!(
            wire(&report)["result"]["sample_location"]["span"],
            if typed {
                Value::Null
            } else {
                serde_json::json!([0, token.len()])
            }
        );
    }
    // Established null and wrong-kind constraints exclude temporal scanning.
    for (nullable, token, expected, code) in [
        (
            false,
            b"true".as_slice(),
            Admission::Rejected,
            Some("sample.type"),
        ),
        (
            false,
            b"null".as_slice(),
            Admission::Rejected,
            Some("sample.null"),
        ),
        (true, b"null".as_slice(), Admission::Admitted, None),
    ] {
        let model = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":{nullable},"format":"date"}}}}}}"#
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", token).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        let wire = wire(&report);
        let errors = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["domain"] == "sample" && row["severity"] == "error")
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), usize::from(code.is_some()));
        if let Some(code) = code {
            assert_eq!(errors[0]["code"], code);
        }
    }
}

#[test]
fn inherited_override_and_all_declaring_selectors_keep_real_origins() {
    let model = br##"{"sdfData":{"D":{"type":"string","nullable":false,"format":"date-time"}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/D","format":"date"}},"sdfAction":{"a":{"sdfInputData":{"type":"string","nullable":false,"format":"date"},"sdfOutputData":{"type":"string","nullable":false,"format":"date"}}},"sdfEvent":{"e":{"sdfOutputData":{"type":"string","nullable":false,"format":"date"}}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""2000-02-29""#).sample(),
    ])
    .unwrap();
    for (kind, pointer, admission) in [
        (SampleKind::Property, "/sdfProperty/p", Admission::Admitted),
        (SampleKind::Data, "/sdfData/D", Admission::Rejected),
        (SampleKind::ActionInput, "/sdfAction/a", Admission::Admitted),
        (
            SampleKind::ActionOutput,
            "/sdfAction/a",
            Admission::Admitted,
        ),
        (SampleKind::EventOutput, "/sdfEvent/e", Admission::Admitted),
    ] {
        let report = catalogue
            .check_sample(
                SampleRequest::new(catalogue.source_id("m").unwrap(), kind, pointer)
                    .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(report.sample_result().unwrap().admission(), admission);
        invariant(&report);
        if kind == SampleKind::Property {
            assert!(
                wire(&report)["result"]["explanations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["sources"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|source| source["pointer"] == "/sdfProperty/p/format"))
            );
        }
    }
    assert_eq!(
        catalogue
            .source(catalogue.source_id("m").unwrap())
            .unwrap()
            .raw_bytes(),
        model
    );
}

#[test]
fn mixed_equality_critical_unknown_quality_and_restricted_donors_stay_unknown() {
    let cases: &[&[u8]] = &[
        // Explicit format:null is an unknown quality, not an ordinary absent-format branch.
        br#"{"sdfProperty":{"p":{"type":"string","format":"date","const":"2000-02-29","sdfChoice":{"date":{},"explicitNull":{"format":null}}}}}"#,
        br#"{"info":{"features":["urn:example:unknown-critical"]},"sdfProperty":{"p":{"type":"string","format":"date"}}}"#,
        br##"{"sdfData":{"A":{"type":"array","items":{"type":"string","format":"date"}}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/A/items"}}}"##,
        br#"{"sdfProperty":{"p":{"format":"date"}}}"#,
        br#"{"sdfProperty":{"p":{"type":"string","format":"future-format"}}}"#,
    ];
    for model in cases {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", br#""2000-02-29""#).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined,
            "{}",
            std::str::from_utf8(model).unwrap()
        );
        assert!(!report.outcome().complete());
        assert!(
            !wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.const"
                    || row["code"] == "sample.temporal_representation")
        );
    }
}

#[test]
fn undeclared_extension_vocabulary_leaves_a_date_constant_comparable() {
    // Following ADR 0037: Undeclared extension vocabulary.
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "m",
            br#"{"sdfProperty":{"p":{"type":"string","format":"date","const":"2000-02-29","unknownEqualityQuality":true}}}"#,
        ),
        SourceInput::supplied("s", br#""2000-02-29""#).sample(),
    ])
    .unwrap();
    assert_eq!(
        selected(&catalogue).sample_result().unwrap().admission(),
        Admission::Admitted
    );
}

#[test]
fn valid_mixed_and_distinct_temporal_const_meanings_use_both_orders() {
    // Names and authored order both put each meaning first once on both supported backends.
    // No shared format: {} is a valid ordinary meaning, not an explicit null override.
    for branches in [
        r#""aTemporal":{"format":"date"},"zText":{}"#,
        r#""aText":{},"zTemporal":{"format":"date"}"#,
        r#""aDate":{"format":"date"},"zTime":{"format":"time"}"#,
        r#""aTime":{"format":"time"},"zDate":{"format":"date"}"#,
    ] {
        let model = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":false,"const":"2000-02-29","sdfChoice":{{{branches}}}}}}}}}"#
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""2000-02-29""#).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-const/1")
                .require()
                .unwrap(),
            AssessmentStatus::Undetermined
        );
        assert!(
            !wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.const")
        );
        assert!(
            !wire(&report)["result"]["witnesses"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["kind"] == "admission")
        );
        invariant(&report);
    }
}

#[test]
fn nested_temporal_properties_items_and_equal_pairs_keep_real_locations() {
    for (model, token, pointer) in [
        (
            r#"{"sdfProperty":{"p":{"type":"object","properties":{"stamp":{"type":"string","format":"date"}}}}}"#,
            r#"{"stamp":"1900-02-29"}"#,
            "/stamp",
        ),
        (
            r#"{"sdfProperty":{"p":{"type":"array","items":{"type":"string","format":"date"}}}}"#,
            r#"["1900-02-29"]"#,
            "/0",
        ),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", token.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        let wire = wire(&report);
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "sample.temporal_calendar")
            .unwrap();
        assert_eq!(finding["primary"]["pointer"], pointer);
        let start = token.find("\"1900-02-29\"").unwrap();
        assert_eq!(
            finding["primary"]["span"],
            serde_json::json!([start, start + 12])
        );
    }
    for (values, expected, unique) in [
        (r#"["2000-02-29","2000-02-29"]"#, Admission::Rejected, true),
        (r#"["2000-02-29","2000-03-01"]"#, Admission::Admitted, false),
        (r#"["bad","bad"]"#, Admission::Rejected, false),
    ] {
        let model=br#"{"sdfProperty":{"p":{"type":"array","uniqueItems":true,"items":{"type":"string","format":"date"}}}}"#;
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", values.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert_eq!(
            wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.unique"),
            unique
        );
    }
}
#[test]
fn source_ceiling_fraction_stays_exact_when_capability_is_unavailable() {
    let text = format!("12:34:56.{}Z", "0".repeat(4 * 1024 * 1024 - 12));
    let token = serde_json::to_string(&text).unwrap();
    assert_eq!(token.len(), 4 * 1024 * 1024);
    let model = model("time", "");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", token.as_bytes()).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(report.outcome().errors(), 0);
    assert!(matches!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted | Admission::Undetermined
    ));
    assert_eq!(
        catalogue
            .source(catalogue.source_id("s").unwrap())
            .unwrap()
            .raw_bytes(),
        token.as_bytes()
    );
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .sample_value()
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        text
    );
}

#[test]
fn temporal_uniqueness_preserves_restricted_null_and_existing_common_meaning() {
    for (format, value) in [
        ("date", "2000-02-29"),
        ("time", "12:34:56Z"),
        ("date-time", "2024-01-01T12:34:56Z"),
    ] {
        for (values, admission, complete, array_status, unique) in [
            (
                serde_json::json!([null, null]),
                Admission::Undetermined,
                false,
                AssessmentStatus::Undetermined,
                false,
            ),
            (
                serde_json::json!([null, value]),
                Admission::Undetermined,
                false,
                AssessmentStatus::Undetermined,
                false,
            ),
            (
                serde_json::json!([true, false]),
                Admission::Rejected,
                true,
                AssessmentStatus::Assessed,
                false,
            ),
            (
                serde_json::json!([true, true]),
                Admission::Rejected,
                true,
                AssessmentStatus::Assessed,
                true,
            ),
            (
                serde_json::json!([true, value]),
                Admission::Rejected,
                true,
                AssessmentStatus::Assessed,
                false,
            ),
            (
                serde_json::json!([true, null]),
                Admission::Rejected,
                false,
                AssessmentStatus::Undetermined,
                false,
            ),
            (
                serde_json::json!([null, true, true]),
                Admission::Rejected,
                false,
                AssessmentStatus::Assessed,
                true,
            ),
        ] {
            let model = serde_json::json!({"sdfProperty":{"p":{"type":"array","uniqueItems":true,"items":{"type":"string","format":format}}}}).to_string();
            let token = values.to_string();
            let catalogue = Catalogue::from_inputs(&[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", token.as_bytes()).sample(),
            ])
            .unwrap();
            let report = selected(&catalogue);
            let wire = wire(&report);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                admission,
                "{format} {token}: {wire}"
            );
            assert_eq!(report.outcome().complete(), complete, "{format} {token}");
            assert_eq!(
                report
                    .sample_result()
                    .unwrap()
                    .area_status("sample-array/1")
                    .require()
                    .unwrap(),
                array_status,
                "{format} {token}"
            );
            let findings = wire["diagnostics"]["items"].as_array().unwrap();
            let unique_finding = findings.iter().find(|row| row["code"] == "sample.unique");
            assert_eq!(unique_finding.is_some(), unique, "{format} {token}");
            let witnesses = wire["result"]["witnesses"].as_array().unwrap();
            assert_eq!(
                witnesses
                    .iter()
                    .any(|row| row["kind"] == "rejection" && row["constraint"] == "uniqueItems"),
                unique
            );
            if let Some(finding) = unique_finding {
                let index = if values.as_array().unwrap().len() == 3 {
                    2
                } else {
                    1
                };
                assert_eq!(finding["domain"], "sample");
                assert_eq!(finding["severity"], "error");
                assert_eq!(finding["basis"]["citation"], "RFC 9880 Appendix C.4");
                assert_eq!(finding["primary"]["pointer"], format!("/{index}"));
                assert!(finding["primary"]["span"].is_array());
                assert!(
                    finding["related"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|row| row["kind"] == "sample"
                            && row["pointer"] == format!("/{}", index - 1))
                );
            }
            if values == serde_json::json!([null, null]) {
                assert_eq!(report.outcome().errors(), 0);
                assert_eq!(
                    report
                        .sample_result()
                        .unwrap()
                        .area_status("sample-type-null/1")
                        .require()
                        .unwrap(),
                    AssessmentStatus::Undetermined
                );
                assert!(!witnesses.iter().any(|row| row["kind"] == "rejection"));
            }
            invariant(&report);
        }
        let length_model = serde_json::json!({"sdfProperty":{"p":{"type":"array","uniqueItems":true,"items":{"type":"string","format":format,"maxLength":1}}}}).to_string();
        let length_token = serde_json::json!([null, value]).to_string();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", length_model.as_bytes()),
            SourceInput::supplied("s", length_token.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(!report.outcome().complete());
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-array/1")
                .require()
                .unwrap(),
            AssessmentStatus::Undetermined
        );
        let length_wire = wire(&report);
        let conditions = length_wire["diagnostics"]["items"].as_array().unwrap();
        assert!(!conditions.iter().any(|row| row["code"] == "sample.unique"));
        let length = conditions
            .iter()
            .find(|row| row["code"] == "sample.length")
            .unwrap();
        assert_eq!(length["primary"]["pointer"], "/1");
        assert!(length["primary"]["span"].is_array());
        assert_eq!(length["basis"]["citation"], "RFC 9880 Appendix C.2");
        invariant(&report);
        for (extra, sample, admission, complete, const_status) in [
            (
                ",\"nullable\":true",
                "null",
                Admission::Admitted,
                true,
                AssessmentStatus::NotApplicable,
            ),
            (
                ",\"nullable\":true,\"const\":null",
                "null",
                Admission::Admitted,
                true,
                AssessmentStatus::Assessed,
            ),
            (
                ",\"nullable\":false,\"const\":null",
                "null",
                Admission::Rejected,
                true,
                AssessmentStatus::Assessed,
            ),
            (
                ",\"const\":true",
                "true",
                Admission::Rejected,
                true,
                AssessmentStatus::Assessed,
            ),
        ] {
            let model = format!(
                r#"{{"sdfProperty":{{"p":{{"type":"string","format":"{format}"{extra}}}}}}}"#
            );
            let catalogue = Catalogue::from_inputs(&[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", sample.as_bytes()).sample(),
            ])
            .unwrap();
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                admission,
                "{format} {extra} {sample}"
            );
            assert_eq!(report.outcome().complete(), complete);
            assert_eq!(
                report
                    .sample_result()
                    .unwrap()
                    .area_status("sample-const/1")
                    .require()
                    .unwrap(),
                const_status
            );
            assert!(
                !wire(&report)["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["code"] == "sample.const")
            );
            invariant(&report);
        }
    }
}
