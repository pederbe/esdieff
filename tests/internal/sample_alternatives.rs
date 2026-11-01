//! Selected samples retain effective alternatives and complete branch witnesses.
//! Follows ADR 0024: Effective alternatives and nullable characteristics, and
//! ADR 0025: Selected numeric samples.
use esdieff::{
    Admission, AlternativeId, AssessmentStatus, Catalogue, Class, ContextState, DataRole,
    EffectiveRequest, Field, Limits, Lookup, SampleKind, SampleRequest, SourceInput,
};

#[test]
fn selected_model_alternatives_stay_accessible_with_a_complete_sample_choice_witness() {
    let text = br##"{"sdfData":{"Base":{"type":"number","minimum":0,"maximum":100,"sdfChoice":{"wide":{"maximum":150},"shared":{}}}},"sdfObject":{"Device":{"sdfProperty":{"p":{"sdfRef":"#/sdfData/Base"}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", text),
        SourceInput::supplied("sample", b"101").sample(),
    ])
    .unwrap();
    let source = catalogue.source_id("model").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(source).pointer("/sdfObject/Device"))
        .unwrap();
    assert!(model.complete());
    let Lookup::Found(property) = model.definition(Class::Property, "p") else {
        panic!("selected Device Property p")
    };
    let data = property.data_description().require().unwrap();
    let mut branches = data.alternatives().require().unwrap();
    for (expected_name, expected_maximum) in [("wide", "150"), ("shared", "100")] {
        let (name, branch) = branches.next().unwrap();
        assert_eq!(name, AlternativeId::Choice(expected_name));
        let branch = branch.require().unwrap();
        assert_eq!(branch.minimum().require().unwrap().lexeme(), "0");
        assert_eq!(
            branch.maximum().require().unwrap().lexeme(),
            expected_maximum
        );
        let explanation = branch
            .json()
            .quality_explanation("minimum")
            .require()
            .unwrap();
        assert_eq!(
            explanation
                .sources()
                .next()
                .unwrap()
                .pointer()
                .require()
                .unwrap(),
            "/sdfData/Base/minimum"
        );
        assert_eq!(
            explanation.consumer().unwrap().pointer().require().unwrap(),
            "/sdfObject/Device/sdfProperty/p/sdfRef"
        );
    }
    assert!(branches.next().is_none());
    let report = catalogue
        .check_sample(
            SampleRequest::from_description(data)
                .unwrap()
                .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    let result = report.sample_result().unwrap();
    assert_eq!(result.selected_kind(), SampleKind::Property);
    assert_eq!(result.admission(), Admission::Admitted);
    assert_eq!(result.context_state(), ContextState::Available);
    assert_eq!(
        result
            .area_status("sample-alternatives/1")
            .require()
            .unwrap(),
        AssessmentStatus::Assessed
    );
    assert_eq!(report.outcome().errors(), 0);
    assert!(report.outcome().complete());
    assert!(matches!(
        result.description().require().unwrap().alternatives(),
        Field::Present(_)
    ));
    assert_eq!(catalogue.source(source).unwrap().raw_bytes(), text);
}

#[test]
fn imported_restricted_alternatives_keep_unknown_donors_and_cannot_be_selected_as_sample_roots() {
    let root = br##"{"namespace":{"d":"https://example.com/data"},"sdfProperty":{"rows":{"sdfRef":"d:#/sdfData/Rows"}}}"##;
    let donor = br##"{"namespace":{"d":"https://example.com/data"},"defaultNamespace":"d","sdfData":{"Rows":{"type":"array","items":{"type":"array","nullable":false,"minimum":1,"maximum":2,"sdfChoice":{"unknown":{},"explicit":{"type":"number","nullable":true}}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root),
        SourceInput::supplied("donor", donor),
        SourceInput::supplied("sample", b"[]").sample(),
    ])
    .unwrap();
    let root_id = catalogue.source_id("root").unwrap();
    let donor_id = catalogue.source_id("donor").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(root_id)).unwrap();
    let Lookup::Found(rows) = model.definition(Class::Property, "rows") else {
        panic!("imported Rows Property")
    };
    let data = rows.data_description().require().unwrap();
    for (id, branch) in data
        .items()
        .require()
        .unwrap()
        .alternatives()
        .require()
        .unwrap()
    {
        let branch = branch.require().unwrap();
        assert_eq!(branch.minimum().require().unwrap().lexeme(), "1");
        assert_eq!(branch.maximum().require().unwrap().lexeme(), "2");
        if id == AlternativeId::Choice("unknown") {
            assert!(matches!(branch.data_type(), Field::Unavailable(_)));
            assert!(matches!(branch.nullable(), Field::Unavailable(_)));
            assert!(!branch.json().capability_complete());
        } else {
            assert!(matches!(branch.data_type(), Field::Present(_)));
            assert!(branch.nullable().require().unwrap());
        }
        let explanation = branch
            .json()
            .quality_explanation("minimum")
            .require()
            .unwrap();
        assert_eq!(explanation.sources().next().unwrap().source_id(), donor_id);
        assert_eq!(
            explanation
                .sources()
                .next()
                .unwrap()
                .pointer()
                .require()
                .unwrap(),
            "/sdfData/Rows/items/minimum"
        );
        assert_eq!(
            explanation.consumer().unwrap().pointer().require().unwrap(),
            "/sdfProperty/rows/sdfRef"
        );
        assert!(SampleRequest::from_description(branch).is_err());
    }
    let report = catalogue
        .check_sample(
            SampleRequest::from_description(data)
                .unwrap()
                .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    let result = report.sample_result().unwrap();
    assert_eq!(result.admission(), Admission::Admitted);
    assert_eq!(
        result.area_status("sample-array/1").require().unwrap(),
        AssessmentStatus::Assessed
    );
    assert_eq!(report.outcome().errors(), 0);
    assert!(report.outcome().complete());
    let deferred_item = result
        .description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert_eq!(deferred_item.role(), DataRole::DirectItem);
    assert!(!deferred_item.json().capability_complete());
    assert!(matches!(deferred_item.data_type(), Field::Unavailable(_)));
    assert!(matches!(deferred_item.nullable(), Field::Unavailable(_)));
    assert!(matches!(
        deferred_item.json().to_json(),
        Field::Unavailable(_)
    ));
    assert!(SampleRequest::from_description(deferred_item).is_err());
    let source_item = catalogue
        .source(donor_id)
        .unwrap()
        .json()
        .require()
        .unwrap()
        .get("sdfData")
        .require()
        .unwrap()
        .get("Rows")
        .require()
        .unwrap()
        .get("items")
        .require()
        .unwrap();
    assert_eq!(deferred_item.json().source_json().source_id(), donor_id);
    assert_eq!(
        deferred_item.json().source_json().raw_json(),
        source_item.raw_json()
    );
    assert_eq!(
        deferred_item
            .json()
            .source_json()
            .pointer()
            .require()
            .unwrap(),
        "/sdfData/Rows/items"
    );
    assert_eq!(catalogue.source(root_id).unwrap().raw_bytes(), root);
    assert_eq!(catalogue.source(donor_id).unwrap().raw_bytes(), donor);
    let nonempty = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root),
        SourceInput::supplied("donor", donor),
        SourceInput::supplied("sample", b"[[]]").sample(),
    ])
    .unwrap();
    let report = nonempty
        .check_sample(
            SampleRequest::new(
                nonempty.source_id("root").unwrap(),
                SampleKind::Property,
                "/sdfProperty/rows",
            )
            .sample(nonempty.source_id("sample").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    assert_eq!(
        nonempty
            .source(nonempty.source_id("donor").unwrap())
            .unwrap()
            .raw_bytes(),
        donor
    );
}

#[test]
fn alternatives_outside_the_selected_sample_context_do_not_enter_model_findings_or_scope() {
    let text = br##"{"sdfProperty":{"p":{"type":"number","maximum":100,"properties":{"unused":{"items":{"type":"array","nullable":false,"sdfChoice":{"invalid":false,"unresolved":{"sdfRef":"missing:#/sdfData/X"}}}}},"sdfRequired":["missing:#/sdfData/X"]}}}"##;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("model", text),
            SourceInput::supplied("sample", b"101").sample(),
        ],
        Limits::default().decoding_work(5000),
    )
    .unwrap();
    let source = catalogue.source_id("model").unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(source, SampleKind::Property, "/sdfProperty/p")
                .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    let result = report.sample_result().unwrap();
    assert_eq!(result.admission(), Admission::Rejected);
    assert_eq!(result.context_state(), ContextState::Available);
    assert_eq!(result.context_findings().errors(), 0);
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(
        result
            .area_status("sample-alternatives/1")
            .require()
            .unwrap(),
        AssessmentStatus::NotApplicable
    );
    assert!(matches!(
        result
            .description()
            .require()
            .unwrap()
            .json()
            .get("properties"),
        Field::Unavailable(_)
    ));
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["request"]["scope"]["dependencies"],
        serde_json::json!([])
    );
    assert_eq!(
        wire["request"]["scope"]["unavailable_required"],
        serde_json::json!([])
    );
    assert_eq!(catalogue.source(source).unwrap().raw_bytes(), text);
}
