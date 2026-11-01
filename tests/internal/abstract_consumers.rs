use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, JsonBuilder, Limits, SampleKind, SampleRequest,
    SourceInput,
};
use serde_json::{Value, json};

fn wire(report: &esdieff::Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}
fn sample<'a>(cat: &'a Catalogue) -> esdieff::Report<'a> {
    cat.check_sample(
        SampleRequest::new(
            cat.source_id("m").unwrap(),
            SampleKind::Property,
            "/sdfProperty/p",
        )
        .sample(cat.source_id("s").unwrap()),
    )
    .unwrap()
}

#[test]
fn all_five_selectors_share_original_and_typed_abstract_channels() {
    for (kind, group, suffix) in [
        (SampleKind::Property, "sdfProperty", ""),
        (SampleKind::Data, "sdfData", ""),
        (SampleKind::ActionInput, "sdfAction", "/sdfInputData"),
        (SampleKind::ActionOutput, "sdfAction", "/sdfOutputData"),
        (SampleKind::EventOutput, "sdfEvent", "/sdfOutputData"),
    ] {
        for (convention, raw, expected) in [
            ("byte-string", r#""Zg""#, Admission::Admitted),
            ("byte-string", r#""_x""#, Admission::Rejected),
            ("unix-time", "1.5", Admission::Admitted),
            ("unix-time", "-1", Admission::Undetermined),
        ] {
            let quality = json!({"sdfType":convention});
            let description = if suffix.is_empty() {
                quality
            } else {
                json!({&suffix[1..]:quality})
            };
            let model = json!({"info":{},group:{"p":description}}).to_string();
            for typed in [false, true] {
                let input = if typed {
                    let builder = JsonBuilder::new();
                    SourceInput::typed(
                        "s",
                        if convention == "byte-string" {
                            builder
                                .text(&serde_json::from_str::<String>(raw).unwrap())
                                .unwrap()
                        } else {
                            builder.number(raw).unwrap()
                        },
                    )
                } else {
                    SourceInput::supplied("s", raw.as_bytes()).sample()
                };
                let cat = Catalogue::take_inputs(vec![
                    SourceInput::supplied("m", model.as_bytes()),
                    input,
                ])
                .unwrap();
                let report = cat
                    .check_sample(
                        SampleRequest::new(
                            cat.source_id("m").unwrap(),
                            kind,
                            &format!("/{group}/p"),
                        )
                        .sample(cat.source_id("s").unwrap()),
                    )
                    .unwrap();
                assert_eq!(
                    report.sample_result().unwrap().admission(),
                    expected,
                    "{group}{suffix} {convention} {raw}"
                );
                assert_eq!(
                    wire(&report)["result"]["sample_location"]["span"].is_null(),
                    typed
                );
                assert_eq!(wire(&report)["coverage"]["requested"]["total"], 9);
            }
        }
    }
}

#[test]
fn nested_compounds_const_and_uniqueness_use_the_same_common_denotation() {
    for (convention, raw, expected, unique) in [
        (
            "byte-string",
            r#"[{"v":"_w"},{"v":"\u005fw"}]"#,
            Admission::Rejected,
            true,
        ),
        (
            "byte-string",
            r#"[{"v":"_w"},{"v":"-w"}]"#,
            Admission::Admitted,
            true,
        ),
        (
            "byte-string",
            r#"[{"v":"_x"},{"v":"_x"}]"#,
            Admission::Rejected,
            false,
        ),
        (
            "unix-time",
            r#"[{"v":1},{"v":1e0}]"#,
            Admission::Rejected,
            true,
        ),
        (
            "unix-time",
            r#"[{"v":1},{"v":2}]"#,
            Admission::Admitted,
            true,
        ),
        (
            "unix-time",
            r#"[{"v":-1},{"v":-1}]"#,
            Admission::Undetermined,
            false,
        ),
    ] {
        let model = json!({"info":{},"sdfProperty":{"p":{"type":"array","uniqueItems":true,
            "items":{"type":"object","properties":{"v":{"sdfType":convention}}}}}})
        .to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = sample(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{convention} {raw}"
        );
        let value = wire(&report);
        let duplicates = value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "sample.unique");
        assert_eq!(duplicates, unique && expected == Admission::Rejected);
        if duplicates {
            assert!(
                value["result"]["meaning"]["admission"]["basis"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|basis| basis["citation"] == "RFC 9880 Appendix C.4")
            );
        }
    }
    let model=br#"{"sdfProperty":{"p":{"type":"object","properties":{"v":{"sdfType":"unix-time"}},"const":{"v":10}}}}"#;
    for (raw, expected) in [
        (br#"{"v":1e1}"#.as_slice(), Admission::Admitted),
        (br#"{"v":11}"#.as_slice(), Admission::Rejected),
    ] {
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", raw).sample(),
        ])
        .unwrap();
        assert_eq!(sample(&cat).sample_result().unwrap().admission(), expected);
    }
}

#[test]
fn consistency_and_full_keep_their_current_inventories_and_actual_use() {
    for (convention, constant, default, expected, warning) in [
        ("byte-string", json!("_w"), json!("-w"), "assessed", true),
        ("unix-time", json!(1), json!(2), "assessed", true),
        ("unix-time", json!(-1), json!(-1), "undetermined", false),
    ] {
        let model=json!({"info":{},"sdfData":{"v":{"sdfType":convention,"const":constant,"default":default}}}).to_string();
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("m", model.as_bytes())]).unwrap();
        for assessment in [Assessment::ExpandedConsistency, Assessment::FullBaseline] {
            let report = cat
                .check(CheckRequest::new().assessment(assessment))
                .unwrap();
            let value = wire(&report);
            assert!(value["result"].get("meaning").is_none());
            assert_eq!(value["version"], "1.0");
            if assessment == Assessment::ExpandedConsistency {
                assert_eq!(
                    report
                        .assessment_area_status("default-consistency/1")
                        .require()
                        .unwrap()
                        .as_str(),
                    expected
                );
                assert_eq!(
                    value["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| finding["code"] == "consistency.default_excluded"),
                    warning
                );
                let id = if convention == "byte-string" {
                    "byte-string-json/1"
                } else {
                    "unix-time-posix-coordinate/1"
                };
                assert!(
                    value["baseline"]["interpretations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|entry| entry == id)
                );
            }
            assert_eq!(
                value["coverage"]["requested"]["total"],
                if assessment == Assessment::FullBaseline {
                    13
                } else {
                    10
                }
            );
            if assessment == Assessment::FullBaseline {
                assert_eq!(value["coverage"]["requirements"]["total"], 54);
            }
        }
    }
}

#[test]
fn actual_numeric_remainder_refusal_retains_consultation_and_no_false_const() {
    let model = br#"{"sdfProperty":{"p":{"sdfType":"unix-time","const":1}}}"#;
    let mut found = false;
    for cap in 1..128 {
        let cat = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model),
                SourceInput::supplied("s", b"1").sample(),
            ],
            Limits::default().numeric_work(cap),
        )
        .unwrap();
        let report = sample(&cat);
        let selected = report.sample_result().unwrap();
        if selected
            .sample_value()
            .require()
            .unwrap()
            .number()
            .present()
            .is_none()
        {
            continue;
        }
        let meaning = selected.meaning();
        if meaning
            .const_values()
            .limitations()
            .any(|basis| basis.id() == Some("sample-number-work-limit"))
        {
            assert_eq!(selected.admission(), Admission::Undetermined);
            assert!(
                selected
                    .consulted_interpretations()
                    .any(|id| id == "unix-time-posix-coordinate/1")
            );
            assert!(
                meaning
                    .const_values()
                    .basis()
                    .all(|basis| basis.citation() != Some("RFC 9880 Appendix C"))
            );
            assert!(meaning.admission().basis().next().is_none());
            found = true;
            break;
        }
    }
    assert!(
        found,
        "a real catalogue remainder must reach the failing comparison"
    );
}
