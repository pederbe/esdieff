use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, Report, SourceInput};

fn full(text: &[u8], limits: Limits) -> Catalogue {
    Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap()
}
fn check(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap()
}
fn status(report: &Report<'_>, member: u16) -> AssessmentStatus {
    report
        .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
        .require()
        .unwrap()
}

#[test]
fn compound_meaning_is_established_without_a_sample() {
    let catalogue = full(
        br#"{"info":{},"sdfData":{"D":{
        "type":"array","minItems":0,"maxItems":20,"uniqueItems":true,
        "items":{"type":"object","required":["key","free"],"properties":{
            "key":{"type":"integer","minimum":0},
            "optional":{"type":"array","items":{"type":"string","minLength":1}}
        }}
    }}}"#,
        Limits::default(),
    );
    let report = check(&catalogue);
    for member in [42, 45, 46, 50, 51, 52, 53, 54] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert_eq!(report.outcome().errors(), 0);
}

#[test]
fn compound_const_uses_its_contextual_subtree_without_proving_satisfiability() {
    let catalogue = full(
        br#"{"info":{},"sdfData":{"D":{
        "type":"object","required":["missing"],"const":{"n":[1.0,"text"]},
        "properties":{"n":{"type":"array","items":{"sdfChoice":{
            "number":{"type":"number"},"text":{"type":"string"}
        }}}}
    }}}"#,
        Limits::default(),
    );
    let report = check(&catalogue);
    for member in [42, 43, 50, 53, 54] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
}

#[test]
fn optional_descriptions_still_require_model_capability() {
    for (quality, member) in [("properties", 53), ("items", 50)] {
        let source = if quality == "properties" {
            r#"{"info":{},"sdfData":{"D":{"type":"object","properties":{"optional":{"type":"future"}}}}}"#.to_owned()
        } else {
            r#"{"info":{},"sdfData":{"D":{"type":"array","items":{"type":"future"}}}}"#.to_owned()
        };
        let catalogue = full(source.as_bytes(), Limits::default());
        let report = check(&catalogue);
        assert_eq!(status(&report, member), AssessmentStatus::Undetermined);
    }
}

#[test]
fn wider_direct_items_do_not_gain_full_compound_meaning() {
    let catalogue = full(
        br#"{"info":{},"sdfData":{"D":{
        "type":"array","items":{"type":"array","items":{"type":"number"}}
    }}}"#,
        Limits::default(),
    );
    let report = check(&catalogue);
    assert_eq!(status(&report, 41), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 42), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 50), AssessmentStatus::Undetermined);
}

#[test]
fn uniqueness_requires_every_final_context_and_false_needs_no_comparison() {
    for (unique, expected) in [
        (true, AssessmentStatus::Undetermined),
        (false, AssessmentStatus::Assessed),
    ] {
        let source = format!(
            r#"{{"info":{{}},"sdfData":{{"D":{{"type":"array","uniqueItems":{unique},"items":{{"sdfChoice":{{"text":{{"type":"string"}},"future":{{"type":"string","format":"future"}}}}}}}}}}}}"#
        );
        let catalogue = full(source.as_bytes(), Limits::default());
        let report = check(&catalogue);
        assert_eq!(status(&report, 52), expected, "{source}");
    }
}

#[test]
fn required_names_do_not_need_property_descriptions_or_close_the_map() {
    let catalogue = full(br#"{"info":{},"sdfData":{"D":{
        "type":"object","required":["free","free"],"properties":{},"const":{"free":null,"extra":true}
    }}}"#, Limits::default());
    let report = check(&catalogue);
    for member in [42, 43, 53, 54] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
}

#[test]
fn unavailable_const_leaf_does_not_erase_known_count_meaning() {
    let catalogue = full(
        br#"{"info":{},"sdfData":{"D":{
        "type":"array","minItems":0,"const":[12345],"items":{"type":"number"}
    }}}"#,
        Limits::default().number_token_bytes(2),
    );
    let report = check(&catalogue);
    assert_eq!(status(&report, 43), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 51), AssessmentStatus::Assessed);
}

#[test]
fn contextual_const_keeps_abstract_denotation_limits() {
    let catalogue = full(
        br#"{"info":{},"sdfData":{"D":{
        "type":"object","const":{"stamp":"not-a-date"},
        "properties":{"stamp":{"type":"string","format":"date-time"}}
    }}}"#,
        Limits::default(),
    );
    let report = check(&catalogue);
    assert_eq!(status(&report, 43), AssessmentStatus::Undetermined);
}

#[test]
fn compound_meaning_uses_one_equality_allowance_across_constants() {
    let source = br#"{"info":{},"sdfData":{
        "a":{"type":"array","const":[1]},
        "b":{"type":"array","const":[2]}
    }}"#;
    let ordinary = full(source, Limits::default());
    assert_eq!(status(&check(&ordinary), 43), AssessmentStatus::Assessed);
    let limited = full(source, Limits::default().equality_pairs(1));
    let report = check(&limited);
    assert_eq!(status(&report, 43), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
}

#[test]
fn parent_capability_keeps_its_result_when_a_later_constant_visit_exhausts_equality() {
    let source = br#"{"info":{},"sdfData":{"D":{
        "type":"object","properties":{
            "value":{"type":"array","const":[1],"minItems":0}
        }
    }}}"#;
    let ordinary = full(source, Limits::default());
    let report = check(&ordinary);
    for member in [35, 42, 43, 51, 53] {
        assert_eq!(status(&report, member), AssessmentStatus::Assessed);
    }
    assert!(report.outcome().complete());

    for detail in [Limits::default(), Limits::default().diagnostic_bytes(1)] {
        let limited = full(source, detail.equality_pairs(1));
        let report = check(&limited);
        assert_eq!(status(&report, 43), AssessmentStatus::Undetermined);
        // The parent's earlier aggregation completed, and independent local
        // type, count and formal-admission observations still finish.
        for member in [35, 42, 51, 53] {
            assert_eq!(status(&report, member), AssessmentStatus::Assessed);
        }
        assert_eq!(report.outcome().errors(), 0);
        assert!(!report.outcome().complete());
    }
}

#[test]
fn discarded_compound_const_and_composed_optional_properties_keep_final_meaning() {
    let catalogue = full(br##"{"info":{},"sdfData":{
        "base":{"type":"object","properties":{"key":{"type":"number"}}},
        "D":{"sdfRef":"#/sdfData/base","const":{"key":2},"properties":{"optional":{"type":"string"}}},
        "choice":{"type":"array","const":["bad"],"sdfChoice":{"one":{"const":[1]},"two":{"const":[2]}}}
    }}"##,Limits::default());
    let report = check(&catalogue);
    for member in [42, 43, 53, 54] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
}

#[test]
fn report_detail_omission_does_not_change_compound_capability() {
    let source = br#"{"info":{},"sdfData":{"D":{"type":"array","uniqueItems":true,"items":{"type":"object","properties":{"n":{"type":"number"}}}},"Value":{"type":"object","const":{"n":1}}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().conditions(1).diagnostic_bytes(1),
    ] {
        let catalogue = full(source, limits);
        let report = check(&catalogue);
        for member in [42, 43, 50, 52, 53, 54] {
            assert_eq!(
                status(&report, member),
                AssessmentStatus::Assessed,
                "M{member}"
            );
        }
    }
}
