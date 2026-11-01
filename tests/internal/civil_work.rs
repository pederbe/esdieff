//! Exact original-request work was qualified through the same real evaluator and counters.
use esdieff::{
    Admission, AssessmentStatus, Catalogue, Limits, SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;

/// Test-local evidence parameters for one exact original model/token and backend.
/// At k-1 every independent area still completes.
/// Separate fixtures, optional budgets or backends require their own measured parameters.
struct QualifiedOriginalWork {
    exact_request_work: usize,
    completed_prerequisite_work: usize,
    independent_length_error: bool,
}

/// Use the single original decoding-work counter with all other limits identical.
fn assert_qualified_original_boundary(
    qualified: QualifiedOriginalWork,
    limits_at: impl Fn(usize) -> Limits,
) {
    assert!(qualified.exact_request_work > qualified.completed_prerequisite_work);
    let below = qualified.exact_request_work.checked_sub(1).unwrap();
    assert!(below >= qualified.completed_prerequisite_work);
    let clock = format!("12:34:56.{}+05:45", "0".repeat(4095) + "1");
    let token = serde_json::to_string(&clock).unwrap();
    let extra = if qualified.independent_length_error {
        ",\"maxLength\":1"
    } else {
        ""
    };
    let model = format!(
        r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":false,"format":"time"{extra}}}}}}}"#
    );
    let mut observations = Vec::new();
    for (work, exhausted) in [(qualified.exact_request_work, false), (below, true)] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model.as_bytes()),
                SourceInput::supplied("s", token.as_bytes()).sample(),
            ],
            limits_at(work),
        )
        .unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("m").unwrap(),
                    SampleKind::Property,
                    "/sdfProperty/p",
                )
                .sample(catalogue.source_id("s").unwrap()),
            )
            .unwrap();
        let expected = if qualified.independent_length_error {
            Admission::Rejected
        } else if exhausted {
            Admission::Undetermined
        } else {
            Admission::Admitted
        };
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert_eq!(report.outcome().complete(), !exhausted);
        assert_eq!(
            report.outcome().errors(),
            u32::from(qualified.independent_length_error)
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-abstract-values/1")
                .require()
                .unwrap(),
            if exhausted {
                AssessmentStatus::Undetermined
            } else {
                AssessmentStatus::Assessed
            }
        );
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .sample_value()
                .require()
                .unwrap()
                .text()
                .require()
                .unwrap(),
            clock
        );
        assert_eq!(
            catalogue
                .source(catalogue.source_id("s").unwrap())
                .unwrap()
                .raw_bytes(),
            token.as_bytes()
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["result"]["sample_location"]["source_id"],
            wire["request"]["sample"]
        );
        assert_eq!(wire["result"]["sample_location"]["pointer"], "");
        assert_eq!(
            wire["result"]["sample_location"]["span"],
            serde_json::json!([0, token.len()])
        );
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert_eq!(
            wire["coverage"]["requested"]["assessed"],
            if exhausted { 3 } else { 4 }
        );
        assert_eq!(wire["coverage"]["requested"]["not_applicable"], 5);
        assert_eq!(
            wire["coverage"]["requested"]["undetermined"],
            u32::from(exhausted)
        );
        assert_eq!(wire["coverage"]["requested"]["unassessed"], 0);
        assert_eq!(wire["outcome"]["incomplete_checks"], u32::from(exhausted));
        assert!(wire["baseline"].get("temporal_knowledge").is_none());
        let items = wire["diagnostics"]["items"].as_array().unwrap();
        assert!(
            !items
                .iter()
                .any(|row| row["code"] == "sample.temporal_representation")
        );
        if exhausted {
            let limits = items
                .iter()
                .filter(|row| {
                    row["code"] == "operation.limit"
                        && row["assessment_unit"] == "sample-abstract-values/1"
                })
                .collect::<Vec<_>>();
            assert_eq!(limits.len(), 1);
            assert_eq!(limits[0]["domain"], "operation");
            assert_eq!(limits[0]["severity"], "incomplete");
            assert_eq!(limits[0]["basis"]["kind"], "operational");
        }
        let other_rows = wire["coverage"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["id"] != "sample-abstract-values/1")
            .map(|row| (row["id"].clone(), row["status"].clone()))
            .collect::<Vec<_>>();
        observations.push((other_rows, wire["result"]["sample_location"].clone()));
    }
    assert_eq!(observations[0], observations[1]);
}

#[test]
fn original_fraction_exact_request_boundary_keeps_independent_areas() {
    for (exact_request_work, completed_prerequisite_work, independent_length_error) in
        [(13655, 5177, false), (19705, 11227, true)]
    {
        assert_qualified_original_boundary(
            QualifiedOriginalWork {
                exact_request_work,
                completed_prerequisite_work,
                independent_length_error,
            },
            |work| Limits::default().decoding_work(work),
        );
    }
}
#[test]
fn original_date_exact_request_boundary_is_temporal_only() {
    for (k, length) in [(5453, false), (7402, true)] {
        let extra = if length { ",\"maxLength\":1" } else { "" };
        let model = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":false,"format":"date"{extra}}}}}}}"#
        );
        for (work, exhausted) in [(k, false), (k - 1, true)] {
            let catalogue = Catalogue::with_limits(
                &[
                    SourceInput::supplied("m", model.as_bytes()),
                    SourceInput::supplied("s", br#""2024-02-29""#).sample(),
                ],
                Limits::default().decoding_work(work),
            )
            .unwrap();
            let report = catalogue
                .check_sample(
                    SampleRequest::new(
                        catalogue.source_id("m").unwrap(),
                        SampleKind::Property,
                        "/sdfProperty/p",
                    )
                    .sample(catalogue.source_id("s").unwrap()),
                )
                .unwrap();
            assert_eq!(report.outcome().complete(), !exhausted);
            assert_eq!(report.outcome().errors(), u32::from(length));
            assert_eq!(
                report.sample_result().unwrap().admission(),
                if length {
                    Admission::Rejected
                } else if exhausted {
                    Admission::Undetermined
                } else {
                    Admission::Admitted
                }
            );
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            assert_eq!(
                wire["coverage"]["requested"]["assessed"],
                if exhausted { 3 } else { 4 }
            );
            assert_eq!(wire["coverage"]["requested"]["not_applicable"], 5);
        }
    }
}
