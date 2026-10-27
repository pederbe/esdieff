//! Shared observations of reports on documents carrying extension vocabulary, following
//! ADR 0037: Undeclared extension vocabulary.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const READING: &str = "undeclared-extensions-ignorable/1";

/// The codes of retained conditions with `severity` in the JSON report `wire`.
pub fn codes<'a>(wire: &'a Value, severity: &str) -> Vec<&'a str> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["severity"] == severity)
        .map(|row| row["code"].as_str().unwrap())
        .collect()
}

/// Whether the JSON report `wire` names the ADR 0037 reading.
pub fn names_reading(wire: &Value) -> bool {
    let interpretations = wire["baseline"]["interpretations"].as_array().unwrap();
    interpretations.iter().any(|id| id == READING)
}

/// A Temperature Object whose `value` Property is a number from -40 to 85 carrying
/// `extra` beside its base qualities.
pub fn temperature(extra: Value) -> Value {
    let mut property = json!({"type": "number", "minimum": -40, "maximum": 85});
    for (key, value) in extra.as_object().unwrap() {
        property[key] = value.clone();
    }
    json!({
        "info": {},
        "sdfObject": {"Temperature": {"sdfProperty": {"value": property}}}
    })
}
