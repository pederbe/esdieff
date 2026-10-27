use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, Report, SourceInput};
use serde_json::json;

fn model(source: &str, limits: Limits) -> Catalogue {
    Catalogue::with_limits(&[SourceInput::supplied("model", source.as_bytes())], limits).unwrap()
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
fn superseded_shared_pattern_requests_only_final_matching_capability() {
    let source = json!({"info":{},"sdfData":{"D":{
        "type":"string","pattern":format!("(?<{}>a)", "x".repeat(125)),
        "sdfChoice":{"one":{"pattern":"a"},"two":{"pattern":"b"}}
    }}})
    .to_string();
    let catalogue = model(&source, Limits::default());
    let report = full(&catalogue);
    for member in [42, 47, 48] {
        assert_eq!(status(&report, member), AssessmentStatus::Assessed);
    }
    assert!(report.outcome().complete());
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "operation.limit")
    );

    // Syntax of an authored shared parameter remains independently checked.
    let invalid = source.replace(&format!("(?<{}>a)", "x".repeat(125)), "(");
    let catalogue = model(&invalid, Limits::default());
    let report = full(&catalogue);
    assert_eq!(status(&report, 48), AssessmentStatus::Assessed);
    assert!(report.outcome().errors() > 0);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "pattern.syntax")
    );

    let inherited = source.replace(r#""two":{"pattern":"b"}"#, r#""two":{}"#);
    let catalogue = model(&inherited, Limits::default());
    assert_eq!(
        status(&full(&catalogue), 48),
        AssessmentStatus::Undetermined
    );
}

#[test]
fn whole_map_and_item_replacement_excludes_discarded_semantic_descendants() {
    for (description, members) in [
        (
            json!({"type":"object","properties":{"discarded":{
                "type":"future","sdfChoice":{"nested":{}}
            }},"sdfChoice":{
                "one":{"properties":{"present":{"type":"string"}}},
                "two":{"properties":{"present":{"type":"integer"}}}
            }}),
            &[42, 53, 54][..],
        ),
        (
            json!({"type":"array","items":{"type":"future"},"sdfChoice":{
                "one":{"items":{"type":"string"}},
                "two":{"items":{"type":"integer"}}
            }}),
            &[42, 50][..],
        ),
    ] {
        let source = json!({"info":{},"sdfData":{"D":description}}).to_string();
        let catalogue = model(&source, Limits::default());
        let report = full(&catalogue);
        for &member in members {
            assert_eq!(
                status(&report, member),
                AssessmentStatus::Assessed,
                "{source} M{member}"
            );
        }
    }
}

#[test]
fn scalar_and_equality_qualities_use_final_shared_replacements() {
    for (description, member, limits) in [
        (
            json!({"type":"future","sdfChoice":{"one":{"type":"string"},"two":{"type":"integer"}}}),
            42,
            Limits::default(),
        ),
        (
            json!({"type":"number","minimum":12345,"sdfChoice":{"one":{"minimum":0},"two":{"minimum":1}}}),
            45,
            Limits::default().number_token_bytes(2),
        ),
        (
            json!({"type":"string","minLength":12345,"sdfChoice":{"one":{"minLength":0},"two":{"minLength":1}}}),
            46,
            Limits::default().number_token_bytes(2),
        ),
        (
            json!({"type":"array","minItems":12345,"sdfChoice":{"one":{"minItems":0},"two":{"minItems":1}}}),
            51,
            Limits::default().number_token_bytes(2),
        ),
        (
            json!({"type":"string","format":"future","sdfChoice":{"one":{"format":"date"},"two":{"format":"time"}}}),
            49,
            Limits::default(),
        ),
        (
            json!({"type":"string","sdfType":"byte-string","sdfChoice":{"one":{"type":"number","sdfType":"unix-time"},"two":{"type":"number","sdfType":"unix-time"}}}),
            26,
            Limits::default(),
        ),
        (
            json!({"type":"array","uniqueItems":true,"items":{"type":"string","format":"uuid"},"sdfChoice":{"one":{"uniqueItems":false},"two":{"uniqueItems":false}}}),
            52,
            Limits::default(),
        ),
    ] {
        let source = json!({"info":{},"sdfData":{"D":description}}).to_string();
        let catalogue = model(&source, limits);
        let report = full(&catalogue);
        assert_eq!(
            status(&report, member),
            if member == 26 {
                AssessmentStatus::NotApplicable
            } else {
                AssessmentStatus::Assessed
            },
            "{source}"
        );
    }
    let catalogue = model(
        r#"{"info":{},"sdfData":{"D":{"type":"object","required":["\ud800"],"sdfChoice":{"one":{"required":["known"]},"two":{"required":["known"]}}}}}"#,
        Limits::default(),
    );
    assert_eq!(status(&full(&catalogue), 54), AssessmentStatus::Assessed);
}

#[test]
fn inherited_unknown_and_unestablished_alternatives_remain_unfinished() {
    for description in [
        json!({"type":"object","properties":{"needed":{"type":"future"}},"sdfChoice":{"one":{"properties":{}},"two":{}}}),
        json!({"type":"future","sdfChoice":{"one":{"type":"string"},"two":{}}}),
        json!({"type":"future","sdfChoice":[]}),
        json!({"type":"future","sdfChoice":{}}),
    ] {
        let source = json!({"info":{},"sdfData":{"D":description}}).to_string();
        let catalogue = model(&source, Limits::default());
        let report = full(&catalogue);
        assert_eq!(
            status(&report, 42),
            AssessmentStatus::Undetermined,
            "{source}"
        );
    }
}

#[test]
fn discarded_extension_constants_do_not_impose_const_or_extension_meaning() {
    let source = json!({"info":{},"sdfData":{"D":{
        "type":"object","properties":{"discarded":{"const":[null]}},
        "sdfChoice":{"one":{"properties":{}},"two":{"properties":{}}}
    }}})
    .to_string();
    let catalogue = model(&source, Limits::default());
    let report = full(&catalogue);
    assert_eq!(status(&report, 35), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 28), AssessmentStatus::NotApplicable);
    assert_eq!(status(&report, 43), AssessmentStatus::NotApplicable);
    assert_eq!(status(&report, 53), AssessmentStatus::Assessed);

    // An owner's documentary default remains stated even if branches state defaults.
    let source = json!({"info":{},"sdfData":{"D":{"default":[null],
        "sdfChoice":{"one":{"default":0.5},"two":{"default":0.5}}
    }}})
    .to_string();
    let catalogue = model(&source, Limits::default());
    let report = full(&catalogue);
    assert_eq!(status(&report, 28), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 44), AssessmentStatus::Assessed);
}

#[test]
fn content_meaning_uses_final_qualities_while_registry_guidance_retains_scope() {
    for description in [
        json!({"contentFormat":"multipart/mixed","sdfChoice":{"one":{"contentFormat":"application/json"},"two":{"contentFormat":"application/cbor"}}}),
        json!({"type":"object","properties":{"discarded":{"contentFormat":"multipart/mixed","sdfChoice":{"nested":{}}}},"sdfChoice":{"one":{"properties":{}},"two":{"properties":{}}}}),
    ] {
        let source = json!({"info":{},"sdfData":{"D":description}}).to_string();
        let catalogue = model(&source, Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 24), AssessmentStatus::Assessed, "{source}");
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "content_format.model_meaning_unassessed")
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "content_format.registry_known")
        );
    }
    let source = json!({"info":{},"sdfData":{"D":{
        "contentFormat":"multipart/mixed","sdfChoice":{"one":{"contentFormat":"application/json"},"two":{}}
    }}}).to_string();
    let catalogue = model(&source, Limits::default());
    assert_eq!(status(&full(&catalogue), 24), AssessmentStatus::Unassessed);
}
