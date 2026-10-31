//! Existing implementation obligations, following ADR 0053.
mod comparison_support;
use comparison_support::{at, catalogue, pointer, witness};
use esdieff::{
    Admission, Classification, ComparisonRequest, DifferenceKind, Direction, Perspective, Revision,
};
use serde_json::{Value, json};

fn model(data: Value, direction: Direction) -> String {
    let mut property = data;
    property["writable"] = json!(direction == Direction::ClientSupplied);
    property["readable"] = json!(direction == Direction::ThingSupplied);
    property["observable"] = json!(false);
    json!({"info":{"version":"1"},"sdfObject":{"Device":{"sdfProperty":{"value":property}}}})
        .to_string()
}

#[test]
fn existing_implementations_must_accept_new_inputs_and_restrict_old_outputs() {
    for (direction, client, implementer, admissions) in [
        (
            Direction::ClientSupplied,
            Classification::Compatible,
            Classification::Breaking,
            (Admission::Rejected, Admission::Admitted),
        ),
        (
            Direction::ThingSupplied,
            Classification::Breaking,
            Classification::Compatible,
            (Admission::Rejected, Admission::Admitted),
        ),
    ] {
        let old = model(json!({"type":"integer","maximum":10}), direction);
        let new = model(json!({"type":"integer","maximum":20}), direction);
        for (old, new, expected) in [(&old, &new, implementer), (&new, &old, client)] {
            let earlier = catalogue(&[("old.sdf.json", old)]);
            let later = catalogue(&[("old.sdf.json", new)]);
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            assert_eq!(comparison.outcome(), expected);
            assert_eq!(comparison.perspective(), Perspective::Implementer);
            assert!(std::ptr::eq(
                comparison.catalogue(Revision::Earlier),
                &earlier
            ));
            let difference = at(&comparison, "/maximum");
            assert_eq!(difference.kind(), DifferenceKind::Changed);
            assert_eq!(
                pointer(&difference, Revision::Earlier),
                "/sdfObject/Device/sdfProperty/value/maximum"
            );
            if expected == Classification::Breaking {
                let (_, actual_direction, a, b) = witness(&difference);
                assert_eq!(actual_direction, direction);
                let expected = if direction == Direction::ClientSupplied {
                    admissions
                } else {
                    (admissions.1, admissions.0)
                };
                assert_eq!((a, b), expected);
                assert_eq!(comparison.version_warnings().len(), 1);
                assert_eq!(
                    comparison
                        .version_warnings()
                        .next()
                        .unwrap()
                        .earlier_version(),
                    "1"
                );
            }
        }
        let earlier = catalogue(&[("model", &old)]);
        let later = catalogue(&[("model", &new)]);
        assert_eq!(
            ComparisonRequest::new(&earlier, &later)
                .compare()
                .unwrap()
                .outcome(),
            client
        );
    }
}

#[test]
fn unchanged_constraints_can_rule_out_a_false_breaking_witness() {
    let earlier = catalogue(&[(
        "model",
        &model(
            json!({"type":"integer","maximum":10,"const":5}),
            Direction::ClientSupplied,
        ),
    )]);
    let later = catalogue(&[(
        "model",
        &model(
            json!({"type":"integer","maximum":20,"const":5}),
            Direction::ClientSupplied,
        ),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_ne!(comparison.outcome(), Classification::Breaking);
    assert!(at(&comparison, "/maximum").witness().is_none());
}

#[test]
fn nested_candidates_are_validated_as_whole_input_values() {
    for (before, after, suffix) in [
        (
            json!({"type":"object","properties":{"count":{"type":"integer","maximum":10}},"required":["count"]}),
            json!({"type":"object","properties":{"count":{"type":"integer","maximum":20}},"required":["count"]}),
            "/properties/count/maximum",
        ),
        (
            json!({"type":"array","minItems":1,"items":{"type":"integer","maximum":10}}),
            json!({"type":"array","minItems":1,"items":{"type":"integer","maximum":20}}),
            "/items/maximum",
        ),
        (
            json!({"type":"string","enum":["a","b"]}),
            json!({"type":"string","enum":["a","b","c"]}),
            "/enum",
        ),
    ] {
        let before = model(before, Direction::ClientSupplied);
        let after = model(after, Direction::ClientSupplied);
        let earlier = catalogue(&[("model", &before)]);
        let later = catalogue(&[("model", &after)]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.outcome(), Classification::Breaking, "{suffix}");
        let difference = at(&comparison, suffix);
        let (_, direction, earlier, later) = witness(&difference);
        assert_eq!(direction, Direction::ClientSupplied);
        assert_eq!((earlier, later), (Admission::Rejected, Admission::Admitted));
    }
}

#[test]
fn acknowledgements_and_unmatched_documents_use_chronological_coordinates() {
    let before = model(
        json!({"type":"integer","maximum":10}),
        Direction::ClientSupplied,
    );
    let after = model(
        json!({"type":"integer","maximum":20}),
        Direction::ClientSupplied,
    );
    let earlier = catalogue(&[("model", &before)]);
    let later = catalogue(&[("model", &after)]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    let entry = at(&comparison, "/maximum").acknowledgement().unwrap();
    let acknowledgements = esdieff::DifferenceAcknowledgements::new(vec![entry]).unwrap();
    let acknowledged = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(acknowledged.disposition(), esdieff::Disposition::Passed);
    assert_eq!(acknowledged.outcome(), Classification::Breaking);
    assert_eq!(acknowledged.acknowledged_count(Classification::Breaking), 1);

    let later = catalogue(&[("added", &after)]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(
        wire["result"]["summary"]["documents"],
        json!({"compared":0,"removed":1,"added":1})
    );
    let kinds: Vec<_> = comparison
        .differences()
        .map(|difference| difference.kind())
        .collect();
    assert!(kinds.contains(&DifferenceKind::Removed));
    assert!(kinds.contains(&DifferenceKind::Added));
    assert_eq!(comparison.outcome(), Classification::Undetermined);
}

#[test]
fn structural_changes_and_unavailable_values_do_not_gain_implementer_proofs() {
    let before = json!({"sdfObject":{"Device":{"sdfProperty":{"value":{"type":"integer","writable":false}}}}});
    for after in [
        json!({"sdfObject":{"Device":{"sdfProperty":{"value":{"type":"integer","writable":true}}}}}),
        json!({"sdfObject":{"Device":{"sdfProperty":{"value":{"type":"integer","writable":false}},"sdfRequired":["#/sdfObject/Device/sdfProperty/value"]}}}),
        json!({"sdfObject":{"Device":{"sdfProperty":{"value":{"type":"integer","writable":false,"sdfRef":"#/missing"}}}}}),
    ] {
        let earlier = catalogue(&[("model", &before.to_string())]);
        let later = catalogue(&[("model", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(comparison.outcome(), Classification::Undetermined);
        assert!(!comparison.complete());
    }
}

#[test]
fn even_empty_reports_state_the_default_perspective() {
    let model = model(json!({"type":"boolean"}), Direction::ThingSupplied);
    let catalogue = catalogue(&[("model", &model)]);
    let comparison = ComparisonRequest::new(&catalogue, &catalogue)
        .compare()
        .unwrap();
    let log: Value = serde_json::from_str(&comparison.to_sarif(std::path::Path::new("/"))).unwrap();
    assert_eq!(log["runs"][0]["properties"]["perspective"], "client");
    assert!(comparison.to_human().contains("Perspective: client."));
    assert!(comparison.to_markdown().contains("Perspective: client."));
}

#[test]
fn structural_obligations_stay_undetermined_and_chronology_is_preserved() {
    let before = json!({"sdfObject":{"Device":{"sdfProperty":{"old":{"type":"boolean"}}}}});
    let after = json!({"sdfObject":{"Device":{"sdfProperty":{"new":{"type":"boolean"}}}}});
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(comparison.outcome(), Classification::Undetermined);
    let removed = at(&comparison, "/old");
    assert_eq!(removed.kind(), DifferenceKind::Removed);
    assert!(removed.side(Revision::Later).is_none());
    let added = at(&comparison, "/new");
    assert_eq!(added.kind(), DifferenceKind::Added);
    assert!(added.side(Revision::Earlier).is_none());
}

#[test]
fn reasons_reports_and_context_keep_their_public_meaning() {
    let before = json!({"sdfObject":{"Device":{"sdfContext":{"label":{"const":"a"}},"sdfProperty":{"value":{"type":"number"}}}}});
    let mut after = before.clone();
    after["sdfObject"]["Device"]["sdfProperty"]["value"]["unit"] = json!("W");
    after["sdfObject"]["Device"]["sdfContext"]["label"]["const"] = json!("b");
    let earlier = catalogue(&[("model", &before.to_string())]);
    let later = catalogue(&[("model", &after.to_string())]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(
        at(&comparison, "/unit").reason().unwrap().code(),
        "comparison.unit_added"
    );
    assert_eq!(
        at(&comparison, "/sdfContext/label").classification(),
        Classification::Compatible
    );
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["policy"]["perspective"], "implementer");
    assert_eq!(wire["policy"]["compatibility_scope"], "client_interactions");
    assert!(comparison.to_human().contains("Perspective: implementer."));
    assert!(
        comparison
            .to_markdown()
            .contains("Perspective: implementer.")
    );
    let sarif: Value =
        serde_json::from_str(&comparison.to_sarif(std::path::Path::new("/"))).unwrap();
    assert_eq!(sarif["runs"][0]["properties"]["perspective"], "implementer");
}
