//! Difference acknowledgements let a maintainer record intended differences, so only
//! unacknowledged differences decide the disposition. Follows ADR 0040: Acknowledged revision
//! differences.
use esdieff::{
    Class, Classification, ComparisonRequest, DifferenceAcknowledgement,
    DifferenceAcknowledgements, DifferenceKind, DifferenceSubject, Disposition, Revision,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{catalogue, revised};

const LOCK: &str = include_str!("../examples/door-lock-check/rich.sdf.json");
const CREDENTIAL: &str = "/sdfObject/Lock/sdfProperty/credential";

/// The lock without its credential Property.
fn without_credential() -> String {
    revised(LOCK, |document| {
        document["sdfObject"]["Lock"]["sdfProperty"]
            .as_object_mut()
            .unwrap()
            .remove("credential");
    })
}

/// The entry that acknowledges the removed credential Property.
fn removed_credential() -> DifferenceAcknowledgement {
    DifferenceAcknowledgement::new(
        DifferenceKind::Removed,
        DifferenceSubject::Definition(Class::Property),
        Classification::Breaking,
    )
    .unwrap()
    .earlier("lock", CREDENTIAL)
    .unwrap()
}

#[test]
fn an_acknowledged_removed_property_passes_while_the_comparison_stays_breaking() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[("lock", &without_credential())]);
    let acknowledgements = DifferenceAcknowledgements::new(vec![removed_credential()]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    assert_eq!(comparison.disposition(), Disposition::Passed);
    assert_eq!(comparison.count(Classification::Breaking), 1);
    assert_eq!(comparison.acknowledged_count(Classification::Breaking), 1);
    assert_eq!(comparison.unused_acknowledgements().len(), 0);
    let difference = comparison.differences().next().unwrap();
    assert_eq!(difference.classification(), Classification::Breaking);
    assert!(difference.acknowledged());
    assert_eq!(difference.acknowledgement(), Some(removed_credential()));
    assert!(difference.side(Revision::Later).is_none());

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["version"], "1.6");
    assert_eq!(wire["result"]["outcome"], "breaking");
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(wire["policy"]["acknowledgements"], 1);
    assert_eq!(
        wire["result"]["summary"]["acknowledged"],
        json!({"breaking": 1, "undetermined": 0})
    );
    assert_eq!(wire["result"]["summary"]["breaking"], 1);
    assert_eq!(wire["result"]["unused_acknowledgements"], json!([]));
    let difference = &wire["result"]["differences"][0];
    assert_eq!(difference["acknowledged"], true);
    assert_eq!(
        difference["acknowledgement"],
        json!({"kind": "removed", "subject": "sdfProperty", "classification": "breaking",
            "earlier": {"document": "lock", "pointer": CREDENTIAL}, "later": null})
    );
}

/// The lock without its credential Property and its authorize Action: two breaking
/// differences.
fn without_credential_and_authorize() -> String {
    revised(&without_credential(), |document| {
        document["sdfObject"]["Lock"]["sdfAction"]
            .as_object_mut()
            .unwrap()
            .remove("authorize");
    })
}

#[test]
fn an_unacknowledged_breaking_difference_still_fails() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[("lock", &without_credential_and_authorize())]);
    let acknowledgements = DifferenceAcknowledgements::new(vec![removed_credential()]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(comparison.disposition(), Disposition::Failed);
    assert_eq!(comparison.acknowledged_count(Classification::Breaking), 1);
    // The unacknowledged breaking difference comes first.
    let order: Vec<_> = comparison
        .differences()
        .map(|difference| {
            (
                difference.subject(),
                difference.classification(),
                difference.acknowledged(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            (
                DifferenceSubject::Definition(Class::Action),
                Classification::Breaking,
                false
            ),
            (
                DifferenceSubject::Definition(Class::Property),
                Classification::Breaking,
                true
            ),
        ]
    );
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["outcome"]["disposition"], "failed");
    assert_eq!(
        wire["result"]["summary"]["acknowledged"],
        json!({"breaking": 1, "undetermined": 0})
    );
    let flags: Vec<_> = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|difference| {
            (
                difference["subject"].clone(),
                difference["acknowledged"].clone(),
            )
        })
        .collect();
    assert_eq!(
        flags,
        [
            (json!("sdfAction"), json!(false)),
            (json!("sdfProperty"), json!(true))
        ]
    );
}

#[test]
fn an_entry_whose_classification_differs_matches_nothing_and_is_unused() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[("lock", &without_credential())]);
    let undetermined = DifferenceAcknowledgement::new(
        DifferenceKind::Removed,
        DifferenceSubject::Definition(Class::Property),
        Classification::Undetermined,
    )
    .unwrap()
    .earlier("lock", CREDENTIAL)
    .unwrap();
    let acknowledgements = DifferenceAcknowledgements::new(vec![undetermined]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(comparison.disposition(), Disposition::Failed);
    assert_eq!(comparison.acknowledged_count(Classification::Breaking), 0);
    assert_eq!(
        comparison.unused_acknowledgements().collect::<Vec<_>>(),
        [0]
    );
    assert!(!comparison.differences().next().unwrap().acknowledged());
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["result"]["unused_acknowledgements"], json!([0]));
    assert_eq!(wire["outcome"]["errors"], 1);
}

#[test]
fn acknowledged_differences_follow_unacknowledged_ones_and_precede_compatible_ones() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let lock = revised(&without_credential(), |document| {
        let lock = &mut document["sdfObject"]["Lock"];
        // Every representative credential matches both patterns, so the change stays unproven.
        lock["sdfAction"]["authorize"]["sdfInputData"]["properties"]["credential"]["pattern"] =
            json!("^[a-z]+$");
        lock["sdfProperty"]["state"]["description"] = json!("Bolt position");
    });
    let later = catalogue(&[("lock", &lock)]);
    let acknowledgements = DifferenceAcknowledgements::new(vec![removed_credential()]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    let order: Vec<_> = comparison
        .differences()
        .map(|difference| (difference.classification(), difference.acknowledged()))
        .collect();
    assert_eq!(
        order,
        [
            (Classification::Undetermined, false),
            (Classification::Breaking, true),
            (Classification::Compatible, false),
        ]
    );
    assert_eq!(comparison.disposition(), Disposition::Undetermined);
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let listed: Vec<_> = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|difference| {
            (
                difference["classification"].clone(),
                difference["acknowledged"].clone(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (json!("undetermined"), json!(false)),
            (json!("breaking"), json!(true)),
            (json!("compatible"), json!(false)),
        ]
    );
    assert_eq!(wire["outcome"]["disposition"], "undetermined");
    assert_eq!(wire["outcome"]["complete"], false);
    let human = comparison.to_human();
    let position = |text: &str| {
        human
            .find(text)
            .unwrap_or_else(|| panic!("{text} in {human}"))
    };
    assert!(
        position("Undetermined differences:") < position("Acknowledged differences:")
            && position("Acknowledged differences:") < position("Compatible differences:")
    );
}

/// An entry for the removed stateChanged Event, which no revision pair here removes.
fn removed_event() -> DifferenceAcknowledgement {
    DifferenceAcknowledgement::new(
        DifferenceKind::Removed,
        DifferenceSubject::Definition(Class::Event),
        Classification::Breaking,
    )
    .unwrap()
    .earlier("lock", "/sdfObject/Lock/sdfEvent/stateChanged")
    .unwrap()
    .with_note("Release 2 drops the event")
    .unwrap()
}

#[test]
fn the_human_report_lists_acknowledged_differences_and_unused_entries() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[("lock", &without_credential_and_authorize())]);
    let acknowledgements =
        DifferenceAcknowledgements::new(vec![removed_credential(), removed_event()]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(
        comparison.unused_acknowledgements().collect::<Vec<_>>(),
        [1]
    );
    let human = comparison.to_human();
    assert!(
        human.starts_with(
            "Revision comparison esdieff.revision-comparison/4: breaking\nDisposition: failed, with 1 breaking and 0 undetermined differences acknowledged\n"
        ),
        "{human}"
    );
    assert!(human.contains("Differences: 2 breaking, 0 undetermined, 0 compatible\n"));
    assert!(
        human.ends_with(
            "Breaking differences:\n  removed sdfAction\n    earlier lock /sdfObject/Lock/sdfAction/authorize\n\
             Acknowledged differences:\n  removed sdfProperty, breaking\n    earlier lock /sdfObject/Lock/sdfProperty/credential\n\
             Unused acknowledgements:\n  /differences/1 removed sdfEvent, breaking\n    earlier lock /sdfObject/Lock/sdfEvent/stateChanged\n"
        ),
        "{human}"
    );
    let styled = comparison.to_human_with_style(esdieff::HumanStyle::Ansi);
    assert!(
        styled.contains("Disposition:\x1b[0m \x1b[31mfailed\x1b[0m"),
        "{styled}"
    );
}

#[test]
fn omitted_details_keep_unacknowledged_rows_first_with_the_counts_and_unused_entries() {
    let acknowledgements =
        DifferenceAcknowledgements::new(vec![removed_credential(), removed_event()]).unwrap();
    let compare = |limits: esdieff::Limits| {
        let revision = |text: &str| {
            esdieff::Catalogue::with_limits(
                &[esdieff::SourceInput::supplied("lock", text.as_bytes())],
                limits.clone(),
            )
            .unwrap()
        };
        let (earlier, later) = (
            revision(LOCK),
            revision(&without_credential_and_authorize()),
        );
        let comparison = ComparisonRequest::new(&earlier, &later)
            .acknowledgements(&acknowledgements)
            .compare()
            .unwrap();
        let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
        (wire, comparison.to_human())
    };
    // An allowance for difference details that holds only the first record.
    let (full, _) = compare(esdieff::Limits::default());
    let first = full["result"]["differences"][0].to_string().len() + 1;
    let (wire, human) = compare(esdieff::Limits::default().diagnostic_bytes(first));
    let result = &wire["result"];
    assert_eq!(result["differences"].as_array().unwrap().len(), 1);
    assert_eq!(result["differences"][0]["subject"], "sdfAction");
    assert_eq!(result["differences"][0]["acknowledged"], false);
    assert_eq!(result["differences_omitted"], 1);
    assert_eq!(result["summary"]["breaking"], 2);
    assert_eq!(
        result["summary"]["acknowledged"],
        json!({"breaking": 1, "undetermined": 0})
    );
    assert_eq!(result["unused_acknowledgements"], json!([1]));
    assert_eq!(wire["policy"]["acknowledgements"], 2);
    assert_eq!(wire["outcome"]["disposition"], "failed");
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["diagnostics"]["truncated"], true);
    assert!(
        human.starts_with(
            "Revision comparison esdieff.revision-comparison/4: breaking\nDisposition: failed, with 1 breaking and 0 undetermined differences acknowledged\n"
        ),
        "{human}"
    );
    assert!(
        human.ends_with(
            "Breaking differences:\n  removed sdfAction\n    earlier lock /sdfObject/Lock/sdfAction/authorize\n\
             Difference details omitted: 1\n\
             Unused acknowledgements:\n  /differences/1 removed sdfEvent, breaking\n    earlier lock /sdfObject/Lock/sdfEvent/stateChanged\n"
        ),
        "{human}"
    );
}

#[test]
fn an_unused_entry_leaves_a_compatible_comparison_passed() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[(
        "lock",
        &revised(LOCK, |document| {
            document["sdfObject"]["Lock"]["sdfProperty"]["state"]["description"] =
                json!("Bolt position");
        }),
    )]);
    let acknowledgements = DifferenceAcknowledgements::new(vec![removed_credential()]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.disposition(), Disposition::Passed);
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["result"]["unused_acknowledgements"], json!([0]));
    // A compatible difference has no entry to acknowledge it.
    assert!(
        wire["result"]["differences"][0]
            .get("acknowledgement")
            .is_none()
    );
    let human = comparison.to_human();
    assert!(human.contains(
        "Disposition: passed, with 0 breaking and 0 undetermined differences acknowledged\n"
    ));
    assert!(human.ends_with(
        "Unused acknowledgements:\n  /differences/0 removed sdfProperty, breaking\n    earlier lock /sdfObject/Lock/sdfProperty/credential\n"
    ));
}

#[test]
fn without_acknowledgements_the_disposition_follows_the_outcome() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[("lock", &without_credential())]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.disposition(), comparison.outcome().disposition());
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["policy"]["acknowledgements"], 0);
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["result"]["unused_acknowledgements"], json!([]));
    let difference = &wire["result"]["differences"][0];
    assert_eq!(difference["acknowledged"], false);
    assert_eq!(
        difference["acknowledgement"]["earlier"],
        json!({"document": "lock", "pointer": CREDENTIAL})
    );
    let human = comparison.to_human();
    assert!(!human.contains("Disposition:") && !human.contains("Acknowledged"));
}

/// A monitor whose read-only Properties state these units, or none.
fn monitor(units: &[(&str, Option<&str>)]) -> String {
    let mut properties = serde_json::Map::new();
    for (name, unit) in units {
        let mut property = json!({"type": "number", "writable": false});
        if let Some(unit) = unit {
            property["unit"] = json!(unit);
        }
        properties.insert((*name).to_owned(), property);
    }
    json!({"sdfObject": {"Monitor": {"sdfProperty": properties}}}).to_string()
}

/// The entry that acknowledges an undetermined unit change of one monitor Property.
fn unit_change(property: &str) -> DifferenceAcknowledgement {
    let pointer = format!("/sdfObject/Monitor/sdfProperty/{property}/unit");
    DifferenceAcknowledgement::new(
        DifferenceKind::Changed,
        DifferenceSubject::Quality("unit"),
        Classification::Undetermined,
    )
    .unwrap()
    .earlier("monitor", &pointer)
    .unwrap()
    .later("monitor", &pointer)
    .unwrap()
}

#[test]
fn an_acknowledged_added_unit_passes_complete_while_it_stays_undetermined() {
    let earlier = catalogue(&[("monitor", &monitor(&[("power", None)]))]);
    let later = catalogue(&[("monitor", &monitor(&[("power", Some("W"))]))]);
    let acknowledgements = DifferenceAcknowledgements::new(vec![unit_change("power")]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    assert_eq!(comparison.outcome(), Classification::Undetermined);
    assert_eq!(comparison.disposition(), Disposition::Passed);
    assert_eq!(
        comparison.acknowledged_count(Classification::Undetermined),
        1
    );
    assert_eq!(comparison.unused_acknowledgements().len(), 0);
    let difference = comparison.differences().next().unwrap();
    assert_eq!(difference.classification(), Classification::Undetermined);
    assert_eq!(difference.reason().unwrap().code(), "comparison.unit_added");
    assert!(difference.acknowledged());
    assert_eq!(difference.acknowledgement(), Some(unit_change("power")));

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["result"]["outcome"], "undetermined");
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(wire["outcome"]["incomplete_checks"], 0);
    assert_eq!(wire["coverage"]["requested"]["assessed"], 1);
    assert_eq!(wire["coverage"]["requested"]["undetermined"], 0);
    assert_eq!(
        wire["result"]["summary"]["acknowledged"],
        json!({"breaking": 0, "undetermined": 1})
    );
    let difference = &wire["result"]["differences"][0];
    assert_eq!(difference["classification"], "undetermined");
    assert_eq!(difference["reason"]["code"], "comparison.unit_added");
    assert_eq!(difference["acknowledged"], true);
    let pointer = "/sdfObject/Monitor/sdfProperty/power/unit";
    assert_eq!(
        difference["acknowledgement"],
        json!({"kind": "changed", "subject": "quality", "quality": "unit",
            "classification": "undetermined",
            "earlier": {"document": "monitor", "pointer": pointer},
            "later": {"document": "monitor", "pointer": pointer}})
    );
    let human = comparison.to_human();
    assert!(
        human.starts_with(
            "Revision comparison esdieff.revision-comparison/4: undetermined\nDisposition: passed, with 0 breaking and 1 undetermined differences acknowledged\n"
        ),
        "{human}"
    );
    assert!(
        human.ends_with(
            "Acknowledged differences:\n  changed quality unit (thing-supplied data), undetermined: unit is added, which changes the stated meaning of values\n    earlier monitor /sdfObject/Monitor/sdfProperty/power/unit (absent)\n    later monitor /sdfObject/Monitor/sdfProperty/power/unit\n"
        ),
        "{human}"
    );
    assert!(!human.contains("Undetermined differences:"), "{human}");
}

#[test]
fn a_difference_the_operation_limit_left_undetermined_is_never_acknowledged() {
    // Looking up a long unrecorded spelling in every recorded row exhausts a low allowance.
    let earlier = monitor(&[("temperature", Some("Cel"))]);
    let later = monitor(&[("temperature", Some(&"x".repeat(4000)))]);
    let acknowledgements =
        DifferenceAcknowledgements::new(vec![unit_change("temperature")]).unwrap();
    let compare = |limits: esdieff::Limits, check: &dyn Fn(&esdieff::RevisionComparison<'_>)| {
        let revision = |text: &str| {
            esdieff::Catalogue::with_limits(
                &[esdieff::SourceInput::supplied("monitor", text.as_bytes())],
                limits.clone(),
            )
            .unwrap()
        };
        let (earlier, later) = (revision(&earlier), revision(&later));
        let comparison = ComparisonRequest::new(&earlier, &later)
            .acknowledgements(&acknowledgements)
            .compare()
            .unwrap();
        check(&comparison);
    };
    compare(esdieff::Limits::default(), &|comparison| {
        let difference = comparison.differences().next().unwrap();
        assert_eq!(
            difference.reason().unwrap().code(),
            "comparison.unit_unrecorded"
        );
        assert!(difference.acknowledged());
        assert_eq!(comparison.disposition(), Disposition::Passed);
    });
    compare(
        esdieff::Limits::default().decoding_work(200_000),
        &|comparison| {
            // Beside the unit change, the stopped document is unavailable.
            let difference = comparison
                .differences()
                .find(|difference| difference.subject() == DifferenceSubject::Quality("unit"))
                .unwrap();
            // The same identity as the acknowledged unit change.
            assert_eq!(difference.kind(), DifferenceKind::Changed);
            assert_eq!(difference.subject(), DifferenceSubject::Quality("unit"));
            assert_eq!(difference.classification(), Classification::Undetermined);
            for revision in [Revision::Earlier, Revision::Later] {
                assert_eq!(
                    difference.side(revision).unwrap().pointer(8192).present(),
                    Some("/sdfObject/Monitor/sdfProperty/temperature/unit".to_owned())
                );
            }
            assert_eq!(difference.reason().unwrap().code(), "operation.limit");
            assert!(!difference.acknowledged() && difference.acknowledgement().is_none());
            assert_eq!(
                comparison.acknowledged_count(Classification::Undetermined),
                0
            );
            assert_eq!(comparison.disposition(), Disposition::Undetermined);
            assert_eq!(
                comparison.unused_acknowledgements().collect::<Vec<_>>(),
                [0]
            );
            let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
            for difference in wire["result"]["differences"].as_array().unwrap() {
                assert_eq!(difference["reason"]["code"], "operation.limit");
                assert_eq!(difference["acknowledged"], false);
                assert!(difference.get("acknowledgement").is_none());
            }
            assert_eq!(wire["outcome"]["complete"], false);
            assert_eq!(wire["result"]["unused_acknowledgements"], json!([0]));
        },
    );
}

/// A changed quality the comparison left undetermined, located at the same pointer in both
/// revisions of one document, with the entry that names it.
struct Undetermined {
    earlier: &'static str,
    later: &'static str,
    quality: &'static str,
    pointer: &'static str,
    reason: &'static str,
}
impl Undetermined {
    fn entry(&self) -> DifferenceAcknowledgement {
        DifferenceAcknowledgement::new(
            DifferenceKind::Changed,
            DifferenceSubject::Quality(self.quality),
            Classification::Undetermined,
        )
        .unwrap()
        .earlier("device", self.pointer)
        .unwrap()
        .later("device", self.pointer)
        .unwrap()
    }
    /// Compare the revisions with the entry supplied, after checking the one difference is
    /// the named change with the expected reason.
    fn compare(&self, check: impl Fn(&esdieff::RevisionComparison<'_>, &Value)) {
        let earlier = catalogue(&[("device", self.earlier)]);
        let later = catalogue(&[("device", self.later)]);
        let acknowledgements = DifferenceAcknowledgements::new(vec![self.entry()]).unwrap();
        let comparison = ComparisonRequest::new(&earlier, &later)
            .acknowledgements(&acknowledgements)
            .compare()
            .unwrap();
        let difference = comparison.differences().next().unwrap();
        assert_eq!(comparison.differences().len(), 1);
        assert_eq!(difference.kind(), DifferenceKind::Changed);
        assert_eq!(
            difference.subject(),
            DifferenceSubject::Quality(self.quality)
        );
        assert_eq!(difference.classification(), Classification::Undetermined);
        assert_eq!(difference.reason().unwrap().code(), self.reason);
        let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
        check(&comparison, &wire["result"]["differences"][0]);
    }
}

/// A Property whose `writable` is not a Boolean, so the directions its bound affects cannot be
/// established.
const UNKNOWN_DIRECTIONS: Undetermined = Undetermined {
    earlier: r#"{"sdfObject":{"Valve":{"sdfProperty":{"level":{"type":"number","minimum":0,"maximum":100,"writable":"yes"}}}}}"#,
    later: r#"{"sdfObject":{"Valve":{"sdfProperty":{"level":{"type":"number","minimum":0,"maximum":10,"writable":"yes"}}}}}"#,
    quality: "maximum",
    pointer: "/sdfObject/Valve/sdfProperty/level/maximum",
    reason: "comparison.directions_unavailable",
};

/// An `sdfRequired` that names a declaration the later revision cannot resolve.
const UNRESOLVED_REQUIREMENT: Undetermined = Undetermined {
    earlier: r#"{"sdfObject":{"Oven":{"sdfRequired":["temp"],"sdfProperty":{"temp":{"type":"number"},"mode":{"type":"string"}}}}}"#,
    later: r#"{"sdfObject":{"Oven":{"sdfRequired":["missing"],"sdfProperty":{"temp":{"type":"number"},"mode":{"type":"string"}}}}}"#,
    quality: "sdfRequired",
    pointer: "/sdfObject/Oven/sdfRequired",
    reason: "required.unavailable",
};

#[test]
fn a_difference_left_undetermined_by_unavailable_content_is_never_acknowledged() {
    for undetermined in [UNKNOWN_DIRECTIONS, UNRESOLVED_REQUIREMENT] {
        undetermined.compare(|comparison, wire| {
            let difference = comparison.differences().next().unwrap();
            assert!(!difference.acknowledged() && difference.acknowledgement().is_none());
            assert_eq!(
                comparison.acknowledged_count(Classification::Undetermined),
                0
            );
            assert_eq!(comparison.disposition(), Disposition::Undetermined);
            assert!(!comparison.complete());
            assert_eq!(
                comparison.unused_acknowledgements().collect::<Vec<_>>(),
                [0]
            );
            assert_eq!(wire["acknowledged"], false);
            assert!(wire.get("acknowledgement").is_none(), "{wire}");
        });
    }
}

#[test]
fn an_undetermined_difference_whose_content_was_examined_can_be_acknowledged() {
    // A unit that is not text, which the comparison does not classify, and a pattern every
    // representative value matches.
    let unclassified = Undetermined {
        earlier: r#"{"sdfObject":{"Valve":{"sdfProperty":{"level":{"type":"number","unit":5,"writable":false}}}}}"#,
        later: r#"{"sdfObject":{"Valve":{"sdfProperty":{"level":{"type":"number","unit":6,"writable":false}}}}}"#,
        quality: "unit",
        pointer: "/sdfObject/Valve/sdfProperty/level/unit",
        reason: "comparison.not_classified",
    };
    let unproven = Undetermined {
        earlier: r#"{"sdfObject":{"Lock":{"sdfProperty":{"code":{"type":"string","pattern":"^[^ ]+$"}}}}}"#,
        later: r#"{"sdfObject":{"Lock":{"sdfProperty":{"code":{"type":"string","pattern":"^[a-z]+$"}}}}}"#,
        quality: "pattern",
        pointer: "/sdfObject/Lock/sdfProperty/code/pattern",
        reason: "comparison.unproven",
    };
    for undetermined in [unclassified, unproven] {
        undetermined.compare(|comparison, wire| {
            let difference = comparison.differences().next().unwrap();
            assert!(difference.acknowledged());
            assert_eq!(difference.acknowledgement(), Some(undetermined.entry()));
            assert_eq!(comparison.disposition(), Disposition::Passed);
            assert_eq!(comparison.unused_acknowledgements().len(), 0);
            assert_eq!(wire["acknowledged"], true);
            assert_eq!(wire["acknowledgement"]["classification"], "undetermined");
        });
    }
}

#[test]
fn a_difference_that_becomes_breaking_comes_back_for_review() {
    // Without the recorded SenML evidence, a change from Cel to K stays undetermined, as under
    // rules that cannot settle it. With the evidence, the same change is breaking.
    let earlier = catalogue(&[("monitor", &monitor(&[("temperature", Some("Cel"))]))]);
    let later = catalogue(&[("monitor", &monitor(&[("temperature", Some("K"))]))]);
    let acknowledgements =
        DifferenceAcknowledgements::new(vec![unit_change("temperature")]).unwrap();
    let compare = |policy| {
        ComparisonRequest::new(&earlier, &later)
            .recorded_registry_policy(policy)
            .acknowledgements(&acknowledgements)
            .compare()
            .unwrap()
    };
    let reviewed = compare(esdieff::RecordedRegistryPolicy::WithholdRecorded);
    assert_eq!(reviewed.outcome(), Classification::Undetermined);
    assert_eq!(reviewed.disposition(), Disposition::Passed);
    assert!(reviewed.differences().next().unwrap().acknowledged());

    let reclassified = compare(esdieff::RecordedRegistryPolicy::UseRecorded);
    let difference = reclassified.differences().next().unwrap();
    assert_eq!(difference.classification(), Classification::Breaking);
    assert!(!difference.acknowledged());
    assert_eq!(reclassified.disposition(), Disposition::Failed);
    assert_eq!(
        reclassified.unused_acknowledgements().collect::<Vec<_>>(),
        [0]
    );
    let wire: Value = serde_json::from_str(&reclassified.to_json()).unwrap();
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["result"]["unused_acknowledgements"], json!([0]));
    assert_eq!(
        wire["result"]["differences"][0]["acknowledgement"]["classification"],
        "breaking"
    );
    let human = reclassified.to_human();
    assert!(
        human.contains("Breaking differences:\n  changed quality unit (thing-supplied data)\n"),
        "{human}"
    );
    assert!(
        human.ends_with(
            "Unused acknowledgements:\n  /differences/0 changed quality unit, undetermined\n    earlier monitor /sdfObject/Monitor/sdfProperty/temperature/unit\n    later monitor /sdfObject/Monitor/sdfProperty/temperature/unit\n"
        ),
        "{human}"
    );
}

/// An acknowledgement file holding these entries.
fn file(entries: Value) -> Vec<u8> {
    json!({"schema": "esdieff.acknowledged-differences", "version": "1", "differences": entries})
        .to_string()
        .into_bytes()
}

#[test]
fn the_file_is_decoded_within_the_callers_limits() {
    // The earlier location is a container nested three below the file.
    let bytes = file(json!([{"kind": "removed", "subject": "sdfProperty",
        "classification": "breaking", "earlier": {"document": "lock", "pointer": CREDENTIAL},
        "later": null}]));
    let decode = |limits: esdieff::Limits| DifferenceAcknowledgements::from_json(&bytes, &limits);
    assert!(decode(esdieff::Limits::default().depth(3)).is_ok());
    assert_eq!(
        decode(esdieff::Limits::default().depth(2)).err(),
        Some("acknowledgement file decoding limit")
    );
    assert_eq!(
        decode(esdieff::Limits::default().depth(0)).err(),
        Some("configuration exceeds supported finite ceilings")
    );
}

#[test]
fn the_file_and_the_typed_builder_give_the_same_results() {
    let typed =
        DifferenceAcknowledgements::new(vec![removed_credential(), removed_event()]).unwrap();
    let decoded = DifferenceAcknowledgements::from_json(
        &file(json!([
            {"kind": "removed", "subject": "sdfProperty", "classification": "breaking",
                "earlier": {"document": "lock", "pointer": CREDENTIAL}, "later": null},
            {"note": "Release 2 drops the event", "later": null, "classification": "breaking",
                "earlier": {"pointer": "/sdfObject/Lock/sdfEvent/stateChanged", "document": "lock"},
                "subject": "sdfEvent", "kind": "removed"}
        ])),
        &esdieff::Limits::default(),
    )
    .unwrap();
    assert_eq!(decoded, typed);
    assert_eq!(
        decoded.entries()[1].note(),
        Some("Release 2 drops the event")
    );
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[("lock", &without_credential_and_authorize())]);
    let report = |acknowledgements| {
        let comparison = ComparisonRequest::new(&earlier, &later)
            .acknowledgements(acknowledgements)
            .compare()
            .unwrap();
        (comparison.to_json(), comparison.to_human())
    };
    assert_eq!(report(&decoded), report(&typed));
    assert!(
        DifferenceAcknowledgements::from_json(&file(json!([])), &esdieff::Limits::default())
            .unwrap()
            .entries()
            .is_empty()
    );
}

#[test]
fn entries_copied_from_the_report_acknowledge_every_eligible_difference() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let lock = revised(&without_credential_and_authorize(), |document| {
        let state = &mut document["sdfObject"]["Lock"]["sdfProperty"]["state"];
        state["description"] = json!("Bolt position");
        state["unit"] = json!("s");
    });
    let later = catalogue(&[("lock", &lock)]);
    let first = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(first.count(Classification::Breaking), 2);
    assert_eq!(first.count(Classification::Undetermined), 1);
    assert_eq!(first.count(Classification::Compatible), 1);
    let entries = |report: &str| {
        let wire: Value = serde_json::from_str(report).unwrap();
        let entries: Vec<_> = wire["result"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|difference| difference.get("acknowledgement").cloned())
            .collect();
        Value::Array(entries)
    };
    let copied = entries(&first.to_json());
    assert_eq!(copied.as_array().unwrap().len(), 3);
    let acknowledgements =
        DifferenceAcknowledgements::from_json(&file(copied.clone()), &esdieff::Limits::default())
            .unwrap();
    let rerun = || {
        ComparisonRequest::new(&earlier, &later)
            .acknowledgements(&acknowledgements)
            .compare()
            .unwrap()
            .to_json()
    };
    let second = rerun();
    let wire: Value = serde_json::from_str(&second).unwrap();
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(
        wire["result"]["summary"]["acknowledged"],
        json!({"breaking": 2, "undetermined": 1})
    );
    assert_eq!(wire["result"]["unused_acknowledgements"], json!([]));
    assert_eq!(rerun(), second);
    assert_eq!(entries(&second), copied);
}

#[test]
fn an_acknowledgement_survives_an_unrelated_document_added_to_both_revisions() {
    let thermostat = include_str!("../examples/thermostat-check/rich.sdf.json");
    let earlier = catalogue(&[("added", thermostat), ("lock", LOCK)]);
    let later = catalogue(&[("added", thermostat), ("lock", &without_credential())]);
    let acknowledgements = DifferenceAcknowledgements::new(vec![removed_credential()]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    let difference = comparison.differences().next().unwrap();
    assert_eq!(
        difference
            .side(Revision::Earlier)
            .unwrap()
            .root_document()
            .get(),
        2
    );
    assert!(difference.acknowledged());
    assert_eq!(comparison.disposition(), Disposition::Passed);
}

#[test]
fn a_shared_definition_seen_through_two_groupings_needs_two_entries() {
    let strip = include_str!("../examples/power-strip-check/rich.sdf.json");
    let second = include_str!("../examples/power-strip-check/missing.sdf.json");
    let meter = include_str!("../examples/power-strip-check/definitions.sdf.json");
    let earlier = catalogue(&[("strip", strip), ("second", second), ("meter", meter)]);
    // Both groupings' total power can now read negative.
    let widened = revised(meter, |document| {
        document["sdfData"]["Power"]["minimum"] = json!(-10);
    });
    let later = catalogue(&[("strip", strip), ("second", second), ("meter", &widened)]);
    let entry = |document: &str| {
        let pointer = "/sdfThing/PowerStrip/sdfProperty/totalPower/minimum";
        DifferenceAcknowledgement::new(
            DifferenceKind::Changed,
            DifferenceSubject::Quality("minimum"),
            Classification::Breaking,
        )
        .unwrap()
        .earlier(document, pointer)
        .unwrap()
        .later(document, pointer)
        .unwrap()
    };
    let compare = |acknowledgements: &DifferenceAcknowledgements| {
        let comparison = ComparisonRequest::new(&earlier, &later)
            .acknowledgements(acknowledgements)
            .compare()
            .unwrap();
        assert_eq!(comparison.count(Classification::Breaking), 2);
        (
            comparison.disposition(),
            comparison.acknowledged_count(Classification::Breaking),
        )
    };
    let one = DifferenceAcknowledgements::new(vec![entry("strip")]).unwrap();
    assert_eq!(compare(&one), (Disposition::Failed, 1));
    let both = DifferenceAcknowledgements::new(vec![entry("strip"), entry("second")]).unwrap();
    assert_eq!(compare(&both), (Disposition::Passed, 2));
}

#[test]
fn a_pointer_that_cannot_be_rendered_leaves_its_difference_unacknowledged() {
    let limits = esdieff::Limits::default().pointer_bytes(16);
    let revision = |text: &str| {
        esdieff::Catalogue::with_limits(
            &[esdieff::SourceInput::supplied("lock", text.as_bytes())],
            limits.clone(),
        )
        .unwrap()
    };
    let (earlier, later) = (revision(LOCK), revision(&without_credential()));
    let acknowledgements = DifferenceAcknowledgements::new(vec![removed_credential()]).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later)
        .acknowledgements(&acknowledgements)
        .compare()
        .unwrap();
    let difference = comparison.differences().next().unwrap();
    assert!(!difference.acknowledged() && difference.acknowledgement().is_none());
    assert_eq!(comparison.disposition(), Disposition::Failed);
    assert_eq!(
        comparison.unused_acknowledgements().collect::<Vec<_>>(),
        [0]
    );
}

#[test]
fn a_reported_entry_that_matches_a_supplied_one_always_acknowledges_its_difference() {
    // Entries for both removals of the lock, as the report presents them.
    let removed = |subject: &str, pointer: &str| {
        json!({"kind": "removed", "subject": subject, "classification": "breaking",
            "earlier": {"document": "lock", "pointer": pointer}, "later": null})
    };
    let entries = [
        removed("sdfProperty", CREDENTIAL),
        removed("sdfAction", "/sdfObject/Lock/sdfAction/authorize"),
    ];
    let supplied =
        DifferenceAcknowledgements::from_json(&file(json!(entries)), &esdieff::Limits::default())
            .unwrap();
    // Materialized-pointer allowances from too small for one pointer to enough for both.
    for total in (20..=400).step_by(5) {
        let limits = esdieff::Limits::default().pointer_output_bytes(total);
        let revision = |text: &str| {
            esdieff::Catalogue::with_limits(
                &[esdieff::SourceInput::supplied("lock", text.as_bytes())],
                limits.clone(),
            )
            .unwrap()
        };
        let (earlier, later) = (
            revision(LOCK),
            revision(&without_credential_and_authorize()),
        );
        let comparison = ComparisonRequest::new(&earlier, &later)
            .acknowledgements(&supplied)
            .compare()
            .unwrap();
        for difference in comparison.differences() {
            if let Some(entry) = difference.acknowledgement() {
                assert_eq!(
                    difference.acknowledged(),
                    supplied.entries().contains(&entry),
                    "allowance {total}"
                );
            }
        }
        let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
        for difference in wire["result"]["differences"].as_array().unwrap() {
            if let Some(entry) = difference.get("acknowledgement") {
                assert_eq!(
                    difference["acknowledged"],
                    entries.contains(entry),
                    "allowance {total}: {difference}"
                );
            }
        }
    }
}

#[test]
fn the_report_reserves_the_unused_entry_list() {
    // Each one-document revision reserves 20504 bytes for its mandatory envelope, and each
    // supplied entry five more for its unused position.
    let limits = esdieff::Limits::default().report_bytes(41012);
    let revision = |text: &str| {
        esdieff::Catalogue::with_limits(
            &[esdieff::SourceInput::supplied("lock", text.as_bytes())],
            limits.clone(),
        )
        .unwrap()
    };
    let (earlier, later) = (revision(LOCK), revision(&without_credential()));
    let acknowledgements = DifferenceAcknowledgements::new(vec![removed_credential()]).unwrap();
    assert!(ComparisonRequest::new(&earlier, &later).compare().is_ok());
    assert_eq!(
        ComparisonRequest::new(&earlier, &later)
            .acknowledgements(&acknowledgements)
            .compare()
            .err(),
        Some("report cannot retain its mandatory envelope")
    );
}

#[test]
fn malformed_files_and_entries_are_refused() {
    let entry = json!({"kind": "removed", "subject": "sdfProperty", "classification": "breaking",
        "earlier": {"document": "lock", "pointer": CREDENTIAL}, "later": null});
    let with = |member: &str, value: Value| {
        let mut entry = entry.clone();
        entry[member] = value;
        file(json!([entry]))
    };
    let without = |member: &str| {
        let mut entry = entry.clone();
        entry.as_object_mut().unwrap().remove(member);
        file(json!([entry]))
    };
    let quality = json!({"kind": "changed", "subject": "quality", "quality": "minimum",
        "classification": "breaking", "earlier": null,
        "later": {"document": "lock", "pointer": "/minimum"}});
    let mut unquality = quality.clone();
    unquality.as_object_mut().unwrap().remove("quality");
    let long = |bytes: usize| "x".repeat(bytes);
    // Distinct quality entries located in both revisions and carrying a note.
    let many = |count: usize| {
        file(Value::Array(
            (0..count)
                .map(|index| {
                    let mut entry = with_note(&quality);
                    let side = json!({"document": "lock", "pointer": format!("/{index}")});
                    entry["earlier"] = side.clone();
                    entry["later"] = side;
                    entry
                })
                .collect(),
        ))
    };
    // Each malformed file with the reason it is refused.
    let refused: Vec<(&str, Vec<u8>)> = vec![
        ("acknowledgement file syntax or duplicate decoded member rejected", b"{\"schema\":".to_vec()),
        ("acknowledgement file requires UTF-8 JSON", b"{\"schema\":\"\xff\"}".to_vec()),
        ("acknowledgement file and each entry and location must be maps", b"[]".to_vec()),
        (
            "unknown acknowledgement file schema or version",
            json!({"schema": "esdieff.check-plan", "version": "1", "differences": []})
                .to_string()
                .into_bytes(),
        ),
        (
            "unknown acknowledgement file schema or version",
            json!({"schema": "esdieff.acknowledged-differences", "version": "2", "differences": []})
                .to_string()
                .into_bytes(),
        ),
        (
            "acknowledgement file requires differences",
            json!({"schema": "esdieff.acknowledged-differences", "version": "1"})
                .to_string()
                .into_bytes(),
        ),
        (
            "unknown acknowledgement file member",
            json!({"schema": "esdieff.acknowledged-differences", "version": "1", "differences": [], "extra": 1})
                .to_string()
                .into_bytes(),
        ),
        (
            "acknowledgement file syntax or duplicate decoded member rejected",
            br#"{"schema":"esdieff.acknowledged-differences","version":"1","version":"1","differences":[]}"#
                .to_vec(),
        ),
        ("unknown acknowledgement file member", with("witness", json!("0"))),
        ("unknown acknowledged difference kind", with("kind", json!("moved"))),
        ("unavailable content cannot be acknowledged", with("kind", json!("unavailable"))),
        ("a compatible difference cannot be acknowledged", with("classification", json!("compatible"))),
        ("unknown acknowledged classification", with("classification", json!("accepted"))),
        ("unknown acknowledged difference subject", with("subject", json!("sdfData"))),
        ("an acknowledgement names a quality exactly when its subject is quality", with("quality", json!("minimum"))),
        ("an acknowledgement names a quality exactly when its subject is quality", file(json!([unquality]))),
        ("required acknowledgement file member absent", without("classification")),
        ("an acknowledgement requires earlier and later members", without("later")),
        ("an acknowledgement requires an earlier or later location", with("earlier", Value::Null)),
        ("acknowledgement file and each entry and location must be maps", with("earlier", json!("lock"))),
        (
            "required acknowledgement file member absent",
            with("earlier", json!({"document": "lock"})),
        ),
        (
            "unknown acknowledgement file member",
            with("earlier", json!({"document": "lock", "pointer": "", "root": 1})),
        ),
        (
            "acknowledged document key must contain 1 through 256 bytes",
            with("earlier", json!({"document": "", "pointer": CREDENTIAL})),
        ),
        (
            "acknowledged document key must contain 1 through 256 bytes",
            with("earlier", json!({"document": long(257), "pointer": CREDENTIAL})),
        ),
        (
            "invalid or oversized JSON pointer selector",
            with("earlier", json!({"document": "lock", "pointer": "sdfObject"})),
        ),
        (
            "invalid JSON pointer escape",
            with("earlier", json!({"document": "lock", "pointer": "/a~2"})),
        ),
        (
            "invalid or oversized JSON pointer selector",
            with("earlier", json!({"document": "lock", "pointer": format!("/{}", long(8192))})),
        ),
        ("acknowledgement note exceeds 1024 bytes", with("note", json!(long(1025)))),
        ("acknowledgement file member must be a string", with("note", json!(1))),
        ("duplicate acknowledgement", file(json!([entry, with_note(&entry)]))),
        ("acknowledgements exceed 4096 entries", many(4097)),
        (
            "acknowledgement file exceeds 1 MiB",
            file(json!([with_note(&entry)]))
                .into_iter()
                .chain(std::iter::repeat_n(b' ', 1024 * 1024))
                .collect(),
        ),
    ];
    for (refusal, bytes) in refused {
        assert_eq!(
            DifferenceAcknowledgements::from_json(&bytes, &esdieff::Limits::default()).err(),
            Some(refusal)
        );
    }
    assert_eq!(
        DifferenceAcknowledgements::from_json(&many(4096), &esdieff::Limits::default())
            .unwrap()
            .entries()
            .len(),
        4096
    );
    let mut long_quality = quality.clone();
    long_quality["quality"] = json!(long(256));
    assert!(
        DifferenceAcknowledgements::from_json(
            &file(json!([quality, long_quality.clone()])),
            &esdieff::Limits::default()
        )
        .is_ok()
    );
    long_quality["quality"] = json!(long(257));
    assert!(
        DifferenceAcknowledgements::from_json(
            &file(json!([long_quality])),
            &esdieff::Limits::default()
        )
        .is_err()
    );
    assert_eq!(
        DifferenceAcknowledgements::from_json(
            &file(json!([with_note(&entry), entry])),
            &esdieff::Limits::default()
        )
        .err(),
        Some("duplicate acknowledgement")
    );
}

fn with_note(entry: &Value) -> Value {
    let mut entry = entry.clone();
    entry["note"] = json!("Matching ignores the note");
    entry
}

#[test]
fn the_typed_builder_refuses_what_the_file_refuses() {
    use Classification::{Breaking, Compatible};
    use DifferenceKind::{Removed, Unavailable};
    let property = DifferenceSubject::Definition(Class::Property);
    assert!(DifferenceAcknowledgement::new(Unavailable, property, Breaking).is_err());
    assert!(DifferenceAcknowledgement::new(Removed, property, Compatible).is_err());
    assert!(
        DifferenceAcknowledgement::new(
            Removed,
            DifferenceSubject::Definition(Class::Data),
            Breaking
        )
        .is_err()
    );
    let entry = DifferenceAcknowledgement::new(Removed, property, Breaking).unwrap();
    assert!(entry.clone().earlier("", CREDENTIAL).is_err());
    assert!(entry.clone().earlier("lock", "sdfObject").is_err());
    assert!(entry.clone().with_note(&"x".repeat(1025)).is_err());
    assert_eq!(
        DifferenceAcknowledgements::new(vec![entry.clone()]).err(),
        Some("an acknowledgement requires an earlier or later location")
    );
    let located = entry.earlier("lock", CREDENTIAL).unwrap();
    assert_eq!(
        DifferenceAcknowledgements::new(vec![located.clone(), located.with_note("again").unwrap()])
            .err(),
        Some("duplicate acknowledgement")
    );
}
