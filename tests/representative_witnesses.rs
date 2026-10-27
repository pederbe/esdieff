//! Breaking witnesses taken from representative values of the revision that must admit
//! them. Follows ADR 0039: Client-side revision comparison.
use esdieff::{
    Catalogue, Classification, ComparisonRequest, Limits, Revision, SourceInput, UndeterminedReason,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{at, catalogue, client, revised, thing, witness};

/// A valve whose `mode` and `settings` only Clients supply and whose `state` and `report`
/// only the Thing supplies.
const VALVE: &str = r#"{"sdfObject": {"Valve": {"sdfProperty": {
    "mode": {"type": "string", "readable": false, "observable": false, "writable": true},
    "state": {"type": "string", "writable": false},
    "settings": {"type": "object", "readable": false, "observable": false, "writable": true,
        "properties": {
            "flow": {"type": "integer", "minimum": 5, "maximum": 50},
            "label": {"type": "string", "minLength": 2}}},
    "report": {"type": "object", "writable": false,
        "properties": {
            "flow": {"type": "integer", "minimum": 5},
            "fault": {"type": "string"}}}
}}}}"#;

/// The valve with each `(property, quality, value)` revision applied, where a null value
/// removes the quality.
fn valve(revisions: &[(&str, &str, Value)]) -> String {
    revised(VALVE, |document| {
        let properties = &mut document["sdfObject"]["Valve"]["sdfProperty"];
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

/// The earlier and later valves with each revision applied to its own side.
fn valves(old: &[(&str, &str, Value)], new: &[(&str, &str, Value)]) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("valve", &valve(old))]),
        catalogue(&[("valve", &valve(new))]),
    )
}

#[test]
fn a_type_change_between_unrelated_types_breaks_with_a_representative_value_of_each_side() {
    let boolean = |property| (property, "type", json!("boolean"));
    let (earlier, later) = valves(&[], &[boolean("mode"), boolean("state")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mode = at(&comparison, "/mode/type");
    assert_eq!(mode.classification(), Classification::Breaking);
    assert_eq!(witness(&mode), client("\"\""));
    let state = at(&comparison, "/state/type");
    assert_eq!(state.classification(), Classification::Breaking);
    assert_eq!(witness(&state), thing("true"));
}

#[test]
fn a_type_added_to_client_data_or_removed_from_thing_data_breaks_with_a_value_of_another_type() {
    let untyped = |property| (property, "type", Value::Null);
    let (earlier, later) = valves(&[untyped("mode")], &[untyped("state")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mode = at(&comparison, "/mode/type");
    assert_eq!(mode.classification(), Classification::Breaking);
    assert_eq!(witness(&mode), client("0"));
    let state = at(&comparison, "/state/type");
    assert_eq!(state.classification(), Classification::Breaking);
    assert_eq!(witness(&state), thing("0"));
}

/// The witness when the write-only `mode`, described by `earlier`, becomes boolean data.
fn retyped(earlier: Value) -> Option<String> {
    let write_only = |mut description: Value| {
        let map = description.as_object_mut().unwrap();
        for (characteristic, value) in [
            ("readable", false),
            ("observable", false),
            ("writable", true),
        ] {
            map.insert(characteristic.to_owned(), json!(value));
        }
        description
    };
    let with_mode = |description: Value| {
        revised(VALVE, |document| {
            document["sdfObject"]["Valve"]["sdfProperty"]["mode"] = write_only(description);
        })
    };
    let earlier = catalogue(&[("valve", &with_mode(earlier))]);
    let later = catalogue(&[("valve", &with_mode(json!({"type": "boolean"})))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mode = at(&comparison, "/mode/type");
    mode.witness().map(|found| {
        assert_eq!(witness(&mode), client(found.value()));
        found.value().to_owned()
    })
}

#[test]
fn representative_values_follow_the_bounds_lengths_formats_and_counts_of_their_type() {
    let cases = [
        (json!({"type": "number"}), "0"),
        (
            json!({"type": "number", "minimum": 2.5, "maximum": 10}),
            "2.5",
        ),
        (json!({"type": "integer", "exclusiveMinimum": 0}), "1"),
        (
            json!({"type": "integer", "minimum": -10, "exclusiveMinimum": -4}),
            "-3",
        ),
        (json!({"type": "integer", "exclusiveMaximum": 10}), "9"),
        (json!({"type": "number", "maximum": -1.25}), "-1.25"),
        (json!({"type": "string", "minLength": 3}), "\"aaa\""),
        (
            json!({"type": "string", "format": "date-time"}),
            "\"1970-01-01T00:00:00Z\"",
        ),
        (
            json!({"type": "string", "format": "date"}),
            "\"1970-01-01\"",
        ),
        (json!({"type": "string", "format": "time"}), "\"00:00:00Z\""),
        (
            json!({"type": "string", "format": "uri"}),
            "\"https://example.com\"",
        ),
        (json!({"type": "string", "format": "uri-reference"}), "\"\""),
        (
            json!({"type": "string", "format": "uuid"}),
            "\"00000000-0000-0000-0000-000000000000\"",
        ),
        (json!({"type": "array"}), "[]"),
        (
            json!({"type": "array", "minItems": 2, "items": {"type": "integer", "minimum": 3}}),
            "[3,3]",
        ),
        (json!({"type": "array", "minItems": 1}), "[null]"),
    ];
    for (description, expected) in cases {
        assert_eq!(
            retyped(description.clone()).as_deref(),
            Some(expected),
            "{description}"
        );
    }
}

#[test]
fn the_constant_enum_values_and_default_come_before_a_value_of_each_type() {
    let cases = [
        (json!({"type": "integer", "const": 7}), "7"),
        (
            json!({"type": "string", "enum": ["low", "high"]}),
            "\"low\"",
        ),
        (json!({"type": "string", "default": "auto"}), "\"auto\""),
        // A default the description rejects gives way to the next representative value.
        (
            json!({"type": "string", "minLength": 2, "default": "x"}),
            "\"aa\"",
        ),
        // Without a type, every base type is admitted, numbers first.
        (json!({"minimum": 4}), "4"),
    ];
    for (description, expected) in cases {
        assert_eq!(
            retyped(description.clone()).as_deref(),
            Some(expected),
            "{description}"
        );
    }
}

#[test]
fn a_representative_map_fills_each_required_entry_from_its_own_description() {
    let nested = json!({"type": "object", "required": ["depth"],
        "properties": {"depth": {"type": "integer", "minimum": 2}}});
    let settings = |required: Value| {
        let text = revised(&valve(&[("settings", "required", required)]), |document| {
            document["sdfObject"]["Valve"]["sdfProperty"]["settings"]["properties"]["nested"] =
                nested.clone();
        });
        catalogue(&[("valve", &text)])
    };
    // An entry without a description is filled with `null`.
    let earlier = settings(json!(["flow", "extra", "nested"]));
    let later = settings(json!(["flow", "extra", "nested", "label"]));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let required = at(&comparison, "/settings/required");
    assert_eq!(
        witness(&required),
        client(r#"{"flow":5,"extra":null,"nested":{"depth":2}}"#)
    );
}

#[test]
fn a_representative_value_another_constraint_excludes_leaves_its_difference_undetermined() {
    // Neither the empty string nor a flow of 1 meets the constraints of the revision that
    // must admit it, and the mode and state admit no `null`, while both revisions admit a
    // `null` setting or report. The Client-supplied mode and settings narrow, and the
    // Thing-supplied state and report widen.
    let digits = |property| (property, "pattern", json!("^[0-9]+$"));
    let not_null = |property| (property, "nullable", json!(false));
    let pinned = |property| (property, "const", json!("1"));
    let stepped = |property, other: &str| {
        let mut properties = json!({"flow": {"type": "integer", "minimum": 1, "multipleOf": 7}});
        properties[other] = json!({"type": "string"});
        (property, "properties", properties)
    };
    let required = |property, names: &[&str]| (property, "required", json!(names));
    let (earlier, later) = valves(
        &[
            digits("mode"),
            not_null("mode"),
            stepped("settings", "label"),
            required("settings", &["flow"]),
            digits("state"),
            not_null("state"),
            pinned("state"),
            stepped("report", "fault"),
            required("report", &["flow", "fault"]),
        ],
        &[
            digits("mode"),
            not_null("mode"),
            pinned("mode"),
            stepped("settings", "label"),
            required("settings", &["flow", "label"]),
            digits("state"),
            not_null("state"),
            stepped("report", "fault"),
            required("report", &["flow"]),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in [
        "/mode/const",
        "/settings/required",
        "/state/const",
        "/report/required",
    ] {
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
fn an_enum_added_to_client_data_or_removed_from_thing_data_breaks_with_a_value_outside_it() {
    let listed = |property| (property, "enum", json!(["open", "shut"]));
    let (earlier, later) = valves(&[listed("state")], &[listed("mode")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mode = at(&comparison, "/mode/enum");
    assert_eq!(mode.classification(), Classification::Breaking);
    assert_eq!(witness(&mode), client("\"\""));
    let state = at(&comparison, "/state/enum");
    assert_eq!(state.classification(), Classification::Breaking);
    assert_eq!(witness(&state), thing("\"\""));
}

#[test]
fn an_enum_removed_from_client_data_or_added_to_thing_data_is_compatible() {
    let listed = |property| (property, "enum", json!(["open", "shut"]));
    let (earlier, later) = valves(&[listed("mode")], &[listed("state")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/mode/enum", "/state/enum"] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Compatible,
            "{suffix}"
        );
        assert!(difference.witness().is_none());
    }
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn a_newly_required_entry_beside_another_breaks_with_a_witness_map_holding_only_the_other() {
    let (earlier, later) = valves(
        &[("settings", "required", json!(["flow"]))],
        &[("settings", "required", json!(["flow", "label"]))],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let required = at(&comparison, "/settings/required");
    assert_eq!(required.classification(), Classification::Breaking);
    assert_eq!(witness(&required), client(r#"{"flow":5}"#));
}

#[test]
fn a_required_entry_dropped_from_thing_data_breaks_with_a_witness_map_lacking_it() {
    let (earlier, later) = valves(
        &[("report", "required", json!(["flow", "fault"]))],
        &[("report", "required", json!(["flow"]))],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let required = at(&comparison, "/report/required");
    assert_eq!(required.classification(), Classification::Breaking);
    assert_eq!(witness(&required), thing(r#"{"flow":5}"#));
}

#[test]
fn a_constant_added_to_client_data_breaks_and_a_removed_one_is_compatible() {
    let pinned = |property| (property, "const", json!("open"));
    let (earlier, later) = valves(&[pinned("state")], &[pinned("mode")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mode = at(&comparison, "/mode/const");
    assert_eq!(mode.classification(), Classification::Breaking);
    assert_eq!(witness(&mode), client("\"\""));
    let state = at(&comparison, "/state/const");
    assert_eq!(state.classification(), Classification::Breaking);
    assert_eq!(witness(&state), thing("\"\""));

    let (earlier, later) = valves(&[pinned("mode")], &[pinned("state")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/mode/const", "/state/const"] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Compatible,
            "{suffix}"
        );
    }
}

#[test]
fn a_changed_constant_breaks_with_the_constant_of_the_revision_that_admits_it() {
    // The witness keeps the spelling of each number in its constant.
    let pinned = |mode, report: &str| {
        let text = valve(&[
            ("mode", "const", json!(mode)),
            ("report", "const", json!("REPORT")),
        ]);
        catalogue(&[("valve", &text.replace("\"REPORT\"", report))])
    };
    let earlier = pinned("open", r#"{"flow": 10, "fault": "none"}"#);
    let later = pinned("shut", r#"{"flow": 2.50e1, "fault": "none"}"#);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mode = at(&comparison, "/mode/const");
    assert_eq!(mode.classification(), Classification::Breaking);
    assert_eq!(witness(&mode), client("\"open\""));
    let report = at(&comparison, "/report/const");
    assert_eq!(report.classification(), Classification::Breaking);
    assert_eq!(witness(&report), thing(r#"{"flow":2.50e1,"fault":"none"}"#));
}

#[test]
fn at_most_eight_candidates_are_tried_for_one_change() {
    // Values that the earlier mode's own length limit excludes come before `ok`, the one
    // removed value that the earlier revision admits.
    let limited = |values: Vec<String>| {
        valve(&[
            ("mode", "maxLength", json!(2)),
            ("mode", "enum", json!(values)),
        ])
    };
    let later = catalogue(&[("valve", &limited(vec!["on".to_owned()]))]);
    for (excluded, witnessed) in [(7, true), (8, false)] {
        let mut values: Vec<_> = (0..excluded).map(|value| format!("long{value}")).collect();
        values.extend(["ok".to_owned(), "on".to_owned()]);
        let earlier = catalogue(&[("valve", &limited(values))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let mode = at(&comparison, "/mode/enum");
        if witnessed {
            assert_eq!(witness(&mode), client("\"ok\""));
        } else {
            assert_eq!(mode.classification(), Classification::Undetermined);
            assert_eq!(mode.reason(), Some(UndeterminedReason::Unproven));
        }
    }
}

type Settled = (Classification, Option<UndeterminedReason>);

#[test]
fn an_allowance_exhausted_while_generating_candidates_leaves_that_difference_undetermined() {
    // The removed Action is classified before the settings are compared.
    let earlier = revised(
        &valve(&[("settings", "required", json!(["flow"]))]),
        |document| {
            document["sdfObject"]["Valve"]["sdfAction"] =
                json!({"flush": {"description": "Flush the valve"}});
        },
    );
    let later = valve(&[("settings", "required", json!(["flow", "label"]))]);
    let within = |work: usize| -> Option<(Settled, Settled, Classification)> {
        let limits = Limits::default().decoding_work(work);
        let revision = |text: &str| {
            Catalogue::with_limits(
                &[SourceInput::supplied("valve", text.as_bytes())],
                limits.clone(),
            )
            .ok()
        };
        let (earlier, later) = (revision(&earlier)?, revision(&later)?);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let settled = |suffix: &str| {
            comparison
                .differences()
                .find(|difference| {
                    [Revision::Earlier, Revision::Later].iter().any(|revision| {
                        difference.side(*revision).is_some_and(|side| {
                            side.pointer(8192)
                                .present()
                                .is_some_and(|pointer| pointer.ends_with(suffix))
                        })
                    })
                })
                .map(|difference| (difference.classification(), difference.reason()))
        };
        Some((
            settled("/sdfAction/flush")?,
            settled("/settings/required")?,
            comparison.outcome(),
        ))
    };
    let generating = "representative value generation exhausted the operation allowance";
    let checking = "witness checking exhausted the operation allowance";
    // Generating candidates has started once the requirement is witnessed, or once its
    // generation or its witness checks run out of the allowance.
    let started = |work| {
        within(work).is_some_and(|(_, (classification, reason), _)| {
            classification == Classification::Breaking
                || reason.is_some_and(|reason| [generating, checking].contains(&reason.message()))
        })
    };
    let (mut low, mut high) = (1, 10_000_000);
    assert!(started(high));
    while low < high {
        let middle = low + (high - low) / 2;
        if started(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let (flush, required, outcome) = within(low).unwrap();
    assert_eq!(flush, (Classification::Breaking, None));
    assert_eq!(required.0, Classification::Undetermined);
    let reason = required.1.unwrap();
    assert_eq!(reason.code(), "operation.limit");
    assert_eq!(reason.message(), generating);
    assert_eq!(outcome, Classification::Breaking);
}

#[test]
fn both_reports_show_representative_witnesses_alike() {
    let (earlier, later) = valves(
        &[
            ("state", "const", json!("idle")),
            ("settings", "required", json!(["flow"])),
        ],
        &[
            ("mode", "type", json!("boolean")),
            ("settings", "required", json!(["flow", "label"])),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(
        wire["request"]["comparison"],
        "esdieff.revision-comparison/2"
    );
    let human = comparison.to_human();
    assert!(human.starts_with("Revision comparison esdieff.revision-comparison/2: breaking\n"));
    let valve = "/sdfObject/Valve/sdfProperty";
    for (quality, raw, direction, earlier, later, absent) in [
        ("mode/type", "\"\"", "client", "admitted", "rejected", ""),
        (
            "state/const",
            "\"\"",
            "thing",
            "rejected",
            "admitted",
            " (absent)",
        ),
        (
            "settings/required",
            r#"{"flow":5}"#,
            "client",
            "admitted",
            "rejected",
            "",
        ),
    ] {
        let pointer = format!("{valve}/{quality}");
        let difference = wire["result"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|difference| difference["earlier"]["location"]["pointer"] == json!(pointer))
            .unwrap_or_else(|| panic!("{pointer}"));
        assert_eq!(
            difference["witness"],
            json!({"direction": format!("{direction}_supplied"), "raw_json": raw,
                "admission": {"earlier": earlier, "later": later}})
        );
        let name = quality.split('/').next_back().unwrap();
        let block = format!(
            "  changed quality {name} ({direction}-supplied data)\n    witness {raw} for {direction}-supplied data: earlier {earlier}, later {later}\n    earlier valve {pointer}\n    later valve {pointer}{absent}\n"
        );
        assert!(human.contains(&block), "{block}\n{human}");
    }
}
