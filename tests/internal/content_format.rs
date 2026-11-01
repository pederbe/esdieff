//! Public declaration assessment follows ADR 0029: Content-format declarations
//! and recorded registry guidance, independently of retained diagnostic details.
use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, SourceInput};
use esdieff::{Limits, RecordedRegistryPolicy};
use serde_json::{Value, json};

const SYNTAX: &str = "content-format-syntax/1";
const REGISTRY: &str = "content-format-registry/1";
fn declaration(text: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"info":{},"sdfData":{"Payload":{"contentFormat":text}}})).unwrap()
}
fn expanded() -> CheckRequest {
    CheckRequest::new().assessment(Assessment::ExpandedContentFormat)
}

#[test]
fn malformed_declaration_is_added_only_to_the_explicit_inventory() {
    let bytes = declaration("text/plain;");
    let cat = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
    let initial = cat.check(CheckRequest::new()).unwrap();
    assert_eq!(initial.outcome().errors(), 0);
    assert_eq!(initial.requested_coverage().total(), Some(8));
    assert!(matches!(
        initial.assessment_area_status(SYNTAX),
        Field::Absent
    ));
    let report = cat.check(expanded()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(report.requested_coverage().total(), Some(10));
    assert_eq!(
        report.requested_coverage().inventory(),
        Some("esdieff.expanded.content-format/1")
    );
    assert!(matches!(
        report.assessment_area_status(SYNTAX),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(matches!(
        report.assessment_area_status(REGISTRY),
        Field::Present(AssessmentStatus::NotApplicable)
    ));
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "content_format.syntax")
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["outcome"]["errors"], 1);
    assert!(
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(cat.sources().next().unwrap().raw_bytes(), bytes.as_slice());
}

#[test]
fn recorded_positive_unknown_withheld_and_excluded_meaning_are_distinct() {
    for (text, registry, status, code) in [
        (
            "60",
            "coap-content-formats",
            AssessmentStatus::Assessed,
            "content_format.registry_known",
        ),
        (
            "APPLICATION/JSON@GZIP",
            "media-types",
            AssessmentStatus::Assessed,
            "content_format.registry_known",
        ),
        (
            "application/x-unfamiliar-example",
            "media-types",
            AssessmentStatus::Undetermined,
            "content_format.registration_unknown",
        ),
        (
            "text/plain;p=\"@identity\"",
            "media-types",
            AssessmentStatus::Assessed,
            "content_format.parameter_semantics_unassessed",
        ),
        (
            "application/json@identity",
            "http-content-codings",
            AssessmentStatus::Undetermined,
            "content_format.interpretation_unavailable",
        ),
    ] {
        let bytes = declaration(text);
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
        let report = cat.check(expanded()).unwrap();
        assert_eq!(report.outcome().errors(), 0, "{text}");
        assert!(
            matches!(
                report.assessment_area_status(SYNTAX),
                Field::Present(AssessmentStatus::Assessed)
            ),
            "{text}"
        );
        assert!(
            matches!(report.assessment_area_status(REGISTRY), Field::Present(s) if s == status),
            "{text}"
        );
        assert!(
            report.diagnostics().conditions().any(|c| c.code() == code),
            "{text}: {code}"
        );
        let knowledge = report.registry_knowledge(registry).require().unwrap();
        assert_eq!(knowledge.state(), "available");
        assert!(knowledge.snapshot().is_some());
        assert!(matches!(
            report.registry_knowledge("iana.senml-units"),
            Field::Absent
        ));
        let withheld = cat
            .check(expanded().recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded))
            .unwrap();
        assert_eq!(withheld.outcome().errors(), 0);
        assert!(matches!(
            withheld.assessment_area_status(REGISTRY),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        assert!(
            withheld
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "content_format.registry_knowledge_withheld")
        );
        let unavailable = withheld.registry_knowledge(registry).require().unwrap();
        assert_eq!(unavailable.state(), "unavailable");
        assert_eq!(unavailable.snapshot(), None);
        assert_eq!(unavailable.reason(), Some("caller_withheld"));
    }
}

#[test]
fn every_positive_record_is_reachable_and_flags_are_dated_advice() {
    for (bytes, numeric) in [
        (
            include_str!("../fixtures/content-format/coap-positive.json"),
            true,
        ),
        (
            include_str!("../fixtures/content-format/media-positive.json"),
            false,
        ),
        (
            include_str!("../fixtures/content-format/http-positive.json"),
            false,
        ),
    ] {
        let rows: Value = serde_json::from_str(bytes).unwrap();
        for row in rows.as_array().unwrap() {
            let text = if numeric {
                row["id"].to_string()
            } else if row.get("template").is_some() {
                row["key"].as_str().unwrap().to_ascii_uppercase()
            } else {
                format!(
                    "application/json@{}",
                    row["key"].as_str().unwrap().to_ascii_uppercase()
                )
            };
            let bytes = declaration(&text);
            let cat = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
            let report = cat.check(expanded()).unwrap();
            assert_eq!(report.outcome().errors(), 0, "{text}");
            assert!(
                matches!(
                    report.assessment_area_status(REGISTRY),
                    Field::Present(AssessmentStatus::Assessed)
                ),
                "{text}"
            );
            let conditions = report.diagnostics();
            let condition = conditions
                .conditions()
                .filter(|c| c.code() == "content_format.registry_known")
                .last()
                .unwrap();
            for (flag, wording) in [
                ("temporary", "temporary"),
                ("obsolete", "obsolete"),
                ("deprecated", "deprecated"),
            ] {
                assert_eq!(
                    condition.message().contains(wording),
                    row["flags"][flag] == true,
                    "{text}/{flag}"
                );
            }
        }
    }
}

#[test]
fn all_optional_details_can_be_zero_without_losing_counts_or_used_facts() {
    let bytes = serde_json::to_vec(&json!({"info":{},"sdfData":{
        "A":{"contentFormat":"text/plain;"},"B":{"contentFormat":"60"},
        "C":{"contentFormat":"application/json@gzip"}
    }}))
    .unwrap();
    for policy in [
        RecordedRegistryPolicy::UseRecorded,
        RecordedRegistryPolicy::WithholdRecorded,
    ] {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("authored", &bytes)],
            Limits::default().report_bytes(39644),
        )
        .unwrap();
        let report = cat
            .check(expanded().recorded_registry_policy(policy))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        let coverage = report.requested_coverage();
        assert_eq!(
            coverage.assessed(),
            if policy == RecordedRegistryPolicy::UseRecorded {
                8
            } else {
                7
            }
        );
        assert_eq!(coverage.unassessed(), 0);
        assert_eq!(coverage.not_applicable(), 2);
        assert_eq!(
            coverage.undetermined(),
            u32::from(policy == RecordedRegistryPolicy::WithholdRecorded)
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(wire["coverage"]["rows"].as_array().unwrap().is_empty());
        assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
        assert_eq!(
            wire["baseline"]["registry_knowledge"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert!(report.to_human().contains("coap-content-formats"));
    }
}

#[test]
fn source_bound_shared_work_distinguishes_before_and_after_first_comparison() {
    let bytes = declaration("60");
    let observation = |work| {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("authored", &bytes)],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let report = cat.check(expanded()).unwrap();
        let syntax = matches!(
            report.assessment_area_status(SYNTAX),
            Field::Present(AssessmentStatus::Assessed)
        );
        let completed = matches!(
            report.assessment_area_status(REGISTRY),
            Field::Present(AssessmentStatus::Assessed)
        );
        (
            syntax,
            report
                .registry_knowledge("coap-content-formats")
                .present()
                .is_some(),
            completed,
        )
    };
    let syntax = (1..5000)
        .find(|&work| observation(work).0)
        .expect("grammar admitted");
    let first = (syntax..5000)
        .find(|&work| observation(work).1)
        .expect("first comparison admitted");
    let complete = (first..5000)
        .find(|&work| observation(work).2)
        .expect("positive lookup completed");
    println!(
        "content numeric source-bound work: grammar={syntax} first_compare={first} completed={complete}"
    );
    assert!(syntax < first && first < complete);
    for (work, consulted) in [(first - 1, false), (first, true)] {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("authored", &bytes)],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let report = cat.check(expanded()).unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(matches!(
            report.assessment_area_status(SYNTAX),
            Field::Present(AssessmentStatus::Assessed)
        ));
        assert!(matches!(
            report.assessment_area_status(REGISTRY),
            Field::Present(AssessmentStatus::Undetermined)
        ));
        assert_eq!(
            report
                .registry_knowledge("coap-content-formats")
                .present()
                .is_some(),
            consulted
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["baseline"]["registry_knowledge"]
                .as_array()
                .unwrap()
                .len(),
            usize::from(consulted)
        );
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "content_format.registration_unknown")
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "operation.limit")
        );
    }
    let cat = Catalogue::with_limits(
        &[SourceInput::supplied("authored", &bytes)],
        Limits::default().decoding_work(first - 1),
    )
    .unwrap();
    let report = cat
        .check(expanded().recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded))
        .unwrap();
    assert_eq!(
        report
            .registry_knowledge("coap-content-formats")
            .require()
            .unwrap()
            .state(),
        "unavailable"
    );
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "content_format.registry_knowledge_withheld")
    );
}

#[test]
fn full_produces_five_real_facts_and_preserves_other_assessments() {
    let bytes = serde_json::to_vec(&json!({"info":{},"sdfData":{
        "A":{"unit":"kg","contentFormat":"60"},
        "B":{"unit":"ms","contentFormat":"application/json@zstd"},
        "C":{"type":"number","nullable":false,"minimum":3,"maximum":1,"sdfRequired":["#/sdfData/A"]}
    }}))
    .unwrap();
    for policy in [
        RecordedRegistryPolicy::UseRecorded,
        RecordedRegistryPolicy::WithholdRecorded,
    ] {
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("authored", &bytes)]).unwrap();
        let report = cat
            .check(
                CheckRequest::new()
                    .assessment(Assessment::FullBaseline)
                    .recorded_registry_policy(policy),
            )
            .unwrap();
        assert!(matches!(
            report.assessment_area_status(SYNTAX),
            Field::Absent
        ));
        assert!(matches!(
            report.assessment_area_status("never-known"),
            Field::Absent
        ));
        assert_eq!(report.requested_coverage().total(), Some(13));
        assert!(!report.outcome().complete());
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let facts = wire["baseline"]["registry_knowledge"].as_array().unwrap();
        // Withheld unit lookup stops before needing its secondary table.
        assert_eq!(
            facts.len(),
            if policy == RecordedRegistryPolicy::UseRecorded {
                5
            } else {
                4
            }
        );
        assert_eq!(
            wire["policy"]["recorded_registry"],
            if policy == RecordedRegistryPolicy::UseRecorded {
                "use_recorded"
            } else {
                "withhold_recorded"
            }
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|c| c.code() == "proof.empty_numeric_interval"),
            "{wire}"
        );
        assert!(wire["result"]["unit_guidance"].is_object());
        assert!(wire["result"].get("content_format").is_none());
    }
}

#[test]
fn registry_work_cannot_complete_unvisited_later_declaration_grammar() {
    let bytes = br#"{"info":{},"sdfData":{"A":{"contentFormat":"60"},"B":{"contentFormat":"text/plain;"}}}"#;
    let mut found = None;
    for work in 1..10000 {
        let cat = Catalogue::with_limits(
            &[SourceInput::supplied("authored", bytes)],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let report = cat.check(expanded()).unwrap();
        if report.outcome().errors() == 0
            && matches!(
                report.assessment_area_status(SYNTAX),
                Field::Present(AssessmentStatus::Assessed)
            )
            && matches!(
                report.assessment_area_status(REGISTRY),
                Field::Present(AssessmentStatus::Undetermined)
            )
            && report
                .registry_knowledge("coap-content-formats")
                .present()
                .is_some()
        {
            println!(
                "unvisited later grammar: work={work} wire={}",
                report.to_json()
            );
            found = Some(work);
            break;
        }
    }
    assert_eq!(
        found, None,
        "a partial registry pass cannot certify unvisited syntax"
    );
}
