//! Selected matching uses the partial Unicode ECMAScript capability in ADR 0014.
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, JsonBuilder, Limits, Report, SampleKind,
    SampleRequest, SourceInput,
};
use serde_json::Value;

fn selected<'a>(catalogue: &'a Catalogue) -> Report<'a> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("model").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap()
}

#[test]
fn matching_capability_and_work_limits_stay_distinct_from_mismatch() {
    for (pattern, limits, code) in [
        (r"(?<x>a)|(?<x>b)", Limits::default(), "pattern.unsupported"),
        (
            "a",
            Limits::default().pattern_subject_bytes(1),
            "operation.limit",
        ),
    ] {
        let model = serde_json::json!({"sdfProperty":{"p":{"type":"string","pattern":pattern,"maxLength":1}}}).to_string();
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("model", model.as_bytes()),
                SourceInput::supplied("sample", br#""aa""#).sample(),
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
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == code),
            "{pattern}: {}",
            report.to_json()
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "sample.length")
        );
        assert_eq!(report.requested_coverage().total(), Some(9));
    }
}

#[test]
fn malformed_patterns_are_model_errors_and_do_not_erase_a_complete_choice_witness() {
    for (qualities, admitted, complete) in [
        (
            serde_json::json!({"type":"string","pattern":"["}),
            false,
            false,
        ),
        (
            serde_json::json!({"type":"string","sdfChoice":{"bad":{"pattern":"["},"good":{"pattern":"^a$"}}}),
            true,
            true,
        ),
    ] {
        let model = serde_json::json!({"sdfProperty":{"p":qualities}}).to_string();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("sample", br#""a""#).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            if admitted {
                Admission::Admitted
            } else {
                Admission::Undetermined
            }
        );
        assert_eq!(report.outcome().complete(), complete);
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(
            report.sample_result().unwrap().context_findings().errors(),
            1
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == "pattern.syntax")
            .unwrap();
        assert_eq!(finding["domain"], "model");
        assert_eq!(finding["primary"]["kind"], "effective");
        assert!(
            !wire["result"]["witnesses"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "rejection")
        );
    }
}

#[test]
fn unicode_patterns_match_without_implicit_anchors() {
    for (pattern, text, admitted) in [
        ("é", "café", true),
        ("^é$", "café", false),
        (r"^\u{1F980}$", "\u{1F980}", true),
        (r"^\uD83E\uDD80$", "\u{1F980}", true),
        (r"^.$", "𝄞", true),
        (r"^\w+$", "é", false),
        (r"^\p{Letter}+$", "é", true),
        (r"(?<=a)b", "ab", true),
        (r"^(?<x>a)\k<x>$", "aa", true),
        (r"^a$", "a\n", false),
        (r"a$", "ba", true),
        (r"^a", "ab", true),
    ] {
        let model = serde_json::json!({"sdfProperty":{"p":{"type":"string","pattern":pattern}}})
            .to_string();
        let sample = serde_json::to_vec(text).unwrap();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("sample", &sample).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            if admitted {
                Admission::Admitted
            } else {
                Admission::Rejected
            },
            "{pattern} against {text:?}"
        );
        assert!(report.outcome().complete(), "{pattern}");
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        if !admitted {
            let condition = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["code"] == "sample.pattern")
                .unwrap();
            assert_eq!(condition["primary"]["pointer"], "");
            assert_eq!(
                condition["primary"]["span"],
                serde_json::json!([0, sample.len()])
            );
            assert!(
                condition["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["pointer"] == "/sdfProperty/p/pattern")
            );
        }
    }
}

#[test]
fn composed_patterns_and_typed_samples_keep_consumer_and_original_locations() {
    let donor = br#"{"namespace":{"own":"https://example.com/pattern"},"defaultNamespace":"own","sdfData":{"Base":{"type":"string","pattern":"^yes$"}}}"#;
    for (override_pattern, expected) in [
        ("", Admission::Rejected),
        (r#", "pattern":"no""#, Admission::Admitted),
        (r#", "pattern":null"#, Admission::Admitted),
    ] {
        let root = format!(
            r##"{{"namespace":{{"d":"https://example.com/pattern"}},"sdfProperty":{{"p":{{"sdfRef":"d:#/sdfData/Base"{override_pattern}}}}}}}"##
        );
        let catalogue = Catalogue::take_inputs(vec![
            SourceInput::supplied("model", root.as_bytes()),
            SourceInput::supplied("donor", donor),
            SourceInput::typed("sample", JsonBuilder::new().text("no").unwrap()),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete());
        if expected == Admission::Rejected {
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            let finding = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["code"] == "sample.pattern")
                .unwrap();
            assert!(finding["primary"]["span"].is_null());
            assert!(
                finding["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["pointer"] == "/sdfProperty/p/pattern")
            );
            assert!(
                finding["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["source_id"] == 2
                        && location["pointer"] == "/sdfData/Base/pattern")
            );
        }
    }
}

#[test]
fn choices_and_declared_value_consistency_share_matching() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"type":"string","sdfChoice":{"first":{"pattern":"^yes$"},"second":{"pattern":"^no$"}}}},"sdfData":{"D":{"type":"string","pattern":"^yes$","default":"no","const":"yes"}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("sample", br#""no""#).sample(),
    ])
    .unwrap();
    let sample = selected(&catalogue);
    assert_eq!(
        sample.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert!(sample.outcome().complete());
    assert!(
        !sample
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "sample.pattern")
    );
    let consistency = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert!(consistency.outcome().complete());
    assert_eq!(consistency.outcome().warnings(), 1);
    assert_eq!(
        consistency
            .assessment_area_status("default-consistency/1")
            .present(),
        Some(esdieff::AssessmentStatus::Assessed)
    );
}

#[test]
fn matching_limits_and_omissions_preserve_definite_pattern_findings() {
    let model = br#"{"sdfProperty":{"p":{"type":"string","pattern":"^yes$"}}}"#;
    for limits in [
        Limits::default(),
        Limits::default()
            .conditions(1)
            .diagnostic_bytes(1)
            .pointer_bytes(1)
            .pointer_output_bytes(1),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("model", model),
                SourceInput::supplied("sample", br#""no""#).sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().errors(), 1);
    }
    for limits in [
        Limits::default().pattern_subject_bytes(0),
        Limits::default().pattern_subject_bytes(1024 * 1024 + 1),
    ] {
        assert!(Catalogue::with_limits(&[], limits).is_err());
    }
}

#[test]
fn consistency_model_syntax_is_collected_only_for_consulted_candidate_patterns() {
    for (model, expected_errors, complete) in [
        (
            r#"{"info":{},"sdfData":{"D":{"type":"string","pattern":"["}}}"#,
            0,
            true,
        ),
        (
            r#"{"info":{},"sdfData":{"D":{"type":"object","default":{},"properties":{"absent":{"type":"string","pattern":"["}}}}}"#,
            0,
            true,
        ),
        (
            r#"{"info":{},"sdfData":{"D":{"type":"object","default":{"present":"a"},"properties":{"present":{"type":"string","pattern":"["}}}}}"#,
            1,
            false,
        ),
        (
            r#"{"info":{},"sdfData":{"D":{"type":"string","const":"a","default":"a","pattern":"["}}}"#,
            1,
            false,
        ),
        (
            r#"{"info":{},"sdfData":{"D":{"type":"string","default":"a","sdfChoice":{"bad":{"pattern":"["},"good":{"pattern":"a"}}}}}"#,
            1,
            true,
        ),
    ] {
        for limits in [
            Limits::default(),
            Limits::default().diagnostic_bytes(1).conditions(1),
        ] {
            let catalogue =
                Catalogue::with_limits(&[SourceInput::supplied("model", model.as_bytes())], limits)
                    .unwrap();
            let report = catalogue
                .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
                .unwrap();
            assert_eq!(report.outcome().errors(), expected_errors, "{model}");
            assert_eq!(report.outcome().complete(), complete, "{model}");
            assert_eq!(report.requested_coverage().total(), Some(10));
            assert!(
                !report
                    .diagnostics()
                    .conditions()
                    .any(|condition| condition.code() == "consistency.default_excluded"),
                "{model}"
            );
        }
    }
}

#[test]
fn sequential_patterns_keep_their_own_text_and_fresh_operation_allowances() {
    let properties: serde_json::Map<_, _> = (0..12)
        .map(|index| {
            (
                format!("p{index}"),
                serde_json::json!({"type":"string","pattern":format!("^value{index}$")}),
            )
        })
        .collect();
    let model = serde_json::json!({"sdfProperty":{"p":{"type":"object","properties":properties}}})
        .to_string();
    let sample: serde_json::Map<_, _> = (0..12)
        .map(|index| {
            (
                format!("p{index}"),
                serde_json::json!(format!("value{index}")),
            )
        })
        .collect();
    let sample = serde_json::to_vec(&sample).unwrap();
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model.as_bytes()),
        SourceInput::supplied("sample", &sample).sample(),
    ])
    .unwrap();
    for _ in 0..3 {
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        assert!(report.outcome().complete());
    }
}
