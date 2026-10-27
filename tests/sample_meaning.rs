//! Final support follows the decisive witness; consultation follows actual use.
use esdieff::{
    Admission, AssessmentStatus, BasisKind, Catalogue, JsonBuilder, Report, SampleBasisRef,
    SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;

const BYTE: &str = "byte-string-json/1";
const UNIX: &str = "unix-time-posix-coordinate/1";
const ORDINARY: &str = "RFC 9880 Appendix C";
const NUMERIC: &str = "RFC 9880 Appendix C.1";
const NEGATIVE: &str = "sample-abstract-denotation-unavailable";

fn scoped_tokens<'a>(human: &'a str, label: &str) -> Vec<&'a str> {
    let lines: Vec<_> = human
        .lines()
        .filter_map(|line| line.strip_prefix(label))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "one actual scoped human producer line: {label}"
    );
    let mut values: Vec<_> = if lines[0] == "none" {
        Vec::new()
    } else {
        lines[0].split("; ").collect()
    };
    values.sort_unstable();
    assert!(values.windows(2).all(|pair| pair[0] != pair[1]));
    values
}
pub(crate) fn assert_scoped_human(human: &str, value: &Value) {
    // Scoped human lines preserve the same ordered references as JSON.
    for (field, prefix) in [
        ("abstract_values", "Sample abstract values"),
        ("const", "Sample const"),
        ("admission", "Sample admission"),
    ] {
        for (key, suffix) in [("basis", " basis: "), ("limitations", " limitations: ")] {
            let expected = wire_human_keys(&value["result"]["meaning"][field][key]);
            let label = format!("{prefix}{suffix}");
            let lines: Vec<_> = human
                .lines()
                .filter_map(|line| line.strip_prefix(&label))
                .collect();
            assert_eq!(lines.len(), 1);
            assert_eq!(
                lines[0],
                if expected.is_empty() {
                    "none".to_owned()
                } else {
                    expected.join("; ")
                }
            );
        }
    }
    let mut ids: Vec<_> = value["baseline"]["interpretations"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| [BYTE, UNIX].contains(id))
        .collect();
    ids.sort_unstable();
    assert_eq!(
        scoped_tokens(human, "Consulted sample interpretations: "),
        ids
    );
    let sample_id = value["result"]["sample"].as_u64().unwrap();
    assert!(human.contains(&format!(
        "Sample {sample_id}: {}",
        value["result"]["admission"].as_str().unwrap()
    )));
    assert!(human.contains(&format!(
        "Complete: {}",
        value["outcome"]["complete"].as_bool().unwrap()
    )));
    for (field, area) in [
        ("abstract_values", "sample-abstract-values/1"),
        ("const", "sample-const/1"),
    ] {
        assert!(human.contains(&format!(
                "Sample area {area}: {}",
                value["result"]["meaning"][field]["status"]
                    .as_str()
                    .unwrap()
            )));
    }
}

fn selected(cat: &Catalogue) -> Report<'_> {
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
fn actual_wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}
fn key(reference: SampleBasisRef) -> &'static str {
    match reference.kind() {
        BasisKind::Rfc => {
            assert!(reference.id().is_none());
            reference.citation().unwrap()
        }
        BasisKind::Interpretation => {
            assert!(reference.citation().is_some());
            reference.id().unwrap()
        }
        BasisKind::Operational => reference.id().unwrap(),
        _ => panic!("unexpected Sample meaning classification"),
    }
}
fn wire_keys(array: &Value) -> Vec<&str> {
    array
        .as_array()
        .unwrap()
        .iter()
        .map(|basis| match basis["kind"].as_str().unwrap() {
            "rfc" => {
                assert!(basis.get("id").is_none());
                basis["citation"].as_str().unwrap()
            }
            "interpretation" => {
                assert!(basis["citation"].is_string());
                basis["id"].as_str().unwrap()
            }
            "operational" => basis["id"].as_str().unwrap(),
            _ => panic!("unexpected meaning wire basis"),
        })
        .collect()
}
fn wire_human_keys(array: &Value) -> Vec<String> {
    array
        .as_array()
        .unwrap()
        .iter()
        .map(
            |reference| match (reference["id"].as_str(), reference["citation"].as_str()) {
                (Some(id), Some(citation)) => format!("{id} ({citation})"),
                (Some(id), None) => id.to_owned(),
                (None, Some(citation)) => citation.to_owned(),
                _ => panic!("reference has no identity"),
            },
        )
        .collect()
}
pub(crate) fn assert_real_parity(report: &Report<'_>) {
    let sample = report.sample_result().unwrap();
    let meaning = sample.meaning();
    let value = actual_wire(report);
    let typed_triple = |reference: SampleBasisRef| {
        (
            match reference.kind() {
                BasisKind::Rfc => "rfc",
                BasisKind::Interpretation => "interpretation",
                BasisKind::Operational => "operational",
                _ => unreachable!(),
            },
            reference.id(),
            reference.citation(),
        )
    };
    let wire_triples = |value: &Value| -> Vec<(String, Option<String>, Option<String>)> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|reference| {
                (
                    reference["kind"].as_str().unwrap().to_owned(),
                    reference["id"].as_str().map(str::to_owned),
                    reference["citation"].as_str().map(str::to_owned),
                )
            })
            .collect()
    };
    let mut seen = Vec::new();
    for id in sample.consulted_interpretations() {
        assert!([BYTE, UNIX].contains(&id));
        assert!(!seen.contains(&id));
        seen.push(id);
    }
    let actual_sample_ids: Vec<_> = value["baseline"]["interpretations"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| [BYTE, UNIX].contains(id))
        .collect();
    assert_eq!(seen, actual_sample_ids);
    for (field, area, state) in [
        (
            "abstract_values",
            "sample-abstract-values/1",
            meaning.abstract_values(),
        ),
        ("const", "sample-const/1", meaning.const_values()),
    ] {
        assert_eq!(state.status(), sample.area_status(area).require().unwrap());
        assert_eq!(
            value["result"]["meaning"][field]["status"],
            state.status().as_str()
        );
        let supported: Vec<_> = state.basis().map(key).collect();
        let limited: Vec<_> = state.limitations().map(key).collect();
        assert_eq!(
            supported,
            wire_keys(&value["result"]["meaning"][field]["basis"])
        );
        assert_eq!(
            limited,
            wire_keys(&value["result"]["meaning"][field]["limitations"])
        );
        for (name, references) in [
            ("basis", state.basis().map(typed_triple).collect::<Vec<_>>()),
            (
                "limitations",
                state.limitations().map(typed_triple).collect::<Vec<_>>(),
            ),
        ] {
            assert_eq!(
                references
                    .into_iter()
                    .map(|(kind, id, citation)| (
                        kind.to_owned(),
                        id.map(str::to_owned),
                        citation.map(str::to_owned)
                    ))
                    .collect::<Vec<_>>(),
                wire_triples(&value["result"]["meaning"][field][name])
            );
        }
        if state.status() == AssessmentStatus::NotApplicable {
            assert!(supported.is_empty());
            assert!(limited.is_empty());
        }
    }
    assert_eq!(
        meaning.admission().basis().map(key).collect::<Vec<_>>(),
        wire_keys(&value["result"]["meaning"]["admission"]["basis"])
    );
    assert_eq!(
        meaning
            .admission()
            .limitations()
            .map(key)
            .collect::<Vec<_>>(),
        wire_keys(&value["result"]["meaning"]["admission"]["limitations"])
    );
    for (name, references) in [
        (
            "basis",
            meaning
                .admission()
                .basis()
                .map(typed_triple)
                .collect::<Vec<_>>(),
        ),
        (
            "limitations",
            meaning
                .admission()
                .limitations()
                .map(typed_triple)
                .collect::<Vec<_>>(),
        ),
    ] {
        assert_eq!(
            references
                .into_iter()
                .map(|(kind, id, citation)| (
                    kind.to_owned(),
                    id.map(str::to_owned),
                    citation.map(str::to_owned)
                ))
                .collect::<Vec<_>>(),
            wire_triples(&value["result"]["meaning"]["admission"][name])
        );
    }
    assert_eq!(value["result"]["admission"], sample.admission().as_str());
    assert_eq!(value["outcome"]["complete"], report.outcome().complete());
    assert_eq!(value["outcome"]["errors"], report.outcome().errors());
    let coverage = report.requested_coverage();
    assert_eq!(coverage.total(), Some(9));
    assert_eq!(
        coverage.assessed()
            + coverage.not_applicable()
            + coverage.unassessed()
            + coverage.undetermined(),
        9
    );
    let human = report.to_human();
    if human.trim_start().starts_with('{') {
        let fallback: Value = serde_json::from_str(&human).unwrap();
        assert_eq!(fallback["result"]["meaning"], value["result"]["meaning"]);
        assert_eq!(fallback["outcome"], value["outcome"]);
    } else {
        assert_scoped_human(&human, &value);
    }
}

#[test]
fn enum_winner_discards_earlier_unavailable_proof_and_limits() {
    for (qualities, raw, convention, expected) in [
        (
            r#"{"type":"string","sdfType":"byte-string","enum":["_x","Zg"]}"#,
            r#""Zg""#,
            BYTE,
            Admission::Admitted,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","sdfChoice":{"a":{"const":-1},"b":{"const":1}}}"#,
            "1",
            UNIX,
            Admission::Admitted,
        ),
        (
            r#"{"type":"string","sdfType":"byte-string","enum":["_x","Zm8"]}"#,
            r#""Zg""#,
            BYTE,
            Admission::Undetermined,
        ),
    ] {
        let model = format!(r#"{{"info":{{}},"sdfProperty":{{"p":{qualities}}}}}"#);
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_real_parity(&report);
        let sample = report.sample_result().unwrap();
        assert_eq!(sample.admission(), expected, "{qualities}");
        assert!(
            sample
                .consulted_interpretations()
                .any(|id| id == convention)
        );
        if expected == Admission::Admitted {
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .limitations()
                    .next()
                    .is_none()
            );
            assert!(sample.meaning().admission().limitations().next().is_none());
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .basis()
                    .map(key)
                    .any(|id| id == convention)
            );
            assert!(report.outcome().complete());
        } else {
            assert!(sample.meaning().admission().basis().next().is_none());
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .basis()
                    .map(key)
                    .any(|id| id == convention)
            );
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .limitations()
                    .next()
                    .is_some()
            );
        }
    }
}

#[test]
fn numeric_enum_keeps_the_existing_text_only_model_contract() {
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "m",
            br#"{"sdfProperty":{"p":{"type":"number","sdfType":"unix-time","enum":[-1,1]}}}"#,
        ),
        SourceInput::supplied("s", b"1").sample(),
    ])
    .unwrap();
    let report = selected(&cat);
    assert_real_parity(&report);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    assert!(
        report
            .sample_result()
            .unwrap()
            .consulted_interpretations()
            .next()
            .is_none()
    );
}

#[test]
fn discarded_byte_rejection_keeps_consultation_but_final_witness_is_ordinary() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"sdfChoice":{"a":{"type":"string","sdfType":"byte-string"},"b":{"type":"string"}}}}}"#;
    let cat = Catalogue::take_inputs(vec![
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""_x""#).sample(),
    ])
    .unwrap();
    let report = selected(&cat);
    assert_real_parity(&report);
    let sample = report.sample_result().unwrap();
    assert_eq!(sample.admission(), Admission::Admitted);
    assert_eq!(
        sample.consulted_interpretations().collect::<Vec<_>>(),
        [BYTE]
    );
    assert_eq!(
        sample.meaning().abstract_values().status(),
        AssessmentStatus::NotApplicable
    );
    assert_eq!(
        sample.meaning().const_values().status(),
        AssessmentStatus::NotApplicable
    );
    assert!(sample.meaning().const_values().basis().next().is_none());
    assert!(
        sample
            .meaning()
            .const_values()
            .limitations()
            .next()
            .is_none()
    );
    let support: Vec<_> = sample.meaning().admission().basis().map(key).collect();
    assert!(support.contains(&ORDINARY));
    assert!(!support.contains(&BYTE));
    assert!(!support.contains(&UNIX));
    assert_eq!(report.outcome().errors(), 0);
    assert!(report.outcome().complete());
}

#[test]
fn negative_fractional_integer_rejection_is_independent_of_unfinished_denotation() {
    let cat = Catalogue::take_inputs(vec![
        SourceInput::supplied(
            "m",
            br#"{"info":{},"sdfProperty":{"p":{"type":"integer","sdfType":"unix-time"}}}"#,
        ),
        SourceInput::supplied("s", b"-0.5").sample(),
    ])
    .unwrap();
    let report = selected(&cat);
    assert_real_parity(&report);
    let sample = report.sample_result().unwrap();
    assert_eq!(sample.admission(), Admission::Rejected);
    assert!(!report.outcome().complete());
    assert!(report.outcome().errors() > 0);
    assert_eq!(
        sample.consulted_interpretations().collect::<Vec<_>>(),
        [UNIX]
    );
    assert_eq!(
        sample.meaning().abstract_values().status(),
        AssessmentStatus::Undetermined
    );
    assert!(sample.meaning().abstract_values().basis().next().is_none());
    assert!(
        sample
            .meaning()
            .abstract_values()
            .limitations()
            .map(key)
            .any(|id| id == NEGATIVE)
    );
    assert!(
        sample
            .meaning()
            .admission()
            .basis()
            .map(key)
            .any(|id| id == NUMERIC)
    );
    assert!(sample.meaning().admission().limitations().next().is_none());
    let source = cat.source(sample.sample_id()).unwrap();
    assert_eq!(
        source
            .sample_value()
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "-0.5"
    );
}

#[test]
fn original_and_typed_newline_are_the_same_actual_decoded_value() {
    for typed in [false, true] {
        let cat = Catalogue::take_inputs(vec![
            SourceInput::supplied(
                "m",
                br#"{"info":{},"sdfProperty":{"p":{"type":"string","sdfType":"byte-string"}}}"#,
            ),
            if typed {
                SourceInput::typed("s", JsonBuilder::new().text("Zg\n").unwrap())
            } else {
                SourceInput::supplied("s", br#""Zg\n""#).sample()
            },
        ])
        .unwrap();
        let report = selected(&cat);
        assert_real_parity(&report);
        let sample = report.sample_result().unwrap();
        assert_eq!(
            sample
                .sample_value()
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap()
                .as_bytes(),
            b"Zg\n"
        );
        assert_eq!(sample.admission(), Admission::Rejected);
        let value = actual_wire(&report);
        if typed {
            assert!(value["result"]["sample_location"]["span"].is_null());
        } else {
            assert_eq!(
                value["result"]["sample_location"]["span"],
                serde_json::json!([0, 6])
            );
        }
    }
}

#[test]
fn ordinary_witness_history_tracks_actual_traversal_and_input_identity() {
    for ordinary_first in [false, true] {
        let choices = if ordinary_first {
            r#"{"a":{"type":"string"},"b":{"type":"string","sdfType":"byte-string"}}"#
        } else {
            r#"{"a":{"type":"string","sdfType":"byte-string"},"b":{"type":"string"}}"#
        };
        let model = format!(r#"{{"sdfProperty":{{"p":{{"sdfChoice":{choices}}}}}}}"#);
        for typed in [false, true] {
            let input = if typed {
                SourceInput::typed("s", JsonBuilder::new().text("_x").unwrap())
            } else {
                SourceInput::supplied("s", br#""_x""#).sample()
            };
            let cat =
                Catalogue::take_inputs(vec![SourceInput::supplied("m", model.as_bytes()), input])
                    .unwrap();
            let report = selected(&cat);
            assert_real_parity(&report);
            let sample = report.sample_result().unwrap();
            assert_eq!(sample.admission(), Admission::Admitted);
            assert_eq!(
                sample.consulted_interpretations().collect::<Vec<_>>(),
                if ordinary_first {
                    Vec::new()
                } else {
                    vec![BYTE]
                }
            );
            assert_eq!(
                sample.meaning().abstract_values().status(),
                AssessmentStatus::NotApplicable
            );
            assert_eq!(
                sample.meaning().const_values().status(),
                AssessmentStatus::NotApplicable
            );
            assert!(sample.meaning().admission().limitations().next().is_none());
            assert!(
                sample
                    .meaning()
                    .admission()
                    .basis()
                    .map(key)
                    .any(|id| id == ORDINARY)
            );
            assert_eq!(report.outcome().errors(), 0);
            assert!(report.outcome().complete());
            let wire = actual_wire(&report);
            assert_eq!(wire["result"]["sample_location"]["span"].is_null(), typed);
            let witness = wire["result"]["witnesses"]
                .as_array()
                .unwrap()
                .iter()
                .find(|witness| witness["kind"] == "admission")
                .unwrap();
            assert_eq!(
                witness["alternative"]["pointer"],
                format!(
                    "/sdfProperty/p/sdfChoice/{}",
                    if ordinary_first { "a" } else { "b" }
                )
            );
            assert_eq!(witness["sample"]["span"].is_null(), typed);
        }
    }
}

#[test]
fn typed_negative_fraction_keeps_exact_input_and_independent_numeric_rejection() {
    let cat = Catalogue::take_inputs(vec![
        SourceInput::supplied(
            "m",
            br#"{"sdfProperty":{"p":{"type":"integer","sdfType":"unix-time"}}}"#,
        ),
        SourceInput::typed("s", JsonBuilder::new().number("-0.5").unwrap()),
    ])
    .unwrap();
    let report = selected(&cat);
    assert_real_parity(&report);
    let sample = report.sample_result().unwrap();
    assert_eq!(sample.admission(), Admission::Rejected);
    assert!(!report.outcome().complete());
    assert!(report.outcome().errors() > 0);
    assert_eq!(
        sample
            .sample_value()
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "-0.5"
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
            .map(key)
            .any(|id| id == NEGATIVE)
    );
    assert!(
        sample
            .meaning()
            .admission()
            .basis()
            .map(key)
            .any(|id| id == NUMERIC)
    );
    assert!(sample.meaning().admission().limitations().next().is_none());
    assert!(actual_wire(&report)["result"]["sample_location"]["span"].is_null());
}

#[test]
fn ordinary_wrong_kind_citation_agrees_in_detail_and_mandatory_support() {
    for (kind, raw) in [("object", "1"), ("array", r#""x""#)] {
        let citation = if kind == "object" {
            "RFC 9880 Appendix C.5"
        } else {
            "RFC 9880 Appendix C.4"
        };
        let model = format!(r#"{{"info":{{}},"sdfProperty":{{"p":{{"type":"{kind}"}}}}}}"#);
        let cat = Catalogue::take_inputs(vec![
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_real_parity(&report);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        let value = actual_wire(&report);
        let condition = value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["domain"] == "sample" && item["code"] == "sample.type")
            .unwrap();
        assert_eq!(condition["basis"]["citation"], citation);
        let basis = wire_keys(&value["result"]["meaning"]["admission"]["basis"]);
        assert!(basis.contains(&citation));
        assert!(!basis.contains(&NUMERIC));
    }
}

#[test]
fn all_known_exclusions_and_unknown_potential_have_different_final_proofs() {
    for unknown in [false, true] {
        let mut choices = serde_json::json!({
            "a":{"type":"string","sdfType":"byte-string","const":"Zg"},
            "b":{"type":"string","sdfType":"byte-string","const":"Zm8"}});
        if unknown {
            choices.as_object_mut().unwrap().insert(
                "c".into(),
                serde_json::json!({"type":"string","sdfType":"vendor"}),
            );
        }
        let model =
            serde_json::json!({"info":{},"sdfProperty":{"p":{"sdfChoice":choices}}}).to_string();
        let cat = Catalogue::take_inputs(vec![
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""Zm9v""#).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_real_parity(&report);
        let sample = report.sample_result().unwrap();
        assert!(sample.consulted_interpretations().any(|id| id == BYTE));
        if unknown {
            assert_eq!(sample.admission(), Admission::Undetermined);
            assert!(sample.meaning().admission().basis().next().is_none());
            assert!(sample.meaning().admission().limitations().next().is_some());
            assert!(!report.outcome().complete());
            assert_eq!(report.outcome().errors(), 0);
        } else {
            assert_eq!(sample.admission(), Admission::Rejected);
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .basis()
                    .map(key)
                    .any(|id| id == ORDINARY)
            );
            assert!(
                sample
                    .meaning()
                    .admission()
                    .basis()
                    .map(key)
                    .any(|id| id == BYTE)
            );
            assert!(
                sample
                    .meaning()
                    .admission()
                    .basis()
                    .map(key)
                    .any(|id| id == "RFC 9880 Section 4.7.2")
            );
            assert!(report.outcome().errors() > 0);
        }
    }
}

pub(crate) fn assert_optional_zero(report: &Report<'_>) {
    assert_real_parity(report);
    let value = actual_wire(report);
    assert!(value["coverage"]["rows"].as_array().unwrap().is_empty());
    assert!(value["coverage"]["rows_omitted"].as_u64().unwrap() > 0);
    assert!(value["result"]["witnesses"].as_array().unwrap().is_empty());
    assert!(value["diagnostics"]["items"].as_array().unwrap().is_empty());
    assert!(value["result"]["meaning"].is_object());
    assert!(value["result"]["description"].is_object() || value["result"]["description"].is_null());
    if report.sample_result().unwrap().admission() == Admission::Rejected {
        assert!(report.outcome().errors() > 0);
        assert!(value["diagnostics"]["omitted"].as_u64().unwrap() > 0);
    }
    if !report.outcome().complete() {
        assert!(report.outcome().incomplete_checks() > 0);
    }
}

#[test]
fn original_mandatory_reserve_keeps_meaning_when_optional_detail_is_zero() {
    const SAMPLE: usize = 2 * (6 * 8192 + 2) + (6 * (8192 + 14) + 2) + 16384;
    const COMMON: usize = 6 * 2 + 4096 * 2 + 16384;
    for (kind, raw, expected) in [
        ("byte-string", r#""Zg""#, Admission::Admitted),
        ("byte-string", r#""_x""#, Admission::Rejected),
        ("unix-time", "-1", Admission::Undetermined),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{"sdfType":"{kind}"}}}}}}"#);
        let inputs = [
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ];
        assert!(
            Catalogue::with_limits(
                &inputs,
                esdieff::Limits::default().report_bytes(COMMON + SAMPLE - 1)
            )
            .is_err()
        );
        let cat = Catalogue::with_limits(
            &inputs,
            esdieff::Limits::default().report_bytes(COMMON + SAMPLE),
        )
        .unwrap();
        let report = selected(&cat);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert_optional_zero(&report);
    }
}

#[test]
fn existing_civil_and_resource_limitations_keep_their_precise_cited_triples() {
    for (format, text, id, citation) in [
        (
            "date-time",
            "2030-06-30T23:59:60Z",
            "temporal-knowledge",
            "RFC 3339 Section 5.7; RFC 9880 Appendix C.2",
        ),
        (
            "time",
            "23:59:60Z",
            "sample-establishment",
            "ADR 0018: Civil date and time sample conventions; RFC 9880 Appendix C.2; RFC 3339 Section 5.6",
        ),
        (
            "uri",
            "mailto:operator@example.test",
            "sample-establishment",
            "ADR 0019: URI, URI-reference and UUID sample conventions; RFC 9880 Appendix C.2",
        ),
    ] {
        let model =
            serde_json::json!({"sdfProperty":{"p":{"type":"string","format":format}}}).to_string();
        let raw = serde_json::to_string(text).unwrap();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_real_parity(&report);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        let limitations: Vec<_> = report
            .sample_result()
            .unwrap()
            .meaning()
            .abstract_values()
            .limitations()
            .collect();
        assert!(
            limitations
                .iter()
                .any(|basis| basis.kind() == BasisKind::Operational
                    && basis.id() == Some(id)
                    && basis.citation() == Some(citation)),
            "{format}: {:?}",
            limitations
                .iter()
                .map(|b| (b.id(), b.citation()))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn rfc_projection_omits_leap_edition_id_without_changing_original_metadata() {
    for (text, code, knowledge) in [
        (
            "2030-06-30T12:34:60Z",
            "sample.temporal_exceptional_slot",
            false,
        ),
        ("2024-06-30T23:59:60Z", "sample.temporal_leap_second", true),
    ] {
        let model = br#"{"sdfProperty":{"p":{"type":"string","format":"date-time"}}}"#;
        let raw = serde_json::to_string(text).unwrap();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_real_parity(&report);
        let value = actual_wire(&report);
        let references = value["result"]["meaning"]["abstract_values"]["basis"]
            .as_array()
            .unwrap();
        assert_eq!(
            references
                .iter()
                .filter(|reference| reference["citation"]
                    == "RFC 3339 Section 5.7; RFC 9880 Appendix C.2")
                .count(),
            1
        );
        assert!(
            references
                .iter()
                .filter(|reference| reference["kind"] == "rfc")
                .all(|reference| reference.get("id").is_none())
        );
        let finding = value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == code)
            .unwrap();
        if knowledge {
            assert_eq!(finding["basis"]["id"], "iers-bulletin-c72/2026-07-06");
            assert_eq!(
                value["baseline"]["temporal_knowledge"]["id"],
                finding["basis"]["id"]
            );
        }
    }
}
