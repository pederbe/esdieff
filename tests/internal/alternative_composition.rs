//! Alternatives retain supplied composition identity, importing boundaries and scoped uncertainty.
//! Follows ADR 0023: Supplied global composition and dependency scope, and
//! ADR 0024: Effective alternatives and nullable characteristics.
use esdieff::{
    AlternativeId, Assessment, AssessmentStatus, Catalogue, CheckRequest, Class, EffectiveRequest,
    Field, InspectRequest, Limits, Lookup, OriginKind, SourceInput, View,
};

#[test]
fn imported_enum_chain_preserves_scalar_sources_outer_consumers_and_dependency_knowledge() {
    let root_text = br##"{"info":{},"namespace":{"a":"https://example.com/A"},"sdfProperty":{"Use":{"sdfRef":"a:#/sdfData/Base","maximum":8}}}"##;
    let middle = br##"{"info":{"title":7},"namespace":{"b":"https://example.com/B","own":"https://example.com/A"},"defaultNamespace":"own","sdfData":{"Base":{"sdfRef":"b:#/sdfData/Base"}}}"##;
    let leaf = br#"{"info":{"modified":"2016-12-31T23:59:60Z"},"namespace":{"own":"https://example.com/B"},"defaultNamespace":"own","sdfData":{"Base":{"minimum":1.0,"enum":["a","a"]}}}"#;
    for reverse in [false, true] {
        let mut inputs = [
            SourceInput::supplied("root", root_text),
            SourceInput::supplied("middle", middle),
            SourceInput::supplied("leaf", leaf),
        ];
        if reverse {
            inputs.reverse();
        }
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let root = catalogue.source_id("root").unwrap();
        let leaf_id = catalogue.source_id("leaf").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
        assert!(model.complete());
        let Lookup::Found(definition) = model.definition(Class::Property, "Use") else {
            panic!("Use")
        };
        for (id, data) in definition
            .data_description()
            .require()
            .unwrap()
            .alternatives()
            .require()
            .unwrap()
        {
            let data = data.require().unwrap();
            assert_eq!(data.minimum().require().unwrap().lexeme(), "1.0");
            assert_eq!(data.maximum().require().unwrap().lexeme(), "8");
            let AlternativeId::Enum(index) = id else {
                panic!("enum")
            };
            let constant = data.json().quality_explanation("const").require().unwrap();
            let source = constant.sources().next().unwrap();
            assert_eq!(source.source_id(), leaf_id);
            assert_eq!(
                source.pointer(8192).require().unwrap(),
                format!("/sdfData/Base/enum/{index}")
            );
            assert_eq!(
                constant.effective().pointer(8192).require().unwrap(),
                format!("/sdfProperty/Use/enum/{index}/const")
            );
            for origin in [
                data.json().explanation(),
                constant,
                data.json()
                    .quality_explanation("minimum")
                    .require()
                    .unwrap(),
                data.json()
                    .default_explanation("nullable")
                    .require()
                    .unwrap(),
            ] {
                let consumer = origin.consumer().unwrap();
                assert_eq!(consumer.source_id(), root);
                assert_eq!(
                    consumer.pointer(8192).require().unwrap(),
                    "/sdfProperty/Use/sdfRef"
                );
            }
        }
        let report = catalogue
            .check(
                CheckRequest::new()
                    .root(root)
                    .assessment(Assessment::ExpandedModified),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(
            report
                .assessment_area_status("modified-date-time/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        assert_eq!(report.requested_coverage().total(), Some(9));
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["request"]["scope"]["dependencies"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(wire["baseline"].get("temporal_knowledge").is_some());
    }
}

#[test]
fn imported_named_alternatives_preserve_nested_consumer_and_default_annotations_in_both_formats() {
    let root_text = br##"{"info":{},"namespace":{"d":"https://example.com/base"},"sdfData":{"Use":{"sdfRef":"d:#/sdfData/Base"}}}"##;
    let base = br##"{"info":{},"namespace":{"own":"https://example.com/base"},"defaultNamespace":"own","sdfData":{"Base":{"type":"object","properties":{"field":{"sdfRef":"#/sdfData/Leaf"}},"sdfChoice":{"branch":{"const":null}}},"Leaf":{"type":"number","minimum":20}}}"##;
    for (limit_index, limits) in [
        Limits::default(),
        Limits::default().pointer_bytes(1),
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
    ]
    .into_iter()
    .enumerate()
    {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("root", root_text),
                SourceInput::supplied("base", base),
            ],
            limits,
        )
        .unwrap();
        let root = catalogue.source_id("root").unwrap();
        let base_id = catalogue.source_id("base").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
        assert!(model.complete());
        let Lookup::Found(definition) = model.definition(Class::Data, "Use") else {
            panic!("Use")
        };
        let branch = definition
            .data_description()
            .require()
            .unwrap()
            .alternatives()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .require()
            .unwrap();
        assert_eq!(
            branch.constant().require().unwrap().kind(),
            esdieff::JsonKind::Null
        );
        let field = branch
            .properties()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .require()
            .unwrap();
        let minimum = field
            .json()
            .quality_explanation("minimum")
            .require()
            .unwrap();
        assert_eq!(minimum.sources().next().unwrap().source_id(), base_id);
        let nested_consumer = minimum.consumer().unwrap();
        assert_eq!(nested_consumer.source_id(), base_id);
        assert_eq!(
            nested_consumer.pointer(8192).require().unwrap(),
            "/sdfData/Base/properties/field/sdfRef"
        );
        let default = branch
            .json()
            .default_explanation("nullable")
            .require()
            .unwrap();
        assert_eq!(default.kind(), OriginKind::Default);
        assert_eq!(default.sources().count(), 0);
        assert_eq!(default.consumer().unwrap().source_id(), root);
        let report = catalogue
            .inspect(InspectRequest::new(root, "/sdfData/Use").view(View::Effective))
            .unwrap();
        assert!(report.outcome().complete());
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let owners = wire["result"]["alternatives"].as_array().unwrap();
        if limit_index == 0 {
            let owner = owners
                .iter()
                .find(|owner| owner["owner"]["pointer"] == "/sdfData/Use")
                .expect("retained owner");
            assert!(
                owner["branches"][0]["explanations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["kind"] == "default"
                        && row["effective"]["pointer"] == "/sdfData/Use/sdfChoice/branch/nullable")
            );
        }
        if let Some(owner) = owners
            .iter()
            .find(|owner| owner["owner"]["pointer"] == "/sdfData/Use")
            && let Some(default) = owner["branches"][0]["explanations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| {
                    row["kind"] == "default"
                        && row["effective"]["pointer"] == "/sdfData/Use/sdfChoice/branch/nullable"
                })
        {
            assert_eq!(default["default_value"], true);
            assert_eq!(default["consumer"]["source_id"], root.get());
            assert!(default["sources"].as_array().unwrap().is_empty());
            let human = report.to_human();
            assert!(human.contains("default true") && human.contains("imported at"));
            assert!(human.contains("/sdfData/Base/properties/field/sdfRef"));
        }
        assert_eq!(wire["version"], "1.0");
    }
}

#[test]
fn a_known_nonmap_branch_alone_keeps_alternative_establishment_incomplete() {
    let text =
        br#"{"info":{},"sdfData":{"Mode":{"minimum":1,"sdfChoice":{"bad":false,"known":{}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let source = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(source)).unwrap();
    assert!(!model.complete());
    let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
        panic!("Mode")
    };
    for (id, branch) in mode
        .data_description()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
    {
        if id == AlternativeId::Choice("bad") {
            assert!(matches!(branch, Field::Unavailable(_)));
        } else {
            assert_eq!(
                branch
                    .require()
                    .unwrap()
                    .minimum()
                    .require()
                    .unwrap()
                    .lexeme(),
                "1"
            );
        }
    }
    let report = catalogue
        .inspect(InspectRequest::new(source, "/sdfData/Mode").view(View::Effective))
        .unwrap();
    assert!(!report.outcome().complete());
}

#[test]
fn ambiguous_shared_fields_keep_unavailable_copied_views_and_related_contributor_locations() {
    let root_text = br##"{"info":{},"namespace":{"p":"https://example.com/shared"},"sdfData":{"Mode":{"minimum":1,"properties":{"field":{"sdfRef":"p:#/sdfData/Target"}},"sdfChoice":{"one":{},"two":{}}}}}"##;
    let target = br#"{"info":{},"namespace":{"own":"https://example.com/shared"},"defaultNamespace":"own","sdfData":{"Target":{"maximum":2}}}"#;
    let mut expected_outcome = None;
    for (limit_index, limits) in [
        Limits::default(),
        Limits::default().pointer_bytes(1),
        Limits::default().diagnostic_bytes(1),
    ]
    .into_iter()
    .enumerate()
    {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("root", root_text),
                SourceInput::supplied("first", target),
                SourceInput::supplied("second", target),
            ],
            limits,
        )
        .unwrap();
        let root = catalogue.source_id("root").unwrap();
        let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
        assert!(!model.complete());
        let Lookup::Found(mode) = model.definition(Class::Data, "Mode") else {
            panic!("Mode")
        };
        for (_, data) in mode
            .data_description()
            .require()
            .unwrap()
            .alternatives()
            .require()
            .unwrap()
        {
            let data = data.require().unwrap();
            assert_eq!(data.minimum().require().unwrap().lexeme(), "1");
            assert!(!data.json().capability_complete());
            let properties = data.json().get("properties").require().unwrap();
            assert!(matches!(
                data.properties().require().unwrap().next().unwrap().1,
                Field::Unavailable(_)
            ));
            let explanation = properties.quality_explanation("field").require().unwrap();
            assert_eq!(explanation.sources().next().unwrap().source_id(), root);
            assert_eq!(
                explanation
                    .sources()
                    .next()
                    .unwrap()
                    .pointer(8192)
                    .require()
                    .unwrap(),
                "/sdfData/Mode/properties/field"
            );
        }
        let report = catalogue
            .inspect(InspectRequest::new(root, "/sdfData/Mode").view(View::Effective))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(!report.outcome().complete());
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        if let Some(expected) = &expected_outcome {
            assert_eq!(&wire["outcome"], expected);
        } else {
            expected_outcome = Some(wire["outcome"].clone());
        }
        if limit_index == 0 {
            let ambiguous = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["code"] == "reference.ambiguous")
                .unwrap();
            let related = ambiguous["related"].as_array().unwrap();
            for name in ["first", "second"] {
                let id = catalogue.source_id(name).unwrap().get();
                assert!(
                    related
                        .iter()
                        .any(|row| row["source_id"] == id && row["pointer"] == "/sdfData/Target")
                );
            }
        } else {
            assert!(
                !wire["diagnostics"]["omissions"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn unavailable_imported_enum_cannot_fall_back_to_source_and_keeps_modified_uncertainty() {
    let root_text = br##"{"info":{},"namespace":{"p":"https://example.com/base"},"sdfData":{"Use":{"sdfRef":"p:#/sdfData/Base"},"Known":{"minimum":3}}}"##;
    let base = br#"{"info":{},"namespace":{"own":"https://example.com/base"},"defaultNamespace":"own","sdfData":{"Base":{"sdfRef":"\uD800","enum":["a"]}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root_text),
        SourceInput::supplied("base", base),
    ])
    .unwrap();
    let root = catalogue.source_id("root").unwrap();
    let report = catalogue
        .check(
            CheckRequest::new()
                .root(root)
                .assessment(Assessment::ExpandedModified),
        )
        .unwrap();
    assert_eq!(
        report
            .assessment_area_status("modified-date-time/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
    assert_eq!(report.requested_coverage().total(), Some(9));
    let model = report.effective_model().unwrap();
    assert!(matches!(
        model.definition(Class::Data, "Use"),
        Lookup::Unavailable(_)
    ));
    let Lookup::Found(known) = model.definition(Class::Data, "Known") else {
        panic!("Known")
    };
    assert_eq!(
        known
            .data_description()
            .require()
            .unwrap()
            .minimum()
            .require()
            .unwrap()
            .lexeme(),
        "3"
    );
    let source = catalogue
        .source(catalogue.source_id("base").unwrap())
        .unwrap();
    let Lookup::Found(base) = source.definition(Class::Data, "Base") else {
        panic!("source Base")
    };
    assert_eq!(
        base.data_description()
            .require()
            .unwrap()
            .source_enum()
            .require()
            .unwrap()
            .count(),
        1
    );
}
