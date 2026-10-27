//! An internal copied reference relocates to the corresponding position in the
//! importing definition and is looked up in its resolved model, at every copy
//! boundary. Other copied references keep their original target.
//! Follows ADR 0036: Copied reference relocation.
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, Edit, EditRequest, EffectiveRequest, Fidelity,
    InspectRequest, JsonBuilder, SampleKind, SampleRequest, SourceInput, View,
};
use serde_json::{Value, json};

const BASE: &[u8] = include_bytes!("fixtures/copied-reference/base.sdf.json");
const DEVICE: &[u8] = include_bytes!("fixtures/copied-reference/device.sdf.json");
const RELOCATION: &str = "copied-reference-relocation/1";

/// `document` with `edit` applied to its parsed JSON.
fn edited(document: &[u8], edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut document: Value = serde_json::from_slice(document).unwrap();
    edit(&mut document);
    serde_json::to_vec(&document).unwrap()
}

/// The effective outcome at `pointer` in the last of `sources`.
fn effective_in(sources: &[(&str, &[u8])], pointer: &str) -> Value {
    let inputs: Vec<_> = sources
        .iter()
        .map(|(key, text)| SourceInput::supplied(key, text))
        .collect();
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let id = catalogue.source_id(sources.last().unwrap().0).unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(id, pointer).view(View::Effective))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    json!({
        "complete": report.outcome().complete(),
        "errors": report.outcome().errors(),
        "json": wire["result"]["json"],
        "conditions": conditions(&wire),
    })
}

/// The effective outcome at `pointer` in the importing document.
fn effective(base: &[u8], device: &[u8], pointer: &str) -> Value {
    effective_in(&[("base", base), ("device", device)], pointer)
}

/// The effective outcome at `pointer` in a single document.
fn single(document: &Value, pointer: &str) -> Value {
    effective_in(
        &[("model", &serde_json::to_vec(document).unwrap())],
        pointer,
    )
}

/// The code, message and primary and first related location of each retained
/// diagnostic, without the requested view's own summary.
fn conditions(wire: &Value) -> Vec<Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["code"] != "inspection.unavailable")
        .map(|item| {
            json!([
                item["code"],
                item["message"],
                item["primary"]["pointer"],
                item["related"][0]["pointer"]
            ])
        })
        .collect()
}

/// A complete number outcome with `minimum`.
fn number(minimum: u32) -> Value {
    json!({
        "complete": true,
        "errors": 0,
        "json": {"type": "number", "minimum": minimum},
        "conditions": [],
    })
}

#[test]
fn copied_measurement_follows_the_importers_refinement() {
    // Neither the source's minimum 0 nor the importer's lookalike `Base` with
    // minimum 99 is the copied measurement.
    let without_lookalike = edited(DEVICE, |device| {
        device["sdfObject"].as_object_mut().unwrap().remove("Base");
    });
    for device in [DEVICE, &without_lookalike] {
        for property in ["value", "qualified"] {
            assert_eq!(
                effective(
                    BASE,
                    device,
                    &format!("/sdfObject/Device/sdfProperty/{property}")
                ),
                number(10),
                "{property}"
            );
        }
    }
}

#[test]
fn sample_checking_uses_the_relocated_constraints() {
    // 5 meets the source's minimum 0 but not the importer's 10, and 50 meets 10
    // but not the lookalike's 99.
    for (sample, expected) in [("5", Admission::Rejected), ("50", Admission::Admitted)] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("base", BASE),
            SourceInput::supplied("device", DEVICE),
            SourceInput::supplied("sample", sample.as_bytes()).sample(),
        ])
        .unwrap();
        for property in ["value", "qualified"] {
            let report = catalogue
                .check_sample(
                    SampleRequest::new(
                        catalogue.source_id("device").unwrap(),
                        SampleKind::Property,
                        &format!("/sdfObject/Device/sdfProperty/{property}"),
                    )
                    .sample(catalogue.source_id("sample").unwrap()),
                )
                .unwrap();
            assert_eq!(
                report.sample_result().unwrap().admission(),
                expected,
                "{sample} {property}"
            );
        }
    }
}

#[test]
fn relocated_content_keeps_its_nearest_origins_and_its_own_reference_as_consumer() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", DEVICE),
    ])
    .unwrap();
    let id = catalogue.source_id("device").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfObject/Device/sdfProperty/value"))
        .unwrap();
    let location = |json: esdieff::JsonView<'_>| {
        let key = ["base", "device"]
            .into_iter()
            .find(|key| catalogue.source_id(key) == Some(json.source_id()))
            .unwrap();
        (key, json.pointer(8192).require().unwrap())
    };
    for (member, origin) in [
        ("type", ("base", "/sdfObject/Base/sdfData/measurement/type")),
        (
            "minimum",
            ("device", "/sdfObject/Device/sdfData/measurement/minimum"),
        ),
    ] {
        let explanation = model
            .explanation(&format!("/sdfObject/Device/sdfProperty/value/{member}"))
            .require()
            .unwrap();
        let sources: Vec<_> = explanation.sources().map(location).collect();
        assert_eq!(sources[0], (origin.0, origin.1.to_owned()), "{member}");
        // The authored copied reference imports the relocated content.
        assert_eq!(
            location(explanation.consumer().unwrap()),
            (
                "base",
                "/sdfObject/Base/sdfProperty/value/sdfRef".to_owned()
            ),
            "{member}"
        );
    }
}

#[test]
fn a_sibling_whose_name_starts_with_the_copied_name_counts_as_outside() {
    let base = edited(BASE, |base| {
        base["sdfObject"]["Base"]["sdfProperty"]["extra"] =
            json!({"sdfRef": "#/sdfObject/BaseExtra/sdfData/measurement"});
        base["sdfObject"]["Base"]["sdfProperty"]["qualifiedExtra"] =
            json!({"sdfRef": "p:#/sdfObject/BaseExtra/sdfData/measurement"});
    });
    for property in ["extra", "qualifiedExtra"] {
        assert_eq!(
            effective(
                &base,
                DEVICE,
                &format!("/sdfObject/Device/sdfProperty/{property}")
            ),
            number(7),
            "{property}"
        );
    }
}

#[test]
fn a_nested_copy_relocates_again_to_the_final_importer() {
    // Product copies Device, which copies Base.
    let device = edited(DEVICE, |device| {
        device["sdfObject"]["Product"] = json!({
            "sdfRef": "#/sdfObject/Device",
            "sdfData": {"measurement": {"minimum": 20}}
        });
    });
    for property in ["value", "qualified"] {
        assert_eq!(
            effective(
                BASE,
                &device,
                &format!("/sdfObject/Product/sdfProperty/{property}")
            ),
            number(20),
            "{property}"
        );
        assert_eq!(
            effective(
                BASE,
                &device,
                &format!("/sdfObject/Device/sdfProperty/{property}")
            ),
            number(10),
            "{property}"
        );
    }
}

#[test]
fn a_copied_definition_that_copies_another_relocates_the_inner_references_first() {
    let document = json!({
        "info": {},
        "sdfObject": {
            "Core": {
                "sdfData": {"measurement": {"type": "number", "minimum": 0}},
                "sdfProperty": {"value": {"sdfRef": "#/sdfObject/Core/sdfData/measurement"}}
            },
            // A target root chain: Base itself copies Core.
            "Base": {"sdfRef": "#/sdfObject/Core", "sdfData": {"measurement": {"minimum": 5}}},
            "Plain": {"sdfRef": "#/sdfObject/Base"},
            "Refined": {"sdfRef": "#/sdfObject/Base", "sdfData": {"measurement": {"minimum": 10}}}
        },
        "sdfThing": {
            // A nested copy: an Object inside the copied Thing copies Core.
            "Kit": {"sdfObject": {"inner": {
                "sdfRef": "#/sdfObject/Core",
                "sdfData": {"measurement": {"minimum": 5}}
            }}},
            "Plain": {"sdfRef": "#/sdfThing/Kit"},
            "Refined": {
                "sdfRef": "#/sdfThing/Kit",
                "sdfObject": {"inner": {"sdfData": {"measurement": {"minimum": 10}}}}
            }
        }
    });
    for (pointer, minimum) in [
        ("/sdfObject/Base/sdfProperty/value", 5),
        ("/sdfObject/Plain/sdfProperty/value", 5),
        ("/sdfObject/Refined/sdfProperty/value", 10),
        ("/sdfThing/Kit/sdfObject/inner/sdfProperty/value", 5),
        ("/sdfThing/Plain/sdfObject/inner/sdfProperty/value", 5),
        ("/sdfThing/Refined/sdfObject/inner/sdfProperty/value", 10),
    ] {
        assert_eq!(single(&document, pointer), number(minimum), "{pointer}");
    }
}

#[test]
fn an_explicit_importer_reference_to_the_sources_internal_data_keeps_the_source() {
    let device = edited(DEVICE, |device| {
        device["sdfObject"]["Device"]["sdfProperty"] = json!({
            "value": {"sdfRef": "b:#/sdfObject/Base/sdfData/measurement"}
        });
    });
    assert_eq!(
        effective(BASE, &device, "/sdfObject/Device/sdfProperty/value"),
        number(0)
    );
}

#[test]
fn a_relocated_target_that_the_patch_deleted_is_missing() {
    let device = edited(DEVICE, |device| {
        device["sdfObject"]["Device"]["sdfData"]["measurement"] = Value::Null;
    });
    for property in ["value", "qualified"] {
        let pointer = format!("/sdfObject/Device/sdfProperty/{property}");
        let reference = format!("/sdfObject/Base/sdfProperty/{property}/sdfRef");
        assert_eq!(
            effective(BASE, &device, &pointer),
            json!({
                "complete": false,
                "errors": 0,
                "json": null,
                "conditions": [[
                    "reference.unavailable",
                    "reference target unavailable",
                    pointer,
                    reference
                ]],
            }),
            "{property}"
        );
    }
}

#[test]
fn a_copied_reference_missing_in_its_source_stays_missing_in_the_importer() {
    // Base lacks the data its `ghost` property references, while Device
    // declares data at the corresponding position.
    let base = edited(BASE, |base| {
        base["sdfObject"]["Base"]["sdfProperty"]["ghost"] =
            json!({"sdfRef": "#/sdfObject/Base/sdfData/ghost"});
    });
    let device = edited(DEVICE, |device| {
        device["sdfObject"]["Device"]["sdfData"]["ghost"] = json!({"type": "number"});
    });
    let pointer = "/sdfObject/Device/sdfProperty/ghost";
    assert_eq!(
        effective(&base, &device, pointer),
        json!({
            "complete": false,
            "errors": 0,
            "json": null,
            "conditions": [[
                "reference.unavailable",
                "reference target unavailable",
                pointer,
                "/sdfObject/Base/sdfProperty/ghost/sdfRef"
            ]],
        })
    );
    // Nothing relocates, so nothing relies on the interpretation.
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", &base),
        SourceInput::supplied("device", &device),
    ])
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("device").unwrap(), pointer)
                .view(View::Effective),
        )
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["baseline"]["interpretations"], json!([]));
}

#[test]
fn a_cycle_created_by_relocation_is_scoped_with_its_reference() {
    // In Base, `a` copies `b`. Device refines its copy of `b` to copy its own `a`,
    // whose copied reference now relocates back to Device's `b`.
    let document = json!({
        "info": {},
        "sdfObject": {
            "Base": {"sdfData": {
                "a": {"sdfRef": "#/sdfObject/Base/sdfData/b"},
                "b": {"type": "number"}
            }},
            "Device": {
                "sdfRef": "#/sdfObject/Base",
                "sdfData": {"b": {"sdfRef": "#/sdfObject/Device/sdfData/a"}}
            },
            "Other": {"sdfData": {"c": {"type": "string"}}}
        }
    });
    let outcome = single(&document, "/sdfObject");
    assert_eq!(outcome["complete"], false);
    assert_eq!(
        outcome["conditions"],
        json!([
            [
                "reference.effective_cycle",
                "effective reference cycle",
                "/sdfObject/Device/sdfData/a",
                "/sdfObject/Base/sdfData/a/sdfRef"
            ],
            [
                "reference.effective_cycle",
                "effective reference cycle",
                "/sdfObject/Device/sdfData/b",
                "/sdfObject/Device/sdfData/b/sdfRef"
            ]
        ])
    );
    // The source and independent definitions stay available.
    assert_eq!(
        single(&document, "/sdfObject/Base/sdfData/a")["json"],
        json!({"type": "number"})
    );
    assert_eq!(single(&document, "/sdfObject/Other")["complete"], true);
}

#[test]
fn relocation_leaves_the_source_documents_unchanged() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", DEVICE),
    ])
    .unwrap();
    let device = catalogue.source_id("device").unwrap();
    let base = catalogue.source_id("base").unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(report.outcome().errors() == 0);
    let value = catalogue
        .effective(EffectiveRequest::new(device).pointer("/sdfObject/Device/sdfProperty/value"))
        .unwrap();
    assert!(value.complete());
    assert_eq!(catalogue.source(base).unwrap().raw_bytes(), BASE);
    assert_eq!(catalogue.source(device).unwrap().raw_bytes(), DEVICE);
    // With Device in the same catalogue, the source's own definition keeps its
    // own measurement.
    assert_eq!(
        effective(BASE, DEVICE, "/sdfObject/Device/sdfProperty/value")["json"]["minimum"],
        10
    );
    assert_eq!(
        effective_in(
            &[("device", DEVICE), ("base", BASE)],
            "/sdfObject/Base/sdfProperty/value"
        ),
        number(0)
    );
}

#[test]
fn the_catalogue_check_keeps_the_sources_own_findings_beside_its_importer() {
    // Base requires a declaration it lacks, which is its own finding.
    let base = edited(BASE, |base| {
        base["sdfObject"]["Base"]["sdfRequired"]
            .as_array_mut()
            .unwrap()
            .push(json!("#/sdfObject/Base/sdfProperty/extra"));
    });
    // The code, message and pointer of each finding located in Base's document.
    let source_findings = |sources: &[(&str, &[u8])]| {
        let inputs: Vec<_> = sources
            .iter()
            .map(|(key, text)| SourceInput::supplied(key, text))
            .collect();
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let base = json!(catalogue.source_id("base").unwrap().get());
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["primary"]["root"] == base || item["primary"]["source_id"] == base)
            .map(|item| json!([item["code"], item["message"], item["primary"]["pointer"]]))
            .collect::<Vec<_>>()
    };
    let alone = source_findings(&[("base", &base)]);
    assert_eq!(
        alone,
        [json!([
            "required.target_missing",
            "Required target was not found in the established scope",
            "/sdfObject/Base/sdfRequired/1"
        ])]
    );
    assert_eq!(
        source_findings(&[("base", &base), ("device", DEVICE)]),
        alone
    );
}

#[test]
fn an_edit_that_leaves_copied_references_in_place_keeps_their_relocated_meaning() {
    // The edit changes the importer's lookalike Base and leaves the copied
    // references, which keep relocating to Device's refinement.
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", DEVICE),
    ])
    .unwrap();
    let original = catalogue.source_id("device").unwrap();
    let result = EditRequest::new(catalogue, "lookalike", original, "revised")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Base/sdfData/measurement",
                "minimum",
                JsonBuilder::new().number("50").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .checking(Assessment::FullBaseline)
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(result.fidelity(), Fidelity::Established);
    // Checking the edited model relies on the relocation and completes.
    let checking = result.checking().outcome().unwrap();
    assert!(checking.complete());
    assert_eq!(checking.errors(), 0);
    let wire: Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["baseline"]["interpretations"], json!([RELOCATION]));
    // The edited document's copies keep Device's refinement.
    let catalogue = result.catalogue();
    for property in ["value", "qualified"] {
        let pointer = format!("/sdfObject/Device/sdfProperty/{property}");
        let report = catalogue
            .inspect(InspectRequest::new(result.generated_source(), &pointer).view(View::Effective))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(report.outcome().complete(), "{property}");
        assert_eq!(
            wire["result"]["json"],
            json!({"type": "number", "minimum": 10}),
            "{property}"
        );
    }
}

#[test]
fn the_relocated_lookup_charges_the_shared_reference_allowance() {
    use esdieff::Limits;
    let inspect = |references: usize| {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("base", BASE),
                SourceInput::supplied("device", DEVICE),
            ],
            Limits::default().reference_applications(references),
        )
        .unwrap();
        let id = catalogue.source_id("device").unwrap();
        let report = catalogue
            .inspect(
                InspectRequest::new(id, "/sdfObject/Device/sdfProperty/value")
                    .view(View::Effective),
            )
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        (report.outcome().complete(), conditions(&wire))
    };
    // Copying Base and the property's own reference leave nothing for composing
    // the importer on the relocated target's path.
    assert_eq!(
        inspect(2),
        (
            false,
            vec![json!([
                "operation.limit",
                "reference application limit",
                "/sdfObject/Device/sdfProperty/value",
                "/sdfObject/Base/sdfProperty/value/sdfRef"
            ])]
        )
    );
    assert_eq!(inspect(3), (true, Vec::new()));
}

#[test]
fn reports_list_the_interpretation_exactly_when_a_result_relied_on_it() {
    // The disposition and interpretations of a Full check of `sources`.
    let check = |sources: &[(&str, &[u8])]| {
        let inputs: Vec<_> = sources
            .iter()
            .map(|(key, text)| SourceInput::supplied(key, text))
            .collect();
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        json!([
            wire["outcome"]["disposition"],
            wire["baseline"]["interpretations"]
        ])
    };
    let listed = json!(["passed", [RELOCATION]]);
    let omitted = json!(["passed", []]);
    assert_eq!(check(&[("base", BASE), ("device", DEVICE)]), listed);
    // Without an importer nothing is copied, so nothing relocates.
    assert_eq!(check(&[("base", BASE)]), omitted);
    // A copy whose only reference is a CURIE outside it relocates nothing, but
    // reads that CURIE with the prefixes of the document that wrote it.
    let curie_only = edited(BASE, |base| {
        let object = base["sdfObject"]["Base"].as_object_mut().unwrap();
        object.remove("sdfRequired");
        object["sdfProperty"]
            .as_object_mut()
            .unwrap()
            .retain(|name, _| name == "outside");
    });
    assert_eq!(check(&[("base", &curie_only), ("device", DEVICE)]), listed);
    assert_eq!(check(&[("base", &curie_only)]), omitted);
    // Copying only a same-document pointer outside the copy relies on nothing.
    let outside = serde_json::to_vec(&json!({
        "info": {},
        "sdfData": {"Outside": {"type": "number"}},
        "sdfObject": {
            "Base": {"sdfProperty": {"p": {"sdfRef": "#/sdfData/Outside"}}},
            "Use": {"sdfRef": "#/sdfObject/Base"}
        }
    }))
    .unwrap();
    assert_eq!(check(&[("model", &outside)]), omitted);
    // A copy within one document that reads a copied CURIE still reads it with
    // the prefixes of the document that wrote it.
    let curie = serde_json::to_vec(&json!({
        "info": {},
        "namespace": {"m": "https://example.test/model"},
        "defaultNamespace": "m",
        "sdfData": {"Outside": {"type": "number"}},
        "sdfObject": {
            "Base": {"sdfProperty": {"p": {"sdfRef": "m:#/sdfData/Outside"}}},
            "Use": {"sdfRef": "#/sdfObject/Base"}
        }
    }))
    .unwrap();
    assert_eq!(check(&[("model", &curie)]), listed);
}
