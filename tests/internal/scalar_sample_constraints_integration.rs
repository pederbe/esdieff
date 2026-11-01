//! Constant and alternative conclusions retain bounds, source identity and report agreement.
//! Follows ADR0015: Bounded value equality in a resolved data context,
//! ADR0016: Selected samples with explicit admission and coverage,
//! ADR0024: Effective alternatives and nullable characteristics,
//! and ADR0025: Selected numeric samples.
use esdieff::{
    Admission, Assessment, AssessmentStatus, Catalogue, CheckRequest, Class, EffectiveRequest,
    Field, JsonBuilder, Limits, Lookup, Report, SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;
fn selected(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap()
}
fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}
#[test]
fn numeric_acquisition_and_all_alternative_comparisons_share_one_remainder() {
    for (qualities,limit,expected) in [
        (r#"{"const":1}"#.to_owned(),3,Admission::Undetermined),
        (r#"{"const":1}"#.to_owned(),30,Admission::Rejected),
        (format!(r#"{{"type":"number","sdfChoice":{{{}}}}}"#,(0..10).map(|index|format!(r#""a{index}":{{"const":1}}"#)).collect::<Vec<_>>().join(",")),25,Admission::Undetermined),
        (r#"{"type":"number","sdfChoice":{"unavailableComparison":{"const":1},"unconstrained":{}}}"#.to_owned(),3,Admission::Admitted),
    ]{
        let model=format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
        let catalogue=Catalogue::with_limits(&[SourceInput::supplied("m",model.as_bytes()),SourceInput::supplied("s",b"2").sample()],Limits::default().numeric_work(limit)).unwrap();
        let first=selected(&catalogue);
        assert_eq!(first.sample_result().unwrap().admission(),expected,"{qualities}");
        assert_eq!(first.outcome().errors(),u32::from(expected==Admission::Rejected));
        assert_eq!(selected(&catalogue).to_json(),first.to_json());
    }
}
#[test]
fn inherited_branch_bounds_overrides_and_reference_deletions_keep_their_sources() {
    let model=br##"{"info":{},"sdfData":{"Branch":{"maximum":20},"Base":{"type":"number","maximum":5,"const":5,"default":10,"sdfChoice":{"shared":{},"changed":{"sdfRef":"#/sdfData/Branch","maximum":null,"const":10}}}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Base"}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", b"10").sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(report.outcome().complete());
    let wire = wire(&report);
    let items = wire["diagnostics"]["items"].as_array().unwrap();
    assert!(items.iter().any(|condition| {
        condition["code"] == "sample.bound"
            && condition["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|location| {
                    location["kind"] == "source" && location["pointer"] == "/sdfData/Base/maximum"
                })
    }));
    assert!(items.iter().any(|condition| {
        condition["code"] == "sample.const"
            && condition["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|location| {
                    location["kind"] == "source" && location["pointer"] == "/sdfData/Base/const"
                })
    }));
    assert!(
        wire["result"]["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["alternative"]["pointer"]
                .as_str()
                .unwrap()
                .contains("/sdfChoice/"))
    );
    assert_eq!(
        catalogue
            .source(catalogue.source_id("m").unwrap())
            .unwrap()
            .raw_bytes(),
        model
    );
}
#[test]
fn enum_occurrences_and_opaque_constant_members_keep_their_original_meaning() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"enum":["on","off","on"]}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""on""#).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    let result = report.sample_result().unwrap();
    assert_eq!(result.admission(), Admission::Admitted);
    assert!(matches!(
        result.description().require().unwrap().data_type(),
        Field::Absent
    ));
    assert_eq!(
        result
            .description()
            .require()
            .unwrap()
            .alternatives()
            .require()
            .unwrap()
            .len(),
        3
    );
    let output = wire(&report);
    assert_eq!(
        output["result"]["witnesses"][0]["alternative"]["pointer"],
        "/sdfProperty/p/enum/0"
    );
    assert!(
        output["result"]["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|explanation| explanation["sources"]
                .as_array()
                .is_some_and(|sources| sources
                    .iter()
                    .any(|source| source["pointer"] == "/sdfProperty/p/enum/0")))
    );
    let opaque=br##"{"info":{},"sdfProperty":{"p":{"const":{"sdfRef":"missing:#/x","type":"vendor","enum":["x"],"sdfChoice":{"notVocabulary":false}}}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", opaque),
        SourceInput::supplied("s", b"{}").sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(
        wire(&report)["request"]["scope"]["dependencies"],
        serde_json::json!([])
    );
    assert_eq!(
        catalogue
            .source(catalogue.source_id("m").unwrap())
            .unwrap()
            .raw_bytes(),
        opaque
    );
}
#[test]
fn original_and_typed_values_keep_conclusions_counts_and_locations_under_omission() {
    let model=br#"{"info":{},"sdfProperty":{"p":{"type":"number","sdfChoice":{"first":{"const":1},"second":{"const":2}}}}}"#;
    for typed in [false, true] {
        let mut counts = Vec::new();
        for limits in [
            Limits::default(),
            Limits::default().conditions(1),
            Limits::default().diagnostic_bytes(1),
            Limits::default().pointer_bytes(1).pointer_output_bytes(1),
            Limits::default().report_bytes(12 + 2 * 4096 + 16384 + 163930),
        ] {
            let input = if typed {
                SourceInput::typed("s", JsonBuilder::new().number("3.0").unwrap())
            } else {
                SourceInput::supplied("s", b"3.0").sample()
            };
            let catalogue = Catalogue::take_inputs_with_limits(
                vec![SourceInput::supplied("m", model), input],
                limits,
            )
            .unwrap();
            let report = selected(&catalogue);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert!(report.outcome().complete());
            assert_eq!(report.outcome().errors(), 2);
            assert_eq!(
                report
                    .sample_result()
                    .unwrap()
                    .area_status("sample-alternatives/1")
                    .require()
                    .unwrap(),
                AssessmentStatus::Assessed
            );
            let wire = wire(&report);
            assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
            assert_eq!(wire["coverage"]["requested"]["total"], 9);
            assert_eq!(
                wire["result"]["sample_location"]["span"],
                if typed {
                    Value::Null
                } else {
                    serde_json::json!([0, 3])
                }
            );
            counts.push(
                wire["diagnostics"]["retained"].as_u64().unwrap()
                    + wire["diagnostics"]["omitted"].as_u64().unwrap(),
            );
            for condition in wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["domain"] == "sample")
            {
                assert_eq!(condition["code"], "sample.const");
                assert_eq!(condition["basis"]["citation"], "RFC 9880 Appendix C");
                assert_eq!(condition["primary"], wire["result"]["sample_location"]);
            }
            assert!(report.to_human().contains("rejected"));
        }
        assert!(counts.iter().all(|count| *count == counts[0]));
    }
}
#[test]
fn unavailable_fixed_work_and_derived_storage_do_not_make_alternatives_valid() {
    let branches = (0..40)
        .map(|index| format!(r#""a{index}":{{"const":{index}}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let model = format!(
        r#"{{"info":{{}},"sdfProperty":{{"p":{{"type":"number","sdfChoice":{{{branches}}}}}}}}}"#
    );
    for limits in [
        Limits::default().decoding_work(6000),
        Limits::default().effective_allocation_bytes(2000),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", b"99").sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
        let wire = wire(&report);
        assert_eq!(wire["request"]["selector"]["pointer"], "/sdfProperty/p");
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
    }
}
#[test]
fn a_sample_result_does_not_change_independently_completed_model_work() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"type":"boolean","const":true}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", b"true").sample(),
    ])
    .unwrap();
    let full = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let before = full.to_json();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert!(report.outcome().complete());
    assert!(matches!(report.unit_guidance(), Field::Absent));
    assert!(full.outcome().complete());
    assert_eq!(full.requested_coverage().total(), Some(13));
    assert_eq!(full.to_json(), before);
}
#[test]
fn inherited_direct_item_uncertainty_requires_each_surviving_quality_override() {
    for (branch, established) in [
        (r#"{}"#, false),
        (r#"{"type":"number","nullable":true}"#, false),
        (r#"{"type":"number","nullable":true,"const":1}"#, true),
    ] {
        let model = format!(
            r##"{{"info":{{}},"sdfData":{{"Rows":{{"type":"array","items":{{"type":"array","nullable":false,"const":1,"sdfChoice":{{"chosen":{branch}}}}}}}}},"sdfProperty":{{"p":{{"sdfRef":"#/sdfData/Rows/items"}}}}}}"##
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", b"1").sample(),
        ])
        .unwrap();
        let effective = catalogue
            .effective(EffectiveRequest::new(catalogue.source_id("m").unwrap()))
            .unwrap();
        let Lookup::Found(rows) = effective.definition(Class::Data, "Rows") else {
            panic!("Rows data definition")
        };
        let (_, branch) = rows
            .data_description()
            .require()
            .unwrap()
            .items()
            .require()
            .unwrap()
            .alternatives()
            .require()
            .unwrap()
            .next()
            .unwrap();
        let branch = branch.require().unwrap();
        assert_eq!(matches!(branch.constant(), Field::Present(_)), established);
        assert!(SampleRequest::from_description(branch).is_err());
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.message() == "reference target role unavailable")
        );
        assert_eq!(report.outcome().errors(), 0);
    }
}
