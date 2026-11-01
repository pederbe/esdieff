use serde_json::Value;
#[test]
fn collection_states_and_presentation_omissions_do_not_change_outcomes() {
    use esdieff::{Catalogue, InspectRequest, Limits, SourceInput, View};
    let text = br#"{"info":{},"sdfData":{"mode":{"properties":{"absent":{},"empty":{"sdfChoice":{}},"unavailable":{"sdfChoice":[]},"branches":{"sdfChoice":{"bad":false,"good":{"minimum":10e1000000}}}}}}}"#;
    let mut original = None;
    for limits in [
        Limits::default(),
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(20),
        Limits::default().pointer_bytes(1),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap();
        let report = catalogue
            .inspect(
                InspectRequest::new(catalogue.source_id("model").unwrap(), "/sdfData/mode")
                    .view(View::Effective),
            )
            .unwrap();
        let output = report.to_json();
        // The test decoder may lack arbitrary precision; verify the exact wire lexeme separately.
        let wire: Value = serde_json::from_str(&output.replace("10e1000000", "10")).unwrap();
        let outcome = wire["outcome"].clone();
        if let Some(expected) = &original {
            assert_eq!(&outcome, expected);
            assert!(
                !wire["diagnostics"]["omissions"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        } else {
            original = Some(outcome);
            let owners = wire["result"]["alternatives"].as_array().unwrap();
            for (name, state) in [
                ("absent", "absent"),
                ("empty", "present"),
                ("unavailable", "unavailable"),
            ] {
                let owner = owners
                    .iter()
                    .find(|row| {
                        row["owner"]["pointer"] == format!("/sdfData/mode/properties/{name}")
                    })
                    .unwrap();
                assert_eq!(owner["state"], state);
                assert!(owner["branches"].as_array().unwrap().is_empty());
            }
            let owner = owners
                .iter()
                .find(|row| row["owner"]["pointer"] == "/sdfData/mode/properties/branches")
                .unwrap();
            assert_eq!(owner["state"], "present");
            assert_eq!(owner["branches"][0]["state"], "unavailable");
            assert_eq!(owner["branches"][1]["state"], "available");
            assert!(output.contains("\"minimum\":10e1000000"));
        }
        assert_eq!(wire["version"], "1.0");
    }
}
