use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, RecordedRegistryPolicy,
    SourceInput,
};
use serde_json::{Value, json};

fn source(format: &str) -> String {
    json!({"info":{},"sdfData":{"D":{"contentFormat":format}}}).to_string()
}
fn status(format: &str) -> AssessmentStatus {
    let text = source(format);
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
    cat.check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap()
        .requirement_status("rfc9880.model.M024/1")
        .require()
        .unwrap()
}

#[test]
fn supported_media_parameters_and_ordered_codings_establish_full_meaning() {
    for text in [
        "text/plain;charset=utf-8@gzip",
        "Text/Plain;CHARSET=\"UTF-8\"@GZIP@deflate",
        r#"text/plain;charset="u\tf-8""#,
        "text/plain",
        "text/plain;charset=us-ascii",
        "application/json",
        "application/json;charset=utf-8",
        "application/json;charset=made-up",
        "application/cbor",
        "application/json@gzip@gzip",
        "application/json@deflate@gzip",
    ] {
        assert_eq!(status(text), AssessmentStatus::Assessed, "{text}");
    }
}
#[test]
fn unsupported_media_rules_parameters_and_duplicates_remain_unassessed() {
    for text in [
        "text/plain;foo=bar",
        "text/plain;charset=utf-16",
        "text/plain;charset=utf-8;charset=utf-8",
        "text/plain;CHARSET=utf-8;charset=us-ascii",
        "application/cbor;p=x",
        "multipart/mixed",
        "application/cose",
    ] {
        assert_eq!(status(text), AssessmentStatus::Unassessed, "{text}");
    }
}
#[test]
fn numeric_formats_establish_the_complete_recorded_tuple() {
    for text in ["0", "16", "50", "60", "11050", "12000"] {
        assert_eq!(status(text), AssessmentStatus::Assessed, "{text}");
    }
}
#[test]
fn unknown_withheld_and_reserved_meaning_remains_undetermined() {
    for text in [
        "application/json@identity",
        "application/json@unfamiliar",
        "application/x-unfamiliar-example",
    ] {
        assert_eq!(status(text), AssessmentStatus::Undetermined, "{text}");
    }
    for text in ["0", "text/plain;charset=utf-8@gzip"] {
        let source = source(text);
        let cat =
            Catalogue::from_inputs(&[SourceInput::supplied("model", source.as_bytes())]).unwrap();
        let report = cat
            .check(
                CheckRequest::new()
                    .assessment(Assessment::FullBaseline)
                    .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded),
            )
            .unwrap();
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M024/1")
                .require()
                .unwrap(),
            AssessmentStatus::Undetermined
        );
    }
}
#[test]
fn actual_meaning_consultation_survives_whole_optional_omission() {
    let text = source("text/plain;charset=utf-8@gzip");
    for limits in [
        Limits::default(),
        Limits::default().conditions(1).diagnostic_bytes(1),
    ] {
        let cat =
            Catalogue::with_limits(&[SourceInput::supplied("model", text.as_bytes())], limits)
                .unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M024/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            wire["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == "content-format-model-meaning/1")
        );
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "content_format.parameter_semantics_unassessed")
        );
    }
}
#[test]
fn expanded_content_format_retains_its_parameter_notice_and_scope() {
    let text = source("text/plain;charset=utf-8@gzip");
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedContentFormat))
        .unwrap();
    assert!(report.outcome().complete());
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.parameter_semantics_unassessed")
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        !wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "content-format-model-meaning/1")
    );
}

#[test]
fn real_full_request_can_consult_all_ten_interpretations_together() {
    let source = br##"{"info":{},"sdfData":{
        "mapped":{"unit":"urn:ietf:params:unit:m%3As"},
        "marker":{"unit":"urn:ietf:PARAMS:unit:m%3As"},
        "number":{"contentFormat":"60"},
        "range":{"contentFormat":"65536"},
        "content":{"contentFormat":"text/plain;charset=utf-8@gzip"},
        "civil":{"type":"string","format":"date-time","const":"2026-10-07T00:00:00Z"},
        "bytes":{"type":"string","sdfType":"byte-string","const":"AQ"},
        "unix":{"type":"number","sdfType":"unix-time","const":0},
        "uuid":{"type":"string","format":"uuid","const":"00000000-0000-0000-0000-000000000000"}
    },"sdfObject":{
        "Base":{"sdfData":{"copied":{"type":"boolean"}}},
        "Use":{"sdfRef":"#/sdfObject/Base","sdfProperty":{"p":{"sdfRef":"#/sdfObject/Use/sdfData/copied"}}}
    }}"##;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", source)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let ids = wire["baseline"]["interpretations"].as_array().unwrap();
    assert_eq!(ids.len(), 10, "{ids:?}");
    assert_eq!(
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
}

#[test]
fn inherited_overridden_and_deleted_content_qualities_keep_final_observations() {
    let source = br##"{"info":{},"sdfData":{
        "base":{"contentFormat":"text/plain;charset=utf-8@gzip"},
        "inherited":{"sdfRef":"#/sdfData/base"},
        "replaced":{"sdfRef":"#/sdfData/base","contentFormat":"application/json"},
        "deleted":{"sdfRef":"#/sdfData/base","contentFormat":null}
    }}"##;
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", source)]).unwrap();
    let report = cat
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_eq!(
        report
            .requirement_status("rfc9880.model.M024/1")
            .require()
            .unwrap(),
        AssessmentStatus::Assessed
    );
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "content_format.model_meaning_unassessed")
    );
}

#[test]
fn unfinished_siblings_and_critical_context_cannot_gain_completion_from_supported_names() {
    for source in [
        r#"{"info":{},"sdfData":{"a":{"contentFormat":"application/json"},"b":{"contentFormat":"multipart/mixed"}}}"#,
        r#"{"info":{"features":["future"]},"sdfData":{"a":{"contentFormat":"application/json"}}}"#,
    ] {
        let cat =
            Catalogue::from_inputs(&[SourceInput::supplied("model", source.as_bytes())]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert!(matches!(
            report
                .requirement_status("rfc9880.model.M024/1")
                .require()
                .unwrap(),
            AssessmentStatus::Unassessed | AssessmentStatus::Undetermined
        ));
        assert!(!report.outcome().complete());
    }
}

#[test]
fn every_recorded_numeric_tuple_is_available_without_unrelated_registry_queries() {
    let rows: Value = serde_json::from_str(include_str!(
        "../fixtures/content-format/coap-positive.json"
    ))
    .unwrap();
    for row in rows.as_array().unwrap() {
        let text = source(&row["id"].as_u64().unwrap().to_string());
        let cat =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M024/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed,
            "{}",
            row["id"]
        );
        assert!(
            report
                .registry_knowledge("coap-content-formats")
                .present()
                .is_some()
        );
        assert!(matches!(
            report.registry_knowledge("media-types"),
            esdieff::Field::Absent
        ));
        assert!(matches!(
            report.registry_knowledge("http-content-codings"),
            esdieff::Field::Absent
        ));
    }
}

#[test]
fn full_meaning_work_exhaustion_preserves_an_earlier_syntax_error_with_no_detail() {
    let source=br#"{"info":{},"sdfData":{"bad":{"contentFormat":"text/plain;"},"valid":{"contentFormat":"text/plain;charset=utf-8@gzip@deflate@gzip"}}}"#;
    let mut seen = false;
    for work in (1..40000).step_by(100) {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("model", source)],
            Limits::default()
                .decoding_work(work)
                .diagnostic_bytes(1)
                .conditions(1),
        )
        .unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let consulted = wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "content-format-model-meaning/1");
        if consulted
            && report
                .requirement_status("rfc9880.model.M024/1")
                .require()
                .unwrap()
                == AssessmentStatus::Undetermined
        {
            assert_eq!(report.outcome().errors(), 1);
            assert!(report.diagnostics().conditions().next().is_none());
            assert!(!report.outcome().complete());
            seen = true;
            break;
        }
    }
    assert!(seen, "exercise a post-consultation shared-work stop");
}
