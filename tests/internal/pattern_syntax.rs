//! Follows ADR0014: Bounded partial ECMAScript pattern capability, and
//! ADR0028: Bounded effective model pattern syntax.
use super::coverage_support;
use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, Limits, SourceInput};
mod controls {
    include!("../fixtures/pattern-syntax/controls.rs");
}

fn pattern_source(pattern: &str) -> String {
    serde_json::json!({"info":{},"sdfData":{"text":{"pattern":pattern}}}).to_string()
}
fn status(report: &esdieff::Report<'_>) -> AssessmentStatus {
    report
        .assessment_area_status("pattern-syntax/1")
        .present()
        .unwrap()
}

#[test]
fn unbalanced_pattern_is_an_error_without_an_explicit_string_type() {
    let source = br#"{"info":{},"sdfProperty":{"p":{"pattern":"["}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", source)]).unwrap();
    let initial = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(initial.outcome().errors(), 0);
    assert!(matches!(
        initial.assessment_area_status("pattern-syntax/1"),
        Field::Absent
    ));
    for assessment in [Assessment::ExpandedPatternSyntax, Assessment::FullBaseline] {
        let report = catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        let diagnostics = report.diagnostics();
        let condition = diagnostics
            .conditions()
            .find(|condition| condition.code() == "pattern.syntax")
            .unwrap();
        assert_eq!(condition.assessment_unit(), Some("pattern-syntax/1"));
        assert_eq!(condition.basis().kind(), esdieff::BasisKind::Rfc);
        assert!(
            condition
                .basis()
                .citation()
                .unwrap()
                .contains("ECMA-262 2025")
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == "pattern.syntax")
            .unwrap();
        assert_eq!(finding["primary"]["kind"], "effective");
        assert_eq!(finding["primary"]["pointer"], "/sdfProperty/p/pattern");
        assert_eq!(
            finding["related"][0]["source_id"],
            catalogue.source_id("model").unwrap().get()
        );
        let span = finding["related"][0]["span"].as_array().unwrap();
        assert_eq!(
            &source[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
            br#""[""#
        );
        if assessment == Assessment::ExpandedPatternSyntax {
            assert!(matches!(
                report.assessment_area_status("pattern-syntax/1"),
                Field::Present(AssessmentStatus::Assessed)
            ));
            assert_eq!(
                report.requested_coverage().inventory(),
                Some("esdieff.expanded.pattern-syntax/1")
            );
            assert_eq!(report.requested_coverage().total(), Some(9));
        } else {
            assert!(matches!(
                report.assessment_area_status("pattern-syntax/1"),
                Field::Absent
            ));
            assert_eq!(
                report.requested_coverage().inventory(),
                Some("esdieff.full-model/1")
            );
            assert_eq!(report.requested_coverage().total(), Some(13));
            coverage_support::assert_requested_counts(&report);
        }
    }
}

#[test]
fn qualified_unicode_name_and_literal_controls_keep_their_distinctions() {
    for (patterns, expected, errors, code) in [
        (
            controls::INVALID,
            AssessmentStatus::Assessed,
            1,
            Some("pattern.syntax"),
        ),
        (controls::VALID, AssessmentStatus::Assessed, 0, None),
        (
            controls::UNSUPPORTED,
            AssessmentStatus::Undetermined,
            0,
            Some("pattern.unsupported"),
        ),
    ] {
        for pattern in patterns {
            let source = pattern_source(pattern);
            let catalogue =
                Catalogue::from_inputs(&[SourceInput::supplied("m", source.as_bytes())]).unwrap();
            let report = catalogue
                .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
                .unwrap();
            assert_eq!(status(&report), expected, "{pattern}");
            assert_eq!(report.outcome().errors(), errors, "{pattern}");
            if let Some(code) = code {
                let diagnostics = report.diagnostics();
                let finding = diagnostics
                    .conditions()
                    .find(|item| item.code() == code)
                    .unwrap();
                assert_eq!(finding.assessment_unit(), Some("pattern-syntax/1"));
                if errors == 0 {
                    assert_eq!(finding.basis().kind(), esdieff::BasisKind::Operational);
                    assert_eq!(finding.basis().id(), Some("pattern-syntax-capability"));
                }
            }
        }
    }
    for pattern in controls::matching_only_capacity_controls() {
        let source = pattern_source(&pattern);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("m", source.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        assert_eq!(status(&report), AssessmentStatus::Assessed);
        assert_eq!(report.outcome().errors(), 0);
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|item| item.assessment_unit() == Some("pattern-syntax/1"))
        );
    }
}

#[test]
fn reducing_capacities_and_decimal_endpoints_leave_incomplete_capability() {
    for (pattern, limits) in [
        ("éé", Limits::default().pattern_bytes(3)),
        ("((a))", Limits::default().pattern_nesting(1)),
        ("a|b|c", Limits::default().pattern_alternatives(2)),
        ("a{1000000}", Limits::default()),
        ("a{1,1000000}", Limits::default()),
        ("a{00001000000,}", Limits::default()),
    ] {
        let source = pattern_source(pattern);
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("m", source.as_bytes())], limits)
                .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        assert_eq!(status(&report), AssessmentStatus::Undetermined, "{pattern}");
        assert_eq!(report.outcome().errors(), 0);
        let diagnostics = report.diagnostics();
        let limit = diagnostics
            .conditions()
            .find(|item| {
                item.code() == "operation.limit"
                    && item.assessment_unit() == Some("pattern-syntax/1")
            })
            .unwrap();
        assert_eq!(limit.basis().id(), Some("pattern-syntax-capability"));
    }
    for pattern in ["a{999999999999999999999999999,2}", "a{00002,00001}"] {
        let source = pattern_source(pattern);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("m", source.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        assert_eq!(status(&report), AssessmentStatus::Assessed);
        assert_eq!(report.outcome().errors(), 1);
    }
}

#[test]
fn absent_and_nonstring_patterns_have_no_string_syntax_operation() {
    for value in ["false", "null", "3", "{}", "[]"] {
        let source = format!(r#"{{"info":{{}},"sdfData":{{"text":{{"pattern":{value}}}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("m", source.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        assert_eq!(status(&report), AssessmentStatus::NotApplicable);
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .filter(|item| item.code() == "data.string")
                .count(),
            1
        );
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|item| item.code().starts_with("pattern."))
        );
    }
    let source = br#"{"info":{},"sdfData":{"opaque":{"const":{"pattern":"["},"default":{"pattern":"["}}},"x:extension":{"pattern":"["}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", source)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    assert_eq!(status(&report), AssessmentStatus::NotApplicable);
    assert_eq!(report.outcome().errors(), 0);
}

#[test]
fn full_joins_pattern_findings_with_known_requested_membership() {
    let source = br#"{"info":{},"sdfData":{"bad":{"pattern":"["},"alsoBad":{"pattern":"("},"unsupported":{"pattern":"(?<x>a)|(?<x>b)"},"large":{"pattern":"abcdefghijklmnopqrstuvwxyz"}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("m", source)],
        Limits::default().pattern_bytes(20),
    )
    .unwrap();
    let full = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_eq!(full.outcome().errors(), 2);
    coverage_support::assert_requested_counts(&full);
    assert_eq!(full.requested_coverage().total(), Some(13));
    assert!(matches!(
        full.assessment_area_status("pattern-syntax/1"),
        Field::Absent
    ));
    let expanded = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    assert_eq!(status(&expanded), AssessmentStatus::Undetermined);
    assert_eq!(expanded.outcome().errors(), 2);
    coverage_support::assert_requested_counts(&expanded);
}

#[test]
fn findings_and_status_survive_count_byte_and_pointer_omission() {
    let source = br#"{"info":{},"sdfData":{"bad":{"multipleOf":0,"pattern":"["},"unknown":{"pattern":"(?<x>a)|(?<x>b)"}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().diagnostic_bytes(1),
        Limits::default().conditions(1),
        Limits::default().pointer_bytes(1).pointer_output_bytes(1),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("m", source)], limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        assert_eq!(report.outcome().errors(), 2);
        assert_eq!(status(&report), AssessmentStatus::Undetermined);
        assert_eq!(report.requested_coverage().total(), Some(9));
        coverage_support::assert_requested_counts(&report);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["outcome"]["errors"], 2);
        coverage_support::assert_wire_counts(&wire);
        assert!(
            report
                .to_human()
                .contains("Assessment area pattern-syntax/1: undetermined")
        );
    }
}

#[test]
fn original_work_is_shared_by_successive_patterns() {
    let pattern = "a".repeat(400);
    let single = pattern_source(&pattern);
    let double =
        serde_json::json!({"info":{},"sdfData":{"a":{"pattern":pattern},"b":{"pattern":pattern}}})
            .to_string();
    let assessed = |source: &str, work| {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("m", source.as_bytes())],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        status(&report) == AssessmentStatus::Assessed
    };
    let (mut lo, mut hi) = (1000, 20000);
    assert!(assessed(&single, hi));
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if assessed(&single, mid) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    assert!(assessed(&single, lo));
    assert!(!assessed(&double, lo));
    assert!(assessed(&double, 20000));
    println!(
        "one_pattern_minimum_operation_work={lo}; two_patterns_at_same_work=undetermined; no per-pattern reset"
    );
}

#[test]
fn failed_optional_finding_growth_preserves_an_established_invalidity() {
    let source = pattern_source("[");
    let evaluate = |allocation| {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("m", source.as_bytes())],
            Limits::default().effective_allocation_bytes(allocation),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
            .unwrap();
        (
            status(&report),
            report.outcome().errors(),
            report
                .diagnostics()
                .conditions()
                .filter(|item| item.code() == "pattern.syntax")
                .count(),
            report
                .diagnostics()
                .conditions()
                .filter(|item| item.code() == "operation.limit")
                .count(),
        )
    };
    let (mut lo, mut hi) = (1, 100000);
    assert_eq!(evaluate(hi).0, AssessmentStatus::Assessed);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if evaluate(mid).0 == AssessmentStatus::Assessed {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let result = evaluate(lo);
    assert_eq!(result, (AssessmentStatus::Assessed, 1, 0, 0));
    assert_eq!(evaluate(lo - 1).0, AssessmentStatus::Undetermined);
    println!(
        "original_allocation_at_proved_but_omitted_finding={lo}; errors=1; area=assessed; synthetic_operation_limits=0"
    );
}

#[test]
fn additive_report_reserve_and_reducing_configuration_are_exact() {
    for (assessment, extra) in [
        (Assessment::ExpandedPatternSyntax, 171),
        (Assessment::FullBaseline, 31448 + 171),
    ] {
        let cap = 6 + 4096 + 16384 + extra;
        let small = Catalogue::with_limits(
            &[SourceInput::supplied("m", b"{}")],
            Limits::default().report_bytes(cap - 1),
        )
        .unwrap();
        assert!(
            small
                .check(CheckRequest::new().assessment(assessment))
                .is_err()
        );
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("m", b"{}")],
            Limits::default().report_bytes(cap),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["diagnostics"]["retained"], 0);
        assert!(report.to_json().len() <= cap && report.to_human().len() <= cap);
    }
    for limits in [
        Limits::default().pattern_bytes(0),
        Limits::default().pattern_bytes(4097),
        Limits::default().pattern_nesting(0),
        Limits::default().pattern_nesting(65),
        Limits::default().pattern_alternatives(0),
        Limits::default().pattern_alternatives(513),
    ] {
        assert!(Catalogue::with_limits(&[SourceInput::supplied("m", b"{}")], limits).is_err());
    }
}
