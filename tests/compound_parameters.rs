//! Recursive compound descriptions preserve full and restricted item roles.
//! Follows ADR 0020: Owned effective views and nearest origins.
mod coverage_support;
use esdieff::{
    BaseType, Catalogue, CheckRequest, Class, DataRole, DataType, EffectiveRequest, Field, Lookup,
    SourceInput,
};

#[test]
fn direct_items_check_established_numeric_and_length_shapes() {
    for (quality, value, code) in [
        ("minimum", "true", "data.number"),
        ("maximum", "[]", "data.number"),
        ("minimum", "1e1000001", "data.number_unavailable"),
        ("minLength", "1e1000001", "data.number_unavailable"),
        ("minLength", "-1", "data.count"),
        ("maxLength", "0.1", "data.count"),
        ("maxLength", "null", "data.count"),
    ] {
        let text = format!(
            r#"{{"info":{{}},"sdfProperty":{{"values":{{"type":"array","items":{{"{quality}":{value}}}}}}}}}"#
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let expected = format!("/sdfProperty/values/items/{quality}");
        assert_eq!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["code"] == code
                    && row["primary"]["kind"] == "source"
                    && row["primary"]["pointer"] == expected)
                .count(),
            1,
            "{quality}: {value}"
        );
        assert_eq!(
            report.outcome().errors(),
            u32::from(code != "data.number_unavailable"),
            "{quality}: {value}"
        );
        assert_eq!(
            report.outcome().complete(),
            code != "data.number_unavailable"
        );
    }
}

#[test]
fn direct_items_check_selector_enum_and_applicable_collection_shapes() {
    for (members, code, quality) in [
        (r#""type":[]"#, "data.string", "type"),
        (r#""format":false"#, "data.string", "format"),
        (r#""enum":[]"#, "data.enum", "enum"),
        (r#""enum":["a",1]"#, "data.enum", "enum"),
        (r#""sdfChoice":[]"#, "data.map_shape", "sdfChoice"),
        (
            r#""type":"object","properties":[]"#,
            "data.map_shape",
            "properties",
        ),
        (
            r#""type":"object","properties":{"value":false}"#,
            "data.map_shape",
            "properties/value",
        ),
        (
            r#""sdfChoice":{"value":false}"#,
            "data.map_shape",
            "sdfChoice/value",
        ),
        (r#""enum":["a"],"sdfChoice":{}"#, "data.enum_choice", "enum"),
    ] {
        let text = format!(
            r#"{{"info":{{}},"sdfProperty":{{"values":{{"type":"array","items":{{{members}}}}}}}}}"#
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 1, "{members}");
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == code
                    && row["primary"]["kind"] == "source"
                    && row["primary"]["pointer"] == format!("/sdfProperty/values/items/{quality}"))
        );
    }
}

#[test]
fn established_direct_shapes_preserve_exact_values_and_unavailable_decoded_text() {
    for kind in ["number", "integer", "string", "boolean", "object"] {
        let text = format!(
            r#"{{"info":{{}},"sdfData":{{"rows":{{"items":{{"type":"{kind}","minimum":-0.0,"maximum":10e1000000,"minLength":1e2,"maxLength":10.0,"format":"future text format","enum":["a:b"]}}}}}}}}"#
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 0, "{kind}");
        let model = catalogue
            .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
            .unwrap();
        let Lookup::Found(rows) = model.definition(Class::Data, "rows") else {
            panic!("rows")
        };
        let direct = rows
            .data_description()
            .require()
            .unwrap()
            .items()
            .require()
            .unwrap();
        assert_eq!(direct.minimum().require().unwrap().lexeme(), "-0.0");
        assert_eq!(direct.maximum().require().unwrap().lexeme(), "10e1000000");
        assert_eq!(direct.minimum_length().require().unwrap().lexeme(), "1e2");
        assert_eq!(direct.maximum_length().require().unwrap().lexeme(), "10.0");
        assert_eq!(direct.format().require().unwrap(), "future text format");
    }
    let text = br#"{"info":{},"sdfProperty":{"rows":{"items":{"enum":["\uD800"]}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "data.enum")
    );
    let Lookup::Found(rows) = catalogue
        .sources()
        .next()
        .unwrap()
        .definition(Class::Property, "rows")
    else {
        panic!("rows")
    };
    let direct = rows
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(direct.source_enum(), Field::Unavailable(_)));
    assert!(matches!(direct.json().get("enum"), Field::Present(_)));
    assert_eq!(catalogue.sources().next().unwrap().raw_bytes(), text);
}

#[test]
fn every_recursive_full_description_checks_compound_shapes_and_positive_step() {
    let description = r#"{"type":"object","required":[],"items":false,"multipleOf":0}"#;
    let positions = [
        (
            format!(r#""sdfProperty":{{"value":{description}}}"#),
            "/sdfProperty/value",
        ),
        (
            format!(r#""sdfData":{{"value":{description}}}"#),
            "/sdfData/value",
        ),
        (
            format!(r#""sdfAction":{{"call":{{"sdfInputData":{description}}}}}"#),
            "/sdfAction/call/sdfInputData",
        ),
        (
            format!(r#""sdfAction":{{"call":{{"sdfOutputData":{description}}}}}"#),
            "/sdfAction/call/sdfOutputData",
        ),
        (
            format!(r#""sdfEvent":{{"change":{{"sdfOutputData":{description}}}}}"#),
            "/sdfEvent/change/sdfOutputData",
        ),
        (
            format!(
                r#""sdfData":{{"outer":{{"type":"object","properties":{{"value":{description}}}}}}}"#
            ),
            "/sdfData/outer/properties/value",
        ),
        (
            format!(r#""sdfData":{{"outer":{{"sdfChoice":{{"value":{description}}}}}}}"#),
            "/sdfData/outer/sdfChoice/value",
        ),
        (
            format!(
                r#""sdfData":{{"outer":{{"type":"array","items":{{"type":"object","properties":{{"value":{description}}}}}}}}}"#
            ),
            "/sdfData/outer/items/properties/value",
        ),
        (
            format!(
                r#""sdfData":{{"outer":{{"type":"array","items":{{"sdfChoice":{{"value":{description}}}}}}}}}"#
            ),
            "/sdfData/outer/items/sdfChoice/value",
        ),
    ];
    for (members, pointer) in positions {
        let text = format!(r#"{{"info":{{}},{members}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 3, "{pointer}");
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        for (code, key) in [
            ("data.map_shape", "items"),
            ("parameter.positive_step", "multipleOf"),
            ("data.required", "required"),
        ] {
            assert_eq!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|row| row["code"] == code
                        && row["primary"]["kind"] == "source"
                        && row["primary"]["pointer"] == format!("{pointer}/{key}"))
                    .count(),
                1,
                "{pointer}: {code}"
            );
        }
    }
}

#[test]
fn wider_direct_content_stays_opaque_while_array_alternatives_and_properties_are_full() {
    let text = br#"{"info":{},"sdfProperty":{"rows":{"type":"array","items":{"type":"array","multipleOf":0,"exclusiveMinimum":true,"minItems":-1,"nullable":[],"default":1,"const":2,"items":{"multipleOf":0,"sdfRequiredInputData":true},"properties":{"value":{"multipleOf":0}},"sdfChoice":{"array":{"type":"array","items":{"type":"object","properties":{"value":{"multipleOf":0}}}}}}},"objects":{"type":"array","items":{"type":"object","required":["value"],"properties":{"value":{"type":"array","items":{"type":"string","minLength":1e2}}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(!report.outcome().complete());
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert_eq!(wire["coverage"]["requested"]["unassessed"], 0);
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "parameter.positive_step"
                && row["primary"]["pointer"]
                    == "/sdfProperty/rows/items/sdfChoice/array/items/properties/value/multipleOf")
    );
    let id = catalogue.source_id("model").unwrap();
    let source = catalogue.source(id).unwrap();
    assert_eq!(source.raw_bytes(), text);
    let Lookup::Found(property) = source.definition(Class::Property, "rows") else {
        panic!("source Property")
    };
    let direct = property
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert_eq!(direct.role(), DataRole::DirectItem);
    assert!(matches!(direct.data_type(), Field::Unavailable(_)));
    assert!(matches!(direct.multiple_of(), Field::Unavailable(_)));
    assert!(matches!(direct.minimum_items(), Field::Unavailable(_)));
    assert!(matches!(direct.nullable(), Field::Unavailable(_)));
    assert!(matches!(direct.declared_default(), Field::Unavailable(_)));
    assert!(matches!(direct.constant(), Field::Unavailable(_)));
    assert!(matches!(direct.items(), Field::Unavailable(_)));
    assert!(matches!(direct.properties(), Field::Unavailable(_)));
    assert_eq!(
        direct
            .json()
            .get("default")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "1"
    );
    let alternative = direct
        .source_alternatives()
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert_eq!(alternative.role(), DataRole::Full);
    assert_eq!(
        alternative.data_type().require().unwrap(),
        DataType::Known(BaseType::Array)
    );
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(property) = model.definition(Class::Property, "rows") else {
        panic!("effective Property")
    };
    let direct = property
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(direct.data_type(), Field::Unavailable(_)));
    assert!(matches!(direct.items(), Field::Unavailable(_)));
    assert!(matches!(direct.minimum_items(), Field::Unavailable(_)));
    assert_eq!(
        direct
            .json()
            .get("default")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "1"
    );
    let alternative = direct
        .source_alternatives()
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert_eq!(alternative.role(), DataRole::Full);
    assert_eq!(
        alternative.data_type().require().unwrap(),
        DataType::Known(BaseType::Array)
    );
    let Lookup::Found(objects) = model.definition(Class::Property, "objects") else {
        panic!("object items Property")
    };
    let direct = objects
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert_eq!(
        direct.data_type().require().unwrap(),
        DataType::Known(BaseType::Map)
    );
    assert_eq!(
        direct
            .required_entries()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "value"
    );
    let field = direct
        .properties()
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert_eq!(field.role(), DataRole::Full);
    assert_eq!(
        field
            .items()
            .require()
            .unwrap()
            .minimum_length()
            .require()
            .unwrap()
            .lexeme(),
        "1e2"
    );
}

#[test]
fn nested_composition_checks_surviving_shapes_with_consumer_locations_and_origins() {
    let text = br##"{"info":{},"sdfData":{"Base":{"type":"array","items":{"type":"object","minimum":true,"properties":{"value":{"multipleOf":0}}}}},"sdfProperty":{"Inherited":{"sdfRef":"#/sdfData/Base"},"Deleted":{"sdfRef":"#/sdfData/Base","items":{"minimum":null,"properties":{"value":{"multipleOf":null}}}},"Override":{"sdfRef":"#/sdfData/Base","items":{"minimum":false}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 7);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    for (kind, root, code, suffix) in [
        ("source", "/sdfData/Base", "data.number", "items/minimum"),
        (
            "source",
            "/sdfData/Base",
            "parameter.positive_step",
            "items/properties/value/multipleOf",
        ),
        (
            "source",
            "/sdfProperty/Override",
            "data.number",
            "items/minimum",
        ),
        (
            "effective",
            "/sdfProperty/Inherited",
            "data.number",
            "items/minimum",
        ),
        (
            "effective",
            "/sdfProperty/Inherited",
            "parameter.positive_step",
            "items/properties/value/multipleOf",
        ),
        (
            "effective",
            "/sdfProperty/Override",
            "data.number",
            "items/minimum",
        ),
        (
            "effective",
            "/sdfProperty/Override",
            "parameter.positive_step",
            "items/properties/value/multipleOf",
        ),
    ] {
        assert_eq!(
            rows.iter()
                .filter(|row| row["code"] == code
                    && row["primary"]["kind"] == kind
                    && row["primary"]["pointer"] == format!("{root}/{suffix}"))
                .count(),
            1,
            "{root}: {code}"
        );
    }
    assert!(!rows.iter().any(|row| {
        row["primary"]["pointer"]
            .as_str()
            .unwrap_or("")
            .starts_with("/sdfProperty/Deleted")
    }));
    let inherited = rows
        .iter()
        .find(|row| {
            row["code"] == "data.number"
                && row["primary"]["pointer"] == "/sdfProperty/Inherited/items/minimum"
        })
        .unwrap();
    assert!(
        inherited["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |location| location["pointer"] == "/sdfData/Base/items/minimum"
                    && location["kind"] == "source"
            )
    );
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(deleted) = model.definition(Class::Property, "Deleted") else {
        panic!("deleted Property")
    };
    let direct = deleted
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(direct.minimum(), Field::Absent));
    let value = direct
        .properties()
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert!(matches!(value.multiple_of(), Field::Absent));
    assert_eq!(
        model
            .explanation("/sdfProperty/Deleted/items/minimum")
            .require()
            .unwrap()
            .kind(),
        esdieff::OriginKind::Deleted
    );
    assert_eq!(catalogue.source(id).unwrap().raw_bytes(), text);
}

#[test]
fn unresolved_direct_imports_do_not_become_effective_source_fallbacks() {
    let text = br##"{"info":{},"sdfData":{"Base":{"type":"number","minimum":7}},"sdfProperty":{"rows":{"type":"array","items":{"sdfRef":"#/sdfData/Base","minimum":true,"multipleOf":0}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "data.number")
    );
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "parameter.positive_step")
    );
    assert!(!report.outcome().complete());
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let Lookup::Found(property) = model.definition(Class::Property, "rows") else {
        panic!("rows")
    };
    assert!(matches!(
        property.data_description().require().unwrap().items(),
        Field::Unavailable(_)
    ));
    assert!(matches!(
        property.json().get("items"),
        Field::Unavailable(_)
    ));
}

#[test]
fn direct_enum_choice_presence_uses_effective_patch_deletions() {
    for (members, errors) in [
        (r#""enum":null,"sdfChoice":{}"#, 2),
        (r#""enum":["x"],"sdfChoice":null"#, 2),
        (r#""enum":null,"sdfChoice":null"#, 3),
    ] {
        let text = format!(r#"{{"info":{{}},"sdfProperty":{{"rows":{{"items":{{{members}}}}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), errors, "{members}");
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .filter(|condition| condition.code() == "data.enum_choice")
                .count(),
            1
        );
    }
    let text = br##"{"info":{},"sdfData":{"Base":{"type":"array","items":{"enum":["x"]}}},"sdfProperty":{"Conflict":{"sdfRef":"#/sdfData/Base","items":{"sdfChoice":{}}},"Deleted":{"sdfRef":"#/sdfData/Base","items":{"enum":null,"sdfChoice":{}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let conflict = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "data.enum_choice")
        .unwrap();
    assert_eq!(conflict["primary"]["kind"], "effective");
    assert_eq!(
        conflict["primary"]["pointer"],
        "/sdfProperty/Conflict/items/enum"
    );
    assert_eq!(
        conflict["related"][0]["pointer"],
        "/sdfData/Base/items/enum"
    );
}

#[test]
fn compound_aggregates_and_native_modified_assessment_survive_detail_limits() {
    use esdieff::{Assessment, AssessmentStatus, Disposition, Limits};
    let text = br#"{"info":{"modified":"1900-02-29"},"sdfProperty":{"rows":{"items":{"minLength":-1,"properties":[],"format":false,"enum":[],"sdfChoice":{"record":{"type":"object","required":[]}}}}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
        Limits::default().pointer_bytes(1),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap();
        for assessment in [
            Assessment::Initial,
            Assessment::FullBaseline,
            Assessment::ExpandedModified,
        ] {
            let report = catalogue
                .check(CheckRequest::new().assessment(assessment))
                .unwrap();
            let expanded = matches!(
                assessment,
                Assessment::ExpandedModified | Assessment::FullBaseline
            );
            // properties is wider here because no object item type establishes it.
            assert_eq!(report.outcome().errors(), if expanded { 6 } else { 5 });
            assert_eq!(report.outcome().notices(), 1);
            assert_eq!(report.outcome().disposition(), Disposition::Failed);
            coverage_support::assert_requested_counts(&report);
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            assert!(wire["baseline"].get("temporal_knowledge").is_none());
            if assessment == Assessment::ExpandedModified {
                assert!(matches!(
                    report.assessment_area_status("modified-date-time/1"),
                    Field::Present(AssessmentStatus::Assessed)
                ));
                assert_eq!(report.requested_coverage().total(), Some(9));
                coverage_support::assert_requested_counts(&report);
                assert_eq!(report.requested_coverage().unassessed(), 0);
            } else if assessment == Assessment::Initial {
                assert_eq!(report.requested_coverage().total(), Some(8));
                assert!(matches!(
                    report.assessment_area_status("modified-date-time/1"),
                    Field::Absent
                ));
            } else {
                assert_eq!(report.requested_coverage().total(), Some(13));
            }
        }
    }
}

#[test]
fn compound_work_and_critical_context_limits_preserve_independent_findings() {
    use esdieff::Limits;
    let wide = format!(
        r#"{{"info":{{}},"sdfProperty":{{"rows":{{"items":{{"format":"{}"}}}}}}}}"#,
        "x".repeat(600)
    );
    let ordinary = br#"{"info":{},"sdfProperty":{"rows":{"items":{"minimum":false}}}}"#;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("ordinary", ordinary),
            SourceInput::supplied("wide", wide.as_bytes()),
        ],
        Limits::default().decoding_work(2_000),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "data.number")
    );
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "operation.limit")
    );
    assert!(!report.outcome().complete());
    assert_eq!(
        catalogue
            .source(catalogue.source_id("wide").unwrap())
            .unwrap()
            .raw_bytes(),
        wide.as_bytes()
    );

    let critical = br#"{"info":{"features":["future"]},"sdfProperty":{"rows":{"items":{"minimum":false,"sdfChoice":[]}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("critical", critical),
        SourceInput::supplied("ordinary", ordinary),
    ])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "data.number")
            .count(),
        1
    );
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "data.map_shape")
    );
    let model = catalogue
        .effective(EffectiveRequest::new(
            catalogue.source_id("critical").unwrap(),
        ))
        .unwrap();
    assert!(matches!(model.json(), Field::Unavailable(_)));
}

#[test]
fn established_object_required_is_a_nonempty_string_array() {
    for (required, errors) in [
        ("[]", 1),
        ("true", 1),
        ("null", 1),
        (r#"["value",1]"#, 1),
        (r#"["value"]"#, 0),
        (r#"["\uD800"]"#, 0),
    ] {
        for direct in [false, true] {
            let description = format!(r#"{{"type":"object","required":{required}}}"#);
            let root = if direct {
                format!(r#"{{"items":{description}}}"#)
            } else {
                description
            };
            let text = format!(r#"{{"info":{{}},"sdfData":{{"record":{root}}}}}"#);
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(
                report.outcome().errors(),
                errors,
                "direct {direct}, {required}"
            );
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(
                        |row| row["code"] == "data.required" && row["primary"]["kind"] == "source"
                    )
                    .count(),
                errors as usize
            );
        }
    }
}

#[test]
fn typed_required_syntax_distinguishes_empty_unavailable_and_documentary_lists() {
    for (description, direct, state) in [
        (r#"{"type":"object","required":[]}"#, false, "unavailable"),
        (r#"{"type":"object","required":[]}"#, true, "unavailable"),
        (
            r#"{"type":"object","required":["\uD800"]}"#,
            true,
            "unavailable",
        ),
        (r#"{"type":"object"}"#, true, "absent"),
        (r#"{"required":["value"]}"#, false, "present"),
        (
            r#"{"type":"string","required":["value"]}"#,
            true,
            "unavailable",
        ),
        (r#"{"type":"object","required":["value"]}"#, true, "present"),
    ] {
        let root = if direct {
            format!(r#"{{"items":{description}}}"#)
        } else {
            description.to_owned()
        };
        let text = format!(r#"{{"info":{{}},"sdfData":{{"record":{root}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
        let id = catalogue.source_id("model").unwrap();
        let Lookup::Found(record) = catalogue
            .source(id)
            .unwrap()
            .definition(Class::Data, "record")
        else {
            panic!("source record")
        };
        let data = record.data_description().require().unwrap();
        let data = if direct {
            data.items().require().unwrap()
        } else {
            data
        };
        match (state, data.required_entries()) {
            ("unavailable", Field::Unavailable(_)) | ("absent", Field::Absent) => {}
            ("present", Field::Present(mut values)) => {
                assert_eq!(values.next().unwrap().text().require().unwrap(), "value")
            }
            _ => panic!("source required syntax: {description}"),
        }
        let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
        let Lookup::Found(record) = model.definition(Class::Data, "record") else {
            panic!("effective record")
        };
        let data = record.data_description().require().unwrap();
        let data = if direct {
            data.items().require().unwrap()
        } else {
            data
        };
        match (state, data.required_entries()) {
            ("unavailable", Field::Unavailable(_)) | ("absent", Field::Absent) => {}
            ("present", Field::Present(mut values)) => {
                assert_eq!(values.next().unwrap().text().require().unwrap(), "value")
            }
            _ => panic!("effective required syntax: {description}"),
        }
    }
}

#[test]
fn required_applicability_uses_reliable_explicit_object_context() {
    for members in [
        r#""required":[]"#,
        r#""type":"string","required":[]"#,
        r#""type":"future-type","required":[]"#,
        r#""type":"\uD800","required":[]"#,
        r#""type":"object","required":[],"\uD800":1"#,
    ] {
        for direct in [false, true] {
            let description = format!(r#"{{{members}}}"#);
            let record = if direct {
                format!(r#"{{"items":{description}}}"#)
            } else {
                description
            };
            let text = format!(
                r#"{{"info":{{}},"sdfData":{{"record":{record},"independent":{{"multipleOf":0}}}}}}"#
            );
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("model", text.as_bytes())]).unwrap();
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(report.outcome().errors(), 1, "direct {direct}: {members}");
            assert!(
                !report
                    .diagnostics()
                    .conditions()
                    .any(|condition| condition.code() == "data.required")
            );
            assert_eq!(
                report
                    .diagnostics()
                    .conditions()
                    .filter(|condition| condition.code() == "parameter.positive_step")
                    .count(),
                1
            );
        }
    }
}

#[test]
fn required_type_changes_invalidate_child_replay_without_inventing_wider_meaning() {
    let text = br##"{"info":{},"sdfData":{"Primitive":{"type":"string","required":[]},"Object":{"type":"object","required":[]},"Array":{"items":{"type":"string","required":[]}},"ObjectItems":{"items":{"type":"object","required":[]}}},"sdfProperty":{"Established":{"sdfRef":"#/sdfData/Primitive","type":"object"},"Removed":{"sdfRef":"#/sdfData/Object","type":"string"},"Deleted":{"sdfRef":"#/sdfData/Object","type":null},"ItemEstablished":{"sdfRef":"#/sdfData/Array","items":{"type":"object"}},"ItemRemoved":{"sdfRef":"#/sdfData/ObjectItems","items":{"type":"array"}},"ItemDeleted":{"sdfRef":"#/sdfData/ObjectItems","items":{"type":null}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 4);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    for (kind, pointer, origin) in [
        ("source", "/sdfData/Object/required", ""),
        ("source", "/sdfData/ObjectItems/items/required", ""),
        (
            "effective",
            "/sdfProperty/Established/required",
            "/sdfData/Primitive/required",
        ),
        (
            "effective",
            "/sdfProperty/ItemEstablished/items/required",
            "/sdfData/Array/items/required",
        ),
    ] {
        let found: Vec<_> = rows
            .iter()
            .filter(|row| {
                row["code"] == "data.required"
                    && row["primary"]["kind"] == kind
                    && row["primary"]["pointer"] == pointer
            })
            .collect();
        assert_eq!(found.len(), 1, "{pointer}");
        if !origin.is_empty() {
            assert_eq!(found[0]["related"][0]["pointer"], origin);
        }
    }
    assert_eq!(
        rows.iter()
            .filter(|row| row["code"] == "data.required")
            .count(),
        4
    );
    assert_eq!(catalogue.sources().next().unwrap().raw_bytes(), text);
}

#[test]
fn literal_required_null_differs_from_nested_patch_deletion_and_unavailable_imports() {
    let text = br##"{"info":{},"sdfData":{"Base":{"type":"object","required":["value"],"properties":{"value":{}}},"Items":{"items":{"type":"object","required":["value"]}}},"sdfProperty":{"Ordinary":{"type":"object","required":null},"Deleted":{"sdfRef":"#/sdfData/Base","required":null},"Override":{"sdfRef":"#/sdfData/Base","required":[]},"ItemDeleted":{"sdfRef":"#/sdfData/Items","items":{"required":null}},"Missing":{"sdfRef":"#/sdfData/missing","type":"object","required":null,"properties":{"value":{"multipleOf":0}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 3);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .filter(|row| row["code"] == "data.required")
            .count(),
        2
    );
    for (kind, pointer) in [
        ("source", "/sdfProperty/Ordinary/required"),
        ("effective", "/sdfProperty/Override/required"),
    ] {
        assert_eq!(
            rows.iter()
                .filter(|row| row["code"] == "data.required"
                    && row["primary"]["kind"] == kind
                    && row["primary"]["pointer"] == pointer)
                .count(),
            1
        );
    }
    assert!(
        rows.iter()
            .any(|row| row["code"] == "parameter.positive_step"
                && row["primary"]["pointer"] == "/sdfProperty/Missing/properties/value/multipleOf")
    );
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let Lookup::Found(deleted) = model.definition(Class::Property, "Deleted") else {
        panic!("deleted")
    };
    assert!(matches!(
        deleted
            .data_description()
            .require()
            .unwrap()
            .required_entries(),
        Field::Absent
    ));
    assert!(matches!(
        model.definition(Class::Property, "Missing"),
        Lookup::Unavailable(_)
    ));
}

#[test]
fn required_checking_limits_keep_earlier_errors_and_raw_lists() {
    use esdieff::Limits;
    let entries = vec![r#""""#; 200].join(",");
    let wide = format!(
        r#"{{"info":{{}},"sdfData":{{"record":{{"type":"object","required":[{entries}]}}}}}}"#
    );
    let early = br#"{"info":{},"sdfProperty":{"rows":{"items":{"type":"object","required":[]}}}}"#;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("early", early),
            SourceInput::supplied("wide", wide.as_bytes()),
        ],
        Limits::default().decoding_work(1_600),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "data.required")
    );
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "operation.limit")
    );
    assert!(!report.outcome().complete());
    let source = catalogue
        .source(catalogue.source_id("wide").unwrap())
        .unwrap();
    assert_eq!(source.raw_bytes(), wide.as_bytes());
    assert!(matches!(source.json(), Field::Present(_)));
}
