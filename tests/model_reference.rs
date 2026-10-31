//! Model references through the public library boundary, following ADR 0050.
use esdieff::{Catalogue, Disposition, ReferenceRequest, SourceInput};

#[test]
fn selected_thermostat_documents_effective_constraints_and_absent_output() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "thermostat",
        include_bytes!("../examples/thermostat-check/rich.sdf.json"),
    )])
    .unwrap();
    let request = ReferenceRequest::new(
        catalogue.source_id("thermostat").unwrap(),
        "/sdfObject/Thermostat",
    );
    let result = catalogue.reference(request).unwrap();
    assert_eq!(result.disposition(), Disposition::Passed);
    let markdown = result.to_markdown();
    for text in [
        "Thermostat",
        "temperature",
        "multipleOf",
        "0\\.5",
        "configure",
        "alarm",
        "No data description",
        "RFC 9880",
        "Source index",
    ] {
        assert!(markdown.contains(text), "missing {text}: {markdown}");
    }
    assert!(markdown.ends_with('\n'));
    assert!(!markdown.contains('\x1b'));
}

fn reference(model: &str, pointer: &str) -> (Disposition, String) {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = catalogue
        .reference(ReferenceRequest::new(
            catalogue.source_id("model").unwrap(),
            pointer,
        ))
        .unwrap();
    (result.disposition(), result.to_markdown().to_owned())
}

#[test]
fn composed_context_preserves_refinements_deletions_and_origins() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "base",
            include_bytes!("../examples/context-reuse/base.sdf.json"),
        ),
        SourceInput::supplied(
            "device",
            include_bytes!("../examples/context-reuse/device.sdf.json"),
        ),
    ])
    .unwrap();
    let request = || {
        ReferenceRequest::new(
            catalogue.source_id("device").unwrap(),
            "/sdfThing/Installation",
        )
    };
    let result = catalogue.reference(request()).unwrap();
    let markdown = result.to_markdown();
    assert_eq!(result.disposition(), Disposition::Passed, "{markdown}");
    for text in [
        "Office",
        "A\\-17",
        "minimum: 2",
        "minimum: \\-10",
        "maximum: 5",
        "inherited",
        "override",
        "Importing consumer",
        "base",
        "device",
        "non-interactive metadata",
        "draft-ietf-asdf-sdf-nonaffordance-06",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
    assert!(!markdown.contains("retired"));
    assert!(!markdown.contains("Factory calibration"));
    assert_eq!(
        markdown,
        catalogue.reference(request()).unwrap().to_markdown()
    );
    // No Context becomes an endpoint and no assumed default is inserted.
}

#[test]
fn selection_errors_are_distinct_from_unavailable_selection() {
    let model = r#"{"sdfThing":{"Device":{"sdfProperty":{"p":{"type":"number"}}}}}"#;
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    for pointer in [
        "",
        "not-pointer",
        "/bad~2",
        "/sdfThing/missing",
        "/sdfThing/Device/sdfProperty/p",
    ] {
        assert!(
            catalogue
                .reference(ReferenceRequest::new(id, pointer))
                .is_err(),
            "{pointer}"
        );
    }
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", b"{")]).unwrap();
    let result = catalogue
        .reference(ReferenceRequest::new(
            catalogue.source_id("model").unwrap(),
            "/sdfThing/Device",
        ))
        .unwrap();
    assert_eq!(result.disposition(), Disposition::Failed);
    assert!(result.to_markdown().contains("Source 1"));
}

#[test]
fn unavailable_branch_keeps_sibling_and_deleted_branch_does_not_spoil_reference() {
    let model = r##"{"sdfThing":{"Device":{"sdfObject":{"broken":{"sdfRef":"#/sdfObject/Missing"},"available":{"sdfProperty":{"p":{"type":"integer","minimum":7}}}}}}}"##;
    let (disposition, markdown) = reference(model, "/sdfThing/Device");
    assert_ne!(disposition, Disposition::Passed);
    assert!(markdown.contains("available (sdfObject)"));
    assert!(markdown.contains("minimum: 7"));
    assert!(markdown.contains("unavailable"));
    let model = r##"{"sdfThing":{"Base":{"sdfObject":{"broken":{"sdfRef":"#/sdfObject/Missing"},"available":{}}},"Device":{"sdfRef":"#/sdfThing/Base","sdfObject":{"broken":null}}}}"##;
    let (disposition, markdown) = reference(model, "/sdfThing/Device");
    assert_eq!(disposition, Disposition::Passed, "{markdown}");
    assert!(!markdown.contains("Missing"));
    assert!(!markdown.contains("Base (sdfThing)"));
}

#[test]
fn limits_produce_whole_markdown_and_honest_omissions() {
    use esdieff::Limits;
    let model = include_bytes!("../examples/thermostat-check/rich.sdf.json");
    let large_model = String::from_utf8(model.to_vec()).unwrap().replace(
        "Measured room temperature, or null while the sensor is unavailable",
        &"x".repeat(60000),
    );
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", large_model.as_bytes())],
        Limits::default().report_bytes(32768),
    )
    .unwrap();
    let result = catalogue
        .reference(ReferenceRequest::new(
            catalogue.source_id("model").unwrap(),
            "/sdfObject/Thermostat",
        ))
        .unwrap();
    assert_eq!(result.disposition(), Disposition::Undetermined);
    assert!(result.to_markdown().len() <= 32768);
    assert!(result.to_markdown().ends_with('\n'));
    assert!(
        result
            .to_markdown()
            .contains("Processing conditions and omissions")
    );
    assert!(result.omissions().byte_limited() > 0);
    navigation(result.to_markdown());
}

fn navigation(markdown: &str) {
    for link in markdown.split("](#").skip(1) {
        let anchor = link.split(')').next().unwrap();
        assert!(
            markdown.contains(&format!("id=\"{anchor}\"")),
            "dangling {anchor}"
        );
    }
}

#[test]
fn supplied_content_is_inert_and_effective_positions_stay_distinct() {
    let model = r##"{"sdfThing":{"Device":{"label":"<script>evil()</script>","description":"[x](https://evil)\n![image](https://evil)\n``` | λ","sdfObject":{"one":{"sdfRef":"#/sdfObject/Base"},"two":{"sdfRef":"#/sdfObject/Base"}}}},"sdfObject":{"Base":{"sdfProperty":{"a/b~c":{"type":"number","minimum":9007199254740993,"maximum":9007199254740995,"extension":"<img src=x>"}}}}}"##;
    let (disposition, markdown) = reference(model, "/sdfThing/Device");
    assert_eq!(disposition, Disposition::Passed);
    assert!(markdown.contains("9007199254740993"));
    assert!(markdown.contains("λ"));
    assert!(!markdown.contains("<script>"));
    assert!(!markdown.contains("![image](https://evil)"));
    assert!(!markdown.contains("[x](https://evil)"));
    assert!(markdown.contains("meaning not established"));
    assert!(markdown.contains("/sdfObject/one/sdfProperty/a\\~1b\\~0c"));
    assert!(markdown.contains("/sdfObject/two/sdfProperty/a\\~1b\\~0c"));
    navigation(&markdown);
}

#[test]
fn nested_model_repetition_required_declarations_choice_and_reusable_data() {
    let model = r##"{"sdfThing":{"Root":{"sdfRequired":["child"],"sdfObject":{"child":{"minItems":1,"maxItems":3,"sdfRequired":["value"],"sdfData":{"Reusable":{"type":"integer","minimum":42}},"sdfProperty":{"value":{"type":"array","nullable":false,"minItems":1,"uniqueItems":true,"items":{"type":"object","required":["nested"],"properties":{"nested":{"type":"string","minLength":2}}}}},"sdfAction":{"call":{"sdfOutputData":{"type":"number","minimum":10,"nullable":false,"sdfChoice":{"low":{"maximum":20},"high":{"minimum":30}}}}}}}}}}"##;
    let (disposition, markdown) = reference(model, "/sdfThing/Root");
    assert_eq!(disposition, Disposition::Passed, "{markdown}");
    for text in [
        "minItems: 1 (grouping repetition)",
        "maxItems: 3 (grouping repetition)",
        "Required declaration relationship",
        "child (sdfObject)",
        "value (sdfProperty)",
        "Reusable (sdfData)",
        "minimum: 42",
        "uniqueItems: true",
        "Map entry nested: required",
        "## low",
        "## high",
        "maximum: 20",
        "minimum: 30",
        "Output (data description)",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
    let low = markdown
        .split("## low")
        .nth(1)
        .unwrap()
        .split("## high")
        .next()
        .unwrap();
    assert!(low.contains("minimum: 10"));
    assert!(low.contains("nullable: false"));
    navigation(&markdown);
}

#[test]
fn unknown_context_default_and_unsupported_role_meaning_are_local_documentation() {
    let (disposition, markdown) = reference(
        r#"{"sdfObject":{"Device":{"sdfContext":{"metadata":{"type":"string","format":"unknown-format"}},"sdfProperty":{"array":{"type":"array","items":{"type":"number","multipleOf":2}}}}}}"#,
        "/sdfObject/Device",
    );
    assert_eq!(disposition, Disposition::Passed);
    assert!(markdown.contains("Context nullable default is unresolved"));
    assert!(markdown.contains("multipleOf: 2 (declared content, meaning not established)"));
    let context = markdown
        .split("metadata (sdfContext)")
        .last()
        .unwrap()
        .split("## Processing")
        .next()
        .unwrap();
    assert!(!context.contains("- readable:"));
}

#[test]
fn every_displayed_example_is_independently_admitted_for_its_selector() {
    use esdieff::{Admission, SampleKind, SampleRequest};
    let model = include_bytes!("../examples/thermostat-check/rich.sdf.json");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let result = catalogue
        .reference(ReferenceRequest::new(
            catalogue.source_id("model").unwrap(),
            "/sdfObject/Thermostat",
        ))
        .unwrap();
    let mut count = 0;
    for section in result.to_markdown().split("## ") {
        let Some((_, example)) = section.split_once("```json\n") else {
            continue;
        };
        let value = example.split("\n```").next().unwrap();
        let pointer = section
            .split("Effective pointer: ")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        let (kind, pointer) = if let Some(pointer) = pointer.strip_suffix("/sdfInputData") {
            (SampleKind::ActionInput, pointer)
        } else if let Some(pointer) = pointer.strip_suffix("/sdfOutputData") {
            (SampleKind::EventOutput, pointer)
        } else {
            (SampleKind::Property, pointer)
        };
        let check = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model),
            SourceInput::supplied("value", value.as_bytes()).sample(),
        ])
        .unwrap();
        let result = check
            .check_sample(
                SampleRequest::new(check.source_id("model").unwrap(), kind, pointer)
                    .sample(check.source_id("value").unwrap()),
            )
            .unwrap();
        assert_eq!(
            result.sample_result().unwrap().admission(),
            Admission::Admitted,
            "{pointer}: {value}"
        );
        count += 1;
    }
    assert_eq!(count, 4);
}

#[test]
fn pattern_candidates_supply_admitted_examples_without_substituting_bad_defaults() {
    let (_, markdown) = reference(
        r#"{"sdfObject":{"Device":{"sdfProperty":{"pattern":{"type":"string","nullable":false,"pattern":"^z$","default":"bad"},"constant":{"type":"integer","nullable":false,"minimum":10,"default":1}}}}}"#,
        "/sdfObject/Device",
    );
    assert!(markdown.contains("documentary default"));
    let pattern = markdown
        .split("## pattern")
        .nth(1)
        .unwrap()
        .split("## constant")
        .next()
        .unwrap();
    assert!(pattern.contains("```json\n\"z\"\n```"));
    assert!(!pattern.contains("```json\n\"bad\"\n```"));
    let constant = markdown.split("## constant").nth(1).unwrap();
    assert!(!constant.contains("```json\n1\n```"));
}

#[test]
fn example_ceiling_is_global_and_leaves_remaining_constraints_visible() {
    use std::fmt::Write;
    let mut model = String::from(r#"{"sdfThing":{"Root":{"sdfObject":{"child":{"sdfProperty":{"#);
    for index in 0..1025 {
        if index > 0 {
            model.push(',');
        }
        write!(
            model,
            "\"p{index}\":{{\"type\":\"integer\",\"nullable\":false,\"minimum\":{index}}}"
        )
        .unwrap();
    }
    model.push_str("}}}}}}");
    let (disposition, markdown) = reference(&model, "/sdfThing/Root");
    assert_eq!(
        disposition,
        Disposition::Passed,
        "{}",
        &markdown[markdown.len().saturating_sub(800)..]
    );
    assert!(markdown.contains("Examples not searched"));
    assert!(markdown.contains("minimum: 1024"));
    assert_eq!(
        markdown
            .matches("Illustrative value admitted by the model")
            .count(),
        1024
    );
}

#[test]
fn low_work_pointer_and_impossible_report_limits_are_explicit() {
    use esdieff::Limits;
    let model = br#"{"sdfObject":{"Device":{"sdfProperty":{"p":{"type":"integer","minimum":1}}}}}"#;
    for limits in [
        Limits::default().decoding_work(1000),
        Limits::default().pointer_output_bytes(1),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", model)], limits).unwrap();
        let result = catalogue
            .reference(ReferenceRequest::new(
                catalogue.source_id("model").unwrap(),
                "/sdfObject/Device",
            ))
            .unwrap();
        assert_eq!(result.disposition(), Disposition::Undetermined);
        assert!(
            result.to_markdown().contains("unavailable")
                || result.to_markdown().contains("allowance")
        );
    }
    // Acquisition accepts this envelope, while the reference also reserves its identity.
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    assert!(
        catalogue
            .reference(ReferenceRequest::new(id, &format!("/{}", "x".repeat(9000))))
            .is_err()
    );
}

#[test]
fn unrelated_failed_source_does_not_spoil_available_selection() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", br#"{"sdfObject":{"Device":{}}}"#),
        SourceInput::failed("unused"),
    ])
    .unwrap();
    let result = catalogue
        .reference(ReferenceRequest::new(
            catalogue.source_id("model").unwrap(),
            "/sdfObject/Device",
        ))
        .unwrap();
    assert_eq!(result.disposition(), Disposition::Passed);
    assert!(result.to_markdown().contains("unused"));
    assert!(result.to_markdown().contains("Acquisition: failed"));
}

#[test]
fn unavailable_nested_names_are_inert_and_unsupported_declarations_are_retained() {
    let model = r##"{"sdfObject":{"Device":{"sdfInputData":{"description":"unsupported-grouping-input"},"sdfProperty":{"p":{"type":"object","properties":{"<img src=x>":{"sdfRef":"#/sdfData/Missing"}}}},"sdfAction":{"call":{"minItems":77}},"sdfEvent":{"event":{"sdfInputData":{"description":"unsupported-event-input"}}}}}}"##;
    let (_, markdown) = reference(model, "/sdfObject/Device");
    assert!(!markdown.contains("<img src=x>"));
    for text in [
        "unsupported\\-grouping\\-input",
        "unsupported\\-event\\-input",
        "77",
        "&lt;img src=x&gt;",
    ] {
        assert!(markdown.contains(text), "missing {text}");
    }
    let (disposition, markdown) = reference(
        r#"{"sdfObject":{"Device":{"sdfContext":{"metadata":{"type":"string","sdfRequired":[true]}}}}}"#,
        "/sdfObject/Device",
    );
    assert_eq!(disposition, Disposition::Passed, "{markdown}");
    assert!(markdown.contains("meaning is not established under the recorded draft"));
}

#[test]
fn atomic_navigation_and_mandatory_identity_survive_reduced_allowances() {
    use esdieff::Limits;
    let model = br#"{"sdfObject":{"Device":{"sdfProperty":{"p":{"type":"integer","minimum":1}}}}}"#;
    for bytes in (10000..30000).step_by(251) {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("model", model)],
            Limits::default().effective_allocation_bytes(bytes),
        )
        .unwrap();
        let result = catalogue
            .reference(ReferenceRequest::new(
                catalogue.source_id("model").unwrap(),
                "/sdfObject/Device",
            ))
            .unwrap();
        navigation(result.to_markdown());
    }
    let name = "!".repeat(3500);
    let model = format!("{{\"sdfObject\":{{\"{name}\":{{}}}}}}");
    let pointer = format!("/sdfObject/{name}");
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", model.as_bytes())],
        Limits::default().report_bytes(32768),
    )
    .unwrap();
    let result = catalogue
        .reference(ReferenceRequest::new(
            catalogue.source_id("model").unwrap(),
            &pointer,
        ))
        .unwrap();
    assert!(result.to_markdown().len() <= 32768);
    navigation(result.to_markdown());
}

#[test]
fn unavailable_content_does_not_hide_established_wrong_target_class() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", br##"{"namespace":{"missing":"https://example.invalid/missing"},"sdfProperty":{"p":{"sdfRef":"missing:#/sdfData/X"}}}"##)]).unwrap();
    assert!(
        catalogue
            .reference(ReferenceRequest::new(
                catalogue.source_id("model").unwrap(),
                "/sdfProperty/p"
            ))
            .is_err()
    );
}

#[test]
fn unsupported_context_requiredness_shapes_are_documentary() {
    for declaration in ["\"anything\"", "7", "true", "{}", "[true]"] {
        let model = r#"{"sdfObject":{"Device":{"sdfContext":{"metadata":{"type":"string","sdfRequired":DECLARATION}}}}}"#.replace("DECLARATION", declaration);
        let (disposition, markdown) = reference(&model, "/sdfObject/Device");
        assert_eq!(disposition, Disposition::Passed, "{markdown}");
        assert!(markdown.contains("meaning is not established under the recorded draft"));
        assert!(markdown.contains("sdfRequired"));
    }
}
