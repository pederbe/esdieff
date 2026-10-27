//! Changes to Array data: inside an array's `items`, to its item counts and to
//! `uniqueItems`. Follows ADR 0039: Client-side revision comparison.
use esdieff::{Catalogue, Classification, ComparisonRequest};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{WitnessRow, at, catalogue, client, revised, thing, witness};

/// A panel whose `levels`, `slots` and `settings` only Clients supply and whose `readings`
/// only the Thing supplies.
const PANEL: &str = r#"{"sdfObject": {"Panel": {"sdfProperty": {
    "levels": {"type": "array", "readable": false, "observable": false, "writable": true,
        "items": {"type": "integer", "minimum": 0, "maximum": 100}},
    "slots": {"type": "array", "readable": false, "observable": false, "writable": true,
        "items": {"type": "object", "required": ["start"], "properties": {
            "start": {"type": "integer", "minimum": 0},
            "end": {"type": "integer", "maximum": 23}}}},
    "readings": {"type": "array", "writable": false,
        "items": {"type": "integer", "minimum": 0, "maximum": 100}},
    "settings": {"type": "object", "readable": false, "observable": false, "writable": true,
        "properties": {
            "points": {"type": "array", "items": {"type": "number", "minimum": 0}}}}
}}}}"#;

/// The panel with each `(pointer, value)` revision applied below its Properties, where a
/// null value removes the member.
fn panel(revisions: &[(&str, Value)]) -> String {
    revised(PANEL, |document| {
        let properties = &mut document["sdfObject"]["Panel"]["sdfProperty"];
        for (pointer, value) in revisions {
            let (owner, name) = pointer.rsplit_once('/').unwrap();
            let owner = properties.pointer_mut(owner).unwrap();
            let owner = owner.as_object_mut().unwrap();
            if value.is_null() {
                owner.remove(name);
            } else {
                owner.insert(name.to_owned(), value.clone());
            }
        }
    })
}

/// The earlier and later panels with each revision applied to its own side.
fn panels(old: &[(&str, Value)], new: &[(&str, Value)]) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("panel", &panel(old))]),
        catalogue(&[("panel", &panel(new))]),
    )
}

#[test]
fn a_narrowed_bound_inside_items_breaks_with_a_witness_array_holding_the_candidate() {
    let (earlier, later) = panels(
        &[],
        &[
            ("/levels/items/maximum", json!(50)),
            ("/readings/items/maximum", json!(50)),
            ("/settings/properties/points/items/minimum", json!(1)),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let levels = at(&comparison, "/levels/items/maximum");
    assert_eq!(levels.classification(), Classification::Breaking);
    assert_eq!(witness(&levels), client("[100]"));
    let points = at(&comparison, "/points/items/minimum");
    assert_eq!(points.classification(), Classification::Breaking);
    assert_eq!(witness(&points), client(r#"{"points":[0]}"#));
    // Thing-supplied data may narrow.
    let readings = at(&comparison, "/readings/items/maximum");
    assert_eq!(readings.classification(), Classification::Compatible);
    assert!(readings.witness().is_none());
}

#[test]
fn a_narrowed_bound_in_an_entry_of_object_items_breaks_with_an_array_of_filled_maps() {
    let (earlier, later) = panels(&[], &[("/slots/items/properties/end/maximum", json!(20))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let end = at(&comparison, "/slots/items/properties/end/maximum");
    assert_eq!(end.classification(), Classification::Breaking);
    assert_eq!(witness(&end), client(r#"[{"end":23,"start":0}]"#));
}

#[test]
fn only_the_direct_item_vocabulary_is_classified_inside_items() {
    let lowered = ("/levels/items/maximum", json!(50));
    let exclusive = ("/levels/items/exclusiveMinimum", json!(0));
    let cases = [
        // Qualities outside the vocabulary, including a `const`, which data elsewhere
        // classifies, and a Nullable characteristic, which has no default inside items.
        (
            vec![],
            vec![("/levels/items/pattern", json!("^1"))],
            "pattern",
        ),
        (vec![], vec![("/levels/items/const", json!(7))], "const"),
        (
            vec![],
            vec![("/levels/items/nullable", json!(true))],
            "nullable",
        ),
        (
            vec![],
            vec![("/levels/items/uniqueItems", json!(false))],
            "uniqueItems",
        ),
        (
            vec![],
            vec![("/levels/items/label", json!("Level"))],
            "label",
        ),
        // A Direct item carrying such a quality has no established meaning, so even a
        // narrowed bound beside it is not classified.
        (vec![exclusive.clone()], vec![exclusive, lowered], "maximum"),
    ];
    for (old, new, quality) in cases {
        let (earlier, later) = panels(&old, &new);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let levels = at(&comparison, &format!("/levels/items/{quality}"));
        assert_eq!(
            levels.classification(),
            Classification::Undetermined,
            "{quality}"
        );
        assert_eq!(
            levels.reason().unwrap().code(),
            "comparison.not_classified",
            "{quality}"
        );
    }
    // Documentary vocabulary of a Direct item stays documentary.
    let (earlier, later) = panels(&[], &[("/levels/items/description", json!("A level"))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let levels = at(&comparison, "/levels/items/description");
    assert_eq!(levels.classification(), Classification::Compatible);
}

#[test]
fn items_added_to_client_data_or_removed_from_thing_data_break_with_an_item_of_another_type() {
    // An array without `items` admits items of every type, and `0` meets the integer items.
    let (earlier, later) = panels(
        &[("/levels/items", Value::Null)],
        &[("/readings/items", Value::Null)],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let levels = at(&comparison, "/levels/items");
    assert_eq!(levels.classification(), Classification::Breaking);
    assert_eq!(witness(&levels), client(r#"[""]"#));
    let readings = at(&comparison, "/readings/items");
    assert_eq!(readings.classification(), Classification::Breaking);
    assert_eq!(witness(&readings), thing(r#"[""]"#));

    // Items removed from Client data and added to Thing data loosen in the safe direction.
    let comparison = ComparisonRequest::new(&later, &earlier).compare().unwrap();
    for suffix in ["/levels/items", "/readings/items"] {
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
fn a_candidate_inside_items_is_repeated_to_meet_the_enclosing_minimum_count() {
    let minimum = || ("/levels/minItems", json!(2));
    let (earlier, later) = panels(
        &[minimum()],
        &[minimum(), ("/levels/items/maximum", json!(50))],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let levels = at(&comparison, "/levels/items/maximum");
    assert_eq!(levels.classification(), Classification::Breaking);
    assert_eq!(witness(&levels), client("[100,100]"));
}

#[test]
fn a_candidate_the_enclosing_array_rejects_in_both_revisions_leaves_its_change_undetermined() {
    // Repeated items are not unique, and a count beyond the value capabilities is never
    // generated. The Client-supplied levels narrow and the Thing-supplied readings widen.
    let unique = [
        ("/levels/minItems", json!(2)),
        ("/levels/uniqueItems", json!(true)),
        ("/readings/minItems", json!(2)),
        ("/readings/uniqueItems", json!(true)),
    ];
    let huge = [
        ("/levels/minItems", json!(2_000_000_000)),
        ("/readings/minItems", json!(2_000_000_000)),
    ];
    for enclosing in [&unique[..], &huge[..]] {
        let mut old = enclosing.to_vec();
        old.push(("/readings/items/maximum", json!(50)));
        let mut new = enclosing.to_vec();
        new.push(("/levels/items/maximum", json!(50)));
        let huge = |text: String| text.replace("2000000000", "18446744073709551616");
        let earlier = catalogue(&[("panel", &huge(panel(&old)))]);
        let later = catalogue(&[("panel", &huge(panel(&new)))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        for suffix in ["/levels/items/maximum", "/readings/items/maximum"] {
            let difference = at(&comparison, suffix);
            assert_eq!(
                difference.classification(),
                Classification::Undetermined,
                "{suffix}"
            );
            assert!(difference.witness().is_none());
            assert_eq!(difference.reason().unwrap().code(), "comparison.unproven");
        }
    }
}

#[test]
fn a_candidate_inside_items_meets_the_item_counts_of_both_revisions() {
    // `high` still admits the value of the removed `mid`, so a placed `[7]` would break only
    // through the raised minimum count, and `[7,7]` only through the unique items that the
    // later revision requires. Repeated as often as either revision requires, `[7,7]` is
    // admitted by both, and an array whose revisions differ in `uniqueItems` gets no
    // candidate of two or more items.
    let levels = |minimum: u32, unique: bool, choice: Value| {
        let items = json!({"type": "integer", "sdfChoice": choice});
        [
            ("/levels/minItems", json!(minimum)),
            ("/levels/uniqueItems", json!(unique)),
            ("/levels/items", items),
        ]
    };
    let low_and_high = json!({"low": {"maximum": 3}, "high": {"minimum": 7}});
    let mut with_mid = low_and_high.clone();
    with_mid["mid"] = json!({"minimum": 7, "maximum": 8});
    for (old, new, changed, expected) in [
        (
            levels(1, false, with_mid.clone()),
            levels(2, false, low_and_high.clone()),
            "/levels/minItems",
            "[7]",
        ),
        (
            levels(2, false, with_mid),
            levels(2, true, low_and_high),
            "/levels/uniqueItems",
            "[7,7]",
        ),
    ] {
        let (earlier, later) = panels(&old, &new);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let mid = at(&comparison, "/levels/items/sdfChoice/mid");
        assert_eq!(
            mid.classification(),
            Classification::Undetermined,
            "{changed}"
        );
        assert!(mid.witness().is_none(), "{changed}");
        assert_eq!(mid.reason().unwrap().code(), "comparison.unproven");
        assert_eq!(witness(&at(&comparison, changed)), client(expected));
    }

    // A widened item maximum of Thing-supplied data breaks through the items alone, beside an
    // explicit `uniqueItems` of `false` or a lowered `maxItems` that still admit the placed
    // array. A `maxItems` below its length would reject it alone.
    let raised = ("/readings/items/maximum", json!(150));
    let minimum = ("/readings/minItems", json!(2));
    for (old, new, expected) in [
        (
            vec![minimum.clone()],
            vec![minimum.clone(), ("/readings/uniqueItems", json!(false))],
            Some("[150,150]"),
        ),
        (
            vec![("/readings/maxItems", json!(5))],
            vec![("/readings/maxItems", json!(3))],
            Some("[150]"),
        ),
        (
            vec![("/readings/maxItems", json!(1))],
            vec![minimum.clone()],
            None,
        ),
    ] {
        let (earlier, later) = panels(&old, &[new.clone(), vec![raised.clone()]].concat());
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let maximum = at(&comparison, "/readings/items/maximum");
        assert_eq!(
            maximum.witness().map(|_| witness(&maximum)),
            expected.map(thing),
            "{new:?}"
        );
    }

    // A lowered item maximum beside the raised count breaks through the items alone.
    let (earlier, later) = panels(
        &[("/levels/minItems", json!(1))],
        &[
            ("/levels/minItems", json!(2)),
            ("/levels/items/maximum", json!(50)),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let maximum = at(&comparison, "/levels/items/maximum");
    assert_eq!(witness(&maximum), client("[100,100]"));
}

/// The classification and witness of the one difference at `pointer`, which `old` and `new`
/// revise in the earlier and the later panel.
fn counted(pointer: &str, old: Value, new: Value) -> (Classification, Option<WitnessRow>) {
    let (earlier, later) = panels(&[(pointer, old)], &[(pointer, new)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let difference = at(&comparison, pointer);
    let found = difference.witness().map(|_| witness(&difference));
    (difference.classification(), found)
}

#[test]
fn a_tightened_item_count_on_client_data_breaks_with_an_array_just_across_the_new_count() {
    // One item short of a raised minimum, and one over a lowered or added maximum, each
    // item a representative value of the earlier `items`.
    let cases = [
        ("/levels/minItems", json!(1), json!(3), "[0,0]"),
        ("/levels/maxItems", json!(4), json!(2), "[0,0,0]"),
        ("/levels/maxItems", Value::Null, json!(2), "[0,0,0]"),
        ("/levels/minItems", Value::Null, json!(1), "[]"),
    ];
    for (pointer, old, new, expected) in cases {
        let found = (Classification::Breaking, Some(client(expected)));
        assert_eq!(counted(pointer, old, new), found, "{pointer} {expected}");
    }
}

#[test]
fn a_loosened_item_count_on_thing_data_breaks_with_an_array_just_across_the_old_count() {
    let cases = [
        ("/readings/minItems", json!(3), json!(1), "[0,0]"),
        ("/readings/maxItems", json!(2), json!(4), "[0,0,0]"),
        ("/readings/maxItems", json!(2), Value::Null, "[0,0,0]"),
        ("/readings/minItems", json!(1), Value::Null, "[]"),
    ];
    for (pointer, old, new, expected) in cases {
        let found = (Classification::Breaking, Some(thing(expected)));
        assert_eq!(counted(pointer, old, new), found, "{pointer} {expected}");
    }
}

#[test]
fn item_counts_loosened_on_client_data_or_tightened_on_thing_data_are_compatible() {
    let cases = [
        ("/levels/minItems", json!(3), json!(1)),
        ("/levels/maxItems", json!(2), Value::Null),
        ("/readings/minItems", json!(1), json!(3)),
        ("/readings/maxItems", json!(4), json!(2)),
        // An added minimum of 0 leaves the counts as they are.
        ("/levels/minItems", Value::Null, json!(0)),
    ];
    for (pointer, old, new) in cases {
        let found = (Classification::Compatible, None);
        assert_eq!(counted(pointer, old, new), found, "{pointer}");
    }
}

#[test]
fn unique_items_that_become_required_on_client_data_break_with_a_repeated_item() {
    let cases = [
        (
            "/levels/uniqueItems",
            Value::Null,
            json!(true),
            client("[0,0]"),
        ),
        (
            "/levels/uniqueItems",
            json!(false),
            json!(true),
            client("[0,0]"),
        ),
        (
            "/readings/uniqueItems",
            json!(true),
            json!(false),
            thing("[0,0]"),
        ),
        (
            "/readings/uniqueItems",
            json!(true),
            Value::Null,
            thing("[0,0]"),
        ),
    ];
    for (pointer, old, new, expected) in cases {
        let found = (Classification::Breaking, Some(expected));
        assert_eq!(counted(pointer, old, new), found, "{pointer}");
    }
    // The repeated items meet the minimum count of the revision that must admit them.
    let minimum = || ("/levels/minItems", json!(3));
    let (earlier, later) = panels(
        &[minimum()],
        &[minimum(), ("/levels/uniqueItems", json!(true))],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let levels = at(&comparison, "/levels/uniqueItems");
    assert_eq!(witness(&levels), client("[0,0,0]"));
}

#[test]
fn unique_items_dropped_from_client_data_or_added_to_thing_data_are_compatible() {
    let cases = [
        ("/levels/uniqueItems", json!(true), json!(false)),
        ("/levels/uniqueItems", json!(true), Value::Null),
        ("/readings/uniqueItems", Value::Null, json!(true)),
    ];
    for (pointer, old, new) in cases {
        let found = (Classification::Compatible, None);
        assert_eq!(counted(pointer, old, new), found, "{pointer}");
    }
}

#[test]
fn absent_unique_items_equals_an_explicit_false() {
    for property in ["levels", "readings"] {
        let pointer = format!("/{property}/uniqueItems");
        let (earlier, later) = panels(&[], &[(&pointer, json!(false))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_eq!(comparison.differences().count(), 0, "{property}");
        assert_eq!(comparison.outcome(), Classification::Compatible);
    }
}

#[test]
fn both_reports_show_array_witnesses_alike() {
    let (earlier, later) = panels(
        &[("/readings/minItems", json!(3))],
        &[
            ("/levels/items/maximum", json!(50)),
            ("/levels/uniqueItems", json!(true)),
            ("/readings/minItems", json!(1)),
        ],
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let human = comparison.to_human();
    let panel = "/sdfObject/Panel/sdfProperty";
    for (quality, raw, direction, admissions, absent) in [
        (
            "levels/items/maximum",
            "[100]",
            "client",
            ["admitted", "rejected"],
            "",
        ),
        (
            "levels/uniqueItems",
            "[0,0]",
            "client",
            ["admitted", "rejected"],
            " (absent)",
        ),
        (
            "readings/minItems",
            "[0,0]",
            "thing",
            ["rejected", "admitted"],
            "",
        ),
    ] {
        let pointer = format!("{panel}/{quality}");
        let difference = wire["result"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|difference| difference["later"]["location"]["pointer"] == json!(pointer))
            .unwrap_or_else(|| panic!("{pointer}"));
        assert_eq!(difference["classification"], "breaking");
        assert_eq!(
            difference["witness"],
            json!({"direction": format!("{direction}_supplied"), "raw_json": raw,
                "admission": {"earlier": admissions[0], "later": admissions[1]}})
        );
        let name = quality.split('/').next_back().unwrap();
        let [earlier, later] = admissions;
        let block = format!(
            "  changed quality {name} ({direction}-supplied data)\n    witness {raw} for {direction}-supplied data: earlier {earlier}, later {later}\n    earlier panel {pointer}{absent}\n    later panel {pointer}\n"
        );
        assert!(human.contains(&block), "{block}\n{human}");
    }
}
