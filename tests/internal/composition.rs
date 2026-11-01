//! Public composition preserves reusable sources and explains effective refinements.
//! Follows ADR 0006: Scoped reference failures, and
//! ADR 0013: Shared workflow and report guarantees.
use esdieff::{Catalogue, Class, InspectRequest, SourceInput, View};
use serde_json::Value;

const TEMPERATURE_DEVICE: &[u8] = include_bytes!("../fixtures/temperature-device.sdf.json");

#[test]
fn inspect_same_document_device_composes_sensor_without_mutating_sources() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("device", TEMPERATURE_DEVICE)]).unwrap();
    let id = catalogue.source_id("device").unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, "/sdfThing/Device/sdfObject/ambient").view(View::Effective),
        )
        .unwrap();
    assert!(report.outcome().complete());
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["result"]["state"], "available");
    let ambient = &wire["result"]["json"];
    assert!(ambient.get("description").is_none());
    let temperature = &ambient["sdfProperty"]["temperature"];
    assert_eq!(temperature["type"], "number");
    assert_eq!(temperature["minimum"], -50);
    assert_eq!(temperature["maximum"], 100);
    assert_eq!(temperature["writable"], false);
    assert_eq!(
        ambient["sdfEvent"]["changed"]["sdfOutputData"]["maximum"],
        150
    );
    assert!(temperature.get("sdfRef").is_none());
    let source = catalogue.source(id).unwrap();
    assert_eq!(source.raw_bytes(), TEMPERATURE_DEVICE);
    let sensor = source.definition(Class::Object, "Sensor");
    let esdieff::Lookup::Found(sensor) = sensor else {
        panic!("source Sensor")
    };
    assert_eq!(
        sensor.description().require().unwrap(),
        "Reusable temperature sensing"
    );
    let esdieff::Lookup::Found(temperature) = source.definition(Class::Data, "Temperature") else {
        panic!("source Temperature")
    };
    assert_eq!(
        temperature
            .data_description()
            .require()
            .unwrap()
            .number("maximum")
            .require()
            .unwrap()
            .lexeme(),
        "150"
    );
}

#[test]
fn effective_typed_groupings_affordances_and_data_use_shared_roles_and_defaults() {
    use esdieff::{BaseType, DataRole, DataType, EffectiveRequest, Field, Lookup};
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("device", TEMPERATURE_DEVICE)]).unwrap();
    let id = catalogue.source_id("device").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(device) = model.definition(Class::Thing, "Device") else {
        panic!("effective Device")
    };
    let ambient = device
        .children(Class::Object)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert!(matches!(ambient.description(), Field::Absent));
    let temperature = ambient
        .children(Class::Property)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert!(!temperature.writable().require().unwrap());
    assert!(temperature.readable().require().unwrap());
    assert!(temperature.observable().require().unwrap());
    let data = temperature.data_description().require().unwrap();
    assert_eq!(data.role(), DataRole::Full);
    assert_eq!(
        data.data_type().require().unwrap(),
        DataType::Known(BaseType::Number)
    );
    assert_eq!(data.number("minimum").require().unwrap().lexeme(), "-50");
    assert_eq!(data.number("maximum").require().unwrap().lexeme(), "100");
    assert!(!data.nullable().require().unwrap());
    let action = ambient
        .children(Class::Action)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert!(matches!(action.input(), Field::Absent));
    let event = ambient
        .children(Class::Event)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert_eq!(
        event
            .output()
            .require()
            .unwrap()
            .number("maximum")
            .require()
            .unwrap()
            .lexeme(),
        "150"
    );
    let Lookup::Found(data) = model.definition(Class::Data, "Temperature") else {
        panic!("effective sdfData")
    };
    assert_eq!(data.class(), Class::Data);
}

#[test]
fn inspection_explains_inherited_refined_and_deleted_qualities_at_distinct_locations() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("device", TEMPERATURE_DEVICE)]).unwrap();
    let id = catalogue.source_id("device").unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, "/sdfThing/Device/sdfObject/ambient").view(View::Effective),
        )
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let explanations = wire["result"]["explanations"].as_array().unwrap();
    let minimum = explanations
        .iter()
        .find(|value| {
            value["effective"]["pointer"]
                == "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature/minimum"
        })
        .expect("inherited minimum explanation");
    assert_eq!(minimum["kind"], "inherited");
    assert_eq!(minimum["effective"]["kind"], "effective");
    assert!(minimum["effective"].get("span").is_none());
    assert_eq!(
        minimum["sources"][0]["pointer"],
        "/sdfData/Temperature/minimum"
    );
    let maximum = explanations
        .iter()
        .find(|value| {
            value["effective"]["pointer"]
                == "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature/maximum"
        })
        .unwrap();
    assert_eq!(maximum["kind"], "override");
    assert_eq!(
        maximum["sources"][0]["pointer"],
        "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature/maximum"
    );
    let writable = explanations
        .iter()
        .find(|value| {
            value["effective"]["pointer"]
                == "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature/writable"
        })
        .unwrap();
    assert_eq!(writable["kind"], "inherited");
    assert_eq!(
        writable["sources"][0]["pointer"],
        "/sdfObject/Sensor/sdfProperty/temperature/writable"
    );
    let deleted = explanations
        .iter()
        .find(|value| {
            value["effective"]["pointer"] == "/sdfThing/Device/sdfObject/ambient/description"
        })
        .unwrap();
    assert_eq!(deleted["kind"], "deleted");
    assert_eq!(
        deleted["sources"][0]["pointer"],
        "/sdfThing/Device/sdfObject/ambient/description"
    );
    assert_eq!(
        deleted["sources"][1]["pointer"],
        "/sdfObject/Sensor/description"
    );
}

#[test]
fn deletion_and_implicit_default_are_both_observable_without_json_insertion() {
    use esdieff::{EffectiveRequest, Field, Lookup, OriginKind};
    let text=br##"{"sdfProperty":{"Base":{"writable":false},"Use":{"sdfRef":"#/sdfProperty/Base","writable":null}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("defaults", text)]).unwrap();
    let id = catalogue.source_id("defaults").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(property) = model.definition(Class::Property, "Use") else {
        panic!("Use")
    };
    assert!(matches!(property.json().get("writable"), Field::Absent));
    assert!(property.writable().require().unwrap());
    let deleted = model
        .explanation("/sdfProperty/Use/writable")
        .require()
        .unwrap();
    assert_eq!(deleted.kind(), OriginKind::Deleted);
    let sources: Vec<_> = deleted.sources().collect();
    assert_eq!(
        sources[0].pointer(8192).require().unwrap(),
        "/sdfProperty/Use/writable"
    );
    assert_eq!(
        sources[1].pointer(8192).require().unwrap(),
        "/sdfProperty/Base/writable"
    );
    let default = model
        .default_explanation("/sdfProperty/Use/writable")
        .require()
        .unwrap();
    assert_eq!(default.kind(), OriginKind::Default);
    assert_eq!(default.sources().count(), 0);
    assert_eq!(
        default.effective().pointer(8192).require().unwrap(),
        "/sdfProperty/Use/writable"
    );
    assert_eq!(
        model
            .default_explanation("/sdfProperty/Use/readable")
            .require()
            .unwrap()
            .sources()
            .count(),
        0
    );
    let report = catalogue
        .inspect(InspectRequest::new(id, "/sdfProperty/Use").view(View::Effective))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(wire["result"]["json"].get("writable").is_none());
    let origins: Vec<_> = wire["result"]["explanations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["effective"]["pointer"] == "/sdfProperty/Use/writable")
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(origins.contains(&"deleted") && origins.contains(&"default"));
}

#[test]
fn selected_composition_spends_reference_budget_only_on_needed_branches() {
    use esdieff::{EffectiveRequest, Limits};
    let text=br##"{"sdfData":{"Use":{"sdfRef":"#/sdfData/Base"},"Base":{"minimum":3},"Unused":{"sdfRef":"foreign:#/sdfData/Missing"}}}"##;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("scope", text)],
        Limits::default().reference_applications(1),
    )
    .unwrap();
    let id = catalogue.source_id("scope").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfData/Use"))
        .unwrap();
    assert!(model.complete());
    assert_eq!(
        model
            .json()
            .require()
            .unwrap()
            .get("minimum")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "3"
    );
}

#[test]
fn partial_named_collections_and_reference_conditions_preserve_established_siblings() {
    use esdieff::{EffectiveRequest, Field, Lookup};
    let text=br##"{"sdfData":{"Good":{"minimum":4},"Missing":{"sdfRef":"#/sdfData/Absent"},"Cycle":{"sdfRef":"#/sdfData/Cycle"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("partial", text)]).unwrap();
    let id = catalogue.source_id("partial").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(!model.complete());
    let rows: Vec<_> = model.definitions(Class::Data).require().unwrap().collect();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .any(|(name, value)| *name == "Good" && matches!(value, Field::Present(_)))
    );
    assert!(
        rows.iter()
            .any(|(name, value)| *name == "Missing" && matches!(value, Field::Unavailable(_)))
    );
    assert!(matches!(
        model.definition(Class::Data, "Good"),
        Lookup::Found(_)
    ));
    assert!(matches!(
        model.definition(Class::Data, "Missing"),
        Lookup::Unavailable(_)
    ));
    let json = model.json().require().unwrap();
    assert_eq!(
        json.get("sdfData")
            .require()
            .unwrap()
            .get("Good")
            .require()
            .unwrap()
            .get("minimum")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "4"
    );
    assert!(matches!(json.to_json(), Field::Unavailable(_)));
    let report = catalogue
        .inspect(InspectRequest::new(id, "").view(View::Effective))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let conditions = wire["diagnostics"]["items"].as_array().unwrap();
    for (code, path) in [
        ("reference.unavailable", "/sdfData/Missing"),
        ("reference.effective_cycle", "/sdfData/Cycle"),
    ] {
        let condition = conditions
            .iter()
            .find(|value| value["code"] == code)
            .expect("needed reference condition");
        assert_eq!(condition["severity"], "incomplete");
        assert_eq!(condition["primary"]["kind"], "effective");
        assert_eq!(condition["primary"]["pointer"], path);
        assert!(
            condition["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source["kind"] == "source"
                    && source["pointer"]
                        .as_str()
                        .is_some_and(|p| p.ends_with("/sdfRef")))
        );
    }
    assert!(
        catalogue
            .inspect(InspectRequest::new(id, "/sdfData/Missing/sdfRef"))
            .unwrap()
            .outcome()
            .complete()
    );
}

#[test]
fn map_shape_does_not_grant_an_opaque_or_information_reference_target_role() {
    use esdieff::{EffectiveRequest, Field, Lookup};
    let text=br##"{"info":{"type":"number"},"opaque":{"type":"number"},"sdfData":{"Good":{"minimum":1},"Opaque":{"sdfRef":"#/opaque"},"Information":{"sdfRef":"#/info"},"Literal":{"const":{"type":"number"}},"Constant":{"sdfRef":"#/sdfData/Literal/const"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("roles", text)]).unwrap();
    let id = catalogue.source_id("roles").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(!model.complete());
    assert!(matches!(
        model.definition(Class::Data, "Good"),
        Lookup::Found(_)
    ));
    for name in ["Opaque", "Information", "Constant"] {
        assert!(matches!(
            model.definition(Class::Data, name),
            Lookup::Unavailable(_)
        ));
    }
    let source = catalogue.source(id).unwrap();
    assert!(matches!(
        source.json().require().unwrap().get("opaque"),
        Field::Present(_)
    ));
}

#[test]
fn inherited_direct_item_object_type_establishes_consumer_property_metadata_checks() {
    use esdieff::CheckRequest;
    let text=br##"{"info":{},"sdfData":{"Base":{"type":"array","items":{"type":"object"}},"Use":{"sdfRef":"#/sdfData/Base","items":{"properties":{"field":{"description":7}}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("metadata", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["code"] == "metadata.string")
        .unwrap();
    assert_eq!(finding["primary"]["kind"], "effective");
    assert_eq!(
        finding["primary"]["pointer"],
        "/sdfData/Use/items/properties/field/description"
    );
    assert!(report.outcome().complete());
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
}

#[test]
fn copied_required_pointer_keeps_its_authored_text_and_typed_requiredness() {
    // The pointer relocates by coordinate when its relationship is resolved,
    // following ADR 0036: Copied reference relocation, so composition neither
    // rewrites its text nor leaves the requiredness unavailable.
    use esdieff::{EffectiveRequest, Field, Lookup};
    let text=br##"{"sdfObject":{"Base":{"sdfProperty":{"value":{"type":"number"}},"sdfRequired":["#/sdfObject/Base/sdfProperty/value"]},"Use":{"sdfRef":"#/sdfObject/Base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("required", text)]).unwrap();
    let id = catalogue.source_id("required").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfObject/Use"))
        .unwrap();
    assert!(model.complete());
    let json = model.json().require().unwrap();
    assert!(matches!(json.get("sdfProperty"), Field::Present(_)));
    assert_eq!(
        json.get("sdfRequired")
            .require()
            .unwrap()
            .at(0)
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "#/sdfObject/Base/sdfProperty/value"
    );
    let whole = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    let Lookup::Found(use_object) = whole.definition(Class::Object, "Use") else {
        panic!("Use")
    };
    assert!(matches!(
        use_object.declaration_requiredness(),
        Field::Present(_)
    ));
    let source = catalogue.source(id).unwrap();
    let Lookup::Found(base) = source.definition(Class::Object, "Base") else {
        panic!("Base")
    };
    assert!(matches!(base.declaration_requiredness(), Field::Present(_)));
}

#[test]
fn target_root_chain_deletes_unneeded_references_and_preserves_literal_nulls() {
    use esdieff::EffectiveRequest;
    let text=br##"{"sdfThing":{"Base":{"description":"base","opaque":{"literal":null},"sdfObject":{"bad":{"sdfRef":"#/sdfObject/Absent"}}},"Middle":{"sdfRef":"#/sdfThing/Base","sdfObject":{"bad":null}},"Use":{"sdfRef":"#/sdfThing/Middle","description":null}},"sdfData":{"Literal":{"const":{"literal":null},"default":null,"enum":[null]},"Copy":{"sdfRef":"#/sdfData/Literal"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("chain", text)]).unwrap();
    let id = catalogue.source_id("chain").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfThing/Use"))
        .unwrap();
    assert!(model.complete());
    let json: Value =
        serde_json::from_str(&model.json().require().unwrap().to_json().require().unwrap())
            .unwrap();
    assert!(json.get("description").is_none());
    assert!(json["sdfObject"].get("bad").is_none());
    assert_eq!(json["opaque"]["literal"], Value::Null);
    let literal = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfData/Copy"))
        .unwrap();
    assert!(!literal.complete());
    let value = literal.json().require().unwrap();
    assert_eq!(
        value
            .get("const")
            .require()
            .unwrap()
            .get("literal")
            .require()
            .unwrap()
            .kind(),
        esdieff::JsonKind::Null
    );
    assert_eq!(
        value.get("default").require().unwrap().kind(),
        esdieff::JsonKind::Null
    );
    assert_eq!(
        value
            .get("enum")
            .require()
            .unwrap()
            .at(0)
            .require()
            .unwrap()
            .kind(),
        esdieff::JsonKind::Null
    );
    // Null stays authored data, but cannot establish an enum text alternative.
    assert!(matches!(value.to_json(), esdieff::Field::Unavailable(_)));
    assert_eq!(catalogue.source(id).unwrap().raw_bytes(), text);
}

#[test]
fn deep_derived_groupings_generate_exact_numbers_without_source_depth_assumptions() {
    use esdieff::EffectiveRequest;
    use std::fmt::Write;
    let mut text = String::from("{\"sdfThing\":{");
    for index in 0..100 {
        if index > 0 {
            text.push(',');
        }
        write!(
            text,
            "\"T{index}\":{{\"sdfThing\":{{\"next\":{{\"sdfRef\":\"#/sdfThing/T{}\"}}}}}}",
            index + 1
        )
        .unwrap();
    }
    text.push_str(",\"T100\":{\"sdfProperty\":{\"value\":{\"type\":\"number\",\"maximum\":10e1000000,\"minimum\":1e-1000000,\"default\":0.1}}}}}");
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("deep", text.as_bytes())]).unwrap();
    let id = catalogue.source_id("deep").unwrap();
    assert!(
        catalogue
            .source(id)
            .unwrap()
            .json()
            .require()
            .unwrap()
            .capability_complete()
    );
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfThing/T0"))
        .unwrap();
    assert!(model.complete());
    let mut json = model.json().require().unwrap();
    for _ in 0..100 {
        json = json
            .get("sdfThing")
            .require()
            .unwrap()
            .get("next")
            .require()
            .unwrap();
    }
    let value = json
        .get("sdfProperty")
        .require()
        .unwrap()
        .get("value")
        .require()
        .unwrap();
    for (name, lexeme) in [
        ("maximum", "10e1000000"),
        ("minimum", "1e-1000000"),
        ("default", "0.1"),
    ] {
        assert_eq!(
            value
                .get(name)
                .require()
                .unwrap()
                .number()
                .require()
                .unwrap()
                .lexeme(),
            lexeme
        );
    }
    let generated = model.json().require().unwrap().to_json().require().unwrap();
    assert!(generated.contains("\"maximum\":10e1000000"));
    assert!(generated.contains("\"minimum\":1e-1000000"));
    assert!(generated.contains("\"default\":0.1"));
    assert!(!generated.contains("sdfRef"));
}

#[test]
fn cumulative_reference_limits_and_early_node_exhaustion_are_observable_domain_results() {
    use esdieff::{CheckRequest, EffectiveRequest, Field, Limits};
    let text =
        br##"{"info":{},"sdfData":{"Base":{"minimum":2},"Use":{"sdfRef":"#/sdfData/Base"}}}"##;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("one", text),
            SourceInput::supplied("two", text),
        ],
        Limits::default().reference_applications(1),
    )
    .unwrap();
    let second = catalogue.source_id("two").unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(wire["diagnostics"]["items"].as_array().unwrap().iter().any(
        |condition| condition["code"] == "operation.limit"
            && condition["primary"]["root"] == second.get()
            && condition["primary"]["pointer"] == "/sdfData/Use"
    ));
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    let limited = Catalogue::with_limits(
        &[SourceInput::supplied("nodes", text)],
        Limits::default().effective_nodes(1),
    )
    .unwrap();
    let id = limited.source_id("nodes").unwrap();
    let model = limited.effective(EffectiveRequest::new(id)).unwrap();
    assert!(matches!(model.json(), Field::Unavailable(_)));
    assert!(
        limited
            .source(id)
            .unwrap()
            .json()
            .require()
            .unwrap()
            .capability_complete()
    );
    let report = limited
        .inspect(InspectRequest::new(id, "").view(View::Effective))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition["code"] == "operation.limit")
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 1);
}

#[test]
fn effective_observation_totals_survive_condition_byte_and_pointer_presentation_limits() {
    use esdieff::{CheckRequest, Limits};
    let text=br##"{"info":{},"sdfData":{"Base":{"type":"array","items":{"type":"object"}},"Use":{"sdfRef":"#/sdfData/Base","items":{"properties":{"field":{"description":7,"descriptino":"advice"}}}}}}"##;
    let base = Catalogue::from_inputs(&[SourceInput::supplied("observations", text)]).unwrap();
    let report = base
        .check(CheckRequest::new().strict_warnings(true))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.outcome().warnings(), 1);
    let expected: Value = serde_json::from_str(&report.to_json()).unwrap();
    for limits in [
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1024),
        Limits::default().pointer_bytes(4).pointer_output_bytes(16),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("observations", text)], limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().strict_warnings(true))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["outcome"], expected["outcome"]);
        assert_eq!(
            wire["coverage"]["requested"],
            expected["coverage"]["requested"]
        );
        assert_eq!(
            wire["diagnostics"]["retained"].as_u64().unwrap()
                + wire["diagnostics"]["omitted"].as_u64().unwrap(),
            expected["diagnostics"]["retained"].as_u64().unwrap()
                + expected["diagnostics"]["omitted"].as_u64().unwrap()
        );
        assert!(
            wire["diagnostics"]["omitted"].as_u64().unwrap() > 0
                || !wire["diagnostics"]["omissions"]
                    .as_array()
                    .unwrap()
                    .is_empty()
        );
    }
}

#[test]
fn effective_inspection_establishes_needed_feature_context_without_assessing_model_metadata() {
    use esdieff::{CheckRequest, EffectiveRequest, Field};
    let clear=br##"{"info":{"license":7},"sdfAction":{"bad":{"sdfRequiredInputData":7}},"sdfData":{"value":{"type":"number"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("inspection", clear)]).unwrap();
    let id = catalogue.source_id("inspection").unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(id, "").view(View::Effective))
        .unwrap();
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 0);
    assert_eq!(
        catalogue
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .errors(),
        2
    );
    let unknown=br##"{"info":{"features":false,"license":7},"sdfAction":{"bad":{"sdfRequiredInputData":7}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("unknown", unknown)]).unwrap();
    let id = catalogue.source_id("unknown").unwrap();
    assert!(matches!(
        catalogue
            .effective(EffectiveRequest::new(id))
            .unwrap()
            .json(),
        Field::Unavailable(_)
    ));
    let report = catalogue
        .inspect(InspectRequest::new(id, "").view(View::Effective))
        .unwrap();
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "structure.feature_context_unavailable")
    );
    assert_eq!(
        catalogue
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .errors(),
        0
    );
    assert!(
        catalogue
            .inspect(InspectRequest::new(id, "/info/license"))
            .unwrap()
            .outcome()
            .complete()
    );
}

#[test]
fn copied_internal_references_relocate_and_direct_item_imports_leave_independent_handles_usable() {
    use esdieff::{EffectiveRequest, Field, Lookup};
    let text=br##"{"sdfThing":{"Base":{"sdfData":{"Local":{"minimum":1}},"sdfProperty":{"internal":{"sdfRef":"#/sdfThing/Base/sdfData/Local"},"good":{"type":"number"}}},"Use":{"sdfRef":"#/sdfThing/Base","sdfData":{"Local":{"minimum":2}}}},"sdfData":{"Target":{"type":"number"},"Array":{"type":"array","items":{"sdfRef":"#/sdfData/Target"}},"WrongTarget":{"sdfRef":"#/sdfData/Array/items"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("identities", text)]).unwrap();
    let id = catalogue.source_id("identities").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(!model.complete());
    let Lookup::Found(use_thing) = model.definition(Class::Thing, "Use") else {
        panic!("partial Use handle")
    };
    let rows: Vec<_> = use_thing
        .children(Class::Property)
        .require()
        .unwrap()
        .collect();
    assert!(
        rows.iter()
            .any(|(name, value)| *name == "good" && matches!(value, Field::Present(_)))
    );
    // The copied internal reference relocates to Use's own refined `Local`.
    let Some((_, Field::Present(internal))) = rows.iter().find(|(name, _)| *name == "internal")
    else {
        panic!("relocated internal reference")
    };
    assert_eq!(
        internal
            .json()
            .get("minimum")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "2"
    );
    let Lookup::Found(array) = model.definition(Class::Data, "Array") else {
        panic!("partial Array handle")
    };
    let array = array.data_description().require().unwrap();
    assert!(matches!(array.items(), Field::Unavailable(_)));
    assert_eq!(
        array
            .json()
            .get("type")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "array"
    );
    assert!(matches!(
        model.definition(Class::Data, "WrongTarget"),
        Lookup::Unavailable(_)
    ));
    assert!(
        catalogue
            .inspect(InspectRequest::new(id, "/sdfData/Array/items/sdfRef"))
            .unwrap()
            .outcome()
            .complete()
    );
}

#[test]
fn copied_required_short_names_boolean_syntax_and_independent_targets_are_retained() {
    use esdieff::{EffectiveRequest, Lookup};
    let text=br##"{"sdfObject":{"Outside":{"sdfProperty":{"external":{"type":"number"}}},"Base":{"sdfProperty":{"direct":{"type":"number"}},"sdfRequired":["direct",true,"#/sdfObject/Outside/sdfProperty/external"]},"Use":{"sdfRef":"#/sdfObject/Base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("independent", text)]).unwrap();
    let id = catalogue.source_id("independent").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(model.complete());
    let Lookup::Found(use_object) = model.definition(Class::Object, "Use") else {
        panic!("Use")
    };
    let syntax = use_object.declaration_requiredness().require().unwrap();
    assert_eq!(
        syntax.at(0).require().unwrap().text().require().unwrap(),
        "direct"
    );
    assert!(syntax.at(1).require().unwrap().boolean().require().unwrap());
    assert_eq!(
        syntax.at(2).require().unwrap().text().require().unwrap(),
        "#/sdfObject/Outside/sdfProperty/external"
    );
}

#[test]
fn copied_full_data_required_lookup_exhaustion_keeps_its_operational_cause_and_source() {
    use esdieff::Limits;
    // A copied pointer in a full data description keeps the data-only limitation
    // of ADR 0023: Supplied global composition and dependency scope, which still
    // looks up its target.
    let name = "B".repeat(512);
    let text = format!(
        r##"{{"sdfObject":{{"{name}":{{"sdfAction":{{"a":{{"sdfOutputData":{{"type":"number","sdfRequired":["#/sdfObject/{name}/sdfAction/a"]}}}}}}}},"Use":{{"sdfRef":"#/sdfObject/{name}"}}}}}}"##
    );
    let mut observed = false;
    for work in (500..=20000).step_by(500) {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("required-limit", text.as_bytes())],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let id = catalogue.source_id("required-limit").unwrap();
        if !catalogue
            .source(id)
            .unwrap()
            .json()
            .present()
            .is_some_and(|json| json.capability_complete())
        {
            continue;
        }
        let report = catalogue
            .inspect(InspectRequest::new(id, "/sdfObject/Use").view(View::Effective))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        for condition in wire["diagnostics"]["items"].as_array().unwrap() {
            if condition["message"].as_str().is_some_and(|message| {
                message.starts_with("reference ") && message.contains("limit")
            }) && condition["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| {
                    source["pointer"]
                        .as_str()
                        .is_some_and(|pointer| pointer.ends_with("/sdfRequired/0"))
                })
            {
                assert_eq!(condition["code"], "operation.limit");
                observed = true;
            }
        }
        if observed {
            break;
        }
    }
    assert!(
        observed,
        "a lowered copied-required lookup retains its operation-limit cause"
    );
}

#[test]
fn bounded_effective_generation_omits_payload_without_source_fallback_or_semantic_loss() {
    use esdieff::{EffectiveRequest, Field, Limits};
    let text = format!("{{\"opaque\":\"{}\"}}", "z".repeat(30000));
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("generation", text.as_bytes())],
        Limits::default().report_bytes(22000),
    )
    .unwrap();
    let id = catalogue.source_id("generation").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(model.complete());
    assert!(matches!(
        model.json().require().unwrap().to_json(),
        Field::Unavailable(_)
    ));
    assert_eq!(
        model
            .json()
            .require()
            .unwrap()
            .get("opaque")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap()
            .len(),
        30000
    );
    let report = catalogue
        .inspect(InspectRequest::new(id, "").view(View::Effective))
        .unwrap();
    assert!(report.outcome().complete());
    let wire_text = report.to_json();
    assert!(wire_text.len() <= 22000);
    let wire: Value = serde_json::from_str(&wire_text).unwrap();
    assert_eq!(wire["result"]["json_present"], false);
    assert_eq!(wire["result"]["state"], "partial");
    assert!(
        wire["result"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |omission| omission["field"] == "result.json" && omission["reason"] == "byte_limit"
            )
    );
    let human = report.to_human();
    assert!(human.len() <= 22000);
    assert!(human.contains("byte_limit"));
    assert!(!human.contains("zzzzz"));
}

#[test]
fn same_document_fragment_decoding_preserves_escaped_name_identity() {
    use esdieff::{EffectiveRequest, Lookup};
    let text =
        br##"{"sdfData":{"A/B~C":{"maximum":0.1},"Use":{"sdfRef":"#/sdfData/A%7E1B%7E0C"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("fragment", text)]).unwrap();
    let id = catalogue.source_id("fragment").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(model.complete());
    let Lookup::Found(use_data) = model.definition(Class::Data, "Use") else {
        panic!("Use")
    };
    assert_eq!(
        use_data
            .data_description()
            .require()
            .unwrap()
            .number("maximum")
            .require()
            .unwrap()
            .lexeme(),
        "0.1"
    );
    let origin = model.explanation("/sdfData/Use/maximum").require().unwrap();
    assert_eq!(
        origin
            .sources()
            .next()
            .unwrap()
            .pointer(8192)
            .require()
            .unwrap(),
        "/sdfData/A~1B~0C/maximum"
    );
}

#[test]
fn selected_scalar_retains_the_established_role_of_its_composed_property_ancestor() {
    use esdieff::{EffectiveRequest, Field, OriginKind};
    let text=br##"{"sdfProperty":{"Base":{"minimum":1},"Use":{"sdfRef":"#/sdfProperty/Base","maximum":4}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("ancestor", text)]).unwrap();
    let id = catalogue.source_id("ancestor").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfProperty/Use/maximum"))
        .unwrap();
    assert!(model.complete());
    assert_eq!(
        model
            .json()
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "4"
    );
    assert!(
        matches!(model.default_explanation("/sdfProperty/Use/readable"),Field::Present(value)if value.kind()==OriginKind::Default)
    );
}

#[test]
fn selected_opaque_maps_retain_json_without_granting_typed_class_roles() {
    use esdieff::{EffectiveRequest, Field};
    let text=br##"{"opaque":{"sdfProperty":{"fake":{"type":"number"}}},"sdfData":{"Good":{"type":"number"}}}"##;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("opaque-selection", text)]).unwrap();
    let id = catalogue.source_id("opaque-selection").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/opaque"))
        .unwrap();
    assert!(model.complete());
    assert!(matches!(
        model.json().require().unwrap().get("sdfProperty"),
        Field::Present(_)
    ));
    assert!(matches!(
        model.definitions(Class::Property),
        Field::Unavailable(_)
    ));

    let text=br##"{"sdfProperty":{"Use":{"sdfProperty":{"fake":{"type":"number"}}}},"sdfObject":{"Container":{"sdfProperty":{"real":{"type":"number"}}}}}"##;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("selected-containment", text)]).unwrap();
    let id = catalogue.source_id("selected-containment").unwrap();
    let property = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfProperty/Use"))
        .unwrap();
    assert!(matches!(
        property.json().require().unwrap().get("sdfProperty"),
        Field::Present(_)
    ));
    assert!(matches!(
        property.definitions(Class::Property),
        Field::Unavailable(_)
    ));
    assert!(matches!(property.definitions(Class::Object), Field::Absent));
    let object = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfObject/Container"))
        .unwrap();
    let Field::Present(mut properties) = object.definitions(Class::Property) else {
        panic!("established object children")
    };
    assert!(matches!(
        properties.next(),
        Some(("real", Field::Present(_)))
    ));
    assert!(properties.next().is_none());
}

#[test]
fn effective_result_storage_exhaustion_preserves_source_findings_and_limit_totals() {
    use esdieff::{CheckRequest, Limits};
    let text = br##"{"info":{"license":7},"sdfData":{"Use":{"sdfRef":"#/sdfData/missing"}}}"##;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("storage", text)],
        Limits::default().effective_allocation_bytes(1),
    )
    .unwrap();
    let id = catalogue.source_id("storage").unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "operation.limit"
                && item["message"] == "derived allocation limit"
                && item["primary"]["kind"] == "effective"
                && item["primary"]["root"] == id.get()
                && item["primary"]["pointer"] == ""
                && item["primary"].get("span").is_none())
    );
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "metadata.string")
    );
    assert_eq!(
        wire["request"]["scope"]["assessed"],
        serde_json::json!([id.get()])
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    let expected = wire["diagnostics"]["retained"].as_u64().unwrap()
        + wire["diagnostics"]["omitted"].as_u64().unwrap();
    for limits in [
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
    ] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("storage", text)],
            limits.effective_allocation_bytes(1),
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(
            wire["diagnostics"]["retained"].as_u64().unwrap()
                + wire["diagnostics"]["omitted"].as_u64().unwrap(),
            expected
        );
        assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() > 0);
    }
    let inputs = [
        SourceInput::supplied("first", text),
        SourceInput::supplied("second", text),
        SourceInput::supplied("third", text),
        SourceInput::supplied("fourth", text),
        SourceInput::supplied("fifth", text),
        SourceInput::supplied("sixth", text),
        SourceInput::supplied("seventh", text),
        SourceInput::supplied("eighth", text),
        SourceInput::supplied("ninth", text),
    ];
    let catalogue = Catalogue::with_limits(
        &inputs,
        Limits::default().effective_allocation_bytes(20_000),
    )
    .unwrap();
    let first = catalogue.source_id("first").unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(report.outcome().errors(), 9);
    assert_eq!(wire["sources"].as_array().unwrap().len(), 9);
    assert_eq!(
        wire["request"]["scope"]["assessed"]
            .as_array()
            .unwrap()
            .len(),
        9
    );
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "reference.unavailable"
                && item["primary"]["root"] == first.get()
                && item["primary"]["pointer"] == "/sdfData/Use")
    );
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "operation.limit")
    );
}

#[test]
fn copied_full_data_required_pointers_preserve_syntax_and_independent_typed_subviews() {
    use esdieff::{BaseType, DataType, EffectiveRequest, Field, Lookup};
    let text=br##"{"sdfObject":{"Base":{"sdfAction":{"A":{"sdfInputData":{"type":"number","minimum":1,"sdfRequired":["#/sdfObject/Base/sdfAction/A"]},"sdfOutputData":{"type":"number","minimum":1,"sdfRequired":["#/sdfObject/Base/sdfAction/A"]}}},"sdfEvent":{"E":{"sdfOutputData":{"type":"number","minimum":1,"sdfRequired":["#/sdfObject/Base/sdfEvent/E"]}}},"sdfProperty":{"good":{"type":"boolean"}}},"Use":{"sdfRef":"#/sdfObject/Base"}}}"##;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("full-required", text)]).unwrap();
    let id = catalogue.source_id("full-required").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(!model.complete());
    let Lookup::Found(use_object) = model.definition(Class::Object, "Use") else {
        panic!("Use")
    };
    let good = use_object
        .children(Class::Property)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert!(good.json().capability_complete());
    for (class, name, field) in [
        (Class::Action, "A", "sdfInputData"),
        (Class::Action, "A", "sdfOutputData"),
        (Class::Event, "E", "sdfOutputData"),
    ] {
        let affordance = use_object
            .children(class)
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .require()
            .unwrap();
        let data = if field == "sdfInputData" {
            affordance.input().require().unwrap()
        } else {
            affordance.output().require().unwrap()
        };
        assert!(!data.json().capability_complete());
        assert_eq!(
            data.data_type().require().unwrap(),
            DataType::Known(BaseType::Number)
        );
        assert_eq!(data.number("minimum").require().unwrap().lexeme(), "1");
        assert_eq!(
            data.json()
                .get("sdfRequired")
                .require()
                .unwrap()
                .at(0)
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap(),
            format!("#/sdfObject/Base/{}/{name}", class.keyword())
        );
        let pointer = format!("/sdfObject/Use/{}/{name}/{field}", class.keyword());
        let report = catalogue
            .inspect(InspectRequest::new(id, &pointer).view(View::Effective))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(!report.outcome().complete());
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == "interpretation.unresolved"
                    && item["primary"]["pointer"] == format!("{pointer}/sdfRequired"))
        );
        let source = catalogue.source(id).unwrap();
        let Lookup::Found(base) = source.definition(Class::Object, "Base") else {
            panic!("Base")
        };
        let affordance = base.children(class).require().unwrap().next().unwrap().1;
        let data = if field == "sdfInputData" {
            affordance.input().require().unwrap()
        } else {
            affordance.output().require().unwrap()
        };
        assert!(data.json().capability_complete());
        assert!(matches!(data.json().get("sdfRequired"), Field::Present(_)));
    }
}

#[test]
fn full_data_required_guards_preserve_independent_syntax_and_exclude_direct_items() {
    use esdieff::{EffectiveRequest, Lookup};
    for required in ["[\"A\"]", "true", "[\"#/sdfAction/Outside\"]"] {
        let text = format!(
            r##"{{"sdfAction":{{"Outside":{{}}}},"sdfObject":{{"Base":{{"sdfAction":{{"A":{{"sdfOutputData":{{"type":"number","sdfRequired":{required}}}}}}},"sdfEvent":{{"E":{{"sdfOutputData":{{"type":"number","sdfRequired":{required}}}}}}}}},"Use":{{"sdfRef":"#/sdfObject/Base"}}}}}}"##
        );
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
            "independent-full-required",
            text.as_bytes(),
        )])
        .unwrap();
        let id = catalogue.source_id("independent-full-required").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
        assert!(model.complete());
        let Lookup::Found(use_object) = model.definition(Class::Object, "Use") else {
            panic!("Use")
        };
        for class in [Class::Action, Class::Event] {
            let data = use_object
                .children(class)
                .require()
                .unwrap()
                .next()
                .unwrap()
                .1
                .require()
                .unwrap()
                .output()
                .require()
                .unwrap();
            assert_eq!(
                data.json()
                    .get("sdfRequired")
                    .require()
                    .unwrap()
                    .source_json()
                    .raw_json(),
                required
            );
        }
    }
    let text=br##"{"sdfObject":{"Base":{"sdfAction":{"A":{"sdfOutputData":{"type":"array","items":{"type":"number","sdfRequired":["#/sdfObject/Base/sdfAction/A"]}}}}},"Use":{"sdfRef":"#/sdfObject/Base"}}}"##;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("direct-required-syntax", text)]).unwrap();
    let id = catalogue.source_id("direct-required-syntax").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
    assert!(model.complete());
    let Lookup::Found(use_object) = model.definition(Class::Object, "Use") else {
        panic!("Use")
    };
    let items = use_object
        .children(Class::Action)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap()
        .output()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert_eq!(items.role(), esdieff::DataRole::DirectItem);
    assert_eq!(
        items
            .json()
            .get("sdfRequired")
            .require()
            .unwrap()
            .at(0)
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "#/sdfObject/Base/sdfAction/A"
    );
}
