//! Selector, parser and reporting bounds follow ADR0025: Selected numeric samples.
mod coverage_support;
use esdieff::{Admission, Catalogue, JsonBuilder, Limits, SampleKind, SampleRequest, SourceInput};
use serde_json::Value;
const MODEL: &[u8] = br#"{"sdfProperty":{"p":{"type":"number","maximum":100}}}"#;

#[test]
fn mandatory_sample_reporting_is_admitted_before_source_acquisition() {
    let result = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", MODEL),
            SourceInput::file("s", std::path::Path::new("/unavailable-sample-input")).sample(),
        ],
        Limits::default().report_bytes(50_000),
    );
    assert_eq!(
        result.err(),
        Some("report cannot retain its mandatory envelope")
    );
    assert!(
        Catalogue::with_limits(
            &[
                SourceInput::supplied("m", MODEL),
                SourceInput::supplied("other", b"{}"),
                SourceInput::supplied("s", b"101").sample(),
            ],
            Limits::default().documents(2)
        )
        .is_err()
    );
}

#[test]
fn semantic_storage_exhaustion_is_a_domain_result_with_a_retained_selector() {
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", MODEL),
            SourceInput::supplied("s", b"101").sample(),
        ],
        Limits::default().effective_allocation_bytes(1),
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
        .expect("valid selector with unfinished derived work");
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert_eq!(
        report.sample_result().unwrap().selected_pointer(),
        "/sdfProperty/p"
    );
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["request"]["selector"]["pointer"], "/sdfProperty/p");
    assert_eq!(wire["result"]["description"], Value::Null);
}

#[test]
fn typed_projection_parsing_still_spends_the_joint_numeric_allowance() {
    let builder = JsonBuilder::new();
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![
            SourceInput::supplied("m", MODEL),
            SourceInput::typed("s", builder.number("1").unwrap()),
        ],
        Limits::default().numeric_work(4),
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
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .sample_value()
            .require()
            .unwrap()
            .span(),
        None
    );
}

#[test]
fn maximal_escaped_action_selector_keeps_the_full_mandatory_selector() {
    let name = "\u{0001}".repeat(8192 - "/sdfAction/".len());
    let pointer = format!("/sdfAction/{name}");
    assert_eq!(pointer.len(), 8192);
    let model =
        serde_json::json!({"sdfAction":{name:{"sdfOutputData":{"type":"number","maximum":100}}}})
            .to_string();
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", b"101").sample(),
        ],
        Limits::default().report_bytes(200_000).diagnostic_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::ActionOutput,
                &pointer,
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(report.outcome().complete());
    let output = report.to_json();
    assert!(output.len() <= 200_000);
    let wire: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(wire["request"]["selector"]["pointer"], pointer);
    assert_eq!(wire["result"]["selector"]["pointer"], pointer);
    assert_eq!(
        wire["result"]["description"]["root"],
        catalogue.source_id("m").unwrap().get()
    );
    assert_eq!(wire["result"]["description"]["pointer"], Value::Null);
    assert_eq!(report.outcome().errors(), 1);
}

#[test]
fn three_maximal_selector_copies_are_reserved_and_optional_witnesses_are_bounded() {
    let name = "\u{0001}".repeat(8192 - "/sdfProperty/".len());
    let pointer = format!("/sdfProperty/{name}");
    let model = serde_json::json!({"sdfProperty":{name.clone():{"type":"number","maximum":100}}})
        .to_string();
    let inputs = [
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", b"1").sample(),
    ];
    assert_eq!(
        Catalogue::with_limits(&inputs, Limits::default().report_bytes(150_000)).err(),
        Some("report cannot retain its mandatory envelope")
    );
    for (limits, cap) in [
        (Limits::default(), 8 * 1024 * 1024),
        (
            Limits::default().report_bytes(200_000).diagnostic_bytes(1),
            200_000,
        ),
    ] {
        let catalogue = Catalogue::with_limits(&inputs, limits).unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    &pointer,
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        assert!(report.outcome().complete());
        let output = report.to_json();
        assert!(output.len() <= cap);
        let wire: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(wire["request"]["selector"]["pointer"], pointer);
        assert_eq!(wire["result"]["selector"]["pointer"], pointer);
        assert_eq!(wire["result"]["description"]["pointer"], pointer);
    }
}

#[test]
fn maximal_property_report_fits_its_exact_admitted_envelope_with_three_pointers() {
    let name = "\u{0001}".repeat(8192 - "/sdfProperty/".len());
    let pointer = format!("/sdfProperty/{name}");
    let model =
        serde_json::json!({"sdfProperty":{name:{"type":"number","maximum":100}}}).to_string();
    let inputs = [
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", b"101").sample(),
    ];
    // Two one-byte keys, two common source records, the common fixed envelope and the sample envelope.
    const CAP: usize = 12 + 2 * 4096 + 16384 + 163930;
    assert_eq!(
        Catalogue::with_limits(&inputs, Limits::default().report_bytes(CAP - 1)).err(),
        Some("report cannot retain its mandatory envelope")
    );
    let catalogue = Catalogue::with_limits(&inputs, Limits::default().report_bytes(CAP)).unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                &pointer,
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
    let output = report.to_json();
    assert!(output.len() <= CAP);
    let wire: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(wire["request"]["selector"]["pointer"], pointer);
    assert_eq!(wire["result"]["selector"]["pointer"], pointer);
    assert_eq!(wire["result"]["description"]["pointer"], pointer);
    assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() > 0);
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    assert_eq!(
        wire["result"]["sample_location"]["span"],
        serde_json::json!([0, 3])
    );
}

#[test]
fn semantic_limits_preserve_domain_causes_and_known_input_counts() {
    let chain = br##"{"sdfData":{"a":{"sdfRef":"#/sdfData/b"},"b":{"sdfRef":"#/sdfData/c"},"c":{"type":"number","maximum":100}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/a"}}}"##;
    for (limits, model) in [
        (Limits::default().decoding_work(1), MODEL),
        (Limits::default().effective_nodes(1), MODEL),
        (
            Limits::default().reference_applications(1),
            chain.as_slice(),
        ),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model),
                SourceInput::supplied("s", b"101").sample(),
            ],
            limits,
        )
        .unwrap();
        let request = || {
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap())
        };
        let report = catalogue
            .check_sample(request())
            .expect("valid bounded request");
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(
            report.sample_result().unwrap().selected_pointer(),
            "/sdfProperty/p"
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "operation.limit")
        );
        assert!(
            catalogue
                .check_sample(
                    SampleRequest::new(
                        catalogue.source_id("m").unwrap(),
                        SampleKind::Property,
                        "/bad~3"
                    )
                    .sample(catalogue.source_id("s").unwrap())
                )
                .is_err()
        );
    }
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", MODEL),
            SourceInput::supplied("s", br#"{"a":1,"a":2}"#).sample(),
        ],
        Limits::default()
            .effective_allocation_bytes(1)
            .diagnostic_bytes(1),
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
    assert_eq!(report.outcome().errors(), 1);
    assert!(!report.outcome().complete());
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    coverage_support::assert_wire_counts(&wire);
    assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() > 0);
}

#[test]
fn handle_identity_ignores_presentation_and_old_owner_storage_allowances() {
    use esdieff::{Class, EffectiveRequest, Field, Lookup};
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", MODEL),
            SourceInput::supplied("s", b"101").sample(),
        ],
        Limits::default().pointer_output_bytes(1),
    )
    .unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("m").unwrap()))
        .unwrap();
    let Lookup::Found(property) = model.definition(Class::Property, "p") else {
        panic!("Property p");
    };
    let handle = property.data_description().require().unwrap();
    assert!(matches!(
        handle.json().location().pointer(8192),
        Field::Unavailable(_)
    ));
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
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["result"]["description"]["pointer"], Value::Null);

    let name = "p".repeat(1024);
    let pointer = format!("/sdfProperty/{name}");
    let source = serde_json::json!({"sdfProperty":{name.clone():{"type":"number","maximum":100}}})
        .to_string();
    let mut witnessed = false;
    for allocation in (2048..=16384).step_by(128) {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", source.as_bytes()),
                SourceInput::supplied("s", b"101").sample(),
            ],
            Limits::default().effective_allocation_bytes(allocation),
        )
        .unwrap();
        let model = catalogue
            .effective(EffectiveRequest::new(catalogue.source_id("m").unwrap()))
            .unwrap();
        let Lookup::Found(property) = model.definition(Class::Property, &name) else {
            continue;
        };
        let Some(handle) = property.data_description().present() else {
            continue;
        };
        if !matches!(
            handle.json().location().pointer(8192),
            Field::Unavailable(_)
        ) {
            continue;
        }
        let from_pointer = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    &pointer,
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        let from_handle = catalogue
            .check_sample(
                SampleRequest::from_description(handle)
                    .unwrap()
                    .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            from_pointer.sample_result().unwrap().admission(),
            from_handle.sample_result().unwrap().admission()
        );
        assert_eq!(
            from_pointer.outcome().complete(),
            from_handle.outcome().complete()
        );
        assert_eq!(
            from_handle.sample_result().unwrap().selected_pointer(),
            pointer
        );
        witnessed = true;
        break;
    }
    assert!(
        witnessed,
        "an available handle with insufficient old-owner pointer storage"
    );
}
