//! Shared equality budgets and real sample locations follow ADR0015: Bounded value equality
//! in a resolved data context, and ADR0016: Selected samples with explicit admission and coverage.
use super::map_support;
use esdieff::{
    Admission, AssessmentStatus, Catalogue, JsonBuilder, Limits, SampleKind, SampleRequest,
    SourceInput,
};
use map_support::selected;
use serde_json::{Value, json};

fn catalogue(qualities: &str, sample: &str, limits: Limits) -> Catalogue {
    let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
    Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ],
        limits,
    )
    .unwrap()
}

#[test]
fn pair_limits_are_request_wide_and_validate_before_acquisition() {
    for pairs in [0, 100_001, usize::MAX] {
        assert_eq!(
            Catalogue::with_limits(
                &[SourceInput::file(
                    "m",
                    std::path::Path::new("/missing-equality-input")
                )],
                Limits::default().equality_pairs(pairs)
            )
            .err(),
            Some("configuration exceeds supported finite ceilings")
        );
    }
    for (qualities, sample, needed) in [
        (
            r#"{"type":"array","const":[1,2],"uniqueItems":true}"#,
            "[1.0,2.0]",
            2,
        ),
        (
            r#"{"type":"object","properties":{"a":{"const":1},"b":{"const":2}}}"#,
            r#"{"a":1,"b":2}"#,
            2,
        ),
        (
            r#"{"type":"boolean","sdfChoice":{"a":{"const":false},"b":{"const":true}}}"#,
            "true",
            2,
        ),
        (r#"{"type":"array","uniqueItems":true}"#, "[1,2,3]", 3),
        (
            r#"{"type":"object","sdfChoice":{"one":{"properties":{"x":{"type":"array","const":[9]}}},"two":{"properties":{"x":{"type":"array","const":[10]}}}}}"#,
            r#"{"x":[1e1]}"#,
            2,
        ),
    ] {
        let limited = catalogue(
            qualities,
            sample,
            Limits::default().equality_pairs(needed - 1),
        );
        let report = selected(&limited);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined,
            "{qualities}"
        );
        assert!(!report.outcome().complete());
        let exact = catalogue(qualities, sample, Limits::default().equality_pairs(needed));
        for _ in 0..2 {
            assert_eq!(
                selected(&exact).sample_result().unwrap().admission(),
                Admission::Admitted
            );
        }
    }
}

#[test]
fn vacuous_and_disabled_uniqueness_do_not_spend_pairs_needed_by_constants() {
    for (qualities, sample) in [
        (r#"{"type":"array","const":[],"uniqueItems":true}"#, "[]"),
        (
            r#"{"type":"array","uniqueItems":true,"items":{"sdfChoice":{"one":{"const":1}}}}"#,
            "[1]",
        ),
        (
            r#"{"type":"array","const":[1,1],"uniqueItems":false}"#,
            "[1,1]",
        ),
        (r#"{"type":"array","const":[1,1]}"#, "[1,1]"),
    ] {
        let input = catalogue(qualities, sample, Limits::default().equality_pairs(1));
        let report = selected(&input);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted,
            "{qualities}"
        );
        assert!(report.outcome().complete());
    }
}

#[test]
fn exact_original_counter_boundaries_leave_affected_equality_unfinished() {
    for (path, qualities, sample, area) in [
        (
            "root_const",
            r#"{"type":"array","const":[10]}"#,
            "[1e1]",
            "sample-const/1",
        ),
        (
            "nested_const",
            r#"{"type":"object","properties":{"x":{"type":"object","const":{"n":10}}}}"#,
            r#"{"x":{"n":1e1}}"#,
            "sample-const/1",
        ),
        (
            "nested_unique",
            r#"{"type":"object","properties":{"x":{"type":"array","uniqueItems":true}}}"#,
            r#"{"x":[10,11]}"#,
            "sample-array/1",
        ),
    ] {
        for (name, upper, configure) in [
            (
                "numeric",
                4096,
                Limits::numeric_work as fn(Limits, usize) -> Limits,
            ),
            ("work", 100_000, Limits::decoding_work),
            ("allocation", 100_000, Limits::effective_allocation_bytes),
        ] {
            let admitted = |remaining| {
                let input = catalogue(qualities, sample, configure(Limits::default(), remaining));
                input
                    .check_sample(
                        SampleRequest::new(
                            input.source_id("m").unwrap(),
                            SampleKind::Property,
                            "/sdfProperty/p",
                        )
                        .sample(input.source_id("s").unwrap()),
                    )
                    .is_ok_and(|report| {
                        report.sample_result().unwrap().admission() == Admission::Admitted
                    })
            };
            assert!(admitted(upper));
            let (mut lower, mut higher) = (1, upper);
            while lower < higher {
                let middle = lower + (higher - lower) / 2;
                if admitted(middle) {
                    higher = middle;
                } else {
                    lower = middle + 1;
                }
            }
            let minimum = lower;
            assert!(minimum > 1);
            assert!(admitted(minimum));
            let limited = catalogue(qualities, sample, configure(Limits::default(), minimum - 1));
            let report = selected(&limited);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Undetermined
            );
            assert!(!report.outcome().complete());
            assert_eq!(
                report.assessment_area_status(area).require().unwrap(),
                AssessmentStatus::Undetermined
            );
            assert_eq!(
                limited
                    .source(limited.source_id("s").unwrap())
                    .unwrap()
                    .sample_value()
                    .require()
                    .unwrap()
                    .raw_json(),
                sample
            );
            println!(
                "PUBLIC_EQUALITY_COUNTER path={path} {name} exact={minimum} minus1=undetermined original_sample_unchanged=true"
            );
        }
    }
}

#[test]
fn duplicate_locations_keep_both_operand_roots_and_effective_witnesses() {
    let model = br#"{"sdfProperty":{"p":{"type":"array","uniqueItems":true}}}"#;
    for typed in [false, true] {
        for (scope, limits) in [
            Limits::default(),
            Limits::default().pointer_bytes(1).pointer_output_bytes(1),
            Limits::default().conditions(1),
            Limits::default().diagnostic_bytes(1),
        ]
        .into_iter()
        .enumerate()
        {
            let builder = JsonBuilder::new();
            let input = if typed {
                SourceInput::typed(
                    "s",
                    builder
                        .array(vec![
                            builder
                                .map(vec![("a/b~c", builder.number("10").unwrap())])
                                .unwrap(),
                            builder
                                .map(vec![("a/b~c", builder.number("1e1").unwrap())])
                                .unwrap(),
                        ])
                        .unwrap(),
                )
            } else {
                SourceInput::supplied("s", br#"[{"a/b~c":10},{"a\u002fb~c":1e1}]"#).sample()
            };
            let input = Catalogue::take_inputs_with_limits(
                vec![SourceInput::supplied("m", model), input],
                limits,
            )
            .unwrap();
            let report = selected(&input);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert_eq!(report.outcome().errors(), 1);
            assert!(report.outcome().complete());
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(wire["result"]["admission"], "rejected");
            assert!(report.to_human().contains("rejected"));
            let finding = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["code"] == "sample.unique");
            let witness = wire["result"]["witnesses"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["kind"] == "rejection" && w["constraint"] == "uniqueItems");
            let finding = if scope == 0 {
                Some(finding.expect("unrestricted duplicate must retain its finding"))
            } else {
                finding
            };
            let witness = if scope == 0 {
                Some(witness.expect("unrestricted duplicate must retain its rejection witness"))
            } else {
                witness
            };
            if let Some(finding) = finding {
                let locations = [(&finding["primary"], 1), (&finding["related"][2], 0)];
                for (location, index) in locations {
                    let value = input
                        .source(input.source_id("s").unwrap())
                        .unwrap()
                        .sample_value()
                        .require()
                        .unwrap()
                        .at(index)
                        .require()
                        .unwrap();
                    assert_eq!(location["kind"], "sample");
                    assert_eq!(
                        location["span"],
                        value
                            .span()
                            .map_or(Value::Null, |span| json!([span.start, span.end]))
                    );
                    if scope == 0 || location["pointer"].is_string() {
                        assert_eq!(location["pointer"], format!("/{index}"));
                    }
                }
                assert_eq!(finding["related"][0]["kind"], "effective");
                assert_eq!(finding["related"][1]["kind"], "source");
                if let Some(witness) = witness {
                    assert_eq!(witness["effective"], finding["related"][0]);
                }
            }
        }
    }
}

#[test]
fn decisive_constant_leaf_does_not_move_a_later_count_condition() {
    let input = catalogue(
        r#"{"type":"array","const":[1],"maxItems":0}"#,
        "[2]",
        Limits::default(),
    );
    let report = selected(&input);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(report.outcome().errors(), 2);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    let constant = rows
        .iter()
        .find(|row| row["code"] == "sample.const")
        .unwrap();
    let count = rows
        .iter()
        .find(|row| row["code"] == "sample.count")
        .unwrap();
    assert_eq!(constant["primary"]["pointer"], "/0");
    assert_eq!(constant["primary"]["span"], json!([1, 2]));
    assert_eq!(constant["related"][0]["pointer"], "/sdfProperty/p/const/0");
    assert_eq!(count["primary"]["pointer"], "");
    assert_eq!(count["primary"]["span"], json!([0, 3]));
}

#[test]
fn nested_duplicates_preserve_original_and_typed_coordinates_after_projection() {
    let model = br#"{"sdfProperty":{"p":{"type":"object","sdfChoice":{"plain":{"properties":{"a/b~c":{"type":"array","uniqueItems":true}}},"ordinary":{"properties":{"a/b~c":{"type":"array","uniqueItems":true,"items":{"type":"number"}}}}}}}}"#;
    for typed in [false, true] {
        let builder = JsonBuilder::new();
        let sample = if typed {
            SourceInput::typed(
                "s",
                builder
                    .map(vec![(
                        "a/b~c",
                        builder
                            .array(vec![
                                builder.number("10").unwrap(),
                                builder.number("1e1").unwrap(),
                            ])
                            .unwrap(),
                    )])
                    .unwrap(),
            )
        } else {
            SourceInput::supplied("s", br#"{"a\u002fb~c":[10,1e1]}"#).sample()
        };
        let input =
            Catalogue::take_inputs(vec![SourceInput::supplied("m", model), sample]).unwrap();
        let report = selected(&input);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(report.outcome().complete());
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let finding = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == "sample.unique")
            .expect("projected duplicate must retain its finding");
        let witness = wire["result"]["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["kind"] == "rejection" && r["constraint"] == "uniqueItems")
            .expect("projected duplicate must retain its witness");
        let value = input
            .source(input.source_id("s").unwrap())
            .unwrap()
            .sample_value()
            .require()
            .unwrap()
            .get("a/b~c")
            .require()
            .unwrap();
        for (location, index) in [(&finding["primary"], 1), (&finding["related"][2], 0)] {
            assert_eq!(location["pointer"], format!("/a~1b~0c/{index}"));
            assert_eq!(location["kind"], "sample");
            assert_eq!(
                location["span"],
                value
                    .at(index)
                    .require()
                    .unwrap()
                    .span()
                    .map_or(Value::Null, |s| json!([s.start, s.end]))
            );
        }
        assert_eq!(finding["related"][0]["kind"], "effective");
        assert_eq!(finding["related"][1]["kind"], "source");
        assert_eq!(witness["effective"], finding["related"][0]);
        assert_eq!(witness["sample"], finding["primary"]);
    }
}

#[test]
fn projected_constant_leaf_restores_the_nested_array_for_later_count_findings() {
    let input = catalogue(
        r#"{"type":"object","properties":{"x":{"type":"array","const":[1],"maxItems":0}}}"#,
        r#"{"x":[2]}"#,
        Limits::default(),
    );
    let report = selected(&input);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert_eq!(report.outcome().errors(), 2);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    let constant = rows
        .iter()
        .find(|row| row["code"] == "sample.const")
        .unwrap();
    let count = rows
        .iter()
        .find(|row| row["code"] == "sample.count")
        .unwrap();
    let array = input
        .source(input.source_id("s").unwrap())
        .unwrap()
        .sample_value()
        .require()
        .unwrap()
        .get("x")
        .require()
        .unwrap();
    assert_eq!(constant["primary"]["pointer"], "/x/0");
    assert_eq!(constant["primary"]["span"], json!([6, 7]));
    assert_eq!(
        constant["related"][0]["pointer"],
        "/sdfProperty/p/properties/x/const/0"
    );
    assert_eq!(count["primary"]["pointer"], "/x");
    assert_eq!(
        count["primary"]["span"],
        array.span().map(|s| json!([s.start, s.end])).unwrap()
    );
}
