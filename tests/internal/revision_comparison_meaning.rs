//! Changes to the meaning qualities `contentFormat` and `sdfType` in a Revision comparison,
//! following ADR 0039: Client-side revision comparison and ADR 0033: Content-format model
//! meaning.
use esdieff::{
    Catalogue, Classification, ComparisonRequest, ContentFormatReason, Difference, DifferenceKind,
    DifferenceSubject, Direction, Limits, Presence, RecordedRegistryPolicy, Revision,
    RevisionComparison, SourceInput, UndeterminedReason,
};
use serde_json::{Value, json};

use super::comparison_support;
use comparison_support::{at, catalogue, client, pointer, thing, witness};

/// A Device whose Properties are these descriptions.
fn device(properties: Value) -> String {
    json!({"sdfObject": {"Device": {"sdfProperty": properties}}}).to_string()
}

/// An earlier Device with these Properties and a later one with those.
fn revisions(earlier: Value, later: Value) -> [Catalogue; 2] {
    [earlier, later].map(|properties| catalogue(&[("device", &device(properties))]))
}

/// The only difference of a comparison.
fn only<'r>(comparison: &'r RevisionComparison<'r>) -> Difference<'r> {
    assert_eq!(comparison.differences().len(), 1);
    comparison.differences().next().unwrap()
}

/// A string Property declaring this content format.
fn payload(format: &str) -> Value {
    payloads(&[("payload", format)])
}

/// String Properties declaring these content formats, by name.
fn payloads(formats: &[(&str, &str)]) -> Value {
    let properties = formats
        .iter()
        .map(|(name, format)| {
            let description = json!({"type": "string", "contentFormat": format});
            ((*name).to_owned(), description)
        })
        .collect::<serde_json::Map<_, _>>();
    Value::Object(properties)
}

#[test]
fn spellings_of_one_established_content_format_are_no_change() {
    let [earlier, later] = revisions(
        payloads(&[
            ("identifier", "50"),
            ("case", "Application/JSON"),
            ("charset", "application/json; charset=latin1"),
            ("ascii", "text/plain"),
            ("text", "0"),
            ("coded", "11050"),
            ("codings", "application/cbor@GZIP@deflate"),
        ]),
        payloads(&[
            ("identifier", "application/json"),
            ("case", "application/json"),
            ("charset", "application/json"),
            ("ascii", "text/plain;charset=\"US-ASCII\""),
            ("text", "text/plain; CHARSET=utf-8"),
            ("coded", "application/json@deflate"),
            ("codings", "application/cbor@gzip@Deflate"),
        ]),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 0);
    assert_eq!(comparison.outcome(), Classification::Compatible);
    for registry in ["coap-content-formats", "http-content-codings"] {
        let knowledge = comparison.registry_knowledge(registry).require().unwrap();
        assert_eq!(knowledge.state(), "available", "{registry}");
    }
    assert!(
        comparison
            .registry_knowledge("media-types")
            .present()
            .is_none()
    );
}

/// Each difference by Property name, with its classification and reason code.
fn rows(comparison: &RevisionComparison<'_>) -> Vec<(String, Classification, &'static str)> {
    let mut rows: Vec<_> = comparison
        .differences()
        .map(|difference| {
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

#[test]
fn recorded_identifiers_codings_and_supported_media_rules_establish_different_meanings() {
    let [earlier, later] = revisions(
        payloads(&[
            ("cbor", "50"),
            ("charset", "text/plain"),
            ("coding", "application/json"),
            ("image", "21"),
            ("order", "application/json@gzip@deflate"),
            ("tuple", "11050"),
        ]),
        payloads(&[
            ("cbor", "60"),
            ("charset", "text/plain; charset=utf-8"),
            ("coding", "application/json@gzip"),
            ("image", "22"),
            ("order", "application/json@deflate@gzip"),
            ("tuple", "12050"),
        ]),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let breaking = |property: &str| (property.to_owned(), Classification::Breaking, "");
    assert_eq!(
        rows(&comparison),
        ["cbor", "charset", "coding", "image", "order", "tuple"].map(breaking)
    );
}

#[test]
fn added_removed_and_unestablished_content_formats_are_undetermined() {
    use Classification::Undetermined;
    let earlier = json!({
        "added": {"type": "string"},
        "removed": {"type": "string", "contentFormat": "application/json"},
        "media": {"type": "string", "contentFormat": "image/gif"},
        "unrecorded": {"type": "string", "contentFormat": "1"},
        "range": {"type": "string", "contentFormat": "65536"},
        "spelling": {"type": "string", "contentFormat": "application/json;"},
        "coding": {"type": "string", "contentFormat": "application/json@vendor"},
        "identity": {"type": "string", "contentFormat": "application/json@identity"},
        "alias": {"type": "string", "contentFormat": "application/json@x-gzip"},
        "parameter": {"type": "string", "contentFormat": "text/plain; charset=latin1"},
        "registered": {"type": "string", "contentFormat": "21"},
        "number": {"type": "string", "contentFormat": 50},
    });
    let later = json!({
        "added": {"type": "string", "contentFormat": "application/json"},
        "removed": {"type": "string"},
        "media": {"type": "string", "contentFormat": "image/png"},
        "unrecorded": {"type": "string", "contentFormat": "application/json"},
        "range": {"type": "string", "contentFormat": "application/json"},
        "spelling": {"type": "string", "contentFormat": "application/json"},
        "coding": {"type": "string", "contentFormat": "application/json"},
        "identity": {"type": "string", "contentFormat": "application/json"},
        "alias": {"type": "string", "contentFormat": "application/json@gzip"},
        "parameter": {"type": "string", "contentFormat": "text/plain"},
        "registered": {"type": "string", "contentFormat": "image/gif"},
        "number": {"type": "string", "contentFormat": "application/json"},
    });
    let [earlier, later] = revisions(earlier, later);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Undetermined);
    let row = |property: &str, code| (property.to_owned(), Undetermined, code);
    let unestablished = "comparison.content_format_unestablished";
    assert_eq!(
        rows(&comparison),
        [
            row("added", "comparison.content_format_added"),
            row("alias", unestablished),
            row("coding", unestablished),
            row("identity", unestablished),
            row("media", unestablished),
            row("number", "comparison.not_classified"),
            row("parameter", unestablished),
            row("range", unestablished),
            row("registered", unestablished),
            row("removed", "comparison.content_format_removed"),
            row("spelling", unestablished),
            row("unrecorded", unestablished),
        ]
    );
    let added = comparison
        .differences()
        .find(|difference| {
            difference.reason()
                == Some(UndeterminedReason::ContentFormat(
                    ContentFormatReason::Added,
                ))
        })
        .unwrap();
    assert_eq!(
        added.side(Revision::Earlier).unwrap().presence(),
        Presence::Absent
    );
    assert_eq!(
        added.reason().unwrap().message(),
        "content format is added, which changes the stated meaning of values"
    );
}

#[test]
fn withheld_evidence_leaves_content_format_changes_that_need_it_undetermined() {
    let [earlier, later] = revisions(
        payloads(&[
            ("identifier", "50"),
            ("coding", "application/json@gzip"),
            ("media", "application/json"),
            ("unsupported", "image/gif"),
        ]),
        payloads(&[
            ("identifier", "application/json"),
            ("coding", "application/json"),
            ("media", "text/plain"),
            ("unsupported", "50"),
        ]),
    );
    let comparison = ComparisonRequest::new(&earlier, &later)
        .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded)
        .compare()
        .unwrap();
    let withheld = "registry.caller_withheld";
    assert_eq!(
        rows(&comparison),
        [
            ("coding".to_owned(), Classification::Undetermined, withheld),
            (
                "identifier".to_owned(),
                Classification::Undetermined,
                withheld
            ),
            // The supported media rules need no recorded evidence.
            ("media".to_owned(), Classification::Breaking, ""),
            (
                "unsupported".to_owned(),
                Classification::Undetermined,
                "comparison.content_format_unestablished"
            ),
        ]
    );
    let identifier = comparison
        .differences()
        .find(|difference| {
            pointer(difference, Revision::Later).ends_with("/identifier/contentFormat")
        })
        .unwrap();
    assert_eq!(
        identifier.reason(),
        Some(UndeterminedReason::ContentFormat(
            ContentFormatReason::EvidenceWithheld
        ))
    );
    for registry in ["coap-content-formats", "http-content-codings"] {
        let knowledge = comparison.registry_knowledge(registry).require().unwrap();
        assert_eq!(
            (knowledge.state(), knowledge.reason()),
            ("unavailable", Some("caller_withheld"))
        );
    }

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    assert_eq!(wire["policy"]["recorded_registry"], "withhold_recorded");
    assert_eq!(
        wire["baseline"]["registry_knowledge"],
        json!([
            {"registry": "coap-content-formats", "state": "unavailable", "snapshot": null,
                "reason": "caller_withheld"},
            {"registry": "http-content-codings", "state": "unavailable", "snapshot": null,
                "reason": "caller_withheld"},
        ])
    );
    assert_eq!(
        wire["baseline"]["interpretations"],
        json!(["content-format-model-meaning/1"])
    );
    let difference = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| {
            difference["later"]["location"]["pointer"]
                == "/sdfObject/Device/sdfProperty/identifier/contentFormat"
        })
        .unwrap();
    assert_eq!(difference["quality"], "contentFormat");
    assert_eq!(
        difference["reason"],
        json!({"code": withheld, "message": "the request withholds recorded content-format evidence"})
    );
    let human = comparison.to_human();
    for expected in [
        "  changed quality contentFormat (client-supplied and thing-supplied data): the request withholds recorded content-format evidence\n    earlier device /sdfObject/Device/sdfProperty/identifier/contentFormat\n",
        "Registry coap-content-formats: unavailable; snapshot null; reason caller_withheld\n",
        "Registry http-content-codings: unavailable; snapshot null; reason caller_withheld\n",
        "Interpretation: content-format-model-meaning/1\n",
    ] {
        assert!(human.contains(expected), "{expected}\n{human}");
    }
}

#[test]
fn a_respelled_content_format_keeps_a_compatibility_proof_and_a_changed_one_blocks_it() {
    // Write-only data whose enum gains a value widens safely for Clients.
    let command = |format: &str, values: Value| {
        json!({"command": {"type": "string", "enum": values, "contentFormat": format,
            "readable": false, "observable": false}})
    };
    let [earlier, later] = revisions(
        command("50", json!(["on"])),
        command("application/json", json!(["on", "off"])),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Compatible);
    let enumeration = only(&comparison);
    assert_eq!(enumeration.subject(), DifferenceSubject::Quality("enum"));

    let [earlier, later] = revisions(
        command("application/json", json!(["on"])),
        command("text/plain", json!(["on", "off"])),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    let mut rows = rows(&comparison);
    rows.sort_by_key(|row| row.1 as u8);
    assert_eq!(
        rows,
        [
            ("command".to_owned(), Classification::Breaking, ""),
            (
                "command".to_owned(),
                Classification::Undetermined,
                "comparison.unproven"
            ),
        ]
    );
}

#[test]
fn a_respelled_content_format_is_no_change_even_without_known_directions() {
    // A writable characteristic that is not a boolean leaves the data directions unknown.
    let unknown = |format: &str| json!({"payload": {"type": "string", "contentFormat": format, "writable": "yes"}});
    let [earlier, later] = revisions(unknown("50"), unknown("application/json"));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.differences().len(), 0);

    let [earlier, later] = revisions(unknown("50"), unknown("text/plain"));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        only(&comparison).reason().unwrap().code(),
        "comparison.directions_unavailable"
    );
}

#[test]
fn a_content_format_comparison_that_exhausts_the_allowance_stays_undetermined() {
    // Classifying a long parameter value costs far more work than composing and walking
    // this small Device.
    let long = format!("text/plain; charset=\"{}\"", "x".repeat(8000));
    let reason = |limits: Limits| {
        let [earlier, later] = [payload("text/plain"), payload(&long)].map(|properties| {
            let device = device(properties);
            let input = SourceInput::supplied("device", device.as_bytes());
            Catalogue::with_limits(&[input], limits.clone()).unwrap()
        });
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert_eq!(comparison.outcome(), Classification::Undetermined);
        comparison
            .differences()
            .find(|difference| difference.subject() == DifferenceSubject::Quality("contentFormat"))
            .map(|difference| difference.reason().unwrap().code())
    };
    assert_eq!(
        reason(Limits::default()),
        Some("comparison.content_format_unestablished")
    );
    assert_eq!(
        reason(Limits::default().decoding_work(200_000)),
        Some("operation.limit")
    );
}

#[test]
fn a_change_between_byte_string_and_unix_time_breaks_every_direction() {
    let [earlier, later] = revisions(
        json!({"stamp": {"sdfType": "byte-string"}}),
        json!({"stamp": {"sdfType": "unix-time"}}),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    let difference = only(&comparison);
    assert_eq!(difference.subject(), DifferenceSubject::Quality("sdfType"));
    assert_eq!(difference.classification(), Classification::Breaking);
    assert!(difference.witness().is_none());
    assert_eq!(
        difference.directions().collect::<Vec<_>>(),
        [Direction::ClientSupplied, Direction::ThingSupplied]
    );
}

/// Write-only and read-only data with these qualities, and each with this `sdfType`.
fn conventions(written: &[(&str, Value, Option<&str>)]) -> Value {
    let mut properties = serde_json::Map::new();
    for (name, description, convention) in written {
        let mut description = description.clone();
        if name.starts_with("write") {
            description["readable"] = json!(false);
            description["observable"] = json!(false);
        } else {
            description["writable"] = json!(false);
        }
        if let Some(convention) = convention {
            description["sdfType"] = json!(convention);
        }
        properties.insert((*name).to_owned(), description);
    }
    Value::Object(properties)
}

#[test]
fn an_added_or_removed_sdf_type_is_breaking_only_with_a_witness() {
    use Classification::{Breaking, Undetermined};
    let string = json!({"type": "string"});
    let lowercase = json!({"type": "string", "pattern": "^[a-z]*$"});
    let number = json!({"type": "number"});
    let earlier = conventions(&[
        ("writeAdded", string.clone(), None),
        ("readRemoved", string.clone(), Some("byte-string")),
        ("readAdded", string.clone(), None),
        ("writeExcluded", lowercase.clone(), None),
        ("readExcluded", lowercase.clone(), Some("byte-string")),
        ("writeUntyped", json!({}), None),
        ("writeNumber", number.clone(), None),
        ("writeVendor", string.clone(), None),
    ]);
    let later = conventions(&[
        ("writeAdded", string.clone(), Some("byte-string")),
        ("readRemoved", string.clone(), None),
        ("readAdded", string, Some("byte-string")),
        ("writeExcluded", lowercase.clone(), Some("byte-string")),
        ("readExcluded", lowercase, None),
        ("writeUntyped", json!({}), Some("unix-time")),
        ("writeNumber", number, Some("unix-time")),
        (
            "writeVendor",
            json!({"type": "string"}),
            Some("vendor-time"),
        ),
    ]);
    let [earlier, later] = revisions(earlier, later);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let row = |property: &str, classification, code| (property.to_owned(), classification, code);
    assert_eq!(
        rows(&comparison),
        [
            row("readAdded", Undetermined, "comparison.unproven"),
            // In both Excluded rows, the pattern of the revision that must admit the candidate
            // already excludes it.
            row("readExcluded", Undetermined, "comparison.unproven"),
            row("readRemoved", Breaking, ""),
            row("writeAdded", Breaking, ""),
            row("writeExcluded", Undetermined, "comparison.unproven"),
            row("writeNumber", Undetermined, "comparison.unproven"),
            row("writeUntyped", Breaking, ""),
            row("writeVendor", Undetermined, "comparison.not_classified"),
        ]
    );
    let witness_at = |property: &str| {
        witness(&at(
            &comparison,
            &format!("/sdfProperty/{property}/sdfType"),
        ))
    };
    assert_eq!(witness_at("writeAdded"), client("\"!\""));
    assert_eq!(witness_at("readRemoved"), thing("\"!\""));
    assert_eq!(witness_at("writeUntyped"), client("\"\""));

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let added = "/sdfObject/Device/sdfProperty/writeAdded/sdfType";
    let difference = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| difference["later"]["location"]["pointer"] == added)
        .unwrap();
    assert_eq!(
        difference["witness"],
        json!({"direction": "client_supplied", "raw_json": "\"!\"",
            "admission": {"earlier": "admitted", "later": "rejected"}})
    );
    let expected = format!(
        "  changed quality sdfType (client-supplied data)\n    witness \"!\" for client-supplied data: earlier admitted, later rejected\n    earlier device {added} (absent)\n    later device {added}\n"
    );
    let human = comparison.to_human();
    assert!(human.contains(&expected), "{expected}\n{human}");
}

#[test]
fn a_change_between_a_convention_and_another_sdf_type_is_not_classified() {
    let string = json!({"type": "string"});
    let numbered = json!({"type": "string", "sdfType": 5});
    // Each candidate of a convention would reach the revision with the other value, which
    // must admit it.
    let earlier = conventions(&[
        ("readByteString", string.clone(), Some("byte-string")),
        ("readUnixTime", json!({}), Some("unix-time")),
        ("writeByteString", string.clone(), Some("vendor-time")),
        ("writeUnixTime", json!({}), Some("vendor-time")),
        ("writeNumbered", numbered, None),
    ]);
    let later = conventions(&[
        ("readByteString", string.clone(), Some("vendor-time")),
        ("readUnixTime", json!({}), Some("vendor-time")),
        ("writeByteString", string.clone(), Some("byte-string")),
        ("writeUnixTime", json!({}), Some("unix-time")),
        ("writeNumbered", string, Some("byte-string")),
    ]);
    let [earlier, later] = revisions(earlier, later);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let row = |property: &str| {
        let code = "comparison.not_classified";
        (property.to_owned(), Classification::Undetermined, code)
    };
    assert_eq!(
        rows(&comparison),
        [
            row("readByteString"),
            row("readUnixTime"),
            row("writeByteString"),
            row("writeNumbered"),
            row("writeUnixTime"),
        ]
    );
}

/// The interpretations that the JSON report names in its baseline.
fn interpretations(comparison: &RevisionComparison<'_>) -> Value {
    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    wire["baseline"]["interpretations"].clone()
}

#[test]
fn a_witness_names_the_value_interpretations_its_checks_relied_on() {
    let string = json!({"type": "string"});
    let format = |name: &str| json!({"type": "string", "format": name});
    // The later convention or format rejects each witness, `"!"` or `""`.
    let cases = [
        (
            string.clone(),
            string.clone(),
            Some("byte-string"),
            "byte-string-json/1",
        ),
        (
            json!({}),
            json!({}),
            Some("unix-time"),
            "unix-time-posix-coordinate/1",
        ),
        (string.clone(), format("uuid"), None, "uuid-bare-json/1"),
        (
            string.clone(),
            format("date-time"),
            None,
            "ADR0018-selected-representation",
        ),
    ];
    for (old, new, convention, expected) in cases {
        let [earlier, later] = revisions(
            conventions(&[("writeAdded", old, None)]),
            conventions(&[("writeAdded", new, convention)]),
        );
        let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
        assert!(only(&comparison).witness().is_some(), "{expected}");
        assert_eq!(interpretations(&comparison), json!([expected]));
    }

    // Checks that establish no witness count for nothing: both revisions admit the
    // conventional UUID, and only the earlier one checks a longer string.
    let bounded = |maximum: u32| json!({"type": "string", "format": "uuid", "maxLength": maximum});
    let [earlier, later] = revisions(
        conventions(&[("writeShorter", bounded(40), None)]),
        conventions(&[("writeShorter", bounded(37), None)]),
    );
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(
        only(&comparison).reason(),
        Some(UndeterminedReason::Unproven)
    );
    assert_eq!(interpretations(&comparison), json!([]));
}

#[test]
fn a_change_between_two_established_content_formats_breaks_every_direction() {
    let [earlier, later] = revisions(payload("application/json"), payload("text/plain"));
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    assert_eq!(comparison.outcome(), Classification::Breaking);
    let difference = only(&comparison);
    assert_eq!(difference.kind(), DifferenceKind::Changed);
    assert_eq!(
        difference.subject(),
        DifferenceSubject::Quality("contentFormat")
    );
    assert_eq!(difference.classification(), Classification::Breaking);
    assert!(difference.reason().is_none());
    assert!(difference.witness().is_none());
    assert_eq!(
        difference.directions().collect::<Vec<_>>(),
        [Direction::ClientSupplied, Direction::ThingSupplied]
    );
    let location = "/sdfObject/Device/sdfProperty/payload/contentFormat";
    assert_eq!(pointer(&difference, Revision::Later), location);

    let wire: Value = serde_json::from_str(&comparison.to_json()).unwrap();
    let differences = wire["result"]["differences"].as_array().unwrap();
    assert_eq!(differences.len(), 1);
    assert_eq!(
        (
            &differences[0]["kind"],
            &differences[0]["subject"],
            &differences[0]["quality"],
            &differences[0]["classification"],
            &differences[0]["directions"],
        ),
        (
            &json!("changed"),
            &json!("quality"),
            &json!("contentFormat"),
            &json!("breaking"),
            &json!(["client_supplied", "thing_supplied"]),
        )
    );
    assert!(differences[0].get("reason").is_none());
    assert!(differences[0].get("witness").is_none());
    // The supported media rules need no recorded evidence.
    assert_eq!(wire["baseline"]["registry_knowledge"], json!([]));
    assert_eq!(
        wire["baseline"]["interpretations"],
        json!(["content-format-model-meaning/1"])
    );
    let expected = format!(
        "  changed quality contentFormat (client-supplied and thing-supplied data)\n    earlier device {location}\n    later device {location}\n"
    );
    let human = comparison.to_human();
    assert!(human.contains(&expected), "{expected}\n{human}");
    assert!(human.contains("Interpretation: content-format-model-meaning/1\n"));
    assert!(!human.contains("Registry"));
}
