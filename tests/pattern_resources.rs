//! Fixed Corvus safeguards through public Model, Sample and Trace operations.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, SampleKind, SampleRequest,
    SourceInput, TraceOutcome, TraceRequest,
};

fn sample<'a>(catalogue: &'a Catalogue, key: &str) -> esdieff::Report<'a> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("model").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id(key).unwrap()),
        )
        .unwrap()
}

#[test]
fn former_javascript_compile_capacities_are_supported_with_checked_answers() {
    for pattern in [
        format!("(?<{}>a)", "x".repeat(125)),
        "()".repeat(255),
        format!("(?<x>a){}", "|a".repeat(256)),
    ] {
        let model = serde_json::json!({"sdfProperty":{"p":{"type":"string","pattern":pattern}}})
            .to_string();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("value", br#""a""#).sample(),
        ])
        .unwrap();
        let report = sample(&catalogue, "value");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted,
            "{}",
            report.to_human()
        );
        assert!(report.outcome().complete());
        let model = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        for member in ["rfc9880.model.M047/1", "rfc9880.model.M048/1"] {
            assert_eq!(
                model.requirement_status(member).require().unwrap(),
                AssessmentStatus::Assessed
            );
        }
    }
}

#[test]
fn a_backreference_timeout_is_incomplete_and_does_not_poison_later_samples_or_entries() {
    let model = br#"{"sdfProperty":{"p":{"type":"string","pattern":"^(?:(a+)\\1*)+$"}}}"#;
    let hard = format!("{}!", "a".repeat(60));
    let raw = serde_json::to_vec(&hard).unwrap();
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("hard", &raw).sample(),
        SourceInput::supplied("easy", br#""aa""#).sample(),
    ])
    .unwrap();
    let incomplete = sample(&catalogue, "hard");
    assert_eq!(
        incomplete.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!incomplete.outcome().complete());
    assert_eq!(incomplete.outcome().errors(), 0);
    assert!(
        incomplete
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "operation.limit")
    );
    let easy = sample(&catalogue, "easy");
    assert_eq!(
        easy.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert!(easy.outcome().complete());
    let mut lines = String::new();
    for value in [&hard, "aa", "b", "a"] {
        lines.push_str(
            &serde_json::json!({"kind":"property","pointer":"/sdfProperty/p","value":value})
                .to_string(),
        );
        lines.push('\n');
    }
    let result = catalogue
        .check_trace(TraceRequest::new(
            catalogue.source_id("model").unwrap(),
            lines.as_bytes(),
        ))
        .unwrap();
    assert_eq!(
        result.count(TraceOutcome::Undetermined),
        1,
        "{}",
        result.to_human()
    );
    assert_eq!(result.count(TraceOutcome::Passed), 2);
    assert_eq!(result.count(TraceOutcome::Failed), 1);
}

#[test]
fn repeated_array_values_do_not_spend_a_javascript_poll_allowance() {
    let model =
        br#"{"sdfProperty":{"p":{"type":"array","items":{"sdfChoice":{"text":{"type":"string","pattern":"^a$"}}}}}}"#;
    let values = serde_json::to_vec(&vec!["a"; 200]).unwrap();
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("values", &values).sample(),
    ])
    .unwrap();
    let report = sample(&catalogue, "values");
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted,
        "{}",
        report.to_human()
    );
    assert!(report.outcome().complete());
}

#[test]
fn ordinary_long_searches_keep_complete_match_and_mismatch_results() {
    let value = serde_json::to_vec(&"a".repeat(1024 * 1024)).unwrap();
    for (pattern, expected) in [("^a+$", Admission::Admitted), ("z", Admission::Rejected)] {
        let model = serde_json::json!({"sdfProperty":{"p":{"type":"string","pattern":pattern}}})
            .to_string();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("value", &value).sample(),
        ])
        .unwrap();
        let report = sample(&catalogue, "value");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{}",
            report.to_human()
        );
        assert!(report.outcome().complete());
    }
}
