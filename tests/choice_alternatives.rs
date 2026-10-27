//! Choices compare by the effective alternatives they derive, which correspond by Given
//! Name. Follows ADR 0039: Client-side revision comparison and ADR 0024: Effective
//! alternatives and nullable characteristics.
use esdieff::{
    Catalogue, Classification, ComparisonRequest, DifferenceKind, DifferenceSubject, Revision,
    UndeterminedReason,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{at, catalogue, client, thing, witness};

/// A heater whose `target` only Clients supply and whose `reading` only the Thing supplies,
/// with these data descriptions.
fn heater(target: Value, reading: Value) -> String {
    let property = |mut description: Value, writable: bool| {
        let map = description.as_object_mut().unwrap();
        for (characteristic, value) in [
            ("readable", !writable),
            ("observable", !writable),
            ("writable", writable),
        ] {
            map.insert(characteristic.to_owned(), json!(value));
        }
        description
    };
    json!({"sdfObject": {"Heater": {"sdfProperty": {
        "target": property(target, true),
        "reading": property(reading, false)
    }}}})
    .to_string()
}

/// The earlier and later heaters, each with the same description for both Properties.
fn heaters(earlier: Value, later: Value) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("heater", &heater(earlier.clone(), earlier))]),
        catalogue(&[("heater", &heater(later.clone(), later))]),
    )
}

/// A number up to 10 or from 50.
fn ranges() -> Value {
    json!({"type": "number", "sdfChoice": {"low": {"maximum": 10}, "high": {"minimum": 50}}})
}

/// The ranges with their choice revised.
fn revised_ranges(revise: impl FnOnce(&mut serde_json::Map<String, Value>)) -> Value {
    let mut ranges = ranges();
    revise(ranges["sdfChoice"].as_object_mut().unwrap());
    ranges
}

#[test]
fn a_removed_alternative_of_client_data_or_an_added_one_of_thing_data_breaks_with_its_value() {
    let earlier = catalogue(&[("heater", &heater(ranges(), ranges()))]);
    let later = heater(
        revised_ranges(|choice| {
            choice.remove("high");
        }),
        revised_ranges(|choice| {
            choice.insert("mid".to_owned(), json!({"minimum": 20, "maximum": 30}));
        }),
    );
    let later = catalogue(&[("heater", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let removed = at(&comparison, "/target/sdfChoice/high");
    assert_eq!(removed.kind(), DifferenceKind::Removed);
    assert_eq!(removed.subject(), DifferenceSubject::Quality("sdfChoice"));
    assert_eq!(removed.classification(), Classification::Breaking);
    assert_eq!(witness(&removed), client("50"));
    let added = at(&comparison, "/reading/sdfChoice/mid");
    assert_eq!(added.kind(), DifferenceKind::Added);
    assert_eq!(added.classification(), Classification::Breaking);
    assert_eq!(witness(&added), thing("20"));
    assert!(added.side(Revision::Earlier).is_some());
    assert_eq!(comparison.differences().len(), 2);
}

#[test]
fn an_added_alternative_of_client_data_or_a_removed_one_of_thing_data_is_compatible() {
    let wider = revised_ranges(|choice| {
        choice.insert("mid".to_owned(), json!({"minimum": 20, "maximum": 30}));
    });
    let earlier = catalogue(&[("heater", &heater(ranges(), wider.clone()))]);
    let later = catalogue(&[("heater", &heater(wider, ranges()))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/target/sdfChoice/mid", "/reading/sdfChoice/mid"] {
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
fn a_branch_that_overrides_a_shared_maximum_upward_widens_its_alternative() {
    // Both alternatives inherit the shared maximum until the later `high` raises its own.
    let shared = |high: Value| {
        json!({"type": "number", "maximum": 10,
            "sdfChoice": {"low": {"minimum": 0}, "high": high}})
    };
    let (earlier, later) = heaters(shared(json!({})), shared(json!({"maximum": 20})));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 2);
    let target = at(&comparison, "/target/sdfChoice/high/maximum");
    assert_eq!(target.kind(), DifferenceKind::Changed);
    assert_eq!(target.subject(), DifferenceSubject::Quality("maximum"));
    assert_eq!(target.classification(), Classification::Compatible);
    let reading = at(&comparison, "/reading/sdfChoice/high/maximum");
    assert_eq!(reading.classification(), Classification::Breaking);
    assert_eq!(witness(&reading), thing("20"));
}

/// A number with these shared qualities and a `low` and `high` alternative.
fn shared(shared: Value, low: Value, high: Value) -> Value {
    let mut description = shared;
    description["sdfChoice"] = json!({"low": low, "high": high});
    description
}

#[test]
fn an_inherited_change_is_reported_once_at_the_description() {
    // `low` inherits the shared minimum, which `high` replaces with its own.
    let bounded = |shared_minimum: i32, high_minimum: u32| {
        shared(
            json!({"type": "number", "minimum": shared_minimum}),
            json!({"maximum": 5}),
            json!({"minimum": high_minimum}),
        )
    };
    let (earlier, later) = heaters(bounded(0, 50), bounded(-10, 60));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 4);
    let target = at(&comparison, "/target/minimum");
    assert_eq!(target.classification(), Classification::Compatible);
    let reading = at(&comparison, "/reading/minimum");
    assert_eq!(witness(&reading), thing("-10"));
    let target = at(&comparison, "/target/sdfChoice/high/minimum");
    assert_eq!(witness(&target), client("50"));
    let reading = at(&comparison, "/reading/sdfChoice/high/minimum");
    assert_eq!(reading.classification(), Classification::Compatible);
}

#[test]
fn a_shared_quality_that_every_alternative_replaces_changes_nothing() {
    let raised = |maximum: u32| {
        shared(
            json!({"type": "number", "maximum": maximum}),
            json!({"maximum": 5}),
            json!({"maximum": 50}),
        )
    };
    let (earlier, later) = heaters(raised(10), raised(20));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/target/maximum", "/reading/maximum"] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Compatible,
            "{suffix}"
        );
    }
}

/// Each meaning quality with an earlier and a later shared value, which differ in meaning,
/// and a value that replaces both.
const MEANINGS: [(&str, &str, &str, &str); 3] = [
    ("unit", "Cel", "s", "K"),
    (
        "contentFormat",
        "application/json",
        "application/cbor",
        "text/plain",
    ),
    ("sdfType", "byte-string", "unix-time", "byte-string"),
];

#[test]
fn a_shared_meaning_quality_that_every_alternative_replaces_changes_nothing() {
    for (quality, earlier, later, replacement) in MEANINGS {
        // `low` inherits the shared quality only when `inherited`.
        let meaning = |value: &str, inherited: bool| {
            let replaced = json!({ quality: replacement });
            let low = if inherited {
                json!({})
            } else {
                replaced.clone()
            };
            shared(json!({"type": "string", quality: value}), low, replaced)
        };
        // Beside an enum the choice establishes no alternatives, so no break can be shown.
        let listed = |value: &str| {
            let mut description = meaning(value, true);
            description["enum"] = json!(["on", "off"]);
            description
        };
        for (old, new, expected) in [
            (
                meaning(earlier, false),
                meaning(later, false),
                Classification::Compatible,
            ),
            (
                meaning(earlier, true),
                meaning(later, true),
                Classification::Breaking,
            ),
            (listed(earlier), listed(later), Classification::Undetermined),
        ] {
            let (old, new) = heaters(old, new);
            let comparison = ComparisonRequest::new(&old, &new).compare().unwrap();
            for property in ["target", "reading"] {
                let difference = at(&comparison, &format!("/{property}/{quality}"));
                assert_eq!(
                    difference.classification(),
                    expected,
                    "{property} {quality} {expected:?}"
                );
                assert!(difference.witness().is_none());
            }
        }
    }
}

#[test]
fn a_shared_bound_is_judged_beside_the_other_bound_of_each_alternative() {
    // The shared minimum makes the raised exclusive minimum redundant, but `high` replaces
    // that minimum, so its own lower bound rises from above 0 to above 5.
    let raised = |exclusive: u32| {
        shared(
            json!({"type": "number", "minimum": 10, "exclusiveMinimum": exclusive}),
            json!({"maximum": 20}),
            json!({"minimum": 0}),
        )
    };
    let (earlier, later) = heaters(raised(0), raised(5));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let target = at(&comparison, "/target/exclusiveMinimum");
    assert_eq!(target.classification(), Classification::Breaking);
    assert_eq!(witness(&target), client("5"));
    let reading = at(&comparison, "/reading/exclusiveMinimum");
    assert_eq!(reading.classification(), Classification::Compatible);
}

#[test]
fn a_choice_added_or_removed_as_a_whole_breaks_with_a_representative_value_of_each_side() {
    // A choice gives the first representative value of each alternative, so `10` from `low`.
    let plain = json!({"type": "number", "minimum": 20});
    for (earlier, later, witnesses) in [
        (plain.clone(), ranges(), [client("20"), thing("10")]),
        (ranges(), plain.clone(), [client("10"), thing("20")]),
    ] {
        let (earlier, later) = heaters(earlier, later);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        for (property, expected) in ["target", "reading"].into_iter().zip(witnesses) {
            let choice = at(&comparison, &format!("/{property}/sdfChoice"));
            assert_eq!(choice.kind(), DifferenceKind::Changed);
            assert_eq!(choice.classification(), Classification::Breaking);
            assert_eq!(witness(&choice), expected, "{property}");
        }
    }
}

#[test]
fn a_choice_or_enum_beside_the_other_stays_undetermined() {
    let listed = |values: Value, choice: Value| json!({"type": "string", "enum": values, "sdfChoice": choice});
    let (earlier, later) = heaters(
        listed(json!(["on", "off"]), json!({"quiet": {}, "loud": {}})),
        listed(json!(["on"]), json!({"quiet": {}})),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for property in ["target", "reading"] {
        let choice = at(&comparison, &format!("/{property}/sdfChoice"));
        assert_eq!(choice.classification(), Classification::Undetermined);
        assert_eq!(choice.reason(), Some(UndeterminedReason::NotClassified));
        let values = at(&comparison, &format!("/{property}/enum"));
        assert_eq!(values.classification(), Classification::Undetermined);
    }
}

#[test]
fn a_removed_alternative_whose_values_another_alternative_admits_stays_undetermined() {
    let (earlier, later) = heaters(
        revised_ranges(|choice| {
            choice.insert("mid".to_owned(), json!({"minimum": 5, "maximum": 8}));
        }),
        ranges(),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let target = at(&comparison, "/target/sdfChoice/mid");
    assert_eq!(target.classification(), Classification::Undetermined);
    assert_eq!(target.reason(), Some(UndeterminedReason::Unproven));
    let reading = at(&comparison, "/reading/sdfChoice/mid");
    assert_eq!(reading.classification(), Classification::Compatible);

    // The same alternative added to Thing-supplied data admits no value `low` rejects.
    let comparison = ComparisonRequest::new(&later, &earlier).compare().unwrap();
    let target = at(&comparison, "/target/sdfChoice/mid");
    assert_eq!(target.classification(), Classification::Compatible);
    let reading = at(&comparison, "/reading/sdfChoice/mid");
    assert_eq!(reading.classification(), Classification::Undetermined);
    assert_eq!(reading.reason(), Some(UndeterminedReason::Unproven));
}

#[test]
fn a_candidate_inside_an_alternative_is_placed_where_its_description_stands() {
    // A level entry with a choice of its own, and an alternative with its own entries.
    let settings = |level: Value, fan: u32| {
        json!({"type": "object", "properties": {
            "level": {"type": "integer", "sdfChoice": level},
            "mode": {"sdfChoice": {
                "manual": {"type": "object", "required": ["speed"],
                    "properties": {"speed": {"type": "integer", "minimum": 1},
                        "fan": {"type": "integer", "maximum": fan}}},
                "auto": {"type": "string"}}}}})
    };
    let levels = json!({"low": {"maximum": 3}, "high": {"minimum": 7}});
    let (earlier, later) = heaters(
        settings(levels.clone(), 5),
        settings(json!({"low": levels["low"]}), 3),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let level = at(&comparison, "/target/properties/level/sdfChoice/high");
    assert_eq!(witness(&level), client(r#"{"level":7}"#));
    let fan = at(
        &comparison,
        "/target/properties/mode/sdfChoice/manual/properties/fan/maximum",
    );
    assert_eq!(witness(&fan), client(r#"{"mode":{"fan":5,"speed":1}}"#));
}

#[test]
fn a_candidate_for_an_alternative_of_array_items_is_placed_as_an_item() {
    let levels = |choice: Value| {
        json!({"type": "array", "minItems": 2,
            "items": {"type": "integer", "sdfChoice": choice}})
    };
    let (earlier, later) = heaters(
        levels(json!({"low": {"maximum": 3}, "high": {"minimum": 7}})),
        levels(json!({"low": {"maximum": 3}})),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let high = at(&comparison, "/target/items/sdfChoice/high");
    assert_eq!(witness(&high), client("[7,7]"));
}

#[test]
fn both_reports_show_alternative_differences_alike() {
    let (earlier, later) = heaters(
        shared(
            json!({"type": "number", "maximum": 10}),
            json!({}),
            json!({"minimum": 5}),
        ),
        shared(
            json!({"type": "number", "maximum": 10}),
            json!({}),
            json!({"minimum": 5, "maximum": 20}),
        ),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let reading = "/sdfObject/Heater/sdfProperty/reading";
    let difference = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| difference["classification"] == "breaking")
        .unwrap();
    assert_eq!(difference["kind"], "changed");
    assert_eq!(difference["quality"], "maximum");
    assert_eq!(
        difference["earlier"]["location"]["pointer"],
        json!(format!("{reading}/sdfChoice/high/maximum"))
    );
    // The earlier alternative inherits the shared maximum, which is its origin.
    assert_eq!(
        difference["earlier"]["origin"]["pointer"],
        json!(format!("{reading}/maximum"))
    );
    assert_eq!(
        difference["witness"],
        json!({"direction": "thing_supplied", "raw_json": "20",
            "admission": {"earlier": "rejected", "later": "admitted"}})
    );
    let human = comparison.to_human();
    let block = format!(
        "  changed quality maximum (thing-supplied data)\n    witness 20 for thing-supplied data: earlier rejected, later admitted\n    earlier heater {reading}/sdfChoice/high/maximum\n      from heater {reading}/maximum\n    later heater {reading}/sdfChoice/high/maximum\n"
    );
    assert!(human.contains(&block), "{block}\n{human}");

    let (earlier, later) = heaters(
        ranges(),
        revised_ranges(|choice| {
            choice.remove("high");
        }),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let target = "/sdfObject/Heater/sdfProperty/target/sdfChoice/high";
    let difference = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| difference["classification"] == "breaking")
        .unwrap();
    assert_eq!(difference["kind"], "removed");
    assert_eq!(difference["subject"], "quality");
    assert_eq!(difference["quality"], "sdfChoice");
    assert_eq!(difference["earlier"]["presence"], "present");
    assert_eq!(difference["later"]["presence"], "absent");
    assert_eq!(difference["later"]["location"]["pointer"], json!(target));
    let human = comparison.to_human();
    let block = format!(
        "  removed quality sdfChoice (client-supplied data)\n    witness 50 for client-supplied data: earlier admitted, later rejected\n    earlier heater {target}\n    later heater {target} (absent)\n"
    );
    assert!(human.contains(&block), "{block}\n{human}");
}
