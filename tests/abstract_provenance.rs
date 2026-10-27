//! Composition keeps selected-channel authority and immutable input provenance.
use esdieff::{Admission, Catalogue, SampleKind, SampleRequest, SourceInput};
use serde_json::Value;
fn selected<'a>(cat: &'a Catalogue, pointer: &str) -> esdieff::Report<'a> {
    cat.check_sample(
        SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Property, pointer)
            .sample(cat.source_id("s").unwrap()),
    )
    .unwrap()
}
#[test]
fn local_overrides_deletions_and_constants_control_only_the_surviving_channel() {
    for (patch, raw, expected, id) in [
        (
            r#""sdfType":"unix-time""#,
            "1.5",
            Admission::Admitted,
            Some("unix-time-posix-coordinate/1"),
        ),
        (
            r#""sdfType":null"#,
            r#""notbase64""#,
            Admission::Admitted,
            None,
        ),
        (
            r#""const":"_w""#,
            r#""_w""#,
            Admission::Admitted,
            Some("byte-string-json/1"),
        ),
    ] {
        let patch: Value = serde_json::from_str(&format!("{{{patch}}}")).unwrap();
        let mut model = serde_json::json!({"sdfData":{"Base":{"sdfType":"byte-string","const":"Zg"}},"sdfProperty":{"p":patch}});
        model["sdfProperty"]["p"]["sdfRef"] = Value::String("#/sdfData/Base".into());
        // A local const override is needed in the POSIX/deletion cases as well.
        if id != Some("byte-string-json/1") {
            model["sdfProperty"]["p"]["const"] = Value::Null;
        }
        let model = model.to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat, "/sdfProperty/p");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{patch}"
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .consulted_interpretations()
                .collect::<Vec<_>>(),
            id.into_iter().collect::<Vec<_>>()
        );
        let original = cat
            .source(cat.source_id("m").unwrap())
            .unwrap()
            .json()
            .require()
            .unwrap();
        assert_eq!(
            original
                .get("sdfData")
                .require()
                .unwrap()
                .get("Base")
                .require()
                .unwrap()
                .get("const")
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap(),
            "Zg"
        );
    }
}
#[test]
fn missing_or_critical_needed_contributors_supply_no_lexical_consultation() {
    for model in [
        r##"{"sdfProperty":{"p":{"sdfType":"byte-string","sdfRef":"#/sdfData/missing"}}}"##,
        r#"{"info":{"features":["https://example.test/critical"]},"sdfProperty":{"p":{"sdfType":"byte-string"}}}"#,
    ] {
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""Zg""#).sample(),
        ])
        .unwrap();
        let report = selected(&cat, "/sdfProperty/p");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(
            report
                .sample_result()
                .unwrap()
                .consulted_interpretations()
                .next()
                .is_none()
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["sources"].as_array().unwrap().len(), 2);
    }
}
#[test]
fn escaped_declaring_and_choice_names_keep_actual_source_and_sample_coordinates() {
    let model = br#"{"sdfProperty":{"p~/":{"sdfChoice":{"x~/":{"sdfType":"byte-string"}}}}}"#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""_x""#).sample(),
    ])
    .unwrap();
    let report = selected(&cat, "/sdfProperty/p~0~1");
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "sample.byte_string_representation")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "");
    assert_eq!(finding["primary"]["span"], serde_json::json!([0, 4]));
    assert!(
        finding
            .to_string()
            .contains("p~0~1/sdfChoice/x~0~1/sdfType")
    );
}
#[test]
fn unrelated_malformed_source_is_visible_without_becoming_selected_model_work() {
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", br#"{"sdfProperty":{"p":{"sdfType":"byte-string"}}}"#),
        SourceInput::supplied("s", br#""Zg""#).sample(),
        SourceInput::supplied("unused", b"{broken"),
    ])
    .unwrap();
    let report = selected(&cat, "/sdfProperty/p");
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["sources"].as_array().unwrap().len(), 3);
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
}

#[test]
fn restricted_item_donor_meaning_requires_a_full_alternative_override() {
    for overridden in [false, true] {
        let alternative = if overridden {
            serde_json::json!({"sdfType":"byte-string"})
        } else {
            serde_json::json!({})
        };
        let model=serde_json::json!({"sdfProperty":{"p":{"type":"array","items":{"type":"string","sdfType":"byte-string","sdfChoice":{"a":alternative}}}}}).to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#"["Zg"]"#).sample(),
        ])
        .unwrap();
        let report = selected(&cat, "/sdfProperty/p");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            if overridden {
                Admission::Admitted
            } else {
                Admission::Undetermined
            }
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .consulted_interpretations()
                .any(|id| id == "byte-string-json/1"),
            overridden
        );
    }
}
