//! The existing consistency evaluator follows ADR 0018: Civil date and time sample conventions.
use esdieff::{Assessment, Catalogue, CheckRequest, SourceInput};
use serde_json::Value;
fn check(model: &str, assessment: Assessment) -> Value {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("m", model.as_bytes())]).unwrap();
    serde_json::from_str(
        &catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap()
            .to_json(),
    )
    .unwrap()
}
#[test]
fn declared_civil_candidates_use_shared_denotation_and_keep_advisory_domains() {
    for (format, text, warnings, unfinished, knowledge) in [
        ("date", "1900-02-29", 1, false, false),
        ("date", "2000-02-29", 0, false, false),
        ("time", "24:00:00Z", 1, false, false),
        ("time", "23:59:60Z", 0, true, false),
        ("date-time", "2024-06-30T23:59:60Z", 1, false, true),
        ("date-time", "2030-06-30T23:59:60Z", 0, true, true),
    ] {
        let model = format!(
            r#"{{"info":{{}},"sdfData":{{"d":{{"type":"string","format":"{format}","default":"{text}"}}}}}}"#
        );
        let wire = check(&model, Assessment::ExpandedConsistency);
        assert_eq!(wire["coverage"]["requested"]["total"], 10);
        let row = wire["coverage"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "default-consistency/1")
            .unwrap();
        assert_eq!(
            row["status"],
            if unfinished {
                "undetermined"
            } else {
                "assessed"
            }
        );
        let conditions = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["code"] == "consistency.default_excluded")
            .collect::<Vec<_>>();
        assert_eq!(conditions.len(), warnings, "{format} {text}");
        if let Some(condition) = conditions.first() {
            assert!(condition.get("domain").is_none());
            assert_eq!(condition["severity"], "warning");
            assert_eq!(condition["basis"]["kind"], "advice");
            assert_eq!(condition["primary"]["pointer"], "/sdfData/d/default");
        }
        assert_eq!(
            wire["baseline"].get("temporal_knowledge").is_some(),
            knowledge
        );
        assert!(
            wire["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == "ADR0018-selected-representation")
        );
        let initial = check(&model, Assessment::Initial);
        assert!(
            !initial["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == "ADR0018-selected-representation")
        );
    }
}
#[test]
fn aggregate_only_choice_keeps_independent_temporal_exclusion_and_unknown_sibling() {
    let model = r#"{"info":{},"sdfData":{"d":{"type":"string","default":"2030-06-30T23:59:60Z","maxLength":1,"sdfChoice":{"a":{"format":"date-time"},"b":{"format":"time"}}}}}"#;
    let wire = check(model, Assessment::ExpandedConsistency);
    assert_eq!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["code"] == "consistency.default_excluded")
            .count(),
        1
    );
    assert_eq!(wire["outcome"]["errors"], 0);
    assert!(!wire["outcome"]["complete"].as_bool().unwrap());
    assert!(wire["baseline"].get("temporal_knowledge").is_some());
}
