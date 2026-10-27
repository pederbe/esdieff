//! A reusable alarm Object that requires its own declaration through an absolute
//! pointer, and an importer whose `siren` Object copies it.
//! Follows ADR 0036: Copied reference relocation.
use esdieff::{Catalogue, SourceInput};
use serde_json::{Value, json};

pub const ALARM: &[u8] = include_bytes!("../fixtures/resolved-name-lookup/alarm.sdf.json");
pub const RELOCATION: &str = "copied-reference-relocation/1";

/// An importing document whose `siren` Object copies the alarm, with `patch`
/// members added to its composition patch.
pub fn siren(patch: Value) -> Vec<u8> {
    let mut siren = json!({"sdfRef": "o:#/sdfObject/alarm"});
    siren
        .as_object_mut()
        .unwrap()
        .extend(patch.as_object().unwrap().clone());
    serde_json::to_vec(&json!({
        "info": {},
        "namespace": {"o": "https://example.test/ocf", "q": "https://example.test/siren"},
        "defaultNamespace": "q",
        "sdfObject": {"siren": siren}
    }))
    .unwrap()
}

pub fn catalogue(siren: &[u8]) -> Catalogue {
    Catalogue::from_inputs(&[
        SourceInput::supplied("alarm", ALARM),
        SourceInput::supplied("siren", siren),
    ])
    .unwrap()
}

/// The code of each retained diagnostic in a report's wire form.
pub fn codes(wire: &Value) -> Vec<&str> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["code"].as_str().unwrap())
        .collect()
}
