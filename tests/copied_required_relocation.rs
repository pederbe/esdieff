//! A copied `sdfRequired` pointer into the copied definition relocates to the
//! importing definition's coordinate and selects the declaration there in the
//! resolved model, with its own Declaration contribution.
//! Follows ADR 0036: Copied reference relocation and ADR 0026: Required
//! declaration relationships and source evidence.
mod siren_support;
use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, InspectRequest, Lookup, Report,
    SourceInput, View,
};
use serde_json::{Value, json};
use siren_support::{ALARM, RELOCATION, catalogue, codes, siren};

const BASE: &[u8] = include_bytes!("fixtures/copied-reference/base.sdf.json");
const DEVICE: &[u8] = include_bytes!("fixtures/copied-reference/device.sdf.json");

/// The effective required declaration outcome at `pointer` in the source
/// `key`: completion, notices, and each reference's status with its targets'
/// source documents, source pointers, effective pointers and capabilities.
fn required(catalogue: &Catalogue, key: &str, pointer: &str) -> Value {
    let id = catalogue.source_id(key).unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, pointer)
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    let source_key = |json: esdieff::JsonView<'_>| {
        ["alarm", "siren", "base", "device", "model"]
            .into_iter()
            .find(|key| catalogue.source_id(key) == Some(json.source_id()))
            .unwrap()
    };
    let references: Vec<Value> = report
        .required_declarations()
        .require()
        .unwrap()
        .owners()
        .filter_map(|owner| owner.references().present())
        .flatten()
        .map(|reference| {
            let targets: Vec<Value> = match reference.targets() {
                Lookup::Found(targets) => targets
                    .map(|target| {
                        let source = target.source_definition().require().unwrap().json();
                        let effective = target
                            .effective_definition()
                            .present()
                            .map(|definition| definition.json().pointer(8192).require().unwrap());
                        json!([
                            source_key(source),
                            source.pointer(8192).require().unwrap(),
                            effective,
                            target.capability().as_str()
                        ])
                    })
                    .collect(),
                _ => Vec::new(),
            };
            json!([reference.status().as_str(), targets])
        })
        .collect();
    json!({
        "complete": report.outcome().complete(),
        "notices": report.outcome().notices(),
        "references": references,
        "interpretations": wire(&report)["baseline"]["interpretations"],
    })
}

fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}

#[test]
fn copied_pointer_without_an_override_completes_the_full_check() {
    let siren = siren(json!({}));
    let catalogue = catalogue(&siren);
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let wire = wire(&report);
    assert!(report.outcome().complete(), "{wire:#}");
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().notices(), 0);
    assert!(!codes(&wire).contains(&"required.target_missing"));
    for member in 16..=20 {
        assert_eq!(
            report
                .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
                .require()
                .unwrap(),
            AssessmentStatus::Assessed,
            "M{member:03}"
        );
    }
    assert_eq!(wire["baseline"]["interpretations"], json!([RELOCATION]));
}

#[test]
fn copied_pointer_selects_the_declaration_at_the_relocated_coordinate() {
    let siren = siren(json!({}));
    assert_eq!(
        required(&catalogue(&siren), "siren", "/sdfObject/siren"),
        json!({
            "complete": true,
            "notices": 0,
            "references": [["resolved", [[
                "alarm",
                "/sdfObject/alarm/sdfProperty/status",
                "/sdfObject/siren/sdfProperty/status",
                "established"
            ]]]],
            "interpretations": [RELOCATION],
        })
    );
}

#[test]
fn deleting_the_required_declaration_leaves_the_relationship_missing() {
    let siren = siren(json!({"sdfProperty": {"status": null}}));
    let catalogue = catalogue(&siren);
    assert_eq!(
        required(&catalogue, "siren", "/sdfObject/siren"),
        json!({
            "complete": false,
            "notices": 1,
            "references": [["missing", []]],
            "interpretations": [RELOCATION],
        })
    );
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let wire = wire(&report);
    assert_eq!(report.outcome().notices(), 1, "{wire:#}");
    assert_eq!(
        codes(&wire)
            .into_iter()
            .filter(|code| *code == "required.target_missing")
            .count(),
        1
    );
}

#[test]
fn a_declaration_reinserted_at_the_coordinate_is_the_one_required() {
    // `chime` copies the siren that deleted `status`, and inserts its own.
    let mut document: Value =
        serde_json::from_slice(&siren(json!({"sdfProperty": {"status": null}}))).unwrap();
    document["sdfObject"]["chime"] = json!({
        "sdfRef": "#/sdfObject/siren",
        "sdfProperty": {"status": {"type": "string"}}
    });
    let siren = serde_json::to_vec(&document).unwrap();
    assert_eq!(
        required(&catalogue(&siren), "siren", "/sdfObject/chime"),
        json!({
            "complete": true,
            "notices": 0,
            "references": [["resolved", [[
                "siren",
                "/sdfObject/chime/sdfProperty/status",
                "/sdfObject/chime/sdfProperty/status",
                "established"
            ]]]],
            "interpretations": [RELOCATION],
        })
    );
}

#[test]
fn a_boolean_reinserted_at_the_relocated_coordinate_is_the_one_required() {
    // Device deletes Base's required number Property, and Product, which copies
    // Device, reinserts it as a Boolean.
    let mut device: Value = serde_json::from_slice(DEVICE).unwrap();
    device["sdfObject"]["Device"]["sdfProperty"] = json!({"value": null});
    device["sdfObject"]["Product"] = json!({
        "sdfRef": "#/sdfObject/Device",
        "sdfProperty": {"value": {"type": "boolean"}}
    });
    let device = serde_json::to_vec(&device).unwrap();
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", &device),
    ])
    .unwrap();
    assert_eq!(
        required(&catalogue, "device", "/sdfObject/Device"),
        json!({
            "complete": false,
            "notices": 1,
            "references": [["missing", []]],
            "interpretations": [RELOCATION],
        })
    );
    assert_eq!(
        required(&catalogue, "device", "/sdfObject/Product"),
        json!({
            "complete": true,
            "notices": 0,
            "references": [["resolved", [[
                "device",
                "/sdfObject/Product/sdfProperty/value",
                "/sdfObject/Product/sdfProperty/value",
                "established"
            ]]]],
            "interpretations": [RELOCATION],
        })
    );
    let id = catalogue.source_id("device").unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, "/sdfObject/Product/sdfProperty/value").view(View::Effective),
        )
        .unwrap();
    assert_eq!(wire(&report)["result"]["json"], json!({"type": "boolean"}));
}

#[test]
fn short_names_and_true_keep_their_carrier_relative_meaning() {
    let model = serde_json::to_vec(&json!({
        "info": {},
        "sdfObject": {
            "Base": {
                "sdfProperty": {"value": {"type": "number"}},
                "sdfAction": {"value": {}},
                "sdfRequired": ["value", true, "#/sdfObject/Base/sdfProperty/value"]
            },
            "Use": {"sdfRef": "#/sdfObject/Base"}
        }
    }))
    .unwrap();
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", &model)]).unwrap();
    let property = json!([
        "model",
        "/sdfObject/Base/sdfProperty/value",
        "/sdfObject/Use/sdfProperty/value",
        "established"
    ]);
    let action = json!([
        "model",
        "/sdfObject/Base/sdfAction/value",
        "/sdfObject/Use/sdfAction/value",
        "established"
    ]);
    let carrier = json!(["model", "/sdfObject/Use", "/sdfObject/Use", "established"]);
    assert_eq!(
        required(&catalogue, "model", "/sdfObject/Use"),
        json!({
            "complete": true,
            "notices": 0,
            "references": [
                ["resolved", [property, action]],
                ["resolved", [carrier]],
                ["resolved", [property]]
            ],
            "interpretations": [RELOCATION],
        })
    );
}

#[test]
fn an_importers_own_array_replaces_the_copied_one_whole() {
    let siren = siren(json!({"sdfRequired": ["duration"]}));
    assert_eq!(
        required(&catalogue(&siren), "siren", "/sdfObject/siren"),
        json!({
            "complete": true,
            "notices": 0,
            "references": [["resolved", [[
                "alarm",
                "/sdfObject/alarm/sdfProperty/duration",
                "/sdfObject/siren/sdfProperty/duration",
                "established"
            ]]]],
            "interpretations": [],
        })
    );
}

#[test]
fn a_copied_pointer_missing_in_its_source_stays_missing_in_the_importer() {
    // Base requires a declaration it lacks, which Device declares at the
    // corresponding position.
    let mut base: Value = serde_json::from_slice(BASE).unwrap();
    base["sdfObject"]["Base"] = json!({"sdfRequired": ["#/sdfObject/Base/sdfProperty/ghost"]});
    let base = serde_json::to_vec(&base).unwrap();
    let mut device: Value = serde_json::from_slice(DEVICE).unwrap();
    device["sdfObject"]["Device"]["sdfProperty"] = json!({"ghost": {"type": "boolean"}});
    let device = serde_json::to_vec(&device).unwrap();
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", &base),
        SourceInput::supplied("device", &device),
    ])
    .unwrap();
    // Nothing relocates, so nothing relies on the interpretation.
    assert_eq!(
        required(&catalogue, "device", "/sdfObject/Device"),
        json!({
            "complete": false,
            "notices": 1,
            "references": [["missing", []]],
            "interpretations": [],
        })
    );
}

#[test]
fn a_copied_required_curie_outside_the_copy_lists_the_interpretation() {
    // Base's only reference requires a declaration of another Object through
    // its own prefix.
    let mut base: Value = serde_json::from_slice(BASE).unwrap();
    base["sdfObject"]["Shared"] = json!({"sdfProperty": {"on": {"type": "boolean"}}});
    base["sdfObject"]["Base"] = json!({"sdfRequired": ["p:#/sdfObject/Shared/sdfProperty/on"]});
    let base = serde_json::to_vec(&base).unwrap();
    let outcome = |sources: &[(&str, &[u8])], pointer: &str| {
        let inputs: Vec<_> = sources
            .iter()
            .map(|(key, text)| SourceInput::supplied(key, text))
            .collect();
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        required(&catalogue, sources.last().unwrap().0, pointer)
    };
    let expected = |interpretations: Value| {
        json!({
            "complete": true,
            "notices": 0,
            "references": [["resolved", [[
                "base",
                "/sdfObject/Shared/sdfProperty/on",
                null,
                "not_requested"
            ]]]],
            "interpretations": interpretations,
        })
    };
    assert_eq!(
        outcome(&[("base", &base), ("device", DEVICE)], "/sdfObject/Device"),
        expected(json!([RELOCATION]))
    );
    // The source checked on its own copies nothing.
    assert_eq!(
        outcome(&[("base", &base)], "/sdfObject/Base"),
        expected(json!([]))
    );
}

#[test]
fn relocating_a_required_pointer_charges_the_shared_work_allowance() {
    use esdieff::{Limits, RequiredReferenceStatus};
    let siren = siren(json!({}));
    let (mut limited, mut resolved) = (false, false);
    for work in (100..=40_000).step_by(100) {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("alarm", ALARM),
                SourceInput::supplied("siren", &siren),
            ],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let id = catalogue.source_id("siren").unwrap();
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
            .inspect(
                InspectRequest::new(id, "/sdfObject/siren")
                    .view(View::Effective)
                    .required_declarations(true),
            )
            .unwrap();
        // Exhaustion leaves the relationship unfinished rather than missing.
        assert_eq!(report.outcome().notices(), 0, "work {work}");
        if !report.outcome().complete() {
            limited = true;
            continue;
        }
        let statuses: Vec<_> = report
            .required_declarations()
            .require()
            .unwrap()
            .owners()
            .filter_map(|owner| owner.references().present())
            .flatten()
            .map(|reference| reference.status())
            .collect();
        assert_eq!(statuses, [RequiredReferenceStatus::Resolved], "work {work}");
        resolved = true;
        break;
    }
    assert!(limited && resolved);
}

#[test]
fn a_nested_copy_relocates_the_requirement_again() {
    // Product copies Device, which copies Base and its requirement.
    let mut device: Value = serde_json::from_slice(DEVICE).unwrap();
    device["sdfObject"]["Product"] = json!({
        "sdfRef": "#/sdfObject/Device",
        "sdfData": {"measurement": {"minimum": 20}}
    });
    let device = serde_json::to_vec(&device).unwrap();
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("device", &device),
    ])
    .unwrap();
    for importer in ["Device", "Product"] {
        assert_eq!(
            required(&catalogue, "device", &format!("/sdfObject/{importer}")),
            json!({
                "complete": true,
                "notices": 0,
                "references": [["resolved", [[
                    "base",
                    "/sdfObject/Base/sdfProperty/value",
                    format!("/sdfObject/{importer}/sdfProperty/value"),
                    "established"
                ]]]],
                "interpretations": [RELOCATION],
            }),
            "{importer}"
        );
    }
    // An Object nested inside a copied Thing copies a requiring Object.
    let model = serde_json::to_vec(&json!({
        "info": {},
        "sdfObject": {"Core": {
            "sdfProperty": {"p": {"type": "number"}},
            "sdfRequired": ["#/sdfObject/Core/sdfProperty/p"]
        }},
        "sdfThing": {
            "Kit": {"sdfObject": {"inner": {"sdfRef": "#/sdfObject/Core"}}},
            "Plain": {"sdfRef": "#/sdfThing/Kit"}
        }
    }))
    .unwrap();
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", &model)]).unwrap();
    assert_eq!(
        required(&catalogue, "model", "/sdfThing/Plain"),
        json!({
            "complete": true,
            "notices": 0,
            "references": [["resolved", [[
                "model",
                "/sdfObject/Core/sdfProperty/p",
                "/sdfThing/Plain/sdfObject/inner/sdfProperty/p",
                "established"
            ]]]],
            "interpretations": [RELOCATION],
        })
    );
}
