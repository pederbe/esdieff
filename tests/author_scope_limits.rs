//! Unfinished contributor lookup retains scope under ADR0023: Supplied global composition and dependency scope.
use esdieff::{AuthorObject, AuthorRequest, Catalogue, Disposition, Limits, SourceInput};
use serde_json::json;

#[test]
fn unfinished_contributor_selection_cannot_establish_a_unique_dependency() {
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied(
                "base",
                br#"{"namespace":{"b":"https://e/x"},"defaultNamespace":"b","sdfObject":{"Sensor":{}}}"#,
            ),
            SourceInput::supplied(
                "candidate",
                br#"{"namespace":{"b":"https://e/x"},"defaultNamespace":"b","sdfObject":{"Other":{}}}"#,
            ),
        ],
        Limits::default().decoding_work(1199),
    )
    .unwrap();
    let source = catalogue.source_id("base").unwrap();
    let result = AuthorRequest::new(catalogue, "recipe", "product", "Thing")
        .unwrap()
        .namespace("reuse", "https://e/x")
        .unwrap()
        .object(AuthorObject::new("sensor", source, "/sdfObject/Sensor", "reuse", false).unwrap())
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.outcome().disposition(), Disposition::Undetermined);
    assert!(!result.outcome().complete());
    let wire: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(wire["request"]["scope"]["dependencies"], json!([]));
    assert!(
        wire["request"]["scope"]["unavailable_required"]
            .as_array()
            .unwrap()
            .contains(&json!(2))
    );
    assert!(
        wire["request"]["scope"]["assessed"]
            .as_array()
            .unwrap()
            .contains(&json!(2))
    );
}
