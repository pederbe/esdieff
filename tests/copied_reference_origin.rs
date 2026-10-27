//! A copied reference stays copied when an importer patches only its siblings.
//! Only an `sdfRef` member or `sdfRequired` array written in the composition
//! patch is the importer's own reference. A copied CURIE is read with the
//! namespace mappings of the document that wrote it.
//! Follows ADR 0036: Copied reference relocation.
use esdieff::{Assessment, Catalogue, CheckRequest, InspectRequest, SourceInput, View};
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

/// The importer document with `patch` members added to the `Device` composition patch.
fn device(patch: Value) -> Vec<u8> {
    let mut document: Value = serde_json::from_slice(DEVICE).unwrap();
    let Value::Object(members) = patch else {
        panic!("composition patch members")
    };
    let device = document["sdfObject"]["Device"].as_object_mut().unwrap();
    for (key, value) in members {
        match (device.get_mut(&key), value) {
            (Some(Value::Object(existing)), Value::Object(added)) => existing.extend(added),
            (_, value) => {
                device.insert(key, value);
            }
        }
    }
    serde_json::to_vec(&document).unwrap()
}

/// The effective outcome at `pointer`, ignoring the description a touched variant adds.
fn effective(device: &[u8], pointer: &str) -> Value {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", device),
    ])
    .unwrap();
    let id = catalogue.source_id("device").unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(id, pointer).view(View::Effective))
        .unwrap();
    let mut wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    if let Some(json) = wire["result"]["json"].as_object_mut() {
        json.remove("description");
    }
    json!({
        "complete": report.outcome().complete(),
        "errors": report.outcome().errors(),
        "state": wire["result"]["state"],
        "json": wire["result"]["json"],
        "conditions": conditions(&wire),
    })
}

/// The required declaration outcome at `pointer` in the last of `sources`.
fn required(sources: &[(&str, &[u8])], pointer: &str) -> Value {
    let inputs: Vec<_> = sources
        .iter()
        .map(|(key, text)| SourceInput::supplied(key, text))
        .collect();
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let id = catalogue.source_id(sources.last().unwrap().0).unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, pointer)
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let references: Vec<_> = wire["result"]["required_declarations"]["owners"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|owner| owner["references"].as_array().unwrap())
        .map(|reference| json!([reference["text"], reference["status"], reference["targets"]]))
        .collect();
    json!({
        "complete": report.outcome().complete(),
        "errors": report.outcome().errors(),
        "notices": report.outcome().notices(),
        "references": references,
        "conditions": conditions(&wire),
    })
}

/// The code, message and primary location of each retained diagnostic.
fn conditions(wire: &Value) -> Vec<Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| json!([item["code"], item["message"], item["primary"]["pointer"]]))
        .collect()
}

#[test]
fn patching_a_copied_property_description_leaves_its_same_document_reference_copied() {
    let pointer = "/sdfObject/Device/sdfProperty/value";
    let untouched = effective(DEVICE, pointer);
    let touched = effective(
        &device(json!({"sdfProperty": {"value": {"description": "touched"}}})),
        pointer,
    );
    assert_eq!(touched, untouched);
    assert!(
        !(touched["complete"] == true && touched["json"]["minimum"] == 0),
        "the source's minimum ignores the importer's refinement: {touched}"
    );
}

#[test]
fn patching_a_copied_property_description_leaves_its_curie_reference_copied() {
    for (property, internal) in [("qualified", true), ("outside", false)] {
        let pointer = format!("/sdfObject/Device/sdfProperty/{property}");
        let untouched = effective(DEVICE, &pointer);
        let touched = effective(
            &device(json!({"sdfProperty": {property: {"description": "touched"}}})),
            &pointer,
        );
        assert_eq!(touched, untouched, "{property}");
        if internal {
            assert!(
                !(touched["complete"] == true && touched["json"]["minimum"] == 0),
                "the source's minimum ignores the importer's refinement: {touched}"
            );
        }
    }
}

#[test]
fn patching_a_sibling_of_a_copied_required_pointer_leaves_the_pointer_copied() {
    let touched = device(json!({"description": "touched"}));
    assert_eq!(
        required(&[("base", BASE), ("device", &touched)], "/sdfObject/Device"),
        required(&[("base", BASE), ("device", DEVICE)], "/sdfObject/Device"),
    );
    // Here the patch touches the copied Object that holds `sdfRequired`, not the
    // importing definition itself.
    let thing = |patch: &str| {
        format!(
            r##"{{"info":{{}},"sdfThing":{{"Base":{{"sdfObject":{{"inner":{{"sdfProperty":{{"p":{{"type":"number"}}}},"sdfRequired":["#/sdfThing/Base/sdfObject/inner/sdfProperty/p"]}}}}}},"Use":{{"sdfRef":"#/sdfThing/Base"{patch}}}}}}}"##
        )
    };
    let untouched = thing("");
    let touched = thing(r#","sdfObject":{"inner":{"description":"touched"}}"#);
    assert_eq!(
        required(&[("model", touched.as_bytes())], "/sdfThing/Use"),
        required(&[("model", untouched.as_bytes())], "/sdfThing/Use"),
    );
}

#[test]
fn full_check_treats_touched_copies_like_untouched_copies() {
    let check = |device: &[u8]| {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("base", BASE),
            SourceInput::supplied("device", device),
        ])
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let copying: Vec<_> = (16..=20)
            .map(|member| {
                let id = format!("rfc9880.model.M{member:03}/1");
                report.requirement_status(&id).require().unwrap().as_str()
            })
            .collect();
        json!({
            "complete": report.outcome().complete(),
            "errors": report.outcome().errors(),
            "conditions": conditions(&wire),
            "copying": copying,
        })
    };
    let touched = device(json!({
        "description": "touched",
        "sdfProperty": {
            "value": {"description": "touched"},
            "qualified": {"description": "touched"},
            "outside": {"description": "touched"}
        }
    }));
    assert_eq!(check(&touched), check(DEVICE));
}

#[test]
fn a_reference_written_in_the_patch_is_the_importers_even_with_the_copied_text() {
    let written = device(json!({
        "sdfProperty": {
            "value": {"sdfRef": "#/sdfObject/Base/sdfData/measurement"},
            "outside": {"sdfRef": "p:#/sdfData/Outside"}
        },
        "sdfRequired": ["#/sdfObject/Base/sdfProperty/value"]
    }));
    // The importer's own document has a lookalike `Base` with minimum 99.
    let value = effective(&written, "/sdfObject/Device/sdfProperty/value");
    assert_eq!(value["complete"], true, "{value}");
    assert_eq!(value["json"]["type"], "number");
    assert_eq!(value["json"]["minimum"], 99);
    // The importer maps `p` to its own namespace, where `Outside` is a string.
    let outside = effective(&written, "/sdfObject/Device/sdfProperty/outside");
    assert_eq!(outside["complete"], true, "{outside}");
    assert_eq!(outside["json"]["type"], "string");
    assert_eq!(outside["json"]["minLength"], 3);
    // The lookalike `Base` declares no `value`.
    let requiredness = required(&[("base", BASE), ("device", &written)], "/sdfObject/Device");
    assert_eq!(
        requiredness["references"],
        json!([["#/sdfObject/Base/sdfProperty/value", "missing", []]])
    );
    assert_eq!(requiredness["notices"], 1);
}

#[test]
fn a_copied_curie_uses_the_prefixes_of_the_document_that_wrote_it() {
    // The importer maps `p` to its own namespace, where `Outside` is a string, but
    // the source wrote `p:#/sdfData/Outside` meaning its own number.
    let outside = effective(DEVICE, "/sdfObject/Device/sdfProperty/outside");
    assert_eq!(outside["complete"], true, "{outside}");
    assert_eq!(outside["conditions"], json!([]));
    assert_eq!(outside["json"], json!({"type": "number", "minimum": 0}));
}

#[test]
fn a_copied_curie_with_a_prefix_its_source_never_mapped_is_an_error_at_the_source() {
    // The source writes `b:`, which only the importer maps (to the source
    // namespace). Reading it with the source's prefixes relies on the
    // interpretation, so the report names it.
    let stray = edited(BASE, |base| {
        base["sdfObject"]["Base"]["sdfProperty"]["stray"] =
            json!({"sdfRef": "b:#/sdfData/Outside"});
    });
    // The copied `sdfRequired` is Base's only reference.
    let required = edited(BASE, |base| {
        base["sdfObject"]["Base"] = json!({
            "sdfProperty": {"on": {"type": "boolean"}},
            "sdfRequired": ["b:#/sdfObject/Base/sdfProperty/on"]
        });
    });
    for (base, requested, reference) in [
        (
            stray,
            "/sdfObject/Device/sdfProperty/stray",
            "/sdfObject/Base/sdfProperty/stray/sdfRef",
        ),
        (
            required,
            "/sdfObject/Device",
            "/sdfObject/Base/sdfRequired/0",
        ),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("base", &base),
            SourceInput::supplied("device", DEVICE),
        ])
        .unwrap();
        let base_id = catalogue.source_id("base").unwrap().get();
        let report = catalogue
            .inspect(
                InspectRequest::new(catalogue.source_id("device").unwrap(), requested)
                    .view(View::Effective),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 1, "{reference}");
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let unmapped: Vec<_> = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["code"] == "reference.prefix_unmapped")
            .collect();
        assert_eq!(unmapped.len(), 1, "{wire}");
        assert_eq!(unmapped[0]["severity"], "error");
        let primary = &unmapped[0]["primary"];
        assert_eq!(
            [&primary["kind"], &primary["source_id"], &primary["pointer"]],
            [&json!("source"), &json!(base_id), &json!(reference)]
        );
        assert_eq!(
            wire["baseline"]["interpretations"],
            json!([RELOCATION]),
            "{reference}"
        );
    }
}

#[test]
fn copied_required_curies_use_the_prefixes_of_the_document_that_wrote_them() {
    let mut base: Value = serde_json::from_slice(BASE).unwrap();
    base["sdfObject"]["Shared"] = json!({"sdfProperty": {"on": {"type": "boolean"}}});
    base["sdfObject"]["Base"]["sdfRequired"] = json!([
        "p:#/sdfObject/Shared/sdfProperty/on",
        "p:#/sdfObject/Base/sdfProperty/value",
        "b:#/sdfObject/Shared/sdfProperty/on"
    ]);
    let base = serde_json::to_vec(&base).unwrap();
    let outcome = required(&[("base", &base), ("device", DEVICE)], "/sdfObject/Device");
    let statuses: Vec<_> = outcome["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|reference| reference[1].clone())
        .collect();
    // The source's `p` reaches its own `Shared`. The importer's `p` would not.
    // The second lies inside the copied `Base` and relocates to the importer's
    // copy. The source never mapped `b`, even though the importer does.
    assert_eq!(
        statuses,
        [
            json!("resolved"),
            json!("resolved"),
            json!("invalid_syntax")
        ],
        "{outcome}"
    );
    assert_eq!(
        outcome["references"][0][2][0]["source_definition"]["pointer"],
        "/sdfObject/Shared/sdfProperty/on",
        "{outcome}"
    );
    assert_eq!(
        outcome["references"][1][2][0]["effective_definition"]["pointer"],
        "/sdfObject/Device/sdfProperty/value",
        "{outcome}"
    );
    assert_eq!(outcome["errors"], 1);
    // The unmapped prefix keeps the requested view unfinished.
    assert_eq!(outcome["complete"], false);
    let unmapped: Vec<_> = outcome["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|condition| condition[0] == "reference.prefix_unmapped")
        .collect();
    assert_eq!(
        unmapped,
        [&json!([
            "reference.prefix_unmapped",
            "reference prefix has no namespace mapping",
            "/sdfObject/Base/sdfRequired/2"
        ])],
        "{outcome}"
    );
}

#[test]
fn a_copied_curie_into_the_copied_definition_relocates_with_the_sources_prefixes() {
    // `p:#/sdfObject/Base/sdfData/measurement` read with the source's prefixes lies
    // inside the copied `Base`, so it relocates to the importer's refined copy.
    // The importer's own `p` would select its lookalike with minimum 99.
    let qualified = effective(DEVICE, "/sdfObject/Device/sdfProperty/qualified");
    assert_eq!(qualified["complete"], true, "{qualified}");
    assert_eq!(qualified["conditions"], json!([]));
    assert_eq!(qualified["json"], json!({"type": "number", "minimum": 10}));
}

/// The outcome errors and the code, severity and primary location of each
/// reference condition when `document` is checked on its own.
fn checked_references(document: &Value) -> (Value, Vec<Value>) {
    let text = serde_json::to_vec(document).unwrap();
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", &text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let references = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["code"].as_str().unwrap().starts_with("reference."))
        .map(|item| {
            json!([
                item["code"],
                item["severity"],
                item["primary"]["kind"],
                item["primary"]["pointer"]
            ])
        })
        .collect();
    (wire["outcome"]["errors"].clone(), references)
}

#[test]
fn a_reference_problem_reached_through_copies_is_one_finding() {
    // Copying `a` reaches its reference once in `a` and again in each importer,
    // yet the document has one authored problem, reported at that reference.
    let cases = [
        (
            json!({"sdfThing": {
                "a": {"sdfObject": {"x": {"sdfRef": "pg:#/sdfObject/X"}}},
                "b": {"sdfRef": "#/sdfThing/a"}
            }}),
            "reference.prefix_unmapped",
            "/sdfThing/a/sdfObject/x/sdfRef",
        ),
        (
            json!({"sdfThing": {
                "a": {"sdfObject": {"x": {"sdfRef": "pg:#/sdfObject/X"}}},
                "b": {"sdfRef": "#/sdfThing/a"},
                "c": {"sdfRef": "#/sdfThing/a"}
            }}),
            "reference.prefix_unmapped",
            "/sdfThing/a/sdfObject/x/sdfRef",
        ),
        (
            json!({"sdfThing": {
                "a": {"sdfObject": {"x": {"sdfRef": "1pg:#/sdfObject/X"}}},
                "b": {"sdfRef": "#/sdfThing/a"}
            }}),
            "reference.syntax",
            "/sdfThing/a/sdfObject/x/sdfRef",
        ),
        (
            json!({"sdfThing": {
                "a": {"sdfObject": {"x": {
                    "sdfProperty": {"p": {"type": "string"}},
                    "sdfRequired": ["pg:#/sdfObject/x/sdfProperty/p"]
                }}},
                "b": {"sdfRef": "#/sdfThing/a"}
            }}),
            "reference.prefix_unmapped",
            "/sdfThing/a/sdfObject/x/sdfRequired/0",
        ),
        // Expanding `B` meets the same reference in its target's root chain.
        (
            json!({"sdfObject": {
                "A": {"sdfRef": "pg:#/sdfObject/X"},
                "B": {"sdfRef": "#/sdfObject/A"}
            }}),
            "reference.prefix_unmapped",
            "/sdfObject/A/sdfRef",
        ),
    ];
    for (document, code, reference) in cases {
        let (errors, references) = checked_references(&document);
        assert_eq!(
            references,
            [json!([code, "error", "source", reference])],
            "{document}"
        );
        assert_eq!(errors, 1, "{document}");
    }
}

#[test]
fn a_catalogue_check_reports_a_reference_problem_another_document_copies_once() {
    // Each document's model reaches `X`'s reference: `source` as written and
    // `importer` through its copy. The catalogue still has one authored problem.
    let source = json!({
        "namespace": {"s": "https://example.com/source"},
        "defaultNamespace": "s",
        "sdfObject": {"X": {"sdfRef": "pg:#/sdfObject/Y"}}
    });
    let importer = json!({
        "namespace": {"s": "https://example.com/source"},
        "sdfObject": {"Z": {"sdfRef": "s:#/sdfObject/X"}}
    });
    let source = serde_json::to_vec(&source).unwrap();
    let importer = serde_json::to_vec(&importer).unwrap();
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("source", &source),
        SourceInput::supplied("importer", &importer),
    ])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let unmapped: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["code"] == "reference.prefix_unmapped")
        .map(|item| json!([item["primary"]["source_id"], item["primary"]["pointer"]]))
        .collect();
    let source_id = catalogue.source_id("source").unwrap().get();
    assert_eq!(
        unmapped,
        [json!([source_id, "/sdfObject/X/sdfRef"])],
        "{wire}"
    );
    assert_eq!(report.outcome().errors(), 1, "{wire}");
}

#[test]
fn an_unavailable_target_reached_through_copies_is_reported_at_each_effective_location() {
    // An unavailable target leaves each effective position that needs it
    // unfinished, so each reach keeps its own effective location.
    let (errors, references) = checked_references(&json!({"sdfThing": {
        "a": {"sdfObject": {"x": {"sdfRef": "#/sdfObject/Missing"}}},
        "b": {"sdfRef": "#/sdfThing/a"}
    }}));
    assert_eq!(errors, 0);
    assert_eq!(
        references,
        [
            json!([
                "reference.unavailable",
                "incomplete",
                "effective",
                "/sdfThing/a/sdfObject/x"
            ]),
            json!([
                "reference.unavailable",
                "incomplete",
                "effective",
                "/sdfThing/b/sdfObject/x"
            ]),
        ]
    );
}
