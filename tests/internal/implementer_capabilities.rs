//! Failure injection through the comparison request boundary.
use esdieff::{Catalogue, Classification, ComparisonRequest, Limits, Perspective, SourceInput};

#[test]
fn incomplete_capability_reads_keep_limits_and_retained_breaks() {
    let before = r#"{"sdfObject":{"Switch":{"sdfProperty":{"on":{"writable":false}}}}}"#;
    let after = before.replace("false", "true");
    let mut large = serde_json::json!({"sdfObject":{}});
    for index in 0..40 {
        large["sdfObject"][format!("Switch{index}")] =
            serde_json::json!({"sdfProperty":{"on":{"writable":false}}});
    }
    let larger_before = large.to_string();
    let larger_after = larger_before.replace("false", "true");
    for allowances in [
        (100..=20000)
            .step_by(250)
            .map(|n| Limits::default().decoding_work(n))
            .collect::<Vec<_>>(),
        (100..=100000)
            .step_by(2000)
            .map(|n| Limits::default().effective_allocation_bytes(n))
            .collect(),
    ] {
        let mut unknown_only = false;
        let mut retained_break = false;
        for limits in allowances {
            let build = |first: &str, second: &str| {
                Catalogue::with_limits(
                    &[
                        SourceInput::supplied("first", first.as_bytes()),
                        SourceInput::supplied("second", second.as_bytes()),
                    ],
                    limits.clone(),
                )
                .unwrap()
            };
            let earlier = build(before, &larger_before);
            let later = build(&after, &larger_after);
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            if comparison.count(Classification::Undetermined) == 0 {
                continue;
            }
            assert!(
                comparison
                    .differences()
                    .filter_map(|d| d.reason())
                    .all(|r| r.code() == "operation.limit")
            );
            if comparison.count(Classification::Breaking) > 0 {
                assert_eq!(comparison.disposition().exit_code(), 1);
                retained_break = true;
            } else {
                assert_eq!(comparison.outcome(), Classification::Undetermined);
                assert_eq!(comparison.disposition().exit_code(), 3);
                unknown_only = true;
            }
            if unknown_only && retained_break {
                break;
            }
        }
        assert!(unknown_only && retained_break);
    }
}
