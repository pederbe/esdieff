//! `Catalogue::generate` builds Generated values for selected data descriptions and settles
//! each one with Sample-value checking. Follows ADR 0041: Generated values settled by
//! Sample-value checking.
use esdieff::{
    Admission, Catalogue, Disposition, GenerationEvidence, GenerationRequest, GenerationState,
    Limits, SampleKind, SourceInput,
};

/// Each value generated for the one selected description, as target, JSON text and admission.
fn generated(model: &str, kind: SampleKind, pointer: &str) -> Vec<(String, String, Admission)> {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let document = catalogue.source_id("model").unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(document).select(kind, pointer))
        .unwrap();
    let mut descriptions = result.descriptions();
    assert_eq!(descriptions.len(), 1);
    let description = descriptions.next().unwrap();
    description
        .values()
        .map(|value| {
            (
                value.target().to_owned(),
                value.to_json().to_owned(),
                value.admission(),
            )
        })
        .collect()
}

fn property(description: &str) -> Vec<(String, String, Admission)> {
    generated(
        &format!(r#"{{"sdfProperty":{{"level":{description}}}}}"#),
        SampleKind::Property,
        "/sdfProperty/level",
    )
}

fn has(values: &[(String, String, Admission)], target: &str, value: &str, admission: Admission) {
    assert!(
        values
            .iter()
            .any(|row| row.0 == target && row.1 == value && row.2 == admission),
        "{target} {value} {admission:?} in {values:?}"
    );
}

#[test]
fn a_minimum_gives_an_admitted_bound_and_a_rejected_value_below_it() {
    let values = property(r#"{"type":"integer","minimum":10}"#);
    has(&values, "minimum", "10", Admission::Admitted);
    has(&values, "below_minimum", "9", Admission::Rejected);
}

#[test]
fn corpus_evidence_distinguishes_admitted_boundaries_from_rejected_candidates() {
    let model = br#"{"sdfProperty":{
        "level":{"type":"integer","nullable":false,"minimum":10,"maximum":20},
        "identifier":{"type":"string","nullable":false,"pattern":"^(foo|bar)$","minLength":3,"maxLength":3},
        "settings":{"type":"object","nullable":false,"required":["mode"],"properties":{"mode":{"const":"on"}}}
    }}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let result = whole(&catalogue).unwrap();
    let descriptions: Vec<_> = result.descriptions().collect();
    // Generation can finish without constructing even one admitted identifier.
    assert!(result.complete());
    let level = descriptions[0];
    assert_eq!(
        level.count_evidence(GenerationEvidence::AdmittedBoundary),
        2
    );
    assert_eq!(level.count_evidence(GenerationEvidence::RejectedOutside), 6);
    assert_eq!(
        level.count_evidence(GenerationEvidence::RejectedCandidate),
        1
    );
    assert_eq!(level.count_evidence(GenerationEvidence::AdmittedExample), 0);
    // The representative 10 yields to the minimum target during deduplication.
    let minimum = level
        .values()
        .find(|value| value.to_json() == "10")
        .unwrap();
    assert_eq!(minimum.target(), "minimum");
    assert_eq!(minimum.evidence(), GenerationEvidence::AdmittedBoundary);

    let identifier = descriptions[1];
    assert_eq!(
        identifier.count_evidence(GenerationEvidence::AdmittedBoundary),
        0
    );
    assert_eq!(
        identifier.count_evidence(GenerationEvidence::AdmittedExample),
        0
    );
    let boundary = identifier
        .values()
        .find(|value| value.target() == "min_length")
        .unwrap();
    assert_eq!(boundary.to_json(), r#""aaa""#);
    assert_eq!(boundary.evidence(), GenerationEvidence::RejectedCandidate);
    assert_eq!(boundary.codes().collect::<Vec<_>>(), ["sample.pattern"]);

    let missing = descriptions[2]
        .values()
        .find(|value| value.target() == "missing_entry")
        .unwrap();
    assert_eq!(missing.to_json(), "{}");
    assert_eq!(missing.evidence(), GenerationEvidence::RejectedOutside);
    assert!(missing.codes().any(|code| code == "sample.required"));
}

#[test]
fn evidence_counts_values_without_claiming_choice_branch_coverage() {
    let model = br#"{"sdfProperty":{"mode":{"nullable":false,"sdfChoice":{
        "first":{"const":1},"same":{"const":1},"other":{"const":9}
    }}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let result = whole(&catalogue).unwrap();
    let mode = result.descriptions().next().unwrap();
    assert_eq!(mode.count_evidence(GenerationEvidence::AdmittedExample), 2);
    assert_eq!(
        mode.values()
            .filter(|value| value.target() == "alternative")
            .count(),
        2
    );
    assert!(
        result
            .to_human()
            .contains("not constraints or individual Choice alternatives")
    );
}

#[test]
fn nested_evidence_uses_the_construction_aim_and_keeps_unsettled_values_distinct() {
    let model = br#"{"sdfProperty":{
        "nested":{"type":"object","required":["items.minimum"],"properties":{
            "items.minimum":{"type":"integer","minimum":10}}},
        "unknown":{"type":"string","nullable":false,"pattern":"(?i)a"}
    }}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let result = whole(&catalogue).unwrap();
    let mut descriptions = result.descriptions();
    let nested = descriptions.next().unwrap();
    let minimum = nested
        .values()
        .find(|value| value.target() == "items.minimum.minimum")
        .unwrap();
    assert_eq!(minimum.evidence(), GenerationEvidence::AdmittedBoundary);
    let below = nested
        .values()
        .find(|value| value.target() == "items.minimum.below_minimum")
        .unwrap();
    assert_eq!(below.evidence(), GenerationEvidence::RejectedOutside);
    let unknown = descriptions.next().unwrap();
    assert_eq!(unknown.state(), GenerationState::Partial);
    assert!(unknown.count_evidence(GenerationEvidence::Undetermined) > 0);
    assert_eq!(
        unknown.count_evidence(GenerationEvidence::AdmittedExample),
        0
    );
}

#[test]
fn a_pattern_and_length_limits_give_an_admitted_identifier() {
    let values = property(
        r#"{"type":"string","nullable":false,"pattern":"^ID-[0-9]+$","minLength":5,"maxLength":7}"#,
    );
    has(&values, "type", r#""ID-00""#, Admission::Admitted);
}

#[test]
fn bounded_pattern_candidates_respect_repetition_classes_and_anchors() {
    for (pattern, minimum, maximum, expected) in [
        (r"^SN-[A-Z]{2}[0-9]{2}$", 0, 9, "SN-AA00"),
        (r"^x[ba]{1,3}z$", 5, 5, "xaaaz"),
        (r"^x[0-9]{2,}z$", 6, 6, "x0000z"),
        (r"^a?b*c+$", 3, 3, "abc"),
        (r"^x\d+\w?\s$", 4, 6, "x00 "),
        (r"^A\.B$", 0, 4, "A.B"),
        (r"^x[a\-]{2}$", 0, 4, "x--"),
        (r"^x[0-9]+?$", 4, 4, "x000"),
        (r"ID$", 4, 4, "aaID"),
        (r"^ID", 4, 4, "IDaa"),
        (r"ID", 4, 4, "IDaa"),
        (r"^$", 0, 0, ""),
    ] {
        let description = serde_json::json!({
            "type": "string", "nullable": false, "pattern": pattern,
            "minLength": minimum, "maxLength": maximum,
        });
        let values = property(&description.to_string());
        let expected = serde_json::to_string(expected).unwrap();
        assert!(
            values.iter().any(
                |(_, value, admission)| value == &expected && *admission == Admission::Admitted
            ),
            "{pattern}: {values:?}"
        );
    }
}

#[test]
fn unsupported_or_impossible_patterns_keep_fallbacks_without_admission_claims() {
    for pattern in ["^(a|b)$", "^(?=a)a$", "^[^a]$", "^é$", "^abc$"] {
        let values = property(
            &serde_json::json!({
                "type":"string", "nullable":false, "pattern":pattern, "maxLength":2
            })
            .to_string(),
        );
        assert!(
            !values
                .iter()
                .any(|(_, _, admission)| *admission == Admission::Admitted),
            "{pattern}: {values:?}"
        );
        has(&values, "type", r#""""#, Admission::Rejected);
    }
}

#[test]
fn overflowing_pattern_repetitions_keep_a_bounded_undetermined_fallback() {
    let values = property(
        r#"{"type":"string","nullable":false,
        "pattern":"^x{999999999999999999999999999999}$","maxLength":2}"#,
    );
    has(&values, "type", r#""""#, Admission::Undetermined);
    assert!(
        !values
            .iter()
            .any(|(_, _, admission)| *admission == Admission::Admitted)
    );
}

#[test]
fn nested_boundary_values_keep_patterned_required_siblings_admissible() {
    let values = property(
        r#"{"type":"object","nullable":false,"required":["id","level"],
        "properties":{"id":{"type":"string","nullable":false,"pattern":"^ID-[0-9]+$","minLength":5},
        "level":{"type":"integer","nullable":false,"minimum":10}}}"#,
    );
    has(
        &values,
        "level.minimum",
        r#"{"level":10,"id":"ID-00"}"#,
        Admission::Admitted,
    );
    has(
        &values,
        "level.below_minimum",
        r#"{"level":9,"id":"ID-00"}"#,
        Admission::Rejected,
    );
}

#[test]
fn pattern_synthesis_spends_its_description_allowance_without_starving_siblings() {
    let model = br#"{"sdfProperty":{
        "id":{"type":"string","nullable":false,"pattern":"^x{20000}$"},
        "level":{"type":"integer","minimum":10}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", model)],
        Limits::default().decoding_work(10_000),
    )
    .unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        ))
        .unwrap();
    let mut descriptions = result.descriptions();
    assert_eq!(
        descriptions.next().unwrap().state(),
        GenerationState::Partial
    );
    let level = descriptions.next().unwrap();
    assert_eq!(level.state(), GenerationState::Complete);
    assert!(
        level
            .values()
            .any(|value| value.to_json() == "10" && value.admission() == Admission::Admitted)
    );
    assert_eq!(result.disposition(), Disposition::Undetermined);
}

const THERMOSTAT: &str = include_str!("../../examples/thermostat-check/rich.sdf.json");

/// The selector of every description a whole-document request selects, in order.
fn selected(model: &str) -> Vec<(SampleKind, String)> {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        ))
        .unwrap();
    result
        .descriptions()
        .map(|description| (description.kind(), description.pointer().to_owned()))
        .collect()
}

#[test]
fn a_whole_document_selects_every_affordance_description_in_document_order() {
    let thermostat = "/sdfObject/Thermostat";
    assert_eq!(
        selected(THERMOSTAT),
        [
            (
                SampleKind::Property,
                format!("{thermostat}/sdfProperty/currentTemperature")
            ),
            (
                SampleKind::Property,
                format!("{thermostat}/sdfProperty/targetTemperature")
            ),
            (
                SampleKind::ActionInput,
                format!("{thermostat}/sdfAction/configure")
            ),
            (
                SampleKind::EventOutput,
                format!("{thermostat}/sdfEvent/alarm")
            ),
        ]
    );
    let nested = r#"{
        "sdfData":{"shared":{"type":"number"}},
        "sdfThing":{"room/1":{
            "sdfObject":{"lamp":{
                "sdfData":{"inner":{"type":"string"}},
                "sdfAction":{"dim":{"sdfInputData":{"type":"number"},"sdfOutputData":{"type":"boolean"}}}
            }},
            "sdfProperty":{"name":{"type":"string"}}
        }},
        "sdfProperty":{"top~level":{"type":"boolean"}},
        "sdfEvent":{"ping":{"description":"no data"}}
    }"#;
    assert_eq!(
        selected(nested),
        [
            (
                SampleKind::ActionInput,
                "/sdfThing/room~11/sdfObject/lamp/sdfAction/dim".to_owned()
            ),
            (
                SampleKind::ActionOutput,
                "/sdfThing/room~11/sdfObject/lamp/sdfAction/dim".to_owned()
            ),
            (
                SampleKind::Property,
                "/sdfThing/room~11/sdfProperty/name".to_owned()
            ),
            (SampleKind::Property, "/sdfProperty/top~0level".to_owned()),
        ]
    );
    // sdfData definitions are generated only when selected explicitly.
    let values = generated(nested, SampleKind::Data, "/sdfData/shared");
    assert!(values.iter().any(|row| row.0 == "type"));
}

#[test]
fn a_selector_that_selects_nothing_is_a_request_error() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", THERMOSTAT.as_bytes()),
        SourceInput::supplied("sample", b"1").sample(),
    ])
    .unwrap();
    let document = catalogue.source_id("model").unwrap();
    let thermostat = "/sdfObject/Thermostat";
    for (kind, pointer) in [
        (
            SampleKind::Property,
            format!("{thermostat}/sdfProperty/missing"),
        ),
        (
            SampleKind::Data,
            format!("{thermostat}/sdfProperty/targetTemperature"),
        ),
        (
            SampleKind::ActionOutput,
            format!("{thermostat}/sdfAction/configure"),
        ),
        (SampleKind::Property, "not a pointer".to_owned()),
    ] {
        let request = GenerationRequest::new(document).select(kind, &pointer);
        assert!(catalogue.generate(request).is_err(), "{kind:?} {pointer}");
    }
    let sample = catalogue.source_id("sample").unwrap();
    assert!(catalogue.generate(GenerationRequest::new(sample)).is_err());
}

#[test]
fn descriptions_beyond_the_ceiling_are_counted() {
    let properties: Vec<String> = (0..1030)
        .map(|index| format!(r#""p{index}":{{"type":"boolean"}}"#))
        .collect();
    let model = format!(r#"{{"sdfProperty":{{{}}}}}"#, properties.join(","));
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        ))
        .unwrap();
    assert_eq!(result.descriptions().len(), 1024);
    assert_eq!(result.descriptions_omitted(), 6);
    assert!(
        result
            .descriptions()
            .all(|description| description.state() == GenerationState::Complete)
    );
    assert_eq!(result.disposition(), Disposition::Undetermined);
}

#[test]
fn a_heavy_enum_leaves_the_other_descriptions_of_its_document_complete() {
    // Each value is checked with an allowance of its own, so a description with many enum
    // values never leaves the other descriptions of its document unsettled.
    let names: Vec<String> = (0..25).map(|index| format!("\"m{index}\"")).collect();
    let model = format!(
        r#"{{"sdfObject":{{"device":{{"sdfProperty":{{
            "mode":{{"type":"string","enum":[{}]}},
            "level":{{"type":"integer","minimum":0,"maximum":100}}
        }},"sdfAction":{{"set":{{"sdfInputData":{{"type":"boolean"}}}}}}}}}}}}"#,
        names.join(",")
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        ))
        .unwrap();
    let states: Vec<_> = result
        .descriptions()
        .map(|description| (description.pointer().to_owned(), description.state()))
        .collect();
    assert_eq!(states.len(), 3);
    assert!(
        states
            .iter()
            .all(|(_, state)| *state == GenerationState::Complete),
        "{states:?}"
    );
    let mode = result.descriptions().next().unwrap();
    assert_eq!(
        mode.values()
            .filter(|value| value.target() == "enum" && value.admission() == Admission::Admitted)
            .count(),
        25
    );
    assert_eq!(result.disposition(), Disposition::Passed);
}

#[test]
fn a_complete_generation_passes_and_an_exhausted_allowance_leaves_it_partial() {
    let model = THERMOSTAT.as_bytes();
    let run = |work: usize| {
        let limits = Limits::default().decoding_work(work);
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", model)], limits).ok()?;
        let result = catalogue
            .generate(GenerationRequest::new(
                catalogue.source_id("model").unwrap(),
            ))
            .ok()?;
        let states: Vec<_> = result
            .descriptions()
            .map(|description| description.state())
            .collect();
        Some((result.disposition(), states))
    };
    let (disposition, states) = run(10_000_000).unwrap();
    assert_eq!(disposition, Disposition::Passed);
    assert!(
        states
            .iter()
            .all(|state| *state == GenerationState::Complete)
    );
    // The least work that completes generation, below which it runs out.
    let (mut low, mut high) = (1, 10_000_000);
    while low < high {
        let middle = low + (high - low) / 2;
        if run(middle).is_some_and(|(disposition, _)| disposition == Disposition::Passed) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let (disposition, states) = run(low - 1).unwrap();
    assert_eq!(disposition, Disposition::Undetermined);
    assert!(states.contains(&GenerationState::Partial), "{states:?}");
}

/// A value's target, JSON text, admission and rejection codes.
type Row = (String, String, &'static str, Vec<String>);

#[test]
fn the_json_report_matches_the_library_result_and_repeats_byte_for_byte() {
    let report = || {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", THERMOSTAT.as_bytes())])
                .unwrap();
        let result = catalogue
            .generate(GenerationRequest::new(
                catalogue.source_id("model").unwrap(),
            ))
            .unwrap();
        let typed: Vec<Vec<Row>> = result
            .descriptions()
            .map(|description| {
                description
                    .values()
                    .map(|value| {
                        (
                            value.target().to_owned(),
                            value.to_json().to_owned(),
                            value.admission().as_str(),
                            value.codes().map(str::to_owned).collect(),
                        )
                    })
                    .collect()
            })
            .collect();
        (result.to_json(), typed, result.count(Admission::Rejected))
    };
    let (json, typed, rejected) = report();
    assert_eq!(report().0, json);
    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(wire["schema"], "esdieff.report");
    assert_eq!(wire["version"], "1.5");
    assert_eq!(wire["operation"], "generate");
    assert_eq!(
        wire["request"]["generation"]["contract"],
        "esdieff.generation/2"
    );
    assert_eq!(wire["request"]["generation"]["document"], 1);
    assert!(wire["request"]["generation"].get("selector").is_none());
    assert_eq!(wire["coverage"]["requested"]["count_unit"], "description");
    assert_eq!(wire["coverage"]["requested"]["total"], 4);
    assert_eq!(wire["coverage"]["requested"]["assessed"], 4);
    assert_eq!(wire["outcome"]["disposition"], "passed");
    let result = &wire["result"];
    assert_eq!(result["kind"], "generate");
    assert_eq!(result["summary"]["rejected"], rejected);
    let descriptions = result["descriptions"].as_array().unwrap();
    assert_eq!(descriptions.len(), typed.len());
    for (description, typed) in descriptions.iter().zip(&typed) {
        assert_eq!(description["state"], "complete");
        assert_eq!(description["values_omitted"], 0);
        let values = description["values"].as_array().unwrap();
        assert_eq!(values.len(), typed.len());
        for (value, (target, text, admission, codes)) in values.iter().zip(typed) {
            assert_eq!(value["target"], target.as_str());
            assert_eq!(
                value["value"],
                serde_json::from_str::<serde_json::Value>(text).unwrap()
            );
            assert_eq!(value["admission"], *admission);
            assert_eq!(value["codes"], serde_json::json!(codes));
        }
    }
    let first = &descriptions[0];
    assert_eq!(first["selector"]["kind"], "property");
    assert_eq!(
        first["description"]["pointer"],
        "/sdfObject/Thermostat/sdfProperty/currentTemperature"
    );
    assert_eq!(
        descriptions[2]["description"]["pointer"],
        "/sdfObject/Thermostat/sdfAction/configure/sdfInputData"
    );
}

#[test]
fn rejected_values_name_the_condition_codes_behind_them() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br#"{"sdfProperty":{"level":{"type":"integer","minimum":10}}}"#,
    )])
    .unwrap();
    let document = catalogue.source_id("model").unwrap();
    let result = catalogue
        .generate(
            GenerationRequest::new(document).select(SampleKind::Property, "/sdfProperty/level"),
        )
        .unwrap();
    let description = result.descriptions().next().unwrap();
    let below = description
        .values()
        .find(|value| value.target() == "below_minimum")
        .unwrap();
    assert_eq!(below.codes().collect::<Vec<_>>(), ["sample.bound"]);
    let minimum = description
        .values()
        .find(|value| value.target() == "minimum")
        .unwrap();
    assert_eq!(minimum.codes().len(), 0);
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(
        wire["request"]["generation"]["selector"],
        serde_json::json!({"kind":"property","document":1,"pointer":"/sdfProperty/level"})
    );
}

fn targets(values: &[(String, String, Admission)]) -> Vec<&str> {
    values.iter().map(|row| row.0.as_str()).collect()
}

#[test]
fn an_exclusive_maximum_gives_one_unit_inside_and_the_bound_itself() {
    let values = property(r#"{"type":"number","exclusiveMaximum":2.5}"#);
    has(&values, "maximum", "2.4", Admission::Admitted);
    has(&values, "above_maximum", "2.5", Admission::Rejected);
}

#[test]
fn bounds_keep_their_authored_spelling() {
    let values = property(r#"{"type":"number","minimum":1e1,"maximum":20.50}"#);
    has(&values, "minimum", "1e1", Admission::Admitted);
    has(&values, "maximum", "20.50", Admission::Admitted);
    has(&values, "above_maximum", "20.6", Admission::Rejected);
}

#[test]
fn a_bounded_integer_lists_its_targets_in_contract_order() {
    let values = property(r#"{"type":"integer","minimum":10,"maximum":20}"#);
    use Admission::{Admitted, Rejected};
    let expected = [
        ("null", "null", Admitted),
        ("minimum", "10", Admitted),
        ("maximum", "20", Admitted),
        ("below_minimum", "9", Rejected),
        ("above_maximum", "21", Rejected),
        ("wrong_type", "\"\"", Rejected),
        ("wrong_type", "true", Rejected),
        ("wrong_type", "{}", Rejected),
        ("wrong_type", "[]", Rejected),
    ];
    let expected: Vec<_> = expected
        .iter()
        .map(|(target, value, admission)| (target.to_string(), value.to_string(), *admission))
        .collect();
    assert_eq!(values, expected);
}

#[test]
fn string_lengths_give_boundary_and_outside_lengths() {
    let values = property(r#"{"type":"string","nullable":false,"minLength":2,"maxLength":4}"#);
    has(&values, "min_length", "\"aa\"", Admission::Admitted);
    has(&values, "max_length", "\"aaaa\"", Admission::Admitted);
    has(&values, "too_short", "\"a\"", Admission::Rejected);
    has(&values, "too_long", "\"aaaaa\"", Admission::Rejected);
    has(&values, "null", "null", Admission::Rejected);
    assert_eq!(
        targets(&values),
        [
            "null",
            "min_length",
            "max_length",
            "too_short",
            "too_long",
            "wrong_type",
            "wrong_type",
            "wrong_type",
            "wrong_type"
        ]
    );
}

#[test]
fn a_documentary_default_gives_its_own_target() {
    let values = property(r#"{"type":"number","default":21}"#);
    has(&values, "default", "21", Admission::Admitted);
    has(&values, "type", "0", Admission::Admitted);
}

#[test]
fn duplicates_follow_the_contract_examples_for_precedence_and_ties() {
    // A tie between Representative-value targets keeps the first target.
    let values = property(r#"{"type":"integer","const":0}"#);
    has(&values, "const", "0", Admission::Admitted);
    assert!(!values.iter().any(|row| row.1 == "0" && row.0 != "const"));
    // A boundary target ranks above a Representative-value target.
    let values = property(r#"{"type":"integer","const":3,"minimum":3}"#);
    has(&values, "minimum", "3", Admission::Admitted);
    assert!(!targets(&values).contains(&"const"));
    assert_eq!(values.iter().filter(|row| row.1 == "3").count(), 1);
}

#[test]
fn targets_appear_only_where_their_constraint_applies() {
    let values = property(r#"{"type":"string"}"#);
    assert_eq!(
        targets(&values),
        [
            "type",
            "null",
            "wrong_type",
            "wrong_type",
            "wrong_type",
            "wrong_type"
        ]
    );
    let values = property(r#"{"type":"boolean","nullable":false}"#);
    has(&values, "type", "true", Admission::Admitted);
    has(&values, "null", "null", Admission::Rejected);
    has(&values, "wrong_type", "0", Admission::Rejected);
    // Without a type every base type is admitted, so no value has a wrong type.
    let values = property(r#"{"description":"anything"}"#);
    assert_eq!(
        targets(&values),
        ["type", "type", "type", "type", "type", "null"]
    );
}

#[test]
fn a_step_gives_a_value_half_a_step_past_a_lattice_value() {
    let values = property(r#"{"type":"number","minimum":5,"maximum":30,"multipleOf":0.5}"#);
    has(&values, "not_multiple", "5.25", Admission::Rejected);
    let values = property(r#"{"type":"number","multipleOf":3}"#);
    has(&values, "not_multiple", "1.5", Admission::Rejected);
    let values = property(r#"{"type":"number","minimum":-2.5,"multipleOf":0.5}"#);
    has(&values, "not_multiple", "-2.25", Admission::Rejected);
}

#[test]
fn a_pattern_adds_no_target_and_unsettled_values_stay_undetermined() {
    let values = property(r#"{"type":"string","pattern":"^[0-9]+$"}"#);
    has(&values, "type", "\"0\"", Admission::Admitted);
    assert!(
        !targets(&values)
            .iter()
            .any(|target| target.contains("pattern"))
    );
    let values = property(r#"{"type":"string","pattern":"["}"#);
    has(&values, "type", "\"\"", Admission::Undetermined);
}

#[test]
fn a_format_gives_its_conventional_value() {
    let values = property(r#"{"type":"string","format":"date-time"}"#);
    has(
        &values,
        "type",
        "\"1970-01-01T00:00:00Z\"",
        Admission::Admitted,
    );
}

#[test]
fn an_array_gives_item_count_boundaries_and_values_outside_them() {
    let values = property(
        r#"{"type":"array","minItems":2,"maxItems":4,"uniqueItems":true,
            "items":{"type":"integer"}}"#,
    );
    has(&values, "min_items", "[0,1]", Admission::Admitted);
    has(&values, "max_items", "[0,1,2,3]", Admission::Admitted);
    has(&values, "too_few_items", "[0]", Admission::Rejected);
    has(
        &values,
        "too_many_items",
        "[0,1,2,3,4]",
        Admission::Rejected,
    );
    has(&values, "duplicate_items", "[0,0]", Admission::Rejected);
    // An item minimum of 0 gives the same array as the item count minimum, and the nested
    // boundary target keeps it.
    let values = property(
        r#"{"type":"array","minItems":2,"uniqueItems":true,
            "items":{"type":"integer","minimum":0}}"#,
    );
    has(&values, "items.minimum", "[0,1]", Admission::Admitted);
    assert!(!targets(&values).contains(&"min_items"));
    // Repeated items fill a longer minimum count.
    let values =
        property(r#"{"type":"array","minItems":3,"uniqueItems":true,"items":{"type":"integer"}}"#);
    has(&values, "duplicate_items", "[0,0,0]", Admission::Rejected);
    // Without uniqueItems the items repeat the first representative item.
    let values = property(r#"{"type":"array","minItems":2,"items":{"type":"string"}}"#);
    has(&values, "min_items", "[\"\",\"\"]", Admission::Admitted);
    assert!(!targets(&values).contains(&"duplicate_items"));
}

const SETTINGS: &str = r#"{"type":"object","required":["mode","level"],"properties":{
    "mode":{"type":"string","nullable":false,"enum":["off","heat"]},
    "level":{"type":"integer","nullable":false,"minimum":5},
    "note":{"type":"string","nullable":false}
}}"#;

#[test]
fn a_map_gives_all_entries_and_one_value_missing_each_required_entry() {
    // Two optional entries keep the map of every entry apart from each entry's own values,
    // which rank above it.
    let values = property(
        r#"{"type":"object","required":["mode","level"],"properties":{
            "mode":{"type":"string","nullable":false,"enum":["off","heat"]},
            "level":{"type":"integer","nullable":false,"minimum":5},
            "note":{"type":"string","nullable":false},
            "label":{"type":"string","nullable":false}
        }}"#,
    );
    has(
        &values,
        "all_entries",
        r#"{"mode":"off","level":5,"note":"","label":""}"#,
        Admission::Admitted,
    );
    has(
        &values,
        "missing_entry",
        r#"{"level":5}"#,
        Admission::Rejected,
    );
    has(
        &values,
        "missing_entry",
        r#"{"mode":"off"}"#,
        Admission::Rejected,
    );
    assert_eq!(
        targets(&values)
            .iter()
            .filter(|target| **target == "missing_entry")
            .count(),
        2
    );
}

#[test]
fn a_nested_entry_keeps_the_other_required_entries_filled() {
    // The level minimum's map holds both required entries at their Representative values, as
    // the type's map and the first mode value's map do, and it keeps the value as the target
    // with the highest precedence.
    let values = property(SETTINGS);
    has(
        &values,
        "level.minimum",
        r#"{"level":5,"mode":"off"}"#,
        Admission::Admitted,
    );
    has(
        &values,
        "level.below_minimum",
        r#"{"level":4,"mode":"off"}"#,
        Admission::Rejected,
    );
    has(
        &values,
        "note.wrong_type",
        r#"{"note":0,"mode":"off","level":5}"#,
        Admission::Rejected,
    );
    has(
        &values,
        "mode.null",
        r#"{"mode":null,"level":5}"#,
        Admission::Rejected,
    );
    let values = property(
        r#"{"type":"array","minItems":2,"uniqueItems":true,
            "items":{"type":"integer","minimum":3,"maximum":9}}"#,
    );
    has(&values, "items.maximum", "[9,3]", Admission::Admitted);
    has(
        &values,
        "items.above_maximum",
        "[10,3]",
        Admission::Rejected,
    );
    assert!(!targets(&values).contains(&"items.null"));
}

#[test]
fn maps_with_the_same_entries_in_another_order_are_kept_once() {
    let values = property(SETTINGS);
    let parsed: Vec<serde_json::Value> = values
        .iter()
        .map(|row| serde_json::from_str(&row.1).unwrap())
        .collect();
    for (index, value) in parsed.iter().enumerate() {
        assert!(
            !parsed[..index].contains(value),
            "{} repeats an earlier value",
            values[index].0
        );
    }
    // The type's map {"mode":"off","level":5} is also the first mode value's map and the
    // level minimum's map, the same entries in another order, so it is kept once, under the
    // level minimum, a boundary target, with that target's text and place.
    assert!(!targets(&values).contains(&"type"));
    let mode: Vec<_> = values
        .iter()
        .filter(|row| row.0 == "mode.enum")
        .map(|row| row.1.as_str())
        .collect();
    assert_eq!(mode, [r#"{"mode":"heat","level":5}"#]);
    has(
        &values,
        "level.minimum",
        r#"{"level":5,"mode":"off"}"#,
        Admission::Admitted,
    );
}

/// A Catalogue of `model` with the report byte limit `limit`, when it accepts one.
fn limited(model: &str, limit: usize) -> Option<Catalogue> {
    let limits = Limits::default().report_bytes(limit);
    Catalogue::with_limits(&[SourceInput::supplied("model", model.as_bytes())], limits).ok()
}

fn whole(catalogue: &Catalogue) -> Result<esdieff::GenerationResult<'_>, &'static str> {
    catalogue.generate(GenerationRequest::new(
        catalogue.source_id("model").unwrap(),
    ))
}

/// Every output of `result` and whether each fits `limit`.
fn outputs_fit(result: &esdieff::GenerationResult<'_>, limit: usize) {
    for (format, output) in [
        ("json", result.to_json()),
        ("human", result.to_human()),
        ("jsonl", result.to_json_lines()),
    ] {
        assert!(output.len() <= limit, "{format} {} > {limit}", output.len());
    }
}

#[test]
fn the_mandatory_envelope_needs_its_reserve_and_the_reserve_suffices() {
    // The least report limit that admits a generation of the thermostat model.
    let accepted = |limit| limited(THERMOSTAT, limit).is_some_and(|c| whole(&c).is_ok());
    let (mut low, mut high) = (1, 8 * 1024 * 1024);
    while low < high {
        let middle = low + (high - low) / 2;
        if accepted(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let catalogue = limited(THERMOSTAT, low - 1).unwrap();
    assert_eq!(
        whole(&catalogue).err(),
        Some("report cannot retain its mandatory envelope")
    );
    let catalogue = limited(THERMOSTAT, low).unwrap();
    let result = whole(&catalogue).unwrap();
    outputs_fit(&result, low);
    // No description's details fit, so each is counted with a report_limit record.
    assert_eq!(result.descriptions().len(), 0);
    assert_eq!(result.descriptions_omitted(), 4);
    assert_eq!(result.disposition(), Disposition::Undetermined);
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(
        wire["result"]["omissions"],
        serde_json::json!([{"field":"result.descriptions","reason":"report_limit","omitted_count":4}])
    );
    assert_eq!(wire["coverage"]["requested"]["unassessed"], 4);
}

#[test]
fn values_that_do_not_fit_the_report_are_omitted_and_counted() {
    let model = r#"{"sdfProperty":{
        "a":{"type":"string","maxLength":20000},
        "b":{"type":"string","maxLength":20000},
        "c":{"type":"string","maxLength":20000}
    }}"#;
    // The least limit that admits the request, then room for about one long string.
    let minimum = (1..=8 * 1024 * 1024)
        .step_by(1024)
        .find(|limit| limited(model, *limit).is_some_and(|c| whole(&c).is_ok()))
        .unwrap();
    let limit = minimum + 30_000;
    let catalogue = limited(model, limit).unwrap();
    let result = whole(&catalogue).unwrap();
    outputs_fit(&result, limit);
    assert_eq!(result.descriptions().len(), 3);
    let omitted: u32 = result
        .descriptions()
        .map(|description| description.values_omitted())
        .sum();
    assert!(omitted >= 4, "{omitted}");
    assert!(
        result
            .descriptions()
            .any(|description| description.state() == GenerationState::Partial)
    );
    assert_eq!(result.disposition().exit_code(), 3);
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    let omissions = wire["result"]["omissions"].as_array().unwrap();
    assert!(omissions.contains(&serde_json::json!({
        "field":"result.descriptions.values","reason":"byte_limit","omitted_count":omitted
    })));
    assert_eq!(wire["diagnostics"]["truncated"], true);
    // The short values of every description still fit.
    for description in result.descriptions() {
        assert!(description.values().any(|value| value.target() == "null"));
    }
}

#[test]
fn a_description_whose_meaning_is_unavailable_has_no_values() {
    let model = r##"{"sdfProperty":{
        "missing":{"sdfRef":"#/sdfData/absent"},
        "level":{"type":"integer","minimum":0}
    }}"##;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = whole(&catalogue).unwrap();
    let states: Vec<_> = result
        .descriptions()
        .map(|description| (description.state(), description.values().len()))
        .collect();
    assert_eq!(states[0], (GenerationState::Unavailable, 0));
    assert_eq!(states[1].0, GenerationState::Complete);
    assert_eq!(result.disposition(), Disposition::Undetermined);
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["result"]["descriptions"][0]["state"], "unavailable");
    assert_eq!(wire["coverage"]["requested"]["undetermined"], 1);
}

#[test]
fn an_unreadable_grouping_leaves_the_selection_partial() {
    let model = r##"{
        "sdfObject":{"broken":{"sdfRef":"#/sdfObject/absent"}},
        "sdfProperty":{"level":{"type":"integer","minimum":0}}
    }"##;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = whole(&catalogue).unwrap();
    assert!(!result.selection_complete());
    assert_eq!(result.disposition(), Disposition::Undetermined);
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["result"]["selection"], "partial");
    assert_eq!(
        wire["coverage"]["requested"]["total"],
        serde_json::Value::Null
    );
    assert_eq!(
        wire["result"]["descriptions"][0]["selector"]["pointer"],
        "/sdfProperty/level"
    );
}

#[test]
fn each_choice_alternative_gives_one_value_and_enum_values_keep_their_order() {
    let values = property(
        r#"{"nullable":false,"sdfChoice":{
            "low":{"type":"number","maximum":0},
            "high":{"type":"number","minimum":100}
        }}"#,
    );
    let alternatives: Vec<_> = values
        .iter()
        .filter(|row| row.0 == "alternative")
        .map(|row| (row.1.as_str(), row.2))
        .collect();
    assert_eq!(
        alternatives,
        [("0", Admission::Admitted), ("100", Admission::Admitted)]
    );
    let values = property(r#"{"type":"string","enum":["off","heat","cool"]}"#);
    let enumerated: Vec<_> = values
        .iter()
        .filter(|row| row.0 == "enum")
        .map(|row| row.1.as_str())
        .collect();
    assert_eq!(enumerated, ["\"off\"", "\"heat\"", "\"cool\""]);
}

/// How many arrays and maps a JSON value nests, as the depth limit counts them.
fn depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(items) => 1 + items.iter().map(depth).max().unwrap_or(0),
        serde_json::Value::Object(entries) => 1 + entries.values().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn nested_values_stay_within_the_value_capabilities_and_the_ceiling() {
    let entries: Vec<String> = (0..12)
        .map(|index| format!(r#""e{index}":{{"type":"boolean"}}"#))
        .collect();
    let model = format!(
        r#"{{"sdfProperty":{{"flags":{{"type":"object","properties":{{{}}}}}}}}}"#,
        entries.join(",")
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let document = catalogue.source_id("model").unwrap();
    let result = catalogue
        .generate(
            GenerationRequest::new(document).select(SampleKind::Property, "/sdfProperty/flags"),
        )
        .unwrap();
    let description = result.descriptions().next().unwrap();
    assert_eq!(description.values().len(), 64);
    // Seven targets of the map itself and six of each of its twelve entries, 79 in all.
    assert_eq!(description.values_omitted(), 79 - 64);
    assert_eq!(description.state(), GenerationState::Partial);
    assert!(
        description
            .values()
            .all(|value| value.admission() != Admission::Undetermined)
    );
    assert_eq!(result.disposition(), Disposition::Undetermined);

    // A chain of references composes deeper than any source nests.
    let mut definitions = Vec::new();
    for index in 0..12 {
        definitions.push(format!(
            r##""d{index}":{{"type":"object","properties":{{"n":{{"sdfRef":"#/sdfData/d{}"}}}}}}"##,
            index + 1
        ));
    }
    definitions.push(r#""d12":{"type":"boolean"}"#.to_owned());
    let model = format!(
        r##"{{"sdfData":{{{}}},"sdfProperty":{{"chain":{{"sdfRef":"#/sdfData/d0"}}}}}}"##,
        definitions.join(",")
    );
    let limit = 6;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", model.as_bytes())],
        Limits::default().depth(limit),
    )
    .unwrap();
    let document = catalogue.source_id("model").unwrap();
    let result = catalogue
        .generate(
            GenerationRequest::new(document).select(SampleKind::Property, "/sdfProperty/chain"),
        )
        .unwrap();
    let description = result.descriptions().next().unwrap();
    let deepest = description
        .values()
        .map(|value| depth(&serde_json::from_str(value.to_json()).unwrap()))
        .max()
        .unwrap();
    assert!(deepest <= limit, "{deepest}");
    // Entries nest as deep as the limit allows a value to, and no deeper.
    let targets: Vec<_> = description.values().map(|value| value.target()).collect();
    assert!(targets.contains(&"n.n.n.n.n.null"), "{targets:?}");
    assert!(
        !targets
            .iter()
            .any(|target| target.starts_with("n.n.n.n.n.n."))
    );
}
