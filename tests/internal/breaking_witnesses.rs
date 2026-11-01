//! Breaking witnesses and compatibility proofs for changed numeric bounds.
//! Follows ADR 0039: Client-side revision comparison.
use esdieff::{
    Admission, Catalogue, Classification, ComparisonRequest, DifferenceSubject, Direction, Limits,
    Revision, RevisionComparison, SourceInput, UndeterminedReason,
};
use serde_json::{Value, json};

use super::comparison_support;
use comparison_support::{at, catalogue, revised, witness};

/// A meter whose `setpoint` only Clients supply and whose `reading` only the Thing supplies.
const METER: &str = r#"{"sdfObject": {"Meter": {"sdfProperty": {
    "setpoint": {"type": "number", "minimum": 0, "maximum": 100,
        "readable": false, "observable": false, "writable": true},
    "reading": {"type": "number", "minimum": 0, "maximum": 100, "writable": false}
}}}}"#;
/// A thermostat whose target temperature is readable and writable.
const THERMOSTAT: &str = include_str!("../../examples/thermostat-check/thermostat.sdf.json");
/// A power strip whose read-only total power reuses the shared Power data with a lower
/// maximum.
const STRIP: &str = include_str!("../../examples/power-strip-check/rich.sdf.json");
const METER_DEFINITIONS: &str =
    include_str!("../../examples/power-strip-check/definitions.sdf.json");

/// The meter with each `(property, quality, value)` revision applied, where a null value
/// removes the quality.
fn meter(revisions: &[(&str, &str, Value)]) -> String {
    revised(METER, |document| {
        let properties = &mut document["sdfObject"]["Meter"]["sdfProperty"];
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

fn compare_meters(revisions: &[(&str, &str, Value)]) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("meter", METER)]),
        catalogue(&[("meter", &meter(revisions))]),
    )
}

#[test]
fn a_narrowed_writable_range_breaks_with_a_value_the_later_revision_rejects() {
    let (earlier, later) = compare_meters(&[("setpoint", "maximum", json!(80))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let maximum = at(&comparison, "/setpoint/maximum");
    assert_eq!(maximum.subject(), DifferenceSubject::Quality("maximum"));
    assert_eq!(maximum.classification(), Classification::Breaking);
    assert!(maximum.reason().is_none());
    assert_eq!(
        witness(&maximum),
        (
            "100".to_owned(),
            Direction::ClientSupplied,
            Admission::Admitted,
            Admission::Rejected
        )
    );
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

#[test]
fn a_widened_readable_range_breaks_with_a_value_the_earlier_revision_rejects() {
    let (earlier, later) = compare_meters(&[("reading", "minimum", json!(-2.5))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let minimum = at(&comparison, "/reading/minimum");
    assert_eq!(minimum.classification(), Classification::Breaking);
    assert_eq!(
        witness(&minimum),
        (
            "-2.5".to_owned(),
            Direction::ThingSupplied,
            Admission::Rejected,
            Admission::Admitted
        )
    );
}

#[test]
fn widening_client_supplied_data_and_narrowing_thing_supplied_data_are_compatible() {
    let (earlier, later) = compare_meters(&[
        ("setpoint", "maximum", Value::Null),
        ("setpoint", "exclusiveMinimum", json!(-1)),
        ("setpoint", "minimum", Value::Null),
        ("reading", "minimum", json!(1e1)),
        ("reading", "exclusiveMaximum", json!(100)),
    ]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mut changed: Vec<_> = comparison
        .differences()
        .map(|difference| {
            let subject = difference.subject();
            assert_eq!(
                difference.classification(),
                Classification::Compatible,
                "{subject:?}"
            );
            assert!(difference.reason().is_none() && difference.witness().is_none());
            let side = difference.side(Revision::Later).unwrap();
            side.pointer(8192).require().unwrap()
        })
        .collect();
    changed.sort();
    let meter = "/sdfObject/Meter/sdfProperty";
    assert_eq!(
        changed,
        [
            format!("{meter}/reading/exclusiveMaximum"),
            format!("{meter}/reading/minimum"),
            format!("{meter}/setpoint/exclusiveMinimum"),
            format!("{meter}/setpoint/maximum"),
            format!("{meter}/setpoint/minimum"),
        ]
    );
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn a_loosened_bound_on_readable_and_writable_data_breaks_through_its_read_direction() {
    let earlier = catalogue(&[("thermostat", THERMOSTAT)]);
    let later = catalogue(&[(
        "thermostat",
        &THERMOSTAT.replacen("\"maximum\": 30", "\"maximum\": 35", 1),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let maximum = at(&comparison, "/targetTemperature/maximum");
    assert!(
        maximum.affects(Direction::ClientSupplied) && maximum.affects(Direction::ThingSupplied)
    );
    assert_eq!(maximum.classification(), Classification::Breaking);
    assert_eq!(
        witness(&maximum),
        (
            "35".to_owned(),
            Direction::ThingSupplied,
            Admission::Rejected,
            Admission::Admitted
        )
    );
}

#[test]
fn an_exclusive_bound_replacing_an_inclusive_one_narrows_by_the_boundary_value() {
    let replaced = |property| {
        [
            (property, "maximum", Value::Null),
            (property, "exclusiveMaximum", json!(100)),
        ]
    };
    let (earlier, later) = compare_meters(&[replaced("setpoint"), replaced("reading")].concat());
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for quality in ["maximum", "exclusiveMaximum"] {
        let setpoint = at(&comparison, &format!("/setpoint/{quality}"));
        assert_eq!(setpoint.classification(), Classification::Breaking);
        assert_eq!(
            witness(&setpoint),
            (
                "100".to_owned(),
                Direction::ClientSupplied,
                Admission::Admitted,
                Admission::Rejected
            )
        );
        // Promising less variation in Thing-supplied data is safe.
        let reading = at(&comparison, &format!("/reading/{quality}"));
        assert_eq!(reading.classification(), Classification::Compatible);
    }
}

#[test]
fn a_lowered_shared_minimum_breaks_the_power_strip_reading_at_its_shared_origin() {
    let earlier = catalogue(&[("strip", STRIP), ("meter", METER_DEFINITIONS)]);
    let meter = revised(METER_DEFINITIONS, |document| {
        let power = &mut document["sdfData"]["Power"];
        power["minimum"] = json!(-10);
        // The power strip's override keeps its effective maximum at 1800.
        power["maximum"] = json!(4000);
    });
    let later = catalogue(&[("strip", STRIP), ("meter", &meter)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 1);
    let minimum = at(
        &comparison,
        "/sdfThing/PowerStrip/sdfProperty/totalPower/minimum",
    );
    assert_eq!(minimum.classification(), Classification::Breaking);
    assert_eq!(
        witness(&minimum),
        (
            "-10".to_owned(),
            Direction::ThingSupplied,
            Admission::Rejected,
            Admission::Admitted
        )
    );
    for revision in [Revision::Earlier, Revision::Later] {
        let origin = minimum.side(revision).unwrap().origin().unwrap();
        let source = origin.sources().next().unwrap();
        let key = comparison
            .catalogue(revision)
            .source(source.source_id())
            .unwrap()
            .key();
        assert_eq!(
            (key, source.pointer(8192).require().unwrap().as_str()),
            ("meter", "/sdfData/Power/minimum")
        );
    }
}

#[test]
fn a_candidate_another_constraint_excludes_leaves_its_difference_undetermined() {
    // The constant already excluded the boundary value that the narrowed setpoint and the
    // widened reading separate.
    let pinned = |property| (property, "const", json!(50));
    let lowered = |property| (property, "maximum", json!(80));
    let earlier = catalogue(&[(
        "meter",
        &meter(&[pinned("setpoint"), pinned("reading"), lowered("reading")]),
    )]);
    let later = catalogue(&[(
        "meter",
        &meter(&[pinned("setpoint"), pinned("reading"), lowered("setpoint")]),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for suffix in ["/setpoint/maximum", "/reading/maximum"] {
        let maximum = at(&comparison, suffix);
        assert_eq!(
            maximum.classification(),
            Classification::Undetermined,
            "{suffix}"
        );
        assert!(maximum.witness().is_none());
        assert_eq!(maximum.reason(), Some(UndeterminedReason::Unproven));
        assert_eq!(maximum.reason().unwrap().code(), "comparison.unproven");
    }
}

#[test]
fn a_loosened_bound_beside_a_changed_unclassified_quality_is_undetermined() {
    let (earlier, later) = compare_meters(&[
        ("setpoint", "maximum", json!(120)),
        // Sample-value checking does not check this format, so it is not classified.
        ("setpoint", "format", json!("email")),
    ]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let maximum = at(&comparison, "/setpoint/maximum");
    assert_eq!(maximum.classification(), Classification::Undetermined);
    assert_eq!(maximum.reason(), Some(UndeterminedReason::Unproven));
    let format = at(&comparison, "/setpoint/format");
    assert_eq!(format.reason(), Some(UndeterminedReason::NotClassified));
}

#[test]
fn action_input_and_event_output_are_witnessed_in_their_own_directions() {
    let device = |input: u32, output: u32| {
        json!({"sdfObject": {"Dimmer": {
            "sdfAction": {"set": {"sdfInputData": {"type": "integer", "maximum": input}}},
            "sdfEvent": {"changed": {"sdfOutputData": {"type": "integer", "maximum": output}}}
        }}})
        .to_string()
    };
    let earlier = catalogue(&[("dimmer", &device(10, 10))]);
    let later = catalogue(&[("dimmer", &device(5, 20))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let input = at(&comparison, "/sdfInputData/maximum");
    assert_eq!(
        witness(&input),
        (
            "10".to_owned(),
            Direction::ClientSupplied,
            Admission::Admitted,
            Admission::Rejected
        )
    );
    let output = at(&comparison, "/sdfOutputData/maximum");
    assert_eq!(
        witness(&output),
        (
            "20".to_owned(),
            Direction::ThingSupplied,
            Admission::Rejected,
            Admission::Admitted
        )
    );
}

#[test]
fn a_gap_without_an_admitted_bound_value_is_witnessed_just_across_its_edge() {
    use Direction::{ClientSupplied, ThingSupplied};
    let none = Value::Null;
    let cases = [
        // Just across the earlier exclusive bound, inside the later inclusive one.
        (
            vec![
                ("setpoint", "minimum", none.clone()),
                ("setpoint", "exclusiveMinimum", json!(0)),
            ],
            vec![("setpoint", "minimum", json!(10))],
            "/setpoint/minimum",
            "1",
            ClientSupplied,
        ),
        // One unit reaches the other edge, so half a unit stays inside.
        (
            vec![
                ("setpoint", "minimum", none.clone()),
                ("setpoint", "exclusiveMinimum", json!(3)),
            ],
            vec![("setpoint", "minimum", json!(4))],
            "/setpoint/exclusiveMinimum",
            "3.5",
            ClientSupplied,
        ),
        // Without the other bound, an integer just across the new one.
        (
            vec![("setpoint", "maximum", none.clone())],
            vec![],
            "/setpoint/maximum",
            "101",
            ClientSupplied,
        ),
        (
            vec![("setpoint", "minimum", none.clone())],
            vec![("setpoint", "minimum", json!(-2.5))],
            "/setpoint/minimum",
            "-2.6",
            ClientSupplied,
        ),
        (
            vec![("reading", "maximum", json!(2.5))],
            vec![("reading", "maximum", none.clone())],
            "/reading/maximum",
            "2.6",
            ThingSupplied,
        ),
        (
            vec![],
            vec![("reading", "maximum", none)],
            "/reading/maximum",
            "101",
            ThingSupplied,
        ),
    ];
    for (old, new, suffix, value, direction) in cases {
        let earlier = catalogue(&[("meter", &meter(&old))]);
        let later = catalogue(&[("meter", &meter(&new))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Breaking,
            "{value}"
        );
        let witness = difference.witness().unwrap();
        assert_eq!((witness.value(), witness.direction()), (value, direction));
    }
}

#[test]
fn data_changes_in_no_direction_both_revisions_offer_are_compatible() {
    // The reading becomes write-only, so no Client direction carries its data in both
    // revisions, and only the characteristic changes can break a Client.
    let earlier = revised(METER, |document| {
        document["sdfObject"]["Meter"]["sdfProperty"]["reading"]["unit"] = json!("Cel");
    });
    let later = revised(METER, |document| {
        let reading = &mut document["sdfObject"]["Meter"]["sdfProperty"]["reading"];
        reading["unit"] = json!("K");
        reading["maximum"] = json!(80);
        reading["pattern"] = json!("^[0-9]+$");
        reading["readable"] = json!(false);
        reading["observable"] = json!(false);
        reading["writable"] = json!(true);
    });
    let earlier = catalogue(&[("meter", &earlier)]);
    let later = catalogue(&[("meter", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    for quality in ["unit", "maximum", "pattern"] {
        let difference = at(&comparison, &format!("/reading/{quality}"));
        assert_eq!(difference.directions().count(), 0, "{quality}");
        assert_eq!(
            difference.classification(),
            Classification::Compatible,
            "{quality}"
        );
    }
    let readable = at(&comparison, "/reading/readable");
    assert_eq!(readable.classification(), Classification::Breaking);
}

/// Whether the JSON report names the reading of ADR 0037: Undeclared extension vocabulary.
fn relies_on_extension_reading(comparison: &RevisionComparison<'_>) -> bool {
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    wire["baseline"]["interpretations"]
        .as_array()
        .unwrap()
        .contains(&json!("undeclared-extensions-ignorable/1"))
}

#[test]
fn a_witness_relies_on_the_extension_vocabulary_its_checks_set_aside() {
    // Only the witness checks evaluate the alternative that carries extension vocabulary.
    let setpoint = |maximum: u32, pinned: bool| {
        let mut setpoint = json!({"type": "number", "maximum": maximum,
            "readable": false, "observable": false, "writable": true,
            "sdfChoice": {"low": {"maximum": 10}, "high": {"minimum": 50, "acme:hint": "boost"}}});
        if pinned {
            setpoint["const"] = json!(60);
        }
        json!({"sdfObject": {"Meter": {"sdfProperty": {"setpoint": setpoint}}}}).to_string()
    };
    for (pinned, classification) in [
        (false, Classification::Breaking),
        (true, Classification::Undetermined),
    ] {
        let earlier = catalogue(&[("meter", &setpoint(100, pinned))]);
        let later = catalogue(&[("meter", &setpoint(80, pinned))]);
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let maximum = at(&comparison, "/setpoint/maximum");
        assert_eq!(maximum.classification(), classification);
        assert_eq!(relies_on_extension_reading(&comparison), !pinned);
    }
}

#[test]
fn both_reports_show_each_witness_with_its_admission_in_both_revisions() {
    // The earlier bound's own spelling is the witness.
    let spelled = METER.replacen("\"maximum\": 100", "\"maximum\": 1e2", 1);
    let earlier = catalogue(&[("meter", &spelled)]);
    let later = catalogue(&[("meter", &meter(&[("setpoint", "maximum", json!(80))]))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let maximum = at(&comparison, "/setpoint/maximum");
    assert_eq!(maximum.witness().unwrap().value(), "1e2");
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let differences = wire["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), 1);
    assert_eq!(
        differences[0]["witness"],
        json!({"direction": "client_supplied", "raw_json": "1e2",
            "admission": {"earlier": "admitted", "later": "rejected"}})
    );
    assert!(differences[0].get("reason").is_none());
    let human = comparison.to_human();
    let setpoint = "/sdfObject/Meter/sdfProperty/setpoint/maximum";
    assert!(
        human.contains(&format!(
            "  changed quality maximum (client-supplied data)\n    witness 1e2 for client-supplied data: earlier admitted, later rejected\n    earlier meter {setpoint}\n    later meter {setpoint}\n"
        )),
        "{human}"
    );

    // Differences without a witness carry none.
    let (earlier, later) = compare_meters(&[("setpoint", "maximum", json!(120))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert!(wire["result"]["differences"][0].get("witness").is_none());
    assert!(!comparison.to_human().contains("witness"));
}

type Settled = (Classification, Option<UndeterminedReason>);

/// The removed alarm, the narrowed setpoint maximum and the outcome of comparing them
/// within `work`, or `None` when the revisions cannot be prepared or the setpoint not reached.
fn within_work(
    earlier: &str,
    later: &str,
    work: usize,
) -> Option<(Settled, Settled, Classification)> {
    let limits = Limits::default().decoding_work(work);
    let revision = |text: &str| {
        Catalogue::with_limits(
            &[SourceInput::supplied("meter", text.as_bytes())],
            limits.clone(),
        )
    };
    let (earlier, later) = (revision(earlier).ok()?, revision(later).ok()?);
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
        settled("/sdfEvent/alarm")?,
        settled("/setpoint/maximum")?,
        comparison.outcome(),
    ))
}

#[test]
fn an_allowance_exhausted_during_witness_checking_leaves_that_difference_undetermined() {
    let earlier = revised(METER, |document| {
        document["sdfObject"]["Meter"]["sdfEvent"] = json!({"alarm": {"description": "Overload"}});
    });
    let later = meter(&[("setpoint", "maximum", json!(80))]);
    let witnessed = |work| {
        within_work(&earlier, &later, work)
            .is_some_and(|(_, maximum, _)| maximum == (Classification::Breaking, None))
    };
    // The least work with which the witness check completes.
    let (mut low, mut high) = (1, 10_000_000);
    assert!(witnessed(high));
    while low < high {
        let middle = low + (high - low) / 2;
        if witnessed(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let (alarm, maximum, outcome) = within_work(&earlier, &later, low - 1).unwrap();
    assert_eq!(alarm, (Classification::Breaking, None));
    assert_eq!(maximum.0, Classification::Undetermined);
    let reason = maximum.1.unwrap();
    assert_eq!(reason.code(), "operation.limit");
    assert_eq!(
        reason.message(),
        "witness checking exhausted the operation allowance"
    );
    assert_eq!(outcome, Classification::Breaking);
}
