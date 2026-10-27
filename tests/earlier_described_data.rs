//! Clients use only what the earlier revision describes, so describing a map entry or output
//! data that it left undescribed is compatible, and a removed entry description breaks only
//! Thing-supplied data. Follows ADR 0039: Client-side revision comparison.
use esdieff::{
    Catalogue, Classification, ComparisonRequest, DifferenceKind, DifferenceSubject, Direction,
    Limits, SourceInput, UndeterminedReason,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{at, catalogue, client, revised, thing, witness};

/// A sprinkler whose `plan` only Clients supply and whose `report` only the Thing supplies,
/// both as Map data requiring a zone.
const SPRINKLER: &str = r#"{"sdfObject": {"Sprinkler": {"sdfProperty": {
    "plan": {"type": "object", "readable": false, "observable": false, "writable": true,
        "required": ["zone"],
        "properties": {
            "zone": {"type": "integer", "minimum": 1, "maximum": 8},
            "minutes": {"type": "integer", "minimum": 0}}},
    "report": {"type": "object", "writable": false,
        "required": ["zone"],
        "properties": {
            "zone": {"type": "integer", "minimum": 1, "maximum": 8},
            "flow": {"type": "number"}}}
}}}}"#;

/// The sprinkler with its Properties revised.
fn sprinkler(revise: impl FnOnce(&mut Value)) -> String {
    revised(SPRINKLER, |document| {
        revise(&mut document["sdfObject"]["Sprinkler"]["sdfProperty"])
    })
}

/// Both revisions of the sprinkler, each with its own revision of its Properties.
fn sprinklers(
    earlier: impl FnOnce(&mut Value),
    later: impl FnOnce(&mut Value),
) -> (Catalogue, Catalogue) {
    (
        catalogue(&[("sprinkler", &sprinkler(earlier))]),
        catalogue(&[("sprinkler", &sprinkler(later))]),
    )
}

#[test]
fn an_optional_entry_added_to_either_direction_of_map_data_is_compatible() {
    let (earlier, later) = sprinklers(
        |_| {},
        |properties| {
            for property in ["plan", "report"] {
                properties[property]["properties"]["note"] =
                    json!({"type": "string", "maxLength": 10});
            }
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 2);
    for (property, direction) in [
        ("plan", Direction::ClientSupplied),
        ("report", Direction::ThingSupplied),
    ] {
        let note = at(&comparison, &format!("/{property}/properties/note"));
        assert_eq!(
            (note.kind(), note.subject()),
            (DifferenceKind::Added, DifferenceSubject::Entry)
        );
        assert_eq!(
            note.classification(),
            Classification::Compatible,
            "{property}"
        );
        assert!(note.reason().is_none() && note.witness().is_none());
        assert_eq!(note.directions().collect::<Vec<_>>(), [direction]);
    }
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

/// Remove an entry description from a Property's map data.
fn without(properties: &mut Value, property: &str, entry: &str) {
    let entries = properties[property]["properties"].as_object_mut().unwrap();
    entries.remove(entry);
}

#[test]
fn a_removed_entry_description_breaks_only_thing_supplied_data() {
    let (earlier, later) = sprinklers(
        |_| {},
        |properties| {
            without(properties, "plan", "minutes");
            without(properties, "report", "flow");
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 2);

    // Maps are open, so the later revision still admits every minutes a Client sends.
    let minutes = at(&comparison, "/plan/properties/minutes");
    assert_eq!(
        (minutes.kind(), minutes.subject()),
        (DifferenceKind::Removed, DifferenceSubject::Entry)
    );
    assert_eq!(minutes.classification(), Classification::Compatible);
    assert!(minutes.reason().is_none() && minutes.witness().is_none());

    // The Thing may now report a flow that is not a number, beside the zone it must report.
    let flow = at(&comparison, "/report/properties/flow");
    assert_eq!(
        (flow.kind(), flow.subject()),
        (DifferenceKind::Removed, DifferenceSubject::Entry)
    );
    assert_eq!(flow.classification(), Classification::Breaking);
    assert!(flow.reason().is_none());
    assert_eq!(witness(&flow), thing(r#"{"flow":"","zone":1}"#));
    assert_eq!(
        flow.directions().collect::<Vec<_>>(),
        [Direction::ThingSupplied]
    );
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

/// Describe a `note` in a Property's map data and require it beside the zone.
fn required_note(properties: &mut Value, property: &str) {
    let map = &mut properties[property];
    map["required"] = json!(["zone", "note"]);
    map["properties"]["note"] = json!({"type": "string"});
}

#[test]
fn an_entry_that_becomes_described_and_required_follows_the_requiredness_of_its_map() {
    let (earlier, later) = sprinklers(
        |_| {},
        |properties| {
            required_note(properties, "plan");
            required_note(properties, "report");
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 4);
    for property in ["plan", "report"] {
        let note = at(&comparison, &format!("/{property}/properties/note"));
        assert_eq!(
            note.classification(),
            Classification::Compatible,
            "{property}"
        );
    }

    // The Thing now always reports a note, which no Client of the earlier revision reads.
    let report = at(&comparison, "/report/required");
    assert_eq!(report.classification(), Classification::Compatible);
    assert!(report.reason().is_none() && report.witness().is_none());

    // A Client of the earlier revision sends no note.
    let plan = at(&comparison, "/plan/required");
    assert_eq!(plan.classification(), Classification::Breaking);
    assert_eq!(witness(&plan), client(r#"{"zone":1}"#));
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

#[test]
fn a_description_added_for_an_entry_the_earlier_revision_requires_narrows_its_map() {
    // Both revisions require a note, which only the later revision describes.
    let (earlier, later) = sprinklers(
        |properties| {
            for property in ["plan", "report"] {
                properties[property]["required"] = json!(["zone", "note"]);
            }
        },
        |properties| {
            required_note(properties, "plan");
            required_note(properties, "report");
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 2);

    // A Client of the earlier revision may send any note, such as one that is not a string.
    let plan = at(&comparison, "/plan/properties/note");
    assert_eq!(plan.kind(), DifferenceKind::Added);
    assert_eq!(plan.classification(), Classification::Breaking);
    assert_eq!(witness(&plan), client(r#"{"note":0,"zone":1}"#));

    // The Thing now reports only string notes, which Clients of the earlier revision accept.
    let report = at(&comparison, "/report/properties/note");
    assert_eq!(report.classification(), Classification::Compatible);
    assert!(report.reason().is_none() && report.witness().is_none());
    assert_eq!(comparison.outcome(), Classification::Breaking);
}

#[test]
fn an_entry_that_an_alternative_inheriting_the_entries_requires_is_used() {
    // The scheduled alternative requires a start that neither revision's alternatives describe
    // until the later revision describes it beside the choice, where both alternatives inherit
    // it.
    let mode = |entries: Value| {
        move |properties: &mut Value| {
            properties["plan"] = json!({"type": "object",
                "readable": false, "observable": false, "writable": true,
                "properties": entries,
                "sdfChoice": {
                    "scheduled": {"required": ["start"]},
                    "manual": {"required": ["zone"]}}});
        }
    };
    let zone = json!({"type": "integer"});
    let (earlier, later) = sprinklers(
        mode(json!({"zone": zone})),
        mode(json!({"zone": zone, "start": {"type": "string"}})),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let start = at(&comparison, "/plan/properties/start");
    assert_eq!(start.kind(), DifferenceKind::Added);
    assert_eq!(start.classification(), Classification::Breaking);
    assert_eq!(witness(&start), client(r#"{"start":0}"#));
}

#[test]
fn entries_that_change_in_the_safe_direction_keep_the_compatibility_proof_of_their_map() {
    // Clients may now leave out the zone, send any minutes, and still send no note.
    let (earlier, later) = sprinklers(
        |_| {},
        |properties| {
            let plan = properties["plan"].as_object_mut().unwrap();
            plan.remove("required");
            without(properties, "plan", "minutes");
            properties["plan"]["properties"]["note"] = json!({"type": "string"});
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 3);
    for suffix in [
        "/plan/required",
        "/plan/properties/minutes",
        "/plan/properties/note",
    ] {
        let difference = at(&comparison, suffix);
        assert_eq!(
            difference.classification(),
            Classification::Compatible,
            "{suffix}"
        );
        assert!(difference.reason().is_none(), "{suffix}");
    }
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn only_a_used_entry_needs_the_directions_of_its_data() {
    // A writable characteristic that is not a boolean leaves the report's directions unknown.
    let unknown = |properties: &mut Value| properties["report"]["writable"] = json!("yes");
    let (earlier, later) = sprinklers(unknown, |properties| {
        unknown(properties);
        without(properties, "report", "flow");
        properties["report"]["properties"]["note"] = json!({"type": "string"});
    });
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let flow = at(&comparison, "/report/properties/flow");
    assert_eq!(
        flow.reason().unwrap().code(),
        "comparison.directions_unavailable"
    );
    let note = at(&comparison, "/report/properties/note");
    assert_eq!(note.classification(), Classification::Compatible);
}

#[test]
fn a_removed_entry_whose_earlier_description_admits_every_candidate_stays_unproven() {
    let (earlier, later) = sprinklers(
        |properties| {
            properties["report"]["properties"]["vendor"] = json!({"description": "Any detail"});
        },
        |_| {},
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let vendor = at(&comparison, "/report/properties/vendor");
    assert_eq!(vendor.kind(), DifferenceKind::Removed);
    assert_eq!(vendor.classification(), Classification::Undetermined);
    assert!(vendor.witness().is_none());
    assert_eq!(vendor.reason(), Some(UndeterminedReason::Unproven));
    assert_eq!(comparison.outcome(), Classification::Undetermined);
}

#[test]
fn json_and_human_reports_agree_on_added_and_removed_entries() {
    let (earlier, later) = sprinklers(
        |_| {},
        |properties| {
            properties["plan"]["properties"]["note"] = json!({"type": "string"});
            without(properties, "report", "flow");
        },
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let differences = wire["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), 2);
    let properties = "/sdfObject/Sprinkler/sdfProperty";
    let flow = format!("{properties}/report/properties/flow");
    let note = format!("{properties}/plan/properties/note");
    let at_side = |side: &str, pointer: &str| {
        differences
            .iter()
            .find(|difference| difference[side]["location"]["pointer"] == pointer)
            .unwrap_or_else(|| panic!("difference at {pointer}"))
            .clone()
    };

    let removed = at_side("earlier", &flow);
    assert_eq!(
        (&removed["kind"], &removed["subject"], &removed["later"]),
        (&json!("removed"), &json!("properties_entry"), &Value::Null)
    );
    assert_eq!(removed["classification"], "breaking");
    assert_eq!(removed["directions"], json!(["thing_supplied"]));
    assert!(removed.get("reason").is_none());
    assert_eq!(
        removed["witness"],
        json!({"direction": "thing_supplied", "raw_json": r#"{"flow":"","zone":1}"#,
            "admission": {"earlier": "rejected", "later": "admitted"}})
    );

    let added = at_side("later", &note);
    assert_eq!(
        (&added["kind"], &added["subject"], &added["earlier"]),
        (&json!("added"), &json!("properties_entry"), &Value::Null)
    );
    assert_eq!(added["classification"], "compatible");
    assert_eq!(added["directions"], json!(["client_supplied"]));
    assert!(added.get("reason").is_none());
    assert!(added.get("witness").is_none_or(Value::is_null));
    assert_eq!(wire["result"]["outcome"], "breaking");

    let human = comparison.to_human();
    for expected in [
        format!(
            "  removed properties entry (thing-supplied data)\n    witness {{\"flow\":\"\",\"zone\":1}} for thing-supplied data: earlier rejected, later admitted\n    earlier sprinkler {flow}\n"
        ),
        format!("  added properties entry (client-supplied data)\n    later sprinkler {note}\n"),
    ] {
        assert!(human.contains(&expected), "{expected}\n{human}");
    }
    assert!(
        human.lines().next().unwrap().ends_with(": breaking"),
        "{human}"
    );
    assert!(human.contains("Differences: 1 breaking, 0 undetermined, 1 compatible\n"));
}

#[test]
fn a_removed_entry_waits_on_its_map_when_the_allowance_runs_out() {
    // The entries change before a long reordered enum, which the allowance can run out
    // comparing. The removed flow waits on its map to be settled, and the added note does not.
    let values: Vec<_> = (0..200).map(|value| format!("\"v{value}\"")).collect();
    let reordered: Vec<_> = values.iter().rev().cloned().collect();
    let report = |entries: &str, values: &[String]| {
        format!(
            r#"{{"sdfObject":{{"Sprinkler":{{"sdfProperty":{{"report":{{"writable":false,"properties":{{{entries}}},"enum":[{}]}}}}}}}}}}"#,
            values.join(",")
        )
    };
    let earlier = report(r#""flow":{"type":"number"}"#, &values);
    let later = report(r#""note":{"type":"string"}"#, &reordered);
    let mut cut_short = 0;
    for work in (10_000..=400_000).step_by(2_000) {
        let limits = Limits::default().decoding_work(work);
        let limited = |text: &str| {
            Catalogue::with_limits(
                &[SourceInput::supplied("sprinkler", text.as_bytes())],
                limits.clone(),
            )
        };
        let (Ok(earlier), Ok(later)) = (limited(&earlier), limited(&later)) else {
            continue;
        };
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let entry = |kind| {
            comparison
                .differences()
                .find(|difference| difference.kind() == kind)
        };
        let stopped = comparison
            .differences()
            .any(|difference| difference.subject() == DifferenceSubject::Document);
        let (Some(flow), Some(note)) =
            (entry(DifferenceKind::Removed), entry(DifferenceKind::Added))
        else {
            continue;
        };
        if stopped {
            cut_short += 1;
            let reason = flow.reason().map(|reason| reason.code());
            assert_eq!(reason, Some("operation.limit"), "work {work}");
            assert_eq!(
                note.classification(),
                Classification::Compatible,
                "work {work}"
            );
        }
    }
    assert!(cut_short > 0);
}

/// A valve whose Action and Event give no output data.
const SILENT_VALVE: &str = r#"{"sdfObject": {"Valve": {
    "sdfAction": {"open": {"sdfInputData": {"type": "integer", "minimum": 0, "maximum": 100}}},
    "sdfEvent": {"blocked": {}}
}}}"#;

/// The valve whose Action and Event give output data.
const VALVE: &str = r#"{"sdfObject": {"Valve": {
    "sdfAction": {"open": {
        "sdfInputData": {"type": "integer", "minimum": 0, "maximum": 100},
        "sdfOutputData": {"type": "boolean"}
    }},
    "sdfEvent": {"blocked": {"sdfOutputData": {"type": "string"}}}
}}}"#;

#[test]
fn output_data_added_to_an_action_or_event_that_had_none_is_compatible() {
    let earlier = catalogue(&[("valve", SILENT_VALVE)]);
    let later = catalogue(&[("valve", VALVE)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 2);
    for affordance in ["sdfAction/open", "sdfEvent/blocked"] {
        let output = at(
            &comparison,
            &format!("/sdfObject/Valve/{affordance}/sdfOutputData"),
        );
        assert_eq!(output.kind(), DifferenceKind::Changed);
        assert_eq!(
            output.subject(),
            DifferenceSubject::Quality("sdfOutputData")
        );
        assert_eq!(output.classification(), Classification::Compatible);
        assert!(output.reason().is_none() && output.witness().is_none());
        assert_eq!(
            output.directions().collect::<Vec<_>>(),
            [Direction::ThingSupplied]
        );
    }
    assert_eq!(comparison.outcome(), Classification::Compatible);

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let human = comparison.to_human();
    for difference in wire["result"]["differences"].as_array().unwrap() {
        let pointer = difference["later"]["location"]["pointer"].as_str().unwrap();
        assert_eq!(difference["quality"], "sdfOutputData");
        assert_eq!(difference["classification"], "compatible");
        assert_eq!(difference["directions"], json!(["thing_supplied"]));
        assert_eq!(difference["earlier"]["presence"], "absent");
        assert!(difference.get("reason").is_none());
        let block = format!(
            "  changed quality sdfOutputData (thing-supplied data)\n    earlier valve {pointer} (absent)\n    later valve {pointer}\n"
        );
        assert!(human.contains(&block), "{human}");
    }
    assert_eq!(wire["result"]["outcome"], "compatible");
}
