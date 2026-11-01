// Ordinary map samples follow ADR0016: Selected samples with explicit admission and coverage.
use super::map_support;
use esdieff::{
    Admission, AssessmentStatus, Catalogue, CheckRequest, JsonBuilder, Limits, Report, SampleKind,
    SampleRequest, SourceInput,
};
use map_support::selected;
use serde_json::Value;

fn supplied(qualities: &str, sample: &str) -> Catalogue {
    let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
    Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", sample.as_bytes()).sample(),
    ])
    .unwrap()
}
fn expect(report: &Report<'_>, admission: Admission, complete: bool) -> Value {
    assert_eq!(report.sample_result().unwrap().admission(), admission);
    assert_eq!(report.outcome().complete(), complete);
    let coverage = report.requested_coverage();
    assert_eq!(coverage.total(), Some(9));
    assert_eq!(
        coverage.assessed()
            + coverage.not_applicable()
            + coverage.unassessed()
            + coverage.undetermined(),
        9
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["request"]["assessment"], "esdieff.sample/1");
    assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
    assert_eq!(wire["result"]["admission"], admission.as_str());
    assert_eq!(wire["outcome"]["complete"], complete);
    let human = report.to_human();
    assert!(human.contains(&format!(
        "Sample {}: {}",
        report.sample_result().unwrap().sample_id().get(),
        admission.as_str()
    )));
    for row in wire["coverage"]["rows"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let status = report.assessment_area_status(id).require().unwrap();
        assert_eq!(row["status"], status.as_str());
        assert!(human.contains(&format!("Sample area {id}: {}", status.as_str())));
    }
    wire
}

#[test]
fn action_required_presence_is_distinct_from_null_and_default() {
    let model = br#"{"sdfAction":{"configure":{"sdfInputData":{"type":"object","properties":{"mode":{"type":"string","nullable":false,"default":"on"}},"required":["mode"]}}}}"#;
    for (sample, expected) in [
        ("{}", Admission::Rejected),
        (r#"{"mode":null}"#, Admission::Rejected),
        (r#"{"mode":"on"}"#, Admission::Admitted),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::ActionInput,
                    "/sdfAction/configure",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        expect(&report, expected, true);
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .sample_value()
                .require()
                .unwrap()
                .raw_json(),
            sample
        );
        if sample == "{}" {
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            let condition = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["domain"] == "sample" && row["severity"] == "error")
                .unwrap();
            assert_eq!(condition["primary"]["pointer"], "");
            assert_eq!(condition["primary"]["span"], serde_json::json!([0, 2]));
            assert!(
                condition["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["kind"] == "effective"
                        && row["pointer"] == "/sdfAction/configure/sdfInputData/required/0")
            );
        }
    }
}

#[test]
fn optional_absence_does_not_request_a_nonexistent_child_value() {
    for child in [
        r#"{"type":"array","items":{"type":"number"}}"#,
        r#"{"type":"string","format":"date"}"#,
        r#"{"sdfRef":"missing:#/sdfData/Unknown"}"#,
        r#"{"type":"number","multipleOf":0}"#,
    ] {
        let qualities = format!(r#"{{"type":"object","properties":{{"optional":{child}}}}}"#);
        let catalogue = supplied(&qualities, "{}");
        expect(&selected(&catalogue), Admission::Admitted, true);
    }
}

#[test]
fn open_entries_and_required_without_description_are_supported() {
    for (qualities, sample, expected) in [
        (
            r#"{"type":"object","properties":{"mode":{"type":"string"}}}"#,
            r#"{"mode":"on","vendor":{"sdfRef":"opaque","x":[1]}}"#,
            Admission::Admitted,
        ),
        (
            r#"{"type":"object","properties":{}}"#,
            r#"{"sdfRequiredInputData":1,"minimun":2}"#,
            Admission::Admitted,
        ),
        (
            r#"{"type":"object","required":["token"]}"#,
            "{}",
            Admission::Rejected,
        ),
        (
            r#"{"type":"object","required":["token"]}"#,
            r#"{"token":{"opaque":[1]}}"#,
            Admission::Admitted,
        ),
    ] {
        let catalogue = supplied(qualities, sample);
        expect(&selected(&catalogue), expected, true);
    }
}

#[test]
fn missing_presence_is_independent_of_another_unknown_child() {
    let catalogue = supplied(
        r#"{"type":"object","properties":{"mode":{"type":"string"},"pending":{"type":"array","items":{"type":"string","format":"future-format"}}},"required":["mode"]}"#,
        r#"{"pending":["not-a-date"]}"#,
    );
    let report = selected(&catalogue);
    expect(&report, Admission::Rejected, false);
    assert!(report.outcome().errors() > 0);
    assert!(matches!(
        report
            .assessment_area_status("sample-array/1")
            .require()
            .unwrap(),
        AssessmentStatus::Unassessed | AssessmentStatus::Undetermined
    ));
}

#[test]
fn scalar_and_nested_map_children_share_exact_existing_constraints() {
    let qualities = r#"{"type":"object","properties":{"level":{"type":"number","maximum":9007199254740992},"nested":{"type":"object","properties":{"flag":{"type":"boolean"},"text":{"type":"string","minLength":2}},"required":["flag","text"]}},"required":["level","nested"]}"#;
    for (sample, expected) in [
        (
            r#"{"level":9007199254740992,"nested":{"flag":true,"text":"é𝄞"}}"#,
            Admission::Admitted,
        ),
        (
            r#"{"level":9007199254740993,"nested":{"flag":true,"text":"é𝄞"}}"#,
            Admission::Rejected,
        ),
        (
            r#"{"level":0,"nested":{"flag":true,"text":"é"}}"#,
            Admission::Rejected,
        ),
        (
            r#"{"level":0,"nested":{"flag":0,"text":"ok"}}"#,
            Admission::Rejected,
        ),
    ] {
        let catalogue = supplied(qualities, sample);
        expect(&selected(&catalogue), expected, true);
    }
}

#[test]
fn child_choice_admission_returns_to_its_containing_map() {
    let qualities = r#"{"type":"object","properties":{"x":{"sdfChoice":{"yes":{"const":true},"no":{"const":false}}},"y":{"type":"string"}},"required":["x","y"]}"#;
    let catalogue = supplied(qualities, r#"{"x":true}"#);
    expect(&selected(&catalogue), Admission::Rejected, true);
}

#[test]
fn different_map_branches_cannot_supply_different_child_witnesses() {
    let qualities = r#"{"type":"object","sdfChoice":{"a":{"properties":{"x":{"const":1},"y":{"const":2}},"required":["x","y"]},"b":{"properties":{"x":{"const":2},"y":{"const":1}},"required":["x","y"]}}}"#;
    let catalogue = supplied(qualities, r#"{"x":1,"y":1}"#);
    expect(&selected(&catalogue), Admission::Rejected, true);
}

#[test]
fn a_whole_known_map_witness_and_a_potentially_matching_unknown_are_distinct() {
    let qualities = r#"{"type":"object","sdfChoice":{"known":{"properties":{"x":{"type":"number","maximum":1}},"required":["x"]},"unknown":{"sdfRef":false}}}"#;
    let known = supplied(qualities, r#"{"x":1}"#);
    expect(&selected(&known), Admission::Admitted, true);
    let unknown = supplied(qualities, r#"{"x":2}"#);
    let report = selected(&unknown);
    expect(&report, Admission::Undetermined, false);
    assert_eq!(report.outcome().errors(), 0);
}

#[test]
fn branch_properties_override_precedes_child_reference_resolution() {
    let qualities = r#"{"type":"object","properties":{"discarded":{"sdfRef":"missing:#/sdfData/Unknown"}},"sdfChoice":{"replacement":{"properties":{"good":{"type":"number"}},"required":["good"]}}}"#;
    let catalogue = supplied(qualities, r#"{"good":1,"discarded":2}"#);
    let report = selected(&catalogue);
    expect(&report, Admission::Admitted, true);
    assert_eq!(report.outcome().errors(), 0);
}

#[test]
fn present_text_does_not_request_numerical_parameter_rules() {
    let catalogue = supplied(
        r#"{"type":"object","properties":{"text":{"type":"string","maximum":"bad"},"absent":{"type":"number","multipleOf":0}}}"#,
        r#"{"text":"ok"}"#,
    );
    expect(&selected(&catalogue), Admission::Admitted, true);
    let needed = supplied(
        r#"{"type":"object","properties":{"n":{"type":"number","maximum":"bad"}}}"#,
        r#"{"n":1}"#,
    );
    let report = selected(&needed);
    expect(&report, Admission::Undetermined, false);
    assert!(report.outcome().errors() > 0);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["domain"] == "model" && row["severity"] == "error")
    );
}

#[test]
fn unknown_child_and_unknown_enclosing_structure_have_different_presence_guards() {
    // `title` is a base quality outside the role of a data description.
    let child = supplied(
        r#"{"type":"object","properties":{"pending":{"title":"t"}},"required":["missing"]}"#,
        r#"{"pending":1}"#,
    );
    expect(&selected(&child), Admission::Rejected, false);
    let outer = supplied(
        r#"{"type":"object","title":"t","required":["missing"]}"#,
        "{}",
    );
    let report = selected(&outer);
    expect(&report, Admission::Undetermined, false);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    // Undeclared extension vocabulary leaves both structures known, following
    // ADR 0037: Undeclared extension vocabulary.
    let child = supplied(
        r#"{"type":"object","properties":{"pending":{"x:meaning":1}},"required":["missing"]}"#,
        r#"{"pending":1}"#,
    );
    expect(&selected(&child), Admission::Rejected, true);
    let outer = supplied(
        r#"{"type":"object","x:meaning":1,"required":["missing"]}"#,
        "{}",
    );
    expect(&selected(&outer), Admission::Rejected, true);
}

#[test]
fn absent_child_structure_stays_unavailable_while_ordinary_constants_compare() {
    for (qualities, sample, admission, complete) in [
        (
            r#"{"properties":{"x":{"type":"number"}}}"#,
            r#"{"x":1}"#,
            Admission::Undetermined,
            false,
        ),
        (
            r#"{"required":["x"]}"#,
            "{}",
            Admission::Undetermined,
            false,
        ),
        (
            r#"{"type":"object","const":{"x":1},"properties":{"x":{"type":"number"}}}"#,
            r#"{"x":1}"#,
            Admission::Admitted,
            true,
        ),
    ] {
        let catalogue = supplied(qualities, sample);
        expect(&selected(&catalogue), admission, complete);
    }
}

#[test]
fn original_and_typed_entry_locations_have_only_real_spans() {
    let model = br#"{"sdfProperty":{"p":{"type":"object","properties":{"a/b~c":{"type":"number","maximum":1}},"required":["a/b~c"]}}}"#;
    let builder = JsonBuilder::new();
    for input in [
        SourceInput::supplied("s", br#"{"a/b~c":2}"#).sample(),
        SourceInput::typed(
            "s",
            builder
                .map(vec![("a/b~c", builder.number("2").unwrap())])
                .unwrap(),
        ),
    ] {
        let catalogue =
            Catalogue::take_inputs(vec![SourceInput::supplied("m", model), input]).unwrap();
        let report = selected(&catalogue);
        let wire = expect(&report, Admission::Rejected, true);
        let sample = catalogue.source(catalogue.source_id("s").unwrap()).unwrap();
        let value = sample
            .sample_value()
            .require()
            .unwrap()
            .get("a/b~c")
            .require()
            .unwrap();
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["domain"] == "sample" && row["severity"] == "error")
            .unwrap();
        assert_eq!(finding["primary"]["pointer"], "/a~1b~0c");
        assert_eq!(
            finding["primary"]["span"],
            value.span().map_or(Value::Null, |span| serde_json::json!([
                span.start, span.end
            ]))
        );
        assert_eq!(sample.has_original_bytes(), value.has_original_bytes());
    }
}

#[test]
fn given_and_payload_names_do_not_trigger_the_legacy_restriction() {
    let model = br#"{"sdfProperty":{"sdfRequiredInputData":{"type":"object","properties":{"sdfRequiredInputData":{"type":"boolean"}},"required":["sdfRequiredInputData"]}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#"{"sdfRequiredInputData":true}"#).sample(),
    ])
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/sdfRequiredInputData",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    expect(&report, Admission::Admitted, true);
    assert!(!report.to_json().contains("policy.historical_action_input"));
    assert!(
        !catalogue
            .check(CheckRequest::new())
            .unwrap()
            .to_json()
            .contains("policy.historical_action_input")
    );
}

#[test]
fn actual_legacy_vocabulary_keeps_its_current_restructuring_guidance() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "m",
        br#"{"sdfAction":{"configure":{"sdfRequiredInputData":{"mode":{"type":"string"}}}}}"#,
    )])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "policy.historical_action_input")
        .unwrap();
    assert_eq!(finding["basis"]["kind"], "acceptance");
    assert_eq!(finding["basis"]["id"], "A001");
    let human = report.to_human();
    for part in ["sdfInputData", "properties", "required"] {
        assert!(human.contains(part));
    }
}

#[test]
fn omitted_optional_detail_preserves_independent_nested_errors_and_statuses() {
    let model = br#"{"sdfProperty":{"p":{"type":"object","properties":{"a":{"type":"number","maximum":0},"b":{"type":"boolean"}}}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().diagnostic_bytes(1),
        Limits::default().conditions(1),
        Limits::default().pointer_bytes(1),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model),
                SourceInput::supplied("s", br#"{"a":1,"b":2}"#).sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue);
        expect(&report, Admission::Rejected, true);
        assert_eq!(report.outcome().errors(), 2);
    }
}
