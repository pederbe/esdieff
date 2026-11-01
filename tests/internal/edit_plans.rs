use esdieff::{Catalogue, Edit, EditRequest, Fidelity, Generation, JsonBuilder, SourceInput};

fn catalogue(bytes: &[u8]) -> Catalogue {
    Catalogue::from_inputs(&[SourceInput::supplied("base", bytes)]).unwrap()
}

#[test]
fn data_quality_capability_is_exactly_property_and_data_without_shape_repair() {
    let qualities = [
        "type",
        "sdfType",
        "const",
        "default",
        "enum",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "minLength",
        "maxLength",
        "pattern",
        "format",
        "minItems",
        "maxItems",
        "uniqueItems",
        "nullable",
        "contentFormat",
        "unit",
    ];
    for group in ["sdfProperty", "sdfData"] {
        for quality in qualities {
            let source =
                format!(r#"{{"{group}":{{"a":{{"{quality}":null}},"b":{{"{quality}":true}}}}}}"#);
            let inputs = catalogue(source.as_bytes());
            let selected = inputs.source_id("base").unwrap();
            let result = EditRequest::new(inputs, "correct", selected, "revised")
                .unwrap()
                .with_edit(
                    Edit::replace(
                        &format!("/{group}/a"),
                        quality,
                        JsonBuilder::new().text("replacement").unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap()
                .with_edit(Edit::remove(&format!("/{group}/b"), quality).unwrap())
                .unwrap()
                .edit()
                .unwrap();
            assert_eq!(
                result.fidelity(),
                Fidelity::Established,
                "{group} {quality}"
            );
            assert_eq!(result.generation(), Generation::Complete);
            let root = result
                .catalogue()
                .source(result.generated_source())
                .unwrap()
                .json()
                .require()
                .unwrap();
            let group = root.get(group).require().unwrap();
            assert_eq!(
                group
                    .get("a")
                    .require()
                    .unwrap()
                    .get(quality)
                    .require()
                    .unwrap()
                    .text()
                    .require()
                    .unwrap(),
                "replacement"
            );
            assert!(
                group
                    .get("b")
                    .require()
                    .unwrap()
                    .get(quality)
                    .present()
                    .is_none()
            );
        }
    }
    for group in ["sdfThing", "sdfObject", "sdfAction", "sdfEvent"] {
        for quality in qualities {
            let source = format!(r#"{{"{group}":{{"a":{{"{quality}":null}}}}}}"#);
            let inputs = catalogue(source.as_bytes());
            let selected = inputs.source_id("base").unwrap();
            assert!(
                EditRequest::new(inputs, "correct", selected, "revised")
                    .unwrap()
                    .with_edit(Edit::remove(&format!("/{group}/a"), quality).unwrap())
                    .unwrap()
                    .edit()
                    .is_err(),
                "{group} {quality}"
            );
        }
    }
}

#[test]
fn textual_plan_preserves_literal_lexemes_and_distinct_intent_spans() {
    let source = r##"{"sdfProperty":{"space /~#é":{"description":"old","maximum":5,"default":{"old":true},"const":4,"unit":"K","x:opaque":[0.10]},"sibling":{"sdfRef":"#/sdfProperty/space ~1~0#é"}}}"##;
    let plan = r#"{"key":"correction","source":"base","generated_key":"revised","generated_label":"Edited","edits":[{"owner":"/sdfProperty/space ~1~0#é","quality":"description","operation":"replace","value":"corrected"},{"owner":"/sdfProperty/space ~1~0#é","quality":"maximum","operation":"replace","value":1.2300e+02},{"owner":"/sdfProperty/space ~1~0#é","quality":"default","operation":"replace","value":{"nested":[-0.00,0.10,1e1000000,1e-1000000,null]}},{"owner":"/sdfProperty/space ~1~0#é","quality":"const","operation":"replace","value":null},{"owner":"/sdfProperty/space ~1~0#é","quality":"unit","operation":"remove"}]}"#;
    let run = || {
        EditRequest::from_plan(catalogue(source.as_bytes()), plan.as_bytes())
            .unwrap()
            .edit()
            .unwrap()
    };
    let result = run();
    assert!(result.document_deliverable());
    assert_eq!(result.request().plan_text(), Some(plan));
    assert_eq!(result.original_source().raw_bytes(), source.as_bytes());
    assert_eq!(result.generated_document(), run().generated_document());
    let root = result
        .catalogue()
        .source(result.generated_source())
        .unwrap()
        .json()
        .require()
        .unwrap();
    let property = root
        .get("sdfProperty")
        .require()
        .unwrap()
        .get("space /~#é")
        .require()
        .unwrap();
    assert_eq!(
        property
            .get("description")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "corrected"
    );
    assert_eq!(
        property
            .get("maximum")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "1.2300e+02"
    );
    let literal = property
        .get("default")
        .require()
        .unwrap()
        .get("nested")
        .require()
        .unwrap();
    for (index, lexeme) in ["-0.00", "0.10", "1e1000000", "1e-1000000"]
        .iter()
        .enumerate()
    {
        let item = literal.at(index).require().unwrap();
        assert_eq!(item.number().require().unwrap().lexeme(), *lexeme);
        let location = result.request_origin(item).require().unwrap();
        assert_eq!(
            location.pointer().require().unwrap(),
            format!("/edits/2/value/nested/{index}")
        );
        let span = location.span().unwrap();
        assert_eq!(&plan[span.start as usize..span.end as usize], *lexeme);
    }
    assert_eq!(
        property.get("const").require().unwrap().kind(),
        esdieff::JsonKind::Null
    );
    assert!(property.get("unit").present().is_none());
    let removal = result.target(4).require().unwrap();
    assert!(matches!(removal.generated_member(), esdieff::Field::Absent));
    let location = removal.request_location();
    assert_eq!(location.key(), "correction");
    assert_eq!(location.pointer().require().unwrap(), "/edits/4");
    let span = location.span().unwrap();
    assert!(plan[span.start as usize..span.end as usize].contains("\"operation\":\"remove\""));
    assert_eq!(
        property
            .get("x:opaque")
            .require()
            .unwrap()
            .at(0)
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "0.10"
    );
    assert_eq!(
        root.get("sdfProperty")
            .require()
            .unwrap()
            .get("sibling")
            .require()
            .unwrap()
            .get("sdfRef")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "#/sdfProperty/space ~1~0#é"
    );
}

#[test]
fn textual_and_typed_intent_select_the_exact_supplied_key() {
    let inputs = || {
        Catalogue::from_inputs(&[
            SourceInput::supplied(
                "target /~#é",
                br#"{"sdfData":{"a":{"minimum":0,"default":true}}}"#,
            )
            .label("decoy"),
            SourceInput::supplied(
                "decoy",
                br#"{"sdfData":{"a":{"minimum":9,"default":false}}}"#,
            )
            .label("target /~#é"),
        ])
        .unwrap()
    };
    let plan = r#"{"key":"same","source":"target /~#é","generated_key":"revised","edits":[{"owner":"/sdfData/a","quality":"minimum","operation":"replace","value":-0.00},{"owner":"/sdfData/a","quality":"default","operation":"remove"}]}"#;
    let textual = EditRequest::from_plan(inputs(), plan.as_bytes())
        .unwrap()
        .edit()
        .unwrap();
    let typed_catalogue = inputs();
    let original = typed_catalogue.source_id("target /~#é").unwrap();
    let typed = EditRequest::new(typed_catalogue, "same", original, "revised")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfData/a",
                "minimum",
                JsonBuilder::new().number("-0.00").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_edit(Edit::remove("/sdfData/a", "default").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(textual.generated_document(), typed.generated_document());
    assert_eq!(
        textual.outcome().disposition(),
        typed.outcome().disposition()
    );
    assert_eq!(textual.original_source().key(), "target /~#é");
    assert_eq!(
        textual
            .catalogue()
            .source(textual.catalogue().source_id("decoy").unwrap())
            .unwrap()
            .raw_bytes(),
        br#"{"sdfData":{"a":{"minimum":9,"default":false}}}"#
    );
    assert!(
        typed
            .target(0)
            .require()
            .unwrap()
            .request_location()
            .span()
            .is_none()
    );
}

#[test]
fn malformed_or_stale_plans_never_generate_a_document() {
    let source = br#"{"sdfProperty":{"a":{"description":"old","default":3}}}"#;
    let row =
        r#"{"owner":"/sdfProperty/a","quality":"description","operation":"replace","value":"new"}"#;
    let valid =
        format!(r#"{{"key":"request","source":"base","generated_key":"revised","edits":[{row}]}}"#);
    let bad = [
        valid.replace(
            "\"key\":\"request\"",
            "\"key\":\"request\",\"unexpected\":0",
        ),
        valid.replace(
            "\"key\":\"request\"",
            "\"key\":\"request\",\"k\\u0065y\":\"duplicate\"",
        ),
        valid.replace("\"description\"", "\"d\\u0065scription\""),
        valid.replace(row, &format!("{row},{row}")),
        valid.replace(
            "\"operation\":\"replace\",\"value\":\"new\"",
            "\"operation\":\"remove\",\"value\":null",
        ),
        valid.replace(
            "\"operation\":\"replace\",\"value\":\"new\"",
            "\"operation\":\"replace\"",
        ),
        valid.replace("\"operation\":\"replace\"", "\"operation\":\"insert\""),
        valid.replace("\"owner\":", "\"extra\":false,\"owner\":"),
        valid.replace("\"owner\":\"/sdfProperty/a\"", "\"owner\":7"),
        valid.replace("\"/sdfProperty/a\"", "\"/sdfProperty/missing\""),
        valid.replace("\"description\"", "\"absent\""),
        valid.replace("\"source\":\"base\"", "\"source\":\"missing\""),
        valid.replace(
            "\"generated_key\":\"revised\"",
            "\"generated_key\":\"base\"",
        ),
        valid.replace(&format!("[{row}]"), "[]"),
    ];
    for (index, plan) in bad.iter().enumerate() {
        if index == 2 {
            // A decoded spelling selects exactly the same quality and is valid.
            assert!(
                EditRequest::from_plan(catalogue(source), plan.as_bytes())
                    .unwrap()
                    .edit()
                    .unwrap()
                    .document_deliverable()
            );
            continue;
        }
        let result =
            EditRequest::from_plan(catalogue(source), plan.as_bytes()).and_then(EditRequest::edit);
        assert!(result.is_err(), "case {index}: {plan}");
    }
    let duplicate = valid.replace(
        row,
        &format!("{row},{}", row.replace("description", "d\\u0065scription")),
    );
    assert!(EditRequest::from_plan(catalogue(source), duplicate.as_bytes()).is_err());
    assert!(EditRequest::from_plan(catalogue(source), b"{oops").is_err());
}

#[test]
fn finite_plan_limits_separate_failed_admission_from_unfinished_operations() {
    let source = br#"{"sdfProperty":{"a":{"maximum":5}}}"#;
    let plan = br#"{"key":"request","source":"base","generated_key":"revised","edits":[{"owner":"/sdfProperty/a","quality":"maximum","operation":"replace","value":0.10}]}"#;
    let limited =
        |limits| Catalogue::with_limits(&[SourceInput::supplied("base", source)], limits).unwrap();
    let hard = EditRequest::from_plan(limited(esdieff::Limits::default().parsed_nodes(7)), plan);
    assert!(
        matches!(hard, Err(reason) if reason.contains("decoding limit") && !reason.contains("syntax"))
    );
    let result = EditRequest::from_plan(limited(esdieff::Limits::default().numeric_work(1)), plan)
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(result.request().edits().len(), 1);
    assert!(!result.outcome().complete());
    assert_eq!(result.fidelity(), Fidelity::Undetermined);
    assert!(!result.document_deliverable());
    assert_eq!(result.original_source().raw_bytes(), source);
    assert_eq!(
        result.request().plan_text(),
        Some(std::str::from_utf8(plan).unwrap())
    );
    let json: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(json["request"]["editing"]["edit_count"], 1);
    assert_eq!(json["result"]["substitution"]["original_source"], 1);
    assert_eq!(json["result"]["substitution"]["generated_source"], 2);
}

#[test]
fn plan_copy_and_generated_decoding_share_one_node_allowance() {
    let source = br#"{"sdfProperty":{"a":{"maximum":5}}}"#;
    let plan = br#"{"key":"request","source":"base","generated_key":"revised","edits":[{"owner":"/sdfProperty/a","quality":"maximum","operation":"replace","value":0.10}]}"#;
    let run = |nodes| {
        let inputs = Catalogue::with_limits(
            &[SourceInput::supplied("base", source)],
            esdieff::Limits::default().parsed_nodes(nodes),
        )
        .unwrap();
        EditRequest::from_plan(inputs, plan)
            .unwrap()
            .edit()
            .unwrap()
    };
    // Four source nodes, ten plan nodes, one owned value and four generated nodes.
    assert!(run(19).document_deliverable());
    let exhausted = run(18);
    assert!(!exhausted.outcome().complete());
    assert!(!exhausted.document_deliverable());
    assert_eq!(exhausted.fidelity(), Fidelity::Undetermined);
    assert!(exhausted.generated_document().is_some());
}

#[test]
fn plans_obey_request_limits_and_retained_source_key_collisions() {
    let source = br#"{"sdfProperty":{"a":{"description":"old"}}}"#;
    let plan = br#"{"key":"request","source":"base","generated_key":"revised","edits":[{"owner":"/sdfProperty/a","quality":"description","operation":"replace","value":"new"}]}"#;
    assert!(
        matches!(EditRequest::from_plan(catalogue(source), &vec![b' '; 4 * 1024 * 1024 + 1]), Err(reason) if reason.contains("byte ceiling"))
    );
    let inputs = Catalogue::with_limits(
        &[SourceInput::supplied("base", source)],
        esdieff::Limits::default().document_bytes(120),
    )
    .unwrap();
    assert!(
        matches!(EditRequest::from_plan(inputs, plan), Err(reason) if reason.contains("byte ceiling"))
    );
    let inputs = Catalogue::with_limits(
        &[SourceInput::supplied("base", source)],
        esdieff::Limits::default().documents(1),
    )
    .unwrap();
    assert!(
        matches!(EditRequest::from_plan(inputs, plan), Err(reason) if reason.contains("source-attempt"))
    );
    let inputs = Catalogue::with_limits(
        &[SourceInput::supplied("base", source)],
        esdieff::Limits::default().report_bytes(32768),
    )
    .unwrap();
    assert!(
        matches!(EditRequest::from_plan(inputs, plan), Err(reason) if reason.contains("reserve"))
    );
    let inputs = Catalogue::with_limits(
        &[SourceInput::supplied("base", source)],
        esdieff::Limits::default().pointer_bytes(10),
    )
    .unwrap();
    assert!(EditRequest::from_plan(inputs, plan).is_err());
    for field in ["key", "generated_key"] {
        let changed = std::str::from_utf8(plan).unwrap().replace(
            &format!(
                "\"{field}\":\"{}\"",
                if field == "key" { "request" } else { "revised" }
            ),
            &format!("\"{field}\":\"{}\"", "x".repeat(257)),
        );
        assert!(EditRequest::from_plan(catalogue(source), changed.as_bytes()).is_err());
    }
    let edited = EditRequest::from_plan(catalogue(source), plan)
        .unwrap()
        .edit()
        .unwrap()
        .into_catalogue();
    let collision = std::str::from_utf8(plan)
        .unwrap()
        .replace("\"source\":\"base\"", "\"source\":\"revised\"")
        .replace(
            "\"generated_key\":\"revised\"",
            "\"generated_key\":\"base\"",
        );
    assert!(
        matches!(EditRequest::from_plan(edited, collision.as_bytes()), Err(reason) if reason.contains("new"))
    );
}
