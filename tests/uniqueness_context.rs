//! Invocation-local context reuse preserves ADR 0015: Bounded value equality in a
//! resolved data context and ADR 0016: Selected samples with explicit admission and
//! coverage.
mod map_support;

use esdieff::{Admission, Catalogue, Limits, SampleKind, SampleRequest, SourceInput};
use map_support::selected;
use serde_json::Value;

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
fn one_hundred_distinct_integers_complete_with_unchanged_defaults() {
    let sample = serde_json::to_string(&(0..100).collect::<Vec<_>>()).unwrap();
    let input = catalogue(
        r#"{"type":"array","nullable":false,"uniqueItems":true,"items":{"type":"integer"}}"#,
        &sample,
        Limits::default(),
    );
    let report = selected(&input);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert!(report.outcome().complete());
}

#[test]
fn repeated_compound_pairs_keep_the_first_actual_duplicate_locations() {
    let input = catalogue(
        r#"{"type":"array","uniqueItems":true,"items":{"type":"object","properties":{"x":{"type":"array","items":{"type":"number"}}}}}"#,
        r#"[{"x":[1,2]},{"x":[2,1]},{"x":[3,4]},{"x":[1.0,2e0]}]"#,
        Limits::default(),
    );
    let report = selected(&input);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "sample.unique")
        .unwrap();
    assert_eq!(finding["primary"]["pointer"], "/3");
    assert_eq!(finding["related"][2]["pointer"], "/0");
}

#[test]
fn invocation_context_follows_actual_array_ancestry_and_does_not_leak_to_constants() {
    let qualities = r#"{"type":"object","properties":{"numbers":{"type":"array","uniqueItems":true,"items":{"type":"number"}},"dates":{"type":"array","uniqueItems":true,"items":{"type":"string","format":"date-time"}},"constant":{"type":"number","const":10}}}"#;
    for sample in [
        r#"{"numbers":[1,2,3],"dates":["bad","bad","bad"],"constant":1e1}"#,
        r#"{"dates":["bad","bad","bad"],"numbers":[1,2,3],"constant":1e1}"#,
    ] {
        let input = catalogue(qualities, sample, Limits::default());
        for _ in 0..2 {
            let report = selected(&input);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert!(!report.outcome().complete());
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert!(
                !wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["code"] == "sample.unique")
            );
            assert!(
                !wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["code"] == "sample.const")
            );
        }
    }
}

#[test]
fn item_classification_stays_operand_dependent_across_cached_candidates() {
    for (sample, expected) in [
        (r#"[1,2,"AQ"]"#, Admission::Admitted),
        (r#"[1,2,"AQ","AQ"]"#, Admission::Undetermined),
        (r#"["AQ",1,2]"#, Admission::Admitted),
    ] {
        let input = catalogue(
            r#"{"type":"array","uniqueItems":true,"items":{"sdfChoice":{"number":{"type":"number"},"bytes":{"sdfType":"byte-string"}}}}"#,
            sample,
            Limits::default(),
        );
        let report = selected(&input);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{sample}\n{}",
            report.to_json()
        );
    }
}

#[test]
fn multiple_invocations_share_original_pair_and_numeric_allowances() {
    let qualities = r#"{"type":"object","properties":{"a":{"type":"array","uniqueItems":true,"items":{"type":"number"}},"b":{"type":"array","uniqueItems":true,"items":{"type":"number"}}}}"#;
    let sample = r#"{"a":[1,2,3],"b":[4,5,6]}"#;
    let limited = catalogue(qualities, sample, Limits::default().equality_pairs(5));
    assert_eq!(
        selected(&limited).sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    let exact = catalogue(qualities, sample, Limits::default().equality_pairs(6));
    assert_eq!(
        selected(&exact).sample_result().unwrap().admission(),
        Admission::Admitted
    );
    let numeric = catalogue(qualities, sample, Limits::default().numeric_work(10));
    assert_eq!(
        selected(&numeric).sample_result().unwrap().admission(),
        Admission::Undetermined
    );
}

#[test]
fn repeated_candidate_growth_has_exact_finite_work_and_storage_boundaries() {
    let qualities = r#"{"type":"array","uniqueItems":true,"items":{"sdfChoice":{"a":{"type":"number"},"b":{"type":"number"},"c":{"type":"number"},"d":{"type":"number"},"e":{"type":"number"},"f":{"type":"number"},"g":{"type":"number"},"h":{"type":"number"},"i":{"type":"number"}}}}"#;
    for configure in [
        Limits::decoding_work as fn(Limits, usize) -> Limits,
        Limits::effective_allocation_bytes,
    ] {
        let admitted = |allowance| {
            let input = catalogue(
                qualities,
                "[1,2,3,4]",
                configure(Limits::default(), allowance),
            );
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
        let (mut lower, mut upper) = (1, 1_000_000);
        assert!(admitted(upper));
        while lower < upper {
            let mid = lower + (upper - lower) / 2;
            if admitted(mid) {
                upper = mid;
            } else {
                lower = mid + 1;
            }
        }
        assert!(admitted(lower));
        let input = catalogue(
            qualities,
            "[1,2,3,4]",
            configure(Limits::default(), lower - 1),
        );
        let report = selected(&input);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(!report.outcome().complete());
    }
}
