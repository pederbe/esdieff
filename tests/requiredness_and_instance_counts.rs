//! Requiredness of declarations and grouping instance counts compare by direction, without
//! a Breaking witness. Follows ADR 0039: Client-side revision comparison and ADR 0026:
//! Required declaration relationships and source evidence.
use esdieff::{
    Classification, ComparisonRequest, DifferenceKind, DifferenceSubject, Presence, Revision,
    UndeterminedReason,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{at, catalogue, revised};

/// An oven whose `temp` Property is mandatory.
const OVEN: &str = r#"{"sdfObject": {"Oven": {
    "sdfRequired": ["temp"],
    "sdfProperty": {"temp": {"type": "number"}, "mode": {"type": "string"}},
    "sdfAction": {"start": {}}
}}}"#;

/// The oven with its definition revised.
fn oven(revise: impl FnOnce(&mut Value)) -> String {
    revised(OVEN, |document| revise(&mut document["sdfObject"]["Oven"]))
}

#[test]
fn a_declaration_that_stops_being_mandatory_breaks_and_one_that_becomes_mandatory_does_not() {
    let earlier = catalogue(&[("oven", OVEN)]);
    for (required, classification) in [
        (json!(["mode"]), Classification::Breaking),
        (json!(["temp", "mode"]), Classification::Compatible),
    ] {
        let later = catalogue(&[("oven", &oven(|oven| oven["sdfRequired"] = required))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_eq!(comparison.differences().len(), 1);
        let difference = at(&comparison, "/sdfObject/Oven/sdfRequired");
        assert_eq!(difference.kind(), DifferenceKind::Changed);
        assert_eq!(
            difference.subject(),
            DifferenceSubject::Quality("sdfRequired")
        );
        assert_eq!(difference.classification(), classification);
        assert!(difference.witness().is_none());
        assert_eq!(difference.directions().count(), 0);
        assert_eq!(comparison.outcome(), classification);
    }
}

/// A kitchen holding an oven, with the requirements of each.
fn kitchen(kitchen: Option<Value>, oven: Option<Value>) -> String {
    let mut document = json!({"sdfThing": {"Kitchen": {"sdfObject": {"Oven": {
        "sdfProperty": {"temp": {"type": "number"}, "mode": {"type": "string"}}
    }}}}});
    let definition = &mut document["sdfThing"]["Kitchen"];
    if let Some(required) = kitchen {
        definition["sdfRequired"] = required;
    }
    if let Some(required) = oven {
        definition["sdfObject"]["Oven"]["sdfRequired"] = required;
    }
    document.to_string()
}
const TEMP: &str = "#/sdfThing/Kitchen/sdfObject/Oven/sdfProperty/temp";

#[test]
fn a_requirement_moved_into_a_required_nested_grouping_is_no_change() {
    let earlier = catalogue(&[("kitchen", &kitchen(Some(json!([TEMP])), None))]);
    let later = catalogue(&[(
        "kitchen",
        &kitchen(Some(json!(["Oven"])), Some(json!(["temp"]))),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 0);
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn an_optional_grouping_that_keeps_a_requirement_its_parent_drops_breaks_at_the_parent() {
    let earlier = catalogue(&[(
        "kitchen",
        &kitchen(Some(json!([TEMP])), Some(json!(["temp"]))),
    )]);
    let later = catalogue(&[("kitchen", &kitchen(None, Some(json!(["temp"]))))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 1);
    let difference = at(&comparison, "/sdfThing/Kitchen/sdfRequired");
    assert_eq!(difference.classification(), Classification::Breaking);
    assert_eq!(
        difference.side(Revision::Later).unwrap().presence(),
        Presence::Absent
    );
}

#[test]
fn a_mandatory_declaration_only_one_revision_has_counts_only_as_its_removal_or_addition() {
    let added = oven(|oven| {
        oven["sdfProperty"]["door"] = json!({"type": "boolean", "sdfRequired": [true]});
    });
    let removed = oven(|oven| {
        oven.as_object_mut().unwrap().remove("sdfRequired");
        oven["sdfProperty"].as_object_mut().unwrap().remove("temp");
    });
    for (later, kind, classification) in [
        (added, DifferenceKind::Added, Classification::Compatible),
        (removed, DifferenceKind::Removed, Classification::Breaking),
    ] {
        let earlier = catalogue(&[("oven", OVEN)]);
        let later = catalogue(&[("oven", &later)]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_eq!(comparison.differences().len(), 1);
        let difference = comparison.differences().next().unwrap();
        assert_eq!(difference.kind(), kind);
        assert_eq!(
            difference.subject(),
            DifferenceSubject::Definition(esdieff::Class::Property)
        );
        assert_eq!(difference.classification(), classification);
    }
}

#[test]
fn a_property_that_becomes_mandatory_keeps_the_compatibility_proof_of_its_data() {
    let reading = |revise: fn(&mut Value)| {
        oven(|oven| {
            oven.as_object_mut().unwrap().remove("sdfRequired");
            let temp = &mut oven["sdfProperty"]["temp"];
            temp["writable"] = json!(false);
            temp["maximum"] = json!(300);
            revise(temp);
        })
    };
    let earlier = catalogue(&[("oven", &reading(|_| {}))]);
    let later = catalogue(&[(
        "oven",
        &reading(|temp| {
            temp["maximum"] = json!(250);
            temp["sdfRequired"] = json!([true]);
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 2);
    let maximum = at(&comparison, "/temp/maximum");
    assert_eq!(maximum.classification(), Classification::Compatible);
    let required = at(&comparison, "/sdfObject/Oven/sdfRequired");
    assert_eq!(required.classification(), Classification::Compatible);
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn a_change_with_a_partial_or_unavailable_resolution_is_undetermined() {
    let earlier = catalogue(&[("oven", OVEN)]);
    for required in [
        json!(["temp", "missing"]),
        json!(["missing"]),
        json!(["temp", "elsewhere:#/sdfObject/Grill/sdfProperty/temp"]),
    ] {
        let later = catalogue(&[("oven", &oven(|oven| oven["sdfRequired"] = required))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let difference = at(&comparison, "/sdfObject/Oven/sdfRequired");
        assert_eq!(difference.classification(), Classification::Undetermined);
        assert_eq!(difference.reason().unwrap().code(), "required.unavailable");
        assert_eq!(comparison.outcome(), Classification::Undetermined);
    }
}

#[test]
fn an_unresolved_requirement_both_revisions_share_is_no_change() {
    let unresolved = oven(|oven| oven["sdfRequired"] = json!(["temp", "missing"]));
    let earlier = catalogue(&[("oven", &unresolved)]);
    let later = catalogue(&[(
        "oven",
        &revised(&unresolved, |document| {
            document["sdfObject"]["Oven"]["sdfProperty"]["mode"]["description"] = json!("Mode");
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 1);
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

/// A kitchen with an array of ovens whose instance count qualities are `counts`.
fn ovens(counts: Value) -> String {
    let mut document = json!({"sdfThing": {"Kitchen": {"sdfObject": {"Oven": {
        "sdfProperty": {"temp": {"type": "number"}}
    }}}}});
    let oven = document["sdfThing"]["Kitchen"]["sdfObject"]["Oven"]
        .as_object_mut()
        .unwrap();
    for (quality, count) in counts.as_object().unwrap() {
        oven.insert(quality.clone(), count.clone());
    }
    document.to_string()
}

#[test]
fn a_narrowed_instance_count_range_is_compatible_and_a_widened_one_breaks() {
    use Classification::{Breaking, Compatible};
    for (earlier, later, changes) in [
        (
            json!({"minItems": 1}),
            json!({"minItems": 2}),
            vec![("minItems", Compatible)],
        ),
        (
            json!({"maxItems": 4}),
            json!({"maxItems": 3}),
            vec![("maxItems", Compatible)],
        ),
        (
            json!({"maxItems": 4}),
            json!({"minItems": 0, "maxItems": 4}),
            vec![("minItems", Compatible)],
        ),
        (
            json!({"minItems": 1}),
            json!({"minItems": 1, "maxItems": 9}),
            vec![("maxItems", Compatible)],
        ),
        (
            json!({"minItems": 1}),
            json!({"minItems": 0}),
            vec![("minItems", Breaking)],
        ),
        (
            json!({"maxItems": 4}),
            json!({"maxItems": 5}),
            vec![("maxItems", Breaking)],
        ),
        (
            json!({"minItems": 1, "maxItems": 4}),
            json!({"minItems": 1}),
            vec![("maxItems", Breaking)],
        ),
        (
            json!({"minItems": 1, "maxItems": 4}),
            json!({"maxItems": 4}),
            vec![("minItems", Breaking)],
        ),
        (
            json!({"minItems": 1, "maxItems": 4}),
            json!({"minItems": 2, "maxItems": 5}),
            vec![("minItems", Compatible), ("maxItems", Breaking)],
        ),
    ] {
        let earlier = catalogue(&[("kitchen", &ovens(earlier))]);
        let later = catalogue(&[("kitchen", &ovens(later))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_eq!(comparison.differences().len(), changes.len());
        for (quality, classification) in changes {
            let difference = at(&comparison, &format!("/sdfObject/Oven/{quality}"));
            assert_eq!(difference.subject(), DifferenceSubject::Quality(quality));
            assert_eq!(difference.classification(), classification, "{quality}");
            assert!(difference.witness().is_none());
            assert_eq!(difference.directions().count(), 0);
        }
    }
}

#[test]
fn a_grouping_that_becomes_or_stops_being_an_array_stays_undetermined() {
    for (earlier, later) in [
        (json!({}), json!({"minItems": 1})),
        (json!({"maxItems": 4}), json!({})),
        (json!({"minItems": 1}), json!({"minItems": "1"})),
        (json!({"minItems": 1}), json!({"minItems": -1})),
        (json!({"minItems": 1}), json!({"minItems": 1.5})),
        (
            json!({"minItems": 1, "maxItems": 4}),
            json!({"minItems": 5, "maxItems": 4}),
        ),
    ] {
        let earlier = catalogue(&[("kitchen", &ovens(earlier))]);
        let later = catalogue(&[("kitchen", &ovens(later))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_eq!(comparison.differences().len(), 1);
        let difference = comparison.differences().next().unwrap();
        assert_eq!(difference.classification(), Classification::Undetermined);
        assert_eq!(difference.reason(), Some(UndeterminedReason::NotClassified));
    }
}

#[test]
fn both_reports_show_requiredness_and_instance_count_changes_alike() {
    let revision = |required: Value, count: Value| {
        catalogue(&[(
            "kitchen",
            &revised(
                &kitchen(Some(json!(["Oven"])), Some(required)),
                |document| document["sdfThing"]["Kitchen"]["sdfObject"]["Oven"]["maxItems"] = count,
            ),
        )])
    };
    let earlier = revision(json!(["temp"]), json!(4));
    let later = revision(json!(["mode"]), json!(2));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let human = comparison.to_human();
    let oven = "/sdfThing/Kitchen/sdfObject/Oven";
    for (pointer, quality, classification) in [
        ("/sdfThing/Kitchen/sdfRequired", "sdfRequired", "breaking"),
        (&format!("{oven}/sdfRequired"), "sdfRequired", "breaking"),
        (&format!("{oven}/maxItems"), "maxItems", "compatible"),
    ] {
        let difference = wire["result"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|difference| difference["later"]["location"]["pointer"] == json!(pointer))
            .unwrap_or_else(|| panic!("{pointer}"));
        assert_eq!(difference["kind"], "changed");
        assert_eq!(difference["subject"], "quality");
        assert_eq!(difference["quality"], quality);
        assert_eq!(difference["classification"], classification);
        assert_eq!(difference["directions"], json!([]));
        assert!(difference.get("witness").is_none_or(Value::is_null));
        let block = format!(
            "  changed quality {quality}\n    earlier kitchen {pointer}\n    later kitchen {pointer}\n"
        );
        assert!(human.contains(&block), "{human}");
    }
    assert_eq!(wire["result"]["differences"].as_array().unwrap().len(), 3);
}

#[test]
fn a_name_a_pointer_and_a_carrier_spelling_one_requirement_are_no_change() {
    let pointer = oven(|oven| oven["sdfRequired"] = json!(["#/sdfObject/Oven/sdfProperty/temp"]));
    let carrier = oven(|oven| {
        oven.as_object_mut().unwrap().remove("sdfRequired");
        oven["sdfProperty"]["temp"]["sdfRequired"] = json!([true]);
    });
    let spellings = [OVEN, &pointer, &carrier];
    for earlier in spellings {
        for later in spellings {
            let earlier = catalogue(&[("oven", earlier)]);
            let later = catalogue(&[("oven", later)]);
            let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
            assert_eq!(comparison.differences().len(), 0);
            assert_eq!(comparison.outcome(), Classification::Compatible);
        }
    }
}
