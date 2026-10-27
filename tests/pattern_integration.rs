//! Composed pattern syntax preserves roles, scope and nearest origins.
mod coverage_support;
use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, InspectRequest, Limits,
    SampleKind, SampleRequest, SourceInput, View,
};
use serde_json::Value;
const DONOR: &[u8] = include_bytes!("fixtures/pattern-syntax/donor.sdf.json");
fn check(catalogue: &Catalogue, root: &str) -> Value {
    let report = catalogue
        .check(
            CheckRequest::new()
                .root(catalogue.source_id(root).unwrap())
                .assessment(Assessment::ExpandedPatternSyntax),
        )
        .unwrap();
    serde_json::from_str(&report.to_json()).unwrap()
}
fn patterns(wire: &Value) -> Vec<&Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["assessment_unit"] == "pattern-syntax/1")
        .collect()
}

#[test]
fn selected_imports_overrides_and_deletions_use_only_surviving_patterns() {
    for reversed in [false, true] {
        for (source, expected) in [
            (
                include_bytes!("fixtures/pattern-syntax/root-inherited.sdf.json").as_slice(),
                1,
            ),
            (
                include_bytes!("fixtures/pattern-syntax/root-override.sdf.json").as_slice(),
                0,
            ),
            (
                include_bytes!("fixtures/pattern-syntax/root-delete.sdf.json").as_slice(),
                0,
            ),
        ] {
            let mut inputs = vec![
                SourceInput::supplied("donor", DONOR),
                SourceInput::supplied("root", source),
            ];
            if reversed {
                inputs.reverse();
            }
            let catalogue = Catalogue::from_inputs(&inputs).unwrap();
            let wire = check(&catalogue, "root");
            assert_eq!(wire["outcome"]["errors"], expected);
            assert_eq!(patterns(&wire).len(), expected as usize);
            if expected == 1 {
                let finding = patterns(&wire)[0];
                assert_eq!(
                    finding["primary"]["root"],
                    catalogue.source_id("root").unwrap().get()
                );
                assert_eq!(finding["primary"]["pointer"], "/sdfProperty/Use/pattern");
                assert_eq!(
                    finding["related"][0]["source_id"],
                    catalogue.source_id("donor").unwrap().get()
                );
                assert_eq!(finding["related"][0]["pointer"], "/sdfData/Base/pattern");
                let span = &finding["related"][0]["span"];
                assert_eq!(
                    &DONOR[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
                    br#""[""#
                );
            }
            let all = catalogue
                .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
                .unwrap();
            assert!(
                all.outcome().errors() >= 2,
                "catalogue separately roots unused donor definitions"
            );
            assert_eq!(
                catalogue
                    .source(catalogue.source_id("donor").unwrap())
                    .unwrap()
                    .raw_bytes(),
                DONOR
            );
        }
    }
}

#[test]
fn explicit_full_override_has_its_own_nearest_authored_token() {
    let root=br#"{"info":{},"namespace":{"d":"https://example.com/patterns"},"sdfProperty":{"Use":{"sdfRef":"d:#/sdfData/Base","pattern":"("}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("donor", DONOR),
        SourceInput::supplied("root", root),
    ])
    .unwrap();
    let wire = check(&catalogue, "root");
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(
        patterns(&wire)[0]["related"][0]["source_id"],
        catalogue.source_id("root").unwrap().get()
    );
    assert_eq!(
        patterns(&wire)[0]["related"][0]["pointer"],
        "/sdfProperty/Use/pattern"
    );
}

#[test]
fn direct_patterns_stay_opaque_until_a_surviving_full_override_establishes_meaning() {
    for (source, status, code) in [
        (
            include_bytes!("fixtures/pattern-syntax/direct-only.sdf.json").as_slice(),
            AssessmentStatus::NotApplicable,
            None,
        ),
        (
            include_bytes!("fixtures/pattern-syntax/direct-inherited.sdf.json").as_slice(),
            AssessmentStatus::Undetermined,
            Some("pattern.unavailable"),
        ),
        (
            include_bytes!("fixtures/pattern-syntax/direct-full-override.sdf.json").as_slice(),
            AssessmentStatus::Assessed,
            None,
        ),
    ] {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", source)]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        assert_eq!(
            report.assessment_area_status("pattern-syntax/1").present(),
            Some(status)
        );
        assert_eq!(report.outcome().errors(), 0);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        if let Some(code) = code {
            assert!(patterns(&wire).iter().any(|item| item["code"] == code));
        } else {
            assert!(patterns(&wire).is_empty());
        }
    }
    let source=br#"{"info":{},"sdfData":{"a":{"type":"array","items":{"type":"object","pattern":"[","properties":{"full":{"pattern":"("}}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", source)]).unwrap();
    let wire = check(&catalogue, "m");
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(
        patterns(&wire)[0]["primary"]["pointer"],
        "/sdfData/a/items/properties/full/pattern"
    );
}

#[test]
fn full_roles_and_final_choice_and_enum_alternatives_are_traversed() {
    let source=br#"{"info":{},"sdfProperty":{"p":{"pattern":"["}},"sdfData":{"object":{"type":"object","properties":{"text":{"pattern":"("}}},"choice":{"sdfChoice":{"bad":{"pattern":"["},"good":{"pattern":"a"}}},"enum":{"pattern":"a","enum":["a","b"]}},"sdfAction":{"a":{"sdfInputData":{"pattern":"["},"sdfOutputData":{"pattern":"("}}},"sdfEvent":{"e":{"sdfOutputData":{"pattern":"["}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", source)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    assert_eq!(report.outcome().errors(), 6);
    assert_eq!(
        report.assessment_area_status("pattern-syntax/1").present(),
        Some(AssessmentStatus::Assessed)
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    for pointer in [
        "/sdfProperty/p/pattern",
        "/sdfData/object/properties/text/pattern",
        "/sdfData/choice/sdfChoice/bad/pattern",
        "/sdfAction/a/sdfInputData/pattern",
        "/sdfAction/a/sdfOutputData/pattern",
        "/sdfEvent/e/sdfOutputData/pattern",
    ] {
        assert!(
            patterns(&wire)
                .iter()
                .any(|item| item["primary"]["pointer"] == pointer),
            "{pointer}"
        );
    }
}

#[test]
fn unknown_contexts_leave_known_siblings_and_other_sources_available() {
    let unknown =
        br#"{"info":{"features":["urn:unknown:critical"]},"sdfProperty":{"p":{"pattern":"["}}}"#;
    let independent = br#"{"info":{},"sdfProperty":{"p":{"pattern":"("}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("unknown", unknown),
        SourceInput::supplied("independent", independent),
    ])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(
        report.assessment_area_status("pattern-syntax/1").present(),
        Some(AssessmentStatus::Undetermined)
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        !patterns(&wire)
            .iter()
            .any(|item| item["code"] == "pattern.syntax"
                && item["primary"]["root"] == catalogue.source_id("unknown").unwrap().get())
    );
    let source=br##"{"info":{},"sdfProperty":{"unknown":{"sdfRef":"#/sdfData/missing"},"bad":{"pattern":"["}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", source)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert_eq!(
        report.assessment_area_status("pattern-syntax/1").present(),
        Some(AssessmentStatus::Undetermined)
    );
    let source = br#"{"info":{"features":false},"sdfData":{"text":{"pattern":"["}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", source)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(
        report.assessment_area_status("pattern-syntax/1").present(),
        Some(AssessmentStatus::Undetermined)
    );
}

#[test]
fn merged_patterns_declarations_and_existing_consumers_keep_independent_results() {
    use esdieff::{Admission, RecordedRegistryPolicy};
    // One source exercises the public operation boundaries without rewriting
    // escaped pattern text or manufacturing registry observations.
    let model = br##"{"info":{},"sdfData":{"A":{"pattern":"[","unit":"kg","contentFormat":"60"},"B":{"pattern":"(?<x>a)|(?<x>b)","unit":"ms","contentFormat":"application/json@zstd"},"C":{"type":"number","nullable":false,"minimum":3,"maximum":1},"D":{"contentFormat":"text/plain;"},"E":{"pattern":"(","sdfRef":"#/sdfData/Missing"},"F":{"pattern":"a","contentFormat":"application/x-esdieff-unlisted"},"U":{"unit":"1measure:custom-unit"}},"sdfObject":{"O":{"sdfProperty":{"P":{"type":"number","minimum":0,"maximum":5,"const":3}},"sdfRequired":["#/sdfObject/O/sdfProperty/P"]}}}"##;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("authored", model),
        SourceInput::supplied("sample", b"6").sample(),
    ])
    .unwrap();
    for profile in [
        Assessment::Initial,
        Assessment::ExpandedPatternSyntax,
        Assessment::ExpandedContentFormat,
        Assessment::FullBaseline,
    ] {
        let request = CheckRequest::new().assessment(profile);
        let report = catalogue.check(request).unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let diagnostics = report.diagnostics();
        let codes: Vec<_> = diagnostics.conditions().map(|c| c.code()).collect();
        assert!(codes.contains(&"proof.empty_numeric_interval"));
        assert_eq!(
            codes.contains(&"pattern.syntax"),
            matches!(
                profile,
                Assessment::ExpandedPatternSyntax | Assessment::FullBaseline
            )
        );
        assert_eq!(
            codes.contains(&"pattern.unsupported"),
            matches!(
                profile,
                Assessment::ExpandedPatternSyntax | Assessment::FullBaseline
            )
        );
        assert_eq!(
            codes.contains(&"content_format.syntax"),
            matches!(
                profile,
                Assessment::ExpandedContentFormat | Assessment::FullBaseline
            )
        );
        if profile == Assessment::FullBaseline {
            coverage_support::assert_requested_counts(&report);
            assert_eq!(report.requested_coverage().total(), Some(13));
            for area in [
                "pattern-syntax/1",
                "content-format-syntax/1",
                "unit-spelling/1",
            ] {
                assert!(matches!(report.assessment_area_status(area), Field::Absent));
            }
            assert_eq!(
                wire["baseline"]["registry_knowledge"]
                    .as_array()
                    .unwrap()
                    .len(),
                5
            );
            assert!(codes.contains(&"unit.uri_syntax"));
            assert!(report.to_human().contains("esdieff.full-model/1"));
        } else if profile == Assessment::ExpandedPatternSyntax {
            assert_eq!(report.requested_coverage().total(), Some(9));
            assert_eq!(
                report.assessment_area_status("pattern-syntax/1").present(),
                Some(AssessmentStatus::Undetermined)
            );
            assert!(
                wire["baseline"]["registry_knowledge"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        } else if profile == Assessment::ExpandedContentFormat {
            assert_eq!(report.requested_coverage().total(), Some(10));
            assert_eq!(
                report
                    .assessment_area_status("content-format-syntax/1")
                    .present(),
                Some(AssessmentStatus::Undetermined)
            );
            assert_eq!(
                report
                    .assessment_area_status("content-format-registry/1")
                    .present(),
                Some(AssessmentStatus::Undetermined)
            );
        } else {
            assert_eq!(report.requested_coverage().total(), Some(8));
        }
    }
    let withheld = catalogue
        .check(
            CheckRequest::new()
                .assessment(Assessment::FullBaseline)
                .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded),
        )
        .unwrap();
    let diagnostics = withheld.diagnostics();
    let codes: Vec<_> = diagnostics.conditions().map(|c| c.code()).collect();
    for code in [
        "pattern.syntax",
        "pattern.unsupported",
        "content_format.syntax",
        "proof.empty_numeric_interval",
        "unit.uri_syntax",
    ] {
        assert!(codes.contains(&code), "{code}");
    }
    let wire: Value = serde_json::from_str(&withheld.to_json()).unwrap();
    let facts = wire["baseline"]["registry_knowledge"].as_array().unwrap();
    assert_eq!(facts.len(), 4);
    assert!(facts.iter().all(|fact| fact["snapshot"].is_null()));
    assert!(
        catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::ExpandedPatternSyntax)
                    .recorded_registry_policy(RecordedRegistryPolicy::UseRecorded)
            )
            .is_err()
    );

    let sample = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("authored").unwrap(),
                SampleKind::Property,
                "/sdfObject/O/sdfProperty/P",
            )
            .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    assert_eq!(
        sample.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(sample.requested_coverage().total(), Some(9));
    let diagnostics = sample.diagnostics();
    let codes: Vec<_> = diagnostics.conditions().map(|c| c.code()).collect();
    assert!(codes.contains(&"sample.bound") && codes.contains(&"sample.const"));
    assert!(!codes.contains(&"pattern.syntax") && !codes.contains(&"content_format.syntax"));
    let required = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("authored").unwrap(), "/sdfObject/O")
                .view(View::Effective)
                .required_declarations(true),
        )
        .unwrap();
    let wire: Value = serde_json::from_str(&required.to_json()).unwrap();
    assert!(wire["result"]["required_declarations"].is_object());
    assert!(
        !required
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "pattern.syntax")
    );
    assert_eq!(
        catalogue
            .sources()
            .find(|source| source.id() == catalogue.source_id("authored").unwrap())
            .unwrap()
            .raw_bytes(),
        model
    );
}

#[test]
fn inspection_sample_and_other_assessments_keep_their_existing_purposes() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"type":"string","pattern":"["}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#""abc""#).sample(),
    ])
    .unwrap();
    let id = catalogue.source_id("m").unwrap();
    for assessment in [
        Assessment::Initial,
        Assessment::ExpandedModified,
        Assessment::ExpandedUnits,
    ] {
        let report = catalogue
            .check(CheckRequest::new().root(id).assessment(assessment))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(matches!(
            report.assessment_area_status("pattern-syntax/1"),
            Field::Absent
        ));
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|item| item.code().starts_with("pattern."))
        );
    }
    let inspected = catalogue
        .inspect(InspectRequest::new(id, "/sdfProperty/p").view(View::Effective))
        .unwrap();
    assert!(inspected.outcome().complete());
    assert!(matches!(
        inspected.assessment_area_status("pattern-syntax/1"),
        Field::Absent
    ));
    let sample = catalogue
        .check_sample(
            SampleRequest::new(id, SampleKind::Property, "/sdfProperty/p")
                .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(sample.outcome().errors(), 1);
    assert!(matches!(
        sample.assessment_area_status("pattern-syntax/1"),
        Field::Absent
    ));
    assert!(
        sample
            .diagnostics()
            .conditions()
            .any(|item| item.code() == "pattern.syntax")
    );
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", b"{}"),
            SourceInput::failed("unreadable"),
        ],
        Limits::default(),
    )
    .unwrap();
    let selected = catalogue
        .check(
            CheckRequest::new()
                .root(catalogue.source_id("m").unwrap())
                .assessment(Assessment::ExpandedPatternSyntax),
        )
        .unwrap();
    assert_eq!(
        selected
            .assessment_area_status("pattern-syntax/1")
            .present(),
        Some(AssessmentStatus::NotApplicable)
    );
    let all = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    assert_eq!(
        all.assessment_area_status("pattern-syntax/1").present(),
        Some(AssessmentStatus::Undetermined)
    );
}
