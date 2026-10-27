use esdieff::{
    Assessment, AuthorObject, AuthorRequest, Catalogue, Class, Disposition, EffectiveRequest,
    Limits, SourceInput,
};

const SOURCE: &str = r#"{"namespace":{"base":"https://example.com/capabilities"},"defaultNamespace":"base","sdfObject":{"Sensor / indoor":{"description":"Reusable sensor","sdfProperty":{"temperature":{"type":"number","minimum":-50,"maximum":150,"multipleOf":0.10,"maxLength":18446744073709551616,"nullable":false,"writable":false}},"sdfRequired":["temperature"]},"Switch":{"sdfProperty":{"enabled":{"type":"boolean"}}}}}"#;

#[test]
fn author_preserves_sources_and_explicit_references() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("reusable", SOURCE.as_bytes())]).unwrap();
    let source = catalogue.source_id("reusable").unwrap();
    let result = AuthorRequest::new(catalogue, "recipe", "product", "Room / Monitor")
        .unwrap()
        .namespace("reuse", "https://example.com/capabilities")
        .unwrap()
        .namespace("product", "https://example.com/products")
        .unwrap()
        .default_namespace("product")
        .unwrap()
        .object(
            AuthorObject::new(
                "ambient / sensor",
                source,
                "/sdfObject/Sensor ~1 indoor",
                "reuse",
                true,
            )
            .unwrap(),
        )
        .unwrap()
        .object(AuthorObject::new("switch", source, "/sdfObject/Switch", "reuse", false).unwrap())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Passed);
    assert_eq!(
        result.catalogue().source(source).unwrap().raw_bytes(),
        SOURCE.as_bytes()
    );
    let document = result.generated_document().unwrap();
    let text = std::str::from_utf8(document).unwrap();
    assert!(text.contains("reuse:#/sdfObject/Sensor%20~1%20indoor"));
    assert!(text.ends_with('\n'));
    let effective = result
        .catalogue()
        .effective(EffectiveRequest::new(result.generated_source()))
        .unwrap();
    let esdieff::Lookup::Found(thing) = effective.definition(Class::Thing, "Room / Monitor") else {
        panic!("generated Thing")
    };
    let object = thing
        .children(Class::Object)
        .require()
        .unwrap()
        .find(|(name, _)| *name == "ambient / sensor")
        .unwrap()
        .1
        .require()
        .unwrap();
    let property = object
        .children(Class::Property)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .require()
        .unwrap();
    assert_eq!(
        property
            .data_description()
            .require()
            .unwrap()
            .multiple_of()
            .require()
            .unwrap()
            .lexeme(),
        "0.10"
    );
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["version"], "1.1");
    assert_eq!(wire["operation"], "author");
    assert_eq!(wire["coverage"]["requested"]["count_unit"], "authoring");
    assert_eq!(wire["result"]["checking"]["state"], "not_requested");
}

fn plan(source: &str) -> String {
    format!(
        r#"{{"key":"recipe","generated_key":"product","thing_name":"Room / Monitor","namespace":{{"reuse":"https://example.com/capabilities","product":"https://example.com/products"}},"default_namespace":"product","objects":[{{"name":"ambient / sensor","source":"{source}","pointer":"/sdfObject/Sensor ~1 indoor","prefix":"reuse","refinement":{{}},"required":true}}]}}"#
    )
}

#[test]
fn requested_checks_keep_their_independent_inventory_and_disposition() {
    for (assessment, expected) in [
        (Assessment::Initial, Disposition::Passed),
        (Assessment::FullBaseline, Disposition::Failed),
    ] {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE.as_bytes())]).unwrap();
        let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
            .unwrap()
            .checking(assessment)
            .unwrap()
            .author()
            .unwrap();
        assert_eq!(result.outcome().disposition(), expected);
        assert_eq!(result.generation(), esdieff::Generation::Complete);
        assert_eq!(result.fidelity(), esdieff::Fidelity::Established);
        assert!(result.generated_document().is_some());
        let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
        assert_eq!(wire["result"]["checking"]["state"], "requested");
        assert_ne!(
            wire["result"]["checking"]["coverage"]["requested"]["count_unit"],
            "authoring"
        );
        assert_eq!(wire["outcome"]["incomplete_checks"], 0);
        if assessment == Assessment::FullBaseline {
            assert!(result.to_json().contains("formal.uint"));
        }
    }
}

#[test]
fn local_only_and_opaque_objects_keep_candidate_and_fidelity_distinct() {
    let local = r#"{"sdfObject":{"Sensor / indoor":{"sdfProperty":{"t":{"type":"number"}}}}}"#;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", local.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
    assert!(result.generated_document().is_none());
    assert!(
        result
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "author.local_only_object")
    );
    let opaque = SOURCE.replace(
        "\"description\":\"Reusable sensor\"",
        "\"description\":\"Reusable sensor\",\"vendor:meaning\":{\"token\":0.10}",
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", opaque.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.generation(), esdieff::Generation::Complete);
    assert_eq!(result.fidelity(), esdieff::Fidelity::Undetermined);
    assert!(result.generated_document().is_some());
    assert!(!result.document_deliverable());
}

#[test]
fn collisions_bad_selectors_and_generation_budgets_are_explicit() {
    let collision = r#"{"namespace":{"p":"https://example.com/products"},"defaultNamespace":"p","sdfThing":{"Room / Monitor":{}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", SOURCE.as_bytes()),
        SourceInput::supplied("collision", collision.as_bytes()),
    ])
    .unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Failed);
    assert!(
        result
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "author.identity_collision")
    );
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("base", SOURCE.as_bytes())],
        Limits::default().catalogue_bytes(SOURCE.len() + 40),
    )
    .unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
    assert!(result.generated_document().is_none());
    assert!(
        result
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "author.generation_limit")
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE.as_bytes())]).unwrap();
    let request = AuthorRequest::from_plan(catalogue, plan("missing").as_bytes());
    assert!(request.is_err());
}

#[test]
fn unknown_contributor_context_never_establishes_unique_authoring_identity() {
    let uncertain = r#"{"info":{"features":["vendor:critical"]},"namespace":{"p":"https://example.com/products"},"defaultNamespace":"p","sdfThing":{"Room / Monitor":{}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", SOURCE.as_bytes()),
        SourceInput::supplied("uncertain", uncertain.as_bytes()),
    ])
    .unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
    assert!(result.generated_document().is_some());
    assert!(!result.document_deliverable());
    assert!(
        !result
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "author.identity_collision")
    );
    let critical_source = SOURCE.replace(
        "\"namespace\":",
        "\"info\":{\"features\":[\"vendor:critical\"]},\"namespace\":",
    );
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", critical_source.as_bytes())])
            .unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
    assert!(result.generated_document().is_none());
    assert!(
        !result
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "author.local_only_object")
    );
}

#[test]
fn generated_required_pointers_encode_hash_names_and_keep_decoded_identity() {
    let source = SOURCE.replace("Sensor / indoor", "Sensor# / indoor");
    let recipe = plan("base")
        .replace("Sensor ~1 indoor", "Sensor# ~1 indoor")
        .replace("ambient / sensor", "ambient#sensor");
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("base", source.as_bytes())]).unwrap();
    let result = AuthorRequest::from_plan(catalogue, recipe.as_bytes())
        .unwrap()
        .checking(Assessment::Initial)
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Passed);
    let generated = std::str::from_utf8(result.generated_document().unwrap()).unwrap();
    assert!(generated.contains("reuse:#/sdfObject/Sensor%23%20~1%20indoor"));
    assert!(generated.contains("#/sdfThing/Room%20~1%20Monitor/sdfObject/ambient%23sensor"));
    let report = result
        .catalogue()
        .inspect(
            esdieff::InspectRequest::new(result.generated_source(), "/sdfThing/Room ~1 Monitor")
                .view(esdieff::View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
}

#[test]
fn generated_attempt_reservation_and_omitted_errors_keep_their_meanings() {
    let keys: Vec<_> = (0..256).map(|index| format!("source-{index}")).collect();
    let inputs: Vec<_> = keys
        .iter()
        .map(|key| SourceInput::supplied(key, b"{}"))
        .collect();
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    assert!(AuthorRequest::new(catalogue, "recipe", "generated", "Room").is_err());
    let collision = r#"{"namespace":{"p":"https://example.com/products"},"defaultNamespace":"p","sdfThing":{"Room / Monitor":{}}}"#;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("base", SOURCE.as_bytes()),
            SourceInput::supplied("collision", collision.as_bytes()),
        ],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let result = AuthorRequest::from_plan(catalogue, plan("base").as_bytes())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Failed);
    assert_eq!(result.outcome().errors(), 1);
    assert_eq!(result.diagnostics().conditions().len(), 0);
    assert!(result.diagnostics().truncated());
    assert!(result.to_human().contains("Errors: 1"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&result.to_json()).unwrap()["outcome"]["errors"],
        1
    );
}
