//! Revision comparison classifies corresponding groupings from the Client's side.
//! Follows ADR 0039: Client-side revision comparison.
use esdieff::{
    Catalogue, Classification, ComparisonRequest, Difference, DifferenceKind, DifferenceSubject,
    Direction, Disposition, Limits, Presence, Revision, RevisionComparison, SourceInput,
    UndeterminedReason,
};
use serde_json::{Value, json};

mod comparison_support;
use comparison_support::{catalogue, pointer, revised};

const LOCK: &str = include_str!("../examples/door-lock-check/rich.sdf.json");
const THERMOSTAT: &str = include_str!("../examples/thermostat-check/rich.sdf.json");
/// A thermostat whose calibration definitions are deliberately not supplied.
const LIMITED_THERMOSTAT: &str = include_str!("../examples/thermostat-check/limited.sdf.json");
const STRIP: &str = include_str!("../examples/power-strip-check/rich.sdf.json");
/// A second device document that imports the shared Power data when it is supplied.
const SECOND_STRIP: &str = include_str!("../examples/power-strip-check/missing.sdf.json");
const METER: &str = include_str!("../examples/power-strip-check/definitions.sdf.json");

fn limited(documents: &[(&str, &str)], limits: Limits) -> Result<Catalogue, &'static str> {
    let inputs: Vec<_> = documents
        .iter()
        .map(|(key, text)| SourceInput::supplied(key, text.as_bytes()))
        .collect();
    Catalogue::with_limits(&inputs, limits)
}

fn edited(text: &str, edits: &[(&str, &str)]) -> String {
    let mut text = text.to_owned();
    for (from, to) in edits {
        assert_eq!(text.matches(from).count(), 1, "edit target {from}");
        text = text.replacen(from, to, 1);
    }
    text
}

fn subject(difference: &Difference<'_>) -> String {
    match difference.subject() {
        DifferenceSubject::Document => "document".to_owned(),
        DifferenceSubject::Definition(class) => class.keyword().to_owned(),
        DifferenceSubject::Entry => "entry".to_owned(),
        DifferenceSubject::Quality(quality) => quality.to_owned(),
        _ => unreachable!("known subjects"),
    }
}

type Row = (Classification, DifferenceKind, String, String, String);

/// Each difference as its classification, kind, subject, and the document key and pointer
/// in the later revision when it has the difference, otherwise in the earlier one.
fn rows(comparison: &RevisionComparison<'_>) -> Vec<Row> {
    let mut rows: Vec<_> = comparison
        .differences()
        .map(|difference| {
            let revision = if difference.side(Revision::Later).is_some() {
                Revision::Later
            } else {
                Revision::Earlier
            };
            let side = difference.side(revision).unwrap();
            let key = comparison
                .catalogue(revision)
                .source(side.root_document())
                .unwrap()
                .key()
                .to_owned();
            (
                difference.classification(),
                difference.kind(),
                subject(&difference),
                key,
                pointer(&difference, revision),
            )
        })
        .collect();
    rows.sort_by(|a, b| (&a.3, &a.4).cmp(&(&b.3, &b.4)));
    rows
}

fn row(
    classification: Classification,
    kind: DifferenceKind,
    subject: &str,
    key: &str,
    pointer: &str,
) -> Row {
    (
        classification,
        kind,
        subject.to_owned(),
        key.to_owned(),
        pointer.to_owned(),
    )
}

#[test]
fn documentary_changes_are_compatible() {
    let earlier = catalogue(&[("thermostat", THERMOSTAT)]);
    let later = catalogue(&[(
        "thermostat",
        &edited(
            THERMOSTAT,
            &[
                (
                    "Requested room temperature in half-degree increments",
                    "Target room temperature in half-degree steps",
                ),
                (
                    r#""mode": { "type": "string""#,
                    r#""mode": { "label": "Heating mode", "$comment": "Cooling is optional", "default": "heat", "type": "string""#,
                ),
            ],
        ),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    assert_eq!(comparison.outcome().disposition(), Disposition::Passed);
    let mut changed: Vec<_> = comparison
        .differences()
        .map(|difference| {
            assert_eq!(difference.kind(), DifferenceKind::Changed);
            assert_eq!(difference.classification(), Classification::Compatible);
            assert!(difference.reason().is_none());
            pointer(&difference, Revision::Later)
        })
        .collect();
    changed.sort();
    let mode = "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/mode";
    assert_eq!(
        changed,
        [
            format!("{mode}/$comment"),
            format!("{mode}/default"),
            format!("{mode}/label"),
            "/sdfObject/Thermostat/sdfProperty/targetTemperature/description".to_owned(),
        ]
    );
}

#[test]
fn removed_groupings_and_affordances_break_and_added_ones_do_not() {
    use Classification::{Breaking, Compatible};
    use DifferenceKind::{Added, Removed};
    let earlier = catalogue(&[
        ("thermostat", THERMOSTAT),
        ("strip", STRIP),
        ("meter", METER),
    ]);
    let thermostat = revised(THERMOSTAT, |document| {
        let thermostat = &mut document["sdfObject"]["Thermostat"];
        thermostat.as_object_mut().unwrap().remove("sdfEvent");
        let actions = thermostat["sdfAction"].as_object_mut().unwrap();
        let configure = actions.remove("configure").unwrap();
        actions.insert("setup".to_owned(), configure);
        thermostat["sdfProperty"]["humidity"] = json!({"type": "number", "writable": false});
    });
    let strip = revised(STRIP, |document| {
        document["sdfThing"]["PowerStrip"]["sdfObject"] =
            json!({"display": {"sdfProperty": {"brightness": {"type": "integer"}}}});
    });
    let meter = revised(METER, |document| {
        document["sdfObject"]
            .as_object_mut()
            .unwrap()
            .remove("RequiredOutlet");
    });
    let later = catalogue(&[
        ("thermostat", &thermostat),
        ("strip", &strip),
        ("meter", &meter),
    ]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Breaking);
    assert_eq!(comparison.outcome().disposition(), Disposition::Failed);
    assert_eq!(comparison.count(Breaking), 4);
    assert_eq!(comparison.count(Compatible), 3);
    let thing = "/sdfThing/PowerStrip/sdfObject";
    let object = "/sdfObject/Thermostat";
    assert_eq!(
        rows(&comparison),
        [
            row(
                Breaking,
                Removed,
                "sdfObject",
                "meter",
                "/sdfObject/RequiredOutlet"
            ),
            row(
                Compatible,
                Added,
                "sdfObject",
                "strip",
                &format!("{thing}/display")
            ),
            row(
                Breaking,
                Removed,
                "sdfObject",
                "strip",
                &format!("{thing}/outlet")
            ),
            row(
                Breaking,
                Removed,
                "sdfAction",
                "thermostat",
                &format!("{object}/sdfAction/configure")
            ),
            row(
                Compatible,
                Added,
                "sdfAction",
                "thermostat",
                &format!("{object}/sdfAction/setup")
            ),
            row(
                Breaking,
                Removed,
                "sdfEvent",
                "thermostat",
                &format!("{object}/sdfEvent/alarm")
            ),
            row(
                Compatible,
                Added,
                "sdfProperty",
                "thermostat",
                &format!("{object}/sdfProperty/humidity")
            ),
        ]
    );
    for difference in comparison.differences() {
        let (present, missing) = if difference.kind() == Removed {
            (Revision::Earlier, Revision::Later)
        } else {
            (Revision::Later, Revision::Earlier)
        };
        assert!(difference.side(missing).is_none());
        assert_eq!(difference.directions().count(), 0);
        let origin = difference.side(present).unwrap().origin().unwrap();
        assert!(origin.sources().next().is_some());
    }
}

/// The document key and pointer of a side's nearest content origin.
fn origin(
    comparison: &RevisionComparison<'_>,
    difference: &Difference<'_>,
    revision: Revision,
) -> (String, String) {
    let origin = difference.side(revision).unwrap().origin().unwrap();
    let source = origin.sources().next().unwrap();
    let key = comparison
        .catalogue(revision)
        .source(source.source_id())
        .unwrap()
        .key()
        .to_owned();
    (key, source.pointer(8192).require().unwrap())
}

#[test]
fn a_changed_shared_definition_is_reported_at_every_grouping_that_uses_it() {
    use Classification::{Breaking, Compatible};
    use DifferenceKind::Changed;
    let earlier = catalogue(&[("strip", STRIP), ("second", SECOND_STRIP), ("meter", METER)]);
    let meter = revised(METER, |document| {
        document["sdfData"]["Power"]["minimum"] = json!(10);
        document["sdfObject"]["Outlet"]["sdfProperty"]["power"]["maximum"] = json!(3000);
    });
    let later = catalogue(&[
        ("strip", STRIP),
        ("second", SECOND_STRIP),
        ("meter", &meter),
    ]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let total = "/sdfThing/PowerStrip/sdfProperty/totalPower/minimum";
    let outlet = "sdfObject/Outlet/sdfProperty/power/maximum";
    assert_eq!(
        rows(&comparison),
        [
            row(
                Compatible,
                Changed,
                "maximum",
                "meter",
                &format!("/{outlet}")
            ),
            // Only the second strip's total power is also written by Clients.
            row(Breaking, Changed, "minimum", "second", total),
            row(
                Compatible,
                Changed,
                "maximum",
                "strip",
                "/sdfThing/PowerStrip/sdfObject/outlet/sdfProperty/power/maximum"
            ),
            row(Compatible, Changed, "minimum", "strip", total),
        ]
    );
    for difference in comparison.differences() {
        let shared = if difference.subject() == DifferenceSubject::Quality("minimum") {
            "/sdfData/Power/minimum".to_owned()
        } else {
            format!("/{outlet}")
        };
        for revision in [Revision::Earlier, Revision::Later] {
            assert_eq!(
                origin(&comparison, &difference, revision),
                ("meter".to_owned(), shared.clone())
            );
            assert_eq!(
                difference.side(revision).unwrap().presence(),
                Presence::Present
            );
        }
    }
    assert!(comparison.to_human().contains(&format!(
        "    earlier second {total}\n      from meter /sdfData/Power/minimum\n    later second {total}\n      from meter /sdfData/Power/minimum\n"
    )));
}

#[test]
fn data_differences_name_the_client_directions_both_revisions_offer() {
    use Direction::{ClientSupplied, ThingSupplied};
    let with_code = |maximum: u32| {
        revised(LOCK, |document| {
            document["sdfObject"]["Lock"]["sdfProperty"]["code"] = json!({
                "type": "integer", "maximum": maximum,
                "readable": false, "observable": false, "writable": true
            });
        })
    };
    let earlier_lock = with_code(9999);
    let later_lock = revised(&with_code(99999), |document| {
        let lock = &mut document["sdfObject"]["Lock"];
        lock["sdfProperty"]["credential"]["maxLength"] = json!(12);
        lock["sdfProperty"]["state"]["description"] = json!("Bolt position");
        let authorize = &mut lock["sdfAction"]["authorize"];
        authorize["sdfInputData"]["properties"]["credential"]["maxLength"] = json!(12);
        authorize["sdfOutputData"]["required"] = json!(["accepted"]);
        lock["sdfEvent"]["stateChanged"]["sdfOutputData"]["properties"]["at"]["maxLength"] =
            json!(40);
    });
    let earlier_thermostat = THERMOSTAT;
    let later_thermostat = edited(
        THERMOSTAT,
        &[(
            "\"multipleOf\": 0.5, \"unit\": \"Cel\", \"readable\": true, \"writable\": true",
            "\"multipleOf\": 0.25, \"unit\": \"Cel\", \"readable\": true, \"writable\": false",
        )],
    );
    let earlier = catalogue(&[("lock", &earlier_lock), ("thermostat", earlier_thermostat)]);
    let later = catalogue(&[("lock", &later_lock), ("thermostat", &later_thermostat)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mut directions: Vec<_> = comparison
        .differences()
        .map(|difference| {
            (
                pointer(&difference, Revision::Later),
                difference.directions().collect::<Vec<_>>(),
            )
        })
        .collect();
    directions.sort_by(|a, b| a.0.cmp(&b.0));
    let lock = "/sdfObject/Lock";
    let target = "/sdfObject/Thermostat/sdfProperty/targetTemperature";
    assert_eq!(
        directions,
        [
            (
                format!("{lock}/sdfAction/authorize/sdfInputData/properties/credential/maxLength"),
                vec![ClientSupplied]
            ),
            (
                format!("{lock}/sdfAction/authorize/sdfOutputData/required"),
                vec![ThingSupplied]
            ),
            (
                format!("{lock}/sdfEvent/stateChanged/sdfOutputData/properties/at/maxLength"),
                vec![ThingSupplied]
            ),
            (
                format!("{lock}/sdfProperty/code/maximum"),
                vec![ClientSupplied]
            ),
            (
                format!("{lock}/sdfProperty/credential/maxLength"),
                vec![ClientSupplied, ThingSupplied]
            ),
            (
                format!("{lock}/sdfProperty/state/description"),
                vec![ThingSupplied]
            ),
            (format!("{target}/multipleOf"), vec![ThingSupplied]),
            (format!("{target}/writable"), vec![ClientSupplied]),
        ]
    );
    let state = comparison
        .differences()
        .find(|difference| difference.subject() == DifferenceSubject::Quality("description"))
        .unwrap();
    assert_eq!(state.classification(), Classification::Compatible);
    assert!(state.affects(ThingSupplied) && !state.affects(ClientSupplied));
}

/// Lock credential pattern and state description changes, with the thermostat moved to a
/// new key.
fn mixed_revisions() -> (Catalogue, Catalogue) {
    mixed_revisions_with(Limits::default())
}
fn mixed_revisions_with(limits: Limits) -> (Catalogue, Catalogue) {
    let earlier = limited(
        &[("lock", LOCK), ("thermostat", THERMOSTAT)],
        limits.clone(),
    )
    .unwrap();
    let lock = revised(LOCK, |document| {
        let lock = &mut document["sdfObject"]["Lock"]["sdfProperty"];
        // Every representative credential matches both patterns, so the change stays unproven.
        lock["credential"]["pattern"] = json!("^[a-z]+$");
        lock["state"]["description"] = json!("Bolt position");
    });
    let later = limited(&[("lock", &lock), ("moved/thermostat", THERMOSTAT)], limits).unwrap();
    (earlier, later)
}

#[test]
fn the_json_report_agrees_with_the_typed_differences() {
    let (earlier, later) = mixed_revisions();
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["schema"], "esdieff.report");
    assert_eq!(wire["version"], "1.5");
    assert_eq!(wire["operation"], "diff");
    assert_eq!(
        wire["request"]["comparison"],
        "esdieff.revision-comparison/2"
    );
    assert_eq!(comparison.contract(), "esdieff.revision-comparison/2");
    assert_eq!(wire["result"]["kind"], "diff");
    assert_eq!(wire["result"]["outcome"], "breaking");
    assert_eq!(wire["outcome"]["disposition"], "failed");
    assert_eq!(wire["outcome"]["complete"], false);
    assert_eq!(wire["outcome"]["errors"], 1);
    assert_eq!(wire["outcome"]["incomplete_checks"], 1);
    assert_eq!(
        wire["result"]["summary"],
        json!({"breaking": 1, "undetermined": 1, "compatible": 2,
            "acknowledged": {"breaking": 0, "undetermined": 0},
            "documents": {"compared": 1, "removed": 1, "added": 1}})
    );
    assert_eq!(
        wire["result"]["documents"],
        json!({"compared": [{"earlier": 1, "later": 1}], "removed": [2], "added": [2]})
    );
    assert_eq!(comparison.compared_documents().collect::<Vec<_>>().len(), 1);
    assert_eq!(comparison.removed_documents().len(), 1);
    assert_eq!(comparison.added_documents().len(), 1);

    // Both revisions number their sources from 1, so each source names its revision.
    let sources: Vec<_> = wire["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| {
            (
                source["revision"].as_str().unwrap(),
                source["id"].as_u64().unwrap(),
                source["key"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        sources,
        [
            ("earlier", 1, "lock"),
            ("earlier", 2, "thermostat"),
            ("later", 1, "lock"),
            ("later", 2, "moved/thermostat"),
        ]
    );
    assert_eq!(
        wire["request"]["scope"]["earlier"]["assessed"],
        json!([1, 2])
    );
    assert_eq!(wire["request"]["scope"]["later"]["assessed"], json!([1, 2]));

    let differences = wire["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), comparison.differences().len());
    assert_eq!(wire["result"]["differences_omitted"], 0);
    for (typed, wire) in comparison.differences().zip(differences) {
        assert_eq!(wire["kind"], typed.kind().as_str());
        assert_eq!(wire["classification"], typed.classification().as_str());
        let directions: Vec<_> = typed
            .directions()
            .map(|direction| direction.as_str())
            .collect();
        assert_eq!(wire["directions"], json!(directions));
        match typed.subject() {
            DifferenceSubject::Quality(quality) => {
                assert_eq!(wire["subject"], "quality");
                assert_eq!(wire["quality"], quality);
            }
            DifferenceSubject::Definition(class) => assert_eq!(wire["subject"], class.keyword()),
            _ => unreachable!("lock and thermostat differences"),
        }
        match typed.reason() {
            Some(reason) => {
                assert_eq!(wire["reason"]["code"], reason.code());
                assert_eq!(wire["reason"]["message"], reason.message());
            }
            None => assert!(wire.get("reason").is_none()),
        }
        for revision in [Revision::Earlier, Revision::Later] {
            let side = &wire[revision.as_str()];
            let Some(typed_side) = typed.side(revision) else {
                assert!(side.is_null());
                continue;
            };
            assert_eq!(side["presence"], typed_side.presence().as_str());
            assert_eq!(side["location"]["kind"], "effective");
            assert_eq!(side["location"]["root"], typed_side.root_document().get());
            assert_eq!(side["location"]["pointer"], pointer(&typed, revision));
            let Some(origin) = typed_side.origin() else {
                assert_eq!(typed_side.presence(), Presence::Absent);
                assert!(side["origin"].is_null());
                continue;
            };
            let source = origin.sources().next().unwrap();
            assert_eq!(side["origin"]["kind"], "source");
            assert_eq!(side["origin"]["source_id"], source.source_id().get());
            assert_eq!(
                side["origin"]["pointer"],
                source.pointer(8192).require().unwrap()
            );
            let span = source.span();
            assert_eq!(side["origin"]["span"], json!([span.start, span.end]));
        }
    }
    let order: Vec<_> = differences
        .iter()
        .map(|difference| difference["classification"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        ["breaking", "undetermined", "compatible", "compatible"]
    );
}

#[test]
fn the_human_report_orders_breaking_undetermined_and_compatible_differences() {
    let (earlier, later) = mixed_revisions();
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let human = comparison.to_human();
    let position = |text: &str| {
        human
            .find(text)
            .unwrap_or_else(|| panic!("{text} in {human}"))
    };
    assert!(human.starts_with("Revision comparison esdieff.revision-comparison/2: breaking\n"));
    assert!(human.contains("Documents: 1 compared, 1 removed, 1 added\n"));
    assert!(human.contains("Removed documents: thermostat\n"));
    assert!(human.contains("Added documents: moved/thermostat\n"));
    assert!(human.contains("Differences: 1 breaking, 1 undetermined, 2 compatible\n"));
    assert!(
        position("Breaking differences:") < position("Undetermined differences:")
            && position("Undetermined differences:") < position("Compatible differences:")
    );
    assert!(human.contains("  removed sdfObject\n    earlier thermostat /sdfObject/Thermostat\n"));
    assert!(human.contains(
        "  changed quality pattern (client-supplied and thing-supplied data): no breaking witness or compatibility proof\n    earlier lock /sdfObject/Lock/sdfProperty/credential/pattern\n    later lock /sdfObject/Lock/sdfProperty/credential/pattern\n"
    ));
    assert!(
        human.contains("    earlier lock /sdfObject/Lock/sdfProperty/state/description (absent)\n")
    );
    assert!(
        human.contains("  added sdfObject\n    later moved/thermostat /sdfObject/Thermostat\n")
    );
    let styled = comparison.to_human_with_style(esdieff::HumanStyle::Ansi);
    assert!(styled.contains("\x1b[31mbreaking\x1b[0m"));
    assert!(styled.contains("\x1b[31mthermostat\x1b[0m"));
    assert!(styled.contains("\x1b[32mmoved/thermostat\x1b[0m"));
}

#[test]
fn unavailable_effective_content_leaves_its_area_undetermined() {
    use Classification::{Breaking, Undetermined};
    use DifferenceKind::{Removed, Unavailable};
    let earlier = catalogue(&[("thermostat", LIMITED_THERMOSTAT), ("lock", LOCK)]);
    let thermostat = revised(LIMITED_THERMOSTAT, |document| {
        document["sdfObject"]["Thermostat"]["sdfProperty"]
            .as_object_mut()
            .unwrap()
            .remove("brokenTemperature");
    });
    let later = catalogue(&[("thermostat", &thermostat), ("lock", "{\"sdfObject\": ")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Breaking);
    let configure = "/sdfObject/Thermostat/sdfAction/configure/sdfInputData/properties/calibration";
    assert_eq!(
        rows(&comparison),
        [
            row(Undetermined, Unavailable, "sdfObject", "lock", ""),
            row(Undetermined, Unavailable, "entry", "thermostat", configure),
            row(
                Breaking,
                Removed,
                "sdfProperty",
                "thermostat",
                "/sdfObject/Thermostat/sdfProperty/brokenTemperature"
            ),
            row(
                Undetermined,
                Unavailable,
                "sdfProperty",
                "thermostat",
                "/sdfObject/Thermostat/sdfProperty/currentTemperature"
            ),
        ]
    );
    for difference in comparison.differences() {
        if difference.kind() != Unavailable {
            continue;
        }
        let reason = difference.reason().unwrap();
        if difference.subject() == DifferenceSubject::Definition(esdieff::Class::Object) {
            assert_eq!(reason.code(), "json.syntax");
            assert_eq!(
                difference.side(Revision::Earlier).unwrap().presence(),
                Presence::Present
            );
            assert_eq!(
                difference.side(Revision::Later).unwrap().presence(),
                Presence::Unavailable
            );
        } else {
            assert_eq!(reason.code(), "reference.unavailable");
            for revision in [Revision::Earlier, Revision::Later] {
                assert_eq!(
                    difference.side(revision).unwrap().presence(),
                    Presence::Unavailable
                );
            }
        }
    }
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(
        wire["request"]["scope"]["later"]["unavailable_required"],
        json!([2])
    );
}

#[test]
fn both_revisions_share_one_limits_configuration() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = limited(&[("lock", LOCK)], Limits::default().parsed_nodes(1000)).unwrap();
    assert_eq!(
        ComparisonRequest::new(&earlier, &later).compare().err(),
        Some("revisions must share one limits configuration")
    );
}

#[test]
fn one_allowance_covers_both_revisions_acquisition() {
    // Each revision's lock parses into 81 nodes. One revision fits 120, but not both.
    let limits = Limits::default().parsed_nodes(120);
    let earlier = limited(&[("lock", LOCK)], limits.clone()).unwrap();
    let removed = revised(LOCK, |document| {
        document["sdfObject"]["Lock"]
            .as_object_mut()
            .unwrap()
            .remove("sdfEvent");
    });
    let later = limited(&[("lock", &removed)], limits).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Undetermined);
    let differences: Vec<_> = comparison.differences().collect();
    assert_eq!(differences.len(), 1);
    let lock = &differences[0];
    assert_eq!(lock.kind(), DifferenceKind::Unavailable);
    assert_eq!(
        lock.subject(),
        DifferenceSubject::Definition(esdieff::Class::Object)
    );
    assert_eq!(lock.reason().unwrap().code(), "operation.limit");
    assert_eq!(
        lock.side(Revision::Earlier).unwrap().presence(),
        Presence::Present
    );
    assert_eq!(
        lock.side(Revision::Later).unwrap().presence(),
        Presence::Unavailable
    );
}

#[test]
fn an_exhausted_allowance_never_reads_as_compatible() {
    let removed = revised(LOCK, |document| {
        document["sdfObject"]["Lock"]
            .as_object_mut()
            .unwrap()
            .remove("sdfEvent");
    });
    let (mut limited_work, mut breaking_kept) = (false, false);
    for work in (4000..=30000).step_by(500) {
        let limits = Limits::default().decoding_work(work);
        let (Ok(earlier), Ok(later)) = (
            limited(&[("lock", LOCK)], limits.clone()),
            limited(&[("lock", &removed)], limits),
        ) else {
            continue;
        };
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_ne!(
            comparison.outcome(),
            Classification::Compatible,
            "work {work}"
        );
        let limit = comparison.differences().any(|difference| {
            difference
                .reason()
                .is_some_and(|reason| reason.code() == "operation.limit")
        });
        limited_work |= limit;
        breaking_kept |= limit && comparison.outcome() == Classification::Breaking;
        let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
        for classification in [
            Classification::Breaking,
            Classification::Undetermined,
            Classification::Compatible,
        ] {
            assert_eq!(
                wire["result"]["summary"][classification.as_str()],
                comparison.count(classification)
            );
        }
    }
    assert!(limited_work && breaking_kept);
}

#[test]
fn report_truncation_keeps_counts_and_the_outcome() {
    let (earlier, later) = mixed_revisions_with(Limits::default().diagnostic_bytes(1000));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 4);
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let shown = wire["result"]["differences"].as_array().unwrap();
    assert!(!shown.is_empty() && shown.len() < 4);
    assert_eq!(shown[0]["classification"], "breaking");
    let omitted = 4 - shown.len();
    assert_eq!(wire["result"]["differences_omitted"], omitted);
    assert_eq!(
        wire["result"]["omissions"],
        json!([{"field": "result.differences", "reason": "byte_limit", "omitted_count": omitted}])
    );
    assert_eq!(wire["diagnostics"]["truncated"], true);
    assert_eq!(wire["diagnostics"]["reasons"], json!(["byte_limit"]));
    assert_eq!(wire["result"]["outcome"], "breaking");
    assert_eq!(wire["outcome"]["disposition"], "failed");
    assert_eq!(
        wire["result"]["summary"],
        json!({"breaking": 1, "undetermined": 1, "compatible": 2,
            "acknowledged": {"breaking": 0, "undetermined": 0},
            "documents": {"compared": 1, "removed": 1, "added": 1}})
    );
    let human = comparison.to_human();
    assert!(human.contains("Differences: 1 breaking, 1 undetermined, 2 compatible\n"));
    assert!(human.contains(&format!("Difference details omitted: {omitted}\n")));
    // Every source survives, and the human report lists the differences the JSON kept.
    assert_eq!(wire["sources"].as_array().unwrap().len(), 4);
    let listed = human
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
        .count();
    assert_eq!(listed, shown.len());
}

#[test]
fn a_report_allowance_too_small_for_both_mandatory_envelopes_fails_before_comparing() {
    // Each one-document revision reserves 20504 bytes for its mandatory envelope.
    let removed = revised(LOCK, |document| {
        document["sdfObject"]["Lock"]
            .as_object_mut()
            .unwrap()
            .remove("sdfEvent");
    });
    for limit in [41007, 41008] {
        let limits = Limits::default().report_bytes(limit);
        let earlier = limited(&[("lock", LOCK)], limits.clone()).unwrap();
        let later = limited(&[("lock", &removed)], limits).unwrap();
        let comparison = ComparisonRequest::new(&earlier, &later).compare();
        if limit == 41007 {
            assert_eq!(
                comparison.err(),
                Some("report cannot retain its mandatory envelope")
            );
            continue;
        }
        let comparison = comparison.unwrap();
        let (json, human) = (comparison.to_json(), comparison.to_human());
        assert!(json.len() <= limit && human.len() <= limit);
        let wire: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["outcome"]["disposition"], "failed");
        assert_eq!(wire["result"]["outcome"], "breaking");
        assert_eq!(wire["result"]["summary"]["breaking"], 1);
        assert_eq!(wire["result"]["differences"], json!([]));
        assert_eq!(wire["result"]["differences_omitted"], 1);
        assert_eq!(wire["sources"].as_array().unwrap().len(), 2);
        assert!(human.contains("Differences: 1 breaking, 0 undetermined, 0 compatible\n"));
        assert!(human.ends_with("Difference details omitted: 1\n"));
    }
}

#[test]
fn a_human_report_longer_than_the_report_allowance_gives_the_json_report() {
    // Long document keys make each human row, which names its documents, longer than its
    // JSON record, which numbers them.
    let key = "k".repeat(256);
    let revision = |description: &str| {
        let mut document = json!({"info": {}, "sdfObject": {"Meter": {"sdfProperty": {}}}});
        for index in 0..200 {
            let name = format!("Reading{index}");
            document["sdfData"][&name] = json!({"type": "number", "description": description});
            document["sdfObject"]["Meter"]["sdfProperty"][&name] =
                json!({"sdfRef": format!("#/sdfData/{name}")});
        }
        document.to_string()
    };
    let limit = 150_000;
    let limits = Limits::default()
        .report_bytes(limit)
        .diagnostic_bytes(limit);
    let earlier = limited(&[(&key, &revision("Reading"))], limits.clone()).unwrap();
    let later = limited(&[(&key, &revision("Meter reading"))], limits).unwrap();
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.count(Classification::Compatible), 200);
    let json = comparison.to_json();
    assert!(json.len() <= limit);
    let human = comparison.to_human();
    assert!(human == json, "human report of {} bytes", human.len());
}

#[test]
fn limited_pointer_presentation_keeps_each_difference() {
    let (earlier, later) = mixed_revisions_with(Limits::default().pointer_bytes(24));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let differences = wire["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), 4);
    let pattern = &differences[1];
    assert_eq!(pattern["quality"], "pattern");
    for revision in ["earlier", "later"] {
        let location = &pattern[revision]["location"];
        assert!(location["pointer"].is_null());
        assert_eq!(location["omission"]["reason"], "location_limit");
        assert_eq!(location["omission"]["revision"], revision);
        assert!(pattern[revision]["origin"]["pointer"].is_null());
    }
    // The thermostat's short pointers still fit.
    assert_eq!(
        differences[0]["earlier"]["location"]["pointer"],
        "/sdfObject/Thermostat"
    );
    let omissions = wire["diagnostics"]["omissions"].as_array().unwrap();
    assert_eq!(
        omissions,
        &[
            json!({"field": "result.differences.location.pointer", "reason": "location_limit", "omitted_count": 4}),
            json!({"field": "result.differences.origin.pointer", "reason": "location_limit", "omitted_count": 3}),
        ]
    );
    assert_eq!(wire["diagnostics"]["reasons"], json!(["location_limit"]));
    assert_eq!(wire["diagnostics"]["truncated"], true);
    assert!(
        comparison
            .to_human()
            .contains("    earlier lock (pointer omitted)\n")
    );
}

#[test]
fn equal_numbers_reordered_sets_and_specified_defaults_report_no_change() {
    let earlier = catalogue(&[("thermostat", THERMOSTAT)]);
    let later = catalogue(&[(
        "thermostat",
        &edited(
            THERMOSTAT,
            &[
                (
                    "\"minimum\": -40, \"maximum\": 80,\n",
                    "\"minimum\": -4e1, \"maximum\": 8.0E1,\n",
                ),
                (r#"["off", "heat", "cool"]"#, r#"["cool", "off", "heat"]"#),
                (
                    r#""required": ["targetTemperature", "mode"]"#,
                    r#""required": ["mode", "targetTemperature"]"#,
                ),
                // Specified defaults equal their explicit values.
                (
                    r#""type": "number", "nullable": true, "minimum": -4e1"#,
                    r#""type": "number", "minimum": -4e1"#,
                ),
                (
                    "\"multipleOf\": 0.5, \"unit\": \"Cel\", \"readable\": true, \"writable\": true",
                    "\"multipleOf\": 0.5, \"unit\": \"Cel\", \"writable\": true, \"observable\": true",
                ),
            ],
        ),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 0);
    assert_eq!(comparison.outcome(), Classification::Compatible);
}

#[test]
fn changed_unclassified_qualities_are_undetermined() {
    use Classification::Undetermined;
    use DifferenceKind::Changed;
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[(
        "lock",
        &revised(LOCK, |document| {
            // Sample-value checking checks neither format, so neither is classified.
            let lock = &mut document["sdfObject"]["Lock"];
            lock["sdfProperty"]["credential"]["format"] = json!("email");
            lock["sdfAction"]["authorize"]["sdfInputData"]["properties"]["credential"]["format"] =
                json!("email");
            lock["sdfEvent"]["stateChanged"]["sdfOutputData"]["properties"]["at"]["format"] =
                json!("email");
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Undetermined);
    assert_eq!(
        comparison.outcome().disposition(),
        Disposition::Undetermined
    );
    let lock = "/sdfObject/Lock";
    assert_eq!(
        rows(&comparison),
        [
            row(
                Undetermined,
                Changed,
                "format",
                "lock",
                &format!("{lock}/sdfAction/authorize/sdfInputData/properties/credential/format")
            ),
            row(
                Undetermined,
                Changed,
                "format",
                "lock",
                &format!("{lock}/sdfEvent/stateChanged/sdfOutputData/properties/at/format")
            ),
            row(
                Undetermined,
                Changed,
                "format",
                "lock",
                &format!("{lock}/sdfProperty/credential/format")
            ),
        ]
    );
    for difference in comparison.differences() {
        assert_eq!(difference.reason(), Some(UndeterminedReason::NotClassified));
        assert_eq!(
            difference.reason().unwrap().code(),
            "comparison.not_classified"
        );
    }
    // Only the event timestamp had a format before.
    for difference in comparison.differences() {
        let earlier = if pointer(&difference, Revision::Later).ends_with("/at/format") {
            Presence::Present
        } else {
            Presence::Absent
        };
        let presence = |revision| difference.side(revision).unwrap().presence();
        assert_eq!(presence(Revision::Earlier), earlier);
        assert_eq!(presence(Revision::Later), Presence::Present);
    }
}

#[test]
fn a_breaking_difference_decides_the_outcome_over_undetermined_ones() {
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[(
        "lock",
        &revised(LOCK, |document| {
            let lock = &mut document["sdfObject"]["Lock"];
            lock["sdfProperty"]["credential"]["pattern"] = json!("^[a-z]+$");
            lock.as_object_mut().unwrap().remove("sdfEvent");
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.count(Classification::Breaking), 1);
    assert_eq!(comparison.count(Classification::Undetermined), 1);
    assert_eq!(comparison.outcome(), Classification::Breaking);
    assert_eq!(comparison.outcome().disposition(), Disposition::Failed);
    let classifications: Vec<_> = comparison
        .differences()
        .map(|difference| difference.classification())
        .collect();
    assert_eq!(
        classifications,
        [Classification::Breaking, Classification::Undetermined]
    );
}

#[test]
fn a_malformed_document_leaves_its_definitions_undetermined_rather_than_removed() {
    let lock = revised(LOCK, |document| {
        document["sdfObject"]["Keypad"] = json!({"sdfProperty": {"code": {"type": "string"}}});
    });
    let readable = catalogue(&[("lock", &lock)]);
    let malformed = catalogue(&[("lock", "{\"sdfObject\": ")]);
    for (earlier, later, failed, read) in [
        (&readable, &malformed, Revision::Later, Revision::Earlier),
        (&malformed, &readable, Revision::Earlier, Revision::Later),
    ] {
        let comparison = ComparisonRequest::new(earlier, later).compare().unwrap();
        assert_eq!(comparison.outcome(), Classification::Undetermined);
        let mut definitions: Vec<_> = comparison
            .differences()
            .map(|difference| {
                assert_eq!(difference.kind(), DifferenceKind::Unavailable);
                assert_eq!(
                    difference.subject(),
                    DifferenceSubject::Definition(esdieff::Class::Object)
                );
                assert_eq!(difference.reason().unwrap().code(), "json.syntax");
                assert_eq!(
                    difference.side(failed).unwrap().presence(),
                    Presence::Unavailable
                );
                assert_eq!(difference.side(read).unwrap().presence(), Presence::Present);
                pointer(&difference, read)
            })
            .collect();
        definitions.sort();
        assert_eq!(definitions, ["/sdfObject/Keypad", "/sdfObject/Lock"]);

        // The malformed document is an acquisition result on its own side.
        let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
        for source in wire["sources"].as_array().unwrap() {
            let decode = if source["revision"] == failed.as_str() {
                "rejected"
            } else {
                "usable"
            };
            assert_eq!(source["decode"], decode);
        }
        assert_eq!(
            wire["request"]["scope"][failed.as_str()]["unavailable_required"],
            json!([1])
        );
        assert_eq!(
            wire["result"]["documents"],
            json!({"compared": [{"earlier": 1, "later": 1}], "removed": [], "added": []})
        );
    }
}

#[test]
fn content_depending_on_an_unknown_critical_feature_is_undetermined() {
    let declared = revised(LOCK, |document| {
        document["info"]["features"] = json!(["acme-locking"]);
    });
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[("lock", &declared)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Undetermined);
    let lock: Vec<_> = comparison.differences().collect();
    assert_eq!(lock.len(), 1);
    assert_eq!(lock[0].kind(), DifferenceKind::Unavailable);
    assert_eq!(
        lock[0].reason().unwrap().code(),
        "structure.feature_context_unavailable"
    );
    assert_eq!(pointer(&lock[0], Revision::Earlier), "/sdfObject/Lock");
    assert_eq!(
        lock[0].side(Revision::Later).unwrap().presence(),
        Presence::Unavailable
    );

    // Only the Property importing the deferred contributor is undetermined. An independent
    // removal in the same Object stays breaking and decides the outcome.
    let meter = |features: Value| {
        json!({"info": {"features": features},
            "namespace": {"meter": "https://example.test/meter"}, "defaultNamespace": "meter",
            "sdfData": {"Power": {"type": "number", "minimum": 0}}})
        .to_string()
    };
    let outlet = |properties: Value| {
        json!({"info": {}, "namespace": {"meter": "https://example.test/meter"},
            "sdfObject": {"Outlet": {"sdfProperty": properties}}})
        .to_string()
    };
    let power = json!({"sdfRef": "meter:#/sdfData/Power"});
    let earlier = catalogue(&[
        (
            "outlet",
            &outlet(json!({"power": power, "on": {"type": "boolean"}})),
        ),
        ("meter", &meter(json!([]))),
    ]);
    let later = catalogue(&[
        ("outlet", &outlet(json!({"power": power}))),
        ("meter", &meter(json!(["acme-metering"]))),
    ]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    use Classification::{Breaking, Undetermined};
    use DifferenceKind::{Removed, Unavailable};
    let properties = "/sdfObject/Outlet/sdfProperty";
    assert_eq!(
        rows(&comparison),
        [
            row(Undetermined, Unavailable, "document", "meter", ""),
            row(
                Breaking,
                Removed,
                "sdfProperty",
                "outlet",
                &format!("{properties}/on")
            ),
            row(
                Undetermined,
                Unavailable,
                "sdfProperty",
                "outlet",
                &format!("{properties}/power")
            ),
        ]
    );
    for difference in comparison.differences() {
        if let Some(reason) = difference.reason() {
            assert_eq!(reason.code(), "structure.feature_context_unavailable");
        }
    }
}

/// Each difference's subject and classification, and whether the JSON report names the
/// reading of ADR 0037: Undeclared extension vocabulary.
fn extension_reading(earlier: &str, later: &str) -> (Vec<(String, Classification)>, bool) {
    let earlier = catalogue(&[("sensor", earlier)]);
    let later = catalogue(&[("sensor", later)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let mut differences: Vec<_> = comparison
        .differences()
        .map(|difference| {
            if difference.subject() == DifferenceSubject::Quality("sdfProtocolMap") {
                assert_eq!(difference.reason(), Some(UndeterminedReason::NotClassified));
            }
            (subject(&difference), difference.classification())
        })
        .collect();
    differences.sort_by(|a, b| a.0.cmp(&b.0));
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let interpretations = wire["baseline"]["interpretations"].as_array().unwrap();
    (
        differences,
        interpretations.contains(&json!("undeclared-extensions-ignorable/1")),
    )
}

#[test]
fn extension_vocabulary_keeps_base_classifications_and_names_the_reading() {
    use Classification::{Compatible, Undetermined};
    let sensor = |object: &str, value: &str, mapping: Option<Value>| {
        let mut property = json!({"description": value, "type": "number", "minimum": -40});
        if let Some(mapping) = mapping {
            property["sdfProtocolMap"] = mapping;
        }
        json!({"info": {}, "sdfObject": {"Temperature": {
            "description": object, "sdfProperty": {"value": property}}}})
        .to_string()
    };
    let ble = |service: &str| Some(json!({"ble": {"serviceID": service}}));
    let earlier = sensor("Room sensor", "Air temperature", ble("1809"));
    let described = |subject: &str, classification| (subject.to_owned(), classification);

    // The Property's base quality keeps its classification beside a changed mapping, and
    // that classification relied on reading the mapping as ignorable.
    assert_eq!(
        extension_reading(&earlier, &sensor("Room sensor", "Room air", ble("180A"))),
        (
            vec![
                described("description", Compatible),
                described("sdfProtocolMap", Undetermined)
            ],
            true
        )
    );
    // Without the mapping the same change relies on no reading.
    assert_eq!(
        extension_reading(
            &sensor("Room sensor", "Air temperature", None),
            &sensor("Room sensor", "Room air", None)
        ),
        (vec![described("description", Compatible)], false)
    );
    // A changed mapping alone is undetermined, so nothing relied on the reading.
    assert_eq!(
        extension_reading(
            &earlier,
            &sensor("Room sensor", "Air temperature", ble("180A"))
        ),
        (vec![described("sdfProtocolMap", Undetermined)], false)
    );
    // The Object's own description carries no extension vocabulary beside it.
    assert_eq!(
        extension_reading(
            &earlier,
            &sensor("Hall sensor", "Air temperature", ble("1809"))
        ),
        (vec![described("description", Compatible)], false)
    );
}

#[test]
fn an_exhausted_allowance_leaves_only_unfinished_work_undetermined() {
    // Both documents lose an Event. The first document's breaking removal stands when the
    // work or allocation allowance runs out composing or comparing the second.
    let removed = revised(LOCK, |document| {
        document["sdfObject"]["Lock"]
            .as_object_mut()
            .unwrap()
            .remove("sdfEvent");
    });
    let work = (10000..=50000)
        .step_by(250)
        .map(|work| Limits::default().decoding_work(work));
    let allocation = (20000..=400000)
        .step_by(2500)
        .map(|bytes| Limits::default().effective_allocation_bytes(bytes));
    for cutoffs in [work.collect::<Vec<_>>(), allocation.collect()] {
        let (mut composing, mut comparing) = (false, false);
        for limits in cutoffs {
            let (Ok(earlier), Ok(later)) = (
                limited(&[("first", LOCK), ("second", LOCK)], limits.clone()),
                limited(&[("first", &removed), ("second", &removed)], limits),
            ) else {
                continue;
            };
            let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
            assert_ne!(comparison.outcome(), Classification::Compatible);

            // Aggregate counts include every difference whose details were not retained.
            let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
            let mut total = 0;
            for classification in [
                Classification::Breaking,
                Classification::Undetermined,
                Classification::Compatible,
            ] {
                let count = comparison.count(classification);
                assert_eq!(wire["result"]["summary"][classification.as_str()], count);
                total += count as usize;
            }
            let shown = wire["result"]["differences"].as_array().unwrap().len();
            assert_eq!(shown, comparison.differences().len());
            assert_eq!(wire["result"]["differences_omitted"], total - shown);

            let key = |difference: &Difference<'_>| {
                let side = difference.side(Revision::Earlier).unwrap();
                earlier
                    .source(side.root_document())
                    .unwrap()
                    .key()
                    .to_owned()
            };
            let first = comparison.differences().any(|difference| {
                difference.kind() == DifferenceKind::Removed && key(&difference) == "first"
            });
            for difference in comparison.differences() {
                if difference.reason().map(|reason| reason.code()) != Some("operation.limit")
                    || !first
                    || key(&difference) != "second"
                {
                    continue;
                }
                assert_eq!(comparison.outcome(), Classification::Breaking);
                // A composition that ran out leaves its side unavailable, while a
                // comparison that ran out leaves both sides present.
                let unavailable =
                    [Revision::Earlier, Revision::Later]
                        .into_iter()
                        .any(|revision| {
                            difference.side(revision).unwrap().presence() == Presence::Unavailable
                        });
                composing |= unavailable;
                comparing |= !unavailable;
            }
        }
        assert!(composing && comparing);
    }
}

#[test]
fn a_change_without_a_rule_stays_not_classified_when_its_description_is_cut_short() {
    // The protocol mapping and the Nullable characteristic change before a long reordered
    // enum, which the allowance can run out comparing. Only the Nullable change has a rule,
    // so only its witness search is cut short.
    let values: Vec<_> = (0..200).map(|value| format!("\"v{value}\"")).collect();
    let reordered: Vec<_> = values.iter().rev().cloned().collect();
    let sensor = |service: &str, nullable: bool, values: &[String]| {
        let code = format!(
            r#""type":"string","sdfProtocolMap":{{"ble":{{"serviceID":"{service}"}}}},"nullable":{nullable},"enum":[{}]"#,
            values.join(",")
        );
        format!(r#"{{"sdfObject":{{"Sensor":{{"sdfProperty":{{"code":{{{code}}}}}}}}}}}"#)
    };
    let earlier = sensor("1809", true, &values);
    let later = sensor("180A", false, &reordered);
    let mut cut_short = 0;
    for work in (10_000..=400_000).step_by(2_000) {
        let limits = Limits::default().decoding_work(work);
        let (Ok(earlier), Ok(later)) = (
            limited(&[("sensor", &earlier)], limits.clone()),
            limited(&[("sensor", &later)], limits),
        ) else {
            continue;
        };
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        let reason = |quality| {
            comparison
                .differences()
                .find(|difference| difference.subject() == DifferenceSubject::Quality(quality))
                .and_then(|difference| difference.reason())
        };
        let stopped = comparison
            .differences()
            .any(|difference| difference.subject() == DifferenceSubject::Document);
        let (Some(mapping), Some(nullable)) = (reason("sdfProtocolMap"), reason("nullable")) else {
            continue;
        };
        if stopped {
            cut_short += 1;
            assert_eq!(mapping, UndeterminedReason::NotClassified, "work {work}");
            assert_eq!(nullable.code(), "operation.limit", "work {work}");
        }
    }
    assert!(cut_short > 0);
}

#[test]
fn effective_cycles_and_unavailable_dependencies_stay_scoped() {
    use Classification::{Breaking, Undetermined};
    use DifferenceKind::{Removed, Unavailable};
    let outlet = |extra: Value| {
        let mut properties = json!({
            "power": {"sdfRef": "meter:#/sdfData/Power"},
            "level": {"sdfRef": "#/sdfObject/Outlet/sdfProperty/limit"},
            "limit": {"sdfRef": "#/sdfObject/Outlet/sdfProperty/level"}
        });
        for (key, value) in extra.as_object().unwrap() {
            properties[key] = value.clone();
        }
        json!({"info": {}, "namespace": {"meter": "https://example.test/meter"},
            "sdfObject": {"Outlet": {"sdfProperty": properties}}})
        .to_string()
    };
    let meter = json!({"info": {}, "namespace": {"meter": "https://example.test/meter"},
        "defaultNamespace": "meter", "sdfData": {"Power": {"type": "number"}}})
    .to_string();
    let earlier = catalogue(&[
        ("outlet", &outlet(json!({"on": {"type": "boolean"}}))),
        ("meter", &meter),
    ]);
    let later = catalogue(&[("outlet", &outlet(json!({}))), ("meter", "{\"sdfData\": ")]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Breaking);
    let properties = "/sdfObject/Outlet/sdfProperty";
    let property = |name: &str| format!("{properties}/{name}");
    assert_eq!(
        rows(&comparison),
        [
            row(Undetermined, Unavailable, "document", "meter", ""),
            row(
                Undetermined,
                Unavailable,
                "sdfProperty",
                "outlet",
                &property("level")
            ),
            row(
                Undetermined,
                Unavailable,
                "sdfProperty",
                "outlet",
                &property("limit")
            ),
            row(Breaking, Removed, "sdfProperty", "outlet", &property("on")),
            row(
                Undetermined,
                Unavailable,
                "sdfProperty",
                "outlet",
                &property("power")
            ),
        ]
    );
    let mut codes: Vec<_> = comparison
        .differences()
        .filter_map(|difference| difference.reason())
        .map(|reason| reason.code())
        .collect();
    codes.sort();
    assert_eq!(
        codes,
        [
            "json.syntax",
            "reference.effective_cycle",
            "reference.effective_cycle",
            "reference.unavailable"
        ]
    );
}

/// Each changed quality as its name, classification, Client directions and the pointer in the
/// later revision.
fn changed_qualities(
    comparison: &RevisionComparison<'_>,
) -> Vec<(String, Classification, Vec<Direction>, String)> {
    let mut rows: Vec<_> = comparison
        .differences()
        .map(|difference| {
            assert_eq!(difference.kind(), DifferenceKind::Changed);
            (
                subject(&difference),
                difference.classification(),
                difference.directions().collect(),
                pointer(&difference, Revision::Later),
            )
        })
        .collect();
    rows.sort_by(|a, b| a.3.cmp(&b.3));
    rows
}

#[test]
fn a_characteristic_that_becomes_false_breaks_the_direction_it_removes() {
    use Classification::Breaking;
    use Direction::{ClientSupplied, ThingSupplied};
    let earlier = catalogue(&[("lock", LOCK)]);
    let later = catalogue(&[(
        "lock",
        &revised(LOCK, |document| {
            let credential = &mut document["sdfObject"]["Lock"]["sdfProperty"]["credential"];
            credential["readable"] = json!(false);
            credential["writable"] = json!(false);
            credential["observable"] = json!(false);
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Breaking);
    assert_eq!(comparison.outcome().disposition(), Disposition::Failed);
    let credential = "/sdfObject/Lock/sdfProperty/credential";
    assert_eq!(
        changed_qualities(&comparison),
        [
            (
                "observable".to_owned(),
                Breaking,
                vec![ThingSupplied],
                format!("{credential}/observable")
            ),
            (
                "readable".to_owned(),
                Breaking,
                vec![ThingSupplied],
                format!("{credential}/readable")
            ),
            (
                "writable".to_owned(),
                Breaking,
                vec![ClientSupplied],
                format!("{credential}/writable")
            ),
        ]
    );
    for difference in comparison.differences() {
        assert!(difference.reason().is_none());
        // The absent characteristic had its Specified default `true`.
        let earlier = difference.side(Revision::Earlier).unwrap();
        assert_eq!(earlier.presence(), Presence::Absent);
        assert!(earlier.origin().is_none());
        assert_eq!(
            difference.side(Revision::Later).unwrap().presence(),
            Presence::Present
        );
    }
}

#[test]
fn a_characteristic_that_becomes_true_is_compatible() {
    use Classification::Compatible;
    use Direction::{ClientSupplied, ThingSupplied};
    let with_code = |code: Value| {
        revised(LOCK, |document| {
            document["sdfObject"]["Lock"]["sdfProperty"]["code"] = code;
        })
    };
    let earlier = catalogue(&[(
        "lock",
        &with_code(json!({"type": "integer", "readable": false, "observable": false})),
    )]);
    let later = catalogue(&[(
        "lock",
        &revised(
            &with_code(json!({"type": "integer", "readable": true})),
            |document| {
                let properties = &mut document["sdfObject"]["Lock"]["sdfProperty"];
                // Explicit false becoming absent takes the Specified default `true`.
                properties["state"]
                    .as_object_mut()
                    .unwrap()
                    .remove("writable");
                // Absent becoming explicit `true` is no change.
                properties["credential"]["observable"] = json!(true);
            },
        ),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Compatible);
    assert_eq!(comparison.outcome().disposition(), Disposition::Passed);
    let lock = "/sdfObject/Lock/sdfProperty";
    assert_eq!(
        changed_qualities(&comparison),
        [
            (
                "observable".to_owned(),
                Compatible,
                vec![ThingSupplied],
                format!("{lock}/code/observable")
            ),
            (
                "readable".to_owned(),
                Compatible,
                vec![ThingSupplied],
                format!("{lock}/code/readable")
            ),
            (
                "writable".to_owned(),
                Compatible,
                vec![ClientSupplied],
                format!("{lock}/state/writable")
            ),
        ]
    );
    let state = comparison
        .differences()
        .find(|difference| difference.subject() == DifferenceSubject::Quality("writable"))
        .unwrap();
    assert_eq!(
        state.side(Revision::Later).unwrap().presence(),
        Presence::Absent
    );
}

#[test]
fn a_characteristic_changed_in_a_shared_definition_breaks_every_grouping_that_uses_it() {
    use Classification::Breaking;
    use Direction::{ClientSupplied, ThingSupplied};
    let earlier = catalogue(&[("strip", STRIP), ("meter", METER)]);
    let meter = revised(METER, |document| {
        let outlet = &mut document["sdfObject"]["Outlet"]["sdfProperty"];
        outlet["on"]["writable"] = json!(false);
        outlet["power"]["observable"] = json!(false);
    });
    let later = catalogue(&[("strip", STRIP), ("meter", &meter)]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Breaking);
    let shared = "/sdfObject/Outlet/sdfProperty";
    let strip = "/sdfThing/PowerStrip/sdfObject/outlet/sdfProperty";
    assert_eq!(
        changed_qualities(&comparison),
        [
            (
                "writable".to_owned(),
                Breaking,
                vec![ClientSupplied],
                format!("{shared}/on/writable")
            ),
            (
                "observable".to_owned(),
                Breaking,
                vec![ThingSupplied],
                format!("{shared}/power/observable")
            ),
            (
                "writable".to_owned(),
                Breaking,
                vec![ClientSupplied],
                format!("{strip}/on/writable")
            ),
            (
                "observable".to_owned(),
                Breaking,
                vec![ThingSupplied],
                format!("{strip}/power/observable")
            ),
        ]
    );
    // Each later side comes from the shared definition. The earlier `on` has its explicit
    // `true` there, while the earlier `power` had no observable characteristic at all.
    for difference in comparison.differences() {
        let writable = difference.subject() == DifferenceSubject::Quality("writable");
        let member = if writable {
            "on/writable"
        } else {
            "power/observable"
        };
        let expected = ("meter".to_owned(), format!("{shared}/{member}"));
        assert_eq!(origin(&comparison, &difference, Revision::Later), expected);
        if writable {
            assert_eq!(
                origin(&comparison, &difference, Revision::Earlier),
                expected
            );
        } else {
            let earlier = difference.side(Revision::Earlier).unwrap();
            assert_eq!(earlier.presence(), Presence::Absent);
            assert!(earlier.origin().is_none());
        }
    }

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let writable = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| {
            difference["later"]["location"]["pointer"] == format!("{strip}/on/writable")
        })
        .unwrap();
    assert_eq!(writable["kind"], "changed");
    assert_eq!(writable["subject"], "quality");
    assert_eq!(writable["quality"], "writable");
    assert_eq!(writable["classification"], "breaking");
    assert_eq!(writable["directions"], json!(["client_supplied"]));
    assert!(writable.get("reason").is_none());
    for revision in ["earlier", "later"] {
        assert_eq!(writable[revision]["presence"], "present");
        assert_eq!(
            writable[revision]["origin"]["pointer"],
            format!("{shared}/on/writable")
        );
    }
    let human = comparison.to_human();
    assert!(human.contains(&format!(
        "  changed quality writable (client-supplied data)\n    earlier strip {strip}/on/writable\n      from meter {shared}/on/writable\n    later strip {strip}/on/writable\n      from meter {shared}/on/writable\n"
    )));
    assert!(human.contains(&format!(
        "  changed quality observable (thing-supplied data)\n    earlier strip {strip}/power/observable (absent)\n    later strip {strip}/power/observable\n      from meter {shared}/power/observable\n"
    )));
}

#[test]
fn input_data_added_to_an_action_without_it_is_breaking() {
    use Classification::{Breaking, Compatible};
    use Direction::{ClientSupplied, ThingSupplied};
    let earlier = catalogue(&[(
        "thermostat",
        &revised(THERMOSTAT, |document| {
            document["sdfObject"]["Thermostat"]["sdfAction"]["reset"] =
                json!({"description": "Restore factory settings"});
        }),
    )]);
    let later = catalogue(&[(
        "thermostat",
        &revised(THERMOSTAT, |document| {
            let actions = &mut document["sdfObject"]["Thermostat"]["sdfAction"];
            actions["reset"] = json!({
                "description": "Restore factory settings",
                "sdfInputData": {"type": "boolean", "nullable": false},
                "sdfOutputData": {"type": "boolean", "nullable": false}
            });
        }),
    )]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Breaking);
    let actions = "/sdfObject/Thermostat/sdfAction";
    assert_eq!(
        changed_qualities(&comparison),
        [
            (
                "sdfInputData".to_owned(),
                Breaking,
                vec![ClientSupplied],
                format!("{actions}/reset/sdfInputData")
            ),
            (
                "sdfOutputData".to_owned(),
                Compatible,
                vec![ThingSupplied],
                format!("{actions}/reset/sdfOutputData")
            ),
        ]
    );
    // No Client of the earlier revision reads output data that the Action did not have.
    for difference in comparison.differences() {
        assert!(difference.reason().is_none());
        assert_eq!(
            difference.side(Revision::Earlier).unwrap().presence(),
            Presence::Absent
        );
    }

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let differences = wire["result"]["differences"].as_array().unwrap();
    let added = differences
        .iter()
        .find(|difference| {
            difference["later"]["location"]["pointer"] == format!("{actions}/reset/sdfInputData")
        })
        .unwrap();
    assert_eq!(added["classification"], "breaking");
    assert_eq!(added["directions"], json!(["client_supplied"]));
    assert!(added.get("reason").is_none());
    assert_eq!(added["earlier"]["presence"], "absent");
    let human = comparison.to_human();
    assert!(human.contains(&format!(
        "Breaking differences:\n  changed quality sdfInputData (client-supplied data)\n    earlier thermostat {actions}/reset/sdfInputData (absent)\n    later thermostat {actions}/reset/sdfInputData\n"
    )));
}
