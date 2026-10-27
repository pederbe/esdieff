use esdieff::{
    Catalogue, Class, Edit, EditRequest, Fidelity, Generation, JsonBuilder, SourceInput,
};
use esdieff::{Disposition, EditOperation, EffectiveRequest, Field, Limits, Lookup, SourceRole};

const ORIGINAL: &[u8] = br#"{"info":{"title":"Original"},"sdfObject":{"Sensor":{"description":"parent","sdfProperty":{"reading":{"description":"old","label":"Reading","type":"number","maximum":1.2300e+02},"other":{"description":"unchanged"}}}}}"#;

#[test]
fn nested_annotations_change_without_mutating_original_or_other_content() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", ORIGINAL)
        .acquisition_uri("https://example.org/source")
        .version_token("revision-one")
        .digest("original-digest")])
    .unwrap();
    let original = catalogue.source_id("base").unwrap();
    let result = EditRequest::new(catalogue, "correction", original, "revised")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor/sdfProperty/reading",
                "description",
                JsonBuilder::new().text("corrected").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_edit(Edit::remove("/sdfObject/Sensor/sdfProperty/reading", "label").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(result.generation(), Generation::Complete);
    assert_eq!(result.fidelity(), Fidelity::Established);
    assert!(result.document_deliverable());
    assert_eq!(result.original_source().id(), original);
    assert_eq!(result.original_source().raw_bytes(), ORIGINAL);
    assert_eq!(result.original_source().digest(), Some("original-digest"));
    assert!(result.catalogue().source(original).is_none());
    let generated = result
        .catalogue()
        .source(result.generated_source())
        .unwrap();
    assert_eq!(generated.acquisition_uri(), None);
    assert_eq!(generated.version_token(), None);
    assert_eq!(generated.digest(), None);
    assert_ne!(generated.id(), original);
    let root = generated.json().require().unwrap();
    let sensor = root
        .get("sdfObject")
        .require()
        .unwrap()
        .get("Sensor")
        .require()
        .unwrap();
    assert_eq!(
        sensor
            .get("description")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "parent"
    );
    let properties = sensor.get("sdfProperty").require().unwrap();
    let reading = properties.get("reading").require().unwrap();
    assert_eq!(
        reading
            .get("description")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "corrected"
    );
    assert!(reading.get("label").present().is_none());
    assert_eq!(
        reading
            .get("maximum")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "1.2300e+02"
    );
    assert_eq!(
        properties
            .get("other")
            .require()
            .unwrap()
            .get("description")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "unchanged"
    );
    assert!(
        generated
            .definitions(Class::Object)
            .require()
            .unwrap()
            .next()
            .is_some()
    );
    assert!(result.generated_document().unwrap().ends_with(b"\n"));
}

fn request(bytes: &[u8]) -> EditRequest {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", bytes)]).unwrap();
    let selected = catalogue.source_id("base").unwrap();
    EditRequest::new(catalogue, "correction", selected, "revised").unwrap()
}

#[test]
fn annotations_on_every_named_class_keep_null_distinct_from_removal() {
    for class in [
        Class::Thing,
        Class::Object,
        Class::Property,
        Class::Action,
        Class::Event,
        Class::Data,
    ] {
        for quality in ["label", "description", "$comment"] {
            let bytes = format!(
                r#"{{"{}":{{"a":{{"{quality}":"old"}},"b":{{"{quality}":"remove"}}}}}}"#,
                class.keyword()
            );
            let result = request(bytes.as_bytes())
                .with_edit(
                    Edit::replace(
                        &format!("/{}/a", class.keyword()),
                        quality,
                        JsonBuilder::new().null().unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap()
                .with_edit(Edit::remove(&format!("/{}/b", class.keyword()), quality).unwrap())
                .unwrap()
                .edit()
                .unwrap();
            assert!(
                result.document_deliverable(),
                "{} {quality}",
                class.keyword()
            );
            let generated = result
                .catalogue()
                .source(result.generated_source())
                .unwrap();
            let a = generated.definition(class, "a");
            let Lookup::Found(a) = a else {
                panic!("named a")
            };
            assert_eq!(
                a.json().get(quality).require().unwrap().kind(),
                esdieff::JsonKind::Null
            );
            let Lookup::Found(b) = generated.definition(class, "b") else {
                panic!("named b")
            };
            assert!(matches!(b.json().get(quality), Field::Absent));
            assert_eq!(
                result
                    .target(0)
                    .require()
                    .unwrap()
                    .request_location()
                    .pointer()
                    .require()
                    .unwrap(),
                "/edits/0"
            );
            assert_eq!(
                result
                    .target(0)
                    .require()
                    .unwrap()
                    .request_location()
                    .span(),
                None
            );
            assert_eq!(
                result
                    .target(0)
                    .require()
                    .unwrap()
                    .generated_member()
                    .require()
                    .unwrap()
                    .kind(),
                esdieff::JsonKind::Null
            );
            assert!(matches!(
                result.target(1).require().unwrap().generated_member(),
                Field::Absent
            ));
        }
    }
}

#[test]
fn source_pointer_selects_special_decoded_names_and_preserves_opaque_numbers() {
    let bytes = "{\"sdfObject\":{\"Room / ~ # ø\":{\"label\":\"old\",\"vendor:payload\":{\"v\":[-0.00,0.10,1.2300e+02,10e999999,10e-1000000]},\"sdfProperty\":{\"sibling\":{\"const\":{\"number\":0.10}}}}}}";
    let result = request(bytes.as_bytes())
        .with_edit(
            Edit::replace(
                "/sdfObject/Room ~1 ~0 # ø",
                "label",
                JsonBuilder::new().text("revised\nø").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .edit()
        .unwrap();
    assert!(result.document_deliverable());
    let text = std::str::from_utf8(result.generated_document().unwrap()).unwrap();
    for lexeme in ["-0.00", "0.10", "1.2300e+02", "10e999999", "10e-1000000"] {
        assert!(text.contains(lexeme));
    }
    let generated = result
        .catalogue()
        .source(result.generated_source())
        .unwrap();
    let Lookup::Found(object) = generated.definition(Class::Object, "Room / ~ # ø") else {
        panic!("decoded object")
    };
    assert_eq!(object.label().require().unwrap(), "revised\nø");
    let location = result
        .request_origin(object.json().get("label").require().unwrap())
        .require()
        .unwrap();
    assert_eq!(location.pointer().require().unwrap(), "/edits/0/value");
    assert_eq!(location.span(), None);
    let acquired = Catalogue::from_inputs(&[SourceInput::supplied(
        "reuse",
        result.generated_document().unwrap(),
    )])
    .unwrap();
    let source = acquired
        .source(acquired.source_id("reuse").unwrap())
        .unwrap();
    let Lookup::Found(object) = source.definition(Class::Object, "Room / ~ # ø") else {
        panic!("reacquired object")
    };
    assert_eq!(object.label().require().unwrap(), "revised\nø");
    assert_eq!(
        object
            .json()
            .get("vendor:payload")
            .require()
            .unwrap()
            .get("v")
            .require()
            .unwrap()
            .at(0)
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "-0.00"
    );
}

#[test]
fn stale_conflicting_or_unsupported_requests_are_refused() {
    assert!(request(ORIGINAL).edit().is_err());
    assert!(Edit::remove("not-a-pointer", "label").is_err());
    assert!(Edit::remove("/sdfObject/a~2", "label").is_err());
    assert!(Edit::new("/sdfObject/a", "label", EditOperation::Replace, None).is_err());
    assert!(
        Edit::new(
            "/sdfObject/a",
            "label",
            EditOperation::Remove,
            Some(JsonBuilder::new().null().unwrap())
        )
        .is_err()
    );
    for (owner, quality) in [
        ("/sdfObject/missing", "label"),
        ("/sdfObject/Sensor", "label"),
        ("/info", "title"),
        ("/sdfObject/Sensor", "sdfRef"),
        ("/sdfObject/Sensor/sdfProperty/reading", "readable"),
    ] {
        assert!(
            request(ORIGINAL)
                .with_edit(Edit::remove(owner, quality).unwrap())
                .unwrap()
                .edit()
                .is_err(),
            "{owner} {quality}"
        );
    }
    let duplicate = request(ORIGINAL)
        .with_edit(Edit::remove("/sdfObject/Sensor", "description").unwrap())
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor",
                "description",
                JsonBuilder::new().null().unwrap(),
            )
            .unwrap(),
        );
    assert!(duplicate.is_err());
    let source = br#"{"sdfProperty":{"p":{"description":"old","items":{"description":"item"},"properties":{"x":{"description":"map"}},"sdfChoice":{"x":{"description":"choice"}},"opaque":{"sdfData":{"x":{"description":"opaque"}}}}},"sdfAction":{"a":{"sdfInputData":{"description":"input"},"sdfOutputData":{"description":"output"}}}}"#;
    for owner in [
        "/sdfProperty/p/items",
        "/sdfProperty/p/properties/x",
        "/sdfProperty/p/sdfChoice/x",
        "/sdfProperty/p/opaque/sdfData/x",
        "/sdfAction/a/sdfInputData",
        "/sdfAction/a/sdfOutputData",
    ] {
        assert!(
            request(source)
                .with_edit(Edit::remove(owner, "description").unwrap())
                .unwrap()
                .edit()
                .is_err(),
            "{owner}"
        );
    }
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", ORIGINAL),
        SourceInput::supplied("sample", b"{}").sample(),
    ])
    .unwrap();
    let sample = catalogue.source_id("sample").unwrap();
    assert!(EditRequest::new(catalogue, "request", sample, "generated").is_err());
}

#[test]
fn composition_boundaries_are_refused_for_any_reference_shape() {
    for reference in ["null", "123", "\"#/sdfObject/base\"", "{}"] {
        let bytes = format!(
            r#"{{"sdfObject":{{"a":{{"sdfRef":{reference},"description":"override","sdfProperty":{{"p":{{"description":null}}}}}},"base":{{"description":"base"}}}}}}"#
        );
        for owner in ["/sdfObject/a", "/sdfObject/a/sdfProperty/p"] {
            assert!(
                request(bytes.as_bytes())
                    .with_edit(Edit::remove(owner, "description").unwrap())
                    .unwrap()
                    .edit()
                    .is_err(),
                "{reference} {owner}"
            );
        }
    }
    let bytes =
        br##"{"sdfData":{"base":{"description":"old"},"consumer":{"sdfRef":"#/sdfData/base"}}}"##;
    assert!(
        request(bytes)
            .with_edit(Edit::remove("/sdfData/consumer", "description").unwrap())
            .unwrap()
            .edit()
            .is_err()
    );
}

#[test]
fn substitution_preserves_other_ids_and_reference_resolution() {
    let original = br##"{"namespace":{"n":"https://example.org/model"},"defaultNamespace":"n","sdfData":{"base":{"description":"old","type":"string"},"local":{"sdfRef":"#/sdfData/base"}}}"##;
    let consumer = br##"{"namespace":{"n":"https://example.org/model"},"sdfData":{"remote":{"sdfRef":"n:#/sdfData/base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", original),
        SourceInput::supplied("consumer", consumer).digest("consumer-digest"),
    ])
    .unwrap();
    let selected = catalogue.source_id("base").unwrap();
    let consumer_id = catalogue.source_id("consumer").unwrap();
    let result = EditRequest::new(catalogue, "change", selected, "generated")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfData/base",
                "description",
                JsonBuilder::new().text("revised").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .edit()
        .unwrap();
    assert!(result.catalogue().source(selected).is_none());
    assert_eq!(
        result.catalogue().source(consumer_id).unwrap().digest(),
        Some("consumer-digest")
    );
    assert_eq!(result.catalogue().sources().count(), 2);
    assert_eq!(result.catalogue().retained_sources().count(), 3);
    for (root, name) in [
        (result.generated_source(), "local"),
        (consumer_id, "remote"),
    ] {
        let model = result
            .catalogue()
            .effective(EffectiveRequest::new(root))
            .unwrap();
        let Lookup::Found(definition) = model.definition(Class::Data, name) else {
            panic!("effective definition")
        };
        assert_eq!(definition.description().require().unwrap(), "revised");
    }
    let json: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(json["version"], "1.2");
    assert_eq!(json["operation"], "edit");
    assert_eq!(json["request"]["assessment"], serde_json::Value::Null);
    assert_eq!(json["request"]["inventory"], serde_json::Value::Null);
    assert_eq!(json["coverage"]["requested"]["count_unit"], "editing");
    assert_eq!(json["coverage"]["requested"]["assessed"], 1);
    assert_eq!(
        json["result"]["substitution"]["original_source"],
        selected.get()
    );
    assert_eq!(
        json["result"]["substitution"]["generated_source"],
        result.generated_source().get()
    );
    assert_eq!(json["sources"].as_array().unwrap().len(), 3);
    assert_eq!(json["result"]["checking"]["state"], "not_requested");
    assert!(result.to_human().contains("passed"));
    assert!(result.to_human().contains("Checking: not requested"));
}

#[test]
fn source_reuse_does_not_reuse_an_id_or_discard_attempt_budgets() {
    let result = request(ORIGINAL)
        .with_edit(Edit::remove("/sdfObject/Sensor", "description").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    let first = result.generated_source();
    let catalogue = result.into_catalogue();
    let second = EditRequest::new(catalogue, "second", first, "second-result")
        .unwrap()
        .with_edit(Edit::remove("/sdfObject/Sensor/sdfProperty/reading", "label").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    assert_ne!(second.generated_source(), first);
    assert_eq!(second.catalogue().retained_sources().count(), 3);
    assert!(second.document_deliverable());
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("base", ORIGINAL)],
        Limits::default().documents(2),
    )
    .unwrap();
    let selected = catalogue.source_id("base").unwrap();
    let result = EditRequest::new(catalogue, "first", selected, "revised")
        .unwrap()
        .with_edit(Edit::remove("/sdfObject/Sensor", "description").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    let generated = result.generated_source();
    assert!(EditRequest::new(result.into_catalogue(), "second", generated, "again").is_err());
}

#[test]
fn incomplete_source_and_bounded_generation_preserve_report_and_original() {
    for source in [
        SourceInput::failed("base"),
        SourceInput::supplied("base", b"{bad"),
        SourceInput::supplied("base", br#"{"sdfObject":{"a":{"label":"\ud800"}}}"#),
    ] {
        let catalogue = Catalogue::take_inputs(vec![source]).unwrap();
        let selected = catalogue.source_id("base").unwrap();
        let result = EditRequest::new(catalogue, "request", selected, "result")
            .unwrap()
            .with_edit(Edit::remove("/sdfObject/a", "label").unwrap())
            .unwrap()
            .edit()
            .unwrap();
        assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
        assert!(!result.document_deliverable());
        assert_eq!(result.fidelity(), Fidelity::Undetermined);
        assert!(
            result
                .diagnostics()
                .conditions()
                .any(|condition| condition.code().starts_with("edit."))
        );
    }
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("base", ORIGINAL)],
        Limits::default()
            .catalogue_bytes(ORIGINAL.len() + 10)
            .diagnostic_bytes(1)
            .pointer_output_bytes(1),
    )
    .unwrap();
    let selected = catalogue.source_id("base").unwrap();
    let result = EditRequest::new(catalogue, "request", selected, "result")
        .unwrap()
        .with_edit(Edit::remove("/sdfObject/Sensor", "description").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(result.generation(), Generation::Partial);
    assert!(!result.document_deliverable());
    assert!(result.generated_document().is_some());
    assert_eq!(result.original_source().raw_bytes(), ORIGINAL);
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["undetermined"], 1);
    assert_eq!(
        wire["result"]["substitution"]["original_source"],
        selected.get()
    );
    assert_eq!(wire["sources"].as_array().unwrap().len(), 2);
    assert!(wire["diagnostics"]["truncated"].as_bool().unwrap());
    assert_eq!(wire["result"]["documents"][0]["content_complete"], false);
    assert_eq!(
        result
            .catalogue()
            .source(result.generated_source())
            .unwrap()
            .role(),
        SourceRole::Generated
    );
}

#[test]
fn each_finite_work_phase_keeps_its_own_incomplete_cause() {
    let source =
        br#"{"sdfObject":{"a":{"description":"old","opaque":{"values":[0.10,1.2300e+02,-0.00]}}}}"#;
    for (work, generation, code) in [
        (200, Generation::Unavailable, "edit.target_limit"),
        (500, Generation::Partial, "edit.generation_limit"),
        (800, Generation::Partial, "edit.generated_decode_limit"),
        (1000, Generation::Complete, "edit.fidelity_limit"),
    ] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("base", source)],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let selected = catalogue.source_id("base").unwrap();
        let result = EditRequest::new(catalogue, "work", selected, "out")
            .unwrap()
            .with_edit(
                Edit::replace(
                    "/sdfObject/a",
                    "description",
                    JsonBuilder::new().text("new").unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
            .edit()
            .unwrap();
        assert_eq!(result.generation(), generation);
        assert_eq!(result.fidelity(), Fidelity::Undetermined);
        assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
        assert!(!result.document_deliverable());
        assert!(
            result
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == code),
            "work {work}"
        );
    }
    for (numeric, code) in [(15, "edit.source_unavailable"), (25, "edit.fidelity_limit")] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("base", source)],
            Limits::default().numeric_work(numeric),
        )
        .unwrap();
        let selected = catalogue.source_id("base").unwrap();
        let result = EditRequest::new(catalogue, "numeric", selected, "out")
            .unwrap()
            .with_edit(
                Edit::replace(
                    "/sdfObject/a",
                    "description",
                    JsonBuilder::new().text("new").unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
            .edit()
            .unwrap();
        assert_eq!(result.fidelity(), Fidelity::Undetermined);
        assert!(!result.document_deliverable());
        assert!(
            result
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == code)
        );
    }
}

#[test]
fn replacement_literal_numbers_replay_exactly_without_model_shape_repair() {
    let input = br#"{"sdfData":{"a":{"description":"old"}}}"#;
    let replacement = || {
        JsonBuilder::new()
            .map(vec![(
                "literal",
                JsonBuilder::new()
                    .array(vec![
                        JsonBuilder::new().number("-0.00").unwrap(),
                        JsonBuilder::new().number("10e999999").unwrap(),
                    ])
                    .unwrap(),
            )])
            .unwrap()
    };
    let execute = || {
        request(input)
            .with_edit(Edit::replace("/sdfData/a", "description", replacement()).unwrap())
            .unwrap()
            .edit()
            .unwrap()
    };
    let first = execute();
    let second = execute();
    assert!(first.document_deliverable());
    assert_eq!(first.generated_document(), second.generated_document());
    let text = std::str::from_utf8(first.generated_document().unwrap()).unwrap();
    assert!(text.contains("-0.00"));
    assert!(text.contains("10e999999"));
    assert_eq!(
        first.outcome().disposition(),
        second.outcome().disposition()
    );
    let acquired = Catalogue::from_inputs(&[SourceInput::supplied(
        "reacquired",
        first.generated_document().unwrap(),
    )])
    .unwrap();
    assert_eq!(
        acquired
            .check(esdieff::CheckRequest::new())
            .unwrap()
            .outcome()
            .disposition(),
        Disposition::Failed
    );
}

#[test]
fn a_genuine_remaining_contributor_collision_stays_ambiguous() {
    let original = br##"{"namespace":{"n":"https://example.org/model"},"defaultNamespace":"n","sdfData":{"base":{"description":"old"}}}"##;
    let consumer = br##"{"namespace":{"n":"https://example.org/model"},"sdfData":{"remote":{"sdfRef":"n:#/sdfData/base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", original),
        SourceInput::supplied("explicit-alias", original),
        SourceInput::supplied("consumer", consumer),
    ])
    .unwrap();
    let selected = catalogue.source_id("base").unwrap();
    let alias = catalogue.source_id("explicit-alias").unwrap();
    let consumer = catalogue.source_id("consumer").unwrap();
    let result = EditRequest::new(catalogue, "change", selected, "generated")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfData/base",
                "description",
                JsonBuilder::new().text("revised").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .edit()
        .unwrap();
    assert!(result.document_deliverable());
    assert_eq!(
        result.catalogue().source(alias).unwrap().raw_bytes(),
        original
    );
    let check = result
        .catalogue()
        .check(esdieff::CheckRequest::new().root(consumer))
        .unwrap();
    assert_eq!(check.outcome().disposition(), Disposition::Undetermined);
    assert!(
        check
            .diagnostics()
            .conditions()
            .any(|condition| condition.code().contains("ambiguous"))
    );
}

#[test]
fn attempt_selector_request_and_report_reservations_are_before_processing() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("base", ORIGINAL)],
        Limits::default().report_bytes(30000),
    )
    .unwrap();
    let selected = catalogue.source_id("base").unwrap();
    assert!(EditRequest::new(catalogue, "request", selected, "new").is_err());
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("base", ORIGINAL)],
        Limits::default().pointer_bytes(10),
    )
    .unwrap();
    let selected = catalogue.source_id("base").unwrap();
    assert!(
        EditRequest::new(catalogue, "request", selected, "new")
            .unwrap()
            .with_edit(Edit::remove("/sdfObject/Sensor", "description").unwrap())
            .is_err()
    );
    let mut many = request(ORIGINAL);
    for index in 0..256 {
        many = many
            .with_edit(Edit::remove(&format!("/sdfObject/{index}"), "description").unwrap())
            .unwrap();
    }
    assert!(
        many.with_edit(Edit::remove("/sdfObject/overflow", "description").unwrap())
            .is_err()
    );
    let result = request(ORIGINAL)
        .with_edit(Edit::remove("/sdfObject/Sensor", "description").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    let generated = result.generated_source();
    assert!(EditRequest::new(result.into_catalogue(), "again", generated, "base").is_err());
}

#[test]
fn acquired_original_path_is_retained_and_never_attached_to_generated_bytes() {
    let path = std::env::temp_dir().join(format!(
        "esdieff-edit-foundation-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, ORIGINAL).unwrap();
    let catalogue = Catalogue::from_inputs(&[SourceInput::file("base", &path)]).unwrap();
    let selected = catalogue.source_id("base").unwrap();
    let result = EditRequest::new(catalogue, "request", selected, "generated")
        .unwrap()
        .with_edit(Edit::remove("/sdfObject/Sensor", "description").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        result.original_source().raw_path_identity(),
        Some(path.as_os_str().as_encoded_bytes())
    );
    assert_eq!(result.original_source().raw_bytes(), ORIGINAL);
    assert_eq!(
        result
            .catalogue()
            .source(result.generated_source())
            .unwrap()
            .raw_path_identity(),
        None
    );
    assert_eq!(
        result.original_source().acquisition(),
        esdieff::Acquisition::Acquired
    );
    assert_eq!(
        result
            .catalogue()
            .source(result.generated_source())
            .unwrap()
            .acquisition(),
        esdieff::Acquisition::Generated
    );
}

#[test]
fn ordinary_authoring_after_substitution_allocates_a_fresh_attempt() {
    let bytes = br#"{"namespace":{"n":"https://example.org/reusable"},"defaultNamespace":"n","sdfObject":{"Sensor":{"description":"old"}}}"#;
    let edited = request(bytes)
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor",
                "description",
                JsonBuilder::new().text("new").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .edit()
        .unwrap();
    let revised = edited.generated_source();
    let result = esdieff::AuthorRequest::new(
        edited.into_catalogue(),
        "assembly",
        "assembly-output",
        "Thing",
    )
    .unwrap()
    .namespace("reused", "https://example.org/reusable")
    .unwrap()
    .object(
        esdieff::AuthorObject::new("sensor", revised, "/sdfObject/Sensor", "reused", false)
            .unwrap(),
    )
    .unwrap()
    .author()
    .unwrap();
    assert!(result.document_deliverable());
    assert_ne!(result.generated_source(), revised);
    assert_eq!(result.generated_source().get(), 3);
    assert_eq!(result.catalogue().sources().count(), 2);
    assert_eq!(result.catalogue().retained_sources().count(), 3);
    let model = result
        .catalogue()
        .effective(EffectiveRequest::new(result.generated_source()))
        .unwrap();
    let Lookup::Found(thing) = model.definition(Class::Thing, "Thing") else {
        panic!("authored Thing")
    };
    let (_, sensor) = thing
        .children(Class::Object)
        .require()
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(
        sensor.require().unwrap().description().require().unwrap(),
        "new"
    );
}
