//! Model declaration inspection and selected samples retain their separate scopes.
//! Follows ADR0025: Selected numeric samples, and
//! ADR0026: Required declaration relationships and source evidence.
use esdieff::{
    Admission, Catalogue, Class, EffectiveRequest, Field, InspectRequest, Lookup,
    RequiredReferenceStatus, RequiredTargetCapability, SampleKind, SampleRequest, SourceInput,
    View,
};
use serde_json::Value;

const SENSOR: &[u8] = include_bytes!("fixtures/temperature-device.sdf.json");
const OBJECT: &str = "/sdfThing/Device/sdfObject/ambient";
const TEMPERATURE: &str = "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature";

#[test]
fn raw_sample_inspection_is_allowed_but_required_inspection_refuses_sample_before_lookup() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", SENSOR),
        SourceInput::supplied("sample", SENSOR).sample(),
    ])
    .unwrap();
    let sample = catalogue.source_id("sample").unwrap();
    let raw = catalogue
        .inspect(InspectRequest::new(sample, OBJECT))
        .unwrap();
    assert!(raw.outcome().complete());
    assert!(matches!(raw.required_declarations(), Field::Absent));
    assert!(matches!(
        catalogue.source(sample).unwrap().definitions(Class::Object),
        Field::Unavailable(_)
    ));
    assert!(catalogue.effective(EffectiveRequest::new(sample)).is_err());
    assert!(
        catalogue
            .inspect(InspectRequest::new(sample, OBJECT).view(View::Effective))
            .is_err()
    );
    for view in [View::Source, View::Effective] {
        for pointer in [OBJECT, "/missing", "invalid pointer"] {
            assert_eq!(
                catalogue
                    .inspect(
                        InspectRequest::new(sample, pointer)
                            .view(view)
                            .required_declarations(true),
                    )
                    .err(),
                Some("sample source cannot be a model root"),
                "required inspection {view:?} at {pointer} must reject the role first"
            );
        }
    }
    let model = catalogue.source_id("model").unwrap();
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(
                InspectRequest::new(model, OBJECT)
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(report.required_declarations().require().unwrap().complete());
    }
}

#[test]
fn inherited_required_target_keeps_source_refinement_and_consumer_alongside_samples() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", SENSOR),
        SourceInput::supplied("good", b"80").sample(),
        SourceInput::supplied("too_hot", b"101").sample(),
    ])
    .unwrap();
    let model = catalogue.source_id("model").unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(model, OBJECT)
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    assert!(report.outcome().complete());
    assert!(report.sample_result().is_none());
    let owner = report
        .required_declarations()
        .require()
        .unwrap()
        .owners()
        .find(|owner| owner.class().present() == Some(Class::Object))
        .unwrap();
    let reference = owner.references().require().unwrap().next().unwrap();
    assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
    let Lookup::Found(mut targets) = reference.targets() else {
        panic!("temperature declaration target")
    };
    let target = targets.next().unwrap();
    let source = target.source_definition().require().unwrap();
    assert_eq!(source.json().source_id(), model);
    assert_eq!(
        source.json().pointer(8192).require().unwrap(),
        "/sdfObject/Sensor/sdfProperty/temperature"
    );
    assert_eq!(target.capability(), RequiredTargetCapability::Established);
    let effective = target.effective_definition().require().unwrap();
    assert_eq!(
        effective.json().location().pointer(8192).require().unwrap(),
        TEMPERATURE
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
        reference
            .effective_json()
            .require()
            .unwrap()
            .explanation()
            .consumer()
            .unwrap()
            .pointer(8192)
            .require()
            .unwrap(),
        format!("{OBJECT}/sdfRef")
    );
    assert_eq!(report.requested_source_ids(), &[model]);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["version"], "1.0");
    assert_eq!(wire["result"]["required_declarations"]["complete"], true);
    assert!(
        report
            .to_human()
            .contains("Required declarations: available")
    );

    for (sample, expected) in [
        ("good", Admission::Admitted),
        ("too_hot", Admission::Rejected),
    ] {
        let sample_id = catalogue.source_id(sample).unwrap();
        let checked = catalogue
            .check_sample(
                SampleRequest::new(model, SampleKind::Property, TEMPERATURE).sample(sample_id),
            )
            .unwrap();
        assert_eq!(checked.sample_result().unwrap().admission(), expected);
        assert!(checked.outcome().complete());
        assert!(matches!(checked.required_declarations(), Field::Absent));
        assert_eq!(checked.requested_source_ids(), &[model]);
        assert_eq!(checked.sample_result().unwrap().sample_id(), sample_id);
        let wire: Value = serde_json::from_str(&checked.to_json()).unwrap();
        assert_eq!(wire["version"], "1.1");
        assert_eq!(wire["operation"], "sample");
        assert!(wire["result"].get("required_declarations").is_none());
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 9);
        assert!(
            checked
                .to_human()
                .contains("Requested assessment: esdieff.sample/1")
        );
    }
}

#[test]
fn sample_namespace_duplicate_does_not_ambiguate_a_model_required_target() {
    let root = br##"{"namespace":{"d":"https://example.com/d"},"sdfObject":{"Root":{"sdfRequired":["d:#/sdfObject/Donor/sdfProperty/p"]}}}"##;
    let donor = br#"{"namespace":{"d":"https://example.com/d"},"defaultNamespace":"d","sdfObject":{"Donor":{"sdfProperty":{"p":{"type":"number"}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root),
        SourceInput::supplied("donor", donor),
        SourceInput::supplied("sample_duplicate", donor).sample(),
    ])
    .unwrap();
    let root = catalogue.source_id("root").unwrap();
    let donor = catalogue.source_id("donor").unwrap();
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(
                InspectRequest::new(root, "/sdfObject/Root")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        let owner = report
            .required_declarations()
            .require()
            .unwrap()
            .owners()
            .next()
            .unwrap();
        let reference = owner.references().require().unwrap().next().unwrap();
        assert_eq!(reference.status(), RequiredReferenceStatus::Resolved);
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("one model contributor must resolve")
        };
        let target = targets.next().unwrap();
        assert!(targets.next().is_none());
        assert_eq!(
            target
                .source_definition()
                .require()
                .unwrap()
                .json()
                .source_id(),
            donor
        );
        assert_eq!(target.capability(), RequiredTargetCapability::NotRequested);
        assert!(matches!(target.effective_definition(), Field::Absent));
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["request"]["scope"]["dependencies"],
            serde_json::json!([donor.get()])
        );
    }
}

#[test]
fn exact_numeric_admission_stays_independent_after_required_inspection() {
    let model = br#"{"sdfObject":{"O":{"sdfRequired":["p"],"sdfProperty":{"p":{"type":"number","multipleOf":0.1,"maximum":100}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("exact", b"0.3").sample(),
        SourceInput::supplied("inexact", b"0.30000000000000004").sample(),
    ])
    .unwrap();
    let model = catalogue.source_id("model").unwrap();
    for view in [View::Source, View::Effective] {
        let relationships = catalogue
            .inspect(
                InspectRequest::new(model, "/sdfObject/O")
                    .view(view)
                    .required_declarations(true),
            )
            .unwrap();
        assert!(
            relationships
                .required_declarations()
                .require()
                .unwrap()
                .complete()
        );
        for (sample, expected) in [
            ("exact", Admission::Admitted),
            ("inexact", Admission::Rejected),
        ] {
            let report = catalogue
                .check_sample(
                    SampleRequest::new(model, SampleKind::Property, "/sdfObject/O/sdfProperty/p")
                        .sample(catalogue.source_id(sample).unwrap()),
                )
                .unwrap();
            assert_eq!(report.sample_result().unwrap().admission(), expected);
            assert!(report.outcome().complete());
            assert!(matches!(report.required_declarations(), Field::Absent));
        }
    }
}
