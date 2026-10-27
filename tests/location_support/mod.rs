//! Typed condition locations compared with the report JSON they were drawn from.
#![allow(dead_code)]
use esdieff::{Condition, Diagnostics, Field, ReportLocation, SourceLocation};
use serde_json::{Value, json};

/// The condition's primary source location, failing the test for any other kind.
pub fn primary_source(condition: &Condition) -> SourceLocation<'_> {
    match condition.primary() {
        Field::Present(ReportLocation::Source(location)) => location,
        _ => panic!("{} has no primary source location", condition.code()),
    }
}

/// The condition's primary sample location, failing the test for any other kind.
pub fn primary_sample(condition: &Condition) -> SourceLocation<'_> {
    match condition.primary() {
        Field::Present(ReportLocation::Sample(location)) => location,
        _ => panic!("{} has no primary sample location", condition.code()),
    }
}

/// Every condition's typed primary and related locations agree with its wire item.
pub fn assert_all_agree(diagnostics: &Diagnostics, wire: &str) {
    let wire: Value = serde_json::from_str(wire).unwrap();
    let items = wire["diagnostics"]["items"].as_array().unwrap();
    assert_eq!(diagnostics.conditions().len(), items.len());
    for (condition, item) in diagnostics.conditions().zip(items) {
        assert_eq!(condition.code(), item["code"]);
        assert_agree(condition, item);
    }
}

/// Kind, identity, pointer and span agree for each location, in report order. An omitted
/// primary is absent and a null primary is unavailable.
fn assert_agree(condition: &Condition, item: &Value) {
    match condition.primary() {
        Field::Present(location) => {
            assert_eq!(observed(&location), expected(&item["primary"]), "{item}")
        }
        Field::Unavailable(_) => assert!(item["primary"].is_null(), "{item}"),
        Field::Absent => assert!(item.get("primary").is_none(), "{item}"),
        _ => panic!("unexpected primary field"),
    }
    let rows = item["related"].as_array().unwrap();
    assert_eq!(condition.related().count(), rows.len(), "{item}");
    for (location, row) in condition.related().zip(rows) {
        assert_eq!(observed(&location), expected(row), "{item}");
    }
}

/// The typed location projected onto its wire fields.
fn observed(location: &ReportLocation<'_>) -> Value {
    match location {
        ReportLocation::Source(source) | ReportLocation::Sample(source) => json!({
            "kind": if matches!(location, ReportLocation::Source(_)) { "source" } else { "sample" },
            "source_id": source.source_id().get(),
            "pointer": pointer(source.pointer()),
            "span": span(source.span()),
        }),
        ReportLocation::Effective(location) => json!({
            "kind": "effective",
            "root": location.root_document().get(),
            "pointer": pointer(location.pointer()),
        }),
        ReportLocation::AuthorRequest(location) => json!({
            "kind": "author_request",
            "key": location.key(),
            "pointer": pointer(location.pointer()),
            "span": span(location.span()),
        }),
        ReportLocation::EditRequest(location) => json!({
            "kind": "edit_request",
            "key": location.key(),
            "pointer": pointer(location.pointer()),
            "span": span(location.span()),
        }),
        _ => panic!("unexpected location kind"),
    }
}

/// The wire row without presentation-only omission records.
fn expected(row: &Value) -> Value {
    let mut row = row.clone();
    let fields = row.as_object_mut().unwrap();
    fields.remove("omission");
    fields.remove("span_omission");
    row
}

fn pointer(field: Field<&str>) -> Value {
    match field {
        Field::Present(pointer) => json!(pointer),
        Field::Unavailable(_) => Value::Null,
        _ => panic!("a location pointer is never absent"),
    }
}

fn span(span: Option<std::ops::Range<u32>>) -> Value {
    span.map_or(Value::Null, |span| json!([span.start, span.end]))
}
