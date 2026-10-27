//! Breaking witnesses and compatibility proofs for changed enum values, types and Nullable
//! characteristics. Follows ADR 0039: Client-side revision comparison.
use esdieff::{Catalogue, Classification, ComparisonRequest};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{at, catalogue, client, revised, thing, witness};

/// A panel whose `command` and `setpoint` only Clients supply and whose `status` and
/// `reading` only the Thing supplies.
const PANEL: &str = r#"{"sdfObject": {"Panel": {"sdfProperty": {
    "command": {"type": "string", "enum": ["on", "off"],
        "readable": false, "observable": false, "writable": true},
    "status": {"type": "string", "enum": ["idle", "busy"], "writable": false},
    "setpoint": {"type": "integer", "minimum": 0, "maximum": 100,
        "readable": false, "observable": false, "writable": true},
    "reading": {"type": "integer", "minimum": 0, "maximum": 100, "writable": false}
}}}}"#;

/// The panel with each `(property, quality, value)` revision applied, where a null value
/// removes the quality.
fn panel(revisions: &[(&str, &str, Value)]) -> String {
    revised(PANEL, |document| {
        let properties = &mut document["sdfObject"]["Panel"]["sdfProperty"];
        for (property, quality, value) in revisions {
            let property = properties[*property].as_object_mut().unwrap();
            if value.is_null() {
                property.remove(*quality);
            } else {
                property.insert((*quality).to_owned(), value.clone());
            }
        }
    })
}

/// The earlier and later panels with each revision applied to its own side.
fn panels(old: &[(&str, &str, Value)], new: &[(&str, &str, Value)]) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("panel", &panel(old))]),
        catalogue(&[("panel", &panel(new))]),
    )
}

#[test]
fn an_enum_value_removed_from_client_data_or_added_to_thing_data_breaks_with_that_value() {
    let (earlier, later) = panels(
        &[],
        &[
            ("command", "enum", json!(["on"])),
            ("status", "enum", json!(["busy", "idle", "error"])),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let command = at(&comparison, "/command/enum");
    assert_eq!(command.classification(), Classification::Breaking);
    assert!(command.reason().is_none());
    assert_eq!(witness(&command), client("\"off\""));
    let status = at(&comparison, "/status/enum");
    assert_eq!(status.classification(), Classification::Breaking);
    assert_eq!(witness(&status), thing("\"error\""));
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

#[test]
fn an_enum_value_added_to_client_data_or_removed_from_thing_data_is_compatible() {
    let (earlier, later) = panels(
        &[],
        &[
            ("command", "enum", json!(["off", "toggle", "on"])),
            ("status", "enum", json!(["idle"])),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/command/enum", "/status/enum"] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Compatible,
            "{suffix}"
        );
        assert!(difference.reason().is_none() && difference.witness().is_none());
    }
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn an_enum_added_beside_a_constant_is_not_proven_compatible() {
    // Each enum value replaces the constant in its own alternative, so the later revision
    // admits `busy` as well as `idle`.
    let pinned = ("status", "const", json!("idle"));
    let (earlier, later) = panels(
        &[pinned.clone(), ("status", "enum", Value::Null)],
        &[pinned],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let status = at(&comparison, "/status/enum");
    assert_eq!(status.classification(), Classification::Undetermined);
}

#[test]
fn integer_data_that_becomes_number_data_breaks_only_thing_supplied_data() {
    let number = |property| (property, "type", json!("number"));
    let (earlier, later) = panels(&[], &[number("reading"), number("setpoint")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let reading = at(&comparison, "/reading/type");
    assert_eq!(reading.classification(), Classification::Breaking);
    assert_eq!(witness(&reading), thing("0.5"));
    let setpoint = at(&comparison, "/setpoint/type");
    assert_eq!(setpoint.classification(), Classification::Compatible);
    assert!(setpoint.witness().is_none());
}

#[test]
fn a_fraction_outside_the_bounds_gives_way_to_one_just_inside_them() {
    let none = Value::Null;
    let bounded = |property, minimum: Value, maximum: Value| {
        vec![
            (property, "minimum", minimum),
            (property, "maximum", maximum),
        ]
    };
    let cases = [
        (bounded("reading", json!(10), json!(30)), "reading", "10.5"),
        (
            bounded("setpoint", json!(10), json!(30)),
            "setpoint",
            "10.5",
        ),
        (
            bounded("reading", none.clone(), json!(-10)),
            "reading",
            "-10.5",
        ),
        // A bound that is a fraction itself.
        (
            bounded("reading", json!(2.5), json!(2.75)),
            "reading",
            "2.5",
        ),
        (
            [
                bounded("reading", none, json!(2.75)),
                vec![("reading", "exclusiveMinimum", json!(2.5))],
            ]
            .concat(),
            "reading",
            "2.55",
        ),
    ];
    for (bounds, property, value) in cases {
        // Thing-supplied data becomes number data, and Client-supplied data integer data.
        let (old, new) = if property == "reading" {
            ("integer", "number")
        } else {
            ("number", "integer")
        };
        let typed = |name: &'static str| [bounds.clone(), vec![(property, "type", json!(name))]];
        let (earlier, later) = panels(&typed(old).concat(), &typed(new).concat());
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let difference = at(&comparison, &format!("/{property}/type"));
        assert_eq!(
            difference.classification(),
            Classification::Breaking,
            "{value}"
        );
        let expected = if property == "reading" {
            thing(value)
        } else {
            client(value)
        };
        assert_eq!(witness(&difference), expected);
    }
}

#[test]
fn other_type_changes_stay_undetermined_without_a_witness() {
    let (earlier, later) = panels(&[], &[("status", "type", json!("boolean"))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let status = at(&comparison, "/status/type");
    assert_eq!(status.classification(), Classification::Undetermined);
    assert_eq!(status.reason().unwrap().code(), "comparison.unproven");
}

#[test]
fn client_data_that_stops_being_nullable_and_thing_data_that_becomes_nullable_break_with_null() {
    // An absent Nullable characteristic has its Specified default `true`.
    let (earlier, later) = panels(
        &[("reading", "nullable", json!(false))],
        &[("setpoint", "nullable", json!(false))],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let setpoint = at(&comparison, "/setpoint/nullable");
    assert_eq!(setpoint.classification(), Classification::Breaking);
    assert_eq!(witness(&setpoint), client("null"));
    let reading = at(&comparison, "/reading/nullable");
    assert_eq!(reading.classification(), Classification::Breaking);
    assert_eq!(witness(&reading), thing("null"));
}

#[test]
fn client_data_that_becomes_nullable_and_thing_data_that_stops_being_nullable_are_compatible() {
    let (earlier, later) = panels(
        &[("setpoint", "nullable", json!(false))],
        &[
            ("setpoint", "nullable", json!(true)),
            ("reading", "nullable", json!(false)),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/setpoint/nullable", "/reading/nullable"] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Compatible,
            "{suffix}"
        );
    }
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn a_candidate_another_constraint_excludes_leaves_its_difference_undetermined() {
    let cases = [
        // A length limit and a constant already excluded the enum value and the null that
        // the later revision rejects.
        (
            vec![("command", "maxLength", json!(2))],
            vec![("command", "enum", json!(["on"]))],
            "/command/enum",
        ),
        (
            vec![("setpoint", "const", json!(50))],
            vec![("setpoint", "nullable", json!(false))],
            "/setpoint/nullable",
        ),
        // So had they for the enum value and the null that the earlier revision rejects.
        (
            vec![("status", "maxLength", json!(4))],
            vec![("status", "enum", json!(["idle", "busy", "waiting"]))],
            "/status/enum",
        ),
        (
            vec![
                ("reading", "const", json!(50)),
                ("reading", "nullable", json!(false)),
            ],
            vec![("reading", "nullable", json!(true))],
            "/reading/nullable",
        ),
        // Number data that stays a whole number admits no fraction.
        (
            vec![("reading", "multipleOf", json!(1))],
            vec![("reading", "type", json!("number"))],
            "/reading/type",
        ),
        (
            vec![
                ("setpoint", "type", json!("number")),
                ("setpoint", "multipleOf", json!(1)),
            ],
            vec![("setpoint", "type", json!("integer"))],
            "/setpoint/type",
        ),
    ];
    for (shared, changed, suffix) in cases {
        let (earlier, later) = panels(&shared, &[shared.clone(), changed].concat());
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Undetermined,
            "{suffix}"
        );
        assert!(difference.witness().is_none());
        assert_eq!(
            difference.reason().unwrap().code(),
            "comparison.unproven",
            "{suffix}"
        );
    }
}

#[test]
fn both_reports_show_enum_type_and_null_witnesses_alike() {
    let (earlier, later) = panels(
        &[("reading", "nullable", json!(false))],
        &[
            ("command", "enum", json!(["on"])),
            ("reading", "nullable", json!(true)),
            ("reading", "type", json!("number")),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let human = comparison.to_human();
    let panel = "/sdfObject/Panel/sdfProperty";
    for (quality, raw, direction, earlier, later) in [
        ("command/enum", "\"off\"", "client", "admitted", "rejected"),
        ("reading/nullable", "null", "thing", "rejected", "admitted"),
        ("reading/type", "0.5", "thing", "rejected", "admitted"),
    ] {
        let pointer = format!("{panel}/{quality}");
        let difference = wire["result"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|difference| difference["later"]["location"]["pointer"] == json!(pointer))
            .unwrap_or_else(|| panic!("{pointer}"));
        assert_eq!(
            difference["witness"],
            json!({"direction": format!("{direction}_supplied"), "raw_json": raw,
                "admission": {"earlier": earlier, "later": later}})
        );
        let name = quality.split('/').next_back().unwrap();
        let block = format!(
            "  changed quality {name} ({direction}-supplied data)\n    witness {raw} for {direction}-supplied data: earlier {earlier}, later {later}\n    earlier panel {pointer}\n    later panel {pointer}\n"
        );
        assert!(human.contains(&block), "{human}");
    }
}
