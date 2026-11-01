//! Public relationships retain declaration evidence independently of effective content.
//! Follows ADR0026: Required declaration relationships and source evidence.
use super::location_support;
use esdieff::{
    Catalogue, Class, Field, InspectRequest, Limits, Lookup, RequiredReferenceStatus,
    RequiredTargetCapability, SourceInput, View,
};

#[test]
fn required_conditions_supply_structural_basis_identifiers_and_citations() {
    let text = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[false,"missing"]}},"sdfData":{"D":{"sdfRequired":[true]}}}"#;
    for view in [View::Source, View::Effective] {
        inspect(text, "", view, |report| {
            let diagnostics = report.diagnostics();
            let mut kinds = std::collections::BTreeSet::new();
            for condition in diagnostics
                .conditions()
                .filter(|c| c.code().starts_with("required."))
            {
                kinds.insert(match condition.basis().kind() {
                    esdieff::BasisKind::Advice => "advice",
                    esdieff::BasisKind::Interpretation => "interpretation",
                    esdieff::BasisKind::Rfc => "rfc",
                    _ => panic!("unexpected required basis"),
                });
                match condition.basis().kind() {
                    esdieff::BasisKind::Rfc | esdieff::BasisKind::Json => {
                        assert!(condition.basis().citation().is_some());
                    }
                    _ => assert_eq!(condition.basis().id(), Some(condition.code())),
                }
            }
            assert_eq!(
                kinds,
                ["advice", "interpretation", "rfc"].into_iter().collect()
            );
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            for condition in wire["diagnostics"]["items"].as_array().unwrap() {
                if condition["code"].as_str().unwrap().starts_with("required.") {
                    let basis = &condition["basis"];
                    if basis["kind"] == "rfc" || basis["kind"] == "json" {
                        assert!(basis["citation"].is_string());
                    } else {
                        assert_eq!(basis["id"], condition["code"]);
                    }
                }
            }
        });
    }
}

#[test]
fn known_missing_source_selectors_are_request_errors_while_limited_context_stays_unfinished() {
    let text = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[true]}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    for required in [false, true] {
        for pointer in [
            "/missing",
            "/sdfObject/O/missing",
            "/sdfObject/O/sdfRequired/1",
            "/sdfObject/O/sdfRequired/01",
            "/sdfObject/O/sdfRequired/-",
            "/sdfObject/O/sdfRequired/999999999999999999999999999999",
        ] {
            assert_eq!(
                catalogue
                    .inspect(InspectRequest::new(id, pointer).required_declarations(required))
                    .err(),
                Some("unknown inspection selector"),
                "required {required} at {pointer}"
            );
        }
    }
    let unavailable =
        Catalogue::from_inputs(&[SourceInput::supplied("model", b"not JSON")]).unwrap();
    let report = unavailable
        .inspect(
            InspectRequest::new(unavailable.source_id("model").unwrap(), "/missing")
                .required_declarations(true),
        )
        .unwrap();
    assert!(!report.outcome().complete());
    let limited = Catalogue::with_limits(
        &[SourceInput::supplied("model", text)],
        Limits::default().effective_allocation_bytes(1),
    )
    .unwrap();
    let report = limited
        .inspect(
            InspectRequest::new(limited.source_id("model").unwrap(), "/missing")
                .required_declarations(true),
        )
        .unwrap();
    assert!(!report.outcome().complete());
    let limited = Catalogue::with_limits(
        &[SourceInput::supplied("model", text)],
        Limits::default().decoding_work(4096),
    )
    .unwrap();
    let id = limited.source_id("model").unwrap();
    assert!(matches!(
        limited.source(id).unwrap().json(),
        Field::Present(_)
    ));
    let expensive_selector = format!("/{}", "missing".repeat(256));
    let report = limited
        .inspect(InspectRequest::new(id, &expensive_selector).required_declarations(true))
        .unwrap();
    assert!(!report.outcome().complete());
    let undecodable = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br#"{"\ud800":{},"sdfObject":{"O":{}}}"#,
    )])
    .unwrap();
    let report = undecodable
        .inspect(
            InspectRequest::new(undecodable.source_id("model").unwrap(), "/missing")
                .required_declarations(true),
        )
        .unwrap();
    assert!(!report.outcome().complete());
    assert!(
        catalogue
            .inspect(InspectRequest::new(id, "/sdfObject/O").required_declarations(true))
            .unwrap()
            .outcome()
            .complete()
    );
}

#[test]
fn malformed_required_override_points_to_offending_bytes_without_losing_declaration_evidence() {
    let text = br##"{"info":{},"sdfObject":{"Base":{"sdfRequired":["p"],"sdfProperty":{"p":{"sdfRequired":[true]}}},"Copy":{"sdfRef":"#/sdfObject/Base","sdfProperty":{"p":{"sdfRequired":false}}}}}"##;
    inspect(text, "/sdfObject/Copy", View::Effective, |report| {
        assert_eq!(report.outcome().errors(), 1);
        let owners = report.required_declarations().require().unwrap();
        let property = owners
            .owners()
            .find(|owner| owner.class().present() == Some(Class::Property))
            .unwrap();
        assert_eq!(
            property
                .source_owner()
                .require()
                .unwrap()
                .pointer()
                .require()
                .unwrap(),
            "/sdfObject/Base/sdfProperty/p"
        );
        let object = owners
            .owners()
            .find(|owner| owner.class().present() == Some(Class::Object))
            .unwrap();
        let reference = object.references().require().unwrap().next().unwrap();
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("required p")
        };
        let target = targets.next().unwrap();
        assert_eq!(
            target
                .source_definition()
                .require()
                .unwrap()
                .json()
                .pointer()
                .require()
                .unwrap(),
            "/sdfObject/Base/sdfProperty/p"
        );
        assert_eq!(
            property.source_syntax().require().unwrap().raw_json(),
            "[true]"
        );
        let syntax = property.effective_syntax().require().unwrap();
        assert_eq!(
            syntax.location().pointer().require().unwrap(),
            "/sdfObject/Copy/sdfProperty/p/sdfRequired"
        );
        assert_eq!(syntax.source_json().raw_json(), "false");
        assert_eq!(
            syntax
                .explanation()
                .consumer()
                .unwrap()
                .pointer()
                .require()
                .unwrap(),
            "/sdfObject/Copy/sdfRef"
        );
        let diagnostics = report.diagnostics();
        let condition = diagnostics
            .conditions()
            .find(|c| c.code() == "required.syntax")
            .unwrap();
        let primary = location_support::primary_source(condition);
        assert_eq!(primary.source_id(), syntax.source_json().source_id());
        let span = primary.span().unwrap();
        assert_eq!(&text[span.start as usize..span.end as usize], b"false");
        assert_eq!(
            span.start as usize..span.end as usize,
            syntax.source_json().span()
        );
    });
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", text)],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("model").unwrap(), "/sdfObject/Copy")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.diagnostics().conditions().count(), 0);
}

#[test]
fn sensor_relationship_retains_declaration_and_refined_content_together() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "device",
        include_bytes!("../fixtures/temperature-device.sdf.json"),
    )])
    .unwrap();
    let id = catalogue.sources().next().unwrap().id();
    let report = catalogue
        .inspect(
            InspectRequest::new(id, "/sdfThing/Device/sdfObject/ambient")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    let relationships = report.required_declarations().require().unwrap();
    assert!(relationships.complete());
    let owner = relationships
        .owners()
        .find(|owner| owner.class().present() == Some(Class::Object))
        .unwrap();
    let reference = owner.references().require().unwrap().next().unwrap();
    assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
    let esdieff::Lookup::Found(mut targets) = reference.targets() else {
        panic!("required temperature missing")
    };
    let target = targets.next().unwrap();
    assert!(target.complete());
    assert_eq!(target.given_name(), "temperature");
    assert_eq!(
        target
            .source_definition()
            .require()
            .unwrap()
            .json()
            .pointer()
            .require()
            .unwrap(),
        "/sdfObject/Sensor/sdfProperty/temperature"
    );
    let effective = target.effective_definition().require().unwrap();
    assert_eq!(target.capability(), RequiredTargetCapability::Established);
    assert_eq!(
        effective.json().location().pointer().require().unwrap(),
        "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature"
    );
    assert_eq!(
        effective
            .data_description()
            .require()
            .unwrap()
            .maximum()
            .require()
            .unwrap()
            .lexeme(),
        "100"
    );
    assert_eq!(
        effective
            .json()
            .get("minimum")
            .require()
            .unwrap()
            .source_json()
            .pointer()
            .require()
            .unwrap(),
        "/sdfData/Temperature/minimum"
    );
    assert_eq!(
        reference
            .effective_json()
            .require()
            .unwrap()
            .explanation()
            .consumer()
            .unwrap()
            .pointer()
            .require()
            .unwrap(),
        "/sdfThing/Device/sdfObject/ambient/sdfRef"
    );
}

#[test]
fn inherited_declaration_survives_alternative_copy_and_nullable_deletion() {
    let text = br##"{"info":{},"sdfData":{"Modes":{"type":"string","enum":["a","b"],"maxLength":7,"nullable":false}},"sdfObject":{"Sensor":{"sdfRequired":["mode"],"sdfProperty":{"mode":{"sdfRef":"#/sdfData/Modes"}}}},"sdfThing":{"Device":{"sdfObject":{"Use":{"sdfRef":"#/sdfObject/Sensor","sdfProperty":{"mode":{"maxLength":100,"nullable":null}}}}}}}"##;
    inspect(
        text,
        "/sdfThing/Device/sdfObject/Use",
        View::Effective,
        |report| {
            assert!(report.outcome().complete());
            let owner = report
                .required_declarations()
                .require()
                .unwrap()
                .owners()
                .next()
                .unwrap();
            let reference = owner.references().require().unwrap().next().unwrap();
            let Lookup::Found(mut targets) = reference.targets() else {
                panic!("mode target")
            };
            let target = targets.next().unwrap();
            assert_eq!(
                target
                    .source_definition()
                    .require()
                    .unwrap()
                    .json()
                    .pointer()
                    .require()
                    .unwrap(),
                "/sdfObject/Sensor/sdfProperty/mode"
            );
            let effective = target.effective_definition().require().unwrap();
            assert_eq!(
                effective.json().location().pointer().require().unwrap(),
                "/sdfThing/Device/sdfObject/Use/sdfProperty/mode"
            );
            let data = effective.data_description().require().unwrap();
            let mut branches = 0;
            for (id, branch) in data.alternatives().require().unwrap() {
                let esdieff::AlternativeId::Enum(index) = id else {
                    panic!("enum branch")
                };
                let branch = branch.require().unwrap();
                assert_eq!(branch.maximum_length().require().unwrap().lexeme(), "100");
                assert!(branch.nullable().require().unwrap());
                let constant = branch
                    .json()
                    .quality_explanation("const")
                    .require()
                    .unwrap();
                assert_eq!(
                    constant
                        .sources()
                        .next()
                        .unwrap()
                        .pointer()
                        .require()
                        .unwrap(),
                    format!("/sdfData/Modes/enum/{index}")
                );
                let nullable = branch
                    .json()
                    .default_explanation("nullable")
                    .require()
                    .unwrap();
                assert_eq!(nullable.kind(), esdieff::OriginKind::Default);
                assert_eq!(nullable.sources().count(), 0);
                branches += 1;
            }
            assert_eq!(branches, 2);
            assert_eq!(
                reference
                    .effective_json()
                    .require()
                    .unwrap()
                    .explanation()
                    .consumer()
                    .unwrap()
                    .pointer()
                    .require()
                    .unwrap(),
                "/sdfThing/Device/sdfObject/Use/sdfRef"
            );
        },
    );
}

fn inspect(text: &[u8], pointer: &str, view: View, check: impl FnOnce(&esdieff::Report<'_>)) {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("model").unwrap(), pointer)
                .view(view)
                .required_declarations(true),
        )
        .unwrap();
    check(&report);
}

#[test]
fn direct_names_select_every_direct_class_and_empty_name_without_data_or_descendants() {
    let text = br#"{"info":{},"sdfThing":{"T":{"sdfRequired":["same",""],"sdfThing":{"same":{},"":{}},"sdfObject":{"same":{"sdfProperty":{"same":{}}},"":{}},"sdfProperty":{"same":{},"":{}},"sdfAction":{"same":{},"":{}},"sdfEvent":{"same":{},"":{}},"sdfData":{"same":{},"":{}}}}}"#;
    for view in [View::Source, View::Effective] {
        inspect(text, "/sdfThing/T", view, |report| {
            assert!(report.outcome().complete());
            let owner = report
                .required_declarations()
                .require()
                .unwrap()
                .owners()
                .next()
                .unwrap();
            for reference in owner.references().require().unwrap() {
                assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
                let Lookup::Found(targets) = reference.targets() else {
                    panic!("direct targets")
                };
                let classes: Vec<_> = targets.map(|target| target.class()).collect();
                assert_eq!(
                    classes,
                    [
                        Class::Thing,
                        Class::Object,
                        Class::Property,
                        Class::Action,
                        Class::Event
                    ]
                );
            }
        });
    }
}

#[test]
fn authored_pointers_decode_fragment_and_tokens_while_short_names_keep_literal_characters() {
    let text = br##"{"info":{},"sdfObject":{"O":{"sdfRequired":["#/sdfObject/O/sdfProperty/a%7E1b%7E0%23","a/b~","#/sdfObject/O/sdfProperty/"],"sdfProperty":{"a/b~#":{},"a/b~":{},"":{}}}}}"##;
    for view in [View::Source, View::Effective] {
        inspect(text, "/sdfObject/O", view, |report| {
            assert!(report.outcome().complete());
            let owner = report
                .required_declarations()
                .require()
                .unwrap()
                .owners()
                .next()
                .unwrap();
            for (reference, name) in owner
                .references()
                .require()
                .unwrap()
                .zip(["a/b~#", "a/b~", ""])
            {
                assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
                assert!(reference.complete());
                let Lookup::Found(mut targets) = reference.targets() else {
                    panic!("escaped target")
                };
                let target = targets.next().unwrap();
                assert_eq!(target.given_name(), name);
                assert_eq!(
                    target
                        .source_definition()
                        .require()
                        .unwrap()
                        .json()
                        .member_name()
                        .require()
                        .unwrap(),
                    name
                );
                assert_eq!(
                    target.capability(),
                    if view == View::Source {
                        RequiredTargetCapability::NotRequested
                    } else {
                        RequiredTargetCapability::Established
                    }
                );
                assert!(targets.next().is_none());
            }
        });
    }
}

#[test]
fn standalone_carrier_markers_are_known_while_pointer_to_reusable_top_is_wrong_role() {
    for keyword in [
        "sdfThing",
        "sdfObject",
        "sdfProperty",
        "sdfAction",
        "sdfEvent",
    ] {
        let text = format!(
            "{{\"info\":{{}},\"{keyword}\":{{\"standalone\":{{\"sdfRequired\":[true]}}}}}}"
        );
        for view in [View::Source, View::Effective] {
            inspect(
                text.as_bytes(),
                &format!("/{keyword}/standalone"),
                view,
                |report| {
                    assert!(report.outcome().complete());
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
                    assert!(reference.complete());
                    assert!(matches!(reference.targets(), Lookup::Found(_)));
                },
            );
        }
    }
    inspect(br##"{"info":{},"sdfObject":{"Standalone":{},"O":{"sdfRequired":["#/sdfObject/Standalone"]}}}"##, "/sdfObject/O", View::Source, |report| {
        assert!(report.outcome().complete()); assert_eq!(report.outcome().errors(), 1);
        assert_eq!(report.required_declarations().require().unwrap().owners().next().unwrap().references().require().unwrap().next().unwrap().status(), RequiredReferenceStatus::WrongTarget);
    });
}

#[test]
fn undecodable_reference_text_stays_unknown_and_does_not_hide_a_carrier() {
    let text = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":["\uD800",true]}}}"#;
    for view in [View::Source, View::Effective] {
        inspect(text, "/sdfObject/O", view, |report| {
            assert!(!report.outcome().complete());
            assert_eq!(report.outcome().errors(), 0);
            let owner = report
                .required_declarations()
                .require()
                .unwrap()
                .owners()
                .next()
                .unwrap();
            let references: Vec<_> = owner.references().require().unwrap().collect();
            assert_eq!(references[0].status(), RequiredReferenceStatus::Unavailable);
            assert!(!references[0].complete());
            assert_eq!(references[1].status(), RequiredReferenceStatus::Resolved);
            assert!(references[1].complete());
        });
    }
}

#[test]
fn known_direct_relationship_does_not_expose_unprocessed_cycle_as_effective_content() {
    let text = br##"{"info":{},"sdfObject":{"O":{"sdfRequired":["p"],"sdfProperty":{"p":{"sdfRef":"#/sdfObject/O/sdfProperty/p"}}}}}"##;
    inspect(text, "/sdfObject/O", View::Effective, |report| {
        assert!(!report.outcome().complete());
        let owner = report
            .required_declarations()
            .require()
            .unwrap()
            .owners()
            .next()
            .unwrap();
        let reference = owner.references().require().unwrap().next().unwrap();
        assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
        assert!(reference.complete());
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("known direct target")
        };
        let target = targets.next().unwrap();
        assert!(target.complete());
        assert!(matches!(target.source_definition(), Field::Present(_)));
        assert_eq!(target.capability(), RequiredTargetCapability::Unavailable);
        assert!(matches!(
            target.effective_definition(),
            Field::Unavailable(_)
        ));
    });
}

#[test]
fn malformed_selected_declaration_or_group_is_unavailable_instead_of_completed_empty() {
    let text = br#"{"info":{},"sdfObject":{"Broken":false},"sdfProperty":false}"#;
    for pointer in ["/sdfObject/Broken", "/sdfProperty"] {
        for view in [View::Source, View::Effective] {
            inspect(text, pointer, view, |report| {
                let collection = report.required_declarations().require().unwrap();
                assert!(!collection.complete());
                assert_ne!(
                    collection.state(),
                    esdieff::RequiredDeclarationsState::Available
                );
                assert!(collection.unavailable_reason().is_some());
                assert!(!report.outcome().complete());
            });
        }
    }
}

#[test]
fn outside_binding_does_not_compose_cycles_or_descendants_and_explicit_effective_is_separate() {
    let text = br##"{"info":{},"sdfData":{"Seven":{"type":"number","maximum":7}},"sdfObject":{"O":{"sdfRequired":["#/sdfObject/Elsewhere/sdfProperty/p","#/sdfObject/Elsewhere/sdfProperty/cyclic"]},"Elsewhere":{"sdfProperty":{"p":{"sdfRef":"#/sdfData/Seven","sdfChoice":false},"cyclic":{"sdfRef":"#/sdfObject/Elsewhere/sdfProperty/cyclic"}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(
                InspectRequest::new(id, "/sdfObject/O")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(report.outcome().complete());
        for reference in report
            .required_declarations()
            .require()
            .unwrap()
            .owners()
            .next()
            .unwrap()
            .references()
            .require()
            .unwrap()
        {
            assert!(reference.complete());
            let Lookup::Found(mut targets) = reference.targets() else {
                panic!("outside binding")
            };
            let target = targets.next().unwrap();
            assert_eq!(target.capability(), RequiredTargetCapability::NotRequested);
            assert!(matches!(target.effective_definition(), Field::Absent));
        }
        assert!(report.to_human().contains("not_requested"));
    }
    let model = catalogue
        .effective(esdieff::EffectiveRequest::new(id).pointer("/sdfObject/Elsewhere/sdfProperty/p"))
        .unwrap();
    assert_eq!(
        model
            .json()
            .require()
            .unwrap()
            .get("maximum")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "7"
    );
}

#[test]
fn relocated_copied_pointer_stays_independent_of_outside_pointer_and_short_name() {
    let statuses = |report: &esdieff::Report<'_>| -> Vec<_> {
        report
            .required_declarations()
            .require()
            .unwrap()
            .owners()
            .filter_map(|owner| owner.references().present())
            .flatten()
            .map(|r| r.status())
            .collect()
    };
    // The copied pointer relocates to Copy's own `p` under ADR 0036: Copied
    // reference relocation.
    let text = br##"{"info":{},"sdfObject":{"Base":{"sdfRequired":["#/sdfObject/Base/sdfProperty/p","#/sdfObject/External/sdfProperty/q","p"],"sdfProperty":{"p":{}}},"External":{"sdfProperty":{"q":{}}},"Copy":{"sdfRef":"#/sdfObject/Base"}}}"##;
    inspect(text, "/sdfObject/Copy", View::Effective, |report| {
        assert!(report.outcome().complete());
        assert_eq!(
            statuses(report),
            [
                RequiredReferenceStatus::Resolved,
                RequiredReferenceStatus::Resolved,
                RequiredReferenceStatus::Resolved
            ]
        );
    });
    // A relocated target below an unavailable import stays unknown without
    // hiding its known siblings.
    let text = br##"{"info":{},"sdfThing":{"Base":{"sdfRequired":["#/sdfThing/Base/sdfObject/inner/sdfProperty/p","#/sdfObject/External/sdfProperty/q","inner"],"sdfObject":{"inner":{"sdfRef":"#/sdfObject/Absent"}}},"Copy":{"sdfRef":"#/sdfThing/Base"}},"sdfObject":{"External":{"sdfProperty":{"q":{}}}}}"##;
    inspect(text, "/sdfThing/Copy", View::Effective, |report| {
        assert!(!report.outcome().complete());
        assert_eq!(
            statuses(report),
            [
                RequiredReferenceStatus::Unavailable,
                RequiredReferenceStatus::Resolved,
                RequiredReferenceStatus::Resolved
            ]
        );
    });
}

#[test]
fn source_matches_stay_partial_when_inheritance_can_add_classes_and_data_applicability_is_unknown()
{
    inspect(br##"{"info":{},"sdfThing":{"T":{"sdfRef":"#/sdfThing/Base","sdfRequired":["p"],"sdfProperty":{"p":{}}},"Base":{"sdfAction":{"p":{}}}}}"##, "/sdfThing/T", View::Source, |report| {
        assert!(!report.outcome().complete()); let row = report.required_declarations().require().unwrap().owners().next().unwrap().references().require().unwrap().next().unwrap();
        assert_eq!(row.status(), RequiredReferenceStatus::Partial); assert!(matches!(row.targets(), Lookup::Found(_)));
    });
    inspect(br#"{"info":{},"sdfData":{"D":{"sdfRequired":[true],"required":["member"],"nullable":true,"properties":{"member":{"type":"number"}}}}}"#, "/sdfData/D", View::Effective, |report| {
        assert!(!report.outcome().complete()); let owner = report.required_declarations().require().unwrap().owners().next().unwrap(); assert!(matches!(owner.class(), Field::Absent));
        assert_eq!(owner.references().require().unwrap().next().unwrap().status(), RequiredReferenceStatus::UnsupportedContext);
    });
}

#[test]
fn deleted_declaration_is_missing_and_reinsertion_gets_its_new_introducing_source() {
    let text = br##"{"info":{},"sdfObject":{"B":{"sdfProperty":{"p":{"type":"number"}}},"Deleted":{"sdfRef":"#/sdfObject/B","sdfRequired":["p"],"sdfProperty":{"p":null}},"Reinserted":{"sdfRef":"#/sdfObject/Deleted","sdfProperty":{"p":{"type":"string"}}}}}"##;
    inspect(text, "/sdfObject/Deleted", View::Effective, |report| {
        assert!(!report.outcome().complete());
        assert_eq!(report.outcome().notices(), 1);
        assert_eq!(
            report
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
                .unwrap()
                .status(),
            RequiredReferenceStatus::Missing
        );
    });
    inspect(text, "/sdfObject/Reinserted", View::Effective, |report| {
        assert!(report.outcome().complete());
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
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("reinserted target")
        };
        let target = targets.next().unwrap();
        assert_eq!(
            target
                .source_definition()
                .require()
                .unwrap()
                .json()
                .pointer()
                .require()
                .unwrap(),
            "/sdfObject/Reinserted/sdfProperty/p"
        );
    });
}

#[test]
fn invalid_and_missing_and_data_unknown_keep_independent_counts_under_all_detail_omission() {
    let text = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[false,"missing"],"sdfData":{"D":{"sdfRequired":[true]}},"sdfProperty":{"p":{"type":"object","required":["x"],"nullable":true}}}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", text)],
        Limits::default().diagnostic_bytes(1).conditions(1),
    )
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("model").unwrap(), "/sdfObject/O")
                .view(View::Effective)
                .required_declarations(true)
                .strict_warnings(true),
        )
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.outcome().notices(), 1);
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().incomplete_checks(), 1);
    assert_eq!(report.outcome().disposition(), esdieff::Disposition::Failed);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 1);
    assert_eq!(wire["result"]["required_declarations"]["state"], "partial");
    assert_eq!(
        wire["result"]["required_declarations"]["owners"],
        serde_json::json!([])
    );
    assert!(
        wire["result"]["required_declarations"]["owners_known"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(wire["diagnostics"]["truncated"].as_bool().unwrap());
    assert!(
        report
            .to_human()
            .contains("Required declarations: partial; complete: false")
    );
}

#[test]
fn exact_mandatory_cap_retains_summary_with_no_optional_transport_and_default_is_unchanged() {
    let pointer = "/sdfObject/O";
    let reserve = 5 * 6 + 4096 + 16384 + pointer.len() * 12 + 1024;
    let text = br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[true]}}}"#;
    let below = Catalogue::with_limits(
        &[SourceInput::supplied("model", text)],
        Limits::default().report_bytes(reserve - 1),
    )
    .unwrap();
    assert!(
        below
            .inspect(
                InspectRequest::new(below.source_id("model").unwrap(), pointer)
                    .required_declarations(true)
            )
            .is_err()
    );
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", text)],
        Limits::default().report_bytes(reserve),
    )
    .unwrap();
    let id = catalogue.source_id("model").unwrap();
    let plain = catalogue.inspect(InspectRequest::new(id, pointer)).unwrap();
    assert!(matches!(plain.required_declarations(), Field::Absent));
    let wire: serde_json::Value = serde_json::from_str(&plain.to_json()).unwrap();
    assert!(wire["result"].get("required_declarations").is_none());
    let report = catalogue
        .inspect(InspectRequest::new(id, pointer).required_declarations(true))
        .unwrap();
    assert!(report.outcome().complete());
    let text = report.to_json();
    assert!(text.len() <= reserve);
    let wire: serde_json::Value = serde_json::from_str(&text).unwrap();
    let summary = &wire["result"]["required_declarations"];
    assert_eq!(summary["state"], "available");
    assert_eq!(summary["complete"], true);
    assert!(summary.get("unavailable_reason").unwrap().is_null());
    assert_eq!(summary["owners_known"], 1);
    assert_eq!(summary["owners_retained"], 1);
    assert_eq!(summary["owners_omitted"], 1);
    assert_eq!(summary["owners"], serde_json::json!([]));
    assert_eq!(summary["omissions"].as_array().unwrap().len(), 1);
    assert_eq!(summary["omissions"][0]["reason"], "report_limit");
    assert_eq!(summary["omissions"][0]["omitted_count"], 1);
    assert_eq!(wire["diagnostics"]["truncated"], true);
    assert!(
        wire["diagnostics"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("report_limit"))
    );
    let human = report.to_human();
    assert!(human.len() <= reserve);
    assert!(human.contains("Required declarations: available; complete: true"));
}

#[test]
fn completed_empty_and_unavailable_and_partial_records_are_structurally_distinct() {
    inspect(
        br#"{"info":{},"sdfData":{"D":{"type":"object","required":["x"],"nullable":true}}}"#,
        "/sdfData/D",
        View::Source,
        |report| {
            let collection = report.required_declarations().require().unwrap();
            assert!(collection.complete());
            assert_eq!(
                collection.state(),
                esdieff::RequiredDeclarationsState::Available
            );
            assert_eq!(collection.owners().count(), 0);
        },
    );
    let catalogue = Catalogue::from_inputs(&[SourceInput::failed("model")]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(id, "").required_declarations(true))
        .unwrap();
    let collection = report.required_declarations().require().unwrap();
    assert!(!collection.complete());
    assert_eq!(
        collection.state(),
        esdieff::RequiredDeclarationsState::Unavailable
    );
    assert_eq!(collection.owners().count(), 0);
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied(
            "model",
            br#"{"info":{},"sdfObject":{"O":{"sdfRequired":[true,true]}}}"#,
        )],
        Limits::default().reference_applications(1),
    )
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("model").unwrap(), "/sdfObject/O")
                .required_declarations(true),
        )
        .unwrap();
    let collection = report.required_declarations().require().unwrap();
    assert_eq!(
        collection.state(),
        esdieff::RequiredDeclarationsState::Partial
    );
    assert_eq!(collection.owners().count(), 1);
    assert!(!collection.complete());
    let owner = collection.owners().next().unwrap();
    assert_eq!(owner.references().require().unwrap().count(), 1);
}

#[test]
fn supplied_global_identity_dependencies_ambiguity_missing_and_critical_context_are_distinct() {
    let consumer = br##"{"info":{},"namespace":{"lib":"https://example.com/library"},"sdfObject":{"O":{"sdfRequired":["lib:#/sdfObject/Library/sdfProperty/p"]}}}"##;
    let library = br#"{"info":{},"namespace":{"l":"https://example.com/library"},"defaultNamespace":"l","sdfObject":{"Library":{"sdfProperty":{"p":{}}}}}"#;
    for reverse in [false, true] {
        let mut inputs = [
            SourceInput::supplied("consumer", consumer),
            SourceInput::supplied("library", library),
        ];
        if reverse {
            inputs.reverse();
        }
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let root = catalogue.source_id("consumer").unwrap();
        let donor = catalogue.source_id("library").unwrap();
        let report = catalogue
            .inspect(InspectRequest::new(root, "/sdfObject/O").required_declarations(true))
            .unwrap();
        assert!(report.outcome().complete());
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
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("supplied declaration")
        };
        assert_eq!(
            targets
                .next()
                .unwrap()
                .source_definition()
                .require()
                .unwrap()
                .json()
                .source_id(),
            donor
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["request"]["scope"]["dependencies"],
            serde_json::json!([donor.get()])
        );
    }
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("consumer", consumer),
        SourceInput::supplied("one", library),
        SourceInput::supplied("two", library),
    ])
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("consumer").unwrap(), "/sdfObject/O")
                .required_declarations(true),
        )
        .unwrap();
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
    assert_eq!(reference.status(), RequiredReferenceStatus::Ambiguous);
    assert_eq!(reference.related_sources().count(), 2);
    assert!(matches!(reference.targets(), Lookup::Ambiguous));
    assert!(!report.outcome().complete());
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["request"]["scope"]["unavailable_required"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("consumer", consumer)]).unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("consumer").unwrap(), "/sdfObject/O")
                .required_declarations(true),
        )
        .unwrap();
    assert_eq!(report.outcome().notices(), 1);
    assert_eq!(
        report
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
            .unwrap()
            .status(),
        RequiredReferenceStatus::Missing
    );
    let unknown = br#"{"info":{"features":["unknown:critical"]},"namespace":{"l":"https://example.com/library"},"defaultNamespace":"l","sdfObject":{"Library":{"sdfProperty":{"p":{}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("consumer", consumer),
        SourceInput::supplied("unknown", unknown),
    ])
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("consumer").unwrap(), "/sdfObject/O")
                .required_declarations(true),
        )
        .unwrap();
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().notices(), 0);
    assert_eq!(
        report
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
            .unwrap()
            .status(),
        RequiredReferenceStatus::Unavailable
    );
}

#[test]
fn authored_pointer_coordinates_distinguish_original_donor_from_current_refined_declaration() {
    for (pointer, capability) in [
        (
            "#/sdfObject/Sensor/sdfProperty/p",
            RequiredTargetCapability::NotRequested,
        ),
        (
            "#/sdfThing/Device/sdfObject/ambient/sdfProperty/p",
            RequiredTargetCapability::Established,
        ),
    ] {
        let text = format!(
            r##"{{"info":{{}},"sdfData":{{"D":{{"type":"number","maximum":7}}}},"sdfObject":{{"Sensor":{{"sdfProperty":{{"p":{{"sdfRef":"#/sdfData/D"}}}}}}}},"sdfThing":{{"Device":{{"sdfObject":{{"ambient":{{"sdfRef":"#/sdfObject/Sensor","sdfRequired":["{pointer}"],"sdfProperty":{{"p":{{"maximum":100}}}}}}}}}}}}}}"##
        );
        inspect(
            text.as_bytes(),
            "/sdfThing/Device/sdfObject/ambient",
            View::Effective,
            |report| {
                assert!(report.outcome().complete());
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
                let Lookup::Found(mut targets) = reference.targets() else {
                    panic!("pointer target")
                };
                let target = targets.next().unwrap();
                assert_eq!(target.capability(), capability);
                if capability == RequiredTargetCapability::Established {
                    assert_eq!(
                        target
                            .source_definition()
                            .require()
                            .unwrap()
                            .json()
                            .pointer()
                            .require()
                            .unwrap(),
                        "/sdfObject/Sensor/sdfProperty/p"
                    );
                    assert_eq!(
                        target
                            .effective_definition()
                            .require()
                            .unwrap()
                            .data_description()
                            .require()
                            .unwrap()
                            .maximum()
                            .require()
                            .unwrap()
                            .lexeme(),
                        "100"
                    );
                }
            },
        );
    }
}

#[test]
fn pointer_to_deleted_current_declaration_is_missing() {
    inspect(br##"{"info":{},"sdfObject":{"B":{"sdfProperty":{"p":{}}},"O":{"sdfRef":"#/sdfObject/B","sdfRequired":["#/sdfObject/O/sdfProperty/p"],"sdfProperty":{"p":null}}}}"##, "/sdfObject/O", View::Effective, |report| {
        assert!(!report.outcome().complete()); assert_eq!(report.outcome().notices(), 1);
        let reference = report.required_declarations().require().unwrap().owners().next().unwrap().references().require().unwrap().next().unwrap(); assert_eq!(reference.status(), RequiredReferenceStatus::Missing); assert!(matches!(reference.targets(), Lookup::NotFound));
    });
}

#[test]
fn group_selection_retains_owners_and_raw_source_absence_is_separate_from_effective_syntax() {
    let text = br##"{"info":{},"sdfObject":{"Base":{"sdfRequired":[true]},"Copy":{"sdfRef":"#/sdfObject/Base"}}}"##;
    for view in [View::Source, View::Effective] {
        inspect(text, "/sdfObject", view, |report| {
            let collection = report.required_declarations().require().unwrap();
            assert!(collection.complete());
            assert_eq!(collection.owners().count(), 2);
            let copy = collection
                .owners()
                .find(|owner| {
                    owner
                        .source_owner()
                        .require()
                        .unwrap()
                        .member_name()
                        .present()
                        == Some("Copy")
                })
                .unwrap();
            assert!(matches!(copy.source_syntax(), Field::Absent));
            if view == View::Effective {
                assert!(matches!(copy.effective_syntax(), Field::Present(_)));
                assert!(matches!(copy.references(), Field::Present(_)));
            } else {
                assert!(matches!(copy.references(), Field::Absent));
                assert!(matches!(copy.effective_syntax(), Field::Absent));
            }
        });
    }
}

#[test]
fn malformed_copied_prefix_is_one_definite_error_with_independent_missing_and_known_name() {
    // The document maps `lib` but contributes no namespace, so `lib:` has no
    // target wherever it is read. The copy at `C` reads it as `B` does.
    let mut codes = Vec::new();
    for pointer in ["/sdfObject/B", "/sdfObject/C"] {
        inspect(br##"{"info":{},"namespace":{"lib":"https://example.com/lib"},"sdfObject":{"B":{"sdfRequired":["bad prefix:#/sdfObject/B/sdfProperty/p","lib:#/sdfObject/B/sdfProperty/p","p"],"sdfProperty":{"p":{}}},"C":{"sdfRef":"#/sdfObject/B"}}}"##, pointer, View::Effective, |report| {
            assert_eq!(report.outcome().errors(), 1, "{pointer}"); assert_eq!(report.outcome().notices(), 1, "{pointer}"); assert!(!report.outcome().complete()); assert_eq!(report.outcome().incomplete_checks(), 1);
            let rows: Vec<_> = report.required_declarations().require().unwrap().owners().next().unwrap().references().require().unwrap().map(|r| r.status()).collect(); assert_eq!(rows, [RequiredReferenceStatus::InvalidSyntax, RequiredReferenceStatus::Missing, RequiredReferenceStatus::Resolved], "{pointer}");
            assert_eq!(report.diagnostics().conditions().filter(|condition| condition.severity() == esdieff::Severity::Error).count(), 1);
            let mut seen: Vec<_> = report.diagnostics().conditions().map(|condition| condition.code().to_string()).collect();
            seen.sort();
            codes.push(seen);
        });
    }
    assert_eq!(codes[0], codes[1]);
}
