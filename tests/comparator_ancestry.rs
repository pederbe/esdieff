//! Decisive compound proof keeps active structure and discards completed siblings.
mod location_support;
use esdieff::{
    Admission, Assessment, AssessmentStatus, BasisKind, Catalogue, CheckRequest, JsonBuilder,
    JsonValue, Report, SampleBasisRef, SampleKind, SampleRequest, SourceInput,
};
use serde_json::{Value, json};

const BYTE: &str = "byte-string-json/1";
const UNIX: &str = "unix-time-posix-coordinate/1";
const CONST: &str = "RFC 9880 Appendix C";
const MAP: &str = "RFC 9880 Appendix C.5";
const ARRAY: &str = "RFC 9880 Appendix C.4";
const NUMBER: &str = "RFC 9880 Appendix C.1";
const BOOLEAN: &str = "RFC 9880 Appendix C.3";
const KIND: &str = "RFC 9880 Section 4.7 and Appendix C";
const UUID: &str = "uuid-opaque-value/1";
fn typed(builder: &JsonBuilder, value: &Value) -> JsonValue {
    match value {
        Value::Null => builder.null().unwrap(),
        Value::Bool(v) => builder.boolean(*v).unwrap(),
        Value::Number(v) => builder.number(&v.to_string()).unwrap(),
        Value::String(v) => builder.text(v).unwrap(),
        Value::Array(v) => builder
            .array(v.iter().map(|v| typed(builder, v)).collect())
            .unwrap(),
        Value::Object(v) => builder
            .map(
                v.iter()
                    .map(|(key, v)| (key.as_str(), typed(builder, v)))
                    .collect(),
            )
            .unwrap(),
    }
}
fn catalogue(description: Value, value: &Value, use_typed: bool) -> Catalogue {
    let model = json!({"info":{},"sdfProperty":{"p":description}}).to_string();
    let raw = value.to_string();
    let input = if use_typed {
        SourceInput::typed("s", typed(&JsonBuilder::new(), value))
    } else {
        SourceInput::supplied("s", raw.as_bytes()).sample()
    };
    Catalogue::take_inputs(vec![SourceInput::supplied("m", model.as_bytes()), input]).unwrap()
}
fn report(cat: &Catalogue) -> Report<'_> {
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
fn key(reference: SampleBasisRef) -> &'static str {
    reference.id().or(reference.citation()).unwrap()
}
fn assert_set(references: impl Iterator<Item = SampleBasisRef>, expected: &[&str]) {
    let mut actual: Vec<_> = references.map(key).collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}
fn assert_views(report: &Report<'_>, use_typed: bool) {
    let sample = report.sample_result().unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let human = report.to_human();
    for (name, label, references) in [
        (
            "abstract_values",
            "Sample abstract values",
            sample
                .meaning()
                .abstract_values()
                .basis()
                .collect::<Vec<_>>(),
        ),
        (
            "const",
            "Sample const",
            sample.meaning().const_values().basis().collect::<Vec<_>>(),
        ),
        (
            "admission",
            "Sample admission",
            sample.meaning().admission().basis().collect::<Vec<_>>(),
        ),
    ] {
        let values = wire["result"]["meaning"][name]["basis"].as_array().unwrap();
        assert_eq!(values.len(), references.len());
        let mut printed = Vec::new();
        for (reference, value) in references.iter().zip(values) {
            assert_eq!(value["id"].as_str(), reference.id());
            assert_eq!(value["citation"].as_str(), reference.citation());
            assert_eq!(
                value["kind"],
                match reference.kind() {
                    BasisKind::Rfc => "rfc",
                    BasisKind::Interpretation => "interpretation",
                    BasisKind::Operational => "operational",
                    _ => panic!("unexpected basis kind"),
                }
            );
            printed.push(match (reference.id(), reference.citation()) {
                (Some(id), Some(cite)) => format!("{id} ({cite})"),
                (Some(id), None) => id.to_owned(),
                (None, Some(cite)) => cite.to_owned(),
                _ => panic!("reference identity"),
            });
        }
        assert!(human.lines().any(|line| line
            == format!(
                "{label} basis: {}",
                if printed.is_empty() {
                    "none".to_owned()
                } else {
                    printed.join("; ")
                }
            )));
    }
    assert_eq!(
        wire["result"]["sample_location"]["span"].is_null(),
        use_typed
    );
    for finding in wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["code"] == "sample.const")
    {
        let witness = wire["result"]["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["constraint"] == "const")
            .unwrap();
        assert_eq!(finding["basis"], witness["basis"]);
        assert_eq!(finding["basis"], json!({"kind":"rfc","citation":CONST}));
        assert_eq!(finding["primary"], witness["sample"]);
        assert!(
            finding["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row == &witness["effective"])
        );
        assert_eq!(finding["primary"]["span"].is_null(), use_typed);
    }
}
#[test]
fn const_rejections_keep_rfc_findings_and_specialized_coverage() {
    for (convention, constant, value, interpretation) in [
        ("byte-string", json!("Zg"), json!("Zw"), BYTE),
        ("unix-time", json!(10), json!(11), UNIX),
    ] {
        for nested in [false, true] {
            for use_typed in [false, true] {
                let quality = json!({"sdfType":convention});
                let (description, value, pointer, support) = if nested {
                    (
                        json!({"type":"object","properties":{"v":{"type":"array","items":{"type":"object","properties":{"n":quality}}}},"const":{"v":[{"n":constant}]}}),
                        json!({"v":[{"n":value}]}),
                        "/v/0/n",
                        vec![CONST, MAP, ARRAY, interpretation],
                    )
                } else {
                    let mut description = quality;
                    description["const"] = constant.clone();
                    (description, value.clone(), "", vec![CONST, interpretation])
                };
                let raw = value.to_string();
                let cat = catalogue(description, &value, use_typed);
                let report = report(&cat);
                let sample = report.sample_result().unwrap();
                assert_eq!(
                    sample.admission(),
                    Admission::Rejected,
                    "{convention} nested={nested} typed={use_typed}: {}",
                    report.to_json()
                );
                assert!(report.outcome().complete());
                assert_eq!(
                    sample.meaning().const_values().status(),
                    AssessmentStatus::Assessed
                );
                assert_set(
                    sample.meaning().abstract_values().basis(),
                    &[interpretation],
                );
                assert_set(sample.meaning().const_values().basis(), &support);
                assert_set(sample.meaning().admission().basis(), &support);
                assert_eq!(
                    sample.meaning().abstract_values().status(),
                    AssessmentStatus::Assessed
                );
                assert_eq!(sample.meaning().abstract_values().limitations().len(), 0);
                assert_eq!(sample.meaning().const_values().limitations().len(), 0);
                assert_eq!(sample.meaning().admission().limitations().len(), 0);
                assert_eq!(
                    sample.consulted_interpretations().collect::<Vec<_>>(),
                    [interpretation]
                );
                let diagnostics = report.diagnostics();
                let finding = diagnostics
                    .conditions()
                    .find(|row| row.code() == "sample.const")
                    .unwrap();
                assert_eq!(finding.basis().kind(), BasisKind::Rfc);
                assert_eq!(finding.basis().citation(), Some(CONST));
                assert_eq!(finding.basis().id(), None);
                let location = location_support::primary_sample(finding);
                assert_eq!(location.source_id(), sample.sample_id());
                assert_eq!(location.pointer().require().unwrap(), pointer);
                let expected_span = if use_typed {
                    None
                } else {
                    let leaf = if nested { &value["v"][0]["n"] } else { &value };
                    let leaf = leaf.to_string();
                    let start = raw.find(&leaf).unwrap() as u32;
                    Some(start..start + leaf.len() as u32)
                };
                assert_eq!(location.span(), expected_span);
                let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
                let witness = wire["result"]["witnesses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["kind"] == "rejection" && row["constraint"] == "const")
                    .unwrap();
                assert_eq!(witness["basis"], json!({"kind":"rfc","citation":CONST}));
                assert_eq!(witness["sample"]["pointer"], pointer);
                assert_eq!(
                    witness["effective"]["pointer"],
                    format!("/sdfProperty/p/const{pointer}")
                );
                assert_eq!(witness["sample"]["span"].is_null(), use_typed);
                let coverage = wire["coverage"]["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["id"] == "sample-const/1")
                    .unwrap();
                assert_eq!(coverage["status"], "assessed");
                assert_eq!(
                    coverage["basis"],
                    json!({"kind":"interpretation","id":interpretation,"citation":"RFC 9880 Section 4.7.1 Table 5"})
                );
                let consulted: Vec<_> = wire["baseline"]["interpretations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|id| [BYTE, UNIX].contains(id))
                    .collect();
                assert_eq!(consulted, [interpretation]);
                assert!(
                    report.to_human().lines().any(|line| line
                        == format!("Consulted sample interpretations: {interpretation}"))
                );
                assert_views(&report, use_typed);
            }
        }
    }
}
#[test]
fn every_decisive_exit_replaces_unrelated_prefix_support() {
    let uuid_a = "f81d4fae-7dec-11d0-a765-00a0c91e6bf6";
    let uuid_b = "00000000-0000-0000-0000-000000000000";
    let cases = [
        (
            "abstract",
            json!({"sdfType":"unix-time"}),
            json!(-1),
            json!(0),
            json!({"sdfType":"byte-string"}),
            json!("Zg"),
            json!("Zw"),
            vec![CONST, MAP, BYTE],
        ),
        (
            "resource",
            json!({"sdfType":"byte-string"}),
            json!("_x"),
            json!("Zg"),
            json!({"type":"string","format":"uuid"}),
            json!(uuid_a),
            json!(uuid_b),
            vec![CONST, MAP, UUID],
        ),
        (
            "kind",
            json!({"sdfType":"byte-string"}),
            json!("_x"),
            json!("Zg"),
            json!({}),
            json!(1),
            json!("text"),
            vec![CONST, MAP, KIND],
        ),
        (
            "map_count",
            json!({"sdfType":"byte-string"}),
            json!("_x"),
            json!("Zg"),
            json!({"type":"object"}),
            json!({}),
            json!({"n":1}),
            vec![CONST, MAP],
        ),
        (
            "primitive",
            json!({"sdfType":"byte-string"}),
            json!("_x"),
            json!("Zg"),
            json!({"type":"number"}),
            json!(1),
            json!(2),
            vec![CONST, MAP, NUMBER],
        ),
        (
            "array_length",
            json!({"sdfType":"byte-string"}),
            json!("_x"),
            json!("Zg"),
            json!({"type":"array","items":{"type":"number"}}),
            json!([1]),
            json!([1, 2]),
            vec![CONST, MAP, ARRAY],
        ),
        (
            "map_key",
            json!({"sdfType":"byte-string"}),
            json!("_x"),
            json!("Zg"),
            json!({"type":"object"}),
            json!({"a":1}),
            json!({"b":1}),
            vec![CONST, MAP],
        ),
    ];
    for (name, prefix, prefix_const, prefix_sample, decisive, constant, value, expected) in cases {
        for use_typed in [false, true] {
            let description = json!({"type":"object","properties":{"a":prefix,"z":decisive},"const":{"a":prefix_const,"z":constant}});
            let cat = catalogue(
                description,
                &json!({"a":prefix_sample,"z":value}),
                use_typed,
            );
            let report = report(&cat);
            let sample = report.sample_result().unwrap();
            assert_eq!(sample.admission(), Admission::Rejected, "{name}");
            assert_eq!(
                sample.meaning().const_values().status(),
                AssessmentStatus::Assessed,
                "{name}"
            );
            assert!(
                sample
                    .meaning()
                    .const_values()
                    .limitations()
                    .next()
                    .is_none()
            );
            assert_set(sample.meaning().const_values().basis(), &expected);
            assert_set(sample.meaning().admission().basis(), &expected);
            assert!(
                sample
                    .meaning()
                    .abstract_values()
                    .basis()
                    .any(|r| key(r) == BYTE)
            );
            assert!(sample.consulted_interpretations().any(|id| id == BYTE));
            if name == "abstract" {
                assert!(sample.consulted_interpretations().any(|id| id == UNIX));
            }
            assert_views(&report, use_typed);
        }
    }
}
#[test]
fn active_nested_containers_survive_but_completed_sibling_containers_do_not() {
    for use_typed in [false, true] {
        let cat = catalogue(
            json!({"type":"object","properties":{"a":{"sdfType":"byte-string"},"z":{"type":"array","items":{"type":"object","properties":{"n":{"type":"number"}}}}},"const":{"a":"_x","z":[{"n":1}]}}),
            &json!({"a":"Zg","z":[{"n":2}]}),
            use_typed,
        );
        let result = report(&cat);
        let sample = result.sample_result().unwrap();
        assert_eq!(sample.admission(), Admission::Rejected);
        assert_set(
            sample.meaning().const_values().basis(),
            &[NUMBER, CONST, MAP, ARRAY],
        );
        assert_set(
            sample.meaning().admission().basis(),
            &[NUMBER, CONST, MAP, ARRAY],
        );
        assert_views(&result, use_typed);
        let cat = catalogue(
            json!({"type":"object","properties":{"a":{"type":"array","items":{"type":"boolean"}},"z":{"type":"object","properties":{"n":{"type":"number"}}}},"const":{"a":[true],"z":{"n":1}}}),
            &json!({"a":[true],"z":{"n":2}}),
            use_typed,
        );
        let result = report(&cat);
        let sample = result.sample_result().unwrap();
        assert_eq!(sample.admission(), Admission::Rejected);
        assert_set(
            sample.meaning().const_values().basis(),
            &[NUMBER, CONST, MAP],
        );
        assert!(
            !sample
                .meaning()
                .const_values()
                .basis()
                .any(|r| [ARRAY, BOOLEAN].contains(&key(r)))
        );
        assert_views(&result, use_typed);
    }
}
#[test]
fn equal_and_unavailable_pairs_keep_their_complete_or_partial_aggregate() {
    for unavailable in [false, true] {
        for use_typed in [false, true] {
            let cat = catalogue(
                json!({"type":"object","properties":{"a":{"sdfType":"byte-string"},"z":{"type":"array","items":{"type":"number"}}},"const":{"a":if unavailable{"_x"}else{"Zg"},"z":[1]}}),
                &json!({"a":"Zg","z":[1]}),
                use_typed,
            );
            let result = report(&cat);
            let sample = result.sample_result().unwrap();
            assert_eq!(
                sample.admission(),
                if unavailable {
                    Admission::Undetermined
                } else {
                    Admission::Admitted
                }
            );
            let mut expected = vec![BYTE, NUMBER, MAP, ARRAY];
            if !unavailable {
                expected.push(CONST);
            }
            assert_set(sample.meaning().const_values().basis(), &expected);
            if unavailable {
                assert!(sample.meaning().admission().basis().next().is_none());
                assert!(
                    sample
                        .meaning()
                        .const_values()
                        .limitations()
                        .any(|r| r.id() == Some("sample-abstract-denotation-unavailable"))
                );
            } else {
                assert!(
                    sample
                        .meaning()
                        .const_values()
                        .limitations()
                        .next()
                        .is_none()
                );
            }
            assert_views(&result, use_typed);
        }
    }
}
#[test]
fn repeated_pairs_and_later_whole_witnesses_reset_only_tentative_proof() {
    for use_typed in [false, true] {
        let cat = catalogue(
            json!({"type":"array","uniqueItems":true,"items":{"type":"object","properties":{"a":{"sdfType":"byte-string"},"z":{"type":"number"}}}}),
            &json!([{"a":"Zg","z":1},{"a":"Zg","z":2},{"a":"Zg","z":2}]),
            use_typed,
        );
        let result = report(&cat);
        let sample = result.sample_result().unwrap();
        assert_eq!(sample.admission(), Admission::Rejected);
        assert_set(
            sample.meaning().admission().basis(),
            &[BYTE, NUMBER, MAP, ARRAY],
        );
        assert_views(&result, use_typed);
        let cat = catalogue(
            json!({"sdfChoice":{"a":{"type":"object","properties":{"a":{"sdfType":"byte-string"},"z":{"type":"number"}},"const":{"a":"_x","z":1}},"b":{"type":"object","properties":{"a":{"sdfType":"byte-string"},"z":{"type":"number"}},"const":{"a":"Zg","z":2}}}}),
            &json!({"a":"Zg","z":2}),
            use_typed,
        );
        let result = report(&cat);
        let sample = result.sample_result().unwrap();
        assert_eq!(sample.admission(), Admission::Admitted);
        assert_set(
            sample.meaning().const_values().basis(),
            &[BYTE, NUMBER, CONST, MAP],
        );
        assert!(
            sample
                .meaning()
                .const_values()
                .limitations()
                .next()
                .is_none()
        );
        assert!(sample.meaning().admission().limitations().next().is_none());
        assert!(sample.consulted_interpretations().any(|id| id == BYTE));
        assert_views(&result, use_typed);
    }
}
#[test]
fn model_default_consistency_uses_the_same_decisive_compound_comparator() {
    let model=json!({"info":{},"sdfData":{"v":{"type":"object","properties":{"a":{"sdfType":"byte-string"},"z":{"type":"number"}},"const":{"a":"_w","z":1},"default":{"a":"_w","z":2}}}}).to_string();
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("m", model.as_bytes())]).unwrap();
    let result = cat
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert_eq!(
        result
            .assessment_area_status("default-consistency/1")
            .require()
            .unwrap(),
        AssessmentStatus::Assessed
    );
    let wire: Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["version"], "1.0");
    assert!(wire["result"].get("meaning").is_none());
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "consistency.default_excluded")
    );
    assert!(
        wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == BYTE)
    );
}
