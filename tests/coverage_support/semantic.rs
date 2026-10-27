//! Consumer projections for report meaning and permitted omissions.
use serde_json::{Map, Value, json};

fn pick(value: &Value, keys: &[&str]) -> Value {
    let mut map = Map::new();
    for key in keys {
        if let Some(field) = value.get(*key) {
            map.insert((*key).into(), canonical(field));
        }
    }
    Value::Object(map)
}

fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|key| (key.clone(), canonical(&map[key])))
                    .collect(),
            )
        }
        Value::Array(rows) => Value::Array(rows.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

fn sorted(rows: impl Iterator<Item = Value>) -> Value {
    let mut rows: Vec<_> = rows.map(|row| canonical(&row)).collect();
    rows.sort_by_cached_key(|row| serde_json::to_string(row).unwrap());
    Value::Array(rows)
}

fn coordinate(value: &Value) -> Value {
    pick(value, &["kind", "source_id", "root", "pointer", "span"])
}

fn fact(condition: &Value) -> Value {
    let mut row = pick(
        condition,
        &["code", "severity", "domain", "assessment_unit", "arguments"],
    );
    row["basis"] = pick(&condition["basis"], &["kind", "id", "citation"]);
    row
}

fn detail(condition: &Value) -> Value {
    let mut row = fact(condition);
    if let Some(owner) = condition.get("coverage_owner") {
        row["coverage_owner"] = owner.clone();
    }
    if let Some(primary) = condition.get("primary") {
        row["primary"] = coordinate(primary);
    }
    row["related"] = sorted(
        condition["related"]
            .as_array()
            .into_iter()
            .flatten()
            .map(coordinate),
    );
    row
}

fn area_row(row: &Value) -> Value {
    let mut projected = pick(row, &["id", "count_unit", "status", "coverage_owner"]);
    projected["basis"] = pick(&row["basis"], &["kind", "id", "citation"]);
    projected
}

fn registry_record(row: &Value) -> Value {
    pick(
        row,
        &[
            "registry",
            "state",
            "snapshot",
            "scope",
            "extraction_policy",
            "digest",
            "raw_digest",
            "source_updated",
            "retrieved_at",
            "entries_count",
            "reason",
        ],
    )
}

fn omission_record(row: &Value) -> Value {
    pick(
        row,
        &[
            "field",
            "reason",
            "omitted_count",
            "source_id",
            "retained_bytes",
        ],
    )
}

fn unit_summary(row: &Value) -> Value {
    let mut value = pick(
        row,
        &["state", "spelling_status", "guidance_status", "reason"],
    );
    value["counts"] = if row["counts"].is_null() {
        Value::Null
    } else {
        pick(
            &row["counts"],
            &[
                "known_primary",
                "known_secondary",
                "unknown",
                "unavailable",
                "legacy",
                "uri_names",
            ],
        )
    };
    value["unavailable_reasons"] = sorted(
        row["unavailable_reasons"]
            .as_array()
            .into_iter()
            .flatten()
            .cloned(),
    );
    value
}

/// Same-model authoritative meaning, independent of optional row/condition retention.
pub fn aggregate_projection(wire: &Value) -> Value {
    assert_eq!(wire["schema"], "esdieff.report");
    assert!(wire["version"].as_str().unwrap().starts_with("1."));
    let mut request = pick(
        &wire["request"],
        &["assessment", "inventory", "selector", "sample"],
    );
    if let Some(alias) = wire["request"].get("requirement_inventory") {
        let expected = if wire["request"]["inventory"] == "esdieff.full-model/1" {
            json!("rfc9880.model/1")
        } else {
            Value::Null
        };
        assert_eq!(
            alias, &expected,
            "optional requirement alias must agree with mandatory inventory binding"
        );
    }
    let mut scope = pick(&wire["request"]["scope"], &["kind", "unavailable_required"]);
    for key in ["roots", "dependencies", "assessed"] {
        scope[key] = sorted(
            wire["request"]["scope"][key]
                .as_array()
                .into_iter()
                .flatten()
                .cloned(),
        );
    }
    request["scope"] = scope;
    let mut baseline = pick(&wire["baseline"], &["id", "rfc"]);
    if let Some(temporal) = wire["baseline"].get("temporal_knowledge") {
        baseline["temporal_knowledge"] = pick(
            temporal,
            &[
                "id",
                "from",
                "through",
                "transition_sha256",
                "bulletin_sha256",
            ],
        );
    }
    baseline["registry_knowledge"] = sorted(
        wire["baseline"]["registry_knowledge"]
            .as_array()
            .into_iter()
            .flatten()
            .map(registry_record),
    );
    if let Some(omissions) = wire["baseline"].get("metadata_omitted") {
        baseline["metadata_omitted"] = sorted(
            omissions
                .as_array()
                .into_iter()
                .flatten()
                .map(omission_record),
        );
    }
    for key in ["corrections", "interpretations", "deviations"] {
        baseline[key] = sorted(
            wire["baseline"][key]
                .as_array()
                .into_iter()
                .flatten()
                .cloned(),
        );
    }
    let mut policy = pick(&wire["policy"], &["strict_warnings", "recorded_registry"]);
    policy["restrictions"] = sorted(
        wire["policy"]["restrictions"]
            .as_array()
            .into_iter()
            .flatten()
            .cloned(),
    );
    let mut aggregate = json!({"operation":wire["operation"], "request":request, "baseline":baseline, "policy":policy,
        "outcome":pick(&wire["outcome"], &["complete", "errors", "warnings", "notices", "incomplete_checks", "strict_warnings", "disposition"]),
        "requested":pick(&wire["coverage"]["requested"], &["inventory", "count_unit", "total", "assessed", "not_applicable", "unassessed", "undetermined"]),
        "broader":pick(&wire["coverage"]["broader"], &["inventory", "state", "inventory_status"])});
    if let Some(units) = wire["result"].get("unit_guidance") {
        aggregate["unit_guidance"] = unit_summary(units);
    }
    aggregate
}

pub fn wire_projection(wire: &Value) -> Value {
    json!({"aggregate":aggregate_projection(wire),
        "diagnostic_retention":pick(&wire["diagnostics"], &["retained", "omitted", "truncated"]),
        "diagnostic_omissions":sorted(wire["diagnostics"]["omissions"].as_array().into_iter().flatten().map(omission_record)),
        "diagnostic_reasons":sorted(wire["diagnostics"]["reasons"].as_array().into_iter().flatten().cloned()),
        "conditions":sorted(wire["diagnostics"]["items"].as_array().into_iter().flatten().map(detail)),
        "rows":sorted(wire["coverage"]["rows"].as_array().into_iter().flatten().map(area_row)),
        "rows_omitted":wire["coverage"]["rows_omitted"],
        "coverage_omissions":sorted(wire["coverage"]["omissions"].as_array().into_iter().flatten().map(omission_record)),
        "requirements":wire["coverage"].get("requirements").map(|row|pick(row,&["inventory","count_unit","total","assessed","not_applicable","unassessed","undetermined"])),
        "requirement_rows":sorted(wire["coverage"]["requirement_rows"].as_array().into_iter().flatten().map(area_row)),
        "requirement_rows_retained":wire["coverage"].get("requirement_rows_retained"),
        "requirement_rows_omitted":wire["coverage"].get("requirement_rows_omitted"),
        "requirement_omissions":sorted(wire["coverage"]["requirement_omissions"].as_array().into_iter().flatten().map(omission_record))})
}

fn location_compatible(original: &Value, limited: &Value) -> bool {
    let mut original = coordinate(original);
    let limited_coordinate = coordinate(limited);
    if original["pointer"].is_string() && limited_coordinate["pointer"].is_null() {
        let omission = &limited["omission"];
        if !matches!(
            omission["reason"].as_str(),
            Some("location_limit" | "not_available")
        ) || omission["omitted_count"] != 1
            || !matches!(
                omission["field"].as_str(),
                Some("diagnostics.items.primary.pointer" | "diagnostics.items.related.pointer")
            )
            || &omission["source_id"]
                != limited_coordinate
                    .get("source_id")
                    .unwrap_or(&limited_coordinate["root"])
        {
            return false;
        }
        original["pointer"] = Value::Null;
    }
    original == limited_coordinate
}

fn retained_compatible(original: &Value, limited: &Value) -> bool {
    if fact(original) != fact(limited) {
        return false;
    }
    if let Some(owner) = limited.get("coverage_owner")
        && original.get("coverage_owner") != Some(owner)
    {
        return false;
    }
    if original.get("primary").is_some() != limited.get("primary").is_some() {
        return false;
    }
    if original.get("primary").is_some()
        && !location_compatible(&original["primary"], &limited["primary"])
    {
        return false;
    }
    let empty = Vec::new();
    let original = original["related"].as_array().unwrap_or(&empty);
    let limited = limited["related"].as_array().unwrap_or(&empty);
    if original.len() != limited.len() {
        return false;
    }
    let mut used = vec![false; original.len()];
    for row in limited {
        let Some(index) = original
            .iter()
            .enumerate()
            .position(|(index, candidate)| !used[index] && location_compatible(candidate, row))
        else {
            return false;
        };
        used[index] = true;
    }
    true
}

fn linked_omission(wire: &Value, record: &Value, field: &str, omitted: u64) -> bool {
    record["field"] == field
        && record["omitted_count"] == omitted
        && matches!(
            record["reason"].as_str(),
            Some("count_limit" | "byte_limit" | "report_limit")
        )
        && wire["diagnostics"]["truncated"] == true
        && wire["diagnostics"]["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.contains(&record["reason"]))
        && wire["diagnostics"]["omissions"]
            .as_array()
            .is_some_and(|records| {
                records
                    .iter()
                    .any(|outer| omission_record(outer) == omission_record(record))
            })
}

/// Validate the optional group emitted by the supported requirement producer.
/// Absence of the entire group gives no per-requirement status evidence.
fn valid_requirements(wire: &Value) -> bool {
    let coverage = &wire["coverage"];
    if let Some(alias) = wire["request"].get("requirement_inventory")
        && (wire["request"]["inventory"] != "esdieff.full-model/1" || alias != "rfc9880.model/1")
    {
        return false;
    }
    let fields = [
        "requirements",
        "requirement_rows",
        "requirement_rows_retained",
        "requirement_rows_omitted",
        "requirement_omissions",
    ];
    if fields.iter().all(|field| coverage.get(*field).is_none()) {
        return true;
    }
    if fields.iter().any(|field| coverage.get(*field).is_none())
        || wire["request"]["inventory"] != "esdieff.full-model/1"
    {
        return false;
    }
    let summary = &coverage["requirements"];
    if summary["inventory"] != "rfc9880.model/1"
        || summary["count_unit"] != "requirement"
        || summary["total"] != 54
    {
        return false;
    }
    let Some(counts) = ["assessed", "not_applicable", "unassessed", "undetermined"]
        .into_iter()
        .map(|field| summary[field].as_u64())
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    if counts.iter().sum::<u64>() != 54 {
        return false;
    }
    let Some(rows) = coverage["requirement_rows"].as_array() else {
        return false;
    };
    let Some(retained) = coverage["requirement_rows_retained"].as_u64() else {
        return false;
    };
    let Some(omitted) = coverage["requirement_rows_omitted"].as_u64() else {
        return false;
    };
    if retained != rows.len() as u64 || retained + omitted != 54 {
        return false;
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut row_counts = [0u64; 4];
    for row in rows {
        let Some(id) = row["id"].as_str() else {
            return false;
        };
        let Some(member) = id
            .strip_prefix("rfc9880.model.M")
            .and_then(|id| id.strip_suffix("/1"))
            .and_then(|id| id.parse::<usize>().ok())
        else {
            return false;
        };
        if !(1..=54).contains(&member)
            || id != format!("rfc9880.model.M{member:03}/1")
            || !ids.insert(id)
            || row["count_unit"] != "requirement"
        {
            return false;
        }
        let Some(status) = ["assessed", "not_applicable", "unassessed", "undetermined"]
            .iter()
            .position(|status| row["status"] == *status)
        else {
            return false;
        };
        row_counts[status] += 1;
    }
    if row_counts
        .iter()
        .zip(&counts)
        .any(|(rows, total)| rows > total)
    {
        return false;
    }
    let Some(records) = coverage["requirement_omissions"].as_array() else {
        return false;
    };
    if omitted == 0 {
        records.is_empty()
    } else {
        records.len() == 1
            && linked_omission(wire, &records[0], "coverage.requirement_rows", omitted)
    }
}

/// Omission verifies aggregates and surviving facts, never invents equality of erased lists.
pub fn allowed_omission(full: &Value, limited: &Value) -> bool {
    if !valid_requirements(full) || !valid_requirements(limited) {
        return false;
    }
    if aggregate_projection(full) != aggregate_projection(limited) {
        return false;
    }
    let full_items = full["diagnostics"]["items"].as_array().unwrap();
    let limited_items = limited["diagnostics"]["items"].as_array().unwrap();
    let numbers = |wire: &Value, key: &str| wire["diagnostics"][key].as_u64().unwrap();
    if numbers(full, "retained") != full_items.len() as u64
        || numbers(limited, "retained") != limited_items.len() as u64
        || numbers(full, "retained") + numbers(full, "omitted")
            != numbers(limited, "retained") + numbers(limited, "omitted")
        || numbers(limited, "omitted") < numbers(full, "omitted")
    {
        return false;
    }
    if numbers(limited, "omitted") > 0 && limited["diagnostics"]["truncated"] != true {
        return false;
    }
    if limited_items.len() < full_items.len()
        && limited["diagnostics"]["reasons"]
            .as_array()
            .unwrap()
            .is_empty()
    {
        return false;
    }
    let mut used = vec![false; full_items.len()];
    for row in limited_items {
        let Some(index) = full_items
            .iter()
            .enumerate()
            .position(|(index, original)| !used[index] && retained_compatible(original, row))
        else {
            return false;
        };
        used[index] = true;
        for location in row
            .get("primary")
            .into_iter()
            .chain(row["related"].as_array().into_iter().flatten())
        {
            if let Some(record) = location.get("omission")
                && (limited["diagnostics"]["truncated"] != true
                    || !limited["diagnostics"]["omissions"]
                        .as_array()
                        .unwrap()
                        .contains(record)
                    || !limited["diagnostics"]["reasons"]
                        .as_array()
                        .unwrap()
                        .contains(&record["reason"]))
            {
                return false;
            }
        }
    }
    for wire in [full, limited] {
        let rows = wire["coverage"]["rows"].as_array().unwrap();
        let omitted = wire["coverage"]["rows_omitted"].as_u64().unwrap();
        let mut ids = std::collections::BTreeSet::new();
        if !rows
            .iter()
            .all(|row| row["id"].as_str().is_some_and(|id| ids.insert(id)))
        {
            return false;
        }
        if rows.len() as u64 + omitted != wire["coverage"]["requested"]["total"].as_u64().unwrap() {
            return false;
        }
        let records: Vec<_> = wire["coverage"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["field"] == "coverage.rows")
            .collect();
        if omitted == 0 {
            if !records.is_empty() {
                return false;
            }
        } else if records.len() != 1 || !linked_omission(wire, records[0], "coverage.rows", omitted)
        {
            return false;
        }
    }
    let full_rows = full["coverage"]["rows"].as_array().unwrap();
    if !limited["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| {
            let row = area_row(row);
            full_rows.iter().any(|original| area_row(original) == row)
        })
    {
        return false;
    }
    if let (Some(original), Some(retained)) = (
        full["coverage"].get("requirements"),
        limited["coverage"].get("requirements"),
    ) {
        let fields = [
            "inventory",
            "count_unit",
            "total",
            "assessed",
            "not_applicable",
            "unassessed",
            "undetermined",
        ];
        if pick(original, &fields) != pick(retained, &fields) {
            return false;
        }
    }
    let original = full["coverage"]["requirement_rows"].as_array();
    limited["coverage"]["requirement_rows"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|row| {
            original.is_some_and(|rows| {
                rows.iter()
                    .any(|candidate| area_row(candidate) == area_row(row))
            })
        })
}
