use esdieff::{
    Assessment, AuthorRequest, Catalogue, Class, Disposition, Edit, EditRequest, EffectiveRequest,
    Fidelity, Generation, JsonBuilder, Lookup, OriginKind, SourceInput,
};

const BASE: &[u8] = include_bytes!("../examples/context-authoring/base.sdf.json");
const RECIPE: &[u8] = include_bytes!("../examples/context-authoring/recipe.json");
const EDITS: &[u8] = include_bytes!("../examples/context-authoring/edits.json");
const CALIBRATION: &str = "/sdfThing/Installation/sdfObject/sensor/sdfContext/calibration";

fn author(source: &[u8], recipe: &[u8]) -> esdieff::AuthorResult {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", source)]).unwrap();
    AuthorRequest::from_plan(catalogue, recipe)
        .unwrap()
        .checking(Assessment::FullBaseline)
        .unwrap()
        .author()
        .unwrap()
}

#[test]
fn authored_context_edits_preserve_other_content_and_relate_targets_to_intent() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", BASE)]).unwrap();
    let result = EditRequest::from_plan(catalogue, EDITS)
        .unwrap()
        .checking(Assessment::FullBaseline)
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(result.fidelity(), Fidelity::Established);
    assert!(result.document_deliverable(), "{}", result.to_json());
    assert_eq!(result.original_source().raw_bytes(), BASE);
    let generated: serde_json::Value =
        serde_json::from_slice(result.generated_document().unwrap()).unwrap();
    let mut expected: serde_json::Value = serde_json::from_slice(BASE).unwrap();
    let site = &mut expected["sdfObject"]["Sensor"]["sdfContext"]["siteName"];
    site["description"] = "Site name chosen during installation".into();
    site["minLength"] = 2.into();
    site.as_object_mut().unwrap().remove("label");
    site.as_object_mut().unwrap().remove("const");
    assert_eq!(generated, expected);
    let target = result.target(0).require().unwrap();
    assert_eq!(
        target.original_member().text().require().unwrap(),
        "Installation name"
    );
    let replacement = target.generated_member().require().unwrap();
    assert_eq!(
        replacement.text().require().unwrap(),
        "Site name chosen during installation"
    );
    let origin = result.request_origin(replacement).require().unwrap();
    assert_eq!(origin.pointer().require().unwrap(), "/edits/0/value");
    let span = origin.span().unwrap();
    assert_eq!(
        &EDITS[span.start as usize..span.end as usize],
        br#""Site name chosen during installation""#
    );
    assert!(
        result
            .target(1)
            .require()
            .unwrap()
            .generated_member()
            .present()
            .is_none()
    );
    assert!(
        result
            .to_json()
            .contains("draft-ietf-asdf-sdf-nonaffordance-06")
    );
}

#[test]
fn context_refinements_preserve_non_affordance_meaning_and_transformation_origins() {
    let result = author(BASE, RECIPE);
    assert_eq!(result.generation(), Generation::Complete);
    assert_eq!(
        result.fidelity(),
        Fidelity::Established,
        "{}",
        result.to_json()
    );
    assert_eq!(
        result.outcome().disposition(),
        Disposition::Passed,
        "{}",
        result.to_json()
    );
    assert!(result.document_deliverable());
    let base = result.catalogue().source_id("base").unwrap();
    assert_eq!(result.catalogue().source(base).unwrap().raw_bytes(), BASE);
    let document = std::str::from_utf8(result.generated_document().unwrap()).unwrap();
    assert!(document.contains("reuse:#/sdfObject/Sensor"));
    assert!(document.contains("2.50"));

    let effective = result
        .catalogue()
        .effective(
            EffectiveRequest::new(result.generated_source())
                .pointer("/sdfThing/Installation/sdfObject/sensor"),
        )
        .unwrap();
    let Lookup::Found(calibration) = effective.definition(Class::Context, "calibration") else {
        panic!("Context calibration")
    };
    assert_eq!(calibration.class(), Class::Context);
    assert!(calibration.readable().present().is_none());
    assert!(calibration.writable().present().is_none());
    assert!(calibration.observable().present().is_none());
    assert_eq!(
        calibration
            .data_description()
            .require()
            .unwrap()
            .minimum()
            .require()
            .unwrap()
            .lexeme(),
        "-10"
    );
    let maximum = effective
        .explanation(&format!("{CALIBRATION}/maximum"))
        .require()
        .unwrap();
    assert_eq!(maximum.kind(), OriginKind::Override);
    let patch = maximum.sources().next().unwrap();
    assert_eq!(patch.number().require().unwrap().lexeme(), "2.50");
    let origin = result.request_origin(patch).require().unwrap();
    assert_eq!(
        origin.pointer().require().unwrap(),
        "/objects/0/refinement/sdfContext/calibration/maximum"
    );
    let span = origin.span().unwrap();
    assert_eq!(&RECIPE[span.start as usize..span.end as usize], b"2.50");
    let deleted = effective
        .explanation(&format!("{CALIBRATION}/description"))
        .require()
        .unwrap();
    assert_eq!(deleted.kind(), OriginKind::Deleted);
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["result"]["checking"]["state"], "requested");
    assert!(
        result
            .to_json()
            .contains("draft-ietf-asdf-sdf-nonaffordance-06")
    );
}

#[test]
fn nested_opaque_context_and_unavailable_references_keep_fidelity_undetermined() {
    for context in [
        serde_json::json!({"type":"object","nullable":false,"properties":{"room":{"type":"string","nullable":false,"vendor:meaning":{"number":1}}}}),
        serde_json::json!({"sdfRef":"#/sdfData/Missing"}),
    ] {
        let mut source: serde_json::Value = serde_json::from_slice(BASE).unwrap();
        source["sdfObject"]["Sensor"]["sdfContext"]["siteName"] = context;
        let result = author(&serde_json::to_vec(&source).unwrap(), RECIPE);
        assert_eq!(result.generation(), Generation::Complete);
        assert_eq!(result.fidelity(), Fidelity::Undetermined);
        assert!(!result.document_deliverable());
        assert!(result.generated_document().is_some());
        assert!(result.to_json().contains("author.fidelity_unknown"));
    }
}

#[test]
fn requested_context_checking_keeps_generation_and_fidelity_distinct() {
    let mut recipe: serde_json::Value = serde_json::from_slice(RECIPE).unwrap();
    recipe["objects"][0]["refinement"]["sdfContext"]["calibration"]["maximum"] = "too high".into();
    let result = author(BASE, &serde_json::to_vec(&recipe).unwrap());
    assert_eq!(result.generation(), Generation::Complete);
    assert_eq!(result.fidelity(), Fidelity::Established);
    assert_eq!(result.outcome().disposition(), Disposition::Failed);
    assert!(!result.document_deliverable());
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "data.number")
        .unwrap();
    assert!(
        finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|location| location["kind"] == "author_request"
                && location["pointer"] == "/objects/0/refinement/sdfContext/calibration/maximum")
    );
}

#[test]
fn context_editing_accepts_the_existing_common_and_data_quality_set() {
    for parent in ["sdfObject", "sdfThing"] {
        for quality in [
            "label",
            "description",
            "$comment",
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
        ] {
            let source = serde_json::to_vec(&serde_json::json!({parent:{"Owner":{"sdfContext":{"a /~":{"type":"number", quality:1}}}}})).unwrap();
            let owner = format!("/{parent}/Owner/sdfContext/a ~1~0");
            for remove in [false, true] {
                let catalogue =
                    Catalogue::from_inputs(&[SourceInput::supplied("base", &source)]).unwrap();
                let original = catalogue.source_id("base").unwrap();
                let edit = if remove {
                    Edit::remove(&owner, quality).unwrap()
                } else {
                    Edit::replace(&owner, quality, JsonBuilder::new().null().unwrap()).unwrap()
                };
                let result = EditRequest::new(catalogue, "edit", original, "revised")
                    .unwrap()
                    .with_edit(edit)
                    .unwrap()
                    .edit()
                    .unwrap();
                assert!(result.document_deliverable(), "{parent} {quality}");
                let generated: serde_json::Value =
                    serde_json::from_slice(result.generated_document().unwrap()).unwrap();
                assert_eq!(
                    generated[parent]["Owner"]["sdfContext"]["a /~"].get(quality),
                    if remove {
                        None
                    } else {
                        Some(&serde_json::Value::Null)
                    }
                );
            }
        }
    }
}

#[test]
fn context_edits_refuse_reference_boundaries_unsupported_qualities_and_non_definition_owners() {
    let source = br##"{"sdfObject":{"O":{"sdfContext":{"c":{"sdfRef":null,"description":"patch"}}},"P":{"sdfRef":{},"sdfContext":{"c":{"description":"patch"}}},"Q":{"sdfContext":{"c":{"description":"local","properties":{"nested":{"description":"nested"}}}}}}}"##;
    for (owner, quality, reason) in [
        (
            "/sdfObject/O/sdfContext/c",
            "description",
            "sdfRef composition boundary",
        ),
        (
            "/sdfObject/P/sdfContext/c",
            "description",
            "sdfRef composition boundary",
        ),
        (
            "/sdfObject/Q/sdfContext/c",
            "readable",
            "unsupported editing Quality Name",
        ),
        (
            "/sdfObject/Q/sdfContext/c",
            "sdfRequired",
            "unsupported editing Quality Name",
        ),
        (
            "/sdfObject/Q/sdfContext/c/properties/nested",
            "description",
            "authored named definition",
        ),
        ("/sdfObject/Q/sdfContext", "c", "authored named definition"),
    ] {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", source)]).unwrap();
        let original = catalogue.source_id("base").unwrap();
        let result = EditRequest::new(catalogue, "edit", original, "revised")
            .unwrap()
            .with_edit(Edit::remove(owner, quality).unwrap())
            .unwrap()
            .edit();
        assert!(result.err().unwrap().contains(reason), "{owner} {quality}");
    }
    let authored = author(BASE, RECIPE);
    let original = authored.generated_source();
    let result = EditRequest::new(authored.into_catalogue(), "edit", original, "revised")
        .unwrap()
        .with_edit(Edit::remove(CALIBRATION, "maximum").unwrap())
        .unwrap()
        .edit();
    assert!(
        result
            .err()
            .unwrap()
            .contains("sdfRef composition boundary")
    );
}
