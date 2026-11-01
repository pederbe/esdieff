//! Compound alternatives reuse selected-value admission and declared-value consistency.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, Report, SampleKind,
    SampleRequest, SourceInput,
};
use serde_json::{Value, json};

const ROOT: &[u8] = include_bytes!("../fixtures/combined-constraints/root.sdf.json");
const DONOR: &[u8] = include_bytes!("../fixtures/combined-constraints/donor.sdf.json");
fn selected<'a>(cat: &'a Catalogue) -> Report<'a> {
    cat.check_sample(
        SampleRequest::new(
            cat.source_id("model").unwrap(),
            SampleKind::Property,
            "/sdfProperty/p",
        )
        .sample(cat.source_id("sample").unwrap()),
    )
    .unwrap()
}
fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}
fn conditions<'a>(wire: &'a Value, code: &str) -> Vec<&'a Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["code"] == code)
        .collect()
}

#[test]
fn composed_compound_const_choices_and_exact_overrides_share_one_admission() {
    for (sample, admitted, codes) in [
        (
            include_bytes!("../fixtures/combined-constraints/admitted.json").as_slice(),
            true,
            vec![],
        ),
        (
            include_bytes!("../fixtures/combined-constraints/over-bound.json").as_slice(),
            false,
            vec!["sample.bound", "sample.const"],
        ),
        (
            include_bytes!("../fixtures/combined-constraints/under-bound.json").as_slice(),
            false,
            vec!["sample.bound", "sample.const"],
        ),
        (
            include_bytes!("../fixtures/combined-constraints/duplicate.json").as_slice(),
            false,
            vec!["sample.unique", "sample.const"],
        ),
        (
            include_bytes!("../fixtures/combined-constraints/missing.json").as_slice(),
            false,
            vec!["sample.required", "sample.const"],
        ),
    ] {
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("model", ROOT),
            SourceInput::supplied("donor", DONOR),
            SourceInput::supplied("sample", sample).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            if admitted {
                Admission::Admitted
            } else {
                Admission::Rejected
            },
            "{}",
            report.to_json()
        );
        assert!(report.outcome().complete(), "{}", report.to_json());
        let wire = wire(&report);
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        for code in codes {
            assert!(!conditions(&wire, code).is_empty(), "{code}: {wire}");
        }
        if let Some(bound) = conditions(&wire, "sample.bound").first() {
            assert_eq!(bound["primary"]["pointer"], "/level");
            let lower = bound["arguments"]["constraint"] == "minimum";
            let effective = if lower {
                "/sdfProperty/p/properties/level/minimum"
            } else {
                "/sdfProperty/p/properties/level/maximum"
            };
            let original = if lower {
                "/sdfData/Record/properties/level/minimum"
            } else {
                "/sdfProperty/p/properties/level/maximum"
            };
            assert!(
                bound["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["kind"] == "effective"
                        && location["pointer"] == effective)
            );
            assert!(
                bound["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["kind"] == "source"
                        && location["source_id"] == if lower { 2 } else { 1 }
                        && location["pointer"] == original)
            );
            let span = bound["primary"]["span"].as_array().unwrap();
            assert_eq!(
                &sample[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
                if lower {
                    b"9007199254740990"
                } else {
                    b"9007199254740994"
                }
            );
        }
        if let Some(duplicate) = conditions(&wire, "sample.unique").first() {
            assert_eq!(duplicate["primary"]["pointer"], "/payload/values/1");
            assert!(
                duplicate["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|location| location["kind"] == "sample"
                        && location["pointer"] == "/payload/values/0")
            );
        }
        if let Some(required) = conditions(&wire, "sample.required").first() {
            assert_eq!(required["primary"]["pointer"], "");
            assert!(required["related"].as_array().unwrap().iter().any(
                |location| location["kind"] == "source"
                    && location["source_id"] == 2
                    && location["pointer"] == "/sdfData/Record/required/1"
            ));
        }
        assert_eq!(
            cat.source(cat.source_id("sample").unwrap())
                .unwrap()
                .raw_bytes(),
            sample
        );
        assert!(
            report
                .to_human()
                .contains(if admitted { "admitted" } else { "rejected" })
        );
    }
}

fn nested(quality: Value) -> Value {
    json!({"type":"array","nullable":false,"items":{"sdfChoice":{"map":{"type":"object","required":["v"],"properties":{"v":{"sdfChoice":{"meaning":quality}}}}}}})
}

#[test]
fn every_supported_scalar_meaning_runs_in_nested_alternatives_and_model_values() {
    for (name, quality, valid, invalid, invalid_admission) in [
        (
            "date-time",
            json!({"type":"string","format":"date-time"}),
            json!("2024-02-29T12:30:00Z"),
            json!("2024-02-30T12:30:00Z"),
            Admission::Rejected,
        ),
        (
            "date",
            json!({"type":"string","format":"date"}),
            json!("2000-02-29"),
            json!("1900-02-29"),
            Admission::Rejected,
        ),
        (
            "time",
            json!({"type":"string","format":"time"}),
            json!("12:34:56Z"),
            json!("24:00:00Z"),
            Admission::Rejected,
        ),
        (
            "uri",
            json!({"type":"string","format":"uri"}),
            json!("https://example.com/x"),
            json!("http://[bad"),
            Admission::Rejected,
        ),
        (
            "uri-reference",
            json!({"type":"string","format":"uri-reference"}),
            json!("../path?x#y"),
            json!("%"),
            Admission::Rejected,
        ),
        (
            "uuid",
            json!({"type":"string","format":"uuid"}),
            json!("550e8400-e29b-41d4-a716-446655440000"),
            json!("x"),
            Admission::Rejected,
        ),
        (
            "pattern",
            json!({"type":"string","pattern":r"^\p{Letter}+$"}),
            json!("aé"),
            json!("12"),
            Admission::Rejected,
        ),
        (
            "byte-string",
            json!({"sdfType":"byte-string"}),
            json!("Zg"),
            json!("Zg=="),
            Admission::Rejected,
        ),
        (
            "unix-time",
            json!({"sdfType":"unix-time"}),
            json!(1.5),
            json!(-1),
            Admission::Undetermined,
        ),
    ] {
        let mut description = nested(quality);
        let valid = json!([{"v":valid}]);
        let invalid = json!([{"v":invalid}]);
        let model = json!({"info":{},"sdfProperty":{"p":description}}).to_string();
        for (sample, expected) in [(&valid, Admission::Admitted), (&invalid, invalid_admission)] {
            let raw = sample.to_string();
            let cat = Catalogue::from_inputs(&[
                SourceInput::supplied("model", model.as_bytes()),
                SourceInput::supplied("sample", raw.as_bytes()).sample(),
            ])
            .unwrap();
            let report = selected(&cat);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{name}: {}",
                report.to_json()
            );
            assert_eq!(
                report.outcome().complete(),
                expected != Admission::Undetermined,
                "{name}: {}",
                report.to_json()
            );
            assert_eq!(report.requested_coverage().total(), Some(9));
            if expected == Admission::Rejected {
                let wire = wire(&report);
                assert!(
                    wire["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|item| item["domain"] == "sample"
                            && item["primary"]["pointer"] == "/0/v"),
                    "{name}: {wire}"
                );
            }
        }
        description["const"] = valid.clone();
        description["default"] = valid.clone();
        let model = json!({"info":{},"sdfData":{"D":description}}).to_string();
        let cat =
            Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        assert!(report.outcome().complete(), "{name}: {}", report.to_json());
        assert_eq!(report.outcome().errors(), 0, "{name}");
        assert_eq!(report.outcome().warnings(), 0, "{name}");
        assert_eq!(report.requested_coverage().total(), Some(10));
        for area in ["const-consistency/1", "default-consistency/1"] {
            assert_eq!(
                report.assessment_area_status(area).present(),
                Some(esdieff::AssessmentStatus::Assessed),
                "{name}"
            );
        }
        description.as_object_mut().unwrap().remove("const");
        description["default"] = invalid;
        let model = json!({"info":{},"sdfData":{"D":description}}).to_string();
        let cat =
            Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0, "{name}");
        assert_eq!(
            report.outcome().warnings(),
            u32::from(invalid_admission == Admission::Rejected),
            "{name}: {}",
            report.to_json()
        );
        assert_eq!(
            report.outcome().complete(),
            invalid_admission != Admission::Undetermined,
            "{name}"
        );
        let wire = wire(&report);
        if let Some(advice) = conditions(&wire, "consistency.default_excluded").first() {
            assert_eq!(advice["severity"], "warning");
            assert_eq!(advice["basis"]["kind"], "advice");
            assert_eq!(advice["primary"]["pointer"], "/sdfData/D/default/0/v");
        }
    }
}

fn compound_choice_model() -> Value {
    json!({"info":{},"sdfProperty":{"p":{
        "type":"object","required":["payload","flag"],"properties":{
            "flag":{"type":"boolean","default":true},
            "payload":{
                "type":"object","nullable":false,"required":["must"],
                "properties":{"must":{"type":"string","const":"shared"}},
                "const":{"must":"shared"},"sdfChoice":{
                    "map":{
                        "required":["tag"],
                        "properties":{
                            "tag":{"type":"string","enum":["on","off"]},
                            "value":{"type":"number","minimum":1}
                        },
                        "const":{"tag":"on","value":2}
                    },
                    "array":{
                        "type":"array","uniqueItems":true,"items":{"sdfChoice":{
                            "number":{"type":"number","minimum":2},
                            "map":{"type":"object","required":["n"],
                                "properties":{"n":{"type":"integer","minimum":1}}}
                        }},
                        "const":[2,{"n":3}]
                    },
                    "null":{"nullable":true,"const":null}
                }
            }
        }
    }}})
}

fn assert_compound_samples(model: &Value, samples: &[(&str, Admission)]) {
    let model = serde_json::to_vec(model).unwrap();
    for (raw, expected) in samples {
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("model", &model),
            SourceInput::supplied("sample", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            *expected,
            "{raw}: {}",
            report.to_json()
        );
        assert!(report.outcome().complete(), "{raw}: {}", report.to_json());
        assert_eq!(
            cat.source(cat.source_id("sample").unwrap())
                .unwrap()
                .raw_bytes(),
            raw.as_bytes()
        );
        if raw.starts_with(r#"{"payload"#) {
            let wire = wire(&report);
            assert!(
                conditions(&wire, "sample.required")
                    .iter()
                    .any(|item| item["arguments"]["entry"] == "flag")
            );
        }
    }
}

#[test]
fn compatible_compound_branches_replace_whole_shared_qualities() {
    let mut model = compound_choice_model();
    model["sdfProperty"]["p"]["properties"]["payload"]["sdfChoice"]
        .as_object_mut()
        .unwrap()
        .remove("array");
    assert_compound_samples(
        &model,
        &[
            (
                r#"{"flag":true,"payload":{"tag":"on","value":2}}"#,
                Admission::Admitted,
            ),
            (r#"{"flag":true,"payload":null}"#, Admission::Admitted),
            (r#"{"payload":{"tag":"on","value":2}}"#, Admission::Rejected),
            (
                r#"{"flag":true,"payload":{"tag":"off","value":2}}"#,
                Admission::Rejected,
            ),
        ],
    );
}

#[test]
fn map_array_and_null_choices_share_neutral_qualities() {
    let model = neutral_compound_choice_model();
    assert_compound_samples(
        &model,
        &[
            (
                r#"{"flag":true,"payload":{"tag":"on","value":2}}"#,
                Admission::Admitted,
            ),
            (
                r#"{"flag":true,"payload":[2,{"n":3}]}"#,
                Admission::Admitted,
            ),
            (r#"{"flag":true,"payload":null}"#, Admission::Admitted),
            (r#"{"payload":{"tag":"on","value":2}}"#, Admission::Rejected),
            (
                r#"{"flag":true,"payload":{"tag":"off","value":2}}"#,
                Admission::Rejected,
            ),
            (
                r#"{"flag":true,"payload":[1,{"n":3}]}"#,
                Admission::Rejected,
            ),
        ],
    );
}

fn neutral_compound_choice_model() -> Value {
    let mut model = compound_choice_model();
    let payload = model["sdfProperty"]["p"]["properties"]["payload"]
        .as_object_mut()
        .unwrap();
    for quality in ["type", "required", "properties", "const"] {
        payload.remove(quality);
    }
    payload.get_mut("sdfChoice").unwrap()["map"]["type"] = json!("object");
    model
}

#[test]
fn compound_const_and_default_candidates_reuse_map_array_and_null_witnesses() {
    for (candidate, excluded) in [
        (json!({"flag":true,"payload":{"tag":"on","value":2}}), false),
        (json!({"flag":true,"payload":[2,{"n":3}]}), false),
        (json!({"flag":true,"payload":null}), false),
        (json!({"payload":{"tag":"on","value":2}}), true),
        (json!({"flag":true,"payload":{"tag":"off","value":2}}), true),
        (json!({"flag":true,"payload":[1,{"n":3}]}), true),
    ] {
        let mut model = neutral_compound_choice_model();
        model["sdfProperty"]["p"]["const"] = candidate.clone();
        model["sdfProperty"]["p"]["default"] = candidate;
        let raw = serde_json::to_vec(&model).unwrap();
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("model", &raw)]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        assert!(report.outcome().complete(), "{}", report.to_json());
        assert_eq!(report.outcome().errors(), 0, "{}", report.to_json());
        assert_eq!(
            report.outcome().warnings(),
            if excluded { 2 } else { 0 },
            "{}",
            report.to_json()
        );
        let wire = wire(&report);
        for code in ["consistency.const_excluded", "consistency.default_excluded"] {
            assert_eq!(conditions(&wire, code).len(), usize::from(excluded));
        }
        assert_eq!(
            cat.source(cat.source_id("model").unwrap())
                .unwrap()
                .raw_bytes(),
            raw
        );
    }
}

#[test]
fn incompatible_shared_structure_keeps_compound_equality_undetermined() {
    let model = serde_json::to_vec(&compound_choice_model()).unwrap();
    let sample = br#"{"flag":true,"payload":{"tag":"on","value":2}}"#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("model", &model),
        SourceInput::supplied("sample", sample).sample(),
    ])
    .unwrap();
    let report = selected(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    for area in ["sample-const/1", "sample-alternatives/1"] {
        assert_eq!(
            report.assessment_area_status(area).require().unwrap(),
            AssessmentStatus::Undetermined,
            "{}",
            report.to_json()
        );
    }
}

#[test]
fn unresolved_nested_compound_branch_blocks_rejection_but_not_independent_failure() {
    let model=br##"{"info":{},"sdfProperty":{"p":{"type":"object","required":["fixed","choice"],"properties":{"fixed":{"type":"integer","minimum":1},"choice":{"sdfChoice":{"known":{"type":"object","required":["v"],"properties":{"v":{"type":"integer","minimum":1}}},"unknown":{"sdfRef":"#/sdfData/Absent"}}}}}}}"##;
    for (raw, expected, errors) in [
        (r#"{"fixed":1,"choice":{"v":1}}"#, Admission::Admitted, 0),
        (
            r#"{"fixed":1,"choice":{"v":0}}"#,
            Admission::Undetermined,
            0,
        ),
        (r#"{"fixed":0,"choice":{"v":0}}"#, Admission::Rejected, 1),
    ] {
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model),
            SourceInput::supplied("sample", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{raw}: {}",
            report.to_json()
        );
        assert_eq!(report.outcome().errors(), errors, "{raw}");
        let wire = wire(&report);
        assert!(
            !conditions(&wire, "sample.bound")
                .iter()
                .any(|item| item["primary"]["pointer"] == "/choice/v")
        );
        if errors > 0 {
            assert_eq!(
                conditions(&wire, "sample.bound")[0]["primary"]["pointer"],
                "/fixed"
            );
        }
        if expected != Admission::Admitted {
            assert!(!report.outcome().complete());
        }
    }
}

#[test]
fn copied_direct_item_pattern_meaning_requires_an_explicit_full_override() {
    let donor=br#"{"info":{},"namespace":{"own":"https://example.com/direct"},"defaultNamespace":"own","sdfData":{"Rows":{"type":"array","items":{"type":"string","pattern":"^x$","sdfChoice":{"branch":{}}}}}}"#;
    for (override_quality, expected, complete) in [
        ("", Admission::Undetermined, false),
        (
            r#", "items":{"sdfChoice":{"branch":{"pattern":"^x$"}}}"#,
            Admission::Admitted,
            true,
        ),
        (r#", "maxItems":0"#, Admission::Rejected, false),
    ] {
        let model = format!(
            r##"{{"info":{{}},"namespace":{{"d":"https://example.com/direct"}},"sdfProperty":{{"p":{{"sdfRef":"d:#/sdfData/Rows"{override_quality}}}}}}}"##
        );
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("donor", donor),
            SourceInput::supplied("sample", br#"["x"]"#).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{override_quality}: {}",
            report.to_json()
        );
        assert_eq!(
            report.outcome().complete(),
            complete,
            "{override_quality}: {}",
            report.to_json()
        );
        if expected == Admission::Rejected {
            assert!(!conditions(&wire(&report), "sample.count").is_empty());
        }
    }
}

#[test]
fn unsupported_nested_meanings_and_original_work_limits_stay_scoped() {
    for quality in [
        json!({"type":"string","sdfType":"byte-string","minLength":1}),
        json!({"type":"string","pattern":r"(?<x>a)|(?<x>b)"}),
    ] {
        let description = json!({"type":"object","required":["fixed","values"],"properties":{"fixed":{"type":"integer","minimum":1},"values":nested(quality)}});
        let model = json!({"info":{},"sdfProperty":{"p":description}}).to_string();
        for (fixed, expected) in [(1, Admission::Undetermined), (0, Admission::Rejected)] {
            let raw = json!({"fixed":fixed,"values":[{"v":"Zg"}]}).to_string();
            let cat = Catalogue::from_inputs(&[
                SourceInput::supplied("model", model.as_bytes()),
                SourceInput::supplied("sample", raw.as_bytes()).sample(),
            ])
            .unwrap();
            let report = selected(&cat);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{}",
                report.to_json()
            );
            assert!(!report.outcome().complete());
            assert_eq!(report.outcome().errors(), u32::from(fixed == 0));
        }
    }
    for limits in [
        Limits::default().equality_pairs(1),
        Limits::default().pattern_bytes(1),
    ] {
        for (raw, expected) in [
            (
                include_bytes!("../fixtures/combined-constraints/admitted.json").as_slice(),
                Admission::Undetermined,
            ),
            (
                include_bytes!("../fixtures/combined-constraints/over-bound.json").as_slice(),
                Admission::Rejected,
            ),
        ] {
            let cat = Catalogue::with_limits(
                &[
                    SourceInput::supplied("model", ROOT),
                    SourceInput::supplied("donor", DONOR),
                    SourceInput::supplied("sample", raw).sample(),
                ],
                limits.clone(),
            )
            .unwrap();
            let report = selected(&cat);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{}",
                report.to_json()
            );
            assert!(!report.outcome().complete());
            if expected == Admission::Rejected {
                assert!(!conditions(&wire(&report), "sample.bound").is_empty());
            }
        }
    }
}

#[test]
fn undeclared_nested_extension_vocabulary_is_not_unsupported_meaning() {
    // Following ADR 0037: Undeclared extension vocabulary.
    let quality = json!({"type":"string","vendor:meaning":true});
    let description = json!({"type":"object","required":["fixed","values"],"properties":{"fixed":{"type":"integer","minimum":1},"values":nested(quality)}});
    let model = json!({"info":{},"sdfProperty":{"p":description}}).to_string();
    for (fixed, expected) in [(1, Admission::Admitted), (0, Admission::Rejected)] {
        let raw = json!({"fixed":fixed,"values":[{"v":"Zg"}]}).to_string();
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("sample", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().errors(), u32::from(fixed == 0));
    }
}
