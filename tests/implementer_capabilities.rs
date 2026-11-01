//! Property interaction obligations, following ADR 0059.
mod comparison_support;
use comparison_support::{at, catalogue, pointer};
use esdieff::{
    Classification, ComparisonRequest, DifferenceKind, Direction, Perspective, Presence, Revision,
};
use serde_json::{Value, json};

fn model(property: Value) -> String {
    json!({"info":{"version":"1"},"sdfObject":{"Switch":{"sdfProperty":{"on":property}}}})
        .to_string()
}

#[test]
fn capability_transitions_use_defaults_and_preserve_each_interaction() {
    for (quality, direction) in [
        ("readable", Direction::ThingSupplied),
        ("writable", Direction::ClientSupplied),
        ("observable", Direction::ThingSupplied),
    ] {
        for (before, after, changed, enabled) in [
            (Some(false), Some(true), true, true),
            (Some(true), Some(false), true, false),
            (None, Some(true), false, false),
            (Some(true), None, false, false),
            (Some(false), None, true, true),
            (None, Some(false), true, false),
            (Some(false), Some(false), false, false),
            (Some(true), Some(true), false, false),
        ] {
            let property = |flag: Option<bool>| {
                let mut value =
                    json!({"type":"boolean","readable":true,"writable":false,"observable":false});
                value.as_object_mut().unwrap().remove(quality);
                if let Some(flag) = flag {
                    value[quality] = json!(flag);
                }
                model(value)
            };
            let earlier = catalogue(&[("model", &property(before))]);
            let later = catalogue(&[("model", &property(after))]);
            for perspective in [Perspective::Implementer, Perspective::Client] {
                let comparison = ComparisonRequest::new(&earlier, &later)
                    .perspective(perspective)
                    .compare()
                    .unwrap();
                assert_eq!(
                    comparison.differences().len(),
                    usize::from(changed),
                    "{quality} {before:?} -> {after:?}"
                );
                if !changed {
                    continue;
                }
                let expected = if enabled == (perspective == Perspective::Implementer) {
                    Classification::Breaking
                } else {
                    Classification::Compatible
                };
                assert_eq!(
                    comparison.outcome(),
                    expected,
                    "{quality} {before:?} -> {after:?}"
                );
                let difference = at(&comparison, &format!("/{quality}"));
                assert_eq!(difference.kind(), DifferenceKind::Changed);
                assert_eq!(difference.directions().collect::<Vec<_>>(), vec![direction]);
                assert!(difference.witness().is_none());
                assert_eq!(
                    comparison.version_warnings().len(),
                    usize::from(expected == Classification::Breaking)
                );
                for (revision, flag) in [(Revision::Earlier, before), (Revision::Later, after)] {
                    assert_eq!(
                        pointer(&difference, revision),
                        format!("/sdfObject/Switch/sdfProperty/on/{quality}")
                    );
                    if flag.is_none() {
                        let side = difference.side(revision).unwrap();
                        assert_eq!(side.presence(), Presence::Absent);
                        assert!(side.origin().is_none());
                    }
                }
            }
        }
    }
}

#[test]
fn capability_rows_are_independent_of_common_data_directions_and_requiredness() {
    for (before, after, expected_data, expected_rows) in [
        // Reads and notifications are distinct, even though the data direction remains.
        (
            json!({"readable":true,"observable":false,"writable":false}),
            json!({"readable":false,"observable":true,"writable":false}),
            None,
            2,
        ),
        (
            json!({"readable":false,"observable":true,"writable":false}),
            json!({"readable":true,"observable":true,"writable":false}),
            None,
            1,
        ),
        (
            json!({"readable":true,"observable":false,"writable":false}),
            json!({"readable":true,"observable":true,"writable":false}),
            None,
            1,
        ),
        // Losing writes does not hide a stricter output obligation in a common direction.
        (
            json!({"readable":true,"observable":false,"writable":true,"maximum":20}),
            json!({"readable":true,"observable":false,"writable":false,"maximum":10}),
            Some(Classification::Breaking),
            2,
        ),
        // With no common data direction, the changed payload remains compatible.
        (
            json!({"readable":true,"observable":false,"writable":false,"maximum":20}),
            json!({"readable":false,"observable":false,"writable":true,"maximum":10}),
            Some(Classification::Compatible),
            3,
        ),
    ] {
        let earlier = catalogue(&[("model", &model(before))]);
        let later = catalogue(&[("model", &model(after))]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.differences().len(), expected_rows);
        if let Some(expected) = expected_data {
            assert_eq!(at(&comparison, "/maximum").classification(), expected);
        }
        for difference in comparison
            .differences()
            .filter(|d| !pointer(d, Revision::Later).ends_with("/maximum"))
        {
            assert!(difference.witness().is_none());
            assert_eq!(difference.directions().count(), 1);
        }
        if expected_data.is_none() {
            assert_eq!(comparison.outcome(), Classification::Breaking);
        }
    }
    // Optional ancestor and Property, with unchanged unresolved requiredness.
    let before = json!({"sdfThing":{"Parent":{"sdfRequired":["#/missing"],"sdfObject":{"Switch":{"sdfProperty":{"on":{"writable":false}}}}}}});
    let mut after = before.clone();
    after["sdfThing"]["Parent"]["sdfObject"]["Switch"]["sdfProperty"]["on"]["writable"] =
        json!(true);
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 1);
    assert_eq!(
        at(&comparison, "/writable").classification(),
        Classification::Breaking
    );
    // Making the Property mandatory is a separate row, alongside releasing writes.
    after["sdfThing"]["Parent"]
        .as_object_mut()
        .unwrap()
        .remove("sdfRequired");
    let mut mandatory = after.clone();
    mandatory["sdfThing"]["Parent"]["sdfObject"]["Switch"]["sdfRequired"] = json!(["on"]);
    mandatory["sdfThing"]["Parent"]["sdfObject"]["Switch"]["sdfProperty"]["on"]["writable"] =
        json!(false);
    let later = catalogue(&[("model", &mandatory.to_string())]);
    let earlier = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.differences().len(), 2);
    assert_eq!(
        at(&comparison, "/Switch/sdfRequired").classification(),
        Classification::Breaking
    );
    assert_eq!(
        at(&comparison, "/writable").classification(),
        Classification::Compatible
    );
}

#[test]
fn invalid_characteristics_and_composition_keep_scoped_causes() {
    let before =
        model(json!({"type":"integer","maximum":20,"writable":false,"readable":"invalid"}));
    let after = model(json!({"type":"integer","maximum":10,"writable":true,"readable":"invalid"}));
    let earlier = catalogue(&[("model", &before)]);
    let later = catalogue(&[("model", &after)]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(
        at(&comparison, "/writable").classification(),
        Classification::Breaking
    );
    assert_eq!(
        at(&comparison, "/maximum").reason().unwrap().code(),
        "comparison.directions_unavailable"
    );
    assert_eq!(comparison.disposition().exit_code(), 1);
    for property in [
        json!({"writable":"invalid"}),
        json!({"writable":true,"sdfRef":"#/missing"}),
    ] {
        let later = catalogue(&[("model", &model(property))]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.outcome(), Classification::Undetermined);
        assert!(comparison.differences().all(|d| d.reason().is_some()));
    }
}

#[test]
fn copied_overridden_and_deleted_characteristics_keep_chronological_origins() {
    let base = |flag| {
        json!({"namespace":{"b":"urn:base"},"defaultNamespace":"b","sdfProperty":{"Base":{"type":"boolean","writable":flag}}}).to_string()
    };
    let consumer = |override_value: Value| {
        json!({"namespace":{"b":"urn:base"},"sdfObject":{"Switch":{"sdfProperty":{
            "Copy":{"sdfRef":"b:#/sdfProperty/Base"},
            "Refined":{"sdfRef":"b:#/sdfProperty/Base","writable":override_value}
        }}}})
        .to_string()
    };
    let before = catalogue(&[
        ("base", &base(false)),
        ("consumer", &consumer(false.into())),
    ]);
    let after = catalogue(&[("base", &base(true)), ("consumer", &consumer(Value::Null))]);
    for (earlier, later, reverse) in [(&before, &after, false), (&after, &before, true)] {
        let comparison = ComparisonRequest::new(earlier, later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        for name in ["Copy", "Refined"] {
            let difference = at(&comparison, &format!("/{name}/writable"));
            assert_eq!(
                difference.classification(),
                if reverse {
                    Classification::Compatible
                } else {
                    Classification::Breaking
                }
            );
            for revision in [Revision::Earlier, Revision::Later] {
                let side = difference.side(revision).unwrap();
                assert_eq!(
                    pointer(&difference, revision),
                    format!("/sdfObject/Switch/sdfProperty/{name}/writable")
                );
                let origin = side.origin().unwrap().sources().next().unwrap();
                let expected = if name == "Copy" {
                    "/sdfProperty/Base/writable".to_owned()
                } else {
                    format!("/sdfObject/Switch/sdfProperty/{name}/writable")
                };
                assert_eq!(origin.pointer().require().unwrap(), expected);
                assert_eq!(
                    comparison
                        .catalogue(revision)
                        .source(origin.source_id())
                        .unwrap()
                        .key(),
                    if name == "Copy" { "base" } else { "consumer" }
                );
                if name == "Refined" && (revision == Revision::Later) != reverse {
                    assert_eq!(side.presence(), Presence::Absent);
                }
            }
        }
    }
}
