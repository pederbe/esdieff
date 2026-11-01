use esdieff::{Catalogue, GenerationRequest, SourceInput};
use serde_json::Value;

fn report(description: &str) -> Value {
    let model = format!(r#"{{"sdfProperty":{{"setting":{description}}}}}"#);
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        ))
        .unwrap();
    serde_json::from_str(&result.to_json()).unwrap()
}

#[test]
fn disjoint_choices_have_witnesses_from_retained_admitted_values() {
    let wire = report(r#"{"nullable":false,"sdfChoice":{"low":{"const":1},"high":{"const":9}}}"#);
    let description = &wire["result"]["descriptions"][0];
    let evidence = &description["choice_evidence"];
    assert_eq!(evidence["state"], "complete");
    let rows = evidence["alternatives"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for (row, name, value) in [(&rows[0], "low", 1), (&rows[1], "high", 9)] {
        assert_eq!(row["alternative"], name);
        assert_eq!(row["status"], "witnessed");
        assert_eq!(row["witness"]["sample_pointer"], "");
        let index = row["witness"]["value_index"].as_u64().unwrap() as usize;
        assert_eq!(description["values"][index]["value"], value);
        assert_eq!(description["values"][index]["admission"], "admitted");
    }
}

fn rows(wire: &Value) -> &[Value] {
    wire["result"]["descriptions"][0]["choice_evidence"]["alternatives"]
        .as_array()
        .unwrap()
}

#[test]
fn overlapping_and_identical_alternatives_credit_only_the_first_admission() {
    for (first, second) in [("wide", "narrow"), ("narrow", "wide")] {
        let wire = report(&format!(
            r#"{{"nullable":false,"sdfChoice":{{"{first}":{{"type":"integer","minimum":0,"maximum":5}},"{second}":{{"type":"integer","minimum":0,"maximum":5}}}}}}"#
        ));
        assert_eq!(rows(&wire)[0]["alternative"], first);
        assert_eq!(rows(&wire)[0]["status"], "witnessed");
        assert_eq!(rows(&wire)[1]["status"], "no_witness_established");
        assert!(rows(&wire)[1].get("witness").is_none());
    }
    for fragment in [r#"{"const":1}"#, r#"{"description":"A documentary name"}"#] {
        let wire = report(&format!(
            r#"{{"nullable":false,"sdfChoice":{{"off":{fragment},"on":{fragment}}}}}"#
        ));
        assert_eq!(rows(&wire)[0]["status"], "witnessed");
        assert_eq!(rows(&wire)[1]["status"], "no_witness_established");
    }
    let wire = report(r#"{"type":"string","enum":["off","on"]}"#);
    assert!(rows(&wire).is_empty());
}

#[test]
fn nested_witnesses_require_whole_map_admission() {
    for (maximum, witnessed) in [("", true), (",\"maximum\":0", false)] {
        let description = r#"{"type":"object","nullable":false,"required":["mode","level"],"properties":{"mode":{"nullable":false,"sdfChoice":{"off":{"const":"off"},"on":{"const":"on"}}},"level":{"type":"integer","nullable":false,"minimum":5MAXIMUM}}}"#.replace("MAXIMUM", maximum);
        let wire = report(&description);
        assert_eq!(rows(&wire).len(), 2);
        for row in rows(&wire) {
            assert_eq!(
                row["owner"],
                serde_json::json!([{"kind":"property","name":"mode"}])
            );
            assert_eq!(
                row["status"],
                if witnessed {
                    "witnessed"
                } else {
                    "no_witness_established"
                }
            );
            if witnessed {
                assert_eq!(row["witness"]["sample_pointer"], "/mode");
                let index = row["witness"]["value_index"].as_u64().unwrap() as usize;
                assert_eq!(
                    wire["result"]["descriptions"][0]["values"][index]["admission"],
                    "admitted"
                );
            }
        }
    }
}

#[test]
fn unsuccessful_outer_proofs_discard_their_successful_nested_choices() {
    let wire = report(
        r#"{"nullable":false,"sdfChoice":{
        "fails":{"type":"object","required":["mode","level"],"properties":{
            "mode":{"nullable":false,"sdfChoice":{"local":{"const":"on"}}},
            "level":{"const":4,"minimum":5,"type":"integer"}}},
        "wins":{"type":"object","required":["mode","level"],"properties":{
            "mode":{"const":"on"},"level":{"const":4}}}
    }}"#,
    );
    assert_eq!(rows(&wire).len(), 3);
    assert_eq!(rows(&wire)[0]["status"], "no_witness_established");
    assert_eq!(rows(&wire)[1]["alternative"], "local");
    assert_eq!(rows(&wire)[1]["status"], "no_witness_established");
    assert_eq!(rows(&wire)[2]["status"], "witnessed");
}

#[test]
fn array_witnesses_use_the_first_actual_item_and_repeat_once() {
    let wire = report(
        r#"{"type":"array","nullable":false,"minItems":2,"items":{
        "sdfChoice":{"low":{"type":"integer","nullable":false,"const":1},"high":{"type":"integer","nullable":false,"const":9}}}}"#,
    );
    assert_eq!(rows(&wire).len(), 2);
    for row in rows(&wire) {
        assert_eq!(row["status"], "witnessed", "{wire:#}");
        assert_eq!(row["owner"], serde_json::json!([{"kind":"items"}]));
        let pointer = row["witness"]["sample_pointer"].as_str().unwrap();
        let index = row["witness"]["value_index"].as_u64().unwrap() as usize;
        let values = wire["result"]["descriptions"][0]["values"]
            .as_array()
            .unwrap();
        let value = &values[index]["value"];
        let wanted = if row["alternative"] == "low" { 1 } else { 9 };
        assert_eq!(*value.pointer(pointer).unwrap(), wanted);
        let first = value
            .as_array()
            .unwrap()
            .iter()
            .position(|value| *value == wanted)
            .unwrap();
        assert_eq!(pointer, format!("/{first}"));
        assert_eq!(values[index]["admission"], "admitted");
    }
}

#[test]
fn inherited_qualities_overrides_and_ancestor_contexts_remain_distinct() {
    let wire = report(
        r#"{"nullable":false,"type":"object","required":["mode"],"sdfChoice":{
        "first":{"properties":{"mode":{"nullable":false,"sdfChoice":{"same":{"const":1}}}}},
        "second":{"properties":{"mode":{"nullable":false,"sdfChoice":{"same":{"const":9}}}}}
    }}"#,
    );
    let rows = rows(&wire);
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[1]["owner"],
        serde_json::json!([{"kind":"choice","name":"first"},{"kind":"property","name":"mode"}])
    );
    assert_eq!(
        rows[3]["owner"],
        serde_json::json!([{"kind":"choice","name":"second"},{"kind":"property","name":"mode"}])
    );
    assert!(rows.iter().all(|row| row["status"] == "witnessed"));
}

#[test]
fn undetermined_whole_values_and_documentary_maps_supply_no_false_witnesses() {
    let wire = report(
        r#"{"type":"object","nullable":false,"required":["mode","unknown"],"properties":{
        "mode":{"nullable":false,"sdfChoice":{"local":{"const":"on"}}},
        "unknown":{"type":"string","format":"acme:unknown"}},
        "default":{"sdfChoice":{"not_a_description":{"const":1}}},
        "description":"sdfChoice is documentary here"}"#,
    );
    assert_eq!(rows(&wire).len(), 1);
    assert_eq!(rows(&wire)[0]["status"], "no_witness_established");
}

#[test]
fn optional_report_details_cannot_displace_typed_values_and_remain_bounded() {
    use esdieff::{ChoiceEvidenceState, Limits};
    let alternatives = (0..256)
        .map(|index| format!(r#""alternative_with_a_long_name_to_exercise_optional_presentation_capacity_{index}":{{"const":1}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let model = r#"{"sdfProperty":{"a":{"nullable":false,"sdfChoice":{ALTERNATIVES}},"b":{"nullable":false,"sdfChoice":{ALTERNATIVES}}}}"#.replace("ALTERNATIVES", &alternatives);
    let mut absent = false;
    let mut partial = false;
    let mut complete = false;
    for limit in (5_000..150_000).step_by(1137) {
        let Ok(catalogue) = Catalogue::with_limits(
            &[SourceInput::supplied("model", model.as_bytes())],
            Limits::default().report_bytes(limit),
        ) else {
            continue;
        };
        let Ok(result) = catalogue.generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        )) else {
            continue;
        };
        let json = result.to_json();
        let human = result.to_human();
        let lines = result.to_json_lines();
        assert!(json.len() <= limit && human.len() <= limit && lines.len() <= limit);
        let wire: Value = serde_json::from_str(&json).unwrap();
        let typed: Vec<_> = result.descriptions().collect();
        let shown = wire["result"]["descriptions"].as_array().unwrap();
        assert_eq!(typed.len(), shown.len());
        for (description, shown) in typed.iter().zip(shown) {
            assert_eq!(
                description.values().len(),
                shown["values"].as_array().unwrap().len()
            );
            assert_eq!(
                description.choice_evidence().state(),
                ChoiceEvidenceState::Complete
            );
            assert_eq!(description.choice_evidence().alternatives().len(), 256);
            match shown.get("choice_evidence") {
                None => absent = true,
                Some(evidence) => {
                    let count = evidence["alternatives"].as_array().unwrap().len();
                    assert_eq!(
                        count + evidence["alternatives_omitted"].as_u64().unwrap() as usize,
                        256
                    );
                    match evidence["state"].as_str().unwrap() {
                        "partial" => partial = true,
                        "complete" => complete = true,
                        other => panic!("{other}"),
                    }
                }
            }
        }
        assert_eq!(json, result.to_json());
        assert_eq!(lines, result.to_json_lines());
    }
    assert!(
        absent && partial && complete,
        "absent={absent} partial={partial} complete={complete}"
    );
}

#[test]
fn referenced_shared_qualities_and_explicit_overrides_use_effective_meaning() {
    let model = br##"{"sdfData":{"Base":{"type":"integer","minimum":5,"nullable":false,"sdfChoice":{"low":{"const":1,"minimum":0},"high":{"const":9}}}},"sdfProperty":{"setting":{"sdfRef":"#/sdfData/Base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        ))
        .unwrap();
    let description = result.descriptions().next().unwrap();
    for row in description.choice_evidence().alternatives() {
        let witness = row
            .witness()
            .expect("final alternative has an admitted constant");
        let value = description.values().nth(witness.value_index()).unwrap();
        assert_eq!(value.admission(), esdieff::Admission::Admitted);
        assert_eq!(value.to_json(), if row.name() == "low" { "1" } else { "9" });
    }
}

#[test]
fn each_supported_selector_scopes_the_evidence_to_its_selected_description() {
    use esdieff::SampleKind;
    let choice = serde_json::json!({"nullable":false,"sdfChoice":{"yes":{"const":true},"no":{"const":false}}});
    let model = serde_json::json!({"sdfData":{"d":choice},"sdfProperty":{"p":choice},"sdfAction":{"a":{"sdfInputData":choice,"sdfOutputData":choice}},"sdfEvent":{"e":{"sdfOutputData":choice}}}).to_string();
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    for (kind, pointer) in [
        (SampleKind::Data, "/sdfData/d"),
        (SampleKind::Property, "/sdfProperty/p"),
        (SampleKind::ActionInput, "/sdfAction/a"),
        (SampleKind::ActionOutput, "/sdfAction/a"),
        (SampleKind::EventOutput, "/sdfEvent/e"),
    ] {
        let result = catalogue
            .generate(
                GenerationRequest::new(catalogue.source_id("model").unwrap()).select(kind, pointer),
            )
            .unwrap();
        let description = result.descriptions().next().unwrap();
        assert_eq!(description.pointer(), pointer);
        assert_eq!(description.kind(), kind);
        assert!(
            description
                .choice_evidence()
                .alternatives()
                .all(|row| row.owner().is_empty() && row.witness().is_some())
        );
    }
}

#[test]
fn enumeration_respects_applicable_structure_and_direct_item_limits() {
    let wire = report(
        r#"{"type":"number","properties":{"unused":{"sdfChoice":{"hidden":{"const":1}}}},"items":{"sdfChoice":{"also_hidden":{"const":1}}}}"#,
    );
    assert!(rows(&wire).is_empty());
    assert_eq!(
        wire["result"]["descriptions"][0]["choice_evidence"]["state"],
        "complete"
    );
    for description in [
        r#"{"properties":{"unknown":{"sdfChoice":{"hidden":{"const":1}}}}}"#,
        r#"{"type":"array","items":{"type":"array","items":{"sdfChoice":{"hidden":{"const":1}}}}}"#,
    ] {
        let wire = report(description);
        assert!(rows(&wire).is_empty());
        assert_eq!(
            wire["result"]["descriptions"][0]["choice_evidence"]["state"],
            "partial"
        );
    }
}

#[test]
fn duplicate_winning_target_moves_choose_the_first_final_retained_witness() {
    let model = br#"{"sdfProperty":{"setting":{"type":"object","nullable":false,"required":["mode","level"],"properties":{
        "mode":{"type":"integer","nullable":false,"sdfChoice":{"wide":{},"narrow":{"const":1}}},
        "level":{"type":"integer","nullable":false,"minimum":1,"maximum":2}
    }}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let result = catalogue
        .generate(GenerationRequest::new(
            catalogue.source_id("model").unwrap(),
        ))
        .unwrap();
    let description = result.descriptions().next().unwrap();
    let row = description
        .choice_evidence()
        .alternatives()
        .find(|row| row.name() == "wide")
        .unwrap();
    let first = description
        .values()
        .position(|value| value.admission() == esdieff::Admission::Admitted)
        .unwrap();
    assert_eq!(row.witness().unwrap().value_index(), first);
    let value = description.values().nth(first).unwrap();
    assert_eq!(value.target(), "mode.alternative");
    let value: Value = serde_json::from_str(value.to_json()).unwrap();
    assert_eq!(value["mode"], 1);
    assert_eq!(value["level"], 1);
    assert_eq!(row.witness().unwrap().sample_pointer(), "/mode");
}

#[test]
fn unreadable_siblings_preserve_known_rows_without_inventing_membership() {
    let wire = report(
        r#"{"nullable":false,"sdfChoice":{"known":{"const":1},"unreadable":{"sdfRef":"missing:#/sdfData/Unknown"}}}"#,
    );
    let evidence = &wire["result"]["descriptions"][0]["choice_evidence"];
    assert_eq!(evidence["state"], "partial");
    assert_eq!(rows(&wire).len(), 2);
    assert_eq!(rows(&wire)[1]["alternative"], "unreadable");
    assert_eq!(rows(&wire)[1]["status"], "no_witness_established");
    assert!(evidence.get("total").is_none());
    assert_eq!(evidence["alternatives_omitted"], 0);
}
