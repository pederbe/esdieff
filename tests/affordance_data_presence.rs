//! Input or output data that an affordance loses changes the affordance as a whole, so it is
//! breaking without a Breaking witness. Follows ADR 0039: Client-side revision comparison.
use esdieff::{
    Classification, ComparisonRequest, DifferenceKind, DifferenceSubject, Direction, Presence,
    Revision,
};
use serde_json::Value;

mod comparison_support;
use comparison_support::{at, catalogue, revised};

/// A valve whose Action takes input and gives output, and whose Event gives output.
const VALVE: &str = r#"{"sdfObject": {"Valve": {
    "sdfAction": {"open": {
        "sdfInputData": {"type": "integer", "minimum": 0, "maximum": 100},
        "sdfOutputData": {"type": "boolean"}
    }},
    "sdfEvent": {"blocked": {"sdfOutputData": {"type": "string"}}}
}}}"#;

/// The valve with its definition revised.
fn valve(revise: impl FnOnce(&mut Value)) -> String {
    revised(VALVE, |document| {
        revise(&mut document["sdfObject"]["Valve"])
    })
}

/// Remove a member from a definition of the valve.
fn without(definition: &mut Value, member: &str) {
    definition.as_object_mut().unwrap().remove(member);
}

#[test]
fn an_action_that_loses_its_input_data_breaks_client_supplied_data() {
    let earlier = catalogue(&[("valve", VALVE)]);
    let later = catalogue(&[(
        "valve",
        &valve(|valve| without(&mut valve["sdfAction"]["open"], "sdfInputData")),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 1);
    let difference = at(&comparison, "/sdfObject/Valve/sdfAction/open/sdfInputData");
    assert_eq!(difference.kind(), DifferenceKind::Changed);
    assert_eq!(
        difference.subject(),
        DifferenceSubject::Quality("sdfInputData")
    );
    assert_eq!(difference.classification(), Classification::Breaking);
    assert!(difference.reason().is_none());
    assert!(difference.witness().is_none());
    assert_eq!(
        difference.directions().collect::<Vec<_>>(),
        [Direction::ClientSupplied]
    );
    assert_eq!(
        difference.side(Revision::Later).unwrap().presence(),
        Presence::Absent
    );
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

#[test]
fn an_action_or_event_that_loses_its_output_data_breaks_thing_supplied_data() {
    let earlier = catalogue(&[("valve", VALVE)]);
    let later = catalogue(&[(
        "valve",
        &valve(|valve| {
            without(&mut valve["sdfAction"]["open"], "sdfOutputData");
            without(&mut valve["sdfEvent"]["blocked"], "sdfOutputData");
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 2);
    for affordance in ["sdfAction/open", "sdfEvent/blocked"] {
        let difference = at(
            &comparison,
            &format!("/sdfObject/Valve/{affordance}/sdfOutputData"),
        );
        assert_eq!(difference.kind(), DifferenceKind::Changed);
        assert_eq!(
            difference.subject(),
            DifferenceSubject::Quality("sdfOutputData")
        );
        assert_eq!(difference.classification(), Classification::Breaking);
        assert!(difference.reason().is_none());
        assert!(difference.witness().is_none());
        assert_eq!(
            difference.directions().collect::<Vec<_>>(),
            [Direction::ThingSupplied]
        );
        assert_eq!(
            difference.side(Revision::Later).unwrap().presence(),
            Presence::Absent
        );
    }
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

#[test]
fn json_and_human_reports_agree_on_removed_input_and_output_data() {
    let earlier = catalogue(&[("valve", VALVE)]);
    let later = catalogue(&[(
        "valve",
        &valve(|valve| {
            without(&mut valve["sdfAction"]["open"], "sdfInputData");
            without(&mut valve["sdfEvent"]["blocked"], "sdfOutputData");
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let human = comparison.to_human();
    let differences = wire["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), 2);
    let object = "/sdfObject/Valve";
    for (pointer, quality, direction, label) in [
        (
            format!("{object}/sdfAction/open/sdfInputData"),
            "sdfInputData",
            "client_supplied",
            "client-supplied",
        ),
        (
            format!("{object}/sdfEvent/blocked/sdfOutputData"),
            "sdfOutputData",
            "thing_supplied",
            "thing-supplied",
        ),
    ] {
        let difference = differences
            .iter()
            .find(|difference| difference["earlier"]["location"]["pointer"] == pointer)
            .unwrap_or_else(|| panic!("{pointer}"));
        assert_eq!(difference["kind"], "changed");
        assert_eq!(difference["subject"], "quality");
        assert_eq!(difference["quality"], quality);
        assert_eq!(difference["classification"], "breaking");
        assert_eq!(difference["directions"], serde_json::json!([direction]));
        assert!(difference.get("reason").is_none());
        assert!(difference.get("witness").is_none_or(Value::is_null));
        assert_eq!(difference["earlier"]["presence"], "present");
        assert_eq!(difference["later"]["presence"], "absent");
        assert_eq!(difference["later"]["location"]["pointer"], pointer);
        let block = format!(
            "  changed quality {quality} ({label} data)\n    earlier valve {pointer}\n    later valve {pointer} (absent)\n"
        );
        assert!(human.contains(&block), "{human}");
    }
    assert_eq!(wire["result"]["outcome"], "breaking");
    assert!(
        human.lines().next().unwrap().ends_with(": breaking"),
        "{human}"
    );
    assert!(human.contains("Differences: 2 breaking, 0 undetermined, 0 compatible\n"));
}
