//! Declaration obligations in corresponding groupings, following ADR 0054.
use super::comparison_support;
use comparison_support::{at, catalogue, pointer};
use esdieff::{
    Catalogue, Classification, ComparisonRequest, DifferenceAcknowledgement,
    DifferenceAcknowledgements, DifferenceKind, DifferenceSubject, Limits, Perspective, Presence,
    Revision, SourceInput,
};
use serde_json::{Value, json};

#[test]
fn new_and_released_obligations_compare_in_both_perspectives_and_directions() {
    for class in [
        "sdfProperty",
        "sdfAction",
        "sdfEvent",
        "sdfObject",
        "sdfThing",
    ] {
        let optional = json!({"info":{"version":"1"},"sdfThing":{"Device":{class:{"part":{}}}}});
        let mut mandatory = optional.clone();
        mandatory["sdfThing"]["Device"]["sdfRequired"] = json!(["part"]);
        for (old, new, entering) in [
            (&optional, &mandatory, true),
            (&mandatory, &optional, false),
        ] {
            let earlier = catalogue(&[("model", &old.to_string())]);
            let later = catalogue(&[("model", &new.to_string())]);
            for perspective in [Perspective::Client, Perspective::Implementer] {
                let expected = if entering == (perspective == Perspective::Implementer) {
                    Classification::Breaking
                } else {
                    Classification::Compatible
                };
                let comparison = ComparisonRequest::new(&earlier, &later)
                    .perspective(perspective)
                    .compare()
                    .unwrap();
                assert_eq!(
                    comparison.outcome(),
                    expected,
                    "{class}, {entering}, {perspective:?}"
                );
                assert_eq!(comparison.differences().len(), 1);
                let difference = at(&comparison, "/sdfThing/Device/sdfRequired");
                assert_eq!(difference.kind(), DifferenceKind::Changed);
                assert_eq!(difference.classification(), expected);
                assert!(difference.reason().is_none());
                assert!(difference.witness().is_none());
                assert_eq!(difference.directions().count(), 0);
                for revision in [Revision::Earlier, Revision::Later] {
                    assert_eq!(
                        pointer(&difference, revision),
                        "/sdfThing/Device/sdfRequired"
                    );
                    let present = entering == (revision == Revision::Later);
                    assert_eq!(
                        difference.side(revision).unwrap().presence(),
                        if present {
                            Presence::Present
                        } else {
                            Presence::Absent
                        }
                    );
                }
                assert_eq!(
                    comparison.version_warnings().len(),
                    usize::from(expected == Classification::Breaking)
                );
                assert_eq!(
                    comparison.disposition().exit_code(),
                    if expected == Classification::Breaking {
                        1
                    } else {
                        0
                    }
                );
            }
        }
    }
}

fn oven(required: Value) -> String {
    json!({"sdfObject":{"Oven":{
        "sdfRequired":required,
        "sdfProperty":{"temp":{"type":"number"},"mode":{"type":"string"}},
        "sdfAction":{"start":{}},"sdfEvent":{"ready":{}}
    }}})
    .to_string()
}

#[test]
fn simultaneous_new_and_released_obligations_break_both_perspectives() {
    for (old, new) in [("temp", "mode"), ("mode", "temp")] {
        let earlier = catalogue(&[("model", &oven(json!([old])))]);
        let later = catalogue(&[("model", &oven(json!([new])))]);
        for perspective in [Perspective::Client, Perspective::Implementer] {
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(perspective)
                .compare()
                .unwrap();
            assert_eq!(comparison.differences().len(), 1);
            assert_eq!(comparison.outcome(), Classification::Breaking);
        }
    }
}

fn kitchen(outer: Option<Value>, inner: Option<Value>) -> String {
    let mut model = json!({"sdfThing":{"Kitchen":{"sdfObject":{"Oven":{
        "sdfProperty":{"temp":{"type":"number"}}
    }}}}});
    if let Some(required) = outer {
        model["sdfThing"]["Kitchen"]["sdfRequired"] = required;
    }
    if let Some(required) = inner {
        model["sdfThing"]["Kitchen"]["sdfObject"]["Oven"]["sdfRequired"] = required;
    }
    model.to_string()
}

#[test]
fn nested_obligations_are_conditional_and_close_through_mandatory_groupings() {
    for outer in [None, Some(json!(["Oven"]))] {
        let optional = kitchen(outer.clone(), None);
        let mandatory = kitchen(outer.clone(), Some(json!(["temp"])));
        for (old, new, expected) in [
            (&optional, &mandatory, Classification::Breaking),
            (&mandatory, &optional, Classification::Compatible),
        ] {
            let earlier = catalogue(&[("model", old)]);
            let later = catalogue(&[("model", new)]);
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            assert_eq!(comparison.outcome(), expected);
            assert_eq!(
                comparison.differences().len(),
                if outer.is_some() { 2 } else { 1 }
            );
            assert_eq!(
                at(&comparison, "/Oven/sdfRequired").classification(),
                expected
            );
        }
    }
    let mandatory = kitchen(Some(json!(["Oven"])), Some(json!(["temp"])));
    let optional = kitchen(None, Some(json!(["temp"])));
    for (old, new, expected) in [
        (&mandatory, &optional, Classification::Compatible),
        (&optional, &mandatory, Classification::Breaking),
    ] {
        let earlier = catalogue(&[("model", old)]);
        let later = catalogue(&[("model", new)]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 1);
        assert_eq!(
            at(&comparison, "/Kitchen/sdfRequired").classification(),
            expected
        );
    }
}

#[test]
fn equivalent_closures_ignore_spelling_order_redundancy_and_carrier_locations() {
    let pointer = "#/sdfObject/Oven/sdfProperty/temp";
    let mut carrier: Value = serde_json::from_str(&oven(json!([]))).unwrap();
    carrier["sdfObject"]["Oven"]
        .as_object_mut()
        .unwrap()
        .remove("sdfRequired");
    carrier["sdfObject"]["Oven"]["sdfProperty"]["temp"]["sdfRequired"] = json!([true]);
    let spellings = [
        oven(json!(["temp"])),
        oven(json!([pointer])),
        oven(json!([pointer, "temp", pointer])),
        carrier.to_string(),
    ];
    let nested = [
        kitchen(
            Some(json!([
                "#/sdfThing/Kitchen/sdfObject/Oven/sdfProperty/temp"
            ])),
            None,
        ),
        kitchen(Some(json!(["Oven"])), Some(json!(["temp"]))),
    ];
    let reordered = [
        oven(json!(["temp", "mode"])),
        oven(json!(["mode", "temp", "mode"])),
    ];
    for forms in [
        spellings.as_slice(),
        nested.as_slice(),
        reordered.as_slice(),
    ] {
        for old in forms {
            for new in forms {
                let earlier = catalogue(&[("model", old)]);
                let later = catalogue(&[("model", new)]);
                for perspective in [Perspective::Client, Perspective::Implementer] {
                    let comparison = ComparisonRequest::new(&earlier, &later)
                        .perspective(perspective)
                        .compare()
                        .unwrap();
                    assert_eq!(comparison.differences().len(), 0, "{old} -> {new}");
                }
            }
        }
    }
}

#[test]
fn shared_names_select_every_eligible_class_and_affordances_use_their_enclosing_grouping() {
    let optional = json!({"sdfObject":{"Oven":{
        "sdfProperty":{"start":{"type":"boolean"}},"sdfAction":{"start":{}}
    }}});
    let mut property_only = optional.clone();
    property_only["sdfObject"]["Oven"]["sdfRequired"] =
        json!(["#/sdfObject/Oven/sdfProperty/start"]);
    let mut both = optional.clone();
    both["sdfObject"]["Oven"]["sdfRequired"] = json!(["start"]);
    let earlier = catalogue(&[("model", &property_only.to_string())]);
    let later = catalogue(&[("model", &both.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    for requirement in [json!([true]), json!(["#/sdfObject/Oven/sdfProperty/start"])] {
        let mut after = optional.clone();
        after["sdfObject"]["Oven"]["sdfAction"]["start"]["sdfRequired"] = requirement;
        let earlier = catalogue(&[("model", &optional.to_string())]);
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 1);
        let difference = at(&comparison, "/Oven/sdfRequired");
        assert_eq!(difference.classification(), Classification::Breaking);
        for revision in [Revision::Earlier, Revision::Later] {
            assert_eq!(
                difference.side(revision).unwrap().presence(),
                Presence::Absent
            );
        }
    }
}

#[test]
fn added_and_removed_endpoints_have_no_duplicate_requiredness_change() {
    let before = oven(json!(["temp"]));
    let mut after: Value = serde_json::from_str(&before).unwrap();
    after["sdfObject"]["Oven"]["sdfProperty"]["door"] =
        json!({"type":"boolean","sdfRequired":[true]});
    for (old, new, expected) in [
        (&before, &after.to_string(), Classification::Breaking),
        (&after.to_string(), &before, Classification::Compatible),
    ] {
        let earlier = catalogue(&[("model", old)]);
        let later = catalogue(&[("model", new)]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 1);
        assert_eq!(at(&comparison, "/door").classification(), expected);
        assert_eq!(
            comparison.disposition().exit_code(),
            if expected == Classification::Breaking {
                1
            } else {
                0
            }
        );
    }
    after["sdfObject"]["Oven"]["sdfRequired"] = json!(["temp", "mode"]);
    let earlier = catalogue(&[("model", &before)]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 2);
    assert_eq!(
        at(&comparison, "/Oven/sdfRequired").classification(),
        Classification::Breaking
    );
    assert_eq!(
        at(&comparison, "/door").classification(),
        Classification::Breaking
    );
    assert_eq!(comparison.disposition().exit_code(), 1);
}

#[test]
fn partial_resolution_keeps_its_reason_and_unchanged_unresolved_relationships_stay_unchanged() {
    for (old, new) in [
        (json!(["temp"]), json!(["temp", "missing"])),
        (json!(["temp", "missing"]), json!(["mode", "missing"])),
        (json!(["temp"]), json!(["missing"])),
        (
            json!([]),
            json!(["elsewhere:#/sdfObject/Oven/sdfProperty/temp"]),
        ),
    ] {
        let earlier = catalogue(&[("model", &oven(old))]);
        let later = catalogue(&[("model", &oven(new))]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        let difference = at(&comparison, "/Oven/sdfRequired");
        assert_eq!(difference.classification(), Classification::Undetermined);
        assert_eq!(difference.reason().unwrap().code(), "required.unavailable");
    }
    let unresolved = oven(json!(["missing"]));
    let earlier = catalogue(&[("model", &unresolved)]);
    let later = catalogue(&[("model", &unresolved)]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 0);
}

#[test]
fn copied_requirements_follow_effective_coordinates_and_keep_nearest_origins() {
    let base = json!({"sdfObject":{"Base":{
        "sdfRequired":["#/sdfObject/Base/sdfProperty/temp"],
        "sdfProperty":{"temp":{"type":"number"}}
    }}})
    .to_string();
    let mut optional: Value = serde_json::from_str(&base).unwrap();
    optional["sdfObject"]["Use"] = json!({"sdfRef":"#/sdfObject/Base","sdfRequired":null});
    let mut required = optional.clone();
    required["sdfObject"]["Use"]
        .as_object_mut()
        .unwrap()
        .remove("sdfRequired");
    let earlier = catalogue(&[("model", &optional.to_string())]);
    let later = catalogue(&[("model", &required.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 1);
    let difference = at(&comparison, "/Use/sdfRequired");
    assert_eq!(difference.classification(), Classification::Breaking);
    assert_eq!(
        difference
            .side(Revision::Later)
            .unwrap()
            .origin()
            .unwrap()
            .sources()
            .next()
            .unwrap()
            .pointer()
            .require()
            .unwrap(),
        "/sdfObject/Base/sdfRequired"
    );
    // A deleted and reinserted declaration is matched by its effective position.
    required["sdfObject"]["Removed"] =
        json!({"sdfRef":"#/sdfObject/Base","sdfRequired":null,"sdfProperty":{"temp":null}});
    required["sdfObject"]["Use"] = json!({"sdfRef":"#/sdfObject/Removed","sdfProperty":{"temp":{"type":"number"}},"sdfRequired":["temp"]});
    optional = required.clone();
    optional["sdfObject"]["Use"]["sdfRequired"] = json!([]);
    let earlier = catalogue(&[("model", &optional.to_string())]);
    let later = catalogue(&[("model", &required.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(
        at(&comparison, "/Use/sdfRequired").classification(),
        Classification::Breaking
    );
}

#[test]
fn old_unknown_acknowledgements_do_not_match_new_classifications() {
    let path = "/sdfObject/Oven/sdfRequired";
    let entry = DifferenceAcknowledgement::new(
        DifferenceKind::Changed,
        DifferenceSubject::Quality("sdfRequired"),
        Classification::Undetermined,
    )
    .unwrap()
    .earlier("model", path)
    .unwrap()
    .later("model", path)
    .unwrap();
    let old = DifferenceAcknowledgements::new(vec![entry]).unwrap();
    let optional = catalogue(&[("model", &oven(json!([])))]);
    let mandatory = catalogue(&[("model", &oven(json!(["temp"])))]);
    for (earlier, later, expected) in [
        (&optional, &mandatory, Classification::Breaking),
        (&mandatory, &optional, Classification::Compatible),
    ] {
        let comparison = ComparisonRequest::new(earlier, later)
            .perspective(Perspective::Implementer)
            .acknowledgements(&old)
            .compare()
            .unwrap();
        assert_eq!(comparison.outcome(), expected);
        assert_eq!(
            comparison.unused_acknowledgements().collect::<Vec<_>>(),
            [0]
        );
        assert!(!at(&comparison, path).acknowledged());
        if expected == Classification::Breaking {
            let entry = at(&comparison, path).acknowledgement().unwrap();
            let updated = DifferenceAcknowledgements::new(vec![entry]).unwrap();
            let acknowledged = ComparisonRequest::new(earlier, later)
                .perspective(Perspective::Implementer)
                .acknowledgements(&updated)
                .compare()
                .unwrap();
            assert_eq!(acknowledged.disposition().exit_code(), 0);
            assert_eq!(acknowledged.outcome(), Classification::Breaking);
        }
    }
}

#[test]
fn bounded_comparison_never_proves_requiredness_from_incomplete_sets() {
    let optional = oven(json!([]));
    let mandatory = oven(json!(["temp"]));
    let mut limited = false;
    for work in [1, 100, 1000, 10_000, 100_000] {
        let limits = Limits::default().decoding_work(work);
        let earlier = Catalogue::with_limits(
            &[SourceInput::supplied("model", optional.as_bytes())],
            limits.clone(),
        )
        .unwrap();
        let later = Catalogue::with_limits(
            &[SourceInput::supplied("model", mandatory.as_bytes())],
            limits,
        )
        .unwrap();
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        for difference in comparison.differences() {
            if difference.classification() == Classification::Undetermined {
                assert!(matches!(
                    difference.reason().unwrap().code(),
                    "operation.limit" | "required.unavailable"
                ));
                limited = true;
            }
        }
        assert_ne!(comparison.outcome(), Classification::Compatible);
    }
    assert!(limited);
}

#[test]
fn declaration_requiredness_keeps_data_and_structural_boundaries_independent() {
    let before = json!({"sdfObject":{"Oven":{
        "sdfProperty":{"temp":{"type":"number","writable":false,"maximum":10}},
        "sdfContext":{"metadata":{"type":"string"}}
    }}});
    let mut after = before.clone();
    after["sdfObject"]["Oven"]["sdfProperty"]["temp"]["sdfRequired"] = json!([true]);
    after["sdfObject"]["Oven"]["sdfProperty"]["temp"]["maximum"] = json!(20);
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 2);
    assert_eq!(
        at(&comparison, "/Oven/sdfRequired").classification(),
        Classification::Breaking
    );
    assert_eq!(
        at(&comparison, "/temp/maximum").classification(),
        Classification::Compatible
    );

    for (class, name, changes, expected) in [
        (
            "sdfContext",
            "metadata",
            json!({"sdfRequired":[true]}),
            Classification::Compatible,
        ),
        (
            "sdfProperty",
            "temp",
            json!({"writable":true}),
            Classification::Breaking,
        ),
        (
            "sdfProperty",
            "temp",
            json!({"sdfRequired":["missing"]}),
            Classification::Undetermined,
        ),
    ] {
        let mut after = before.clone();
        for (quality, value) in changes.as_object().unwrap() {
            after["sdfObject"]["Oven"][class][name][quality] = value.clone();
        }
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.outcome(), expected, "{changes}");
    }

    // A top-level carrier has no enclosing grouping in either revision.
    for class in ["sdfProperty", "sdfObject", "sdfThing"] {
        let before = json!({class:{"Standalone":{}}});
        let after = json!({class:{"Standalone":{"sdfRequired":[true]}}});
        let earlier = catalogue(&[("model", &before.to_string())]);
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 0);
    }
}

#[test]
fn data_required_and_nullable_keep_their_value_rules_and_data_only_sdf_required_stays_unknown() {
    for (before, after, suffix, expected) in [
        (
            json!({"type":"object","properties":{"x":{"type":"number"}}}),
            json!({"type":"object","properties":{"x":{"type":"number"}},"required":["x"]}),
            "/required",
            Classification::Breaking,
        ),
        (
            json!({"type":"number","nullable":true}),
            json!({"type":"number","nullable":false}),
            "/nullable",
            Classification::Breaking,
        ),
        (
            json!({"type":"object","properties":{"x":{"type":"number"}}}),
            json!({"type":"object","properties":{"x":{"type":"number","sdfRequired":[true]}}}),
            "/properties/x/sdfRequired",
            Classification::Undetermined,
        ),
    ] {
        let model = |data| json!({"sdfObject":{"Oven":{"sdfProperty":{"temp":data}}}}).to_string();
        let earlier = catalogue(&[("model", &model(before))]);
        let later = catalogue(&[("model", &model(after))]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 1);
        let difference = at(&comparison, suffix);
        assert_eq!(difference.classification(), expected);
        if expected == Classification::Breaking {
            assert!(difference.witness().is_some());
        } else {
            assert_eq!(
                difference.reason().unwrap().code(),
                "comparison.not_classified"
            );
        }
    }
}

#[test]
fn copied_cross_document_requirements_and_unavailable_composition_keep_separate_results() {
    let base = r##"{"namespace":{"base":"urn:base"},"defaultNamespace":"base",
        "sdfObject":{"Base":{"sdfRequired":["#/sdfObject/Base/sdfProperty/temp"],
        "sdfProperty":{"temp":{"type":"number"}}}}}"##;
    let before = r##"{"namespace":{"b":"urn:base"},"sdfObject":{"Oven":{
        "sdfRef":"b:#/sdfObject/Base","sdfRequired":null}}}"##;
    let after = before.replace(",\"sdfRequired\":null", "");
    let earlier = catalogue(&[("base", base), ("model", before)]);
    let later = catalogue(&[("base", base), ("model", &after)]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 1);
    assert_eq!(
        at(&comparison, "/Oven/sdfRequired").classification(),
        Classification::Breaking
    );

    let before = json!({"sdfObject":{"Oven":{"sdfProperty":{"temp":{"type":"number"}}},
        "Broken":{"sdfRef":"#/missing"}}});
    let mut after = before.clone();
    after["sdfObject"]["Oven"]["sdfRequired"] = json!(["temp"]);
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    // Existing document-wide relationship resolution remains conservative beside
    // unavailable composition. Its independent unavailable row also survives.
    assert_eq!(comparison.outcome(), Classification::Undetermined);
    assert_eq!(
        at(&comparison, "/Oven/sdfRequired")
            .reason()
            .unwrap()
            .code(),
        "required.unavailable"
    );
    assert!(
        comparison
            .differences()
            .any(|difference| difference.kind() == DifferenceKind::Unavailable)
    );
}
