//! Independently expected samples checked by both Esdieff and a Draft 2020-12 validator.
use esdieff::{
    Admission, Catalogue, Disposition, Limits, SampleKind, SampleRequest, SchemaRequest,
    SourceInput,
};
use serde_json::Value;

fn compare(model: &str, kind: SampleKind, pointer: &str, cases: &[(&str, bool)]) {
    for &(sample, expected) in cases {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("sample", sample.as_bytes()).sample(),
        ])
        .unwrap();
        let id = catalogue.source_id("model").unwrap();
        let export = catalogue
            .export_schema(SchemaRequest::new(id, kind, pointer))
            .unwrap();
        assert_eq!(
            export.disposition(),
            Disposition::Passed,
            "{}",
            export.to_json()
        );
        let schema: Value = serde_json::from_str(export.schema().unwrap()).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let value: Value = serde_json::from_str(sample).unwrap();
        assert_eq!(
            validator.is_valid(&value),
            expected,
            "JSON Schema {sample} under {model}"
        );
        let report = catalogue
            .check_sample(
                SampleRequest::new(id, kind, pointer)
                    .sample(catalogue.source_id("sample").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            if expected {
                Admission::Admitted
            } else {
                Admission::Rejected
            },
            "SDF {sample} under {model}: {}",
            report.to_json()
        );
    }
}

fn property(description: &str, cases: &[(&str, bool)]) {
    compare(
        &format!(r#"{{"sdfProperty":{{"value":{description}}}}}"#),
        SampleKind::Property,
        "/sdfProperty/value",
        cases,
    );
}

#[test]
fn nullable_numbers_keep_exact_decimal_constraints() {
    property(
        r#"{"type":"number","minimum":0.1,"maximum":0.3,"multipleOf":0.1}"#,
        &[
            ("null", true),
            ("0.1", true),
            ("0.3", true),
            ("0.2", true),
            ("0.10000000000000001", false),
            ("0.4", false),
            ("\"0.1\"", false),
        ],
    );
    property(
        r#"{"type":"integer","nullable":false,"exclusiveMinimum":9007199254740992,"maximum":9007199254740994}"#,
        &[
            ("9007199254740993", true),
            ("9007199254740992", false),
            ("9007199254740995", false),
            ("9007199254740993.5", false),
            ("null", false),
        ],
    );
}

#[test]
fn choices_apply_whole_overrides_and_allow_overlapping_branches() {
    property(
        r#"{"type":"number","minimum":10,"nullable":false,"sdfChoice":{"low":{"minimum":0,"maximum":20},"high":{"minimum":15,"maximum":30}}}"#,
        &[
            ("0", true),
            ("16", true),
            ("30", true),
            ("31", false),
            ("null", false),
        ],
    );
    property(
        r#"{"type":"integer","nullable":false,"sdfChoice":{"number":{},"text":{"type":"string","minLength":2}}}"#,
        &[
            ("3", true),
            ("\"hi\"", true),
            ("\"x\"", false),
            ("null", false),
        ],
    );
}

#[test]
fn map_presence_is_separate_from_nullability_and_extra_entries_stay_open() {
    property(
        r#"{"type":"object","nullable":false,"required":["name","name"],"properties":{"name":{"type":"string","nullable":false,"minLength":2},"flag":{"type":"boolean"}}}"#,
        &[
            (r#"{"name":"ok"}"#, true),
            (r#"{"name":"ok","extra":17,"flag":null}"#, true),
            (r#"{"name":null}"#, false),
            (r#"{"name":"x"}"#, false),
            (r#"{"flag":true}"#, false),
            ("null", false),
        ],
    );
}

#[test]
fn compound_constants_and_uniqueness_use_json_value_equality() {
    property(
        r#"{"type":"object","const":{"a":10,"b":[true,null]}}"#,
        &[
            (r#"{"b":[true,null],"a":1e1}"#, true),
            (r#"{"a":10,"b":[null,true]}"#, false),
            ("null", false),
        ],
    );
    property(
        r#"{"type":"array","nullable":false,"minItems":1,"maxItems":3,"uniqueItems":true}"#,
        &[
            ("[1,2]", true),
            ("[1,1.0]", false),
            ("[]", false),
            ("[1,2,3,4]", false),
            (r#"[{"a":1,"b":2},{"b":2,"a":1}]"#, false),
        ],
    );
}

#[test]
fn enums_and_constants_still_constrain_nullable_values() {
    property(
        r#"{"type":"string","enum":["on","off","on"]}"#,
        &[
            (r#""on""#, true),
            (r#""off""#, true),
            (r#""other""#, false),
            ("null", false),
        ],
    );
    property(
        r#"{"type":"string","const":null}"#,
        &[("null", true), (r#""null""#, false)],
    );
    property(
        r#"{"nullable":false}"#,
        &[("null", false), ("4", true), (r#"{"a":true}"#, true)],
    );
}

#[test]
fn all_selected_payload_kinds_and_effective_references_work() {
    for (kind, group, pointer, qualities) in [
        (
            SampleKind::ActionInput,
            "sdfAction",
            "/sdfAction/do",
            r#""sdfInputData":{"type":"boolean","nullable":false}"#,
        ),
        (
            SampleKind::ActionOutput,
            "sdfAction",
            "/sdfAction/do",
            r#""sdfOutputData":{"type":"boolean","nullable":false}"#,
        ),
        (
            SampleKind::EventOutput,
            "sdfEvent",
            "/sdfEvent/do",
            r#""sdfOutputData":{"type":"boolean","nullable":false}"#,
        ),
    ] {
        compare(
            &format!(r#"{{"{group}":{{"do":{{{qualities}}}}}}}"#),
            kind,
            pointer,
            &[("true", true), ("null", false), ("1", false)],
        );
    }
    compare(
        r#"{"sdfObject":{"Device":{"sdfContext":{"serial":{"type":"string","nullable":false}}}}}"#,
        SampleKind::Context,
        "/sdfObject/Device/sdfContext/serial",
        &[(r#""abc""#, true), ("null", false)],
    );
    compare(
        r##"{"sdfData":{"base":{"type":"integer","minimum":2,"maximum":10}},"sdfProperty":{"value":{"sdfRef":"#/sdfData/base","minimum":5}}}"##,
        SampleKind::Property,
        "/sdfProperty/value",
        &[("4", false), ("5", true), ("10", true), ("11", false)],
    );
}

#[test]
fn maintained_thermostat_models_supply_realistic_export_samples() {
    let model = include_str!("../../examples/thermostat-check/rich.sdf.json");
    compare(
        model,
        SampleKind::Property,
        "/sdfObject/Thermostat/sdfProperty/targetTemperature",
        &[
            ("5", true),
            ("20.5", true),
            ("20.25", false),
            ("null", false),
        ],
    );
    compare(
        model,
        SampleKind::ActionInput,
        "/sdfObject/Thermostat/sdfAction/configure",
        &[
            (r#"{"targetTemperature":20,"mode":"heat"}"#, true),
            (r#"{"targetTemperature":20,"mode":"wrong"}"#, false),
        ],
    );
}

fn refused(description: &str) -> String {
    let model = format!(r#"{{"sdfProperty":{{"value":{description}}}}}"#);
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
    let result = catalogue
        .export_schema(SchemaRequest::new(
            catalogue.source_id("model").unwrap(),
            SampleKind::Property,
            "/sdfProperty/value",
        ))
        .unwrap();
    assert_eq!(
        result.disposition(),
        Disposition::Undetermined,
        "{}",
        result.to_json()
    );
    assert!(result.schema().is_none());
    let report: Value = serde_json::from_str(result.to_json()).unwrap();
    assert!(report["schema"].is_null());
    assert_eq!(report["fidelity"], "refused");
    result.reason().unwrap().to_owned()
}

#[test]
fn unsupported_meaning_never_escapes_as_a_partial_schema() {
    for description in [
        r#"{"type":"string","format":"date-time"}"#,
        r#"{"sdfType":"byte-string"}"#,
        r#"{"contentFormat":"application/json"}"#,
        r#"{"type":"string","pattern":"a+"}"#,
        r#"{"type":"array","items":{"type":"integer"}}"#,
        r#"{"type":"object","properties":{"optional":{"format":"uuid"}}}"#,
        r#"{"type":"integer","vendor:quality":3}"#,
        r#"{"sdfChoice":{"one":{"type":"integer"},"two":{"format":"date"}}}"#,
        r##"{"sdfRef":"#/sdfData/missing"}"##,
    ] {
        refused(description);
    }
}

#[test]
fn malformed_constraints_and_unestablished_structure_are_refused() {
    for description in [
        r#"{"type":"unknown"}"#,
        r#"{"nullable":"yes"}"#,
        r#"{"multipleOf":0}"#,
        r#"{"minLength":-1}"#,
        r#"{"maxItems":1.5}"#,
        r#"{"required":["x"]}"#,
        r#"{"properties":{"x":{"type":"boolean"}}}"#,
        r#"{"type":"object","required":[]}"#,
    ] {
        refused(description);
    }
}

#[test]
fn context_without_an_explicit_nullable_contract_is_refused() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br#"{"sdfObject":{"Device":{"sdfContext":{"serial":{"type":"string"}}}}}"#,
    )])
    .unwrap();
    let result = catalogue
        .export_schema(SchemaRequest::new(
            catalogue.source_id("model").unwrap(),
            SampleKind::Context,
            "/sdfObject/Device/sdfContext/serial",
        ))
        .unwrap();
    assert!(result.schema().is_none());
}

#[test]
fn byte_work_and_node_limits_withhold_whole_schemas() {
    let model = format!(
        r#"{{"sdfProperty":{{"value":{{"type":"string","description":"{}"}}}}}}"#,
        "x".repeat(30000)
    );
    for limits in [
        Limits::default().report_bytes(24000),
        Limits::default().effective_nodes(1),
        Limits::default().decoding_work(1000),
    ] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", model.as_bytes())], limits)
                .unwrap();
        let result = catalogue
            .export_schema(SchemaRequest::new(
                catalogue.source_id("model").unwrap(),
                SampleKind::Property,
                "/sdfProperty/value",
            ))
            .unwrap();
        assert!(result.schema().is_none(), "{}", result.to_json());
        assert_eq!(result.disposition(), Disposition::Undetermined);
    }
}

#[test]
fn exhaustion_after_identity_and_composition_returns_a_refusal_report() {
    for work in [600, 700] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied(
                "model",
                br#"{"sdfProperty":{"value":{"type":"boolean"}}}"#,
            )],
            Limits::default().decoding_work(work),
        )
        .unwrap();
        let result = catalogue.export_schema(SchemaRequest::new(
            catalogue.source_id("model").unwrap(),
            SampleKind::Property,
            "/sdfProperty/value",
        ));
        let result = result.expect("export exhaustion must retain its reserved report");
        assert!(result.schema().is_none());
        assert_eq!(result.disposition(), Disposition::Undetermined);
        assert!(result.reason().unwrap().contains("ceiling"));
    }
}
