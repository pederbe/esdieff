//! Unsupported Full vocabulary names its quality and nearest source contribution.
//! Follows ADR 0003: Check findings and completeness and
//! ADR 0037: Undeclared extension vocabulary.
use esdieff::{Assessment, Catalogue, CheckRequest, Limits, SourceInput};
use serde_json::{Value, json};

fn check(text: &str, limits: Limits) -> (Value, String) {
    let catalogue =
        Catalogue::with_limits(&[SourceInput::supplied("model", text.as_bytes())], limits).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    (
        serde_json::from_str(&report.to_json()).unwrap(),
        report.to_human(),
    )
}

fn vocabulary(wire: &Value) -> Vec<&Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            matches!(
                item["code"].as_str(),
                Some("extension.optional_unsupported" | "quality.unfamiliar")
            )
        })
        .collect()
}

#[test]
fn full_names_each_unsupported_quality_and_locates_its_source() {
    let text = r#"{
      "info":{},
      "sdfData":{"D":{"type":"object","properties":{
        "first":{"type":"number","writable":true},
        "second":{"type":"number","writable":false,"sdfEnum":[1,2],"vendor:extra":{}}
      }}}
    }"#;
    let (wire, human) = check(text, Limits::default());
    assert_eq!(wire["outcome"]["disposition"], "undetermined");
    let items = vocabulary(&wire);
    assert_eq!(items.len(), 4);
    for (pointer, quality, raw, code) in [
        (
            "/sdfData/D/properties/first/writable",
            "writable",
            "true",
            "extension.optional_unsupported",
        ),
        (
            "/sdfData/D/properties/second/writable",
            "writable",
            "false",
            "extension.optional_unsupported",
        ),
        (
            "/sdfData/D/properties/second/sdfEnum",
            "sdfEnum",
            "[1,2]",
            "quality.unfamiliar",
        ),
        (
            "/sdfData/D/properties/second/vendor:extra",
            "vendor:extra",
            "{}",
            "extension.optional_unsupported",
        ),
    ] {
        let item = items
            .iter()
            .find(|item| item["primary"]["pointer"] == pointer)
            .unwrap();
        assert_eq!(item["code"], code);
        assert_eq!(item["arguments"]["quality"], quality);
        if code == "extension.optional_unsupported" {
            assert_eq!(item["arguments"]["observed_occurrences"], 1);
        }
        assert_eq!(item["primary"]["kind"], "source");
        let span = item["primary"]["span"].as_array().unwrap();
        assert_eq!(
            &text[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
            raw
        );
        assert!(human.contains(pointer));
        assert!(human.contains(&format!("Quality: {quality:?}")));
    }
}

#[test]
fn full_locates_unknown_selectors_without_inspecting_opaque_content() {
    let text = r#"{"info":{},"sdfData":{
      "D":{"type":"future","format":"future","sdfType":"future", "vendor:extra":{"hidden":1}},
      "opaque":{"type":"object","const":{"writable":true,"sdfEnum":[]}}
    }}"#;
    let (wire, _) = check(text, Limits::default());
    let items = vocabulary(&wire);
    assert_eq!(items.len(), 4);
    for key in ["type", "format", "sdfType", "vendor:extra"] {
        assert!(items.iter().any(|item| {
            item["arguments"]["quality"] == key
                && item["primary"]["pointer"] == format!("/sdfData/D/{key}")
        }));
    }
}

#[test]
fn composed_quality_keeps_its_effective_position_and_donor_source() {
    let donor = br#"{"info":{},"namespace":{"d":"urn:donor"},"defaultNamespace":"d",
      "sdfProperty":{"P":{"type":"number","writable":true}}}"#;
    let consumer = br#"{"info":{},"namespace":{"d":"urn:donor"},
      "sdfData":{"D":{"type":"object","properties":{"entry":{"sdfRef":"d:#/sdfProperty/P"}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("donor", donor),
        SourceInput::supplied("consumer", consumer),
    ])
    .unwrap();
    let root = catalogue.source_id("consumer").unwrap();
    let report = catalogue
        .check(
            CheckRequest::new()
                .root(root)
                .assessment(Assessment::FullBaseline),
        )
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let items = vocabulary(&wire);
    let item = items
        .iter()
        .find(|item| item["arguments"]["quality"] == "writable")
        .unwrap();
    assert_eq!(
        item["primary"],
        json!({"kind":"effective","root":root.get(),"pointer":"/sdfData/D/properties/entry/writable"})
    );
    assert_eq!(item["related"].as_array().unwrap().len(), 1);
    assert_eq!(
        item["related"][0]["source_id"],
        catalogue.source_id("donor").unwrap().get()
    );
    assert_eq!(item["related"][0]["pointer"], "/sdfProperty/P/writable");
    let span = item["related"][0]["span"].as_array().unwrap();
    assert_eq!(
        &donor[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
        b"true"
    );
    assert!(report.to_human().contains("/sdfProperty/P/writable"));
}

#[test]
fn presentation_limits_preserve_full_outcomes_and_name_without_pointer() {
    let text = r#"{"info":{},"sdfData":{"D":{"type":"object","properties":{
      "one":{"type":"number","writable":true},
      "two":{"type":"number","writable":false,"sdfEnum":[1,2]}
    }}}}"#;
    let (ample, _) = check(text, Limits::default());
    for (limits, reason) in [
        (Limits::default().conditions(1), "count_limit"),
        (Limits::default().diagnostic_bytes(1), "byte_limit"),
        (Limits::default().pointer_bytes(1), "location_limit"),
    ] {
        let (wire, _) = check(text, limits);
        assert_eq!(wire["outcome"], ample["outcome"]);
        assert_eq!(
            wire["coverage"]["requested"],
            ample["coverage"]["requested"]
        );
        assert!(
            wire["diagnostics"]["reasons"]
                .as_array()
                .unwrap()
                .contains(&json!(reason))
        );
        if reason == "location_limit" {
            for item in vocabulary(&wire) {
                assert!(item["arguments"]["quality"].is_string());
                assert!(item["primary"]["pointer"].is_null());
                assert_eq!(item["primary"]["omission"]["reason"], reason);
            }
        }
    }
}

#[test]
fn initial_keeps_its_optional_vocabulary_summary() {
    let text = br#"{"info":{},"sdfData":{"D":{"type":"number","vendor:one":{},"vendor:two":{}}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let items = vocabulary(&wire);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["arguments"], json!({"observed_occurrences":2}));
    assert_eq!(items[0]["primary"]["pointer"], "");
}

#[test]
fn full_cli_names_the_quality_in_both_formats() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let text = br#"{"info":{},"sdfData":{"D":{"type":"object","properties":{"entry":{"type":"number","writable":true}}}}}"#;
    for format in ["json", "human"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args([
                "check",
                "--assessment",
                "full-baseline",
                "--format",
                format,
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(text).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(3));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            let items = vocabulary(&wire);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["arguments"]["quality"], "writable");
            assert_eq!(
                items[0]["primary"]["pointer"],
                "/sdfData/D/properties/entry/writable"
            );
        } else {
            let human = String::from_utf8(output.stdout).unwrap();
            assert!(human.contains("Quality: \"writable\""));
            assert!(human.contains("/sdfData/D/properties/entry/writable"));
        }
    }
}

#[test]
fn long_quality_names_are_omitted_with_their_byte_budget() {
    let name = "q".repeat(4096);
    let text = json!({"info":{},"sdfData":{"D":{"type":"number",name.as_str():1}}}).to_string();
    let (ample, _) = check(&text, Limits::default());
    assert_eq!(vocabulary(&ample)[0]["arguments"]["quality"], name);
    let (small, _) = check(&text, Limits::default().diagnostic_bytes(4096));
    assert!(vocabulary(&small).is_empty());
    assert_eq!(small["outcome"], ample["outcome"]);
    assert!(
        small["diagnostics"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("byte_limit"))
    );
}
