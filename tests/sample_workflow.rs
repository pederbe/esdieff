//! Declaring selection and provenance follow ADR0025: Selected numeric samples.
mod location_support;
use esdieff::{
    Admission, Catalogue, CheckRequest, Class, ConditionDomain, ContextState, EffectiveRequest,
    Field, JsonBuilder, Limits, Lookup, SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;
const MODEL: &[u8] = br##"{"sdfData":{"D":{"type":"number","maximum":150}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/D","maximum":100}},"sdfAction":{"a":{"sdfInputData":{"sdfRef":"#/sdfData/D"},"sdfOutputData":{"type":"number","maximum":100}}},"sdfEvent":{"e":{"sdfOutputData":{"sdfRef":"#/sdfData/D"}}}}"##;
#[test]
fn five_declaring_selectors_share_the_public_workflow_and_keep_sources_immutable() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", MODEL),
        SourceInput::supplied("s", b"101").sample(),
    ])
    .unwrap();
    for (kind, pointer, admission) in [
        (SampleKind::Property, "/sdfProperty/p", Admission::Rejected),
        (SampleKind::Data, "/sdfData/D", Admission::Admitted),
        (SampleKind::ActionInput, "/sdfAction/a", Admission::Admitted),
        (
            SampleKind::ActionOutput,
            "/sdfAction/a",
            Admission::Rejected,
        ),
        (SampleKind::EventOutput, "/sdfEvent/e", Admission::Admitted),
    ] {
        let report = {
            let pointer = pointer.to_owned();
            catalogue
                .check_sample(
                    SampleRequest::new(catalogue.source_id("m").unwrap(), kind, &pointer)
                        .sample(catalogue.source_id("s").unwrap()),
                )
                .unwrap()
        };
        assert_eq!(report.sample_result().unwrap().admission(), admission);
        assert!(report.outcome().complete());
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
        assert_eq!(wire["request"]["sample"], wire["result"]["sample"]);
        assert_eq!(wire["result"]["context"]["model_assessment"], Value::Null);
        let coverage = report.requested_coverage();
        assert_eq!(coverage.inventory(), Some("esdieff.sample/1"));
        assert_eq!(coverage.count_unit(), "sample_area");
        assert_eq!(
            coverage.assessed()
                + coverage.not_applicable()
                + coverage.unassessed()
                + coverage.undetermined(),
            9
        );
        assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 9);
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|condition| condition["domain"].is_string())
        );
        assert!(report.to_human().contains(admission.as_str()));
        if admission == Admission::Rejected {
            let diagnostics = report.diagnostics();
            let condition = diagnostics
                .conditions()
                .find(|c| c.domain() == Some(ConditionDomain::Sample))
                .unwrap();
            assert_eq!(
                location_support::primary_sample(condition).span(),
                Some(0..3)
            );
            assert!(!wire["result"]["witnesses"].as_array().unwrap().is_empty());
        }
    }
    assert_eq!(
        catalogue
            .source(catalogue.source_id("m").unwrap())
            .unwrap()
            .raw_bytes(),
        MODEL
    );
    assert_eq!(
        catalogue
            .source(catalogue.source_id("s").unwrap())
            .unwrap()
            .raw_bytes(),
        b"101"
    );
}
#[test]
fn handle_selection_matches_pointer_and_rejects_cross_owner_or_wrong_roots() {
    let inputs = [
        SourceInput::supplied("m", MODEL),
        SourceInput::supplied("s", b"101").sample(),
    ];
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let other = Catalogue::from_inputs(&inputs).unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("m").unwrap()))
        .unwrap();
    let Lookup::Found(property) = model.definition(Class::Property, "p") else {
        panic!("Property p");
    };
    let handle = property.data_description().require().unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::from_description(handle)
                .unwrap()
                .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(
        report.sample_result().unwrap().selected_pointer(),
        "/sdfProperty/p"
    );
    assert!(
        other
            .check_sample(
                SampleRequest::from_description(handle)
                    .unwrap()
                    .sample(other.source_id("s").unwrap())
            )
            .is_err()
    );
    for (kind, pointer) in [
        (SampleKind::Property, "/sdfAction/a"),
        (SampleKind::ActionInput, "/sdfProperty/p"),
        (SampleKind::Data, "/sdfProperty/p/maximum"),
        (SampleKind::EventOutput, "/sdfEvent/absent"),
    ] {
        assert!(
            catalogue
                .check_sample(
                    SampleRequest::new(catalogue.source_id("m").unwrap(), kind, pointer)
                        .sample(catalogue.source_id("s").unwrap())
                )
                .is_err()
        );
    }
    assert!(
        catalogue
            .check_sample(SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p"
            ))
            .is_err()
    );
}
#[test]
fn typed_rejection_has_no_original_span_in_any_report_location() {
    let builder = JsonBuilder::new();
    let catalogue = Catalogue::take_inputs(vec![
        SourceInput::supplied("m", MODEL),
        SourceInput::typed("s", builder.number("101").unwrap()),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    let result = report.sample_result().unwrap();
    assert_eq!(result.admission(), Admission::Rejected);
    assert_eq!(result.sample_value().require().unwrap().span(), None);
    assert!(matches!(
        catalogue.source(result.sample_id()).unwrap().json(),
        Field::Unavailable(_)
    ));
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["result"]["sample_location"]["span"], Value::Null);
    for condition in wire["diagnostics"]["items"].as_array().unwrap() {
        if condition["domain"] == "sample" {
            assert_eq!(condition["primary"]["span"], Value::Null);
        }
    }
    for witness in wire["result"]["witnesses"].as_array().unwrap() {
        assert_eq!(witness["sample"]["span"], Value::Null);
    }
}
#[test]
fn root_refusal_is_distinct_from_known_wrong_selector_and_null() {
    for sample in [b"null null".as_slice(), br#"{"a":1,"a":2}"#, b"\xff"] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", MODEL),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
        assert_eq!(
            report.sample_result().unwrap().context_state(),
            ContextState::Available
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-context/1")
                .require()
                .unwrap(),
            esdieff::AssessmentStatus::Undetermined
        );
        assert_eq!(
            catalogue
                .source(catalogue.source_id("s").unwrap())
                .unwrap()
                .raw_bytes(),
            sample
        );
    }
    for (nullable, expected) in [
        ("true", Admission::Admitted),
        ("false", Admission::Rejected),
    ] {
        let model = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"number","nullable":{nullable},"maximum":100}}}}}}"#
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", b"null").sample(),
        ])
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete());
    }
}
#[test]
fn optional_presentation_omission_preserves_a_definite_rejection_and_nine_counts() {
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", MODEL),
            SourceInput::supplied("s", b"101").sample(),
        ],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(report.outcome().errors(), 1);
    assert!(report.outcome().complete());
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    assert_eq!(wire["coverage"]["rows_omitted"], 9);
    assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() > 0);
}
#[test]
fn sdf_looking_sample_is_never_a_model_root_or_namespace_contributor() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", MODEL),
        SourceInput::supplied("s", MODEL).sample(),
    ])
    .unwrap();
    let id = catalogue.source_id("s").unwrap();
    assert!(catalogue.check(CheckRequest::new().root(id)).is_err());
    assert!(catalogue.effective(EffectiveRequest::new(id)).is_err());
    assert!(
        catalogue
            .inspect(esdieff::InspectRequest::new(id, "").view(esdieff::View::Effective))
            .is_err()
    );
    assert!(
        catalogue
            .inspect(esdieff::InspectRequest::new(id, ""))
            .is_ok()
    );
    assert!(matches!(
        catalogue.source(id).unwrap().definitions(Class::Data),
        Field::Unavailable(_)
    ));
    let check = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(
        check.requested_source_ids(),
        &[catalogue.source_id("m").unwrap()]
    );
    assert!(check.sample_result().is_none());
    let wire: Value = serde_json::from_str(&check.to_json()).unwrap();
    assert_eq!(wire["version"], "1.0");
}

#[test]
fn required_contributors_have_exact_origins_and_unused_refs_do_not_enter_scope() {
    let root = br##"{"namespace":{"d":"https://example.com/data"},"sdfProperty":{"p":{"sdfRef":"d:#/sdfData/D","maximum":80,"properties":{"unused":{"sdfRef":"missing:#/sdfData/X"}},"sdfRequired":["missing:#/sdfData/X"]},"other":{"sdfRef":"missing:#/sdfData/X"}}}"##;
    let contributor = br##"{"namespace":{"d":"https://example.com/data"},"defaultNamespace":"d","sdfData":{"D":{"type":"number","minimum":0,"maximum":100}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("root", root),
        SourceInput::supplied("data", contributor),
        SourceInput::supplied("s", b"81").sample(),
        SourceInput::supplied("unused", br#"{"sdfProperty":{"bad":{"type":"invalid"}}}"#),
    ])
    .unwrap();
    let root_id = catalogue.source_id("root").unwrap();
    let data_id = catalogue.source_id("data").unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(root_id, SampleKind::Property, "/sdfProperty/p")
                .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().errors(), 1);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["request"]["scope"]["roots"],
        serde_json::json!([root_id.get()])
    );
    assert_eq!(
        wire["request"]["scope"]["dependencies"],
        serde_json::json!([data_id.get()])
    );
    let explanation = wire["result"]["explanations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|explanation| explanation["effective"]["pointer"] == "/sdfProperty/p/minimum")
        .unwrap();
    assert_eq!(explanation["sources"][0]["source_id"], data_id.get());
    assert_eq!(explanation["sources"][0]["pointer"], "/sdfData/D/minimum");
    assert_eq!(explanation["consumer"]["source_id"], root_id.get());
    assert_eq!(explanation["consumer"]["pointer"], "/sdfProperty/p/sdfRef");
    assert!(matches!(
        report
            .sample_result()
            .unwrap()
            .description()
            .require()
            .unwrap()
            .json()
            .get("properties"),
        Field::Unavailable(_)
    ));
}

#[test]
fn leaf_capabilities_outside_the_needed_context_remain_source_observable() {
    let model = br#"{"sdfProperty":{"p":{"type":"number","maximum":100,"description":"\uD800","default":1234567,"properties":{"unused":{"maximum":1234567}}}}}"#;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", b"101").sample(),
        ],
        Limits::default().number_token_bytes(3),
    )
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(
        report.sample_result().unwrap().context_findings().errors(),
        0
    );
    assert!(matches!(
        catalogue
            .source(catalogue.source_id("m").unwrap())
            .unwrap()
            .json()
            .require()
            .unwrap()
            .get("sdfProperty")
            .require()
            .unwrap()
            .get("p")
            .require()
            .unwrap()
            .get("description")
            .require()
            .unwrap()
            .text(),
        Field::Unavailable(_)
    ));
}

#[test]
fn needed_model_findings_are_not_fabricated_sample_mismatches() {
    for model in [
        br#"{"sdfProperty":{"p":{"type":"number","maximum":"bad"}}}"#.as_slice(),
        br#"{"sdfProperty":{"p":{"sdfRef":"missing:#/sdfData/D","maximum":100}}}"#.as_slice(),
        br#"{"sdfProperty":{"p":{"type":"number","multipleOf":0}}}"#.as_slice(),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", b"101").sample(),
        ])
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
        assert!(
            report.sample_result().unwrap().context_findings().errors() > 0,
            "{}",
            std::str::from_utf8(model).unwrap()
        );
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(
                    |condition| condition.domain() == Some(ConditionDomain::Sample)
                        && condition.severity() == esdieff::Severity::Error
                )
        );
    }
}

#[test]
fn unresolved_reference_meaning_is_unfinished_without_a_definite_model_error() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "m",
            br#"{"sdfProperty":{"p":{"sdfRef":"invalid","maximum":100}}}"#,
        ),
        SourceInput::supplied("s", b"101").sample(),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(
        report.sample_result().unwrap().context_findings().errors(),
        0
    );
}

#[test]
fn admitted_null_explains_the_established_nullable_default_without_a_source_span() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "m",
            br#"{"sdfProperty":{"p":{"type":"number","maximum":100}}}"#,
        ),
        SourceInput::supplied("s", b"null").sample(),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let default = wire["result"]["explanations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["kind"] == "default" && row["effective"]["pointer"] == "/sdfProperty/p/nullable"
        })
        .expect("nullable default explanation");
    assert_eq!(default["sources"], serde_json::json!([]));
    assert_eq!(default["default_value"], true);
}

#[test]
fn ordinary_maps_and_arrays_have_their_own_assessed_coverage() {
    for (kind, sample, area, admission, status, complete) in [
        (
            "object",
            b"{}".as_slice(),
            "sample-map/1",
            Admission::Admitted,
            esdieff::AssessmentStatus::Assessed,
            true,
        ),
        (
            "array",
            b"[]".as_slice(),
            "sample-array/1",
            Admission::Admitted,
            esdieff::AssessmentStatus::Assessed,
            true,
        ),
    ] {
        let model = format!(r#"{{"sdfProperty":{{"p":{{"type":"{kind}"}}}}}}"#);
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(report.sample_result().unwrap().admission(), admission);
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-type-null/1")
                .require()
                .unwrap(),
            status
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status(area)
                .require()
                .unwrap(),
            status
        );
        assert_eq!(report.outcome().complete(), complete);
    }
}
