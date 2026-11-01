//! Context differences share revision comparison without becoming interactions.
use esdieff::{
    Class, Classification, ComparisonRequest, DifferenceKind, DifferenceSubject, Revision,
};
use serde_json::json;

use super::comparison_support;
use comparison_support::{at, catalogue, revised};

const MODEL: &str = r#"{
    "info":{"version":"1.0.0"},
    "sdfObject":{"Device":{
        "sdfContext":{"calibration":{"type":"number","maximum":10}}
    }}
}"#;
const CONTEXT: &str = "/sdfObject/Device/sdfContext/calibration";

#[test]
fn changed_context_constraints_are_compatible_for_client_interactions() {
    let later = revised(MODEL, |document| {
        document["sdfObject"]["Device"]["sdfContext"]["calibration"]["maximum"] = json!(5);
    });
    let earlier = catalogue(&[("model", MODEL)]);
    let later = catalogue(&[("model", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 1);
    let difference = at(&comparison, CONTEXT);
    assert_eq!(difference.kind(), DifferenceKind::Changed);
    assert_eq!(
        difference.subject(),
        DifferenceSubject::Definition(Class::Context)
    );
    assert_eq!(difference.classification(), Classification::Compatible);
    assert_eq!(difference.directions().count(), 0);
    assert!(difference.witness().is_none());
    assert!(difference.side(Revision::Earlier).is_some());
    assert!(difference.side(Revision::Later).is_some());
    assert_eq!(comparison.version_warnings().len(), 0);
}

#[test]
fn context_additions_and_removals_keep_names_separate_from_properties() {
    let earlier = json!({"sdfThing":{"Site":{
        "sdfContext":{"serial/number":{"type":"string"}},
        "sdfObject":{"Device":{
            "sdfContext":{"value":{"type":"number"}},
            "sdfProperty":{"value":{"type":"number","maximum":10}}
        }}
    }}})
    .to_string();
    let later = revised(&earlier, |document| {
        let thing = &mut document["sdfThing"]["Site"];
        thing.as_object_mut().unwrap().remove("sdfContext");
        let object = &mut thing["sdfObject"]["Device"];
        object["sdfContext"] = json!({"replacement":{"type":"string"}});
        object["sdfProperty"]["value"]["maximum"] = json!(11);
    });
    let earlier = catalogue(&[("model", &earlier)]);
    let later = catalogue(&[("model", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.count(Classification::Breaking), 1);
    assert_eq!(comparison.count(Classification::Compatible), 3);
    for (suffix, kind) in [
        ("/sdfContext/serial~1number", DifferenceKind::Removed),
        ("/sdfContext/value", DifferenceKind::Removed),
        ("/sdfContext/replacement", DifferenceKind::Added),
    ] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.subject(),
            DifferenceSubject::Definition(Class::Context)
        );
        assert_eq!(difference.kind(), kind);
        assert_eq!(difference.classification(), Classification::Compatible);
        assert_eq!(difference.directions().count(), 0);
    }
    assert_eq!(
        at(&comparison, "/sdfProperty/value/maximum").classification(),
        Classification::Breaking
    );
}

#[test]
fn unavailable_context_content_is_scoped_even_beside_a_known_metadata_change() {
    let model = r##"{"sdfObject":{"Device":{
        "sdfContext":{"calibration":{"type":"object","properties":{
            "offset":{"type":"number"},
            "unknown":{"sdfRef":"#/sdfData/Missing"}
        }}},
        "sdfProperty":{"reading":{"type":"integer","maximum":10}}
    }}}"##;
    let later = revised(model, |document| {
        let object = &mut document["sdfObject"]["Device"];
        object["sdfContext"]["calibration"]["description"] = json!("Revised metadata");
        object["sdfContext"]["added"] = json!({"sdfRef":"#/sdfData/AlsoMissing"});
        object["sdfProperty"]["reading"]["maximum"] = json!(11);
    });
    let earlier = catalogue(&[("model", model)]);
    let later = catalogue(&[("model", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    assert!(!comparison.complete());
    for pointer in [CONTEXT, "/sdfContext/added"] {
        let difference = at(&comparison, pointer);
        assert_eq!(difference.kind(), DifferenceKind::Unavailable);
        assert_eq!(difference.classification(), Classification::Undetermined);
        assert!(
            difference
                .reason()
                .unwrap()
                .code()
                .starts_with("reference.")
        );
        assert_eq!(difference.directions().count(), 0);
    }
    assert_eq!(
        at(&comparison, "/sdfProperty/reading/maximum").classification(),
        Classification::Breaking
    );
}

#[test]
fn reports_limit_compatibility_to_client_interactions() {
    let later = revised(MODEL, |document| {
        document["sdfObject"]["Device"]
            .as_object_mut()
            .unwrap()
            .remove("sdfContext");
    });
    let earlier = catalogue(&[("model", MODEL)]);
    let later = catalogue(&[("model", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: serde_json::Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["version"], "1.7");
    assert_eq!(
        wire["request"]["comparison"],
        "esdieff.revision-comparison/9"
    );
    assert_eq!(wire["policy"]["compatibility_scope"], "client_interactions");
    assert_eq!(
        wire["baseline"]["extensions"][0]["id"],
        "draft-ietf-asdf-sdf-nonaffordance-06"
    );
    assert_eq!(wire["result"]["differences"][0]["subject"], "sdfContext");
    assert_eq!(wire["result"]["differences"][0]["directions"], json!([]));
    let scope = "Client interactions only. Context metadata compatibility is not assessed.";
    assert!(comparison.to_human().contains(scope));
    assert!(comparison.to_markdown().contains(scope));
}

#[test]
fn inherited_and_refined_context_is_compared_in_each_corresponding_grouping() {
    let base = r#"{"namespace":{"base":"https://example.com/base"},"defaultNamespace":"base",
        "sdfObject":{"Sensor":{"sdfContext":{
            "calibration":{"type":"number","maximum":10},
            "retired":{"type":"boolean"}
        }}},
        "sdfThing":{"Site":{"sdfContext":{"name":{"type":"string","const":"Office"}}}}
    }"#;
    let device = r##"{"namespace":{"base":"https://example.com/base"},
        "sdfThing":{"Installation":{
            "sdfRef":"base:#/sdfThing/Site",
            "sdfObject":{"sensor":{"sdfRef":"base:#/sdfObject/Sensor"}}
        }}
    }"##;
    let later = revised(device, |document| {
        let site = &mut document["sdfThing"]["Installation"];
        site["sdfContext"] = json!({"name":{"const":"Factory"}});
        site["sdfObject"]["sensor"]["sdfContext"] = json!({
            "calibration":{"maximum":5},"retired":null,"added":{"type":"string"}
        });
    });
    let earlier = catalogue(&[("device", device), ("base", base)]);
    let later = catalogue(&[("device", &later), ("base", base)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 4);
    for (suffix, kind) in [
        ("/sdfContext/name", DifferenceKind::Changed),
        ("/sdfContext/calibration", DifferenceKind::Changed),
        ("/sdfContext/retired", DifferenceKind::Removed),
        ("/sdfContext/added", DifferenceKind::Added),
    ] {
        let difference = at(&comparison, suffix);
        assert_eq!(difference.kind(), kind);
        assert_eq!(difference.classification(), Classification::Compatible);
    }
    let wire: serde_json::Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let removed = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| difference["kind"] == "removed")
        .unwrap();
    assert_eq!(
        removed["earlier"]["origin"]["source_id"],
        earlier.source_id("base").unwrap().get()
    );
}

#[test]
fn shared_data_changes_still_break_properties_that_reuse_it() {
    let model = r##"{"sdfData":{"Value":{"type":"integer","maximum":10,"nullable":false}},
        "sdfObject":{"Device":{
            "sdfContext":{"calibration":{"sdfRef":"#/sdfData/Value"}},
            "sdfProperty":{"reading":{"sdfRef":"#/sdfData/Value","writable":false}}
        }}
    }"##;
    let later = revised(model, |document| {
        document["sdfData"]["Value"]["maximum"] = json!(20);
    });
    let earlier = catalogue(&[("model", model)]);
    let later = catalogue(&[("model", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        at(&comparison, CONTEXT).classification(),
        Classification::Compatible
    );
    let property = at(&comparison, "/sdfProperty/reading/maximum");
    assert_eq!(property.classification(), Classification::Breaking);
    assert_eq!(
        comparison_support::witness(&property),
        comparison_support::thing("20")
    );
    assert_eq!(comparison.count(Classification::Undetermined), 0);
}

#[test]
fn moving_a_property_to_context_keeps_the_affordance_removal() {
    let model = r#"{"info":{"version":"1"},"sdfObject":{"Device":{
        "sdfProperty":{"serial":{"type":"string","writable":false}}
    }}}"#;
    let later = revised(model, |document| {
        let object = &mut document["sdfObject"]["Device"];
        object.as_object_mut().unwrap().remove("sdfProperty");
        object["sdfContext"] = json!({"serial":{"type":"string"}});
    });
    let earlier = catalogue(&[("model", model)]);
    let later = catalogue(&[("model", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let property = at(&comparison, "/sdfProperty/serial");
    assert_eq!(property.kind(), DifferenceKind::Removed);
    assert_eq!(property.classification(), Classification::Breaking);
    let context = at(&comparison, "/sdfContext/serial");
    assert_eq!(context.kind(), DifferenceKind::Added);
    assert_eq!(context.classification(), Classification::Compatible);
    assert_eq!(comparison.version_warnings().len(), 1);
}

#[test]
fn unsupported_context_exports_and_critical_features_preserve_independent_changes() {
    let model = r##"{"sdfObject":{"Device":{
        "sdfContext":{"calibration":{"type":"number"}},
        "sdfProperty":{
            "exported":{"sdfRef":"#/sdfObject/Device/sdfContext/calibration"},
            "reading":{"type":"number","maximum":10}
        }
    }}}"##;
    let later = revised(model, |document| {
        document["sdfObject"]["Device"]["sdfProperty"]["reading"]["maximum"] = json!(11);
    });
    let critical = r#"{"info":{"features":["https://example.com/unknown"]},
        "sdfObject":{"Unknown":{"sdfContext":{"metadata":{"type":"string"}}}}
    }"#;
    let earlier = catalogue(&[("model", model), ("critical", critical)]);
    let later = catalogue(&[("model", &later), ("critical", critical)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    assert!(!comparison.complete());
    assert_eq!(
        at(&comparison, "/sdfProperty/exported")
            .reason()
            .unwrap()
            .code(),
        "reference.capability_unavailable"
    );
    assert_eq!(
        at(&comparison, "/sdfProperty/reading/maximum").classification(),
        Classification::Breaking
    );
    assert!(comparison.differences().any(|difference| {
        difference
            .reason()
            .is_some_and(|reason| reason.code() == "structure.feature_context_unavailable")
    }));
}

#[test]
fn exact_metadata_values_and_map_order_do_not_create_differences() {
    let earlier = catalogue(&[("model", MODEL)]);
    let later = catalogue(&[(
        "model",
        r#"{"sdfObject":{"Device":{"sdfContext":{
        "calibration":{"maximum":1e1,"type":"number"}
    }}},"info":{"version":"1.0.0"}}"#,
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.differences().len(), 0);
}

#[test]
fn unavailable_metadata_scalars_cannot_be_hidden_by_another_difference() {
    for value in ["1e1000001", r#""\ud800""#, r#"{"\ud800":1}"#] {
        let before = [
            r#"{"sdfObject":{"Device":{"sdfContext":{"value":{"const":"#,
            value,
            "}}}}}",
        ]
        .concat();
        let after = before.replace("\"const\":", "\"description\":\"Updated\",\"const\":");
        let empty = r#"{"sdfObject":{"Device":{}}}"#;
        for (earlier, later) in [
            (before.as_str(), after.as_str()),
            (empty, before.as_str()),
            (before.as_str(), empty),
        ] {
            let earlier = catalogue(&[("model", earlier)]);
            let later = catalogue(&[("model", later)]);
            let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
            assert_eq!(
                comparison.outcome(),
                Classification::Undetermined,
                "{value}: {}",
                comparison.to_json()
            );
            let difference = at(&comparison, "/sdfContext/value");
            assert_eq!(difference.kind(), DifferenceKind::Unavailable);
            assert_eq!(difference.classification(), Classification::Undetermined);
            assert!(difference.reason().is_some());
        }
    }
}

#[test]
fn limits_keep_unfinished_context_and_completed_interaction_results_visible() {
    let property = r#"{"sdfObject":{"Device":{"sdfProperty":{"removed":{"type":"boolean"}}}}}"#;
    let empty = r#"{"sdfObject":{"Device":{}}}"#;
    let metadata = json!({"sdfObject":{"Metadata":{"sdfContext":{"values":{
        "type":"array","const":vec![true; 256]
    }}}}})
    .to_string();
    let changed = metadata.replace("true", "false");
    let mut partial = false;
    let mut complete = false;
    for work in (5000..=100000).step_by(5000) {
        let limits = esdieff::Limits::default().decoding_work(work);
        let inputs = |context: &str, device: &str| {
            esdieff::Catalogue::with_limits(
                &[
                    esdieff::SourceInput::supplied("device", device.as_bytes()),
                    esdieff::SourceInput::supplied("metadata", context.as_bytes()),
                ],
                limits.clone(),
            )
        };
        let (Ok(earlier), Ok(later)) = (inputs(&metadata, property), inputs(&changed, empty))
        else {
            continue;
        };
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        if comparison.count(Classification::Breaking) == 1 {
            if comparison.complete() {
                complete = true;
                assert_eq!(comparison.count(Classification::Compatible), 1);
            } else {
                partial = true;
                assert!(comparison.differences().any(|difference| {
                    difference
                        .reason()
                        .is_some_and(|reason| reason.code() == "operation.limit")
                }));
            }
        }
    }
    assert!(
        partial,
        "a bounded comparison keeps the earlier interaction removal"
    );
    assert!(complete, "sufficient work compares Context too");
}
