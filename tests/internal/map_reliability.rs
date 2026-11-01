// Presence reliability follows ADR0016: Selected samples with explicit admission and coverage.
use super::map_support;
use esdieff::{Admission, Catalogue, Field, Limits, Report, SourceInput};
use map_support::selected;
use serde_json::Value;

fn supplied(qualities: &str, sample: &str) -> Catalogue {
    let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
    Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", sample.as_bytes()).sample(),
    ])
    .unwrap()
}
fn wire(report: &Report<'_>, admission: Admission, complete: bool) -> Value {
    assert_eq!(report.sample_result().unwrap().admission(), admission);
    assert_eq!(report.outcome().complete(), complete);
    let coverage = report.requested_coverage();
    assert_eq!(coverage.total(), Some(9));
    assert_eq!(
        coverage.assessed()
            + coverage.not_applicable()
            + coverage.unassessed()
            + coverage.undetermined(),
        9
    );
    let value: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(value["result"]["admission"], admission.as_str());
    assert_eq!(value["outcome"]["complete"], complete);
    assert!(report.to_human().contains(admission.as_str()));
    value
}
fn no_sample_rejection(value: &Value) {
    assert!(
        !value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["domain"] == "sample" && row["severity"] == "error")
    );
    assert!(
        !value["result"]["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "rejection")
    );
    // Independent input/model conditions are deliberately permitted.
}

#[test]
fn unknown_late_required_name_cannot_leave_early_missing_presence_rejection() {
    let unknown = supplied(r#"{"type":"object","required":["missing","\ud800"]}"#, "{}");
    no_sample_rejection(&wire(&selected(&unknown), Admission::Undetermined, false));
    let known = supplied(r#"{"type":"object","required":["missing","later"]}"#, "{}");
    let report = selected(&known);
    let value = wire(&report, Admission::Rejected, true);
    assert!(
        value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["domain"] == "sample" && row["severity"] == "error")
    );
}

#[test]
fn unknown_late_map_key_invalidates_even_a_known_early_match() {
    let qualities =
        r#"{"type":"object","properties":{"known":{"type":"number"}},"required":["known"]}"#;
    let unknown = supplied(qualities, r#"{"known":1,"\ud800":2}"#);
    let sample = unknown
        .source(unknown.source_id("s").unwrap())
        .unwrap()
        .sample_value()
        .require()
        .unwrap();
    assert!(matches!(sample.get("known"), Field::Unavailable(_)));
    no_sample_rejection(&wire(&selected(&unknown), Admission::Undetermined, false));
    let known = supplied(qualities, r#"{"known":1,"later":2}"#);
    let sample = known
        .source(known.source_id("s").unwrap())
        .unwrap()
        .sample_value()
        .require()
        .unwrap();
    assert!(matches!(sample.get("known"), Field::Present(_)));
    wire(&selected(&known), Admission::Admitted, true);
}

#[test]
fn omitted_parameter_details_preserve_each_independent_needed_model_error() {
    let model = br#"{"sdfProperty":{"p":{"type":"object","properties":{"a":{"type":"number","maximum":"bad"},"b":{"type":"number","maximum":"bad"},"c":{"type":"number","maximum":"bad"},"d":{"type":"number","maximum":"bad"}}}}}"#;
    for limits in [Limits::default(), Limits::default().diagnostic_bytes(1)] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model),
                SourceInput::supplied("s", br#"{"a":1,"b":1,"c":1,"d":1}"#).sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&catalogue);
        wire(&report, Admission::Undetermined, false);
        assert!(report.outcome().errors() >= 4);
    }
}

#[test]
fn inherited_needed_reference_expands_after_copy_and_optional_reference_stays_absent() {
    let model = br##"{"sdfData":{"Limit":{"type":"number","maximum":1},"Scalar":{"sdfRef":"#/sdfData/Limit"},"Base":{"type":"object","properties":{"needed":{"sdfRef":"#/sdfData/Scalar"},"optional":{"sdfRef":"missing:#/sdfData/Never"}},"required":["needed"]}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Base","sdfChoice":{"inherited":{},"replacement":{"properties":{"good":{"type":"number"}},"required":["good"]}}}}}"##;
    for (sample, admission) in [
        (r#"{"needed":1}"#, Admission::Admitted),
        (r#"{"needed":2}"#, Admission::Rejected),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue);
        let value = wire(&report, admission, true);
        assert!(
            !value["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"]
                    .as_str()
                    .is_some_and(|code| code.starts_with("reference.")))
        );
    }
}
