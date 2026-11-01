//! Failure injection at the revision comparison boundary for grouping counts.
use esdieff::{Catalogue, Classification, ComparisonRequest, Limits, Perspective, SourceInput};

#[test]
fn incomplete_count_reads_never_prove_compatibility() {
    let before =
        r#"{"sdfObject":{"Sockets":{"minItems":9007199254740993,"maxItems":9007199254740998}}}"#;
    let after = before.replace("9007199254740993", "9007199254740994");
    for limits in [
        Limits::default().numeric_work(1),
        Limits::default().decoding_work(100),
        Limits::default().effective_allocation_bytes(100),
    ] {
        let earlier = Catalogue::with_limits(
            &[SourceInput::supplied("model", before.as_bytes())],
            limits.clone(),
        )
        .unwrap();
        let later =
            Catalogue::with_limits(&[SourceInput::supplied("model", after.as_bytes())], limits)
                .unwrap();
        for perspective in [Perspective::Client, Perspective::Implementer] {
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(perspective)
                .compare()
                .unwrap();
            assert_eq!(
                comparison.outcome(),
                Classification::Undetermined,
                "{}",
                comparison.to_json()
            );
            assert!(
                comparison
                    .differences()
                    .all(|d| d.reason().is_some_and(|r| matches!(
                        r.code(),
                        "operation.limit" | "data.number_unavailable"
                    ))),
                "{}",
                comparison.to_json()
            );
        }
    }
}

#[test]
fn established_count_breaks_survive_later_work_and_allocation_failures() {
    let before = r#"{"sdfObject":{"Sockets":{"maxItems":4}}}"#;
    let after = before.replace("maxItems\":4", "maxItems\":3");
    let mut later_document = serde_json::json!({"sdfObject":{}});
    for index in 0..40 {
        later_document["sdfObject"][format!("Sockets{index}")] = serde_json::json!({"maxItems":4});
    }
    let larger_before = later_document.to_string();
    let larger_after = larger_before.replace("maxItems\":4", "maxItems\":3");
    for cutoffs in [
        (500..=20000)
            .step_by(250)
            .map(|work| Limits::default().decoding_work(work))
            .collect::<Vec<_>>(),
        (2000..=100000)
            .step_by(2000)
            .map(|bytes| Limits::default().effective_allocation_bytes(bytes))
            .collect(),
    ] {
        let mut retained = false;
        for limits in cutoffs {
            let earlier = Catalogue::with_limits(
                &[
                    SourceInput::supplied("first", before.as_bytes()),
                    SourceInput::supplied("second", larger_before.as_bytes()),
                ],
                limits.clone(),
            )
            .unwrap();
            let later = Catalogue::with_limits(
                &[
                    SourceInput::supplied("first", after.as_bytes()),
                    SourceInput::supplied("second", larger_after.as_bytes()),
                ],
                limits,
            )
            .unwrap();
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            if comparison.count(Classification::Breaking) > 0
                && comparison.count(Classification::Undetermined) > 0
            {
                assert_eq!(comparison.outcome(), Classification::Breaking);
                assert_eq!(comparison.disposition().exit_code(), 1);
                retained = true;
                break;
            }
        }
        assert!(
            retained,
            "a later failure must retain the earlier count classification"
        );
    }
}
