//! Declaration presence from the implementer's perspective, following ADR 0055.
use super::comparison_support;
use comparison_support::{at, catalogue, pointer};
use esdieff::{Classification, ComparisonRequest, DifferenceKind, Perspective, Revision};
use serde_json::json;

#[test]
fn optional_and_mandatory_presence_in_both_directions_and_perspectives() {
    for class in [
        "sdfProperty",
        "sdfAction",
        "sdfEvent",
        "sdfObject",
        "sdfThing",
    ] {
        let absent = json!({"info":{"version":"1"},"sdfThing":{"Device":{}}});
        for mandatory in [false, true] {
            let mut present = absent.clone();
            present["sdfThing"]["Device"][class] = json!({"part":{}});
            if mandatory {
                present["sdfThing"]["Device"]["sdfRequired"] = json!(["part"]);
            }
            for added in [false, true] {
                let (old, new) = if added {
                    (&absent, &present)
                } else {
                    (&present, &absent)
                };
                let earlier = catalogue(&[("model", &old.to_string())]);
                let later = catalogue(&[("model", &new.to_string())]);
                for perspective in [Perspective::Client, Perspective::Implementer] {
                    let expected = match perspective {
                        Perspective::Client if !added => Classification::Breaking,
                        Perspective::Implementer if added && mandatory => Classification::Breaking,
                        _ => Classification::Compatible,
                    };
                    let comparison = ComparisonRequest::new(&earlier, &later)
                        .perspective(perspective)
                        .compare()
                        .unwrap();
                    assert_eq!(
                        comparison.differences().len(),
                        1,
                        "{class} {mandatory} {added} {perspective:?}"
                    );
                    let difference = at(&comparison, "/part");
                    assert_eq!(
                        difference.classification(),
                        expected,
                        "{class} {mandatory} {added} {perspective:?}"
                    );
                    assert_eq!(
                        difference.kind(),
                        if added {
                            DifferenceKind::Added
                        } else {
                            DifferenceKind::Removed
                        }
                    );
                    let revision = if added {
                        Revision::Later
                    } else {
                        Revision::Earlier
                    };
                    assert_eq!(
                        pointer(&difference, revision),
                        format!("/sdfThing/Device/{class}/part")
                    );
                    assert!(
                        difference
                            .side(if added {
                                Revision::Earlier
                            } else {
                                Revision::Later
                            })
                            .is_none()
                    );
                    assert!(difference.witness().is_none());
                    assert_eq!(difference.directions().count(), 0);
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
}

#[test]
fn conditional_subtrees_and_incoming_requirements_keep_maximal_rows() {
    let empty = json!({"sdfThing":{"Kitchen":{}}});
    let optional_oven = json!({"sdfThing":{"Kitchen":{"sdfObject":{"Oven":{}}}}});
    let child = json!({"sdfThing":{"Kitchen":{"sdfObject":{"Oven":{
        "sdfRequired":["temp"],"sdfProperty":{"temp":{"type":"number"}}
    }}}}});
    for (before, outer, expected, suffix) in [
        (&optional_oven, None, Classification::Breaking, "/temp"),
        (&empty, None, Classification::Compatible, "/Oven"),
        (
            &empty,
            Some(json!(["Oven"])),
            Classification::Breaking,
            "/Oven",
        ),
        (
            &empty,
            Some(json!([
                "#/sdfThing/Kitchen/sdfObject/Oven/sdfProperty/temp"
            ])),
            Classification::Breaking,
            "/Oven",
        ),
    ] {
        let mut after = child.clone();
        if let Some(required) = outer {
            after["sdfThing"]["Kitchen"]["sdfRequired"] = required;
        }
        for reverse in [false, true] {
            let (old, new) = if reverse {
                (&after, before)
            } else {
                (before, &after)
            };
            let earlier = catalogue(&[("model", &old.to_string())]);
            let later = catalogue(&[("model", &new.to_string())]);
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            assert_eq!(comparison.differences().len(), 1);
            assert_eq!(
                at(&comparison, suffix).classification(),
                if reverse {
                    Classification::Compatible
                } else {
                    expected
                }
            );
        }
    }
    // An incoming pointer from another corresponding grouping establishes an obligation.
    let before = json!({"sdfThing":{"Kitchen":{}},"sdfObject":{"Controller":{}}});
    let mut after = child;
    after["sdfObject"] = json!({"Controller":{"sdfRequired":["#/sdfThing/Kitchen/sdfObject/Oven/sdfProperty/temp"]}});
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 1);
    assert_eq!(
        at(&comparison, "/Oven").classification(),
        Classification::Breaking
    );
}

#[test]
fn reference_forms_and_same_name_classes_use_resolved_membership() {
    let before = json!({"sdfThing":{"Device":{}}});
    let path = "#/sdfThing/Device/sdfProperty/part";
    for required in [
        json!(["part"]),
        json!([path]),
        json!([path, "part", path]),
        json!(["part", path]),
    ] {
        let after =
            json!({"sdfThing":{"Device":{"sdfRequired":required,"sdfProperty":{"part":{}}}}});
        let earlier = catalogue(&[("model", &before.to_string())]);
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 1);
        assert_eq!(
            at(&comparison, "/part").classification(),
            Classification::Breaking
        );
    }
    for carrier in [false, true] {
        let mut after = json!({"sdfThing":{"Device":{
            "sdfRequired":["part"],"sdfProperty":{"part":{}},"sdfAction":{"part":{}},"sdfEvent":{"part":{}}
        }}});
        if carrier {
            after["sdfThing"]["Device"]
                .as_object_mut()
                .unwrap()
                .remove("sdfRequired");
            for class in ["sdfProperty", "sdfAction", "sdfEvent"] {
                after["sdfThing"]["Device"][class]["part"]["sdfRequired"] = json!([true]);
            }
        }
        let earlier = catalogue(&[("model", &before.to_string())]);
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 3);
        assert!(
            comparison
                .differences()
                .all(|d| d.classification() == Classification::Breaking)
        );
    }
}

#[test]
fn independent_existing_obligations_are_not_absorbed_by_presence() {
    let before = json!({"sdfObject":{"Oven":{"sdfProperty":{"mode":{}}}}});
    // The new optional affordance owns a requirement of the corresponding mode.
    let after = json!({"sdfObject":{"Oven":{"sdfProperty":{"mode":{},"temp":{"sdfRequired":["#/sdfObject/Oven/sdfProperty/mode"]}}}}});
    for reverse in [false, true] {
        let (old, new) = if reverse {
            (&after, &before)
        } else {
            (&before, &after)
        };
        let earlier = catalogue(&[("model", &old.to_string())]);
        let later = catalogue(&[("model", &new.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 2);
        assert_eq!(
            at(&comparison, "/temp").classification(),
            Classification::Compatible
        );
        assert_eq!(
            at(&comparison, "/Oven/sdfRequired").classification(),
            if reverse {
                Classification::Compatible
            } else {
                Classification::Breaking
            }
        );
    }
    let before = json!({"sdfObject":{"Oven":{"sdfProperty":{"mode":{},"temp":{}}}}});
    let after = json!({"sdfObject":{"Oven":{"sdfRequired":["mode"],"sdfProperty":{"mode":{}}}}});
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 2);
    assert_eq!(
        at(&comparison, "/temp").classification(),
        Classification::Compatible
    );
    assert_eq!(
        at(&comparison, "/Oven/sdfRequired").classification(),
        Classification::Breaking
    );
}

#[test]
fn later_resolution_is_required_for_additions_and_removals() {
    for added in [false, true] {
        for required in [
            json!(["missing"]),
            json!(["#/sdfObject/Oven/sdfProperty/temp"]),
        ] {
            let before = json!({"sdfObject":{"Oven":{"sdfProperty":{"temp":{}}}}});
            let mut after = json!({"sdfObject":{"Oven":{"sdfRequired":required}}});
            let mut before = before;
            if added {
                before["sdfObject"]["Oven"]
                    .as_object_mut()
                    .unwrap()
                    .remove("sdfProperty");
                after["sdfObject"]["Oven"]["sdfProperty"] = json!({"temp":{}});
                after["sdfObject"]["Oven"]["sdfRequired"] = json!(["temp", "missing"]);
            } else {
                // No syntax change, so there need not be a separate requiredness row.
                before["sdfObject"]["Oven"]["sdfRequired"] =
                    after["sdfObject"]["Oven"]["sdfRequired"].clone();
            }
            let earlier = catalogue(&[("model", &before.to_string())]);
            let later = catalogue(&[("model", &after.to_string())]);
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            let presence = at(&comparison, "/temp");
            assert_eq!(presence.classification(), Classification::Undetermined);
            assert_eq!(presence.reason().unwrap().code(), "required.unavailable");
            assert_eq!(
                presence.kind(),
                if added {
                    DifferenceKind::Added
                } else {
                    DifferenceKind::Removed
                }
            );
            assert_eq!(comparison.disposition().exit_code(), 3);
            if !added {
                assert_eq!(comparison.differences().len(), 1);
            }
        }
    }
    // An earlier unresolved requirement cannot invalidate an established later obligation.
    let before = json!({"sdfObject":{"Oven":{"sdfRequired":["missing"]}}});
    let after = json!({"sdfObject":{"Oven":{"sdfRequired":["temp"],"sdfProperty":{"temp":{}}}}});
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(
        at(&comparison, "/temp").classification(),
        Classification::Breaking
    );
    assert_eq!(
        at(&comparison, "/Oven/sdfRequired").classification(),
        Classification::Undetermined
    );
    assert_eq!(comparison.disposition().exit_code(), 1);
}

#[test]
fn copied_refined_and_reinserted_presence_uses_consumer_coordinates_and_nearest_origins() {
    let base = json!({"sdfObject":{"Base":{
        "sdfRequired":["#/sdfObject/Base/sdfProperty/temp"],"sdfProperty":{"temp":{"type":"number"}}
    }}});
    let mut after = base.clone();
    after["sdfObject"]["Use"] = json!({"sdfRef":"#/sdfObject/Base"});
    let mut before = after.clone();
    before["sdfObject"]["Use"]["sdfProperty"] = json!({"temp":null});
    before["sdfObject"]["Use"]["sdfRequired"] = json!(null);
    for refined in [false, true] {
        let mut new = after.clone();
        if refined {
            new["sdfObject"]["Use"]["sdfProperty"] = json!({"temp":{"maximum":20}});
        }
        let earlier = catalogue(&[("model", &before.to_string())]);
        let later = catalogue(&[("model", &new.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 1);
        let difference = at(&comparison, "/Use/sdfProperty/temp");
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
            if refined {
                "/sdfObject/Use/sdfProperty/temp"
            } else {
                "/sdfObject/Base/sdfProperty/temp"
            }
        );
    }
    // Reinsertion creates the effective declaration at the consumer even after donor deletion.
    after["sdfObject"]["Removed"] = before["sdfObject"]["Use"].clone();
    before["sdfObject"]["Removed"] = before["sdfObject"]["Use"].clone();
    before["sdfObject"]["Use"] = json!({"sdfRef":"#/sdfObject/Removed"});
    after["sdfObject"]["Use"] = json!({"sdfRef":"#/sdfObject/Removed","sdfProperty":{"temp":{"type":"number"}},"sdfRequired":["temp"]});
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 1);
    assert_eq!(
        at(&comparison, "/Use/sdfProperty/temp").classification(),
        Classification::Breaking
    );
}

#[test]
fn top_level_boundaries_and_obsolete_payloads_keep_their_meaning() {
    for class in [
        "sdfThing",
        "sdfObject",
        "sdfProperty",
        "sdfAction",
        "sdfEvent",
        "sdfData",
    ] {
        let absent = catalogue(&[("model", "{}")]);
        let present = catalogue(&[(
            "model",
            &json!({class:{"Standalone":{"sdfRequired":[true]}}}).to_string(),
        )]);
        for (earlier, later) in [(&absent, &present), (&present, &absent)] {
            let comparison = ComparisonRequest::new(earlier, later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            if matches!(class, "sdfThing" | "sdfObject") {
                assert_eq!(comparison.outcome(), Classification::Undetermined);
                assert_eq!(comparison.differences().len(), 1);
            } else {
                assert_eq!(comparison.differences().len(), 0);
            }
        }
    }
    // Releasing a declaration does not require evaluating its old data constraints.
    let before = json!({"sdfObject":{"Oven":{"sdfProperty":{"temp":{"type":"number","minimum":"invalid"}}}}});
    let after = json!({"sdfObject":{"Oven":{}}});
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(
        at(&comparison, "/temp").classification(),
        Classification::Compatible
    );
    // No identity mapping across a rename. An optional replacement is compatible, a
    // mandatory replacement is breaking at the addition alone.
    for mandatory in [false, true] {
        let before = json!({"sdfObject":{"Oven":{"sdfProperty":{"old":{}}}}});
        let mut after = json!({"sdfObject":{"Oven":{"sdfProperty":{"new":{}}}}});
        if mandatory {
            after["sdfObject"]["Oven"]["sdfRequired"] = json!(["new"]);
        }
        let earlier = catalogue(&[("model", &before.to_string())]);
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 2);
        assert_eq!(
            at(&comparison, "/old").classification(),
            Classification::Compatible
        );
        assert_eq!(
            at(&comparison, "/new").classification(),
            if mandatory {
                Classification::Breaking
            } else {
                Classification::Compatible
            }
        );
    }
}

#[test]
fn unavailable_composition_and_limits_cannot_establish_presence_obligations() {
    for added in [false, true] {
        let absent = json!({"sdfObject":{"Oven":{}}});
        let present = json!({"sdfObject":{"Oven":{"sdfProperty":{"temp":{"sdfRef":"#/missing"}}}}});
        let (old, new) = if added {
            (&absent, &present)
        } else {
            (&present, &absent)
        };
        let earlier = catalogue(&[("model", &old.to_string())]);
        let later = catalogue(&[("model", &new.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(
            at(&comparison, "/temp").classification(),
            Classification::Undetermined
        );
        assert!(
            at(&comparison, "/temp")
                .reason()
                .unwrap()
                .code()
                .starts_with("reference.")
        );
    }
    use esdieff::{Catalogue, Limits, SourceInput};
    let old = r#"{"sdfObject":{"Oven":{}}}"#;
    let new = r#"{"sdfObject":{"Oven":{"sdfRequired":["temp"],"sdfProperty":{"temp":{}}}}}"#;
    let mut limited = false;
    for (old, new) in [(old, new), (new, old)] {
        for work in [1, 100, 1000, 10_000, 100_000] {
            let limits = Limits::default().decoding_work(work);
            let earlier = Catalogue::with_limits(
                &[SourceInput::supplied("model", old.as_bytes())],
                limits.clone(),
            )
            .unwrap();
            let later =
                Catalogue::with_limits(&[SourceInput::supplied("model", new.as_bytes())], limits)
                    .unwrap();
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            for difference in comparison.differences() {
                if difference.classification() == Classification::Undetermined {
                    limited = true;
                    assert!(matches!(
                        difference.reason().unwrap().code(),
                        "operation.limit" | "required.unavailable"
                    ));
                }
            }
        }
    }
    assert!(limited);
}

#[test]
fn old_presence_acknowledgements_do_not_match_new_classifications() {
    use esdieff::{
        Class, DifferenceAcknowledgement, DifferenceAcknowledgements, DifferenceSubject,
    };
    let earlier = catalogue(&[("model", r#"{"sdfObject":{"Oven":{}}}"#)]);
    for mandatory in [false, true] {
        let mut after = json!({"sdfObject":{"Oven":{"sdfProperty":{"temp":{}}}}});
        if mandatory {
            after["sdfObject"]["Oven"]["sdfRequired"] = json!(["temp"]);
        }
        let later = catalogue(&[("model", &after.to_string())]);
        let old = DifferenceAcknowledgements::new(vec![
            DifferenceAcknowledgement::new(
                DifferenceKind::Added,
                DifferenceSubject::Definition(Class::Property),
                Classification::Undetermined,
            )
            .unwrap()
            .later("model", "/sdfObject/Oven/sdfProperty/temp")
            .unwrap(),
        ])
        .unwrap();
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .acknowledgements(&old)
            .compare()
            .unwrap();
        assert_eq!(
            comparison.unused_acknowledgements().collect::<Vec<_>>(),
            [0]
        );
        assert!(!at(&comparison, "/temp").acknowledged());
        if mandatory {
            let updated = DifferenceAcknowledgements::new(vec![
                at(&comparison, "/temp").acknowledgement().unwrap(),
            ])
            .unwrap();
            let accepted = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .acknowledgements(&updated)
                .compare()
                .unwrap();
            assert_eq!(accepted.disposition().exit_code(), 0);
        }
    }
}

#[test]
fn ambiguous_requirements_keep_presence_unknown_and_independent_breaks_visible() {
    let provider = r#"{"namespace":{"p":"urn:duplicate"},"defaultNamespace":"p","sdfObject":{"Base":{"sdfProperty":{"value":{}}}}}"#;
    for added in [false, true] {
        let base = json!({"namespace":{"p":"urn:duplicate"},"sdfObject":{"Oven":{
            "sdfRequired":["p:#/sdfObject/Base/sdfProperty/value"],
            "sdfProperty":{"mode":{"type":"number","minimum":0,"writable":true,"readable":false,"observable":false}}
        }}});
        let mut old = base.clone();
        let mut new = base;
        if added {
            new["sdfObject"]["Oven"]["sdfProperty"]["temp"] = json!({});
        } else {
            old["sdfObject"]["Oven"]["sdfProperty"]["temp"] = json!({});
        }
        // Broader accepted inputs independently require implementation work.
        new["sdfObject"]["Oven"]["sdfProperty"]["mode"]["minimum"] = json!(-1);
        let earlier = catalogue(&[
            ("model", &old.to_string()),
            ("p1", provider),
            ("p2", provider),
        ]);
        let later = catalogue(&[
            ("model", &new.to_string()),
            ("p1", provider),
            ("p2", provider),
        ]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(
            at(&comparison, "/temp").reason().unwrap().code(),
            "required.unavailable"
        );
        assert_eq!(
            at(&comparison, "/mode/minimum").classification(),
            Classification::Breaking
        );
        assert_eq!(comparison.disposition().exit_code(), 1);
    }
}

#[test]
fn mandatory_presence_needs_no_payload_realization_and_moves_get_no_identity_mapping() {
    let before = json!({"sdfThing":{"Device":{"sdfObject":{"First":{"sdfProperty":{"temp":{}}},"Second":{}}}}});
    let mut after = json!({"sdfThing":{"Device":{"sdfObject":{"First":{},"Second":{"sdfProperty":{"temp":{"type":"number","minimum":10,"maximum":0}}}}}}});
    for mandatory in [false, true] {
        if mandatory {
            after["sdfThing"]["Device"]["sdfObject"]["Second"]["sdfRequired"] = json!(["temp"]);
        }
        let earlier = catalogue(&[("model", &before.to_string())]);
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), 2);
        assert_eq!(
            at(&comparison, "/First/sdfProperty/temp").classification(),
            Classification::Compatible
        );
        let added = at(&comparison, "/Second/sdfProperty/temp");
        assert_eq!(
            added.classification(),
            if mandatory {
                Classification::Breaking
            } else {
                Classification::Compatible
            }
        );
        assert!(added.witness().is_none());
    }
}

#[test]
fn unavailable_identity_never_establishes_an_optional_addition() {
    let earlier = catalogue(&[("model", r#"{"sdfThing":{"Device":{}}}"#)]);
    let later = catalogue(&[(
        "model",
        r#"{"sdfThing":{"Device":{"sdfProperty":{"\ud800":{}}}}}"#,
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.outcome(), Classification::Undetermined);
    assert!(
        comparison
            .differences()
            .all(|difference| difference.classification() == Classification::Undetermined)
    );
}

#[test]
fn incoming_obligations_do_not_treat_unavailable_earlier_groupings_as_absent() {
    let old = r##"{"sdfThing":{"Kitchen":{"sdfRef":"#/missing","sdfObject":{"Controller":{}}}},"sdfObject":{"Oven":{}}}"##;
    for known_obligation in [false, true] {
        // Put Kitchen first so its unavailable correspondence is encountered before Oven.
        let new = format!(
            r##"{{"sdfThing":{{"Kitchen":{{"sdfObject":{{"Controller":{{"sdfRequired":["#/sdfObject/Oven/sdfProperty/temp"]}}}}}}}},"sdfObject":{{"Oven":{{"sdfProperty":{{"temp":{{}}}}{}}}}}}}"##,
            if known_obligation {
                r#", "sdfRequired":["temp"]"#
            } else {
                ""
            }
        );
        let earlier = catalogue(&[("model", old)]);
        let later = catalogue(&[("model", &new)]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        let presence = at(&comparison, "/temp");
        if known_obligation {
            assert_eq!(presence.classification(), Classification::Breaking);
        } else {
            assert_eq!(presence.classification(), Classification::Undetermined);
            assert_eq!(presence.reason().unwrap().code(), "reference.unavailable");
        }
        assert_eq!(
            at(&comparison, "/Kitchen").classification(),
            Classification::Undetermined
        );
    }
}
