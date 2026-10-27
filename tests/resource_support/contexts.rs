use super::format_use_contract::{GENERIC, assert_sample_use};
use esdieff::{
    Admission, AssessmentStatus, Catalogue, Class, Field, Limits, SampleKind, SampleRequest,
    SourceInput,
};
use serde_json::Value;

fn data<'a>(cat: &'a Catalogue) -> esdieff::Report<'a> {
    cat.check_sample(
        SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
            .sample(cat.source_id("s").unwrap()),
    )
    .unwrap()
}

#[test]
fn scalar_const_uses_both_denotations_and_all_independent_constraints() {
    use Admission::{Admitted as A, Rejected as R, Undetermined as U};
    for (id, qualities, sample, expected, complete) in [
        (
            "uuid-case",
            r#""format":"uuid","const":"F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6""#,
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#.as_slice(),
            A,
            true,
        ),
        (
            "uuid-byte-difference",
            r#""format":"uuid","const":"f81d4fae-7dec-11d0-a765-00a0c91e6bf7""#,
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#.as_slice(),
            R,
            true,
        ),
        (
            "uuid-invalid-identical",
            r#""format":"uuid","const":"not-a-uuid""#,
            br#""not-a-uuid""#.as_slice(),
            R,
            false,
        ),
        (
            "uuid-equal-too-long",
            r#""format":"uuid","const":"f81d4fae-7dec-11d0-a765-00a0c91e6bf6","maxLength":1"#,
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#.as_slice(),
            R,
            true,
        ),
        (
            "uri-identical",
            r#""format":"uri","const":"https://example.test/""#,
            br#""https://example.test/""#.as_slice(),
            A,
            true,
        ),
        (
            "uri-host-case",
            r#""format":"uri","const":"https://EXAMPLE.test/""#,
            br#""https://example.test/""#.as_slice(),
            U,
            false,
        ),
        (
            "uri-percent-case",
            r#""format":"uri","const":"https://example.test/%2f""#,
            br#""https://example.test/%2F""#.as_slice(),
            U,
            false,
        ),
        (
            "uri-unknown-identical",
            r#""format":"uri","const":"custom:device""#,
            br#""custom:device""#.as_slice(),
            U,
            false,
        ),
        (
            "reference-identical-expression",
            r#""format":"uri-reference","const":"device""#,
            br#""device""#.as_slice(),
            A,
            true,
        ),
        (
            "reference-path-spelling",
            r#""format":"uri-reference","const":"./device""#,
            br#""device""#.as_slice(),
            U,
            false,
        ),
        (
            "reference-invalid-identical",
            r#""format":"uri-reference","const":"a%GG""#,
            br#""a%GG""#.as_slice(),
            R,
            false,
        ),
    ] {
        let model = format!(
            r#"{{"info":{{}},"sdfData":{{"V":{{"type":"string","nullable":false,{qualities}}}}}}}"#
        );
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = data(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{id}"
        );
        assert_eq!(report.outcome().complete(), complete, "{id}");
        if id.ends_with("invalid-identical") {
            assert_eq!(
                report
                    .assessment_area_status("sample-const/1")
                    .require()
                    .unwrap(),
                AssessmentStatus::Undetermined
            );
            assert!(
                !report
                    .diagnostics()
                    .conditions()
                    .any(|condition| condition.code() == "sample.const")
            );
        }
    }
}

#[test]
fn nested_alternatives_keep_complete_witnesses_and_common_meaning_separate() {
    for (id, qualities, sample, expected) in [
        (
            "known-witness",
            r#""type":"string","sdfChoice":{"Known":{"format":"uuid"},"Unknown":{"format":"custom"}}"#,
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#.as_slice(),
            Admission::Admitted,
        ),
        (
            "unknown-may-match",
            r#""type":"string","sdfChoice":{"Known":{"format":"uuid"},"Unknown":{"format":"custom"}}"#,
            br#""not-a-uuid""#.as_slice(),
            Admission::Undetermined,
        ),
        (
            "nested-case-const",
            r#""type":"string","format":"uuid","const":"F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6","sdfChoice":{"Outer":{"sdfChoice":{"Inner":{}}}}"#,
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#.as_slice(),
            Admission::Admitted,
        ),
        (
            "explicit-ordinary-override",
            r#""type":"string","format":"uuid","sdfChoice":{"Ordinary":{"format":null,"const":"ordinary"}}"#,
            br#""ordinary""#.as_slice(),
            Admission::Undetermined,
        ),
        // Ordinary null is not a merge-patch deletion outside a referenced patch.
        // This control must stay unavailable rather than silently erasing format.
        (
            "incompatible-common",
            r#""type":"string","const":"f81d4fae-7dec-11d0-a765-00a0c91e6bf6","sdfChoice":{"U":{"format":"uri"},"Q":{"format":"uuid"}}"#,
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#.as_slice(),
            Admission::Undetermined,
        ),
        (
            "null-branch",
            r#""type":"string","format":"uuid","nullable":false,"sdfChoice":{"Null":{"nullable":true}}"#,
            b"null".as_slice(),
            Admission::Admitted,
        ),
    ] {
        let model = format!(r#"{{"info":{{}},"sdfData":{{"V":{{{qualities}}}}}}}"#);
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = data(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{id}"
        );
        // Additional production cases must reverse branch order and use a real
        // sdfRef patch to delete shared format/const before ordinary inheritance.
        // Do not turn ordinary JSON null above into patch-deletion behavior.
    }
}

#[test]
fn copied_format_override_keeps_source_effective_and_sample_provenance() {
    let model = br##"{"info":{},"sdfData":{"U":{"type":"string","nullable":false,"format":"uri"}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/U","format":"uuid"}},"sdfEvent":{"e":{"sdfOutputData":{"sdfRef":"#/sdfData/U"}}}}"##;
    let sample = br#""../device""#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", sample).sample(),
    ])
    .unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(
                cat.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .description()
            .require()
            .unwrap()
            .format()
            .require()
            .unwrap(),
        "uuid"
    );
    let donor = cat
        .source(cat.source_id("m").unwrap())
        .unwrap()
        .definitions(Class::Data)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert_eq!(
        donor
            .data_description()
            .require()
            .unwrap()
            .format()
            .require()
            .unwrap(),
        "uri"
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["result"]["sample_location"]["span"],
        serde_json::json!([0, 11])
    );
    assert!(
        wire["diagnostics"]["items"].as_array().unwrap().iter().any(
            |condition| condition["domain"] == "sample"
                && condition["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["kind"] == "effective"
                        && location["pointer"] == "/sdfProperty/p/format")
        )
    );
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|location| location["kind"] == "source"
                    && location["pointer"] == "/sdfProperty/p/format"))
    );
    let event = cat
        .check_sample(
            SampleRequest::new(
                cat.source_id("m").unwrap(),
                SampleKind::EventOutput,
                "/sdfEvent/e",
            )
            .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        event
            .sample_result()
            .unwrap()
            .description()
            .require()
            .unwrap()
            .format()
            .require()
            .unwrap(),
        "uri"
    );
    assert_eq!(
        cat.source(cat.source_id("m").unwrap()).unwrap().raw_bytes(),
        model
    );
}

#[test]
fn unknown_scheme_keeps_independent_length_rejection_and_optional_zero_report() {
    let model = br#"{"info":{},"sdfData":{"V":{"type":"string","format":"uri","maxLength":1}}}"#;
    let sample = br#""mailto:operator@example.test""#;
    // Original unchanged two one-byte caller keys and two source records.
    const CAP: usize = 12 + 2 * 4096 + 16384 + 163930;
    let inputs = [
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", sample).sample(),
    ];
    assert!(Catalogue::with_limits(&inputs, Limits::default().report_bytes(CAP - 1)).is_err());
    let cat = Catalogue::with_limits(
        &inputs,
        Limits::default()
            .report_bytes(CAP)
            .diagnostic_bytes(1)
            .conditions(1),
    )
    .unwrap();
    let report = data(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(!report.outcome().complete());
    assert!(report.outcome().errors() > 0);
    assert!(matches!(
        report.assessment_area_status("sample-abstract-values/1"),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    assert!(wire["coverage"]["rows"].as_array().unwrap().is_empty());
    assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
    assert!(wire["outcome"]["errors"].as_u64().unwrap() > 0);
    assert!(report.to_json().len() <= CAP);
    assert!(report.to_human().len() <= CAP);
    assert_sample_use(&report, &wire, &report.to_human(), &[GENERIC]);
}
