//! Grouping count obligations through public revision comparisons.
mod comparison_support;
use comparison_support::{at, catalogue, pointer};
use esdieff::{
    Classification, ComparisonRequest, DifferenceKind, DifferenceSubject, Perspective, Revision,
};

const A: &str = r#"{"info":{"version":"1"},"sdfThing":{"Strip":{"sdfRequired":["Socket"],"sdfObject":{"Socket":{"minItems":2,"maxItems":4,"sdfProperty":{"on":{"type":"boolean"}}}}}}}"#;
const B: &str = r#"{"info":{"version":"1"},"sdfThing":{"Strip":{"sdfRequired":["Socket"],"sdfObject":{"Socket":{"minItems":3,"maxItems":4,"sdfProperty":{"on":{"type":"boolean"}}}}}}}"#;

#[test]
fn every_earlier_socket_count_must_remain_permitted() {
    use Classification::{Breaking, Compatible};
    for (before, after, implementer, client) in
        [(A, B, Breaking, Compatible), (B, A, Compatible, Breaking)]
    {
        let earlier = catalogue(&[("strip", before)]);
        let later = catalogue(&[("strip", after)]);
        for (perspective, expected) in [
            (Perspective::Implementer, implementer),
            (Perspective::Client, client),
        ] {
            let comparison = ComparisonRequest::new(&earlier, &later)
                .perspective(perspective)
                .compare()
                .unwrap();
            assert_eq!(comparison.outcome(), expected);
            assert_eq!(comparison.differences().len(), 1);
            let difference = at(&comparison, "/minItems");
            assert_eq!(difference.classification(), expected);
            assert_eq!(difference.kind(), DifferenceKind::Changed);
            assert_eq!(difference.subject(), DifferenceSubject::Quality("minItems"));
            for revision in [Revision::Earlier, Revision::Later] {
                assert_eq!(
                    pointer(&difference, revision),
                    "/sdfThing/Strip/sdfObject/Socket/minItems"
                );
            }
            assert!(difference.witness().is_none());
            assert_eq!(difference.directions().count(), 0);
            assert_eq!(
                comparison.version_warnings().len(),
                usize::from(expected == Breaking)
            );
        }
    }
}

/// Counts at either class, at a top-level or optional nested coordinate.
fn grouping(counts: &str, class: &str, nested: bool) -> String {
    let grouping = format!(r#"{{"{class}":{{"Sockets":{{{counts}}}}}}}"#);
    if nested {
        format!(r#"{{"sdfThing":{{"Optional":{grouping}}}}}"#)
    } else {
        grouping
    }
}

#[test]
fn count_matrix_preserves_all_earlier_counts_in_both_classes_and_at_each_depth() {
    use Classification::{Breaking as B, Compatible as C};
    // Literal expectations from the count policy, each entry is (implementer, client).
    // Reverse expectations differ only when the bounds have different effective meanings.
    for (before, after, rows) in [
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":3,"maxItems":4"#,
            vec![("minItems", B, C, C, B)],
        ),
        (
            r#""minItems":3,"maxItems":4"#,
            r#""minItems":2,"maxItems":4"#,
            vec![("minItems", C, B, B, C)],
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":2,"maxItems":3"#,
            vec![("maxItems", B, C, C, B)],
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":2,"maxItems":6"#,
            vec![("maxItems", C, B, B, C)],
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":1,"maxItems":6"#,
            vec![("minItems", C, B, B, C), ("maxItems", C, B, B, C)],
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":3,"maxItems":3"#,
            vec![("minItems", B, C, C, B), ("maxItems", B, C, C, B)],
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":3,"maxItems":6"#,
            vec![("minItems", B, C, C, B), ("maxItems", C, B, B, C)],
        ),
        (
            r#""minItems":3,"maxItems":6"#,
            r#""minItems":2,"maxItems":4"#,
            vec![("minItems", C, B, B, C), ("maxItems", B, C, C, B)],
        ),
        (
            r#""maxItems":4"#,
            r#""minItems":0,"maxItems":4"#,
            vec![("minItems", C, C, C, C)],
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""maxItems":4"#,
            vec![("minItems", C, B, B, C)],
        ),
        (
            r#""minItems":2"#,
            r#""minItems":2,"maxItems":4"#,
            vec![("maxItems", B, C, C, B)],
        ),
        (
            r#""maxItems":0"#,
            r#""maxItems":1"#,
            vec![("maxItems", C, B, B, C)],
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":2e0,"maxItems":4"#,
            vec![],
        ),
        (
            r#""minItems":9007199254740993"#,
            r#""minItems":9007199254740994"#,
            vec![("minItems", B, C, C, B)],
        ),
    ] {
        for class in ["sdfObject", "sdfThing"] {
            for nested in [false, true] {
                let before = catalogue(&[("sockets", &grouping(before, class, nested))]);
                let after = catalogue(&[("sockets", &grouping(after, class, nested))]);
                for reverse in [false, true] {
                    let (earlier, later) = if reverse {
                        (&after, &before)
                    } else {
                        (&before, &after)
                    };
                    for perspective in [Perspective::Client, Perspective::Implementer] {
                        let comparison = ComparisonRequest::new(earlier, later)
                            .perspective(perspective)
                            .compare()
                            .unwrap();
                        assert_eq!(comparison.differences().len(), rows.len());
                        let mut outcome = C;
                        for &(quality, forward_i, forward_c, reverse_i, reverse_c) in &rows {
                            let expected = match (reverse, perspective) {
                                (false, Perspective::Implementer) => forward_i,
                                (false, Perspective::Client) => forward_c,
                                (true, Perspective::Implementer) => reverse_i,
                                (true, Perspective::Client) => reverse_c,
                                _ => unreachable!("selected perspectives"),
                            };
                            let difference = at(&comparison, &format!("/{quality}"));
                            assert_eq!(
                                difference.classification(),
                                expected,
                                "{class} {nested} {reverse} {perspective:?} {quality}"
                            );
                            assert_eq!(difference.kind(), DifferenceKind::Changed);
                            assert!(difference.witness().is_none());
                            assert_eq!(difference.directions().count(), 0);
                            if expected == B {
                                outcome = B;
                            }
                        }
                        assert_eq!(comparison.outcome(), outcome);
                    }
                }
            }
        }
    }
}

#[test]
fn count_facts_stand_beside_presence_requiredness_and_unresolved_relationships() {
    use serde_json::json;
    for required in [json!([]), json!(["Socket"])] {
        let mut before: serde_json::Value = serde_json::from_str(A).unwrap();
        before["sdfThing"]["Strip"]["sdfRequired"] = required;
        let mut after = before.clone();
        let socket = &mut after["sdfThing"]["Strip"]["sdfObject"]["Socket"];
        socket["maxItems"] = json!(3);
        socket["sdfProperty"]["extra"] = json!({"type":"boolean"});
        socket["sdfRequired"] = json!(["on"]);
        let earlier = catalogue(&[("strip", &before.to_string())]);
        let later = catalogue(&[("strip", &after.to_string())]);
        let comparison = ComparisonRequest::new(&earlier, &later)
            .perspective(Perspective::Implementer)
            .compare()
            .unwrap();
        assert_eq!(
            at(&comparison, "/maxItems").classification(),
            Classification::Breaking
        );
        assert_eq!(
            at(&comparison, "/extra").classification(),
            Classification::Compatible
        );
        assert_eq!(
            at(&comparison, "/Socket/sdfRequired").classification(),
            Classification::Breaking
        );
        assert_eq!(comparison.outcome(), Classification::Breaking);
    }
}

#[test]
fn scalar_transitions_and_invalid_complete_ranges_remain_undetermined() {
    for (before, after) in [
        ("", r#""minItems":0"#),
        ("", r#""maxItems":1"#),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":-1,"maxItems":5"#,
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":1.5,"maxItems":5"#,
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":"2","maxItems":5"#,
        ),
        (
            r#""minItems":2,"maxItems":4"#,
            r#""minItems":5,"maxItems":4"#,
        ),
    ] {
        for class in ["sdfObject", "sdfThing"] {
            let a = catalogue(&[("sockets", &grouping(before, class, true))]);
            let b = catalogue(&[("sockets", &grouping(after, class, true))]);
            for (earlier, later) in [(&a, &b), (&b, &a)] {
                for perspective in [Perspective::Client, Perspective::Implementer] {
                    let comparison = ComparisonRequest::new(earlier, later)
                        .perspective(perspective)
                        .compare()
                        .unwrap();
                    assert_eq!(comparison.outcome(), Classification::Undetermined);
                    for difference in comparison.differences() {
                        assert_eq!(
                            difference.reason(),
                            Some(esdieff::UndeterminedReason::NotClassified)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn composed_counts_report_consumer_coordinates_and_nearest_origins_in_chronological_order() {
    for class in ["sdfObject", "sdfThing"] {
        let base = |min| {
            format!(
                r#"{{"namespace":{{"b":"urn:base"}},"defaultNamespace":"b","{class}":{{"Base":{{"minItems":{min},"maxItems":4}}}}}}"#
            )
        };
        let consumer = |patch: &str| {
            let reference = format!("b:#/{class}/Base");
            let refined: serde_json::Value =
                serde_json::from_str(&format!(r#"{{{patch}}}"#)).unwrap();
            let mut document = serde_json::json!({"namespace":{"b":"urn:base"},"sdfThing":{"Parent":{class:{"Copy":{"sdfRef":reference},"Refined":refined}}}});
            document["sdfThing"]["Parent"][class]["Refined"]["sdfRef"] = reference.into();
            document.to_string()
        };
        let old_consumer = consumer(r#""maxItems":6"#);
        let new_consumer = consumer(r#""maxItems":3,"minItems":null"#);
        let before = catalogue(&[("base", &base(2)), ("consumer", &old_consumer)]);
        let after = catalogue(&[("base", &base(3)), ("consumer", &new_consumer)]);
        for (earlier, later, reverse) in [(&before, &after, false), (&after, &before, true)] {
            let comparison = ComparisonRequest::new(earlier, later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            let copy = at(&comparison, "/Copy/minItems");
            assert_eq!(
                copy.classification(),
                if reverse {
                    Classification::Compatible
                } else {
                    Classification::Breaking
                }
            );
            for revision in [Revision::Earlier, Revision::Later] {
                let side = copy.side(revision).unwrap();
                assert_eq!(
                    side.pointer().require().unwrap(),
                    format!("/sdfThing/Parent/{class}/Copy/minItems")
                );
                let origin = side.origin().unwrap().sources().next().unwrap();
                assert_eq!(
                    origin.pointer().require().unwrap(),
                    format!("/{class}/Base/minItems")
                );
                assert_eq!(
                    comparison
                        .catalogue(revision)
                        .source(origin.source_id())
                        .unwrap()
                        .key(),
                    "base"
                );
            }
            let removed = at(&comparison, "/Refined/minItems");
            assert_eq!(
                removed.classification(),
                if reverse {
                    Classification::Breaking
                } else {
                    Classification::Compatible
                }
            );
            let absent_revision = if reverse {
                Revision::Earlier
            } else {
                Revision::Later
            };
            let absent = removed.side(absent_revision).unwrap();
            assert_eq!(absent.presence(), esdieff::Presence::Absent);
            let origin = absent.origin().unwrap().sources().next().unwrap();
            assert_eq!(
                origin.pointer().require().unwrap(),
                format!("/sdfThing/Parent/{class}/Refined/minItems")
            );
            assert_eq!(
                comparison
                    .catalogue(absent_revision)
                    .source(origin.source_id())
                    .unwrap()
                    .key(),
                "consumer"
            );
            assert_eq!(
                at(&comparison, "/Refined/maxItems").classification(),
                if reverse {
                    Classification::Compatible
                } else {
                    Classification::Breaking
                }
            );
        }
    }
}

#[test]
fn whole_groupings_and_unavailable_composition_keep_their_reporting_boundaries() {
    for class in ["sdfObject", "sdfThing"] {
        let present = catalogue(&[(
            "sockets",
            &grouping(r#""minItems":2,"maxItems":4"#, class, true),
        )]);
        let absent = catalogue(&[("sockets", r#"{"sdfThing":{"Optional":{}}}"#)]);
        for (earlier, later) in [(&absent, &present), (&present, &absent)] {
            let comparison = ComparisonRequest::new(earlier, later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            assert_eq!(comparison.differences().len(), 1);
            let row = comparison.differences().next().unwrap();
            assert!(matches!(row.subject(), DifferenceSubject::Definition(_)));
            assert_eq!(row.classification(), Classification::Compatible);
        }
        let unresolved = catalogue(&[(
            "sockets",
            &grouping(
                r##""sdfRef":"#/missing","minItems":3,"maxItems":4"##,
                class,
                true,
            ),
        )]);
        for (earlier, later) in [(&present, &unresolved), (&unresolved, &present)] {
            let comparison = ComparisonRequest::new(earlier, later)
                .perspective(Perspective::Implementer)
                .compare()
                .unwrap();
            assert_eq!(comparison.outcome(), Classification::Undetermined);
            assert!(comparison.differences().any(|d| d.reason().is_some()));
        }
    }
}

#[test]
fn unchanged_unresolved_requiredness_does_not_hide_established_counts() {
    let before = r#"{"sdfObject":{"Unresolved":{"sdfRequired":["missing"]},"Sockets":{"minItems":2,"maxItems":4}}}"#;
    let after = before.replace("maxItems\":4", "maxItems\":3");
    let earlier = catalogue(&[("model", before)]);
    let later = catalogue(&[("model", &after)]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .perspective(Perspective::Implementer)
        .compare()
        .unwrap();
    assert_eq!(
        at(&comparison, "/Sockets/maxItems").classification(),
        Classification::Breaking
    );
    assert_eq!(comparison.outcome(), Classification::Breaking);
}
