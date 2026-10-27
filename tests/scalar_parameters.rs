//! Scalar parameter checks and typed access through supplied model operations.
//! Follows ADR 0012: Exact numbers and JSON input policies, and
//! ADR 0020: Owned effective views and nearest origins.
mod coverage_support;
use esdieff::{BasisKind, Catalogue, CheckRequest, Class, Field, Lookup, Severity, SourceInput};

#[test]
fn ordinary_property_zero_step_is_a_definite_parameter_problem() {
    let text = br#"{"info":{},"sdfProperty":{"level":{"type":"number","multipleOf":0}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    coverage_support::assert_requested_counts(&report);
    let diagnostics = report.diagnostics();
    let condition = diagnostics.conditions().next().unwrap();
    assert_eq!(condition.code(), "parameter.positive_step");
    assert_eq!(condition.severity(), Severity::Error);
    assert_eq!(condition.basis().kind(), BasisKind::Rfc);
    assert_eq!(
        catalogue
            .source(catalogue.source_id("model").unwrap())
            .unwrap()
            .raw_bytes(),
        text
    );
}

#[test]
fn full_numeric_parameters_require_numbers_without_boolean_exclusive_bounds() {
    for key in [
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
    ] {
        let text = format!(r#"{{"info":{{}},"sdfData":{{"value":{{"{key}":true}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1, "{key}");
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "data.number")
        );
    }
}

#[test]
fn unavailable_exact_numeric_capability_is_not_a_parameter_violation() {
    let text =
        br#"{"info":{},"sdfData":{"large":{"multipleOf":1e1000001},"bad":{"multipleOf":-0.0}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let diagnostics = report.diagnostics();
    let unavailable = diagnostics
        .conditions()
        .find(|condition| condition.code() == "data.number_unavailable")
        .unwrap();
    assert_eq!(unavailable.severity(), Severity::Incomplete);
    assert_eq!(unavailable.basis().kind(), BasisKind::Operational);
    assert!(
        diagnostics
            .conditions()
            .any(|condition| condition.code() == "parameter.positive_step")
    );
}

#[test]
fn full_lengths_and_counts_use_nonnegative_mathematical_integrality() {
    for key in ["minLength", "maxLength", "minItems", "maxItems"] {
        for (value, errors) in [
            ("10.0", 0),
            ("1e2", 0),
            ("-0.0", 0),
            ("10e1000000", 0),
            ("0.1", 1),
            ("-1", 1),
            ("true", 1),
            ("null", 1),
        ] {
            let text = format!(r#"{{"info":{{}},"sdfData":{{"value":{{"{key}":{value}}}}}}}"#);
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(report.outcome().errors(), errors, "{key}: {value}");
            if errors > 0 {
                assert!(
                    report
                        .diagnostics()
                        .conditions()
                        .any(|condition| condition.code() == "data.count")
                );
            }
        }
    }
}

#[test]
fn full_text_and_boolean_parameters_follow_their_vocabulary_roles() {
    for (group, key, code) in [
        ("sdfData", "type", "data.string"),
        ("sdfData", "sdfType", "data.string"),
        ("sdfData", "pattern", "data.string"),
        ("sdfData", "format", "data.string"),
        ("sdfData", "unit", "data.string"),
        ("sdfData", "contentFormat", "data.string"),
        ("sdfData", "nullable", "data.boolean"),
        ("sdfData", "uniqueItems", "data.boolean"),
        ("sdfProperty", "readable", "data.boolean"),
        ("sdfProperty", "writable", "data.boolean"),
        ("sdfProperty", "observable", "data.boolean"),
    ] {
        let text = format!(r#"{{"info":{{}},"{group}":{{"value":{{"{key}":[]}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1, "{group}: {key}");
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == code)
        );
    }
    let text = br#"{"info":{},"sdfThing":{"group":{"nullable":[],"minimum":true}},"sdfData":{"value":{"type":"extension-value","format":"future-format","sdfType":"future-semantic","readable":[]}},"sdfProperty":{"value":{"nullable":true,"uniqueItems":false,"readable":false,"writable":true,"observable":false}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    assert_eq!(
        catalogue
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .errors(),
        0
    );
}

#[test]
fn full_enum_requires_nonempty_string_array_without_rejecting_unavailable_text_as_nonstrings() {
    for (value, errors) in [
        ("[]", 1),
        ("true", 1),
        (r#"["a",2]"#, 1),
        ("[null]", 1),
        (r#"["a"]"#, 0),
        (r#"["\uD800"]"#, 0),
    ] {
        let text = format!(r#"{{"info":{{}},"sdfData":{{"value":{{"enum":{value}}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), errors, "{value}");
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .filter(|condition| condition.code() == "data.enum")
                .count(),
            errors as usize
        );
    }
}

#[test]
fn full_map_qualities_and_named_full_descriptions_require_maps() {
    for body in [
        r#""items":false"#,
        r#""sdfChoice":[]"#,
        r#""type":"object","properties":[]"#,
        r#""sdfChoice":{"choice":false}"#,
        r#""type":"object","properties":{"field":false}"#,
    ] {
        let text = format!(r#"{{"info":{{}},"sdfData":{{"value":{{{body}}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1, "{body}");
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "data.map_shape")
        );
    }
}

#[test]
fn enum_choice_conflicts_use_surviving_presence_independently_of_value_shape() {
    for (body, errors) in [
        (r#""enum":["x"],"sdfChoice":{}"#, 1),
        (r#""enum":null,"sdfChoice":{}"#, 2),
        (r#""enum":["x"],"sdfChoice":null"#, 2),
        (r#""enum":null,"sdfChoice":null"#, 3),
    ] {
        let text = format!(r#"{{"info":{{}},"sdfData":{{"value":{{{body}}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), errors, "{body}");
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .filter(|condition| condition.code() == "data.enum_choice")
                .count(),
            1
        );
    }
    let text = br##"{"info":{},"sdfData":{"Base":{"enum":["x"]},"Conflict":{"sdfRef":"#/sdfData/Base","sdfChoice":{}},"Deleted":{"sdfRef":"#/sdfData/Base","enum":null,"sdfChoice":{}},"Choice":{"sdfChoice":{}},"RemovedChoice":{"sdfRef":"#/sdfData/Choice","sdfChoice":null,"enum":["y"]}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let conflict = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|condition| condition["code"] == "data.enum_choice")
        .unwrap();
    assert_eq!(conflict["primary"]["kind"], "effective");
    assert_eq!(conflict["primary"]["pointer"], "/sdfData/Conflict/enum");
    assert_eq!(conflict["related"][0]["pointer"], "/sdfData/Base/enum");
}

#[test]
fn typed_enum_rejects_empty_and_unavailable_text_without_losing_raw_values() {
    let text = br#"{"info":{},"sdfData":{"empty":{"enum":[]},"unknown":{"enum":["\uD800"]},"known":{"enum":["x"]}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let source = catalogue
        .source(catalogue.source_id("model").unwrap())
        .unwrap();
    for name in ["empty", "unknown"] {
        let Lookup::Found(definition) = source.definition(Class::Data, name) else {
            panic!("source data")
        };
        let data = definition.data_description().require().unwrap();
        assert!(
            matches!(data.source_enum(), Field::Unavailable(_)),
            "{name}"
        );
        assert!(matches!(data.json().get("enum"), Field::Present(_)));
    }
    let Lookup::Found(definition) = source.definition(Class::Data, "known") else {
        panic!("source data")
    };
    assert_eq!(
        definition
            .data_description()
            .require()
            .unwrap()
            .source_enum()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "x"
    );
}

#[test]
fn effective_scalar_access_borrows_exact_constraints_and_retains_documentary_values() {
    use esdieff::EffectiveRequest;
    let text = br##"{"info":{},"sdfData":{"Base":{"type":"number","minimum":-0.1,"maximum":10e1000000,"exclusiveMinimum":-1e-1000000,"exclusiveMaximum":20,"multipleOf":0.01,"minLength":10.0,"maxLength":1e2,"minItems":-0.0,"maxItems":10e1000000,"uniqueItems":false,"pattern":"[x]","format":"future-format","sdfType":"future-semantic","unit":"Cel","contentFormat":"50","const":0.1,"default":{"exact":10e1000000}}},"sdfProperty":{"value":{"sdfRef":"#/sdfData/Base","maximum":100,"multipleOf":null},"bad":{"multipleOf":0,"minItems":0.1}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(property) = model.definition(Class::Property, "value") else {
        panic!("effective Property")
    };
    let data = property.data_description().require().unwrap();
    assert_eq!(data.minimum().require().unwrap().lexeme(), "-0.1");
    assert_eq!(data.maximum().require().unwrap().lexeme(), "100");
    assert_eq!(
        data.exclusive_minimum().require().unwrap().lexeme(),
        "-1e-1000000"
    );
    assert_eq!(data.exclusive_maximum().require().unwrap().lexeme(), "20");
    assert!(matches!(data.multiple_of(), Field::Absent));
    assert_eq!(data.minimum_length().require().unwrap().lexeme(), "10.0");
    assert_eq!(data.maximum_length().require().unwrap().lexeme(), "1e2");
    assert_eq!(data.minimum_items().require().unwrap().lexeme(), "-0.0");
    assert_eq!(
        data.maximum_items().require().unwrap().lexeme(),
        "10e1000000"
    );
    assert!(!data.unique_items().require().unwrap());
    assert_eq!(data.pattern().require().unwrap(), "[x]");
    assert_eq!(data.format().require().unwrap(), "future-format");
    assert_eq!(data.sdf_type().require().unwrap(), "future-semantic");
    assert_eq!(data.unit().require().unwrap(), "Cel");
    assert_eq!(data.content_format().require().unwrap(), "50");
    assert_eq!(
        data.constant()
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "0.1"
    );
    assert!(
        data.declared_default()
            .require()
            .unwrap()
            .to_json()
            .require()
            .unwrap()
            .contains("10e1000000")
    );
    let Lookup::Found(base) = catalogue
        .source(id)
        .unwrap()
        .definition(Class::Data, "Base")
    else {
        panic!("source Base")
    };
    let source_data = base.data_description().require().unwrap();
    assert_eq!(
        source_data.maximum().require().unwrap().lexeme(),
        "10e1000000"
    );
    assert_eq!(
        source_data.multiple_of().require().unwrap().lexeme(),
        "0.01"
    );
    let Lookup::Found(bad) = model.definition(Class::Property, "bad") else {
        panic!("effective bad Property")
    };
    let bad = bad.data_description().require().unwrap();
    assert!(matches!(bad.multiple_of(), Field::Unavailable(_)));
    assert!(matches!(bad.minimum_items(), Field::Unavailable(_)));
    assert!(matches!(bad.json().get("default"), Field::Absent));
}

#[test]
fn property_characteristics_distinguish_authored_values_and_specified_defaults() {
    use esdieff::{Characteristic, EffectiveRequest, OriginKind};
    let text = br##"{"info":{},"sdfProperty":{"Base":{"readable":false,"writable":false,"observable":false},"Inherited":{"sdfRef":"#/sdfProperty/Base"},"Override":{"sdfRef":"#/sdfProperty/Base","readable":true,"writable":true,"observable":true},"Deleted":{"sdfRef":"#/sdfProperty/Base","readable":null,"writable":null,"observable":null},"Absent":{},"Bad":{"writable":0}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let source = catalogue.source(id).unwrap();
    let Lookup::Found(authored) = source.definition(Class::Property, "Base") else {
        panic!("source Property")
    };
    assert_eq!(
        authored.writable_characteristic().require().unwrap(),
        Characteristic::Declared(false)
    );
    let Lookup::Found(inherited) = source.definition(Class::Property, "Inherited") else {
        panic!("source Property")
    };
    assert!(matches!(inherited.writable_characteristic(), Field::Absent));
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    for (name, expected) in [
        ("Inherited", Characteristic::Declared(false)),
        ("Override", Characteristic::Declared(true)),
        ("Deleted", Characteristic::SpecifiedDefault(true)),
        ("Absent", Characteristic::SpecifiedDefault(true)),
    ] {
        let Lookup::Found(property) = model.definition(Class::Property, name) else {
            panic!("effective Property")
        };
        assert_eq!(
            property.readable_characteristic().require().unwrap(),
            expected
        );
        assert_eq!(
            property.writable_characteristic().require().unwrap(),
            expected
        );
        assert_eq!(
            property.observable_characteristic().require().unwrap(),
            expected
        );
        assert_eq!(property.writable().require().unwrap(), expected.value());
    }
    let Lookup::Found(bad) = model.definition(Class::Property, "Bad") else {
        panic!("effective Property")
    };
    assert!(matches!(
        bad.writable_characteristic(),
        Field::Unavailable(_)
    ));
    assert_eq!(
        model
            .explanation("/sdfProperty/Deleted/writable")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Deleted
    );
    assert_eq!(
        model
            .default_explanation("/sdfProperty/Deleted/writable")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Default
    );
    assert!(matches!(
        model
            .json()
            .require()
            .unwrap()
            .get("sdfProperty")
            .require()
            .unwrap()
            .get("Deleted")
            .require()
            .unwrap()
            .get("writable"),
        Field::Absent
    ));
}

#[test]
fn scalar_checks_cover_all_established_full_data_positions_and_preserve_direct_item_limits() {
    let text = br#"{
      "info":{}, "sdfProperty":{"p":{"multipleOf":0}},
      "sdfAction":{"a":{"sdfInputData":{"multipleOf":0},"sdfOutputData":{"multipleOf":0},"sdfData":{"local":{"multipleOf":0}}}},
      "sdfEvent":{"e":{"sdfOutputData":{"multipleOf":0},"sdfData":{"local":{"multipleOf":0}}}},
      "sdfData":{
        "d":{"multipleOf":0},
        "nested":{"type":"object","properties":{"field":{"multipleOf":0}},"sdfChoice":{"alternative":{"multipleOf":0}},"items":{"type":"object","properties":{"item-field":{"multipleOf":0}},"sdfChoice":{"item-choice":{"type":"array","multipleOf":0}}}},
        "wide":{"type":"array","items":{"type":"array","multipleOf":0,"minItems":-1,"nullable":false,"items":false}},
        "literal":{"const":{"nullable":7,"items":false},"default":{"multipleOf":0}}
      }
    }"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 11, "{}", report.to_json());
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "parameter.positive_step")
            .count(),
        11
    );
    let source = catalogue
        .source(catalogue.source_id("model").unwrap())
        .unwrap();
    let Lookup::Found(wide) = source.definition(Class::Data, "wide") else {
        panic!("source data")
    };
    let item = wide
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(item.multiple_of(), Field::Unavailable(_)));
    assert!(matches!(item.minimum_items(), Field::Unavailable(_)));
    assert!(matches!(item.nullable(), Field::Unavailable(_)));
    assert!(matches!(item.items(), Field::Unavailable(_)));
}

#[test]
fn inherited_steps_and_deletion_keep_distinct_source_effective_and_origin_observations() {
    use esdieff::{EffectiveRequest, OriginKind};
    let text = br##"{"info":{},"sdfData":{"BadBase":{"multipleOf":0},"GoodBase":{"multipleOf":0.01}},"sdfProperty":{"Inherited":{"sdfRef":"#/sdfData/BadBase"},"Deleted":{"sdfRef":"#/sdfData/BadBase","multipleOf":null},"Good":{"sdfRef":"#/sdfData/GoodBase"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|condition| condition["code"] == "parameter.positive_step")
        .collect();
    assert!(
        rows.iter()
            .any(|condition| condition["primary"]["kind"] == "source"
                && condition["primary"]["pointer"] == "/sdfData/BadBase/multipleOf")
    );
    assert!(
        rows.iter()
            .any(|condition| condition["primary"]["kind"] == "effective"
                && condition["primary"]["pointer"] == "/sdfProperty/Inherited/multipleOf")
    );
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(deleted) = model.definition(Class::Property, "Deleted") else {
        panic!("effective Property")
    };
    assert!(matches!(
        deleted.data_description().require().unwrap().multiple_of(),
        Field::Absent
    ));
    assert_eq!(
        model
            .explanation("/sdfProperty/Deleted/multipleOf")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Deleted
    );
    let Lookup::Found(good) = model.definition(Class::Property, "Good") else {
        panic!("effective Property")
    };
    assert_eq!(
        good.data_description()
            .require()
            .unwrap()
            .multiple_of()
            .require()
            .unwrap()
            .lexeme(),
        "0.01"
    );
    assert_eq!(
        model
            .explanation("/sdfProperty/Good/multipleOf")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Inherited
    );
}

#[test]
fn needed_unknown_context_leaves_effective_access_unavailable_and_preserves_independent_findings() {
    use esdieff::EffectiveRequest;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("critical", br#"{"info":{"features":["urn:example:critical"]},"sdfProperty":{"value":{"multipleOf":0,"writable":false}}}"#),
        SourceInput::supplied("ordinary", br##"{"info":{},"sdfProperty":{"bad":{"sdfRef":"#/sdfData/Missing","multipleOf":0,"default":0.1}}}"##),
    ]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "extension.critical_unsupported")
    );
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "reference.unavailable")
    );
    let critical = catalogue
        .effective(EffectiveRequest::new(
            catalogue.source_id("critical").unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        critical.definition(Class::Property, "value"),
        Lookup::Unavailable(_)
    ));
    let ordinary_id = catalogue.source_id("ordinary").unwrap();
    let ordinary = catalogue
        .effective(EffectiveRequest::new(ordinary_id))
        .unwrap();
    assert!(matches!(
        ordinary.definition(Class::Property, "bad"),
        Lookup::Unavailable(_)
    ));
    let Lookup::Found(source_bad) = catalogue
        .source(ordinary_id)
        .unwrap()
        .definition(Class::Property, "bad")
    else {
        panic!("source Property")
    };
    assert_eq!(
        source_bad
            .data_description()
            .require()
            .unwrap()
            .declared_default()
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "0.1"
    );
}

#[test]
fn scalar_totals_and_requested_coverage_survive_presentation_truncation() {
    use esdieff::{Disposition, Limits};
    let text = br#"{"info":{},"sdfProperty":{"value":{"type":[],"writable":"no","minimum":"low","multipleOf":0,"minLength":0.1,"enum":[],"sdfChoice":{},"items":[],"descriptino":true,"example:extra":true}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
        Limits::default().pointer_bytes(1),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 8);
        assert_eq!(report.outcome().warnings(), 1);
        assert_eq!(report.outcome().notices(), 1);
        coverage_support::assert_requested_counts(&report);
        assert_eq!(report.outcome().disposition(), Disposition::Failed);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["coverage"]["requested"]["inventory"],
            "esdieff.initial/1"
        );
        assert_eq!(wire["coverage"]["requested"]["total"], 8);
        assert_eq!(wire["coverage"]["requested"]["unassessed"], 0);
        assert!(
            report
                .to_human()
                .contains("Errors: 8; warnings: 1; notices: 1; incomplete checks: 2")
        );
    }
}

#[test]
fn specified_default_detail_omission_preserves_available_typed_characteristics() {
    use esdieff::{Characteristic, InspectRequest, Limits, View};
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied(
            "model",
            br#"{"sdfProperty":{"value":{}}}"#,
        )],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let id = catalogue.source_id("model").unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(id, "").view(View::Effective))
        .unwrap();
    assert!(report.outcome().complete());
    let Lookup::Found(property) = report
        .effective_model()
        .unwrap()
        .definition(Class::Property, "value")
    else {
        panic!("effective Property")
    };
    assert_eq!(
        property.writable_characteristic().require().unwrap(),
        Characteristic::SpecifiedDefault(true)
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire["result"]["explanations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        wire["result"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|omission| omission["field"] == "result.explanations"
                && omission["reason"] == "byte_limit")
    );
    assert!(
        report
            .to_human()
            .contains("Inspection detail omitted result.explanations: byte_limit")
    );
}

#[test]
fn finite_scalar_work_preserves_an_earlier_definite_error_and_retained_content() {
    use esdieff::Limits;
    let selector = "x".repeat(600);
    let wide = format!(r#"{{"info":{{}},"sdfData":{{"wide":{{"type":"{selector}"}}}}}}"#);
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied(
                "bad",
                br#"{"info":{},"sdfProperty":{"value":{"multipleOf":0}}}"#,
            ),
            SourceInput::supplied("wide", wide.as_bytes()),
        ],
        Limits::default().decoding_work(2_000),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert!(report.outcome().errors() >= 1);
    assert!(!report.outcome().complete());
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "parameter.positive_step")
    );
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "operation.limit")
    );
    let source = catalogue
        .source(catalogue.source_id("wide").unwrap())
        .unwrap();
    assert_eq!(source.raw_bytes(), wide.as_bytes());
    assert!(matches!(source.json(), Field::Present(_)));
    assert_eq!(
        source
            .json()
            .require()
            .unwrap()
            .get("sdfData")
            .require()
            .unwrap()
            .get("wide")
            .require()
            .unwrap()
            .get("type")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        selector
    );
}
