//! Context reuse follows the recorded draft and the existing composition contracts.
use esdieff::{
    Assessment, Catalogue, CheckRequest, Class, EffectiveRequest, Field, InspectRequest, Limits,
    Lookup, OriginKind, SourceInput, View,
};
use serde_json::{Value, json};

const BASE: &[u8] = include_bytes!("../examples/context-reuse/base.sdf.json");
const DEVICE: &[u8] = include_bytes!("../examples/context-reuse/device.sdf.json");
const SENSOR: &str = "/sdfThing/Installation/sdfObject/sensor";

fn catalogue(model: &[u8]) -> Catalogue {
    Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap()
}

#[test]
fn supplied_groupings_preserve_refine_add_and_delete_context_with_nearest_origins() {
    for reversed in [false, true] {
        let mut inputs = [
            SourceInput::supplied("base", BASE),
            SourceInput::supplied("device", DEVICE),
        ];
        if reversed {
            inputs.reverse();
        }
        let catalogue = Catalogue::from_inputs(&inputs).unwrap();
        let id = catalogue.source_id("device").unwrap();
        let report = catalogue
            .check(
                CheckRequest::new()
                    .root(id)
                    .assessment(Assessment::FullBaseline),
            )
            .unwrap();
        assert!(report.outcome().complete(), "{}", report.to_json());
        assert_eq!(report.outcome().errors(), 0, "{}", report.to_json());
        // The unused, unavailable donor entry is still assessed in catalogue scope.
        assert!(
            !catalogue
                .check(CheckRequest::new())
                .unwrap()
                .outcome()
                .complete()
        );
        let wire = inspect(&catalogue, "device", "/sdfThing/Installation");
        let installation = &wire["result"]["json"];
        assert_eq!(installation["sdfContext"]["siteName"]["type"], "string");
        assert_eq!(installation["sdfContext"]["siteName"]["const"], "Office");
        let contexts = &installation["sdfObject"]["sensor"]["sdfContext"];
        assert_eq!(contexts["location"]["properties"]["floor"]["minimum"], 2);
        assert_eq!(contexts["location"]["required"][0], "room");
        assert_eq!(contexts["calibration"]["minimum"], -10);
        // The internal copied reference follows the imported Offset refinement.
        assert_eq!(contexts["calibration"]["maximum"], 5);
        assert!(contexts["calibration"].get("description").is_none());
        assert!(contexts.get("retired").is_none());
        assert_eq!(contexts["assetTag"]["const"], "A-17");
        let model = catalogue.effective(EffectiveRequest::new(id)).unwrap();
        for (tail, kind, source, source_pointer, consumer) in [
            (
                "location/properties/room/type",
                OriginKind::Inherited,
                "base",
                "/sdfData/Location/properties/room/type",
                "/sdfObject/Sensor/sdfContext/location/sdfRef".to_owned(),
            ),
            (
                "calibration/maximum",
                OriginKind::Override,
                "device",
                "/sdfThing/Installation/sdfObject/sensor/sdfData/Offset/maximum",
                "/sdfObject/Sensor/sdfContext/calibration/sdfRef".to_owned(),
            ),
            (
                "calibration/description",
                OriginKind::Deleted,
                "device",
                "/sdfThing/Installation/sdfObject/sensor/sdfContext/calibration/description",
                "/sdfObject/Sensor/sdfContext/calibration/sdfRef".to_owned(),
            ),
            (
                "retired",
                OriginKind::Deleted,
                "device",
                "/sdfThing/Installation/sdfObject/sensor/sdfContext/retired",
                format!("{SENSOR}/sdfRef"),
            ),
        ] {
            let explanation = model
                .explanation(&format!("{SENSOR}/sdfContext/{tail}"))
                .require()
                .unwrap();
            assert_eq!(explanation.kind(), kind, "{tail}");
            let nearest = explanation.sources().next().unwrap();
            assert_eq!(
                nearest.source_id(),
                catalogue.source_id(source).unwrap(),
                "{tail}"
            );
            assert_eq!(
                nearest.pointer(8192).require().unwrap(),
                source_pointer,
                "{tail}"
            );
            assert_eq!(
                explanation
                    .consumer()
                    .unwrap()
                    .pointer(8192)
                    .require()
                    .unwrap(),
                consumer,
                "{tail}"
            );
        }
        assert_eq!(
            catalogue
                .source(catalogue.source_id("base").unwrap())
                .unwrap()
                .raw_bytes(),
            BASE
        );
    }
}

#[test]
fn same_document_data_reuse_checks_final_constraints_without_importing_implicit_defaults() {
    for (nullable, incomplete, excluded) in [
        (None, true, false),
        (Some(true), false, false),
        (Some(false), false, true),
    ] {
        let mut model = json!({"sdfData":{"Text":{"type":"string","const":null}},"sdfObject":{"O":{"sdfContext":{"value":{"sdfRef":"#/sdfData/Text"},"bad":{"sdfRef":"#/sdfData/Text","minLength":"bad"}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Text"}}}}});
        if let Some(nullable) = nullable {
            model["sdfObject"]["O"]["sdfContext"]["value"]["nullable"] = json!(nullable);
        }
        let catalogue = catalogue(&serde_json::to_vec(&model).unwrap());
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let conditions = wire["diagnostics"]["items"].as_array().unwrap();
        assert!(
            conditions
                .iter()
                .any(|row| row["code"] == "data.count" && row["primary"]["kind"] == "effective"),
            "{wire}"
        );
        assert_eq!(
            conditions
                .iter()
                .any(|row| row["code"] == "consistency.const_excluded"
                    && row["primary"]["pointer"] == "/sdfData/Text/const"
                    && row["related"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|location| location["pointer"] == "/sdfObject/O/sdfContext/value")),
            excluded,
            "{wire}"
        );
        assert_eq!(
            conditions
                .iter()
                .any(|row| row["code"] == "context.nullable_default_unavailable"
                    && row["primary"]["pointer"] == "/sdfObject/O/sdfContext/value"),
            incomplete,
            "{wire}"
        );
        assert!(
            !conditions
                .iter()
                .any(|row| row["code"] == "context.nullable_default_unavailable"
                    && row["primary"]["pointer"]
                        .as_str()
                        .unwrap_or("")
                        .contains("sdfProperty")),
            "{wire}"
        );
    }
}

#[test]
fn unavailable_contexts_leave_independent_data_and_affordances_assessable() {
    for (definitions, code, limits) in [
        (
            json!({"missing":{"sdfRef":"#/sdfData/Absent"}}),
            "reference.unavailable",
            Limits::default(),
        ),
        (
            json!({"cycle":{"sdfRef":"#/sdfObject/O/sdfContext/cycle"}}),
            "reference.effective_cycle",
            Limits::default(),
        ),
        (
            json!({"limited":{"sdfRef":"#/sdfData/D"}}),
            "operation.limit",
            Limits::default().reference_applications(1),
        ),
    ] {
        let mut model = json!({"sdfData":{"D":{"sdfRef":"#/sdfData/E"},"E":{"type":"string"}},"sdfObject":{"O":{"sdfContext":definitions,"sdfProperty":{"p":{"type":"string"}}}}});
        model["sdfObject"]["O"]["sdfContext"]["independent"] =
            json!({"type":"number","multipleOf":0});
        let bytes = serde_json::to_vec(&model).unwrap();
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("model", &bytes)], limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert!(!report.outcome().complete());
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|row| row.code() == code),
            "{}",
            report.to_json()
        );
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|row| row.code() == "parameter.positive_step"),
            "{}",
            report.to_json()
        );
        let wire = inspect(&catalogue, "model", "/sdfObject/O/sdfProperty/p");
        assert_eq!(wire["result"]["state"], "available");
        assert_eq!(wire["result"]["json"]["type"], "string");
    }
}

fn inspect(catalogue: &Catalogue, source: &str, pointer: &str) -> Value {
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id(source).unwrap(), pointer)
                .view(View::Effective),
        )
        .unwrap();
    serde_json::from_str(&report.to_json()).unwrap()
}

#[test]
fn copied_context_reference_uses_the_refined_importing_definition() {
    let catalogue = catalogue(
        br##"{
      "sdfObject":{
        "Base":{"sdfContext":{
          "location":{"type":"object","properties":{"floor":{"type":"integer","minimum":0}}},
          "floor":{"sdfRef":"#/sdfObject/Base/sdfContext/location/properties/floor"}
        }},
        "Device":{"sdfRef":"#/sdfObject/Base","sdfContext":{
          "location":{"properties":{"floor":{"minimum":2}}}
        }}
      }
    }"##,
    );
    let wire = inspect(&catalogue, "model", "/sdfObject/Device/sdfContext/floor");
    assert_eq!(wire["result"]["state"], "available", "{wire}");
    assert_eq!(wire["result"]["json"]["type"], "integer");
    assert_eq!(wire["result"]["json"]["minimum"], 2);
    assert!(
        wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row == "copied-reference-relocation/1")
    );
    let id = catalogue.source_id("model").unwrap();
    let model = catalogue
        .effective(EffectiveRequest::new(id).pointer("/sdfObject/Device"))
        .unwrap();
    let Lookup::Found(context) = model.definition(Class::Context, "floor") else {
        panic!("Context floor")
    };
    assert!(matches!(
        context.data_description().require().unwrap().nullable(),
        Field::Unavailable(_)
    ));
    assert!(matches!(context.readable(), Field::Absent));
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(report.outcome().complete(), "{}", report.to_json());
    assert_eq!(report.outcome().errors(), 0, "{}", report.to_json());
}

#[test]
fn ambiguous_or_critical_contributors_do_not_hide_independent_context_findings() {
    let donor = json!({"namespace":{"shared":"https://example.com/context"},"defaultNamespace":"shared","sdfData":{"D":{"type":"integer"}}});
    let root = br##"{"namespace":{"b":"https://example.com/context"},"sdfObject":{"O":{"sdfContext":{"import":{"sdfRef":"b:#/sdfData/D"},"independent":{"type":"number","multipleOf":0}}}}}"##;
    for critical in [false, true] {
        let mut other = donor.clone();
        if critical {
            other["info"] = json!({"features":["https://example.com/unknown"]});
        }
        let first = serde_json::to_vec(&donor).unwrap();
        let second = serde_json::to_vec(&other).unwrap();
        for reverse in [false, true] {
            let mut inputs = [
                SourceInput::supplied("first", &first),
                SourceInput::supplied("second", &second),
                SourceInput::supplied("root", root),
            ];
            if reverse {
                inputs.reverse();
            }
            let catalogue = Catalogue::from_inputs(&inputs).unwrap();
            let report = catalogue
                .check(
                    CheckRequest::new()
                        .root(catalogue.source_id("root").unwrap())
                        .assessment(Assessment::FullBaseline),
                )
                .unwrap();
            assert!(!report.outcome().complete());
            assert!(
                report
                    .diagnostics()
                    .conditions()
                    .any(|condition| condition.code()
                        == if critical {
                            "structure.feature_context_unavailable"
                        } else {
                            "reference.ambiguous"
                        }),
                "{}",
                report.to_json()
            );
            assert!(
                report
                    .diagnostics()
                    .conditions()
                    .any(|condition| condition.code() == "parameter.positive_step")
            );
            let wire = inspect(&catalogue, "root", "/sdfObject/O/sdfContext/independent");
            assert_eq!(wire["result"]["json"]["multipleOf"], 0);
        }
    }
}

#[test]
fn inherited_context_requirements_remain_undefined_beside_same_name_properties() {
    for required in ["serial", "#/sdfObject/Base/sdfContext/serial"] {
        let model = json!({"sdfObject":{
            "Base":{"sdfContext":{"serial":{"type":"string"}},"sdfProperty":{"serial":{"type":"integer"}},"sdfRequired":[required]},
            "Device":{"sdfRef":"#/sdfObject/Base"}
        }});
        let catalogue = catalogue(&serde_json::to_vec(&model).unwrap());
        let report = catalogue
            .inspect(
                InspectRequest::new(catalogue.source_id("model").unwrap(), "/sdfObject/Device")
                    .view(View::Effective)
                    .required_declarations(true),
            )
            .unwrap();
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        let references: Vec<_> = wire["result"]["required_declarations"]["owners"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|owner| owner["references"].as_array().unwrap())
            .collect();
        assert!(
            references.iter().any(|reference| reference["status"]
                == if required == "serial" {
                    "resolved"
                } else {
                    "undefined_context"
                }),
            "{wire}"
        );
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code() == "context.required_undefined"),
            required != "serial"
        );
    }
}

fn cli(args: &[&str], status: i32) -> Value {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(status),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_examples_check_and_inspect_final_context_values_and_origins() {
    let base = "examples/context-reuse/base.sdf.json";
    let device = "examples/context-reuse/device.sdf.json";
    let wire = cli(
        &[
            "check",
            "--root",
            device,
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            base,
            device,
        ],
        0,
    );
    assert_eq!(wire["outcome"]["complete"], true);
    let wire = cli(
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            device,
            "--pointer",
            SENSOR,
            "--format",
            "json",
            base,
            device,
        ],
        0,
    );
    assert_eq!(
        wire["result"]["json"]["sdfContext"]["calibration"]["maximum"],
        5
    );
    assert_eq!(
        wire["result"]["json"]["sdfContext"]["assetTag"]["minLength"],
        1
    );
    assert!(
        wire["result"]["json"]["sdfContext"]
            .get("retired")
            .is_none()
    );
    for (tail, kind, source_pointer, consumer) in [
        (
            "location/properties/room/type",
            "inherited",
            "/sdfData/Location/properties/room/type",
            "/sdfObject/Sensor/sdfContext/location/sdfRef",
        ),
        (
            "calibration/maximum",
            "override",
            "/sdfThing/Installation/sdfObject/sensor/sdfData/Offset/maximum",
            "/sdfObject/Sensor/sdfContext/calibration/sdfRef",
        ),
        (
            "calibration/description",
            "deleted",
            "/sdfThing/Installation/sdfObject/sensor/sdfContext/calibration/description",
            "/sdfObject/Sensor/sdfContext/calibration/sdfRef",
        ),
    ] {
        let explanation = wire["result"]["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["effective"]["pointer"] == format!("{SENSOR}/sdfContext/{tail}"))
            .unwrap();
        assert_eq!(explanation["kind"], kind);
        assert_eq!(explanation["sources"][0]["pointer"], source_pointer);
        assert_eq!(explanation["consumer"]["pointer"], consumer);
    }
    let unavailable = "examples/context-reuse/unavailable.sdf.json";
    let check = cli(&["check", "--format", "json", unavailable], 3);
    assert_eq!(check["outcome"]["complete"], false);
    assert_eq!(check["sources"].as_array().unwrap().len(), 1);
    assert!(
        check["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "reference.unavailable")
    );
    let wire = cli(
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            unavailable,
            "--pointer",
            "/sdfObject/Device/sdfContext/assetTag",
            "--format",
            "json",
            unavailable,
        ],
        0,
    );
    assert_eq!(wire["result"]["json"]["const"], "A-17");
}

#[test]
fn context_path_composition_retains_nested_defaults_and_blocks_export_to_properties() {
    let catalogue = catalogue(br##"{"sdfObject":{"O":{
      "sdfContext":{
        "base":{"type":"object","properties":{"x":{"type":"string"}}},
        "alias":{"sdfRef":"#/sdfObject/O/sdfContext/base"},
        "user":{"sdfRef":"#/sdfObject/O/sdfContext/alias/properties/x"},
        "nested":{"type":"object","properties":{"x":{"sdfRef":"#/sdfObject/O/sdfContext/alias/properties/x"}}}
      },
      "sdfProperty":{"export":{"sdfRef":"#/sdfObject/O/sdfContext/alias/properties/x","const":null}}
    }}}"##);
    for pointer in [
        "/sdfObject/O/sdfContext/user",
        "/sdfObject/O/sdfContext/nested/properties/x",
    ] {
        let wire = inspect(&catalogue, "model", pointer);
        assert_eq!(wire["result"]["state"], "available", "{wire}");
        assert_eq!(wire["result"]["json"]["type"], "string");
        let model = catalogue
            .effective(
                EffectiveRequest::new(catalogue.source_id("model").unwrap()).pointer(pointer),
            )
            .unwrap();
        assert!(matches!(
            model
                .json()
                .require()
                .unwrap()
                .default_explanation("nullable"),
            Field::Unavailable(_)
        ));
    }
    let wire = inspect(&catalogue, "model", "/sdfObject/O/sdfProperty/export");
    assert_eq!(wire["result"]["state"], "unavailable");
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "reference.capability_unavailable")
    );
}

#[test]
fn deleting_inherited_nullability_restores_only_the_context_default_boundary() {
    let catalogue = catalogue(
        br##"{"sdfData":{"D":{"type":"string","nullable":false,"const":null}},
      "sdfObject":{"O":{"sdfContext":{"c":{"sdfRef":"#/sdfData/D","nullable":null}},
      "sdfProperty":{"p":{"sdfRef":"#/sdfData/D","nullable":null}}}}}"##,
    );
    let model = catalogue
        .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
        .unwrap();
    let pointer = "/sdfObject/O/sdfContext/c/nullable";
    assert!(matches!(
        model.default_explanation(pointer),
        Field::Unavailable(_)
    ));
    assert_eq!(
        model.explanation(pointer).require().unwrap().kind(),
        OriginKind::Deleted
    );
    assert_eq!(
        model
            .default_explanation("/sdfObject/O/sdfProperty/p/nullable")
            .require()
            .unwrap()
            .kind(),
        OriginKind::Default
    );
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "context.nullable_default_unavailable")
    );
}
