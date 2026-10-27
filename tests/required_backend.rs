//! Qualify the actual report backend behavior used by this test executable.
//! Follows ADR 0026: Required declaration relationships and source evidence.
use serde_json::Value;
#[test]
fn resolved_backend_runtime_evidence() {
    let value: Value =
        serde_json::from_str(r#"{"z":123456789012345678901234567890,"a":0}"#).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(",");
    let number = value["z"].to_string();
    let exact = number == "123456789012345678901234567890";
    println!(
        "serde_json_runtime key_order={keys}; number={number}; arbitrary_precision_exact={exact}; Value_bytes={}; Map_bytes={}",
        std::mem::size_of::<Value>(),
        std::mem::size_of::<serde_json::Map<String, Value>>()
    );
    assert!(matches!(keys.as_str(), "a,z" | "z,a"));
}
