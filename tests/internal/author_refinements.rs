use super::location_support;
use esdieff::{
    Assessment, AuthorObject, AuthorRequest, Catalogue, Disposition, EffectiveRequest, Field,
    JsonBuilder, OriginKind, ReportLocation, SourceInput,
};

const SOURCE: &str = r#"{"namespace":{"base":"https://example.com/capabilities"},"defaultNamespace":"base","sdfObject":{"Sensor / indoor":{"description":"Reusable sensor","sdfRequired":["temperature"],"sdfProperty":{"temperature":{"type":"number","nullable":false,"minimum":-50,"maximum":150,"multipleOf":0.10,"maxLength":18446744073709551616,"unit":"Cel","writable":false}},"sdfAction":{"calibrate":{"sdfInputData":{"type":"number","maximum":150},"sdfOutputData":{"type":"boolean"}}},"sdfEvent":{"alarm":{"sdfOutputData":{"type":"number","maximum":150}}}}}}"#;
const PLAN: &str = r#"{"key":"room-recipe","generated_key":"room-product","thing_name":"Room / Monitor","namespace":{"reuse":"https://example.com/capabilities","product":"https://example.com/products"},"default_namespace":"product","objects":[{"name":"ambient / sensor","source":"base","pointer":"/sdfObject/Sensor ~1 indoor","prefix":"reuse","refinement":{"description":null,"sdfProperty":{"temperature":{"description":"Product temperature","maximum":1e2}}},"required":true}]}"#;
const TEMPERATURE: &str =
    "/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/sdfProperty/temperature";

fn authored(assessment: Assessment) -> esdieff::AuthorResult {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE.as_bytes())]).unwrap();
    AuthorRequest::from_plan(catalogue, PLAN.as_bytes())
        .unwrap()
        .checking(assessment)
        .unwrap()
        .author()
        .unwrap()
}

fn budgeted_author(limits: esdieff::Limits, checking: bool) -> esdieff::AuthorResult {
    let extensions = (0..20)
        .map(|index| format!("\"vendor:meaning{index}\":{{\"x\":1}}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = SOURCE.replace(
        "\"description\":\"Reusable sensor\"",
        &format!("\"description\":\"Reusable sensor\",{extensions}"),
    );
    let catalogue =
        Catalogue::with_limits(&[SourceInput::supplied("base", source.as_bytes())], limits)
            .unwrap();
    let plan = PLAN.replace("\"maximum\":1e2", "\"maximum\":\"too hot\"");
    let request = AuthorRequest::from_plan(catalogue, plan.as_bytes()).unwrap();
    let request = if checking {
        request.checking(Assessment::Initial).unwrap()
    } else {
        request
    };
    request.author().unwrap()
}

#[test]
fn author_diagnostics_share_bytes_counts_and_locations_across_requested_checking() {
    for checking in [false, true] {
        let reference = budgeted_author(esdieff::Limits::default(), checking);
        let reference_wire: serde_json::Value = serde_json::from_str(&reference.to_json()).unwrap();
        let total = reference_wire["diagnostics"]["retained"].as_u64().unwrap()
            + reference_wire["diagnostics"]["omitted"].as_u64().unwrap();
        assert_eq!(total, if checking { 25 } else { 20 });
        for bytes in [1000, 1500, 2000, 3000, 5000] {
            for count in [1, 3, 100] {
                for pointers in [1, 128, 100_000] {
                    let result = budgeted_author(
                        esdieff::Limits::default()
                            .diagnostic_bytes(bytes)
                            .conditions(count)
                            .pointer_output_bytes(pointers),
                        checking,
                    );
                    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
                    let diagnostics = &wire["diagnostics"];
                    let items = diagnostics["items"].as_array().unwrap();
                    let cost = items
                        .iter()
                        .map(|item| serde_json::to_vec(item).unwrap().len() + 1)
                        .sum::<usize>();
                    assert!(
                        cost <= bytes,
                        "{checking}/{bytes}/{count}/{pointers}: {cost}"
                    );
                    assert!(items.len() <= count);
                    assert_eq!(result.diagnostics().conditions().len(), items.len());
                    assert_eq!(result.to_json(), result.to_json());
                    assert_eq!(diagnostics["retained"], items.len());
                    assert_eq!(
                        items.len() as u64 + diagnostics["omitted"].as_u64().unwrap(),
                        total
                    );
                    assert_eq!(wire["outcome"], reference_wire["outcome"]);
                    assert_eq!(
                        wire["result"]["generation"],
                        reference_wire["result"]["generation"]
                    );
                    assert_eq!(
                        wire["result"]["fidelity"],
                        reference_wire["result"]["fidelity"]
                    );
                    assert!(diagnostics["truncated"].as_bool().unwrap());
                    let mut pointer_cost = 0;
                    for item in items {
                        for location in std::iter::once(&item["primary"])
                            .chain(item["related"].as_array().unwrap())
                        {
                            if !location.is_object() {
                                continue;
                            }
                            if let Some(pointer) = location["pointer"].as_str() {
                                pointer_cost += pointer.len();
                            } else {
                                assert!(
                                    location["omission"].is_object(),
                                    "{checking}/{bytes}/{count}/{pointers}: {item}"
                                );
                                assert!(location["omission"]["reason"].as_str().is_some());
                            }
                        }
                    }
                    assert!(pointer_cost <= pointers, "{pointer_cost} > {pointers}");
                    let reasons = diagnostics["reasons"].as_array().unwrap();
                    let omissions = diagnostics["omissions"].as_array().unwrap();
                    let partitioned = omissions
                        .iter()
                        .filter(|omission| omission["field"] == "diagnostics.items")
                        .map(|omission| omission["omitted_count"].as_u64().unwrap())
                        .sum::<u64>();
                    if checking {
                        assert!(partitioned <= diagnostics["omitted"].as_u64().unwrap());
                        let check = result
                            .catalogue()
                            .check(esdieff::CheckRequest::new().root(result.generated_source()))
                            .unwrap();
                        let checked: serde_json::Value =
                            serde_json::from_str(&check.to_json()).unwrap();
                        for omission in checked["diagnostics"]["omissions"].as_array().unwrap() {
                            assert!(
                                omissions.contains(omission),
                                "copied omission changed: {omission}"
                            );
                        }
                        let coverage_cost = wire["result"]["checking"]["coverage"]["rows"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|row| serde_json::to_vec(row).unwrap().len() + 1)
                            .sum::<usize>();
                        assert!(cost + coverage_cost <= bytes);
                    } else {
                        assert_eq!(partitioned, diagnostics["omitted"].as_u64().unwrap());
                    }
                    for reason in reasons {
                        assert!(
                            omissions
                                .iter()
                                .any(|omission| omission["reason"] == *reason)
                        );
                    }
                    if count == 1 {
                        assert!(reasons.contains(&serde_json::json!("count_limit")));
                    }
                }
            }
        }
    }
}

#[test]
fn author_scope_retains_fidelity_dependencies_without_requesting_a_model_check() {
    for unknown_contributor in [false, true] {
        let inputs = if unknown_contributor {
            vec![
                SourceInput::supplied("base", SOURCE.as_bytes()),
                SourceInput::failed("possible-contributor"),
            ]
        } else {
            vec![SourceInput::supplied("base", SOURCE.as_bytes())]
        };
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let result = AuthorRequest::from_plan(catalogue, PLAN.as_bytes())
            .unwrap()
            .author()
            .unwrap();
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        let scope = &wire["request"]["scope"];
        assert_eq!(
            scope["roots"],
            serde_json::json!([result.generated_source().get()])
        );
        assert_eq!(wire["result"]["checking"]["state"], "not_requested");
        assert!(
            scope["assessed"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(1))
        );
        if unknown_contributor {
            assert_eq!(scope["dependencies"], serde_json::json!([]));
            assert_eq!(scope["unavailable_required"], serde_json::json!([2]));
            assert!(
                !scope["assessed"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(2))
            );
            assert!(
                result
                    .to_human()
                    .contains("Unavailable required sources: 2")
            );
        } else {
            assert_eq!(scope["dependencies"], serde_json::json!([1]));
            assert_eq!(scope["unavailable_required"], serde_json::json!([]));
            assert!(result.to_human().contains("Dependency sources: 1"));
        }
    }
    let catalogue = Catalogue::from_inputs(&[SourceInput::failed("base")]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, PLAN.as_bytes())
        .unwrap()
        .author()
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(
        wire["request"]["scope"]["dependencies"],
        serde_json::json!([])
    );
    let unavailable = wire["request"]["scope"]["unavailable_required"]
        .as_array()
        .unwrap();
    assert_eq!(unavailable.len(), 2);
    assert!(unavailable.contains(&serde_json::json!(1)));
    assert!(unavailable.contains(&serde_json::json!(2)));
    assert_eq!(wire["request"]["scope"]["assessed"], serde_json::json!([2]));
}

#[test]
fn caller_inspects_refined_ranges_deletion_and_nearest_source_and_request_origins() {
    let result = authored(Assessment::Initial);
    assert_eq!(result.outcome().disposition(), Disposition::Passed);
    let base = result.catalogue().source_id("base").unwrap();
    assert_eq!(
        result.catalogue().source(base).unwrap().raw_bytes(),
        SOURCE.as_bytes()
    );
    let effective = result
        .catalogue()
        .effective(EffectiveRequest::new(result.generated_source()))
        .unwrap();
    let maximum = effective
        .explanation(&format!("{TEMPERATURE}/maximum"))
        .require()
        .unwrap();
    assert_eq!(maximum.kind(), OriginKind::Override);
    assert_eq!(
        maximum.effective().root_document(),
        result.generated_source()
    );
    assert_eq!(
        maximum.effective().pointer().require().unwrap(),
        format!("{TEMPERATURE}/maximum")
    );
    let origins: Vec<_> = maximum.sources().collect();
    assert_eq!(origins[0].number().require().unwrap().lexeme(), "1e2");
    assert_eq!(origins[1].number().require().unwrap().lexeme(), "150");
    let request = result.request_origin(origins[0]).require().unwrap();
    assert_eq!(request.key(), "room-recipe");
    assert_eq!(
        request.pointer().require().unwrap(),
        "/objects/0/refinement/sdfProperty/temperature/maximum"
    );
    let span = request.span().unwrap();
    assert_eq!(&PLAN[span.start as usize..span.end as usize], "1e2");
    assert!(matches!(result.request_origin(origins[1]), Field::Absent));
    let minimum = effective
        .explanation(&format!("{TEMPERATURE}/minimum"))
        .require()
        .unwrap();
    assert_eq!(minimum.kind(), OriginKind::Inherited);
    assert_eq!(
        minimum
            .sources()
            .next()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "-50"
    );
    let deleted = effective
        .explanation("/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/description")
        .require()
        .unwrap();
    assert_eq!(deleted.kind(), OriginKind::Deleted);
    let null = deleted
        .sources()
        .find(|source| source.source_id() == result.generated_source())
        .unwrap();
    let deletion_request = result.request_origin(null).require().unwrap();
    let span = deletion_request.span().unwrap();
    assert_eq!(&PLAN[span.start as usize..span.end as usize], "null");
    let unchanged = effective.explanation("/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/sdfEvent/alarm/sdfOutputData/maximum").require().unwrap();
    assert_eq!(
        unchanged
            .sources()
            .next()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "150"
    );
}

#[test]
fn builder_origins_have_no_plan_span_and_foreign_views_cannot_alias_generated_identity() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE.as_bytes())]).unwrap();
    let base = catalogue.source_id("base").unwrap();
    let json = JsonBuilder::new();
    let patch = json
        .map(vec![(
            "sdfProperty",
            json.map(vec![(
                "temperature",
                json.map(vec![("maximum", json.number("1e2").unwrap())])
                    .unwrap(),
            )])
            .unwrap(),
        )])
        .unwrap();
    let result = AuthorRequest::new(catalogue, "typed-recipe", "product", "Room / Monitor")
        .unwrap()
        .namespace("reuse", "https://example.com/capabilities")
        .unwrap()
        .object(
            AuthorObject::new(
                "ambient / sensor",
                base,
                "/sdfObject/Sensor ~1 indoor",
                "reuse",
                true,
            )
            .unwrap()
            .refinement(patch)
            .unwrap(),
        )
        .unwrap()
        .author()
        .unwrap();
    let effective = result
        .catalogue()
        .effective(EffectiveRequest::new(result.generated_source()))
        .unwrap();
    let maximum = effective
        .explanation(&format!("{TEMPERATURE}/maximum"))
        .require()
        .unwrap();
    let origin = result
        .request_origin(maximum.sources().next().unwrap())
        .require()
        .unwrap();
    assert!(origin.span().is_none());
    assert_eq!(
        origin.pointer().require().unwrap(),
        "/objects/0/refinement/sdfProperty/temperature/maximum"
    );
    let foreign = Catalogue::from_inputs(&[
        SourceInput::supplied("base", SOURCE.as_bytes()),
        SourceInput::supplied("generated", result.generated_document().unwrap()),
    ])
    .unwrap();
    let foreign_effective = foreign
        .effective(EffectiveRequest::new(
            foreign.source_id("generated").unwrap(),
        ))
        .unwrap();
    let foreign_source = foreign_effective
        .explanation(&format!("{TEMPERATURE}/maximum"))
        .require()
        .unwrap()
        .sources()
        .next()
        .unwrap();
    assert!(matches!(
        result.request_origin(foreign_source),
        Field::Unavailable(_)
    ));
}

#[test]
fn representative_caller_keeps_author_and_requested_check_coverage_distinct() {
    for assessment in [Assessment::Initial, Assessment::FullBaseline] {
        let result = authored(assessment);
        let author = result.requested_coverage();
        assert_eq!(author.count_unit(), "authoring");
        assert_eq!(author.total(), Some(1));
        let checking = result.checking();
        assert_eq!(checking.assessment(), Some(assessment));
        assert_eq!(
            checking.requested_source_ids().unwrap(),
            [result.generated_source()]
        );
        let coverage = checking.requested_coverage().unwrap();
        assert_eq!(coverage.count_unit(), "assessment_area");
        assert_eq!(
            coverage.total(),
            Some(if assessment == Assessment::Initial {
                8
            } else {
                13
            })
        );
        assert_eq!(
            checking.outcome().unwrap().disposition(),
            if assessment == Assessment::Initial {
                Disposition::Passed
            } else {
                Disposition::Failed
            }
        );
        if assessment == Assessment::FullBaseline {
            assert_eq!(
                checking
                    .requirement_coverage()
                    .require()
                    .unwrap()
                    .count_unit(),
                "requirement"
            );
            assert_eq!(
                checking.requirement_coverage().require().unwrap().total(),
                Some(54)
            );
            assert_eq!(coverage.unassessed() + coverage.undetermined(), 0);
            assert!(checking.outcome().unwrap().errors() > 0);
            assert!(result.to_json().contains("formal.uint"));
        }
        let inspection = result
            .catalogue()
            .inspect(
                esdieff::InspectRequest::new(result.generated_source(), TEMPERATURE)
                    .view(esdieff::View::Effective),
            )
            .unwrap();
        assert_eq!(inspection.outcome().disposition(), Disposition::Passed);
        assert_eq!(inspection.requested_coverage().count_unit(), "inspection");
        assert!(result.generated_document().is_some());
    }
}

#[test]
fn author_check_findings_relate_generated_sources_to_exact_recipe_locations() {
    let invalid = PLAN.replace("1e2", "\"too hot\"");
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, invalid.as_bytes())
        .unwrap()
        .checking(Assessment::Initial)
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Failed);
    assert_eq!(
        result.outcome().complete(),
        result.checking().outcome().unwrap().complete()
    );
    assert_eq!(result.fidelity(), esdieff::Fidelity::Established);
    assert!(result.generated_document().is_some());
    assert!(!result.document_deliverable());
    let report: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    let finding = report["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "data.number" && finding["primary"]["kind"] == "source")
        .unwrap();
    let recipe = finding["related"]
        .as_array()
        .unwrap()
        .iter()
        .find(|location| location["kind"] == "author_request")
        .expect("author-request origin");
    assert_eq!(recipe["key"], "room-recipe");
    assert_eq!(
        recipe["pointer"],
        "/objects/0/refinement/sdfProperty/temperature/maximum"
    );
    let span = recipe["span"].as_array().unwrap();
    assert_eq!(
        &invalid[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
        "\"too hot\""
    );
    assert!(
        result
            .to_human()
            .contains("/objects/0/refinement/sdfProperty/temperature/maximum")
    );
    let diagnostics = result.diagnostics();
    location_support::assert_all_agree(&diagnostics, &result.to_json());
    let typed = diagnostics
        .conditions()
        .find(|condition| {
            condition.code() == "data.number"
                && matches!(
                    condition.primary(),
                    Field::Present(ReportLocation::Source(_))
                )
        })
        .unwrap()
        .related()
        .find_map(|location| match location {
            ReportLocation::AuthorRequest(location) => Some(location),
            _ => None,
        })
        .expect("typed author-request origin");
    assert_eq!(typed.key(), "room-recipe");
    assert!(matches!(
        typed.pointer(),
        Field::Present("/objects/0/refinement/sdfProperty/temperature/maximum")
    ));
    let span = typed.span().unwrap();
    assert_eq!(
        &invalid[span.start as usize..span.end as usize],
        "\"too hot\""
    );
}

#[test]
fn copied_internal_reference_follows_the_refined_generated_declaration() {
    // The reusable Sensor's own reference to its temperature relocates into the
    // generated Object, where the refinement applies. Follows ADR 0036: Copied
    // reference relocation.
    let source = SOURCE.replace("\"sdfProperty\":{", "\"sdfProperty\":{\"linked\":{\"sdfRef\":\"base:#/sdfObject/Sensor%20~1%20indoor/sdfProperty/temperature\"},");
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", source.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, PLAN.as_bytes())
        .unwrap()
        .checking(Assessment::Initial)
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.generation(), esdieff::Generation::Complete);
    assert_eq!(result.fidelity(), esdieff::Fidelity::Established);
    assert!(
        !result
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "author.fidelity_unknown")
    );
    let effective = result
        .catalogue()
        .effective(EffectiveRequest::new(result.generated_source()))
        .unwrap();
    let linked = TEMPERATURE.replace("temperature", "linked");
    let maximum = effective
        .explanation(&format!("{linked}/maximum"))
        .require()
        .unwrap();
    let source = maximum.sources().next().unwrap();
    assert_eq!(source.number().require().unwrap().lexeme(), "1e2");
    assert!(result.request_origin(source).present().is_some());
}

#[test]
fn reused_pointer_requiredness_relocates_into_the_generated_declaration() {
    // The reusable Sensor requires its own temperature through an absolute
    // pointer, which relocates into the generated Object. Follows ADR 0036:
    // Copied reference relocation.
    let source = SOURCE.replace(
        "\"sdfRequired\":[\"temperature\"]",
        "\"sdfRequired\":[\"#/sdfObject/Sensor%20~1%20indoor/sdfProperty/temperature\"]",
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", source.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, PLAN.as_bytes())
        .unwrap()
        .checking(Assessment::FullBaseline)
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.generation(), esdieff::Generation::Complete);
    assert_eq!(result.fidelity(), esdieff::Fidelity::Established);
    assert!(result.outcome().complete());
    assert!(
        !result
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "author.fidelity_unknown"
                || condition.code() == "required.target_missing")
    );
}

#[test]
fn copied_reference_uncertainty_keeps_independent_refined_observations_available() {
    // A short-name `sdfRef` keeps no processing meaning.
    let source = SOURCE.replace(
        "\"sdfProperty\":{",
        "\"sdfProperty\":{\"linked\":{\"sdfRef\":\"temperature\"},",
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", source.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, PLAN.as_bytes())
        .unwrap()
        .checking(Assessment::Initial)
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.generation(), esdieff::Generation::Complete);
    assert_eq!(result.fidelity(), esdieff::Fidelity::Undetermined);
    assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
    let effective = result
        .catalogue()
        .effective(EffectiveRequest::new(result.generated_source()))
        .unwrap();
    let maximum = effective
        .explanation(&format!("{TEMPERATURE}/maximum"))
        .require()
        .unwrap();
    assert_eq!(
        maximum
            .sources()
            .next()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "1e2"
    );
    assert!(
        result
            .request_origin(maximum.sources().next().unwrap())
            .present()
            .is_some()
    );
    assert!(
        result
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "author.fidelity_unknown")
    );
}

#[test]
fn request_origins_preserve_hash_names_without_reinterpreting_pointer_fragments() {
    let source = SOURCE.replace("Sensor / indoor", "Sensor# / indoor");
    let plan = PLAN
        .replace("Sensor ~1 indoor", "Sensor# ~1 indoor")
        .replace("ambient / sensor", "ambient#sensor");
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", source.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan.as_bytes())
        .unwrap()
        .author()
        .unwrap();
    let effective = result
        .catalogue()
        .effective(EffectiveRequest::new(result.generated_source()))
        .unwrap();
    let maximum = effective
        .explanation(
            "/sdfThing/Room ~1 Monitor/sdfObject/ambient#sensor/sdfProperty/temperature/maximum",
        )
        .require()
        .unwrap();
    let origin = result
        .request_origin(maximum.sources().next().unwrap())
        .require()
        .unwrap();
    assert_eq!(
        origin.pointer().require().unwrap(),
        "/objects/0/refinement/sdfProperty/temperature/maximum"
    );
    let span = origin.span().unwrap();
    assert_eq!(&plan[span.start as usize..span.end as usize], "1e2");
}

#[test]
fn duplicate_reference_findings_are_counted_once_when_check_detail_is_omitted() {
    const SOURCE: &str = r#"{"namespace":{"b":"https://example.com/base"},"defaultNamespace":"b","sdfObject":{"Sensor":{"sdfProperty":{"ok":{"type":"number"}}}}}"#;
    const PLAN: &str = r#"{"key":"recipe","generated_key":"product","thing_name":"Product","namespace":{"reuse":"https://example.com/base","product":"https://example.com/products"},"default_namespace":"product","objects":[{"name":"sensor","source":"base","pointer":"/sdfObject/Sensor","prefix":"reuse","required":false,"refinement":{"sdfProperty":{"broken1":{"sdfRef":"missing:#/sdfData/Lost1"},"broken2":{"sdfRef":"missing:#/sdfData/Lost2"}}}}]}"#;
    const REUSED_SOURCE: &str = r#"{"namespace":{"b":"https://example.com/base"},"defaultNamespace":"b","sdfObject":{"Sensor":{"sdfProperty":{"broken":{"sdfRef":"1bad:#/sdfData/Lost"}}}}}"#;
    const REUSED_PLAN: &str = r#"{"key":"recipe","generated_key":"product","thing_name":"Product","namespace":{"reuse":"https://example.com/base","product":"https://example.com/products"},"default_namespace":"product","objects":[{"name":"sensor1","source":"base","pointer":"/sdfObject/Sensor","prefix":"reuse","required":false},{"name":"sensor2","source":"base","pointer":"/sdfObject/Sensor","prefix":"reuse","required":false}]}"#;
    // The reused source has one authored reference problem, which both
    // declarations reach, so a partial check still establishes all of it.
    for (source, plan, partial_references, errors, partial_errors) in [
        (SOURCE, PLAN, 5, 2, 1),
        (REUSED_SOURCE, REUSED_PLAN, 6, 1, 1),
    ] {
        for references in [partial_references, 100_000] {
            let checked = if references == partial_references {
                partial_errors
            } else {
                errors
            };
            let mut expected = None;
            for (presentation, limits) in [
                ("default", esdieff::Limits::default()),
                ("count", esdieff::Limits::default().conditions(1)),
                ("bytes", esdieff::Limits::default().diagnostic_bytes(1)),
                (
                    "locations",
                    esdieff::Limits::default().pointer_output_bytes(1),
                ),
            ] {
                let catalogue = Catalogue::with_limits(
                    &[SourceInput::supplied("base", source.as_bytes())],
                    limits.reference_applications(references),
                )
                .unwrap();
                let result = AuthorRequest::from_plan(catalogue, plan.as_bytes())
                    .unwrap()
                    .checking(Assessment::Initial)
                    .unwrap()
                    .author()
                    .unwrap();
                let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
                let diagnostics = &wire["diagnostics"];
                let total = diagnostics["retained"].as_u64().unwrap()
                    + diagnostics["omitted"].as_u64().unwrap();
                assert_eq!(result.outcome().disposition(), Disposition::Failed);
                assert_eq!(result.outcome().errors(), errors);
                assert_eq!(result.checking().outcome().unwrap().errors(), checked);
                if let Some(expected) = expected {
                    assert_eq!(total, expected, "{references} references, {presentation}");
                } else {
                    expected = Some(total);
                }
                if checked != errors && presentation == "locations" {
                    assert!(
                        diagnostics["reasons"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|reason| reason == "presentation_limit")
                    );
                }
                assert_eq!(
                    u64::from(result.diagnostics().omitted())
                        + result.diagnostics().conditions().len() as u64,
                    total
                );
            }
        }
    }
}

#[test]
fn a_source_reference_problem_reused_by_two_declarations_is_one_finding() {
    // Both declarations copy the same source Object, so composition reaches its
    // broken reference twice. The source still has one authored problem.
    let source = SOURCE.replace(
        "\"sdfProperty\":{",
        "\"sdfProperty\":{\"broken\":{\"sdfRef\":\"missing:#/sdfData/Lost\"},",
    );
    let plan = PLAN.replace(
        "]}",
        r#",{"name":"second","source":"base","pointer":"/sdfObject/Sensor ~1 indoor","prefix":"reuse","refinement":{},"required":false}]}"#,
    );
    for checking in [false, true] {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("base", source.as_bytes())]).unwrap();
        let mut request = AuthorRequest::from_plan(catalogue, plan.as_bytes()).unwrap();
        assert_eq!(request.objects().len(), 2);
        if checking {
            request = request.checking(Assessment::Initial).unwrap();
        }
        let result = request.author().unwrap();
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        let unmapped: Vec<_> = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|condition| condition["code"] == "reference.prefix_unmapped")
            .collect();
        assert_eq!(unmapped.len(), 1, "{wire}");
        assert_eq!(result.outcome().errors(), 1, "{wire}");
        if checking {
            assert_eq!(result.checking().outcome().unwrap().errors(), 1);
        }
    }
}

#[test]
fn opaque_and_reference_limitations_retain_affected_coordinates_and_independent_errors() {
    let invalid_plan = PLAN.replace(
        "\"sdfProperty\":{",
        "\"sdfProperty\":{\"broken\":{\"sdfRef\":\"missing:#/sdfData/Lost\"},",
    );
    for checking in [false, true] {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE.as_bytes())]).unwrap();
        let mut request = AuthorRequest::from_plan(catalogue, invalid_plan.as_bytes()).unwrap();
        if checking {
            request = request.checking(Assessment::Initial).unwrap();
        }
        let result = request.author().unwrap();
        assert_eq!(result.outcome().disposition(), Disposition::Failed);
        if checking {
            assert_eq!(
                result.outcome().errors(),
                result.checking().outcome().unwrap().errors()
            );
        }
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        let reference = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|condition| condition["code"] == "reference.prefix_unmapped")
            .unwrap();
        assert_eq!(reference["severity"], "error");
        assert_eq!(reference["basis"]["kind"], "rfc");
    }
    let opaque = SOURCE.replace(
        "\"description\":\"Reusable sensor\"",
        "\"description\":\"Reusable sensor\",\"vendor:location\":{\"longitude\":1e2}",
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", opaque.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, PLAN.as_bytes())
        .unwrap()
        .author()
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    let limitation = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|condition| {
            condition["primary"]["pointer"]
                == "/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/vendor:location"
        })
        .unwrap();
    assert_eq!(limitation["primary"]["kind"], "effective");
    assert_eq!(limitation["related"][0]["source_id"], 1);
    assert_eq!(
        limitation["related"][0]["pointer"],
        "/sdfObject/Sensor ~1 indoor/vendor:location"
    );
    assert!(limitation["related"][0]["span"].is_array());
}
