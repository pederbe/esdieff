//! A name reference selects the element at its pointer in the document's
//! resolved model, so an importer can reference and require declarations that
//! exist only through a copy.
//! Follows ADR 0036: Copied reference relocation.
use super::siren_support;
use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, EffectiveRequest, InspectRequest,
    Lookup, OriginKind, Report, RequiredReferenceStatus, RequiredTargetCapability, SourceInput,
    View,
};
use serde_json::{Value, json};
use siren_support::{ALARM, RELOCATION, catalogue, codes, siren};

/// A third document whose `panel` Object is `panel`. It maps `s` to the
/// importer's namespace, so its CURIEs name the importer's global names.
fn panel(panel: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "info": {},
        "namespace": {"s": "https://example.test/siren"},
        "sdfObject": {"panel": panel}
    }))
    .unwrap()
}

fn catalogue_with_panel(siren: &[u8], panel: &[u8]) -> Catalogue {
    Catalogue::from_inputs(&[
        SourceInput::supplied("alarm", ALARM),
        SourceInput::supplied("siren", siren),
        SourceInput::supplied("panel", panel),
    ])
    .unwrap()
}

fn location(json: esdieff::JsonView<'_>, catalogue: &Catalogue) -> (String, String) {
    let key = ["alarm", "siren", "panel"]
        .into_iter()
        .find(|key| catalogue.source_id(key) == Some(json.source_id()))
        .unwrap();
    (key.to_owned(), json.pointer().require().unwrap())
}

fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}

#[test]
fn importer_pointer_to_copied_declaration_completes_the_full_check() {
    let siren = siren(json!({"sdfRequired": ["#/sdfObject/siren/sdfProperty/status"]}));
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
    assert!(
        wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .contains(&json!(RELOCATION))
    );
}

#[test]
fn importer_reference_to_copied_data_uses_the_composed_content() {
    // Without a refinement the data exists only through the copy.
    for (refinement, minimum) in [(None, "0"), (Some(json!({"minimum": 5})), "5")] {
        let mut properties = json!({"loud": {"sdfRef": "#/sdfObject/siren/sdfProperty/duration"}});
        if let Some(refinement) = refinement {
            properties["duration"] = refinement;
        }
        let siren = siren(json!({"sdfProperty": properties}));
        let catalogue = catalogue(&siren);
        let id = catalogue.source_id("siren").unwrap();
        let model = catalogue
            .effective(EffectiveRequest::new(id).pointer("/sdfObject/siren/sdfProperty/loud"))
            .unwrap();
        assert!(model.complete());
        let loud = model.json().require().unwrap();
        assert_eq!(
            loud.get("type")
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap(),
            "number"
        );
        assert_eq!(
            loud.get("minimum")
                .require()
                .unwrap()
                .number()
                .require()
                .unwrap()
                .lexeme(),
            minimum
        );
        let explanation = model
            .explanation("/sdfObject/siren/sdfProperty/loud/type")
            .require()
            .unwrap();
        assert_eq!(explanation.kind(), OriginKind::Inherited);
        assert_eq!(
            location(explanation.sources().next().unwrap(), &catalogue),
            (
                "alarm".to_owned(),
                "/sdfObject/alarm/sdfProperty/duration/type".to_owned()
            )
        );
        assert_eq!(
            location(explanation.consumer().unwrap(), &catalogue),
            (
                "siren".to_owned(),
                "/sdfObject/siren/sdfProperty/loud/sdfRef".to_owned()
            )
        );
        let origin = location(
            model
                .explanation("/sdfObject/siren/sdfProperty/loud/minimum")
                .require()
                .unwrap()
                .sources()
                .next()
                .unwrap(),
            &catalogue,
        );
        assert_eq!(
            origin.0,
            if minimum == "5" { "siren" } else { "alarm" },
            "{origin:?}"
        );
    }
}

/// The diagnostic codes and messages of an effective inspection, with the
/// location each one relates to.
fn conditions(catalogue: &Catalogue, pointer: &str) -> (bool, Vec<Value>) {
    conditions_in(catalogue, "siren", pointer)
}

fn conditions_in(catalogue: &Catalogue, key: &str, pointer: &str) -> (bool, Vec<Value>) {
    let id = catalogue.source_id(key).unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(id, pointer).view(View::Effective))
        .unwrap();
    let wire = wire(&report);
    let items = wire["diagnostics"]["items"]
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
        .collect();
    (report.outcome().complete(), items)
}

#[test]
fn deleted_composed_target_is_missing_like_any_other_target() {
    // The importer restates requiredness by short name, which keeps the copied
    // pointer out of these cases.
    let deleted = siren(json!({"sdfRequired": ["status"], "sdfProperty": {
        "duration": null,
        "loud": {"sdfRef": "#/sdfObject/siren/sdfProperty/duration"}
    }}));
    let absent = siren(json!({"sdfRequired": ["status"], "sdfProperty": {
        "loud": {"sdfRef": "#/sdfObject/siren/sdfProperty/never"}
    }}));
    // Only the deletion makes the resolved model differ from the document as
    // written, which names the interpretation.
    for (siren, interpretations) in [(deleted, json!([RELOCATION])), (absent, json!([]))] {
        let catalogue = catalogue(&siren);
        let (complete, items) = conditions(&catalogue, "/sdfObject/siren");
        assert!(!complete);
        assert_eq!(
            items,
            [json!([
                "reference.unavailable",
                "reference target unavailable",
                "/sdfObject/siren/sdfProperty/loud",
                "/sdfObject/siren/sdfProperty/loud/sdfRef"
            ])]
        );
        // The copied status stays available beside the scoped failure.
        let id = catalogue.source_id("siren").unwrap();
        let model = catalogue
            .effective(EffectiveRequest::new(id).pointer("/sdfObject/siren/sdfProperty/status"))
            .unwrap();
        assert!(model.complete());
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            wire(&report)["baseline"]["interpretations"],
            interpretations
        );
    }

    let siren = siren(json!({
        "sdfProperty": {"status": null},
        "sdfRequired": ["#/sdfObject/siren/sdfProperty/status"]
    }));
    let catalogue = catalogue(&siren);
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_eq!(report.outcome().notices(), 1);
    let full_wire = wire(&report);
    let codes = codes(&full_wire);
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "required.target_missing")
            .count(),
        1,
        "{codes:?}"
    );
    assert_eq!(
        full_wire["baseline"]["interpretations"],
        json!([RELOCATION])
    );
    let id = catalogue.source_id("siren").unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, "/sdfObject/siren")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert_eq!(
        wire(&report)["baseline"]["interpretations"],
        json!([RELOCATION])
    );
    let reference = report
        .required_declarations()
        .require()
        .unwrap()
        .owners()
        .next()
        .unwrap()
        .references()
        .require()
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(reference.status(), RequiredReferenceStatus::Missing);
    assert!(matches!(reference.targets(), Lookup::NotFound));
}

#[test]
fn a_member_deleted_on_the_path_to_an_absent_target_names_no_interpretation() {
    // `never` exists nowhere, so deleting I's properties on the way to it leaves
    // the same missing target as keeping them.
    let check = |properties: Option<Value>| {
        let mut importer = json!({"sdfRef": "#/sdfObject/A"});
        if let Some(properties) = properties {
            importer["sdfProperty"] = properties;
        }
        let model = serde_json::to_vec(&json!({
            "info": {},
            "sdfObject": {
                "A": {"sdfProperty": {"x": {}}},
                "I": importer,
                "U": {"sdfRef": "#/sdfObject/I/sdfProperty/never"}
            }
        }))
        .unwrap();
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", &model)]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        json!({
            "conditions": conditions_in(&catalogue, "model", "/sdfObject/U").1,
            "interpretations": wire(&report)["baseline"]["interpretations"],
        })
    };
    let expected = json!({
        "conditions": [[
            "reference.unavailable",
            "reference target unavailable",
            "/sdfObject/U",
            "/sdfObject/U/sdfRef"
        ]],
        "interpretations": [],
    });
    assert_eq!(check(None), expected);
    assert_eq!(check(Some(Value::Null)), expected);
}

#[test]
fn cycle_through_path_composition_is_scoped_with_its_reference() {
    // The data's own reference looks beneath itself, through its own copy.
    let siren = siren(json!({"sdfRequired": ["status"], "sdfData": {
        "d": {"sdfRef": "#/sdfObject/siren/sdfData/d/properties/x"}
    }}));
    let catalogue = catalogue(&siren);
    let (complete, items) = conditions(&catalogue, "/sdfObject/siren");
    assert!(!complete);
    assert_eq!(
        items,
        [json!([
            "reference.effective_cycle",
            "effective reference cycle",
            "/sdfObject/siren/sdfData/d",
            "/sdfObject/siren/sdfData/d/sdfRef"
        ])]
    );
    let id = catalogue.source_id("siren").unwrap();
    assert!(
        catalogue
            .effective(EffectiveRequest::new(id).pointer("/sdfObject/siren/sdfProperty/status"))
            .unwrap()
            .complete()
    );

    // Two definitions each reach the other's copied content.
    let text = br##"{"info":{},"sdfObject":{
        "X":{"sdfRef":"#/sdfObject/Y/sdfData/d"},
        "Y":{"sdfRef":"#/sdfObject/X/sdfData/d"},
        "Z":{"sdfData":{"d":{"type":"number"}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("siren", text)]).unwrap();
    let (complete, items) = conditions(&catalogue, "/sdfObject");
    assert!(!complete);
    assert_eq!(
        items,
        [
            json!([
                "reference.effective_cycle",
                "effective reference cycle",
                "/sdfObject/X",
                "/sdfObject/X/sdfRef"
            ]),
            json!([
                "reference.effective_cycle",
                "effective reference cycle",
                "/sdfObject/Y",
                "/sdfObject/Y/sdfRef"
            ])
        ]
    );
    assert!(
        catalogue
            .effective(
                EffectiveRequest::new(catalogue.source_id("siren").unwrap())
                    .pointer("/sdfObject/Z")
            )
            .unwrap()
            .complete()
    );
}

#[test]
fn nested_path_composition_has_a_fixed_stack_bound() {
    // Each Thing's target lies in the next Thing's copy, so looking it up
    // composes the next Thing, whose own lookup composes the one after.
    let things: serde_json::Map<String, Value> = (0..300)
        .map(|index| {
            let thing = if index < 299 {
                json!({"sdfRef": format!("#/sdfThing/T{}/sdfThing/c", index + 1), "sdfThing": {"c": {}}})
            } else {
                json!({"sdfThing": {"c": {}}})
            };
            (format!("T{index}"), thing)
        })
        .collect();
    let text = serde_json::to_vec(&json!({"info": {}, "sdfThing": things})).unwrap();
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("siren", &text)]).unwrap();
    let id = catalogue.source_id("siren").unwrap();
    let complete = |pointer: &str| {
        catalogue
            .effective(EffectiveRequest::new(id).pointer(pointer))
            .unwrap()
            .complete()
    };
    // Eighteen nested compositions fit the bound.
    assert!(complete("/sdfThing/T280"));
    let (complete, items) = conditions(&catalogue, "/sdfThing/T0");
    assert!(!complete);
    assert_eq!(
        items,
        [json!([
            "operation.limit",
            "reference path nesting limit",
            "/sdfThing/T0",
            "/sdfThing/T0/sdfRef"
        ])]
    );
}

#[test]
fn path_composition_charges_the_shared_work_and_reference_allowances() {
    use esdieff::Limits;
    let siren = siren(json!({"sdfRequired": ["status"], "sdfProperty": {
        "loud": {"sdfRef": "#/sdfObject/siren/sdfProperty/duration"}
    }}));
    let inspect = |limits: Limits| {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("alarm", ALARM),
                SourceInput::supplied("siren", &siren),
            ],
            limits,
        )
        .unwrap();
        conditions(&catalogue, "/sdfObject/siren")
    };
    let limited = json!([
        "operation.limit",
        "reference application limit",
        "/sdfObject/siren/sdfProperty/loud",
        "/sdfObject/siren/sdfProperty/loud/sdfRef"
    ]);
    // Copying the alarm and the property's own reference leave nothing for
    // composing the property's path.
    assert_eq!(
        inspect(Limits::default().reference_applications(2)),
        (false, vec![limited])
    );
    assert_eq!(
        inspect(Limits::default().reference_applications(3)),
        (true, Vec::new())
    );
    let mut observed = false;
    for work in (100..=4000).step_by(25) {
        let (complete, items) = inspect(Limits::default().decoding_work(work));
        if complete {
            break;
        }
        observed |= items.iter().any(|item| {
            item[0] == "operation.limit"
                && item[2] == "/sdfObject/siren/sdfProperty/loud"
                && item[3] == "/sdfObject/siren/sdfProperty/loud/sdfRef"
        });
    }
    assert!(
        observed,
        "lowered work exhausts the property's path composition"
    );
}

#[test]
fn interpretation_is_listed_exactly_when_a_target_was_found_only_through_composition() {
    for (siren, listed) in [
        (siren(json!({"sdfRequired": ["status"]})), false),
        // The written importer member is the whole target, so composing its
        // path changes nothing.
        (
            siren(json!({"sdfRequired": ["status"], "sdfProperty": {
                "loud": {"sdfRef": "o:#/sdfObject/alarm/sdfProperty/duration"},
                "quiet": {"sdfRef": "#/sdfObject/siren/sdfProperty/loud"}
            }})),
            false,
        ),
        // A path that needs composition to an absent target.
        (
            siren(json!({"sdfRequired": ["status"], "sdfProperty": {
                "loud": {"sdfRef": "#/sdfObject/siren/sdfProperty/never"}
            }})),
            false,
        ),
        // The written refinement alone is not the target: its type is copied.
        (
            siren(json!({"sdfRequired": ["status"], "sdfProperty": {
                "duration": {"minimum": 5},
                "loud": {"sdfRef": "#/sdfObject/siren/sdfProperty/duration"}
            }})),
            true,
        ),
    ] {
        let catalogue = catalogue(&siren);
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        let expected = if listed {
            json!([RELOCATION])
        } else {
            json!([])
        };
        assert_eq!(wire(&report)["baseline"]["interpretations"], expected);
    }
}

#[test]
fn required_inspection_resolves_importer_pointer_in_both_views() {
    let siren = siren(json!({"sdfRequired": ["#/sdfObject/siren/sdfProperty/status"]}));
    let catalogue = catalogue(&siren);
    let id = catalogue.source_id("siren").unwrap();
    for (view, capability) in [
        (View::Effective, RequiredTargetCapability::Established),
        (View::Source, RequiredTargetCapability::NotRequested),
    ] {
        let report = catalogue
            .inspect(
                InspectRequest::new(id, "/sdfObject/siren")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(report.outcome().complete(), "{:#}", wire(&report));
        assert_eq!(report.outcome().notices(), 0);
        let reference = report
            .required_declarations()
            .require()
            .unwrap()
            .owners()
            .next()
            .unwrap()
            .references()
            .require()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("composed declaration")
        };
        let target = targets.next().unwrap();
        assert_eq!(target.capability(), capability);
        let source = target.source_definition().require().unwrap().json();
        assert_eq!(source.source_id(), catalogue.source_id("alarm").unwrap());
        assert_eq!(
            source.pointer().require().unwrap(),
            "/sdfObject/alarm/sdfProperty/status"
        );
        assert!(
            wire(&report)["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .contains(&json!(RELOCATION))
        );
    }
}

#[test]
fn third_document_curie_selects_the_copied_declaration_in_the_importers_resolved_model() {
    // Without a refinement the declaration exists only through the copy.
    for description in [None, Some("Siren sounding")] {
        let mut patch = json!({"sdfRequired": ["status"]});
        if let Some(description) = description {
            patch["sdfProperty"] = json!({"status": {"description": description}});
        }
        let siren = siren(patch);
        let panel = panel(json!({"sdfProperty": {
            "state": {"sdfRef": "s:#/sdfObject/siren/sdfProperty/status"}
        }}));
        let catalogue = catalogue_with_panel(&siren, &panel);
        let id = catalogue.source_id("panel").unwrap();
        let model = catalogue
            .effective(EffectiveRequest::new(id).pointer("/sdfObject/panel/sdfProperty/state"))
            .unwrap();
        assert!(model.complete());
        let state = model.json().require().unwrap();
        assert_eq!(
            state
                .get("type")
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap(),
            "boolean"
        );
        assert_eq!(
            state
                .get("description")
                .present()
                .map(|json| json.text().require().unwrap()),
            description
        );
        let explanation = model
            .explanation("/sdfObject/panel/sdfProperty/state/type")
            .require()
            .unwrap();
        assert_eq!(explanation.kind(), OriginKind::Inherited);
        assert_eq!(
            location(explanation.sources().next().unwrap(), &catalogue),
            (
                "alarm".to_owned(),
                "/sdfObject/alarm/sdfProperty/status/type".to_owned()
            )
        );
        assert_eq!(
            location(explanation.consumer().unwrap(), &catalogue),
            (
                "panel".to_owned(),
                "/sdfObject/panel/sdfProperty/state/sdfRef".to_owned()
            )
        );
    }
}

#[test]
fn third_document_requires_the_copied_declaration_through_its_global_name() {
    let siren = siren(json!({"sdfRequired": ["status"]}));
    let panel = panel(json!({
        "sdfProperty": {"state": {"type": "boolean"}},
        "sdfRequired": ["s:#/sdfObject/siren/sdfProperty/status"]
    }));
    let catalogue = catalogue_with_panel(&siren, &panel);
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let full = wire(&report);
    assert!(report.outcome().complete(), "{full:#}");
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().notices(), 0);
    assert_eq!(
        report
            .requirement_status("rfc9880.model.M020/1")
            .require()
            .unwrap(),
        AssessmentStatus::Assessed
    );
    assert_eq!(full["baseline"]["interpretations"], json!([RELOCATION]));
    let id = catalogue.source_id("panel").unwrap();
    for view in [View::Effective, View::Source] {
        let report = catalogue
            .inspect(
                InspectRequest::new(id, "/sdfObject/panel")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(report.outcome().complete(), "{:#}", wire(&report));
        let reference = report
            .required_declarations()
            .require()
            .unwrap()
            .owners()
            .next()
            .unwrap()
            .references()
            .require()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("composed declaration")
        };
        let target = targets.next().unwrap();
        assert_eq!(target.capability(), RequiredTargetCapability::NotRequested);
        assert_eq!(
            location(
                target.source_definition().require().unwrap().json(),
                &catalogue
            ),
            (
                "alarm".to_owned(),
                "/sdfObject/alarm/sdfProperty/status".to_owned()
            )
        );
        assert_eq!(
            wire(&report)["baseline"]["interpretations"],
            json!([RELOCATION])
        );
    }
}

#[test]
fn selected_scope_needs_the_imports_met_while_composing_the_contributors_path() {
    let siren = siren(json!({"sdfRequired": ["status"]}));
    let panel = panel(json!({"sdfProperty": {
        "state": {"sdfRef": "s:#/sdfObject/siren/sdfProperty/status"}
    }}));
    let catalogue = catalogue_with_panel(&siren, &panel);
    let root = catalogue.source_id("panel").unwrap();
    let needed = json!([
        catalogue.source_id("alarm").unwrap().get(),
        catalogue.source_id("siren").unwrap().get()
    ]);
    for report in [
        catalogue.check(CheckRequest::new().root(root)).unwrap(),
        catalogue
            .inspect(InspectRequest::new(root, "/sdfObject/panel").view(View::Effective))
            .unwrap(),
    ] {
        let wire = wire(&report);
        assert!(report.outcome().complete(), "{wire:#}");
        assert_eq!(wire["request"]["scope"]["dependencies"], needed);
        assert_eq!(wire["request"]["scope"]["unavailable_required"], json!([]));
    }
}

#[test]
fn ambiguous_or_unavailable_contributors_along_the_path_keep_scoped_outcomes() {
    let siren = siren(json!({"sdfRequired": ["status"]}));
    let panel = panel(json!({"sdfProperty": {
        "state": {"sdfRef": "s:#/sdfObject/siren/sdfProperty/status"}
    }}));
    let state = "/sdfObject/panel/sdfProperty/state";
    let reference = "/sdfObject/panel/sdfProperty/state/sdfRef";

    // Two importers contribute the same address through their copies.
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("alarm", ALARM),
        SourceInput::supplied("siren", &siren),
        SourceInput::supplied("other", &siren),
        SourceInput::supplied("panel", &panel),
    ])
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("panel").unwrap(), "/sdfObject/panel")
                .view(View::Effective),
        )
        .unwrap();
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    let wire = wire(&report);
    let condition = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "reference.ambiguous")
        .unwrap();
    assert_eq!(condition["primary"]["pointer"], state);
    let related = condition["related"].as_array().unwrap();
    for key in ["siren", "other"] {
        let id = catalogue.source_id(key).unwrap().get();
        assert!(
            related
                .iter()
                .any(|location| location["source_id"] == id
                    && location["pointer"] == "/sdfObject/siren"),
            "missing contributor {key}"
        );
    }

    // The importer's own import is ambiguous or unavailable. An unavailable
    // acquisition could also contribute the panel's namespace, so which
    // uncertainty names that outcome is not fixed.
    for (alarms, message) in [
        (
            vec![
                SourceInput::supplied("alarm", ALARM),
                SourceInput::supplied("second", ALARM),
            ],
            Some("reference path composition unavailable"),
        ),
        (vec![SourceInput::failed("alarm")], None),
    ] {
        let mut inputs = alarms;
        inputs.push(SourceInput::supplied("siren", &siren));
        inputs.push(SourceInput::supplied("panel", &panel));
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let (complete, items) = conditions_in(&catalogue, "panel", "/sdfObject/panel");
        assert!(!complete);
        let at_state: Vec<_> = items.iter().filter(|item| item[2] == state).collect();
        assert_eq!(at_state.len(), 1, "{items:?}");
        assert_eq!(at_state[0][0], "reference.unavailable");
        assert_eq!(at_state[0][3], reference);
        if let Some(message) = message {
            assert_eq!(at_state[0][1], message);
        }
    }
}

#[test]
fn global_names_record_the_interpretation_only_when_the_target_needs_composition() {
    let siren = siren(json!({"sdfRequired": ["status"]}));
    for (panel, listed) in [
        // The importer's written definition is the whole target.
        (panel(json!({"sdfRef": "s:#/sdfObject/siren"})), false),
        (
            panel(json!({"sdfProperty": {
                "state": {"sdfRef": "s:#/sdfObject/siren/sdfProperty/status"}
            }})),
            true,
        ),
    ] {
        let catalogue = catalogue_with_panel(&siren, &panel);
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        let wire = wire(&report);
        assert!(report.outcome().complete(), "{wire:#}");
        let expected = if listed {
            json!([RELOCATION])
        } else {
            json!([])
        };
        assert_eq!(wire["baseline"]["interpretations"], expected);
    }
}

#[test]
fn global_path_composition_charges_the_shared_work_and_reference_allowances() {
    use esdieff::Limits;
    let siren = siren(json!({"sdfRequired": ["status"]}));
    let panel = panel(json!({"sdfProperty": {
        "state": {"sdfRef": "s:#/sdfObject/siren/sdfProperty/status"}
    }}));
    let inspect = |limits: Limits| {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("alarm", ALARM),
                SourceInput::supplied("siren", &siren),
                SourceInput::supplied("panel", &panel),
            ],
            limits,
        )
        .unwrap();
        conditions_in(&catalogue, "panel", "/sdfObject/panel")
    };
    let limited = json!([
        "operation.limit",
        "reference application limit",
        "/sdfObject/panel/sdfProperty/state",
        "/sdfObject/panel/sdfProperty/state/sdfRef"
    ]);
    // The property's own reference leaves nothing for composing the
    // importer's copy of the alarm on the path.
    assert_eq!(
        inspect(Limits::default().reference_applications(1)),
        (false, vec![limited])
    );
    assert_eq!(
        inspect(Limits::default().reference_applications(2)),
        (true, Vec::new())
    );
    let mut observed = false;
    for work in (100..=4000).step_by(25) {
        let (complete, items) = inspect(Limits::default().decoding_work(work));
        if complete {
            break;
        }
        observed |= items.iter().any(|item| {
            item[0] == "operation.limit"
                && item[2] == "/sdfObject/panel/sdfProperty/state"
                && item[3] == "/sdfObject/panel/sdfProperty/state/sdfRef"
        });
    }
    assert!(
        observed,
        "lowered work exhausts the property's path composition"
    );
}
