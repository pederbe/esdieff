//! Supplied composition preserves exact source content and effective refinements.
mod coverage_support;
use esdieff::{Catalogue, Class, EffectiveRequest, Field, Lookup, OriginKind, SourceInput};
const CONTROLLER: &[u8] = br##"{"namespace":{"level":"https://example.com/level","alias":"https://example.com/level"},"sdfObject":{"Controller":{"sdfProperty":{"level":{"sdfRef":"level:#/sdfData/Level","maximum":80,"description":null}}}}}"##;
const LEVEL: &[u8] = br##"{"namespace":{"data":"https://example.com/level"},"defaultNamespace":"data","sdfData":{"Level":{"type":"number","minimum":0,"maximum":100,"description":"Reusable level"}}}"##;
#[test]
fn controller_level_composes_supplied_data_and_preserves_source_origins() {
    for reverse in [false, true] {
        let mut inputs = [
            SourceInput::supplied("controller", CONTROLLER),
            SourceInput::supplied("level", LEVEL),
        ];
        if reverse {
            inputs.reverse();
        }
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let contributor = catalogue.source_id("level").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
        assert!(model.complete(), "supplied primitive import is established");
        let Lookup::Found(controller) = model.definition(Class::Object, "Controller") else {
            panic!("Controller")
        };
        let (_, level) = controller
            .children(Class::Property)
            .require()
            .unwrap()
            .next()
            .unwrap();
        let level = level.require().unwrap();
        let data = level.data_description().require().unwrap();
        assert_eq!(data.number("minimum").require().unwrap().lexeme(), "0");
        assert_eq!(data.number("maximum").require().unwrap().lexeme(), "80");
        assert!(matches!(level.description(), Field::Absent));
        let path = "/sdfObject/Controller/sdfProperty/level";
        for (quality, kind) in [
            ("minimum", OriginKind::Inherited),
            ("maximum", OriginKind::Override),
            ("description", OriginKind::Deleted),
        ] {
            let explanation = model
                .explanation(&format!("{path}/{quality}"))
                .require()
                .unwrap();
            assert_eq!(explanation.kind(), kind);
            let sources: Vec<_> = explanation
                .sources()
                .map(|json| (json.source_id(), json.pointer(8192).require().unwrap()))
                .collect();
            assert!(sources.contains(&(contributor, format!("/sdfData/Level/{quality}"))));
            if quality != "minimum" {
                assert!(sources.contains(&(root, format!("{path}/{quality}"))));
            }
            assert_eq!(
                explanation.effective().pointer(8192).require().unwrap(),
                format!("{path}/{quality}")
            );
        }
        let Lookup::Found(source) = catalogue
            .source(contributor)
            .unwrap()
            .definition(Class::Data, "Level")
        else {
            panic!("Level")
        };
        assert_eq!(source.description().require().unwrap(), "Reusable level");
        assert_eq!(
            source
                .data_description()
                .require()
                .unwrap()
                .number("maximum")
                .require()
                .unwrap()
                .lexeme(),
            "100"
        );
    }
}

#[test]
fn mandatory_cross_document_chain_preserves_patches_with_equal_local_node_indices() {
    let controller = br##"{"namespace":{"next":"https://example.com/A","own":"https://example.com/C"},"defaultNamespace":"own","sdfData":{"Level":{"sdfRef":"next:#/sdfData/Level","maximum":80}}}"##;
    let middle = br##"{"namespace":{"next":"https://example.com/B","own":"https://example.com/A"},"defaultNamespace":"own","sdfData":{"Level":{"sdfRef":"next:#/sdfData/Level","minimum":4,"maximum":90}}}"##;
    let base = br##"{"namespace":{"next":"https://example.com/unused","own":"https://example.com/B"},"defaultNamespace":"own","sdfData":{"Level":{"type":"number","minimum":0,"maximum":100}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("controller", controller),
        SourceInput::supplied("middle", middle),
        SourceInput::supplied("base", base),
    ])
    .unwrap();
    let root = catalogue.source_id("controller").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
    assert!(model.complete());
    let Lookup::Found(level) = model.definition(Class::Data, "Level") else {
        panic!("Level")
    };
    assert_eq!(
        level
            .data_description()
            .require()
            .unwrap()
            .number("minimum")
            .require()
            .unwrap()
            .lexeme(),
        "4"
    );
    assert_eq!(
        level
            .data_description()
            .require()
            .unwrap()
            .number("maximum")
            .require()
            .unwrap()
            .lexeme(),
        "80"
    );
}

#[test]
fn copied_curies_in_nested_imports_and_data_only_requiredness_use_their_source_prefixes() {
    for body in [
        r##"{"type":"object","properties":{"field":{"sdfRef":"data:#/sdfData/Leaf"}}}"##,
        r##"{"type":"number","minimum":0,"sdfRequired":["data:#/sdfAction/A"]}"##,
    ] {
        let contributor = format!(
            r##"{{"namespace":{{"data":"https://example.com/level"}},"defaultNamespace":"data","sdfData":{{"Base":{body},"Leaf":{{"type":"number"}}}},"sdfAction":{{"A":{{}}}}}}"##
        );
        // Only the contributor maps `data`, so the importer's mappings would
        // leave the copied CURIEs unmapped.
        let root_text = br##"{"namespace":{"level":"https://example.com/level"},"sdfData":{"Use":{"sdfRef":"level:#/sdfData/Base"},"Independent":{"type":"integer"}}}"##;
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("root", root_text),
            SourceInput::supplied("contributor", contributor.as_bytes()),
        ])
        .unwrap();
        let root = catalogue.source_id("root").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
        assert!(model.complete(), "{body}");
        assert!(matches!(
            model.definition(Class::Data, "Independent"),
            Lookup::Found(_)
        ));
        let report = catalogue
            .inspect(
                esdieff::InspectRequest::new(root, "/sdfData/Use")
                    .view(esdieff::View::Effective)
                    .required_declarations(true),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            !wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == "interpretation.unresolved"),
            "{wire}"
        );
        let Lookup::Found(use_data) = model.definition(Class::Data, "Use") else {
            panic!("Use")
        };
        let data = use_data.data_description().require().unwrap();
        if body.contains("sdfRequired") {
            assert_eq!(data.number("minimum").require().unwrap().lexeme(), "0");
            assert_eq!(
                data.json()
                    .get("sdfRequired")
                    .require()
                    .unwrap()
                    .source_json()
                    .raw_json(),
                r##"["data:#/sdfAction/A"]"##
            );
            // Requiredness in a data-only description keeps its separate limitation.
            assert!(!report.outcome().complete());
            let statuses: Vec<_> = report
                .required_declarations()
                .require()
                .unwrap()
                .owners()
                .flat_map(|owner| owner.references().require().unwrap())
                .map(|reference| reference.status())
                .collect();
            assert_eq!(
                statuses,
                [esdieff::RequiredReferenceStatus::UnsupportedContext]
            );
        } else {
            assert!(report.outcome().complete());
            assert_eq!(
                wire["result"]["json"]["properties"]["field"],
                serde_json::json!({"type": "number"})
            );
        }
    }
}

#[test]
fn ambiguity_retains_all_addressed_contributors_without_identical_text_coalescing() {
    for second in [
        LEVEL.to_vec(),
        String::from_utf8(LEVEL.to_vec())
            .unwrap()
            .replace("100", "99")
            .into_bytes(),
    ] {
        for reverse in [false, true] {
            let mut inputs = [
                SourceInput::supplied("controller", CONTROLLER),
                SourceInput::supplied("first", LEVEL),
                SourceInput::supplied("second", &second),
            ];
            if reverse {
                inputs.reverse();
            }
            let catalogue = Catalogue::from_inputs(&inputs).unwrap();
            let root = catalogue.source_id("controller").unwrap();
            let report = catalogue
                .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
                .unwrap();
            assert!(!report.outcome().complete());
            assert_eq!(report.outcome().errors(), 0);
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            let condition = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["code"] == "reference.ambiguous")
                .unwrap();
            let related = condition["related"].as_array().unwrap();
            for key in ["first", "second"] {
                let id = catalogue.source_id(key).unwrap().get();
                assert!(
                    related.iter().any(|location| location["source_id"] == id
                        && location["pointer"] == "/sdfData/Level"),
                    "missing contributor {key}"
                );
            }
        }
    }
}

#[test]
fn mandatory_contributor_reference_errors_keep_original_locations_and_failed_outcomes() {
    for reference in ["absent:#/sdfData/Leaf", "bad space:#/sdfData/Leaf"] {
        let contributor = String::from_utf8(LEVEL.to_vec()).unwrap().replace(
            "\"type\":\"number\"",
            &format!("\"sdfRef\":\"{reference}\",\"type\":\"number\""),
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("controller", CONTROLLER),
            SourceInput::supplied("level", contributor.as_bytes()),
        ])
        .unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let dependency = catalogue.source_id("level").unwrap();
        for report in [
            catalogue
                .check(esdieff::CheckRequest::new().root(root))
                .unwrap(),
            catalogue
                .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
                .unwrap(),
        ] {
            assert_eq!(report.outcome().errors(), 1);
            assert_eq!(report.outcome().disposition(), esdieff::Disposition::Failed);
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            let code = if reference.starts_with("absent:") {
                "reference.prefix_unmapped"
            } else {
                "reference.syntax"
            };
            let condition = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["code"] == code)
                .unwrap();
            assert_eq!(condition["severity"], "error");
            assert_eq!(condition["basis"]["kind"], "rfc");
            assert_eq!(condition["primary"]["kind"], "source");
            assert_eq!(condition["primary"]["source_id"], dependency.get());
            assert_eq!(condition["primary"]["pointer"], "/sdfData/Level/sdfRef");
        }
    }
}

#[test]
fn omitted_reference_findings_and_dependency_temporal_knowledge_keep_semantic_results() {
    for limits in [
        esdieff::Limits::default(),
        esdieff::Limits::default()
            .conditions(1)
            .diagnostic_bytes(1)
            .pointer_output_bytes(1),
    ] {
        let malformed = String::from_utf8(LEVEL.to_vec()).unwrap().replace(
            "\"type\":\"number\"",
            "\"sdfRef\":\"absent:#/sdfData/Leaf\",\"type\":\"number\"",
        );
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("controller", CONTROLLER),
                SourceInput::supplied("level", malformed.as_bytes()),
            ],
            limits.clone(),
        )
        .unwrap();
        let root = catalogue.source_id("controller").unwrap();
        for report in [
            catalogue
                .check(esdieff::CheckRequest::new().root(root))
                .unwrap(),
            catalogue
                .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
                .unwrap(),
        ] {
            assert_eq!(report.outcome().errors(), 1);
            assert_eq!(report.outcome().disposition(), esdieff::Disposition::Failed);
            assert!(!report.outcome().complete());
        }
        let dated = String::from_utf8(LEVEL.to_vec()).unwrap().replace(
            "\"namespace\"",
            "\"info\":{\"modified\":\"2016-12-31T23:59:60Z\"},\"namespace\"",
        );
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("controller", CONTROLLER),
                SourceInput::supplied("level", dated.as_bytes()),
            ],
            limits,
        )
        .unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let report = catalogue
            .check(
                esdieff::CheckRequest::new()
                    .root(root)
                    .assessment(esdieff::Assessment::ExpandedModified),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(
            report
                .assessment_area_status("modified-date-time/1")
                .require()
                .unwrap(),
            esdieff::AssessmentStatus::Assessed
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(wire["baseline"]["temporal_knowledge"].is_object());
        let ordinary = catalogue
            .check(esdieff::CheckRequest::new().root(root))
            .unwrap();
        let wire: serde_json::Value = serde_json::from_str(&ordinary.to_json()).unwrap();
        assert!(wire["baseline"]["temporal_knowledge"].is_null());
    }
}

#[test]
fn importing_consumer_locations_are_preserved_without_replacing_content_origins() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("controller", CONTROLLER),
        SourceInput::supplied("level", LEVEL),
    ])
    .unwrap();
    let root = catalogue.source_id("controller").unwrap();
    let report = catalogue
        .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    for quality in ["minimum", "maximum", "description"] {
        let path = format!("/sdfObject/Controller/sdfProperty/level/{quality}");
        let explanation = wire["result"]["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["effective"]["pointer"] == path && row["kind"] != "default")
            .unwrap();
        assert_eq!(explanation["consumer"]["source_id"], root.get());
        assert_eq!(
            explanation["consumer"]["pointer"],
            "/sdfObject/Controller/sdfProperty/level/sdfRef"
        );
    }
}

#[test]
fn selected_scope_reports_needed_sources_and_excludes_unused_contributor_content() {
    let contributor = String::from_utf8(LEVEL.to_vec())
        .unwrap()
        .replace("\"namespace\"", "\"info\":{\"title\":7},\"namespace\"")
        .replace("\"Level\":", "\"Unused\":{\"description\":7},\"Level\":");
    for reverse in [false, true] {
        let mut inputs = [
            SourceInput::supplied("controller", CONTROLLER),
            SourceInput::supplied("level", contributor.as_bytes()),
            SourceInput::supplied(
                "unrelated-attempt",
                br##"{"sdfData":{"Unused":{"description":7}}}"##,
            ),
        ];
        if reverse {
            inputs.reverse();
        }
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let dependency = catalogue.source_id("level").unwrap();
        let selected = catalogue
            .check(esdieff::CheckRequest::new().root(root))
            .unwrap();
        assert_eq!(selected.outcome().errors(), 0);
        let wire: serde_json::Value = serde_json::from_str(&selected.to_json()).unwrap();
        assert_eq!(
            wire["request"]["scope"]["dependencies"],
            serde_json::json!([dependency.get()])
        );
        assert_eq!(wire["sources"].as_array().unwrap().len(), 3);
        assert_eq!(
            wire["request"]["scope"]["assessed"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let catalogue_report = catalogue.check(esdieff::CheckRequest::new()).unwrap();
        assert!(catalogue_report.outcome().errors() >= 3);
    }
}

#[test]
fn expanded_modified_assesses_each_needed_source_once_without_unrelated_metadata() {
    for modified in ["7", r#""2023-02-29""#] {
        let contributor = String::from_utf8(LEVEL.to_vec()).unwrap().replace(
            "\"namespace\"",
            &format!("\"info\":{{\"title\":7,\"modified\":{modified}}},\"namespace\""),
        );
        let controller = String::from_utf8(CONTROLLER.to_vec()).unwrap().replace(
            "\"sdfProperty\":{\"level\":",
            "\"sdfProperty\":{\"other\":{\"sdfRef\":\"alias:#/sdfData/Level\"},\"level\":",
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("controller", controller.as_bytes()),
            SourceInput::supplied("level", contributor.as_bytes()),
        ])
        .unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let ordinary = catalogue
            .check(esdieff::CheckRequest::new().root(root))
            .unwrap();
        assert_eq!(ordinary.outcome().errors(), 0);
        let report = catalogue
            .check(
                esdieff::CheckRequest::new()
                    .root(root)
                    .assessment(esdieff::Assessment::ExpandedModified),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(
            report
                .assessment_area_status("modified-date-time/1")
                .require()
                .unwrap(),
            esdieff::AssessmentStatus::Assessed
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let errors: Vec<_> = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["primary"]["pointer"], "/info/modified");
    }
}

#[test]
fn original_required_curie_checks_binding_without_resolving_declaration_identity() {
    let text=br##"{"sdfObject":{"Device":{"sdfRequired":["missing:#/sdfProperty/level"],"sdfProperty":{"level":{"type":"number"}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("root", text)]).unwrap();
    let root = catalogue.source_id("root").unwrap();
    let report = catalogue
        .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let condition = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "reference.prefix_unmapped")
        .unwrap();
    assert_eq!(
        condition["primary"]["pointer"],
        "/sdfObject/Device/sdfRequired/0"
    );
    let model = report.effective_model().unwrap();
    let Lookup::Found(device) = model.definition(Class::Object, "Device") else {
        panic!("Device")
    };
    assert!(matches!(
        device.children(Class::Property),
        Field::Present(_)
    ));
}

#[test]
fn curie_literal_assembly_and_decoded_fragments_select_the_same_supplied_definition() {
    for (binding, reference) in [
        ("https://example.com/level", "p:#/sdfData/Level"),
        ("https://example.com/", "p:level#/sdfData/Level"),
        ("https://example.com/level#/sdfData/", "p:Level"),
        ("https://example.com/level", "p:#%2FsdfData%2FLevel"),
    ] {
        let root_text = format!(
            r##"{{"namespace":{{"p":"{binding}"}},"sdfData":{{"Use":{{"sdfRef":"{reference}"}}}}}}"##
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("root", root_text.as_bytes()),
            SourceInput::supplied("level", LEVEL),
        ])
        .unwrap();
        let root = catalogue.source_id("root").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
        assert!(model.complete(), "{binding} + {reference}");
        let Lookup::Found(data) = model.definition(Class::Data, "Use") else {
            panic!("Use")
        };
        assert_eq!(
            data.data_description()
                .require()
                .unwrap()
                .number("maximum")
                .require()
                .unwrap()
                .lexeme(),
            "100"
        );
    }
}

#[test]
fn known_absent_contributions_and_unavailable_candidates_remain_distinct() {
    let local_only = String::from_utf8(LEVEL.to_vec())
        .unwrap()
        .replace("\"defaultNamespace\":\"data\",", "");
    for (second, expected) in [
        (Some(local_only.as_bytes()), "available"),
        (None, "unavailable"),
    ] {
        let attempt = second.map_or_else(
            || SourceInput::failed("candidate"),
            |text| SourceInput::supplied("candidate", text),
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("controller", CONTROLLER),
            SourceInput::supplied("level", LEVEL),
            attempt,
        ])
        .unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let report = catalogue
            .inspect(
                esdieff::InspectRequest::new(root, "/sdfObject/Controller/sdfProperty/level")
                    .view(esdieff::View::Effective),
            )
            .unwrap();
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["result"]["state"], expected);
        assert_eq!(wire["sources"].as_array().unwrap().len(), 3);
        assert_eq!(report.outcome().errors(), 0);
    }
}

#[test]
fn unknown_critical_context_cannot_establish_namespace_contribution_absence() {
    for namespace in [
        "",
        r#""namespace":{"own":"https://example.com/private"},"defaultNamespace":"own","#,
    ] {
        let private = format!(
            r##"{{{namespace}"info":{{"features":["https://example.com/unknown-feature"],"title":7,"modified":7}},"sdfData":{{"Private":{{"description":7}}}}}}"##
        );
        for reverse in [false, true] {
            for limits in [
                esdieff::Limits::default(),
                esdieff::Limits::default()
                    .conditions(1)
                    .diagnostic_bytes(1)
                    .pointer_output_bytes(1),
            ] {
                let mut inputs = [
                    SourceInput::supplied("controller", CONTROLLER),
                    SourceInput::supplied("level", LEVEL),
                    SourceInput::supplied("private", private.as_bytes()),
                ];
                if reverse {
                    inputs.reverse();
                }
                let catalogue = Catalogue::with_limits(&inputs, limits).unwrap();
                let root = catalogue.source_id("controller").unwrap();
                let private_id = catalogue.source_id("private").unwrap();
                let inspect = catalogue
                    .inspect(
                        esdieff::InspectRequest::new(
                            root,
                            "/sdfObject/Controller/sdfProperty/level",
                        )
                        .view(esdieff::View::Effective),
                    )
                    .unwrap();
                assert!(!inspect.outcome().complete());
                assert_eq!(inspect.outcome().errors(), 0);
                let wire: serde_json::Value = serde_json::from_str(&inspect.to_json()).unwrap();
                assert_eq!(wire["sources"].as_array().unwrap().len(), 3);
                assert_eq!(
                    wire["request"]["scope"]["dependencies"],
                    serde_json::json!([])
                );
                assert_eq!(
                    wire["request"]["scope"]["unavailable_required"],
                    serde_json::json!([private_id.get()])
                );
                let selected = catalogue
                    .check(
                        esdieff::CheckRequest::new()
                            .root(root)
                            .assessment(esdieff::Assessment::ExpandedModified),
                    )
                    .unwrap();
                assert!(!selected.outcome().complete());
                assert_eq!(selected.outcome().errors(), 0);
                assert_eq!(
                    selected
                        .assessment_area_status("modified-date-time/1")
                        .require()
                        .unwrap(),
                    esdieff::AssessmentStatus::Undetermined
                );
                let all = catalogue.check(esdieff::CheckRequest::new()).unwrap();
                let all_wire: serde_json::Value = serde_json::from_str(&all.to_json()).unwrap();
                assert_eq!(
                    all_wire["request"]["scope"]["assessed"]
                        .as_array()
                        .unwrap()
                        .len(),
                    3
                );
            }
        }
    }
}

#[test]
fn shared_namespace_contributors_select_addressed_definitions_and_keep_number_lexemes() {
    let controller = String::from_utf8(CONTROLLER.to_vec())
        .unwrap()
        .replace("\"maximum\":80", "\"maximum\":8.000e1");
    let level = String::from_utf8(LEVEL.to_vec())
        .unwrap()
        .replace("\"minimum\":0", "\"minimum\":-0.000e1000000")
        .replace("\"maximum\":100", "\"maximum\":1.000e2");
    let disjoint = br##"{"namespace":{"other":"https://example.com/level"},"defaultNamespace":"other","sdfData":{"Other":{"type":"number","minimum":3}}}"##;
    for reverse in [false, true] {
        let mut inputs = [
            SourceInput::supplied("controller", controller.as_bytes()),
            SourceInput::supplied("level", level.as_bytes()),
            SourceInput::supplied("other", disjoint),
        ];
        if reverse {
            inputs.reverse();
        }
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let report = catalogue
            .inspect(
                esdieff::InspectRequest::new(root, "/sdfObject/Controller/sdfProperty/level")
                    .view(esdieff::View::Effective),
            )
            .unwrap();
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
        let model = report.effective_model().unwrap();
        let json = model.json().require().unwrap();
        assert_eq!(
            json.get("minimum")
                .require()
                .unwrap()
                .number()
                .require()
                .unwrap()
                .lexeme(),
            "-0.000e1000000"
        );
        assert_eq!(
            json.get("maximum")
                .require()
                .unwrap()
                .number()
                .require()
                .unwrap()
                .lexeme(),
            "8.000e1"
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["request"]["scope"]["dependencies"],
            serde_json::json!([catalogue.source_id("level").unwrap().get()])
        );
        assert_eq!(wire["sources"].as_array().unwrap().len(), 3);
    }
}

#[test]
fn unreliable_candidate_envelopes_and_member_identity_cannot_prove_absence() {
    for candidate in [
        b"[]".as_slice(),
        br#"{"\uD800":1}"#.as_slice(),
        br#"{"sdfData":"#.as_slice(),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("controller", CONTROLLER),
            SourceInput::supplied("level", LEVEL),
            SourceInput::supplied("uncertain", candidate),
        ])
        .unwrap();
        let root = catalogue.source_id("controller").unwrap();
        let report = catalogue
            .inspect(
                esdieff::InspectRequest::new(root, "/sdfObject/Controller/sdfProperty/level")
                    .view(esdieff::View::Effective),
            )
            .unwrap();
        assert!(!report.outcome().complete());
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["request"]["scope"]["unavailable_required"],
            serde_json::json!([catalogue.source_id("uncertain").unwrap().get()])
        );
    }
}

#[test]
fn consumer_deleted_nested_references_do_not_create_dependencies_or_selected_findings() {
    let contributor=br##"{"namespace":{"d":"https://example.com/base"},"defaultNamespace":"d","info":{"title":7},"sdfData":{"Base":{"type":"object","properties":{"removed":{"sdfRef":"bad space:#/sdfData/Absent","description":7}}}}}"##;
    let root_text=br##"{"namespace":{"d":"https://example.com/base"},"sdfData":{"Use":{"sdfRef":"d:#/sdfData/Base","properties":{"removed":null}},"Independent":{"description":7}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root_text),
        SourceInput::supplied("base", contributor),
    ])
    .unwrap();
    let root = catalogue.source_id("root").unwrap();
    let inspection = catalogue
        .inspect(esdieff::InspectRequest::new(root, "/sdfData/Use").view(esdieff::View::Effective))
        .unwrap();
    assert!(inspection.outcome().complete());
    assert_eq!(inspection.outcome().errors(), 0);
    let selected = catalogue
        .check(esdieff::CheckRequest::new().root(root))
        .unwrap();
    assert_eq!(selected.outcome().errors(), 1);
    let wire: serde_json::Value = serde_json::from_str(&selected.to_json()).unwrap();
    assert!(
        !wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "reference.syntax")
    );
    let all = catalogue.check(esdieff::CheckRequest::new()).unwrap();
    assert!(all.outcome().errors() >= 4);
}

#[test]
fn source_closure_uncertainty_and_omitted_details_preserve_modified_and_reference_outcomes() {
    let missing = String::from_utf8(CONTROLLER.to_vec())
        .unwrap()
        .replace("Level", "Absent");
    for root_text in [CONTROLLER, missing.as_bytes()] {
        for limits in [
            esdieff::Limits::default(),
            esdieff::Limits::default()
                .conditions(1)
                .diagnostic_bytes(1)
                .pointer_output_bytes(1),
        ] {
            let catalogue = Catalogue::with_limits(
                &[
                    SourceInput::supplied("root", root_text),
                    SourceInput::failed("needed-candidate"),
                ],
                limits,
            )
            .unwrap();
            let root = catalogue.source_id("root").unwrap();
            let report = catalogue
                .check(
                    esdieff::CheckRequest::new()
                        .root(root)
                        .assessment(esdieff::Assessment::ExpandedModified),
                )
                .unwrap();
            assert_eq!(
                report
                    .assessment_area_status("modified-date-time/1")
                    .require()
                    .unwrap(),
                esdieff::AssessmentStatus::Undetermined
            );
            assert_eq!(report.outcome().errors(), 0);
            assert_eq!(report.requested_coverage().total(), Some(9));
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["sources"].as_array().unwrap().len(), 2);
            assert_eq!(
                wire["request"]["scope"]["unavailable_required"],
                serde_json::json!([catalogue.source_id("needed-candidate").unwrap().get()])
            );
        }
    }
}

#[test]
fn prefix_uncertainty_does_not_hide_independent_syntax_or_required_reference_errors() {
    let copied=br##"{"namespace":{"d":"https://example.com/base"},"defaultNamespace":"d","sdfData":{"Base":{"type":"object","properties":{"field":{"sdfRef":"bad space:#/sdfData/Leaf"}}}}}"##;
    let root_text=br##"{"namespace":{"d":"https://example.com/base"},"sdfData":{"Use":{"sdfRef":"d:#/sdfData/Base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root_text),
        SourceInput::supplied("base", copied),
    ])
    .unwrap();
    let root = catalogue.source_id("root").unwrap();
    assert_eq!(
        catalogue
            .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
            .unwrap()
            .outcome()
            .errors(),
        1
    );
    let required=br##"{"sdfObject":{"Device":{"sdfRequired":["absent:#/sdfProperty/A","bad space:#/sdfProperty/B"]}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("root", required)]).unwrap();
    let root = catalogue.source_id("root").unwrap();
    let report = catalogue
        .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
        .unwrap();
    assert_eq!(report.outcome().errors(), 2);
}

#[test]
fn mandatory_chains_keep_outer_consumers_while_nested_composition_starts_its_own_boundary() {
    let root_text=br##"{"namespace":{"d":"https://example.com/A"},"sdfData":{"Use":{"sdfRef":"d:#/sdfData/Base"}}}"##;
    let middle=br##"{"namespace":{"d":"https://example.com/B","own":"https://example.com/A"},"defaultNamespace":"own","sdfData":{"Base":{"sdfRef":"d:#/sdfData/Base"}}}"##;
    let base=br##"{"namespace":{"own":"https://example.com/B"},"defaultNamespace":"own","sdfData":{"Base":{"type":"object","description":"Outer","properties":{"field":{"sdfRef":"#/sdfData/Leaf"}}},"Leaf":{"type":"number","minimum":20}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root_text),
        SourceInput::supplied("middle", middle),
        SourceInput::supplied("base", base),
    ])
    .unwrap();
    let root = catalogue.source_id("root").unwrap();
    let base_id = catalogue.source_id("base").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
    assert!(model.complete());
    let outer = model
        .explanation("/sdfData/Use/description")
        .require()
        .unwrap();
    let consumer = outer.consumer().unwrap();
    assert_eq!(consumer.source_id(), root);
    assert_eq!(
        consumer.pointer(8192).require().unwrap(),
        "/sdfData/Use/sdfRef"
    );
    let nested = model
        .explanation("/sdfData/Use/properties/field/minimum")
        .require()
        .unwrap();
    let consumer = nested.consumer().unwrap();
    assert_eq!(consumer.source_id(), base_id);
    assert_eq!(
        consumer.pointer(8192).require().unwrap(),
        "/sdfData/Base/properties/field/sdfRef"
    );
    assert_eq!(
        nested
            .sources()
            .next()
            .unwrap()
            .pointer(8192)
            .require()
            .unwrap(),
        "/sdfData/Leaf/minimum"
    );
}

#[test]
fn assembly_capability_limits_and_exact_namespace_nonmatches_do_not_invent_rfc_errors() {
    for (binding, reference, code) in [
        (
            "https://example.com/level#/sdfData/",
            "p:#/sdfData/Level",
            "reference.capability_unavailable",
        ),
        (
            "https://example.com/level",
            "π:#/sdfData/Level",
            "reference.capability_unavailable",
        ),
        (
            "https://example.com/%6cevel",
            "p:#/sdfData/Level",
            "reference.unavailable",
        ),
    ] {
        let root_text = format!(
            r##"{{"namespace":{{"p":"{binding}","π":"{binding}"}},"sdfData":{{"Use":{{"sdfRef":"{reference}"}}}}}}"##
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("root", root_text.as_bytes()),
            SourceInput::supplied("level", LEVEL),
        ])
        .unwrap();
        let root = catalogue.source_id("root").unwrap();
        let report = catalogue
            .inspect(esdieff::InspectRequest::new(root, "").view(esdieff::View::Effective))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(!report.outcome().complete());
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == code)
        );
    }
}

#[test]
fn cumulative_reference_limits_leave_unfinished_dependency_metadata_scope_explicit() {
    let second = String::from_utf8(CONTROLLER.to_vec())
        .unwrap()
        .replace("https://example.com/level", "https://example.com/other");
    let other = String::from_utf8(LEVEL.to_vec())
        .unwrap()
        .replace("https://example.com/level", "https://example.com/other")
        .replace("\"namespace\"", "\"info\":{\"modified\":7},\"namespace\"");
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("first", CONTROLLER),
            SourceInput::supplied("second", second.as_bytes()),
            SourceInput::supplied("level", LEVEL),
            SourceInput::supplied("other", other.as_bytes()),
        ],
        esdieff::Limits::default().reference_applications(1),
    )
    .unwrap();
    let request = esdieff::CheckRequest::new()
        .root(catalogue.source_id("first").unwrap())
        .root(catalogue.source_id("second").unwrap())
        .assessment(esdieff::Assessment::ExpandedModified);
    let report = catalogue.check(request).unwrap();
    assert_eq!(
        report
            .assessment_area_status("modified-date-time/1")
            .require()
            .unwrap(),
        esdieff::AssessmentStatus::Undetermined
    );
    assert_eq!(report.outcome().errors(), 0);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "operation.limit")
    );
}

#[test]
fn imported_compound_constraints_keep_object_premises_restricted_items_and_date_scope() {
    use esdieff::{Assessment, AssessmentStatus, BaseType, DataRole, DataType, Limits};
    let root_text=br##"{"info":{},"namespace":{"d":"https://example.com/base"},"sdfProperty":{"Use":{"sdfRef":"d:#/sdfProperty/Base","type":"object"}}}"##;
    for (date, status) in [
        ("2016-12-31T23:59:60Z", AssessmentStatus::Assessed),
        ("2030-12-31T23:59:60Z", AssessmentStatus::Undetermined),
    ] {
        let base = format!(
            r##"{{"info":{{"title":7,"modified":"{date}"}},"namespace":{{"own":"https://example.com/base"}},"defaultNamespace":"own","sdfProperty":{{"Base":{{"type":"string","required":[],"multipleOf":0,"items":{{"type":"string","minLength":-1,"minItems":-1,"nullable":[],"multipleOf":0}}}},"Unused":{{"description":7}}}}}}"##
        );
        for reverse in [false, true] {
            for limits in [
                Limits::default(),
                Limits::default()
                    .conditions(1)
                    .diagnostic_bytes(1)
                    .pointer_output_bytes(1),
            ] {
                let mut inputs = [
                    SourceInput::supplied("root", root_text),
                    SourceInput::supplied("base", base.as_bytes()),
                ];
                if reverse {
                    inputs.reverse();
                }
                let catalogue = Catalogue::with_limits(&inputs, limits).unwrap();
                let root = catalogue.source_id("root").unwrap();
                let dependency = catalogue.source_id("base").unwrap();
                let ordinary = catalogue
                    .check(esdieff::CheckRequest::new().root(root))
                    .unwrap();
                assert_eq!(ordinary.outcome().errors(), 3);
                let report = catalogue
                    .check(
                        esdieff::CheckRequest::new()
                            .root(root)
                            .assessment(Assessment::ExpandedModified),
                    )
                    .unwrap();
                assert_eq!(report.outcome().errors(), 3);
                assert_eq!(
                    report
                        .assessment_area_status("modified-date-time/1")
                        .require()
                        .unwrap(),
                    status
                );
                assert_eq!(report.requested_coverage().total(), Some(9));
                let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
                assert!(wire["baseline"]["temporal_knowledge"].is_object());
                assert_eq!(
                    wire["request"]["scope"]["dependencies"],
                    serde_json::json!([dependency.get()])
                );
                let model = report.effective_model().unwrap();
                let Lookup::Found(property) = model.definition(Class::Property, "Use") else {
                    panic!("Use")
                };
                let data = property.data_description().require().unwrap();
                assert_eq!(
                    data.data_type().require().unwrap(),
                    DataType::Known(BaseType::Map)
                );
                assert!(matches!(data.required_entries(), Field::Unavailable(_)));
                let item = data.items().require().unwrap();
                assert_eq!(item.role(), DataRole::DirectItem);
                assert_eq!(
                    item.data_type().require().unwrap(),
                    DataType::Known(BaseType::String)
                );
                assert!(matches!(item.minimum_items(), Field::Unavailable(_)));
                assert!(matches!(item.multiple_of(), Field::Unavailable(_)));
                assert!(matches!(item.nullable(), Field::Unavailable(_)));
                assert_eq!(
                    item.json()
                        .get("minLength")
                        .require()
                        .unwrap()
                        .number()
                        .require()
                        .unwrap()
                        .lexeme(),
                    "-1"
                );
                let all = catalogue.check(esdieff::CheckRequest::new()).unwrap();
                assert!(all.outcome().errors() > ordinary.outcome().errors());
                assert_eq!(
                    catalogue.source(dependency).unwrap().raw_bytes(),
                    base.as_bytes()
                );
            }
        }
    }
}

#[test]
fn imported_required_arrays_follow_consumer_type_changes_without_source_replay() {
    for (source_type, consumer_type, selected_errors, catalogue_errors) in
        [("string", "object", 1, 1), ("object", "string", 0, 1)]
    {
        let base = format!(
            r##"{{"info":{{}},"namespace":{{"d":"https://example.com/base"}},"defaultNamespace":"d","sdfData":{{"Base":{{"type":"{source_type}","required":[]}}}}}}"##
        );
        let root_text = format!(
            r##"{{"info":{{}},"namespace":{{"d":"https://example.com/base"}},"sdfData":{{"Use":{{"sdfRef":"d:#/sdfData/Base","type":"{consumer_type}"}}}}}}"##
        );
        for reverse in [false, true] {
            let mut inputs = [
                SourceInput::supplied("root", root_text.as_bytes()),
                SourceInput::supplied("base", base.as_bytes()),
            ];
            if reverse {
                inputs.reverse();
            }
            let catalogue = Catalogue::from_inputs(&inputs).unwrap();
            let root = catalogue.source_id("root").unwrap();
            let selected = catalogue
                .check(esdieff::CheckRequest::new().root(root))
                .unwrap();
            assert_eq!(selected.outcome().errors(), selected_errors);
            assert_eq!(
                catalogue
                    .check(esdieff::CheckRequest::new())
                    .unwrap()
                    .outcome()
                    .errors(),
                catalogue_errors
            );
            let wire: serde_json::Value = serde_json::from_str(&selected.to_json()).unwrap();
            let required: Vec<_> = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["code"] == "data.required")
                .collect();
            assert_eq!(required.len(), selected_errors as usize);
            if let Some(row) = required.first() {
                assert_eq!(row["primary"]["kind"], "effective");
                assert_eq!(row["primary"]["pointer"], "/sdfData/Use/required");
                assert!(
                    row["related"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|location| location["source_id"]
                            == catalogue.source_id("base").unwrap().get()
                            && location["pointer"] == "/sdfData/Base/required")
                );
            }
        }
    }
}

#[test]
fn imported_specified_defaults_keep_consumer_locations_and_empty_authored_origins() {
    use esdieff::{Characteristic, InspectRequest, Limits, View};
    let base=br##"{"info":{},"namespace":{"d":"https://example.com/base"},"defaultNamespace":"d","sdfProperty":{"Base":{"type":"number","nullable":false,"writable":false,"minimum":0}}}"##;
    let root_text=br##"{"info":{},"namespace":{"d":"https://example.com/base"},"sdfProperty":{"Use":{"sdfRef":"d:#/sdfProperty/Base","nullable":null,"writable":null}}}"##;
    for omitted in [false, true] {
        let limits = if omitted {
            Limits::default()
                .diagnostic_bytes(1)
                .pointer_output_bytes(1)
        } else {
            Limits::default()
        };
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("root", root_text),
                SourceInput::supplied("base", base),
            ],
            limits,
        )
        .unwrap();
        let root = catalogue.source_id("root").unwrap();
        let report = catalogue
            .inspect(InspectRequest::new(root, "").view(View::Effective))
            .unwrap();
        assert!(report.outcome().complete());
        let model = report.effective_model().unwrap();
        let Lookup::Found(property) = model.definition(Class::Property, "Use") else {
            panic!("Use")
        };
        assert_eq!(
            property.writable_characteristic().require().unwrap(),
            Characteristic::SpecifiedDefault(true)
        );
        assert!(
            property
                .data_description()
                .require()
                .unwrap()
                .nullable()
                .require()
                .unwrap()
        );
        for quality in ["writable", "nullable"] {
            let explanation = model
                .default_explanation(&format!("/sdfProperty/Use/{quality}"))
                .require()
                .unwrap();
            assert_eq!(explanation.sources().count(), 0);
            let consumer = explanation.consumer().unwrap();
            assert_eq!(consumer.source_id(), root);
            assert_eq!(
                consumer.pointer(8192).require().unwrap(),
                "/sdfProperty/Use/sdfRef"
            );
        }
        if !omitted {
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            for quality in ["writable", "nullable"] {
                let path = format!("/sdfProperty/Use/{quality}");
                let explanation = wire["result"]["explanations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["kind"] == "default" && row["effective"]["pointer"] == path)
                    .unwrap();
                assert_eq!(explanation["default_value"], true);
                assert_eq!(explanation["sources"], serde_json::json!([]));
                assert_eq!(explanation["consumer"]["source_id"], root.get());
            }
        }
    }
}

#[test]
fn undecodable_mandatory_reference_retains_unfinished_dependency_metadata_scope() {
    use esdieff::{Assessment, AssessmentStatus, CheckRequest, Limits};
    let root_text=br##"{"info":{},"namespace":{"level":"https://example.com/level"},"sdfProperty":{"Controller":{"sdfRef":"level:#/sdfData/Level"}}}"##;
    for reference in [r#""\uD800""#, "7", "null", "false"] {
        let level = format!(
            r##"{{"info":{{}},"namespace":{{"own":"https://example.com/level"}},"defaultNamespace":"own","sdfData":{{"Level":{{"sdfRef":{reference},"type":"number"}}}}}}"##
        );
        for reverse in [false, true] {
            for limits in [
                Limits::default(),
                Limits::default()
                    .conditions(1)
                    .diagnostic_bytes(1)
                    .pointer_output_bytes(1),
                Limits::default().reference_applications(1),
            ] {
                let mut inputs = [
                    SourceInput::supplied("controller", root_text),
                    SourceInput::supplied("level", level.as_bytes()),
                ];
                if reverse {
                    inputs.reverse();
                }
                let catalogue = Catalogue::with_limits(&inputs, limits).unwrap();
                let root = catalogue.source_id("controller").unwrap();
                let dependency = catalogue.source_id("level").unwrap();
                let report = catalogue
                    .check(
                        CheckRequest::new()
                            .root(root)
                            .assessment(Assessment::ExpandedModified),
                    )
                    .unwrap();
                assert!(!report.effective_model().unwrap().complete());
                assert_eq!(report.outcome().errors(), 0);
                assert_eq!(
                    report
                        .assessment_area_status("modified-date-time/1")
                        .require()
                        .unwrap(),
                    AssessmentStatus::Undetermined
                );
                assert_eq!(report.requested_coverage().total(), Some(9));
                coverage_support::assert_requested_counts(&report);
                let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
                assert_eq!(
                    wire["request"]["scope"]["dependencies"],
                    serde_json::json!([dependency.get()])
                );
                assert!(wire["baseline"].get("temporal_knowledge").is_none());
                assert_eq!(
                    catalogue.source(dependency).unwrap().raw_bytes(),
                    level.as_bytes()
                );
            }
        }
    }
}
