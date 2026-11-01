//! Element locations follow ADR0016: Selected samples with explicit admission and coverage,
//! and ADR0020: Owned effective views and nearest origins.
use super::map_support;
use esdieff::{Admission, Catalogue, JsonBuilder, Limits, SourceInput};
use map_support::selected;
use serde_json::{Value, json};

#[test]
fn nested_array_elements_retain_sample_effective_and_nearest_source_locations() {
    let model = br##"{"sdfData":{"Box":{"type":"object","properties":{"reading":{"type":"number","maximum":1}},"required":["reading"]}},"sdfProperty":{"p":{"type":"array","items":{"type":"object","properties":{"box":{"sdfRef":"#/sdfData/Box"}},"required":["box"]}}}}"##;
    for typed in [false, true] {
        for missing in [false, true] {
            let mut previous = None;
            for limited in [false, true] {
                let builder = JsonBuilder::new();
                let sample = if typed {
                    let value = if missing {
                        builder.map(vec![]).unwrap()
                    } else {
                        builder
                            .map(vec![("reading", builder.number("2").unwrap())])
                            .unwrap()
                    };
                    SourceInput::typed(
                        "s",
                        builder
                            .array(vec![builder.map(vec![("box", value)]).unwrap()])
                            .unwrap(),
                    )
                } else {
                    SourceInput::supplied(
                        "s",
                        if missing {
                            br#"[{"box":{}}]"#
                        } else {
                            br#"[{"box":{"reading":2}}]"#
                        },
                    )
                    .sample()
                };
                let limits = if limited {
                    Limits::default().pointer_bytes(1)
                } else {
                    Limits::default()
                };
                let catalogue = Catalogue::take_inputs_with_limits(
                    vec![SourceInput::supplied("m", model), sample],
                    limits,
                )
                .unwrap();
                let source = catalogue.source(catalogue.source_id("s").unwrap()).unwrap();
                let parent = source
                    .sample_value()
                    .require()
                    .unwrap()
                    .at(0)
                    .require()
                    .unwrap()
                    .get("box")
                    .require()
                    .unwrap();
                let value = if missing {
                    parent
                } else {
                    parent.get("reading").require().unwrap()
                };
                let span = value
                    .span()
                    .map_or(Value::Null, |span| json!([span.start, span.end]));
                let report = selected(&catalogue);
                assert_eq!(
                    report.sample_result().unwrap().admission(),
                    Admission::Rejected
                );
                assert!(report.outcome().complete());
                let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
                let aggregate = (
                    wire["outcome"].clone(),
                    wire["coverage"]["requested"].clone(),
                    wire["result"]["admission"].clone(),
                );
                if let Some(previous) = &previous {
                    assert_eq!(previous, &aggregate);
                }
                previous = Some(aggregate);
                let code = if missing {
                    "sample.required"
                } else {
                    "sample.bound"
                };
                let condition = wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["code"] == code)
                    .unwrap();
                let witness = wire["result"]["witnesses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["kind"] == "rejection")
                    .unwrap();
                for (location, field) in [
                    (&condition["primary"], "diagnostics.items.primary.pointer"),
                    (&witness["sample"], "result.witnesses.sample.pointer"),
                ] {
                    assert_eq!(location["source_id"], source.id().get());
                    assert_eq!(location["span"], span);
                    if limited {
                        assert!(location["pointer"].is_null());
                        assert_eq!(location["omission"]["field"], field);
                        assert_eq!(location["omission"]["reason"], "location_limit");
                    } else {
                        assert_eq!(
                            location["pointer"],
                            if missing { "/0/box" } else { "/0/box/reading" }
                        );
                    }
                }
                let suffix = if missing {
                    "required/0"
                } else {
                    "properties/reading/maximum"
                };
                let effective = condition["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|location| location["kind"] == "effective")
                    .unwrap();
                let origin = condition["related"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|location| location["kind"] == "source")
                    .unwrap();
                if !limited {
                    assert_eq!(
                        effective["pointer"],
                        format!("/sdfProperty/p/items/properties/box/{suffix}")
                    );
                    assert_eq!(origin["pointer"], format!("/sdfData/Box/{suffix}"));
                    assert!(origin["span"].is_array());
                }
                assert_eq!(value.has_original_bytes(), !typed);
                assert_eq!(span.is_null(), typed);
            }
        }
    }
}
