// Nested sample provenance follows ADR0016: Selected samples with explicit admission and coverage.
use esdieff::{Admission, Catalogue, JsonBuilder, Limits, SampleKind, SampleRequest, SourceInput};
use serde_json::{Value, json};
use std::ops::Range;

const MODEL: &[u8] = br#"{"sdfProperty":{"p":{"type":"object","properties":{"box":{"type":"object","properties":{"reading":{"type":"number","maximum":1}},"required":["reading"]}},"required":["box"]}}}"#;

fn sample_input(typed: bool, missing: bool) -> SourceInput<'static> {
    if !typed {
        return SourceInput::supplied(
            "s",
            if missing {
                br#"{"box":{}}"#
            } else {
                br#"{"box":{"reading":2}}"#
            },
        )
        .sample();
    }
    let builder = JsonBuilder::new();
    let inner = if missing {
        builder.map(vec![]).unwrap()
    } else {
        builder
            .map(vec![("reading", builder.number("2").unwrap())])
            .unwrap()
    };
    SourceInput::typed("s", builder.map(vec![("box", inner)]).unwrap())
}

fn span_json(span: Option<Range<usize>>) -> Value {
    span.map_or(Value::Null, |span| json!([span.start, span.end]))
}

fn assert_omitted_sample(location: &Value, field: &str, source: u32, span: &Value) {
    assert_eq!(location["kind"], "sample");
    assert_eq!(location["source_id"], source);
    assert!(location["pointer"].is_null());
    assert_eq!(&location["span"], span);
    assert_eq!(location["omission"]["field"], field);
    assert_eq!(location["omission"]["reason"], "location_limit");
    assert_eq!(location["omission"]["source_id"], source);
    assert_eq!(location["omission"]["omitted_count"], 1);
}

fn nested_control(typed: bool, missing: bool) {
    let constraint = if missing { "required" } else { "maximum" };
    let pointer = if missing { "/box" } else { "/box/reading" };
    let mut aggregate: Option<(Value, Value, Value)> = None;
    for omitted in [false, true] {
        let limits = if omitted {
            Limits::default().pointer_bytes(1)
        } else {
            Limits::default()
        };
        let catalogue = Catalogue::take_inputs_with_limits(
            vec![
                SourceInput::supplied("m", MODEL),
                sample_input(typed, missing),
            ],
            limits,
        )
        .unwrap();
        let model_id = catalogue.source_id("m").unwrap();
        let sample_id = catalogue.source_id("s").unwrap();
        let root = catalogue
            .source(sample_id)
            .unwrap()
            .sample_value()
            .require()
            .unwrap();
        let parent = root.get("box").require().unwrap();
        let node = if missing {
            parent
        } else {
            parent.get("reading").require().unwrap()
        };
        assert_eq!(node.raw_json(), if missing { "{}" } else { "2" });
        assert_eq!(node.source_id(), sample_id);
        assert_eq!(node.has_original_bytes(), !typed);
        let expected_span = span_json(node.span());
        if typed {
            assert!(expected_span.is_null());
        } else {
            assert_ne!(expected_span, span_json(root.span()));
        }
        let box_model = catalogue
            .source(model_id)
            .unwrap()
            .json()
            .require()
            .unwrap()
            .get("sdfProperty")
            .require()
            .unwrap()
            .get("p")
            .require()
            .unwrap()
            .get("properties")
            .require()
            .unwrap()
            .get("box")
            .require()
            .unwrap();
        let parameter = if missing {
            box_model
                .get("required")
                .require()
                .unwrap()
                .at(0)
                .require()
                .unwrap()
        } else {
            box_model
                .get("properties")
                .require()
                .unwrap()
                .get("reading")
                .require()
                .unwrap()
                .get("maximum")
                .require()
                .unwrap()
        };
        let parameter_span = span_json(Some(parameter.span()));
        let report = catalogue
            .check_sample(
                SampleRequest::new(model_id, SampleKind::Property, "/sdfProperty/p")
                    .sample(sample_id),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert!(report.outcome().complete());
        assert!(report.outcome().errors() > 0);
        let coverage = report.requested_coverage();
        assert_eq!(coverage.total(), Some(9));
        assert_eq!(
            coverage.assessed()
                + coverage.not_applicable()
                + coverage.unassessed()
                + coverage.undetermined(),
            9
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["result"]["admission"], "rejected");
        assert!(report.to_human().contains("rejected"));
        let current = (
            wire["outcome"].clone(),
            wire["coverage"]["requested"].clone(),
            wire["result"]["admission"].clone(),
        );
        if let Some(previous) = &aggregate {
            assert_eq!(&current, previous);
        }
        aggregate = Some(current);
        let condition = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| {
                row["domain"] == "sample"
                    && row["severity"] == "error"
                    && row["arguments"]["constraint"] == constraint
            })
            .unwrap();
        let witness = wire["result"]["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["kind"] == "rejection" && row["constraint"] == constraint)
            .unwrap();
        assert!(
            condition["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["kind"] == "source"
                    && row["source_id"] == model_id.get()
                    && row["span"] == parameter_span)
        );
        assert_eq!(witness["effective"]["kind"], "effective");
        assert_eq!(witness["effective"]["root"], model_id.get());
        if omitted {
            assert_omitted_sample(
                &condition["primary"],
                "diagnostics.items.primary.pointer",
                sample_id.get(),
                &expected_span,
            );
            assert_omitted_sample(
                &witness["sample"],
                "result.witnesses.sample.pointer",
                sample_id.get(),
                &expected_span,
            );
            assert!(
                wire["diagnostics"]["omissions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["field"] == "diagnostics.items.primary.pointer"
                        && row["source_id"] == sample_id.get()
                        && row["reason"] == "location_limit")
            );
        } else {
            for location in [&condition["primary"], &witness["sample"]] {
                assert_eq!(location["kind"], "sample");
                assert_eq!(location["source_id"], sample_id.get());
                assert_eq!(location["pointer"], pointer);
                assert_eq!(location["span"], expected_span);
            }
        }
    }
}

#[test]
fn nested_scalar_rejection_preserves_original_and_typed_omission_provenance() {
    for typed in [false, true] {
        nested_control(typed, false);
    }
}

#[test]
fn nested_required_absence_uses_actual_parent_and_required_index_origin() {
    for typed in [false, true] {
        nested_control(typed, true);
    }
}
