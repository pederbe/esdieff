//! Context values use the selected-sample contract and the recorded draft defaults.
use esdieff::{
    Admission, Catalogue, Class, EffectiveRequest, Field, Limits, Lookup, SampleKind,
    SampleRequest, SourceInput,
};
use serde_json::{Value, json};

const POINTER: &str = "/sdfObject/Device/sdfContext/value";
const SENSOR: &str = "/sdfThing/Installation/sdfObject/sensor/sdfContext";
const BASE: &[u8] = include_bytes!("../../examples/context-reuse/base.sdf.json");
const DEVICE: &[u8] = include_bytes!("../../examples/context-reuse/device.sdf.json");

fn selected<'a>(catalogue: &'a Catalogue, pointer: &str) -> esdieff::Report<'a> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("model").unwrap(),
                SampleKind::Context,
                pointer,
            )
            .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap()
}

fn inline(qualities: &str, value: &str, limits: Limits) -> Value {
    let model = format!(
        r#"{{"namespace":{{"missing":"https://example.invalid/unavailable"}},"sdfObject":{{"Device":{{"sdfContext":{{"value":{qualities}}}}}}}}}"#
    );
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("model", model.as_bytes()),
            SourceInput::supplied("sample", value.as_bytes()).sample(),
        ],
        limits,
    )
    .unwrap();
    serde_json::from_str(&selected(&catalogue, POINTER).to_json()).unwrap()
}

#[test]
fn inline_context_values_have_an_explicit_selector() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "model",
            br#"{"sdfObject":{"Device":{"sdfContext":{"offset":{"type":"number","maximum":5}}}}}"#,
        ),
        SourceInput::supplied("sample", b"6").sample(),
    ])
    .unwrap();
    let kind = SampleKind::from_name("context").expect("Context sample kind");
    let report = catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("model").unwrap(),
                kind,
                "/sdfObject/Device/sdfContext/offset",
            )
            .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(report.outcome().complete(), "{}", report.to_json());
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["baseline"]["extensions"][0]["id"],
        "draft-ietf-asdf-sdf-nonaffordance-06"
    );
    assert!(
        wire["baseline"]["extensions"][0]["scope"]
            .as_str()
            .unwrap()
            .contains("selected-value checking")
    );
}

#[test]
fn typed_context_selection_keeps_same_name_property_and_request_errors_distinct() {
    let model = br#"{"sdfThing":{"Device":{"sdfContext":{"value":{"type":"number","maximum":5}},"sdfProperty":{"value":{"type":"number","minimum":10,"readable":false}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", model),
        SourceInput::supplied("sample", b"4").sample(),
    ])
    .unwrap();
    let id = catalogue.source_id("model").unwrap();
    let sample = catalogue.source_id("sample").unwrap();
    let effective = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfThing/Device"))
        .unwrap();
    for (class, expected) in [
        (Class::Context, Admission::Admitted),
        (Class::Property, Admission::Rejected),
    ] {
        let Lookup::Found(definition) = effective.definition(class, "value") else {
            panic!("selected definition")
        };
        if class == Class::Context {
            assert!(matches!(definition.readable(), Field::Absent));
            assert!(matches!(definition.writable(), Field::Absent));
            assert!(matches!(definition.observable(), Field::Absent));
        }
        let report = catalogue
            .check_sample(
                SampleRequest::from_description(definition.data_description().require().unwrap())
                    .unwrap()
                    .sample(sample),
            )
            .unwrap();
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert!(report.outcome().complete(), "{}", report.to_json());
    }
    for (kind, pointer) in [
        (SampleKind::Property, "/sdfThing/Device/sdfContext/value"),
        (SampleKind::Data, "/sdfThing/Device/sdfContext/value"),
        (SampleKind::Context, "/sdfThing/Device/sdfProperty/value"),
        (SampleKind::Context, "/sdfThing/Device/sdfContext"),
        (SampleKind::Context, "/sdfThing/Device/sdfContext/absent"),
        (
            SampleKind::Context,
            "/sdfThing/Device/sdfContext/value/maximum",
        ),
    ] {
        assert!(
            catalogue
                .check_sample(SampleRequest::new(id, kind, pointer).sample(sample))
                .is_err()
        );
    }
    assert_eq!(catalogue.source(id).unwrap().raw_bytes(), model);
}

#[test]
fn inherited_and_referenced_context_values_use_refinements_and_preserve_origins() {
    for (tail, value, admission) in [
        ("calibration", "5", Admission::Admitted),
        ("calibration", "6", Admission::Rejected),
        ("assetTag", r#""A-17""#, Admission::Admitted),
        ("assetTag", r#""A-18""#, Admission::Rejected),
        (
            "location",
            r#"{"room":"Lab","floor":2}"#,
            Admission::Admitted,
        ),
        (
            "location",
            r#"{"room":"Lab","floor":1}"#,
            Admission::Rejected,
        ),
        ("location", r#"{"floor":2}"#, Admission::Rejected),
    ] {
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("base", BASE),
            SourceInput::supplied("model", DEVICE),
            SourceInput::supplied("sample", value.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&catalogue, &format!("{SENSOR}/{tail}"));
        assert_eq!(
            report.sample_result().unwrap().admission(),
            admission,
            "{tail}: {value}"
        );
        assert!(report.outcome().complete(), "{}", report.to_json());
        if tail == "calibration" && admission == Admission::Rejected {
            let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
            let origin = wire["result"]["explanations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["effective"]["pointer"] == format!("{SENSOR}/calibration/maximum"))
                .unwrap();
            assert_eq!(
                origin["sources"][0]["source_id"],
                catalogue.source_id("model").unwrap().get()
            );
            assert_eq!(
                origin["sources"][0]["pointer"],
                "/sdfThing/Installation/sdfObject/sensor/sdfData/Offset/maximum"
            );
            assert_eq!(
                origin["consumer"]["pointer"],
                "/sdfObject/Sensor/sdfContext/calibration/sdfRef"
            );
        }
        assert_eq!(
            catalogue
                .source(catalogue.source_id("base").unwrap())
                .unwrap()
                .raw_bytes(),
            BASE
        );
        assert_eq!(
            catalogue
                .source(catalogue.source_id("model").unwrap())
                .unwrap()
                .raw_bytes(),
            DEVICE
        );
    }
}

#[test]
fn context_constraints_share_exact_equality_choices_arrays_and_map_presence() {
    for (qualities, value, expected) in [
        (
            r#"{"type":"number","minimum":9007199254740993}"#,
            "9007199254740992",
            "rejected",
        ),
        (
            r#"{"type":"number","const":9007199254740993}"#,
            "9007199254740993.0",
            "admitted",
        ),
        (
            r#"{"type":"string","enum":["north","south"]}"#,
            r#""north""#,
            "admitted",
        ),
        (
            r#"{"type":"string","enum":["north","south"]}"#,
            r#""east""#,
            "rejected",
        ),
        (
            r#"{"type":"number","maximum":5,"sdfChoice":{"wide":{"maximum":10},"shared":{}}}"#,
            "9",
            "admitted",
        ),
        (
            r#"{"type":"number","maximum":5,"sdfChoice":{"wide":{"maximum":10},"shared":{}}}"#,
            "11",
            "rejected",
        ),
        (
            r#"{"type":"array","items":{"type":"number","minimum":0},"minItems":1,"maxItems":2,"uniqueItems":true}"#,
            "[0,1]",
            "admitted",
        ),
        (
            r#"{"type":"array","items":{"type":"number","minimum":0},"minItems":1,"maxItems":2,"uniqueItems":true}"#,
            "[1,1.0]",
            "rejected",
        ),
        (
            r#"{"type":"array","items":{"type":"number","minimum":0}}"#,
            "[-1]",
            "rejected",
        ),
        (
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"integer"}}}"#,
            "{}",
            "rejected",
        ),
        (
            r#"{"type":"object","const":{"id":1},"properties":{"id":{"type":"integer"}}}"#,
            r#"{"id":1.0}"#,
            "admitted",
        ),
    ] {
        let wire = inline(qualities, value, Limits::default());
        assert_eq!(
            wire["result"]["admission"], expected,
            "{qualities}: {value}\n{wire}"
        );
        assert_eq!(wire["outcome"]["complete"], true, "{wire}");
    }
}

#[test]
fn context_nullability_defaults_stay_scoped_and_documentary_defaults_do_not_supply_values() {
    for (qualities, value, admission, complete) in [
        (r#"{"type":"string"}"#, "null", "undetermined", false),
        (
            r#"{"type":"string","nullable":true}"#,
            "null",
            "admitted",
            true,
        ),
        (
            r#"{"type":"string","nullable":false}"#,
            "null",
            "rejected",
            true,
        ),
        (
            r#"{"type":"object","properties":{"entry":{"type":"string"}}}"#,
            r#"{"entry":null}"#,
            "undetermined",
            false,
        ),
        (
            r#"{"type":"string","sdfChoice":{"inherited":{},"explicit":{"nullable":true}}}"#,
            "null",
            "admitted",
            true,
        ),
        (
            r#"{"type":"object","required":["entry"],"properties":{"entry":{"type":"string","default":"known"}}}"#,
            "{}",
            "rejected",
            true,
        ),
    ] {
        let wire = inline(qualities, value, Limits::default());
        assert_eq!(wire["result"]["admission"], admission, "{wire}");
        assert_eq!(wire["outcome"]["complete"], complete, "{wire}");
        assert!(
            !wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "context.nullable_default_unavailable"
                    && row["severity"] == "error")
        );
    }
}

#[test]
fn referenced_context_uses_inherited_nullability_and_deletion_restores_uncertainty() {
    for (nullable, expected, complete) in [
        (None, Admission::Admitted, true),
        (Some(json!(false)), Admission::Rejected, true),
        (Some(Value::Null), Admission::Undetermined, false),
    ] {
        let mut model = json!({"sdfData":{"D":{"type":"string","nullable":true}},"sdfObject":{"Device":{"sdfContext":{"value":{"sdfRef":"#/sdfData/D"}}}}});
        if let Some(nullable) = nullable {
            model["sdfObject"]["Device"]["sdfContext"]["value"]["nullable"] = nullable;
        }
        let model = serde_json::to_vec(&model).unwrap();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("model", &model),
            SourceInput::supplied("sample", b"null").sample(),
        ])
        .unwrap();
        let report = selected(&catalogue, POINTER);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            expected,
            "{}",
            report.to_json()
        );
        assert_eq!(report.outcome().complete(), complete);
    }
}

#[test]
fn unavailable_meaning_and_exhausted_limits_do_not_admit_context_values() {
    for (qualities, value, limits) in [
        (
            r#"{"sdfType":"link"}"#,
            r#""https://example.com/device""#,
            Limits::default(),
        ),
        (
            r##"{"sdfRef":"missing:#/sdfData/D"}"##,
            "1",
            Limits::default(),
        ),
        (
            r#"{"type":"number","maximum":5}"#,
            "4",
            Limits::default().effective_allocation_bytes(1),
        ),
        (
            r#"{"type":"number","maximum":100}"#,
            "101",
            Limits::default().numeric_work(4),
        ),
    ] {
        let wire = inline(qualities, value, limits);
        assert_eq!(wire["result"]["admission"], "undetermined", "{wire}");
        assert_eq!(wire["outcome"]["complete"], false, "{wire}");
        assert_eq!(wire["outcome"]["errors"], 0, "{wire}");
        assert_eq!(wire["request"]["selector"]["kind"], "context");
    }
}

#[test]
fn context_rejections_keep_sample_locations_and_diagnostic_omission_independent() {
    for limits in [Limits::default(), Limits::default().diagnostic_bytes(1)] {
        let wire = inline(
            r#"{"type":"array","items":{"type":"number","minimum":0}}"#,
            "[1,-1]",
            limits,
        );
        assert_eq!(wire["result"]["admission"], "rejected", "{wire}");
        assert_eq!(wire["outcome"]["complete"], true);
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        if wire["diagnostics"]["items"].as_array().unwrap().is_empty() {
            assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() > 0);
        } else {
            let finding = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["code"] == "sample.bound")
                .unwrap();
            assert_eq!(finding["primary"]["pointer"], "/1");
            assert_eq!(finding["primary"]["source_id"], 2);
            assert_eq!(finding["primary"]["span"], json!([3, 5]));
            assert!(
                finding["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["pointer"] == format!("{POINTER}/items/minimum"))
            );
        }
    }
}
