//! Public effective-role, provenance and finite-budget qualification.
//! Follows ADR 0029: Content-format declarations and recorded registry guidance.

use esdieff::{
    Admission, AlternativeId, Assessment, AssessmentStatus, Catalogue, CheckRequest, Class,
    EffectiveRequest, Field, InspectRequest, Limits, Lookup, OriginKind, SampleKind, SampleRequest,
    SourceInput,
};
use serde_json::{Value, json};

#[path = "../fixtures/content-format/grammar-cases.rs"]
mod grammar;

const SYNTAX: &str = "content-format-syntax/1";
const REGISTRY: &str = "content-format-registry/1";
const BASE: &[u8] = include_bytes!("../fixtures/content-format/base.sdf.json");
const OVERRIDE: &[u8] = include_bytes!("../fixtures/content-format/override.sdf.json");
const DELETION: &[u8] = include_bytes!("../fixtures/content-format/deletion.sdf.json");
const INHERITED: &[u8] = include_bytes!("../fixtures/content-format/inherited.sdf.json");

fn declaration(text: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "info": { "title": "Content-format declaration" },
        "sdfData": { "Payload": { "type": "string", "contentFormat": text } }
    }))
    .unwrap()
}

fn expanded() -> CheckRequest {
    CheckRequest::new().assessment(Assessment::ExpandedContentFormat)
}

#[test]
fn scalar_samples_required_targets_and_registry_checks_keep_their_own_meaning() {
    for (format, errors) in [("application/json@zstd", 0), ("text/plain;", 1)] {
        let model = serde_json::to_vec(&json!({
            "info": {},
            "sdfData": {"Packet": {"contentFormat": "60"}},
            "sdfObject": {"O": {
                "sdfRequired": ["#/sdfObject/O/sdfProperty/P"],
                "sdfProperty": {
                    "P": {"type": "number", "const": 7, "minimum": 5,
                          "maximum": 9, "unit": "kg", "contentFormat": format},
                    "Ordinary": {"type": "number", "const": 7, "unit": "kg"},
                    "Q": {"type": "number", "unit": "ms"}
                }
            }}
        }))
        .unwrap();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("model", &model),
            SourceInput::supplied("seven", b"7").sample(),
            SourceInput::supplied("eight", b"8").sample(),
        ])
        .unwrap();
        let model_id = catalogue.source_id("model").unwrap();
        assert_eq!(catalogue.source(model_id).unwrap().raw_bytes(), model);
        let initial = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(initial.outcome().errors(), 0);
        assert_eq!(initial.requested_coverage().total(), Some(8));

        let content = catalogue.check(expanded()).unwrap();
        assert_eq!(content.outcome().errors(), errors);
        let full = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(full.outcome().errors(), errors);
        assert!(matches!(full.assessment_area_status(SYNTAX), Field::Absent));
        let full_wire: Value = serde_json::from_str(&full.to_json()).unwrap();
        assert_eq!(
            full_wire["baseline"]["registry_knowledge"]
                .as_array()
                .unwrap()
                .len(),
            if errors == 0 { 5 } else { 3 }
        );
        assert!(full.unit_guidance().present().is_some());

        let required = catalogue
            .inspect(InspectRequest::new(model_id, "/sdfObject/O").required_declarations(true))
            .unwrap();
        let declarations = required.required_declarations().require().unwrap();
        let object = declarations
            .owners()
            .find(|owner| owner.class().present() == Some(Class::Object))
            .unwrap();
        let reference = object.references().require().unwrap().next().unwrap();
        let Lookup::Found(mut targets) = reference.targets() else {
            panic!("established required Property");
        };
        assert!(targets.next().is_some());
        assert!(targets.next().is_none());
        for (pointer, sample, admission, count) in [
            (
                "/sdfObject/O/sdfProperty/P",
                "seven",
                Admission::Undetermined,
                0,
            ),
            (
                "/sdfObject/O/sdfProperty/P",
                "eight",
                Admission::Undetermined,
                0,
            ),
            (
                "/sdfObject/O/sdfProperty/Ordinary",
                "seven",
                Admission::Admitted,
                0,
            ),
            (
                "/sdfObject/O/sdfProperty/Ordinary",
                "eight",
                Admission::Rejected,
                1,
            ),
        ] {
            let report = catalogue
                .check_sample(
                    SampleRequest::new(model_id, SampleKind::Property, pointer)
                        .sample(catalogue.source_id(sample).unwrap()),
                )
                .unwrap();
            assert_eq!(report.sample_result().unwrap().admission(), admission);
            assert_eq!(report.outcome().errors(), count);
            assert_eq!(report.requested_coverage().total(), Some(9));
            assert!(matches!(
                report.assessment_area_status(SYNTAX),
                Field::Absent
            ));
            assert!(matches!(
                report.registry_knowledge("media-types"),
                Field::Absent
            ));
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["outcome"]["errors"], count);
            assert!(
                report
                    .to_human()
                    .contains(if count == 0 { "Errors: 0" } else { "Errors: 1" })
            );
        }
    }
}

#[test]
fn a_malformed_declaration_is_added_only_to_the_new_immutable_request() {
    let bytes = declaration("text/plain;");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
    let initial = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(initial.outcome().errors(), 0);
    assert_eq!(initial.requested_coverage().total(), Some(8));
    assert!(matches!(
        initial.assessment_area_status(SYNTAX),
        Field::Absent
    ));
    assert!(matches!(
        initial.assessment_area_status(REGISTRY),
        Field::Absent
    ));
    let modified = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(modified.requested_coverage().total(), Some(9));
    assert!(matches!(
        modified.assessment_area_status(SYNTAX),
        Field::Absent
    ));
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.requested_coverage().total(), Some(10));
    assert_eq!(
        report.requested_coverage().inventory(),
        Some("esdieff.expanded.content-format/1")
    );
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.syntax")
    );
    let full = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(full.outcome().complete());
    assert_eq!(full.outcome().errors(), 1);
    assert!(matches!(
        full.requirement_status("rfc9880.model.M024/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(matches!(full.assessment_area_status(SYNTAX), Field::Absent));
    assert_eq!(
        catalogue.sources().next().unwrap().raw_bytes(),
        bytes.as_slice()
    );
}

#[test]
fn every_reviewed_lexical_branch_keeps_range_separate_from_abnf() {
    for case in grammar::CASES {
        let bytes = declaration(case.text);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
        let report = catalogue.check(expanded()).unwrap();
        let diagnostics = report.diagnostics();
        let lexical = diagnostics
            .conditions()
            .filter(|c| c.code() == "content_format.syntax")
            .count();
        let range = diagnostics
            .conditions()
            .filter(|c| c.code() == "content_format.number_range")
            .count();
        assert_eq!(lexical, usize::from(!case.grammar), "{}", case.name);
        assert_eq!(
            range,
            usize::from(case.number_domain == Some(false)),
            "{}",
            case.name
        );
        assert_eq!(
            report.outcome().errors(),
            u32::from(!case.grammar || case.number_domain == Some(false)),
            "{}",
            case.name
        );
        assert!(
            matches!(
                report.assessment_area_status(SYNTAX),
                Field::Present(AssessmentStatus::Assessed)
            ),
            "{}",
            case.name
        );
        assert_eq!(
            catalogue.sources().next().unwrap().raw_bytes(),
            bytes.as_slice()
        );
    }
    for length in [127, 128] {
        for text in [
            format!("{}/b", "a".repeat(length)),
            format!("a/{}", "b".repeat(length)),
        ] {
            let bytes = declaration(&text);
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
            let report = catalogue.check(expanded()).unwrap();
            assert_eq!(report.outcome().errors(), u32::from(length == 128));
        }
    }
}

#[test]
fn registry_positive_unknown_and_unsupported_parameter_meaning_have_different_outcomes() {
    for (text, status, incomplete_code, parameter_notice) in [
        ("60", AssessmentStatus::Assessed, None, false),
        (
            "application/json@GZIP",
            AssessmentStatus::Assessed,
            None,
            false,
        ),
        (
            "application/json;p=unfamiliar",
            AssessmentStatus::Assessed,
            None,
            true,
        ),
        (
            "application/unknown-model-format",
            AssessmentStatus::Undetermined,
            Some("content_format.registration_unknown"),
            false,
        ),
        (
            "65535",
            AssessmentStatus::Undetermined,
            Some("content_format.registration_unknown"),
            false,
        ),
        (
            "application/json@identity",
            AssessmentStatus::Undetermined,
            Some("content_format.interpretation_unavailable"),
            false,
        ),
    ] {
        let bytes = declaration(text);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
        let report = catalogue.check(expanded()).unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(matches!(
            report.assessment_area_status(SYNTAX),
            Field::Present(AssessmentStatus::Assessed)
        ));
        assert!(
            matches!(report.assessment_area_status(REGISTRY), Field::Present(actual) if actual == status)
        );
        let diagnostics = report.diagnostics();
        if let Some(code) = incomplete_code {
            assert!(diagnostics.conditions().any(|c| c.code() == code));
            assert!(!report.outcome().complete());
        } else {
            assert!(
                diagnostics
                    .conditions()
                    .any(|c| c.code() == "content_format.registry_known")
            );
        }
        assert_eq!(
            diagnostics
                .conditions()
                .any(|c| c.code() == "content_format.parameter_semantics_unassessed"),
            parameter_notice
        );
        // Never infer that absence of a warning establishes parameter or payload validity.
    }
    let bytes = declaration("60");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
    let report = catalogue.check(expanded()).unwrap();
    let Field::Present(knowledge) = report.registry_knowledge("coap-content-formats") else {
        panic!("actual-use numeric snapshot is structurally available")
    };
    assert_eq!(knowledge.state(), "available");
    assert_eq!(knowledge.scope(), Some("relevant_dated_positive_records"));
    assert!(knowledge.snapshot().is_some());
    assert!(knowledge.digest().is_some());
    assert!(matches!(
        report.registry_knowledge("unrequested-registry"),
        Field::Absent
    ));
}

#[test]
fn selected_override_and_deletion_do_not_assess_removed_contributor_values() {
    for (consumer, expected, kind) in [
        (OVERRIDE, Some("application/json"), OriginKind::Override),
        (DELETION, None, OriginKind::Deleted),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("base", BASE),
            SourceInput::supplied("model", consumer),
        ])
        .unwrap();
        let root = catalogue.source_id("model").unwrap();
        let selected = catalogue.check(expanded().root(root)).unwrap();
        assert_eq!(selected.outcome().errors(), 0);
        let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
        let Lookup::Found(property) = model.definition(Class::Property, "Payload") else {
            panic!("effective Property")
        };
        let data = property.data_description().require().unwrap();
        match expected {
            Some(text) => {
                assert!(matches!(data.content_format(), Field::Present(actual) if actual == text))
            }
            None => assert!(matches!(data.content_format(), Field::Absent)),
        }
        let explanation = model
            .explanation("/sdfProperty/Payload/contentFormat")
            .require()
            .unwrap();
        assert_eq!(explanation.kind(), kind);
        assert_eq!(explanation.effective().root_document(), root);
        assert!(explanation.sources().any(|s| s.source_id() == root));
        assert_eq!(
            catalogue
                .source(catalogue.source_id("base").unwrap())
                .unwrap()
                .raw_bytes(),
            BASE
        );
        let whole = catalogue.check(expanded()).unwrap();
        assert!(whole.outcome().errors() >= 2);
        if expected.is_none() {
            assert!(matches!(
                selected.assessment_area_status(SYNTAX),
                Field::Present(AssessmentStatus::NotApplicable)
            ));
        }
    }
}

#[test]
fn inherited_errors_keep_effective_and_nearest_source_coordinates() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", BASE),
        SourceInput::supplied("model", INHERITED),
    ])
    .unwrap();
    let root = catalogue.source_id("model").unwrap();
    let source = catalogue.source_id("base").unwrap();
    let report = catalogue.check(expanded().root(root)).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let condition = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "content_format.syntax")
        .unwrap();
    assert_eq!(condition["primary"]["kind"], "effective");
    assert_eq!(condition["primary"]["root"], root.get());
    assert_eq!(
        condition["primary"]["pointer"],
        "/sdfProperty/Payload/contentFormat"
    );
    assert!(
        condition["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|loc| loc["kind"] == "source"
                && loc["source_id"] == source.get()
                && loc["pointer"] == "/sdfData/Payload/contentFormat")
    );
    let model = catalogue.effective(EffectiveRequest::new(root)).unwrap();
    let origin = model
        .explanation("/sdfProperty/Payload/contentFormat")
        .require()
        .unwrap();
    assert_eq!(origin.kind(), OriginKind::Inherited);
    assert!(origin.sources().any(|s| s.source_id() == source));
    assert!(origin.consumer().is_some());
}

#[test]
fn unknown_critical_meaning_does_not_erase_a_clear_sources_independent_error() {
    let clear = declaration("text/plain;");
    let critical = include_bytes!("../fixtures/content-format/critical.sdf.json");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("critical", critical),
        SourceInput::supplied("clear", &clear),
    ])
    .unwrap();
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(!report.outcome().complete());
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(matches!(
        report.assessment_area_status(REGISTRY),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    let selected = catalogue
        .check(expanded().root(catalogue.source_id("critical").unwrap()))
        .unwrap();
    assert_eq!(selected.outcome().errors(), 0);
    assert!(!selected.outcome().complete());
}

#[test]
fn effective_unavailability_and_restricted_donor_meaning_are_not_raw_text_fallbacks() {
    let unavailable = include_bytes!("../fixtures/content-format/unavailable-effective.sdf.json");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", unavailable)]).unwrap();
    let source = catalogue.sources().next().unwrap();
    let Lookup::Found(property) = source.definition(Class::Property, "Payload") else {
        panic!("source Property retained")
    };
    assert!(matches!(
        property
            .data_description()
            .require()
            .unwrap()
            .content_format(),
        Field::Present("text/plain;")
    ));
    let report = catalogue.check(expanded()).unwrap();
    assert!(!report.outcome().complete());
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.syntax")
    );
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));

    let donor = include_bytes!("../fixtures/content-format/donor-meaning.sdf.json");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", donor)]).unwrap();
    let source = catalogue.source_id("model").unwrap();
    let model = catalogue.effective(EffectiveRequest::new(source)).unwrap();
    let Lookup::Found(array) = model.definition(Class::Data, "Array") else {
        panic!("Array description")
    };
    let item = array
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(item.content_format(), Field::Unavailable(_)));
    let mut alternatives = item.alternatives().require().unwrap();
    let (_, inherited) = alternatives
        .find(|(id, _)| *id == AlternativeId::Choice("Inherited"))
        .unwrap();
    assert!(matches!(
        inherited.require().unwrap().content_format(),
        Field::Unavailable(_)
    ));
    let mut alternatives = item.alternatives().require().unwrap();
    let (_, overridden) = alternatives
        .find(|(id, _)| *id == AlternativeId::Choice("Override"))
        .unwrap();
    assert!(matches!(
        overridden.require().unwrap().content_format(),
        Field::Present("application/json")
    ));
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));
}

#[test]
fn unsupported_decoded_text_and_known_nonstring_shape_keep_independent_meanings() {
    let bytes = include_bytes!("../fixtures/content-format/unavailable-text.sdf.json");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "json.syntax")
    );
    for scalar in [
        Value::Null,
        Value::Bool(false),
        json!(42),
        json!({}),
        json!([]),
    ] {
        let bytes = serde_json::to_vec(&json!({"info":{"title":"Wrong format shape"},"sdfData":{"Payload":{"contentFormat":scalar}}})).unwrap();
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", &bytes)]).unwrap();
        let report = catalogue.check(expanded()).unwrap();
        assert_eq!(report.outcome().errors(), 1);
        let diagnostics = report.diagnostics();
        assert_eq!(
            diagnostics
                .conditions()
                .filter(|c| c.code() == "data.string")
                .count(),
            1
        );
        assert!(
            !diagnostics
                .conditions()
                .any(|c| c.code() == "content_format.syntax")
        );
        assert!(matches!(
            report.assessment_area_status(SYNTAX),
            Field::Present(AssessmentStatus::Assessed)
        ));
    }
}

#[test]
fn presentation_omission_preserves_semantic_findings_and_actual_use_knowledge() {
    let bytes = serde_json::to_vec(&json!({
        "info": { "title": "Independent findings" },
        "sdfData": {
            "Bad": { "contentFormat": "text/plain;" },
            "Known": { "contentFormat": "60" }
        }
    }))
    .unwrap();
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("authored", &bytes)],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(matches!(
        report.assessment_area_status(REGISTRY),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert_eq!(report.diagnostics().conditions().len(), 0);
    assert!(matches!(
        report.registry_knowledge("coap-content-formats"),
        Field::Present(_)
    ));
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["coverage"]["requested"]["total"], 10);
    assert!(
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["registry"] == "coap-content-formats" && r["state"] == "available")
    );
}

#[test]
fn long_numeric_text_is_exact_and_finite_capability_is_not_a_range_verdict() {
    let bytes = declaration(&"9".repeat(20_000));
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.number_range")
    );
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.syntax")
    );
    let limited = Catalogue::with_limits(
        &[SourceInput::supplied("authored", &bytes)],
        Limits::default().decoding_work(10),
    )
    .unwrap();
    let report = limited.check(expanded()).unwrap();
    assert!(!report.outcome().complete());
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.number_range")
    );
}

#[test]
fn structural_wire_consumer_keeps_absence_withholding_and_future_details_distinct() {
    fn knowledge<'a>(wire: &'a Value, id: &str) -> Option<(&'a str, Option<&'a str>)> {
        assert_eq!(wire["schema"], "esdieff.report");
        assert_eq!(wire["version"], "1.0");
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["registry"] == id)
            .map(|row| (row["state"].as_str().unwrap(), row["snapshot"].as_str()))
    }
    let bytes = declaration("application/json@gzip");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
    for policy in [
        esdieff::RecordedRegistryPolicy::UseRecorded,
        esdieff::RecordedRegistryPolicy::WithholdRecorded,
    ] {
        let report = catalogue
            .check(expanded().recorded_registry_policy(policy))
            .unwrap();
        let mut wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let requested = &wire["coverage"]["requested"];
        assert_eq!(requested["inventory"], "esdieff.expanded.content-format/1");
        assert_eq!(requested["count_unit"], "assessment_area");
        let sum: u64 = ["assessed", "not_applicable", "unassessed", "undetermined"]
            .iter()
            .map(|key| requested[key].as_u64().unwrap())
            .sum();
        assert_eq!(sum, 10);
        assert_eq!(knowledge(&wire, "coap-content-formats"), None);
        let before = knowledge(&wire, "media-types").unwrap();
        assert_eq!(
            before.0,
            if policy == esdieff::RecordedRegistryPolicy::UseRecorded {
                "available"
            } else {
                "unavailable"
            }
        );
        assert_eq!(
            before.1.is_some(),
            policy == esdieff::RecordedRegistryPolicy::UseRecorded
        );
        let owned = (before.0.to_owned(), before.1.map(str::to_owned));
        wire["future_optional"] = json!({"field":true});
        wire["diagnostics"]["items"]
            .as_array_mut()
            .unwrap()
            .push(json!({"code":"future.unknown_condition","severity":"notice"}));
        let after = knowledge(&wire, "media-types").unwrap();
        assert_eq!((after.0.to_owned(), after.1.map(str::to_owned)), owned);
        assert_eq!(wire["outcome"]["errors"], 0);
        assert_eq!(
            wire["outcome"]["complete"],
            policy == esdieff::RecordedRegistryPolicy::UseRecorded
        );
    }
}

#[test]
fn finished_unknown_context_does_not_suppress_a_clear_independent_query() {
    let critical = include_bytes!("../fixtures/content-format/critical.sdf.json");
    let clear = declaration("60");
    for policy in [
        esdieff::RecordedRegistryPolicy::UseRecorded,
        esdieff::RecordedRegistryPolicy::WithholdRecorded,
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("critical", critical),
            SourceInput::supplied("clear", &clear),
        ])
        .unwrap();
        let report = catalogue
            .check(expanded().recorded_registry_policy(policy))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(matches!(
            report.assessment_area_status(SYNTAX),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        assert!(matches!(
            report.assessment_area_status(REGISTRY),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        let facts = report
            .registry_knowledge("coap-content-formats")
            .require()
            .unwrap();
        assert_eq!(
            facts.state(),
            if policy == esdieff::RecordedRegistryPolicy::UseRecorded {
                "available"
            } else {
                "unavailable"
            }
        );
        assert!(report.diagnostics().conditions().any(|c| c.code()
            == if policy == esdieff::RecordedRegistryPolicy::UseRecorded {
                "content_format.registry_known"
            } else {
                "content_format.registry_knowledge_withheld"
            }));
        assert!(matches!(
            report.registry_knowledge("media-types"),
            Field::Absent
        ));
    }
}

#[test]
fn complete_no_query_population_needs_no_replay_or_fabricated_evidence() {
    for text in ["text/plain;", "65536"] {
        let bytes = declaration(text);
        let first = (1..10000)
            .find(|&work| {
                let catalogue = Catalogue::with_limits(
                    &[SourceInput::supplied("authored", &bytes)],
                    Limits::default().decoding_work(work),
                )
                .unwrap();
                let report = catalogue.check(expanded()).unwrap();
                matches!(
                    report.assessment_area_status(SYNTAX),
                    Field::Present(AssessmentStatus::Assessed)
                )
            })
            .unwrap();
        for policy in [
            esdieff::RecordedRegistryPolicy::UseRecorded,
            esdieff::RecordedRegistryPolicy::WithholdRecorded,
        ] {
            let catalogue = Catalogue::with_limits(
                &[SourceInput::supplied("authored", &bytes)],
                Limits::default().decoding_work(first),
            )
            .unwrap();
            let report = catalogue
                .check(expanded().recorded_registry_policy(policy))
                .unwrap();
            assert_eq!(report.outcome().errors(), 1);
            assert!(matches!(
                report.assessment_area_status(SYNTAX),
                Field::Present(AssessmentStatus::Assessed)
            ));
            assert!(matches!(
                report.assessment_area_status(REGISTRY),
                Field::Present(AssessmentStatus::NotApplicable)
            ));
            assert!(matches!(
                report.registry_knowledge("coap-content-formats"),
                Field::Absent
            ));
            assert!(matches!(
                report.registry_knowledge("media-types"),
                Field::Absent
            ));
            println!("no-query first complete text={text} work={first} policy={policy:?}");
        }
    }
    let critical = include_bytes!("../fixtures/content-format/critical.sdf.json");
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("critical", critical)]).unwrap();
    let report = catalogue
        .check(
            expanded().recorded_registry_policy(esdieff::RecordedRegistryPolicy::WithholdRecorded),
        )
        .unwrap();
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(matches!(
        report.assessment_area_status(REGISTRY),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(matches!(
        report.registry_knowledge("media-types"),
        Field::Absent
    ));
}

#[test]
fn full_roles_and_excluded_sample_direct_rows_keep_their_populations() {
    let bytes = br#"{"info":{},"sdfProperty":{"P":{"contentFormat":"text/plain;"}},"sdfAction":{"A":{"sdfInputData":{"contentFormat":"text/plain;"},"sdfOutputData":{"contentFormat":"text/plain;"}}},"sdfEvent":{"E":{"sdfOutputData":{"contentFormat":"text/plain;"}}},"sdfData":{"D":{"type":"object","properties":{"K":{"contentFormat":"text/plain;"}}}}}"#;
    let sample = declaration("text/plain;");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", bytes),
        SourceInput::supplied("sample", &sample).sample(),
    ])
    .unwrap();
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 5);
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|c| c.code() == "content_format.syntax")
            .count(),
        5
    );
    let direct = br#"{"info":{},"sdfData":{"Readings":{"type":"array","items":{"type":"string","contentFormat":"text/plain;","sdfChoice":{"InheritedOpaque":{},"ExplicitFull":{"contentFormat":"text/plain;"}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", direct)]).unwrap();
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|c| c.code() == "content_format.syntax")
            .count(),
        1
    );
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));
}

#[test]
fn unavailable_member_identity_preserves_clear_source_evidence() {
    let unknown = br#"{"info":{},"sdfData":{"Bad":{"contentFormat":"text/plain;","\uD800":true}}}"#;
    let clear = declaration("60");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("unknown", unknown),
        SourceInput::supplied("clear", &clear),
    ])
    .unwrap();
    let report = catalogue.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.syntax")
    );
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert!(matches!(
        report.assessment_area_status(REGISTRY),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    assert_eq!(
        report
            .registry_knowledge("coap-content-formats")
            .require()
            .unwrap()
            .state(),
        "available"
    );
}
