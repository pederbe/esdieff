//! Unit changes in a Revision comparison, following ADR 0039: Client-side revision
//! comparison and ADR 0027: Unit names and recorded registry guidance.
use esdieff::{
    Catalogue, Classification, ComparisonRequest, Difference, DifferenceKind, DifferenceSubject,
    Direction, Disposition, Limits, Presence, RecordedRegistryPolicy, Revision, RevisionComparison,
    SourceInput, UndeterminedReason, UnitReason,
};
use serde_json::{Value, json};

use super::comparison_support;
use comparison_support::{catalogue, pointer};

const THERMOSTAT: &str = include_str!("../../examples/thermostat-check/rich.sdf.json");
const STRIP: &str = include_str!("../../examples/power-strip-check/rich.sdf.json");
const METER: &str = include_str!("../../examples/power-strip-check/definitions.sdf.json");

fn edited(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "edit target {from}");
    text.replacen(from, to, 1)
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

/// Each unit difference by Property name, with its classification and reason code.
fn unit_rows(comparison: &RevisionComparison<'_>) -> Vec<(String, Classification, &'static str)> {
    let mut rows: Vec<_> = comparison
        .differences()
        .map(|difference| {
            assert_eq!(difference.subject(), DifferenceSubject::Quality("unit"));
            assert_eq!(difference.kind(), DifferenceKind::Changed);
            let pointer = pointer(&difference, Revision::Later);
            let property = pointer.split('/').nth(4).unwrap().to_owned();
            (
                property,
                difference.classification(),
                difference.reason().map_or("", |reason| reason.code()),
            )
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows
}

/// The only difference of a comparison.
fn only<'r>(comparison: &'r RevisionComparison<'r>) -> Difference<'r> {
    assert_eq!(comparison.differences().len(), 1);
    comparison.differences().next().unwrap()
}

#[test]
fn a_change_between_current_unit_names_breaks_every_direction_the_data_carries() {
    let earlier = catalogue(&[("thermostat", THERMOSTAT)]);
    let later = catalogue(&[(
        "thermostat",
        &edited(
            THERMOSTAT,
            "\"multipleOf\": 0.5, \"unit\": \"Cel\", \"readable\": true",
            "\"multipleOf\": 0.5, \"unit\": \"K\", \"readable\": true",
        ),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    assert_eq!(comparison.outcome().disposition(), Disposition::Failed);
    let unit = only(&comparison);
    assert_eq!(unit.kind(), DifferenceKind::Changed);
    assert_eq!(unit.subject(), DifferenceSubject::Quality("unit"));
    assert_eq!(unit.classification(), Classification::Breaking);
    assert!(unit.reason().is_none());
    assert_eq!(
        unit.directions().collect::<Vec<_>>(),
        [Direction::ClientSupplied, Direction::ThingSupplied]
    );
    assert_eq!(
        pointer(&unit, Revision::Later),
        "/sdfObject/Thermostat/sdfProperty/targetTemperature/unit"
    );
}

#[test]
fn secondary_units_names_are_current_names() {
    // Watt-hours become kilowatt-hours in the shared Energy definition, so Clients misread
    // every reading of the power strip's energy counter by a factor of a thousand.
    let earlier = catalogue(&[("strip", STRIP), ("meter", METER)]);
    let meter = edited(METER, "\"unit\": \"Wh\"", "\"unit\": \"kWh\"");
    let later = catalogue(&[("strip", STRIP), ("meter", &meter)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    let unit = only(&comparison);
    assert_eq!(unit.subject(), DifferenceSubject::Quality("unit"));
    assert_eq!(unit.classification(), Classification::Breaking);
    assert_eq!(
        unit.directions().collect::<Vec<_>>(),
        [Direction::ThingSupplied]
    );
    assert_eq!(
        pointer(&unit, Revision::Later),
        "/sdfThing/PowerStrip/sdfProperty/energy/unit"
    );
    let origin = unit.side(Revision::Later).unwrap().origin().unwrap();
    let source = origin.sources().next().unwrap();
    assert_eq!(later.source(source.source_id()).unwrap().key(), "meter");
    assert_eq!(source.pointer().require().unwrap(), "/sdfData/Energy/unit");

    // A Secondary Units name and a SenML Units name are different current names too.
    let earlier = catalogue(&[("monitor", &monitor(&[("latency", Some("ms"))]))]);
    let later = catalogue(&[("monitor", &monitor(&[("latency", Some("s"))]))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(only(&comparison).classification(), Classification::Breaking);
}

#[test]
fn legacy_uri_and_unrecorded_names_and_added_or_removed_units_are_undetermined() {
    use Classification::Undetermined;
    let earlier = monitor(&[
        ("heartRate", Some("beat/min")),
        ("pulse", Some("Cel")),
        ("rate", Some("1/min")),
        ("level", Some("Cel")),
        ("ratio", Some("urn:ietf:params:unit:example%3Aratio")),
        ("temperature", Some("Cel")),
        ("energy", Some("kWh")),
        ("power", None),
        ("voltage", Some("V")),
    ]);
    let later = monitor(&[
        ("heartRate", Some("1/min")),
        ("pulse", Some("beats")),
        ("rate", Some("Cel")),
        ("level", Some("https://example.test/units/level")),
        ("ratio", Some("/")),
        ("temperature", Some("cel")),
        ("energy", Some("kwh")),
        ("power", Some("W")),
        ("voltage", None),
    ]);
    let earlier = catalogue(&[("monitor", &earlier)]);
    let later = catalogue(&[("monitor", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Undetermined);
    let row = |property: &str, code| (property.to_owned(), Undetermined, code);
    assert_eq!(
        unit_rows(&comparison),
        [
            row("energy", "comparison.unit_unrecorded"),
            row("heartRate", "comparison.unit_legacy"),
            row("level", "comparison.unit_uri_name"),
            row("power", "comparison.unit_added"),
            row("pulse", "comparison.unit_legacy"),
            row("rate", "comparison.unit_legacy"),
            row("ratio", "comparison.unit_uri_name"),
            row("temperature", "comparison.unit_unrecorded"),
            row("voltage", "comparison.unit_removed"),
        ]
    );
    let voltage = comparison
        .differences()
        .find(|difference| difference.reason().unwrap().code() == "comparison.unit_removed")
        .unwrap();
    assert_eq!(
        voltage.side(Revision::Later).unwrap().presence(),
        Presence::Absent
    );
}

#[test]
fn withheld_evidence_leaves_unit_changes_that_need_it_undetermined() {
    let earlier = monitor(&[("temperature", Some("Cel")), ("power", None)]);
    let later = monitor(&[("temperature", Some("K")), ("power", Some("W"))]);
    let earlier = catalogue(&[("monitor", &earlier)]);
    let later = catalogue(&[("monitor", &later)]);
    let comparison = ComparisonRequest::new(&earlier, &later)
        .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded)
        .compare()
        .unwrap();
    assert_eq!(comparison.outcome(), Classification::Undetermined);
    let row = |property: &str, code| (property.to_owned(), Classification::Undetermined, code);
    assert_eq!(
        unit_rows(&comparison),
        [
            // An added unit needs no recorded evidence.
            row("power", "comparison.unit_added"),
            row("temperature", "registry.caller_withheld"),
        ]
    );
    let temperature = comparison
        .differences()
        .find(|difference| {
            difference.reason() == Some(UndeterminedReason::Unit(UnitReason::EvidenceWithheld))
        })
        .unwrap();
    assert_eq!(
        pointer(&temperature, Revision::Later),
        "/sdfObject/Monitor/sdfProperty/temperature/unit"
    );
    let senml = comparison
        .registry_knowledge("iana.senml-units")
        .require()
        .unwrap();
    assert_eq!(
        (senml.state(), senml.reason()),
        ("unavailable", Some("caller_withheld"))
    );
    assert!(
        comparison
            .registry_knowledge("iana.secondary-units")
            .present()
            .is_none()
    );

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["policy"]["recorded_registry"], "withhold_recorded");
    assert_eq!(
        wire["baseline"]["registry_knowledge"],
        json!([{"registry": "iana.senml-units", "state": "unavailable", "snapshot": null,
            "reason": "caller_withheld"}])
    );
    let withheld = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| difference["reason"]["code"] == "registry.caller_withheld")
        .unwrap();
    assert_eq!(withheld["quality"], "unit");
    assert_eq!(
        withheld["reason"]["message"],
        "the request withholds recorded unit evidence"
    );
    let human = comparison.to_human();
    assert!(human.contains(
        "  changed quality unit (thing-supplied data): the request withholds recorded unit evidence\n    earlier monitor /sdfObject/Monitor/sdfProperty/temperature/unit\n"
    ));
    assert!(human.contains(
        "Registry iana.senml-units: unavailable; snapshot null; reason caller_withheld\n"
    ));
}

#[test]
fn the_report_names_the_recorded_evidence_a_unit_classification_used() {
    let earlier = catalogue(&[("thermostat", THERMOSTAT)]);
    let later = catalogue(&[(
        "thermostat",
        &edited(
            THERMOSTAT,
            "\"multipleOf\": 0.5, \"unit\": \"Cel\", \"readable\": true",
            "\"multipleOf\": 0.5, \"unit\": \"K\", \"readable\": true",
        ),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let senml = comparison
        .registry_knowledge("iana.senml-units")
        .require()
        .unwrap();
    assert_eq!(
        (senml.state(), senml.snapshot()),
        (
            "available",
            Some("iana.senml-units/2026-10-05/6246352c590e933b")
        )
    );
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["policy"]["recorded_registry"], "use_recorded");
    let records = wire["baseline"]["registry_knowledge"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["registry"], "iana.senml-units");
    assert_eq!(records[0]["state"], "available");
    assert_eq!(records[0]["entries_count"], 67);
    assert!(comparison.to_human().contains(
        "Registry iana.senml-units: available; snapshot iana.senml-units/2026-10-05/6246352c590e933b; scope senml-units; entries_count 67\n"
    ));

    // A comparison without unit lookups uses no recorded evidence.
    let unchanged = ComparisonRequest::new(&earlier, &earlier)
        .compare()
        .unwrap();
    let wire: Value = serde_json::from_str(&unchanged.to_json()).unwrap();
    assert_eq!(wire["baseline"]["registry_knowledge"], json!([]));
    assert!(!unchanged.to_human().contains("Registry"));
}

#[test]
fn a_unit_lookup_that_exhausts_the_allowance_stays_undetermined() {
    // Looking up a long spelling in every recorded row costs far more work than composing
    // and walking this small monitor.
    let spelling = "x".repeat(4000);
    let earlier = monitor(&[("temperature", Some("Cel"))]);
    let later = monitor(&[("temperature", Some(&spelling))]);
    let reason = |limits: Limits| {
        let earlier = Catalogue::with_limits(
            &[SourceInput::supplied("monitor", earlier.as_bytes())],
            limits.clone(),
        )
        .unwrap();
        let later = Catalogue::with_limits(
            &[SourceInput::supplied("monitor", later.as_bytes())],
            limits,
        )
        .unwrap();
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_eq!(comparison.outcome(), Classification::Undetermined);
        comparison
            .differences()
            .find(|difference| difference.subject() == DifferenceSubject::Quality("unit"))
            .map(|difference| difference.reason().unwrap().code())
    };
    assert_eq!(
        reason(Limits::default()),
        Some("comparison.unit_unrecorded")
    );
    assert_eq!(
        reason(Limits::default().decoding_work(200_000)),
        Some("operation.limit")
    );
}

#[test]
fn unit_rules_apply_only_to_data() {
    // A unit outside a data description is an opaque grouping quality.
    let with_unit = |unit: &str| {
        json!({"sdfObject": {"Monitor": {"unit": unit, "sdfProperty": {"level": {"type": "number"}}}}})
            .to_string()
    };
    let earlier = catalogue(&[("monitor", &with_unit("Cel"))]);
    let later = catalogue(&[("monitor", &with_unit("K"))]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let unit = only(&comparison);
    assert_eq!(unit.classification(), Classification::Undetermined);
    assert_eq!(unit.reason(), Some(UndeterminedReason::NotClassified));
}

#[test]
fn unit_changes_in_input_and_output_data_follow_their_directions() {
    let thermostat = edited(
        THERMOSTAT,
        "\"multipleOf\": 0.5, \"unit\": \"Cel\"\n",
        "\"multipleOf\": 0.5, \"unit\": \"K\"\n",
    );
    let thermostat = edited(
        &thermostat,
        "\"maximum\": 80, \"unit\": \"Cel\" },",
        "\"maximum\": 80, \"unit\": \"K\" },",
    );
    let earlier = catalogue(&[("thermostat", THERMOSTAT)]);
    let later = catalogue(&[("thermostat", &thermostat)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mut units: Vec<_> = comparison
        .differences()
        .map(|difference| {
            assert_eq!(difference.classification(), Classification::Breaking);
            (
                pointer(&difference, Revision::Later),
                difference.directions().collect::<Vec<_>>(),
            )
        })
        .collect();
    units.sort_by(|a, b| a.0.cmp(&b.0));
    let thermostat = "/sdfObject/Thermostat";
    assert_eq!(
        units,
        [
            (
                format!(
                    "{thermostat}/sdfAction/configure/sdfInputData/properties/targetTemperature/unit"
                ),
                vec![Direction::ClientSupplied]
            ),
            (
                format!("{thermostat}/sdfEvent/alarm/sdfOutputData/properties/temperature/unit"),
                vec![Direction::ThingSupplied]
            ),
        ]
    );
}
