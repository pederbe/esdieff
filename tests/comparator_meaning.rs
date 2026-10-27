//! Actual current comparator causes follow decisive proof and whole witnesses.
use esdieff::{Admission, AssessmentStatus, Catalogue, SampleKind, SampleRequest, SourceInput};
use serde_json::{Value, json};
fn run(description: Value, raw: &str) -> (Catalogue, String) {
    let model = json!({"sdfProperty":{"p":description}}).to_string();
    (
        Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap(),
        model,
    )
}
fn report(cat: &Catalogue) -> esdieff::Report<'_> {
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
fn limitations(value: &Value, area: &str) -> Vec<(String, Option<String>)> {
    value["result"]["meaning"][area]["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|reference| {
            (
                reference["id"].as_str().unwrap().to_owned(),
                reference["citation"].as_str().map(str::to_owned),
            )
        })
        .collect()
}
#[test]
fn actual_resource_and_civil_pair_causes_keep_precise_and_joint_references() {
    for (format, constant, sample, ids) in [
        (
            "uri",
            "bad",
            "mailto:operator@example.test",
            vec![
                "sample-abstract-denotation-unavailable",
                "sample-establishment",
            ],
        ),
        (
            "date-time",
            "bad",
            "2030-06-30T23:59:60Z",
            vec![
                "sample-abstract-denotation-unavailable",
                "temporal-knowledge",
            ],
        ),
        (
            "time",
            "00:00:00Z",
            "23:59:60Z",
            vec!["sample-establishment"],
        ),
        (
            "uri",
            "https://example.test/a",
            "https://example.test/b",
            vec!["sample-establishment"],
        ),
    ] {
        let raw = serde_json::to_string(sample).unwrap();
        for typed in [false, true] {
            let description = json!({"type":"string","format":format,"const":constant});
            let cat = if typed {
                let model = json!({"sdfProperty":{"p":description}}).to_string();
                Catalogue::take_inputs(vec![
                    SourceInput::supplied("m", model.as_bytes()),
                    SourceInput::typed("s", esdieff::JsonBuilder::new().text(sample).unwrap()),
                ])
                .unwrap()
            } else {
                run(description, &raw).0
            };
            let report = report(&cat);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Undetermined
            );
            assert_eq!(report.outcome().errors(), 0);
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            let actual = limitations(&wire, "const");
            let mut actual_ids: Vec<_> = actual.iter().map(|item| item.0.as_str()).collect();
            actual_ids.sort_unstable();
            let mut expected_ids = ids.clone();
            expected_ids.sort_unstable();
            assert_eq!(actual_ids, expected_ids, "{format} {sample}");
            assert!(
                actual
                    .iter()
                    .filter(
                        |item| item.0 == "sample-establishment" || item.0 == "temporal-knowledge"
                    )
                    .all(|item| item.1.is_some())
            );
            assert!(
                wire["result"]["meaning"]["admission"]["basis"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(report.to_human().contains("Sample const limitations: "));
            let expected = wire["result"]["meaning"]["const"]["limitations"]
                .as_array()
                .unwrap();
            let borrowed: Vec<_> = report
                .sample_result()
                .unwrap()
                .meaning()
                .const_values()
                .limitations()
                .collect();
            assert_eq!(borrowed.len(), expected.len());
            let mut human = Vec::new();
            for (reference, value) in borrowed.into_iter().zip(expected) {
                assert_eq!(reference.kind(), esdieff::BasisKind::Operational);
                assert_eq!(value["kind"], "operational");
                assert_eq!(reference.id(), value["id"].as_str());
                assert_eq!(reference.citation(), value["citation"].as_str());
                human.push(match reference.citation() {
                    Some(citation) => format!("{} ({citation})", reference.id().unwrap()),
                    None => reference.id().unwrap().to_owned(),
                });
            }
            assert!(
                report
                    .to_human()
                    .lines()
                    .any(|line| line == format!("Sample const limitations: {}", human.join("; ")))
            );
            assert_eq!(wire["result"]["sample_location"]["span"].is_null(), typed);
        }
    }
}
#[test]
fn a_later_compound_difference_keeps_independent_unfinished_abstract_area() {
    let (cat, _) = run(
        json!({"type":"object","properties":{"a":{"type":"string","format":"uri"},"z":{"type":"number"}},"const":{"a":"mailto:a@example.test","z":1}}),
        r#"{"a":"mailto:a@example.test","z":2}"#,
    );
    let report = report(&cat);
    let sample = report.sample_result().unwrap();
    assert_eq!(sample.admission(), Admission::Rejected);
    assert!(!report.outcome().complete());
    assert_eq!(
        sample.meaning().const_values().status(),
        AssessmentStatus::Assessed
    );
    assert!(
        sample
            .meaning()
            .const_values()
            .limitations()
            .next()
            .is_none()
    );
    assert_eq!(
        sample.meaning().abstract_values().status(),
        AssessmentStatus::Undetermined
    );
    assert!(
        sample
            .meaning()
            .abstract_values()
            .limitations()
            .any(|reference| reference.id() == Some("sample-establishment")
                && reference.citation().unwrap().contains("ADR 0019"))
    );
    assert!(sample.meaning().admission().limitations().next().is_none());
    assert!(
        sample
            .meaning()
            .admission()
            .basis()
            .any(|reference| reference.citation() == Some("RFC 9880 Appendix C.1"))
    );
}
#[test]
fn repeated_enum_and_choice_witnesses_discard_only_tentative_causes() {
    let uuid = "f81d4fae-7dec-11d0-a765-00a0c91e6bf6";
    let raw = serde_json::to_string(uuid).unwrap();
    for winning in [true, false] {
        let later = if winning {
            uuid
        } else {
            "00000000-0000-0000-0000-000000000000"
        };
        let (cat, _) = run(
            json!({"type":"string","format":"uuid","enum":["bad",later]}),
            &raw,
        );
        let report = report(&cat);
        let sample = report.sample_result().unwrap();
        assert_eq!(
            sample.admission(),
            if winning {
                Admission::Admitted
            } else {
                Admission::Undetermined
            }
        );
        if winning {
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .limitations()
                    .next()
                    .is_none()
            );
        } else {
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .limitations()
                    .any(|reference| reference.id()
                        == Some("sample-abstract-denotation-unavailable"))
            );
        }
        assert_eq!(
            sample.meaning().abstract_values().status(),
            AssessmentStatus::Assessed
        );
        assert_eq!(report.outcome().errors(), 0);
    }
    let (cat, _) = run(
        json!({"sdfChoice":{"a":{"type":"string","format":"uri"},"b":{"type":"string"}}}),
        r#""mailto:a@example.test""#,
    );
    let report = report(&cat);
    let sample = report.sample_result().unwrap();
    assert_eq!(sample.admission(), Admission::Admitted);
    assert!(sample.meaning().admission().limitations().next().is_none());
    assert_eq!(
        sample.meaning().abstract_values().status(),
        AssessmentStatus::NotApplicable
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        !wire["baseline"]["format_knowledge"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn unavailable_unicode_scalar_decoding_is_input_limit_and_preserves_independent_rejection() {
    for nested in [false, true] {
        let description = if nested {
            json!({"type":"object","properties":{"a":{"sdfType":"byte-string"},"z":{"type":"integer"}}})
        } else {
            json!({"sdfType":"byte-string"})
        };
        let raw = if nested {
            r#"{"a":"\uD800","z":0.5}"#
        } else {
            r#""\uD800""#
        };
        let (cat, _) = run(description, raw);
        let report = report(&cat);
        let sample = report.sample_result().unwrap();
        assert_eq!(
            sample.admission(),
            if nested {
                Admission::Rejected
            } else {
                Admission::Undetermined
            }
        );
        assert!(!report.outcome().complete());
        let source = cat.source(cat.source_id("s").unwrap()).unwrap();
        assert_eq!(source.raw_bytes(), raw.as_bytes());
        let root = source.sample_value().require().unwrap();
        let text = if nested {
            root.get("a").require().unwrap()
        } else {
            root
        };
        assert_eq!(text.kind(), esdieff::JsonKind::String);
        assert!(matches!(text.text(), esdieff::Field::Unavailable(_)));
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["result"]["sample_location"]["span"],
            json!([0, raw.len()])
        );
        let findings = wire["diagnostics"]["items"].as_array().unwrap();
        assert!(
            findings
                .iter()
                .any(|row| row["code"] == "input.unicode_unavailable")
        );
        assert!(
            !findings
                .iter()
                .any(|row| row["code"] == "sample.byte_string_representation")
        );
        assert!(
            findings
                .iter()
                .any(|row| row["code"] == "sample.coverage_unavailable"
                    && row["message"] == "Abstract scalar input is unavailable")
        );
        assert_eq!(
            sample.meaning().abstract_values().status(),
            AssessmentStatus::Undetermined
        );
        assert!(
            sample
                .meaning()
                .abstract_values()
                .limitations()
                .any(|reference| reference.id() == Some("sample-establishment")
                    && reference.citation().is_none())
        );
        assert!(
            sample
                .consulted_interpretations()
                .any(|id| id == "byte-string-json/1")
        );
        if nested {
            assert!(report.outcome().errors() > 0);
            assert!(
                sample
                    .meaning()
                    .admission()
                    .basis()
                    .any(|reference| reference.citation() == Some("RFC 9880 Appendix C.1"))
            );
            assert!(sample.meaning().admission().limitations().next().is_none());
        } else {
            assert_eq!(report.outcome().errors(), 0);
            assert!(sample.meaning().admission().basis().next().is_none());
        }
    }
}

#[test]
fn decisive_numeric_difference_excludes_earlier_partial_byte_const_support() {
    for byte_first in [true, false] {
        let properties = if byte_first {
            r#"{"a":{"sdfType":"byte-string"},"z":{"type":"number"}}"#
        } else {
            r#"{"a":{"type":"number"},"z":{"sdfType":"byte-string"}}"#
        };
        let constant = if byte_first {
            r#"{"a":"_x","z":1}"#
        } else {
            r#"{"a":1,"z":"_x"}"#
        };
        let raw = if byte_first {
            r#"{"a":"Zg","z":2}"#
        } else {
            r#"{"a":2,"z":"Zg"}"#
        };
        let description: Value = serde_json::from_str(&format!(
            r#"{{"type":"object","properties":{properties},"const":{constant}}}"#
        ))
        .unwrap();
        let (cat, _) = run(description, raw);
        let report = report(&cat);
        let sample = report.sample_result().unwrap();
        assert_eq!(sample.admission(), Admission::Rejected);
        assert_eq!(
            sample.meaning().const_values().status(),
            AssessmentStatus::Assessed
        );
        assert!(
            sample
                .meaning()
                .const_values()
                .limitations()
                .next()
                .is_none()
        );
        assert!(
            sample
                .meaning()
                .abstract_values()
                .basis()
                .any(|reference| reference.id() == Some("byte-string-json/1"))
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        eprintln!("DECISIVE_BYTE_COUNTEREXAMPLE byte_first={byte_first} {wire}");
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "sample.const")
            .unwrap();
        let key = if byte_first { "z" } else { "a" };
        assert_eq!(finding["primary"]["pointer"], format!("/{key}"));
        assert!(
            finding["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["pointer"] == format!("/sdfProperty/p/const/{key}"))
        );
        for references in [
            sample.meaning().const_values().basis().collect::<Vec<_>>(),
            sample.meaning().admission().basis().collect::<Vec<_>>(),
        ] {
            assert!(
                !references
                    .iter()
                    .any(|reference| reference.id() == Some("byte-string-json/1")),
                "byte_first={byte_first}: final decisive proof inherited unavailable-child Byte support"
            );
            assert!(
                references
                    .iter()
                    .any(|reference| reference.citation() == Some("RFC 9880 Appendix C.1"))
            );
        }
    }
}
