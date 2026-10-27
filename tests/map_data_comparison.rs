//! Changes inside Map data: nested `properties` entries, newly required entries and added
//! or removed entries. Follows ADR 0039: Client-side revision comparison.
use esdieff::{
    Catalogue, Classification, ComparisonRequest, DifferenceSubject, Direction, Revision,
    UndeterminedReason,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{
    at, catalogue, client as client_witness, revised, thing as thing_witness, witness,
};

/// A heater whose `schedule` only Clients supply and whose `status` only the Thing supplies,
/// both as Map data.
const HEATER: &str = r#"{"sdfObject": {"Heater": {"sdfProperty": {
    "schedule": {"type": "object", "readable": false, "observable": false, "writable": true,
        "properties": {
            "level": {"type": "integer", "minimum": 0, "maximum": 100},
            "window": {"type": "object", "properties": {
                "start": {"type": "integer", "minimum": 0, "maximum": 23},
                "end": {"type": "integer", "minimum": 0, "maximum": 23}}}}},
    "status": {"type": "object", "writable": false,
        "properties": {
            "level": {"type": "integer", "minimum": 0, "maximum": 100},
            "fault": {"type": "string"}}}
}}}}"#;

/// The heater with its Properties revised.
fn heater(revise: impl FnOnce(&mut Value)) -> String {
    revised(HEATER, |document| {
        revise(&mut document["sdfObject"]["Heater"]["sdfProperty"])
    })
}

#[test]
fn a_narrowed_nested_bound_breaks_with_a_witness_map_holding_the_candidate_at_its_entry() {
    let earlier = catalogue(&[("heater", HEATER)]);
    let later = catalogue(&[(
        "heater",
        &heater(|properties| {
            let schedule = &mut properties["schedule"]["properties"];
            schedule["level"]["maximum"] = json!(80);
            schedule["window"]["properties"]["end"]["maximum"] = json!(20);
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let schedule = "/sdfObject/Heater/sdfProperty/schedule";
    let level = at(&comparison, "/schedule/properties/level/maximum");
    assert_eq!(level.subject(), DifferenceSubject::Quality("maximum"));
    assert_eq!(level.classification(), Classification::Breaking);
    assert_eq!(witness(&level), client_witness(r#"{"level":100}"#));
    let end = at(&comparison, "/window/properties/end/maximum");
    assert_eq!(end.classification(), Classification::Breaking);
    assert_eq!(witness(&end), client_witness(r#"{"window":{"end":23}}"#));
    for revision in [Revision::Earlier, Revision::Later] {
        assert_eq!(
            end.side(revision).unwrap().pointer(8192).require().unwrap(),
            format!("{schedule}/properties/window/properties/end/maximum")
        );
    }
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

/// A shared time window that the heater's schedule reuses.
const WINDOW: &str = r#"{"namespace": {"heating": "https://example.test/heating"},
    "defaultNamespace": "heating",
    "sdfData": {"Window": {"type": "object", "properties": {
        "start": {"type": "integer", "minimum": 0, "maximum": 23},
        "end": {"type": "integer", "minimum": 0, "maximum": 23}}}}}"#;

#[test]
fn a_nested_change_through_a_shared_definition_names_its_content_origins() {
    let device = revised(
        &heater(|properties| {
            properties["schedule"]["properties"]["window"] =
                json!({"sdfRef": "heating:#/sdfData/Window"});
        }),
        |document| {
            document["namespace"] = json!({"heating": "https://example.test/heating"});
        },
    );
    let window = revised(WINDOW, |document| {
        document["sdfData"]["Window"]["properties"]["end"]["maximum"] = json!(20);
    });
    let earlier = catalogue(&[("heater", &device), ("window", WINDOW)]);
    let later = catalogue(&[("heater", &device), ("window", &window)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 1);
    let end = at(
        &comparison,
        "/sdfObject/Heater/sdfProperty/schedule/properties/window/properties/end/maximum",
    );
    assert_eq!(end.classification(), Classification::Breaking);
    assert_eq!(witness(&end), client_witness(r#"{"window":{"end":23}}"#));
    for revision in [Revision::Earlier, Revision::Later] {
        let origin = end.side(revision).unwrap().origin().unwrap();
        let source = origin.sources().next().unwrap();
        let key = comparison
            .catalogue(revision)
            .source(source.source_id())
            .unwrap()
            .key();
        assert_eq!(
            (key, source.pointer(8192).require().unwrap().as_str()),
            ("window", "/sdfData/Window/properties/end/maximum")
        );
    }
}

#[test]
fn a_nested_candidate_is_placed_beside_representative_values_of_enclosing_required_entries() {
    // Every schedule needs a level, so the candidate at the window end is placed beside one.
    let narrowed = |level: Value| {
        move |properties: &mut Value| {
            let schedule = &mut properties["schedule"];
            schedule["required"] = json!(["level"]);
            schedule["properties"]["level"] = level;
            schedule["properties"]["window"]["properties"]["end"]["maximum"] = json!(20);
        }
    };
    let level = json!({"type": "integer", "minimum": 0, "maximum": 100});
    let earlier = catalogue(&[(
        "heater",
        &heater(|properties| {
            properties["schedule"]["required"] = json!(["level"]);
        }),
    )]);
    let later = catalogue(&[("heater", &heater(narrowed(level)))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let end = at(&comparison, "/window/properties/end/maximum");
    assert_eq!(end.classification(), Classification::Breaking);
    assert_eq!(
        witness(&end),
        client_witness(r#"{"window":{"end":23},"level":0}"#)
    );

    // A level that its own description rejects in both revisions leaves no witness.
    let stepped = heater(narrowed(
        json!({"type": "integer", "minimum": 1, "multipleOf": 7}),
    ));
    let unchanged = revised(&stepped, |document| {
        document["sdfObject"]["Heater"]["sdfProperty"]["schedule"]["properties"]["window"]["properties"]
            ["end"]["maximum"] = json!(23);
    });
    let earlier = catalogue(&[("heater", &unchanged)]);
    let later = catalogue(&[("heater", &stepped)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let end = at(&comparison, "/window/properties/end/maximum");
    assert_eq!(end.classification(), Classification::Undetermined);
    assert!(end.witness().is_none());
    assert_eq!(end.reason(), Some(UndeterminedReason::Unproven));

    // So does such a level beside the widened fault of Thing-supplied data.
    let faults = |maximum: u32| {
        move |properties: &mut Value| {
            let status = &mut properties["status"];
            status["required"] = json!(["level"]);
            status["properties"]["level"] =
                json!({"type": "integer", "minimum": 1, "multipleOf": 7});
            status["properties"]["fault"]["maxLength"] = json!(maximum);
        }
    };
    let (earlier, later) = heaters(faults(3), faults(5));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let fault = at(&comparison, "/status/properties/fault/maxLength");
    assert_eq!(fault.classification(), Classification::Undetermined);
    assert!(fault.witness().is_none());
    assert_eq!(fault.reason(), Some(UndeterminedReason::Unproven));
}

/// The status requiring its `level` and `fault`, with a level up to `maximum` and a fault
/// matching `pattern`.
fn status(maximum: u32, pattern: &str) -> impl FnOnce(&mut Value) {
    move |properties: &mut Value| {
        let status = &mut properties["status"];
        status["required"] = json!(["level", "fault"]);
        status["properties"]["level"] = json!({"type": "integer", "maximum": maximum});
        status["properties"]["fault"]["pattern"] = json!(pattern);
    }
}

#[test]
fn a_candidate_is_not_placed_beside_a_required_entry_whose_description_changed_too() {
    // On its own, the raised maximum breaks with the representative fault beside it.
    let (earlier, later) = heaters(status(10, "^x*$"), status(20, "^x*$"));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let level = at(&comparison, "/status/properties/level/maximum");
    assert_eq!(witness(&level), thing_witness(r#"{"level":20,"fault":""}"#));

    // Each candidate would stand beside a representative value of the other changed entry,
    // which the earlier revision may reject for that entry's own change. Here the earlier
    // level of 20 would be the reason, although `""` matches both patterns.
    let (earlier, later) = heaters(status(10, "^x*$"), status(20, "^y*$"));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/level/maximum", "/fault/pattern"] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Undetermined,
            "{suffix}"
        );
        assert!(difference.witness().is_none(), "{suffix}");
        assert_eq!(
            difference.reason(),
            Some(UndeterminedReason::Unproven),
            "{suffix}"
        );
    }
}

#[test]
fn a_candidate_is_placed_beside_an_entry_whose_documentation_alone_changed() {
    // The fault's description and documentary default change, and so does a comment deep
    // inside the detail, which leaves both revisions reading them alike.
    let documented = |maximum: u32, text: &'static str| {
        move |properties: &mut Value| {
            status(maximum, "^x*$")(properties);
            let status = &mut properties["status"];
            status["required"] = json!(["level", "fault", "detail"]);
            let fault = &mut status["properties"]["fault"];
            fault["description"] = json!(text);
            fault["default"] = json!("x".repeat(text.len()));
            status["properties"]["detail"] = json!({"type": "object", "properties": {
                "code": {"type": "integer", "$comment": text}}});
        }
    };
    let (earlier, later) = heaters(documented(10, "old"), documented(20, "newer"));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let level = at(&comparison, "/status/properties/level/maximum");
    assert_eq!(
        witness(&level),
        thing_witness(r#"{"level":20,"fault":"xxxxx","detail":{}}"#)
    );
}

#[test]
fn a_candidate_is_placed_beside_every_entry_either_revision_requires() {
    // The later status stops requiring a level, so the earlier revision would reject a placed
    // fault without one for that reason alone. Beside a level, `""` matches both patterns.
    let (earlier, later) = heaters(status(10, "^x*$"), |properties| {
        status(10, "^y*$")(properties);
        properties["status"]["required"] = json!(["fault"]);
    });
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let fault = at(&comparison, "/fault/pattern");
    assert_eq!(fault.classification(), Classification::Undetermined);
    assert_eq!(fault.reason(), Some(UndeterminedReason::Unproven));
    let required = at(&comparison, "/status/required");
    assert_eq!(witness(&required), thing_witness(r#"{"fault":""}"#));
}

/// Both revisions of the heater, each with its own revision of its Properties.
fn heaters(
    earlier: impl FnOnce(&mut Value),
    later: impl FnOnce(&mut Value),
) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("heater", &heater(earlier))]),
        catalogue(&[("heater", &heater(later))]),
    )
}

/// Make the schedule readable as well as writable.
fn readable(properties: &mut Value) {
    let schedule = properties["schedule"].as_object_mut().unwrap();
    schedule.remove("readable");
    schedule.remove("observable");
}

#[test]
fn a_newly_required_entry_in_client_supplied_data_breaks_with_a_witness_map_lacking_it() {
    let (earlier, later) = heaters(
        |_| {},
        |properties| properties["schedule"]["required"] = json!(["level"]),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let required = at(&comparison, "/schedule/required");
    assert_eq!(required.subject(), DifferenceSubject::Quality("required"));
    assert_eq!(required.classification(), Classification::Breaking);
    assert!(required.reason().is_none());
    assert_eq!(witness(&required), client_witness("{}"));

    // A newly required nested entry has its witness inside the enclosing map.
    let window = |required: Value| {
        move |properties: &mut Value| {
            properties["schedule"]["properties"]["window"]["required"] = required;
        }
    };
    let (earlier, later) = heaters(|_| {}, window(json!(["end"])));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let required = at(&comparison, "/window/required");
    assert_eq!(required.classification(), Classification::Breaking);
    assert_eq!(witness(&required), client_witness(r#"{"window":{}}"#));

    // A representative window of the earlier revision holds only the entry it requires.
    let (earlier, later) = heaters(window(json!(["start"])), window(json!(["end", "start"])));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let required = at(&comparison, "/window/required");
    assert_eq!(required.classification(), Classification::Breaking);
    assert_eq!(
        witness(&required),
        client_witness(r#"{"window":{"start":0}}"#)
    );

    // Readable and writable data breaks through its write direction.
    let (earlier, later) = heaters(readable, |properties| {
        readable(properties);
        properties["schedule"]["required"] = json!(["level"]);
    });
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let required = at(&comparison, "/schedule/required");
    assert!(required.affects(Direction::ThingSupplied));
    assert_eq!(witness(&required), client_witness("{}"));
}

#[test]
fn requirements_that_loosen_in_the_safe_direction_are_compatible() {
    // The Thing promises a level, and Clients may leave out the window.
    let (earlier, later) = heaters(
        |properties| properties["schedule"]["required"] = json!(["level", "window"]),
        |properties| {
            properties["schedule"]["required"] = json!(["level"]);
            properties["status"]["required"] = json!(["level"]);
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for property in ["schedule", "status"] {
        let required = at(&comparison, &format!("/{property}/required"));
        assert_eq!(
            required.classification(),
            Classification::Compatible,
            "{property}"
        );
        assert!(required.witness().is_none() && required.reason().is_none());
    }
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn entries_of_data_in_no_direction_both_revisions_offer_are_compatible() {
    // The status becomes write-only, so no Client exchanges it in both revisions.
    let (earlier, later) = heaters(
        |_| {},
        |properties| {
            let status = &mut properties["status"];
            status["properties"]["mode"] = json!({"type": "string"});
            status["readable"] = json!(false);
            status["observable"] = json!(false);
            status["writable"] = json!(true);
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mode = at(&comparison, "/status/properties/mode");
    assert_eq!(mode.classification(), Classification::Compatible);
    assert_eq!(mode.directions().count(), 0);
}

#[test]
fn both_reports_show_witness_maps_and_added_entries() {
    let (earlier, later) = heaters(
        |_| {},
        |properties| {
            let schedule = &mut properties["schedule"];
            schedule["required"] = json!(["level"]);
            schedule["properties"]["window"]["properties"]["end"]["maximum"] = json!(20);
            properties["status"]["properties"]["mode"] = json!({"type": "string"});
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let differences = wire["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), 3);
    let schedule = "/sdfObject/Heater/sdfProperty/schedule";
    let end = format!("{schedule}/properties/window/properties/end/maximum");
    let mode = "/sdfObject/Heater/sdfProperty/status/properties/mode";
    let later_at = |pointer: &str| {
        differences
            .iter()
            .find(|difference| difference["later"]["location"]["pointer"] == pointer)
            .unwrap_or_else(|| panic!("difference at {pointer}"))
    };
    let admission = json!({"earlier": "admitted", "later": "rejected"});
    // The window stands beside the level that the later revision requires, so the later
    // revision rejects it for its end alone.
    assert_eq!(
        later_at(&end)["witness"],
        json!({"direction": "client_supplied", "raw_json": r#"{"window":{"end":23},"level":0}"#,
            "admission": admission})
    );
    assert_eq!(
        later_at(&format!("{schedule}/required"))["witness"],
        json!({"direction": "client_supplied", "raw_json": "{}", "admission": admission})
    );
    let entry = later_at(mode);
    assert_eq!(
        (&entry["kind"], &entry["subject"], &entry["earlier"]),
        (&json!("added"), &json!("properties_entry"), &Value::Null)
    );
    assert_eq!(entry["classification"], "compatible");
    assert!(entry.get("reason").is_none() && entry.get("witness").is_none());

    let human = comparison.to_human();
    for expected in [
        format!(
            "  changed quality maximum (client-supplied data)\n    witness {{\"window\":{{\"end\":23}},\"level\":0}} for client-supplied data: earlier admitted, later rejected\n    earlier heater {end}\n    later heater {end}\n"
        ),
        format!(
            "  changed quality required (client-supplied data)\n    witness {{}} for client-supplied data: earlier admitted, later rejected\n    earlier heater {schedule}/required (absent)\n    later heater {schedule}/required\n"
        ),
        format!("  added properties entry (thing-supplied data)\n    later heater {mode}\n"),
    ] {
        assert!(human.contains(&expected), "{expected}\n{human}");
    }
}
