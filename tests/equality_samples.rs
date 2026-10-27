//! Contextual equality follows ADR0015: Bounded value equality in a resolved data context.
mod map_support;
use esdieff::{Admission, AssessmentStatus, Catalogue, ContextState, JsonBuilder, SourceInput};
use map_support::selected;

fn check(
    qualities: &str,
    sample: &str,
    admission: Admission,
    area: &str,
    status: AssessmentStatus,
) {
    let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", sample.as_bytes()).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        admission,
        "{qualities} {sample}\n{}",
        report.to_json()
    );
    assert_eq!(
        report.assessment_area_status(area).require().unwrap(),
        status,
        "{qualities} {sample}"
    );
    match admission {
        Admission::Admitted => assert!(report.outcome().complete(), "{qualities} {sample}"),
        Admission::Undetermined => assert!(!report.outcome().complete()),
        Admission::Rejected => {}
        _ => unreachable!(),
    }
    assert_eq!(report.requested_coverage().total(), Some(9));
    let coverage = report.requested_coverage();
    assert_eq!(
        coverage.assessed()
            + coverage.not_applicable()
            + coverage.unassessed()
            + coverage.undetermined(),
        9
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["result"]["admission"], admission.as_str());
    let human = report.to_human();
    for row in wire["coverage"]["rows"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let observed = report.assessment_area_status(id).require().unwrap();
        assert_eq!(row["status"], observed.as_str());
        assert!(human.contains(&format!("Sample area {id}: {}", observed.as_str())));
    }
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
}

#[test]
fn ordinary_equality_does_not_require_an_authored_type_quality() {
    for (qualities, sample, admission) in [
        (
            r#"{"const":[1,{"n":10}]}"#,
            r#"[1.0,{"n":1e1}]"#,
            Admission::Admitted,
        ),
        (r#"{"const":[1,2]}"#, "[2,1]", Admission::Rejected),
        (
            r#"{"const":{"a":1,"b":null}}"#,
            r#"{"b":null,"\u0061":1.0}"#,
            Admission::Admitted,
        ),
        (r#"{"const":{"a":null}}"#, "{}", Admission::Rejected),
    ] {
        check(
            qualities,
            sample,
            admission,
            "sample-const/1",
            AssessmentStatus::Assessed,
        );
    }
}

#[test]
fn ordinary_untyped_uniqueness_has_its_own_public_controls() {
    for (sample, admission) in [
        ("[]", Admission::Admitted),
        ("[1]", Admission::Admitted),
        ("[10,1e1]", Admission::Rejected),
        (r#"[{"n":1},{"n":1.0}]"#, Admission::Rejected),
        ("[[1,2],[2,1]]", Admission::Admitted),
    ] {
        check(
            r#"{"uniqueItems":true}"#,
            sample,
            admission,
            "sample-array/1",
            AssessmentStatus::Assessed,
        );
    }
    for qualities in [
        r#"{"uniqueItems":true,"items":{}}"#,
        r#"{"uniqueItems":true,"items":{"type":"number"}}"#,
    ] {
        check(
            qualities,
            "[1,1]",
            Admission::Undetermined,
            "sample-array/1",
            AssessmentStatus::Undetermined,
        );
    }
}

#[test]
fn unavailable_untyped_counts_survive_uniqueness_and_independent_rejection() {
    let cases: [(&str, &str, &[&str], Admission); 3] = [
        (
            r#"{"uniqueItems":true,"minItems":2}"#,
            "[]",
            &[],
            Admission::Undetermined,
        ),
        (
            r#"{"uniqueItems":true,"maxItems":1}"#,
            "[1,2]",
            &["1", "2"],
            Admission::Undetermined,
        ),
        (
            r#"{"uniqueItems":true,"maxItems":1}"#,
            "[1,1]",
            &["1", "1"],
            Admission::Rejected,
        ),
    ];
    for (qualities, sample, numbers, admission) in cases {
        let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
        for typed in [false, true] {
            let builder = JsonBuilder::new();
            let input = if typed {
                SourceInput::typed(
                    "s",
                    builder
                        .array(numbers.iter().map(|n| builder.number(n).unwrap()).collect())
                        .unwrap(),
                )
            } else {
                SourceInput::supplied("s", sample.as_bytes()).sample()
            };
            let catalogue =
                Catalogue::take_inputs(vec![SourceInput::supplied("m", model.as_bytes()), input])
                    .unwrap();
            let report = selected(&catalogue);
            assert_eq!(report.sample_result().unwrap().admission(), admission);
            assert_eq!(
                report.sample_result().unwrap().context_state(),
                ContextState::Partial
            );
            assert!(!report.outcome().complete());
            assert_eq!(
                report
                    .assessment_area_status("sample-array/1")
                    .require()
                    .unwrap(),
                AssessmentStatus::Undetermined
            );
            assert_eq!(report.requested_coverage().total(), Some(9));
            let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["result"]["admission"], admission.as_str());
            assert_eq!(wire["result"]["context"]["state"], "partial");
            assert_eq!(wire["outcome"]["complete"], false);
            assert_eq!(
                report.outcome().errors(),
                u32::from(admission == Admission::Rejected)
            );
            assert!(
                report
                    .to_human()
                    .contains("Sample area sample-array/1: undetermined")
            );
        }
    }
}

#[test]
fn bare_containers_complete_without_a_constant_or_active_uniqueness() {
    for (qualities, sample, area) in [
        ("{}", "[]", "sample-array/1"),
        ("{}", "[1,{}]", "sample-array/1"),
        ("{}", "{}", "sample-map/1"),
        ("{}", r#"{"opaque":[1,null]}"#, "sample-map/1"),
        (r#"{"uniqueItems":false}"#, "[1,1]", "sample-array/1"),
    ] {
        check(
            qualities,
            sample,
            Admission::Admitted,
            area,
            AssessmentStatus::Assessed,
        );
    }
}

#[test]
fn structured_constants_compare_exact_numbers_decoded_text_and_order() {
    for (qualities, sample, expected) in [
        (
            r#"{"type":"array","const":[10,-0,"on"]}"#,
            r#"[1e1,0,"\u006fn"]"#,
            Admission::Admitted,
        ),
        (
            r#"{"type":"array","const":[1,2]}"#,
            "[2,1]",
            Admission::Rejected,
        ),
        (
            r#"{"type":"array","const":[0.1]}"#,
            "[0.10000000000000001]",
            Admission::Rejected,
        ),
        (
            r#"{"type":"object","const":{"a":9007199254740992,"b":true}}"#,
            r#"{"b":true,"\u0061":9007199254740992}"#,
            Admission::Admitted,
        ),
        (
            r#"{"type":"object","const":{"a":9007199254740992}}"#,
            r#"{"a":9007199254740993}"#,
            Admission::Rejected,
        ),
        (
            r#"{"type":"object","const":{"a":null}}"#,
            "{}",
            Admission::Rejected,
        ),
        (
            r#"{"type":"object","const":{}}"#,
            r#"{"a":null}"#,
            Admission::Rejected,
        ),
        (
            r#"{"type":"array","const":[[1],{"x":[true,"é"]}]}"#,
            r#"[[1.0],{"x":[true,"e\u0301"]}]"#,
            Admission::Rejected,
        ),
    ] {
        check(
            qualities,
            sample,
            expected,
            "sample-const/1",
            AssessmentStatus::Assessed,
        );
    }
}

#[test]
fn uniqueness_compares_repeated_scalars_and_structured_values() {
    for (sample, expected) in [
        ("[10,1e1]", Admission::Rejected),
        ("[-0,0]", Admission::Rejected),
        (r#"["on","\u006fn"]"#, Admission::Rejected),
        ("[9007199254740992,9007199254740993]", Admission::Admitted),
        (
            r#"[{"a":1,"b":null},{"b":null,"\u0061":1.0}]"#,
            Admission::Rejected,
        ),
        ("[[1,2],[2,1]]", Admission::Admitted),
        (r#"[{}, {"a":null}]"#, Admission::Admitted),
        ("[true,1,null]", Admission::Admitted),
    ] {
        check(
            r#"{"type":"array","uniqueItems":true}"#,
            sample,
            expected,
            "sample-array/1",
            AssessmentStatus::Assessed,
        );
    }
}

#[test]
fn vacuous_or_disabled_uniqueness_does_not_require_value_equality() {
    for (qualities, sample, expected, status) in [
        (
            r#"{"type":"array","uniqueItems":true}"#,
            "[]",
            Admission::Admitted,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"array","uniqueItems":true}"#,
            "[{}]",
            Admission::Admitted,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"array","uniqueItems":false}"#,
            "[1,1]",
            Admission::Admitted,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"array","uniqueItems":true,"items":{"format":"future-format"}}"#,
            "[]",
            Admission::Admitted,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"array","uniqueItems":true,"items":{"type":"string","format":"future-format"}}"#,
            r#"["not-a-date"]"#,
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
    ] {
        check(qualities, sample, expected, "sample-array/1", status);
    }
}

#[test]
fn unavailable_leaf_meaning_preserves_independent_compound_differences() {
    for (qualities, sample, admission, status) in [
        (
            r#"{"type":"object","properties":{"stamp":{"type":"string","format":"future-format"}},"const":{"stamp":"not-a-date","n":1}}"#,
            r#"{"stamp":"not-a-date","n":1}"#,
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"type":"object","properties":{"stamp":{"type":"string","format":"future-format"}},"const":{"stamp":"not-a-date","n":1}}"#,
            r#"{"stamp":"not-a-date","n":2}"#,
            Admission::Rejected,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"object","properties":{"stamp":{"type":"string","format":"future-format"}},"const":{"stamp":"not-a-date"}}"#,
            r#"{"stamp":"not-a-date","extra":null}"#,
            Admission::Rejected,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"items":{"format":"future-format"},"const":["not-a-date"]}"#,
            r#"["not-a-date"]"#,
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"properties":{"stamp":{"format":"future-format"}},"const":{"stamp":"not-a-date"}}"#,
            r#"{"stamp":"not-a-date"}"#,
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"items":{},"const":[1]}"#,
            "[]",
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"properties":{"stamp":{}},"const":{"stamp":1}}"#,
            "{}",
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
    ] {
        check(qualities, sample, admission, "sample-const/1", status);
    }
}

#[test]
fn one_equal_pair_disproves_uniqueness_despite_other_unknown_pairs() {
    let qualities = r#"{"type":"array","uniqueItems":true,"items":{"type":"object","properties":{"stamp":{"type":"string","format":"future-format"}}}}"#;
    check(
        qualities,
        r#"[{"stamp":"not-a-date"},{"n":1},{"n":1.0}]"#,
        Admission::Rejected,
        "sample-array/1",
        AssessmentStatus::Undetermined,
    );
    check(
        qualities,
        r#"[{"stamp":"not-a-date"},{"stamp":"not-a-date"},{"n":1}]"#,
        Admission::Undetermined,
        "sample-array/1",
        AssessmentStatus::Undetermined,
    );
}

#[test]
fn choice_admission_does_not_choose_compound_equality_meaning() {
    let qualities = r#"{"type":"object","const":{"x":"not-a-date"},"sdfChoice":{"plain":{},"dated":{"properties":{"x":{"type":"string","format":"date-time"}}}}}"#;
    check(
        qualities,
        r#"{"x":"not-a-date"}"#,
        Admission::Undetermined,
        "sample-const/1",
        AssessmentStatus::Undetermined,
    );
    let override_constant =
        r#"{"type":"array","const":[1],"sdfChoice":{"original":{},"override":{"const":[2]}}}"#;
    check(
        override_constant,
        "[2]",
        Admission::Admitted,
        "sample-const/1",
        AssessmentStatus::Assessed,
    );
}

#[test]
fn unavailable_root_reference_type_and_extension_meanings_do_not_supply_equality() {
    for qualities in [
        r#"{"type":"magic","const":[]}"#,
        r##"{"type":"array","const":[],"sdfRef":"missing:#/sdfData/X"}"##,
        r#"{"type":"array","uniqueItems":true,"items":{"sdfType":"byte-string"}}"#,
    ] {
        let sample = if qualities.contains("uniqueItems") {
            "[1,1]"
        } else {
            "[]"
        };
        let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined,
            "{qualities}"
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(
            !wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["domain"] == "sample" && row["severity"] == "error")
        );
        assert!(!report.outcome().complete());
    }
    // Undeclared extension vocabulary leaves the array equality intact, following
    // ADR 0037: Undeclared extension vocabulary.
    check(
        r#"{"type":"array","const":[],"x:meaning":1}"#,
        "[]",
        Admission::Admitted,
        "sample-const/1",
        AssessmentStatus::Assessed,
    );
    let model = br#"{"info":{"features":["urn:unknown:critical"]},"sdfProperty":{"p":{"type":"array","const":[],"uniqueItems":true}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", b"[]").sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!report.outcome().complete());
}

#[test]
fn whole_description_choices_preserve_nested_equality_meanings() {
    let qualities = r#"{"type":"object","sdfChoice":{"plain":{"properties":{"x":{"type":"object","const":{"stamp":"not-a-date"}}}},"dated":{"properties":{"x":{"type":"object","const":{"stamp":"not-a-date"},"properties":{"stamp":{"type":"string","format":"date-time"}}}}}}}"#;
    check(
        qualities,
        r#"{"x":{"stamp":"not-a-date"}}"#,
        Admission::Undetermined,
        "sample-const/1",
        AssessmentStatus::Undetermined,
    );
}

#[test]
fn enclosing_choices_preserve_ordinary_and_unknown_nested_comparisons() {
    for (qualities, sample, admission, area, status) in [
        (
            r#"{"type":"object","sdfChoice":{"one":{"properties":{"x":{"type":"array","const":[1]}}},"two":{"properties":{"x":{"type":"array","const":[1.0]}}}}}"#,
            r#"{"x":[1e0]}"#,
            Admission::Admitted,
            "sample-const/1",
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"object","sdfChoice":{"plain":{"properties":{"x":{"type":"array","uniqueItems":true}}},"abstract":{"properties":{"x":{"type":"array","uniqueItems":true,"items":{"type":"string","format":"date-time"}}}}}}"#,
            r#"{"x":["not-a-date","not-a-date"]}"#,
            Admission::Undetermined,
            "sample-array/1",
            AssessmentStatus::Undetermined,
        ),
        (
            r#"{"type":"object","sdfChoice":{"plain":{"properties":{"x":{"type":"array","uniqueItems":true}}},"ordinary":{"properties":{"x":{"type":"array","uniqueItems":true,"items":{"type":"number"}}}}}}"#,
            r#"{"x":[10,1e1]}"#,
            Admission::Rejected,
            "sample-array/1",
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"object","sdfChoice":{"plain":{"properties":{"x":{"type":"object","const":{"stamp":"not-a-date","n":1}}}},"dated":{"properties":{"x":{"type":"object","const":{"stamp":"not-a-date","n":1},"properties":{"stamp":{"type":"string","format":"date-time"}}}}}}}"#,
            r#"{"x":{"stamp":"not-a-date","n":2}}"#,
            Admission::Rejected,
            "sample-const/1",
            AssessmentStatus::Assessed,
        ),
    ] {
        check(qualities, sample, admission, area, status);
    }
}

#[test]
fn local_and_enclosing_choices_project_through_decoded_members_and_array_elements() {
    for (leaf, sample, admission, status) in [
        (
            r#"{"type":"object","const":{"stamp":10},"sdfChoice":{"one":{},"two":{"properties":{"stamp":{"type":"number"}}}}}"#,
            r#"{"box":[{"a\u002fb~c":{"stamp":1e1}}]}"#,
            Admission::Admitted,
            AssessmentStatus::Assessed,
        ),
        (
            r#"{"type":"object","const":{"stamp":"not-a-date"},"sdfChoice":{"plain":{},"dated":{"properties":{"stamp":{"type":"string","format":"date-time"}}}}}"#,
            r#"{"box":[{"a\u002fb~c":{"stamp":"not-a-date"}}]}"#,
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
    ] {
        let leaf: serde_json::Value = serde_json::from_str(leaf).unwrap();
        let container = serde_json::json!({"properties":{"box":{"type":"array","items":{"type":"object","properties":{"a/b~c":leaf}}}}});
        let qualities =
            serde_json::json!({"type":"object","sdfChoice":{"one":container,"two":container}})
                .to_string();
        check(&qualities, sample, admission, "sample-const/1", status);
    }
}

#[test]
fn projection_distinguishes_absent_children_from_unavailable_enclosing_meaning() {
    for (other, admission, status) in [
        ("{}", Admission::Admitted, AssessmentStatus::Assessed),
        (
            r##"{"properties":{"x":{"type":"number","sdfRef":"missing:#/sdfData/X"}}}"##,
            Admission::Undetermined,
            AssessmentStatus::Undetermined,
        ),
        // Undeclared extension vocabulary supplies no other meaning, following
        // ADR 0037: Undeclared extension vocabulary.
        (
            r#"{"x:meaning":1}"#,
            Admission::Admitted,
            AssessmentStatus::Assessed,
        ),
    ] {
        let qualities = format!(
            r#"{{"type":"object","sdfChoice":{{"plain":{{"properties":{{"x":{{"type":"number","const":10}}}}}},"other":{other}}}}}"#
        );
        check(
            &qualities,
            r#"{"x":1e1}"#,
            admission,
            "sample-const/1",
            status,
        );
    }
    check(
        r#"{"sdfChoice":{"one":{"properties":{"x":{"const":10}}},"two":{}}}"#,
        r#"{"x":1e1}"#,
        Admission::Admitted,
        "sample-const/1",
        AssessmentStatus::NotApplicable,
    );
    check(
        r#"{"sdfChoice":{"one":{"properties":{"x":{"const":10}}},"two":{"properties":{"x":{"const":10}}}}}"#,
        r#"{"x":1e1}"#,
        Admission::Undetermined,
        "sample-map/1",
        AssessmentStatus::Undetermined,
    );
    check(
        r#"{"sdfChoice":{"one":{"properties":{"x":{"const":10}}},"two":{"properties":{"x":{"const":10}}}}}"#,
        r#"{"x":1e1}"#,
        Admission::Undetermined,
        "sample-const/1",
        AssessmentStatus::NotApplicable,
    );
}

#[test]
fn projected_unknown_children_preserve_established_outer_differences() {
    for (leaf, sample) in [
        (
            r#"{"type":"array","const":["not-a-date"],"items":{"type":"string","format":"date-time"}}"#,
            r#"{"x":["not-a-date","extra"]}"#,
        ),
        (
            r#"{"type":"object","const":{"stamp":"not-a-date"},"properties":{"stamp":{"type":"string","format":"date-time"}}}"#,
            r#"{"x":{"stamp":"not-a-date","extra":null}}"#,
        ),
    ] {
        let leaf: serde_json::Value = serde_json::from_str(leaf).unwrap();
        let branch = serde_json::json!({"properties":{"x":leaf}});
        let qualities =
            serde_json::json!({"type":"object","sdfChoice":{"one":branch,"two":branch}})
                .to_string();
        check(
            &qualities,
            sample,
            Admission::Rejected,
            "sample-const/1",
            AssessmentStatus::Assessed,
        );
    }
}
