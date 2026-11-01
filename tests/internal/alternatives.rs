//! Effective alternatives retain constraints and keep nullability separate from presence.
//! Follows ADR 0005: Nullability and presence, and ADR 0020: Owned effective views and nearest origins.
use esdieff::{Catalogue, Class, EffectiveRequest, Field, Lookup, OriginKind, SourceInput};

#[test]
fn specified_nullable_does_not_insert_json_or_remove_const_and_required_syntax() {
    let text = br#"{"info":{},"sdfData":{"mode":{"type":"string","const":"on","default":"off","required":["entry"],"sdfRequired":["declared"]}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let Lookup::Found(source) = catalogue
        .source(id)
        .unwrap()
        .definition(Class::Data, "mode")
    else {
        panic!("source mode")
    };
    let source = source.data_description().require().unwrap();
    assert!(matches!(source.nullable(), Field::Absent));
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "mode") else {
        panic!("effective mode")
    };
    let data = mode.data_description().require().unwrap();
    assert!(data.nullable().require().unwrap());
    assert_eq!(
        data.constant().require().unwrap().text().require().unwrap(),
        "on"
    );
    assert_eq!(
        data.declared_default()
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "off"
    );
    assert_eq!(
        data.required_entries()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "entry"
    );
    assert_eq!(
        mode.declaration_requiredness()
            .require()
            .unwrap()
            .at(0)
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "declared"
    );
    assert!(matches!(data.json().get("nullable"), Field::Absent));
    let default = model
        .default_explanation("/sdfData/mode/nullable")
        .require()
        .unwrap();
    assert_eq!(default.kind(), OriginKind::Default);
    assert_eq!(default.sources().count(), 0);
    assert_eq!(catalogue.source(id).unwrap().raw_bytes(), text);
}

#[test]
fn reference_deletions_keep_documentary_and_specified_defaults_distinct() {
    let text = br##"{"info":{},"sdfData":{"Base":{"type":"string","const":"on","default":"off","nullable":false}},"sdfProperty":{"Mode":{"sdfRef":"#/sdfData/Base","default":null,"nullable":null}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(mode) = model.definition(Class::Property, "Mode") else {
        panic!("effective mode")
    };
    let data = mode.data_description().require().unwrap();
    assert!(matches!(data.declared_default(), Field::Absent));
    assert!(data.nullable().require().unwrap());
    assert_eq!(
        data.constant().require().unwrap().text().require().unwrap(),
        "on"
    );
    assert_eq!(
        model
            .explanation("/sdfProperty/Mode/default")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Deleted
    );
    assert_eq!(
        model
            .explanation("/sdfProperty/Mode/nullable")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Deleted
    );
    assert_eq!(
        model
            .default_explanation("/sdfProperty/Mode/nullable")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Default
    );
    assert_eq!(catalogue.source(id).unwrap().raw_bytes(), text);
}

#[test]
fn named_alternative_inherits_shared_qualities_and_replaces_whole_map_override() {
    use esdieff::{AlternativeId, BaseType, DataType};
    let text = br#"{"info":{},"sdfData":{"Mode":{"type":"object","nullable":false,"minimum":10.0,"default":"shared","properties":{"old":{"type":"string"}},"sdfChoice":{"fresh":{"properties":{"new":{"type":"integer"}},"const":null}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("mode")
    };
    let mut alternatives = mode
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap();
    let (identity, branch) = alternatives.next().unwrap();
    assert_eq!(identity, AlternativeId::Choice("fresh"));
    let branch = branch.require().unwrap();
    assert!(matches!(
        branch.data_type(),
        Field::Present(DataType::Known(BaseType::Map))
    ));
    assert_eq!(branch.minimum().require().unwrap().lexeme(), "10.0");
    assert!(!branch.nullable().require().unwrap());
    assert_eq!(
        branch.constant().require().unwrap().kind(),
        esdieff::JsonKind::Null
    );
    assert_eq!(
        branch
            .declared_default()
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "shared"
    );
    let mut properties = branch.properties().require().unwrap();
    let (name, property) = properties.next().unwrap();
    assert_eq!(name, "new");
    assert!(matches!(
        property.require().unwrap().data_type(),
        Field::Present(DataType::Known(BaseType::Integer))
    ));
    assert!(properties.next().is_none());
    assert!(alternatives.next().is_none());
    assert_eq!(catalogue.source(id).unwrap().raw_bytes(), text);
}

#[test]
fn inherited_restricted_qualities_remain_unknown_until_full_branch_overrides_them() {
    use esdieff::{AlternativeId, BaseType, DataType};
    let text = br#"{"info":{},"sdfData":{"Rows":{"type":"array","items":{"type":"array","nullable":false,"default":"outer","minItems":1,"minimum":1,"maximum":2,"sdfChoice":{"unknown":{},"established":{"type":"string","nullable":true,"default":"branch","minItems":2}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(rows) = model.definition(Class::Data, "Rows") else {
        panic!("Rows")
    };
    let items = rows
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    let alternatives = items.alternatives().require().unwrap();
    for (name, branch) in alternatives {
        let branch = branch.require().unwrap();
        assert_eq!(branch.minimum().require().unwrap().lexeme(), "1");
        assert_eq!(branch.maximum().require().unwrap().lexeme(), "2");
        match name {
            AlternativeId::Choice("unknown") => {
                assert!(matches!(branch.data_type(), Field::Unavailable(_)));
                assert!(matches!(branch.nullable(), Field::Unavailable(_)));
                assert!(matches!(branch.declared_default(), Field::Unavailable(_)));
                assert!(matches!(branch.minimum_items(), Field::Unavailable(_)));
                assert!(
                    !branch
                        .json()
                        .get("nullable")
                        .require()
                        .unwrap()
                        .boolean()
                        .require()
                        .unwrap()
                );
                assert!(!branch.json().capability_complete());
            }
            AlternativeId::Choice("established") => {
                assert!(matches!(
                    branch.data_type(),
                    Field::Present(DataType::Known(BaseType::String))
                ));
                assert!(branch.nullable().require().unwrap());
                assert_eq!(branch.minimum_items().require().unwrap().lexeme(), "2");
                assert_eq!(
                    branch
                        .declared_default()
                        .require()
                        .unwrap()
                        .text()
                        .require()
                        .unwrap(),
                    "branch"
                );
                assert!(branch.json().capability_complete());
            }
            _ => panic!("known names"),
        }
    }
    assert!(!model.complete());
}

#[test]
fn typed_nullable_and_direct_origin_handles_explain_final_alternative_qualities() {
    use esdieff::{AlternativeId, Characteristic};
    let text = br#"{"info":{},"sdfData":{"Mode":{"minimum":1,"nullable":false,"sdfChoice":{"inherited":{},"override":{"minimum":2,"nullable":true},"empty":{}}},"Rows":{"items":{"sdfChoice":{"full":{}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let Lookup::Found(source) = catalogue
        .source(id)
        .unwrap()
        .definition(Class::Data, "Mode")
    else {
        panic!("source Mode")
    };
    assert_eq!(
        source
            .data_description()
            .require()
            .unwrap()
            .nullable_characteristic()
            .require()
            .unwrap(),
        Characteristic::Declared(false)
    );
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("Mode")
    };
    for (identity, branch) in mode
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
    {
        let branch = branch.require().unwrap();
        let minimum = branch
            .json()
            .quality_explanation("minimum")
            .require()
            .unwrap();
        let pointer = match identity {
            AlternativeId::Choice("inherited") => "/sdfData/Mode/sdfChoice/inherited/minimum",
            AlternativeId::Choice("override") => "/sdfData/Mode/sdfChoice/override/minimum",
            AlternativeId::Choice("empty") => "/sdfData/Mode/sdfChoice/empty/minimum",
            _ => panic!("known choice"),
        };
        assert_eq!(
            minimum.effective().pointer(8192).require().unwrap(),
            pointer
        );
        match identity {
            AlternativeId::Choice("override") => {
                assert_eq!(
                    branch.nullable_characteristic().require().unwrap(),
                    Characteristic::Declared(true)
                );
                assert_eq!(minimum.kind(), OriginKind::Override);
                assert_eq!(minimum.sources().count(), 2);
            }
            AlternativeId::Choice(_) => {
                assert_eq!(
                    branch.nullable_characteristic().require().unwrap(),
                    Characteristic::Declared(false)
                );
                assert_eq!(minimum.kind(), OriginKind::Inherited);
            }
            _ => panic!("choice"),
        }
    }
    let Lookup::Found(rows) = model.definition(Class::Data, "Rows") else {
        panic!("Rows")
    };
    let items = rows
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(
        items.nullable_characteristic(),
        Field::Unavailable(_)
    ));
    let branch = items
        .alternatives()
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert_eq!(
        branch.nullable_characteristic().require().unwrap(),
        Characteristic::SpecifiedDefault(true)
    );
    let default = branch
        .json()
        .default_explanation("nullable")
        .require()
        .unwrap();
    assert_eq!(default.kind(), OriginKind::Default);
    assert_eq!(default.sources().count(), 0);
    assert_eq!(
        default.effective().pointer(8192).require().unwrap(),
        "/sdfData/Rows/items/sdfChoice/full/nullable"
    );
}

#[test]
fn shared_object_type_establishes_required_shape_check_in_the_alternative_context() {
    use esdieff::CheckRequest;
    let text = br#"{"info":{},"sdfData":{"Mode":{"type":"object","sdfChoice":{"branch":{"required":[]}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let required = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["code"] == "data.required")
        .collect::<Vec<_>>();
    assert_eq!(required.len(), 1);
    assert_eq!(required[0]["primary"]["kind"], "effective");
    assert_eq!(
        required[0]["primary"]["pointer"],
        "/sdfData/Mode/sdfChoice/branch/required"
    );
    assert_eq!(report.outcome().errors(), 1);
}

#[test]
fn branch_reference_tombstones_allow_shared_fallback_with_separate_origins() {
    let text = br##"{"info":{},"sdfData":{"Base":{"nullable":true,"properties":{"deleted":{"type":"string"}},"default":"base"},"Mode":{"type":"object","nullable":false,"properties":{"shared":{"type":"integer"}},"default":"shared","sdfChoice":{"branch":{"sdfRef":"#/sdfData/Base","nullable":null,"properties":null,"default":null},"literal":{"const":null}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("Mode")
    };
    for (_, data) in mode
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
    {
        let data = data.require().unwrap();
        assert!(!data.nullable().require().unwrap());
        assert_eq!(
            data.properties().require().unwrap().next().unwrap().0,
            "shared"
        );
        assert_eq!(
            data.declared_default()
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap(),
            "shared"
        );
        assert_eq!(
            data.json()
                .quality_explanation("nullable")
                .require()
                .unwrap()
                .kind(),
            OriginKind::Inherited
        );
    }
    for quality in ["nullable", "properties", "default"] {
        assert_eq!(
            model
                .explanation(&format!("/sdfData/Mode/sdfChoice/branch/{quality}"))
                .require()
                .unwrap()
                .kind(),
            OriginKind::Deleted
        );
    }
    assert!(matches!(
        model
            .json()
            .require()
            .unwrap()
            .get("sdfData")
            .require()
            .unwrap()
            .get("Mode")
            .require()
            .unwrap()
            .get("sdfChoice")
            .require()
            .unwrap()
            .get("branch")
            .require()
            .unwrap()
            .get("nullable"),
        Field::Absent
    ));
}

#[test]
fn nested_alternatives_derive_after_final_inheritance_and_whole_quality_overrides() {
    let text = br#"{"info":{},"sdfData":{"Mode":{"type":"object","nullable":false,"minimum":1.0,"properties":{"shared":{"type":"string"}},"sdfChoice":{"outer":{"minimum":2e0,"properties":{"replaced":{"type":"integer"}},"sdfChoice":{"inner":{},"explicit":{"nullable":true}}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("Mode")
    };
    let outer = mode
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    for (id, inner) in outer.alternatives().require().unwrap() {
        let inner = inner.require().unwrap();
        assert_eq!(inner.minimum().require().unwrap().lexeme(), "2e0");
        assert_eq!(
            inner.properties().require().unwrap().next().unwrap().0,
            "replaced"
        );
        assert_eq!(
            inner.nullable().require().unwrap(),
            id == esdieff::AlternativeId::Choice("explicit")
        );
        assert!(inner.json().capability_complete());
    }
}

#[test]
fn enum_occurrences_preserve_exact_scalar_source_identity_and_duplicate_order() {
    use esdieff::AlternativeId;
    let text = br#"{"info":{},"sdfData":{"Mode":{"enum":["vendor:value","\u0061","a","\uD834\uDD1E"],"minimum":10e1000000}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let source_id = catalogue.source_id("model").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(source_id))
        .unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("Mode")
    };
    for ((identity, data), (index, expected)) in mode
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
        .zip(["vendor:value", "a", "a", "𝄞"].into_iter().enumerate())
    {
        let data = data.require().unwrap();
        assert_eq!(identity, AlternativeId::Enum(index));
        assert_eq!(
            data.constant().require().unwrap().text().require().unwrap(),
            expected
        );
        assert_eq!(data.minimum().require().unwrap().lexeme(), "10e1000000");
        let origin = data.json().quality_explanation("const").require().unwrap();
        let source = origin.sources().next().unwrap();
        assert_eq!(source.source_id(), source_id);
        assert_eq!(
            source.pointer(8192).require().unwrap(),
            format!("/sdfData/Mode/enum/{index}")
        );
        assert_eq!(
            origin.effective().pointer(8192).require().unwrap(),
            format!("/sdfData/Mode/enum/{index}/const")
        );
    }
    assert!(model.complete());
}

#[test]
fn collection_absence_empty_unavailable_and_known_unavailable_branches_are_distinct() {
    let text = br##"{"info":{},"sdfData":{"Absent":{},"Empty":{"sdfChoice":{}},"Bad":{"sdfChoice":[]},"Enum":{"enum":["known","\uD800"]},"Branches":{"minimum":1,"sdfChoice":{"bad":false,"missing":{"sdfRef":"#/sdfData/No"},"known":{}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    for (name, state) in [
        ("Absent", "absent"),
        ("Empty", "empty"),
        ("Bad", "unknown"),
        ("Enum", "unknown"),
    ] {
        let Lookup::Found(data) = model.definition(Class::Data, name) else {
            panic!("known definition")
        };
        match (
            state,
            data.data_description().require().unwrap().alternatives(),
        ) {
            ("absent", Field::Absent) | ("unknown", Field::Unavailable(_)) => {}
            ("empty", Field::Present(rows)) => assert_eq!(rows.count(), 0),
            _ => panic!("{name}: distinct collection state"),
        }
    }
    let Lookup::Found(branches) = model.definition(Class::Data, "Branches") else {
        panic!("Branches")
    };
    let rows = branches
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap();
    assert_eq!(rows.len(), 3);
    for (name, branch) in rows {
        match name {
            esdieff::AlternativeId::Choice("bad" | "missing") => {
                assert!(matches!(branch, Field::Unavailable(_)))
            }
            esdieff::AlternativeId::Choice("known") => assert_eq!(
                branch
                    .require()
                    .unwrap()
                    .minimum()
                    .require()
                    .unwrap()
                    .lexeme(),
                "1"
            ),
            _ => panic!("known branch names"),
        }
    }
    assert!(!model.complete());
}

#[test]
fn all_full_positions_expose_alternatives_and_specified_nullable_without_property_flags() {
    use esdieff::Characteristic;
    let text = br#"{"info":{},"sdfProperty":{"p":{"readable":false,"writable":false,"observable":true,"enum":["one"]}},"sdfAction":{"a":{"sdfInputData":{"enum":["one"]},"sdfOutputData":{"enum":["one"]}}},"sdfEvent":{"e":{"sdfOutputData":{"enum":["one"]}}},"sdfData":{"d":{"enum":["one"]},"object":{"properties":{"field":{"enum":["one"]}}},"array":{"items":{"sdfChoice":{"full":{"enum":["one"]}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let source_id = catalogue.source_id("model").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(source_id))
        .unwrap();
    let definition = |class, name| {
        let Lookup::Found(value) = model.definition(class, name) else {
            panic!("definition")
        };
        value
    };
    let positions = [
        definition(Class::Property, "p")
            .data_description()
            .require()
            .unwrap(),
        definition(Class::Action, "a").input().require().unwrap(),
        definition(Class::Action, "a").output().require().unwrap(),
        definition(Class::Event, "e").output().require().unwrap(),
        definition(Class::Data, "d")
            .data_description()
            .require()
            .unwrap(),
        definition(Class::Data, "object")
            .data_description()
            .require()
            .unwrap()
            .properties()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .require()
            .unwrap(),
        definition(Class::Data, "array")
            .data_description()
            .require()
            .unwrap()
            .items()
            .require()
            .unwrap()
            .alternatives()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .require()
            .unwrap(),
    ];
    for data in positions {
        let branch = data
            .alternatives()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .require()
            .unwrap();
        assert_eq!(
            branch.nullable_characteristic().require().unwrap(),
            Characteristic::SpecifiedDefault(true)
        );
        for flag in ["readable", "writable", "observable"] {
            assert!(matches!(branch.json().get(flag), Field::Absent));
        }
    }
}

#[test]
fn alternative_amplification_limits_preserve_source_and_independent_known_views() {
    use esdieff::{CheckRequest, Limits};
    let mut text = String::from(
        "{\"info\":{},\"sdfProperty\":{\"Bad\":{\"multipleOf\":0}},\"sdfData\":{\"Known\":{\"minimum\":7},\"Many\":{\"properties\":{",
    );
    for index in 0..25 {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&format!("\"p{index}\":{{\"minimum\":{index}}}"));
    }
    text.push_str("},\"enum\":[\"a\",\"b\",\"c\",\"d\",\"e\",\"f\"]}}}");
    for limits in [
        Limits::default().effective_nodes(220),
        Limits::default().effective_allocation_bytes(50_000),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", text.as_bytes())], limits)
                .unwrap();
        let source_id = catalogue.source_id("model").unwrap();
        let model = catalogue
            .effective(EffectiveRequest::new(source_id))
            .unwrap();
        assert!(!model.complete());
        let Lookup::Found(known) = model.definition(Class::Data, "Known") else {
            panic!("Known retained")
        };
        assert_eq!(
            known
                .data_description()
                .require()
                .unwrap()
                .minimum()
                .require()
                .unwrap()
                .lexeme(),
            "7"
        );
        assert_eq!(
            catalogue.source(source_id).unwrap().raw_bytes(),
            text.as_bytes()
        );
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert!(report.outcome().errors() >= 1);
        assert!(!report.outcome().complete());
        assert!(report.to_json().contains("parameter.positive_step"));
    }
}

#[test]
fn direct_object_donors_keep_established_structure_and_opaque_optional_independence() {
    let text = br#"{"info":{},"sdfData":{"Rows":{"items":{"type":"object","minimum":1,"required":["entry"],"properties":{"entry":{"type":"string"}},"vendor:opaque":{"enum":[],"nullable":7},"sdfChoice":{"branch":{}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let Lookup::Found(rows) = model.definition(Class::Data, "Rows") else {
        panic!("Rows")
    };
    let branch = rows
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert_eq!(
        branch
            .required_entries()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "entry"
    );
    assert_eq!(
        branch.properties().require().unwrap().next().unwrap().0,
        "entry"
    );
    assert_eq!(
        branch
            .json()
            .get("vendor:opaque")
            .require()
            .unwrap()
            .get("nullable")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "7"
    );
    assert!(branch.json().capability_complete());
    assert!(model.complete());
}

#[test]
fn needed_scalar_failure_and_unknown_choice_identity_preserve_independent_branches() {
    let text = br#"{"info":{},"sdfData":{"Mode":{"minimum":1,"sdfChoice":{"partial":{"maximum":1e1000001},"known":{"maximum":2}}},"Unknown":{"sdfChoice":{"\uD800":{},"name":{}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("Mode")
    };
    for (id, branch) in mode
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
    {
        let branch = branch.require().unwrap();
        assert_eq!(branch.minimum().require().unwrap().lexeme(), "1");
        if id == esdieff::AlternativeId::Choice("partial") {
            assert!(matches!(branch.maximum(), Field::Unavailable(_)));
            assert!(!branch.json().capability_complete());
        } else {
            assert_eq!(branch.maximum().require().unwrap().lexeme(), "2");
            assert!(branch.json().capability_complete());
        }
    }
    let Lookup::Found(unknown) = model.definition(Class::Data, "Unknown") else {
        panic!("Unknown")
    };
    assert!(matches!(
        unknown.data_description().require().unwrap().alternatives(),
        Field::Unavailable(_)
    ));
}
