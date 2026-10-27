//! Sample-value checking admits or rejects a sample on the base description of a
//! definition carrying undeclared extension vocabulary, and Sample reports name the
//! reading whenever the result relied on it.
//! Follows ADR 0037: Undeclared extension vocabulary.
mod undeclared_extension_support;
use esdieff::{Admission, Catalogue, SampleKind, SampleRequest, SourceInput};
use serde_json::{Value, json};
use undeclared_extension_support::temperature;

/// The observable result of one Sample check.
struct Sampled {
    admission: Admission,
    complete: bool,
    wire: Value,
}

impl Sampled {
    /// The codes of retained conditions with `severity`.
    fn codes(&self, severity: &str) -> Vec<&str> {
        undeclared_extension_support::codes(&self.wire, severity)
    }
    fn names_reading(&self) -> bool {
        undeclared_extension_support::names_reading(&self.wire)
    }
}

/// Checks the JSON text `sample` against the `kind` description selected at `pointer`
/// in `document`, with `others` as further model sources.
fn check_among(
    document: &Value,
    others: &[&Value],
    kind: SampleKind,
    pointer: &str,
    sample: &str,
) -> Sampled {
    let models: Vec<_> = std::iter::once(document)
        .chain(others.iter().copied())
        .map(|model| serde_json::to_vec(model).unwrap())
        .collect();
    let keys: Vec<_> = (0..models.len()).map(|index| format!("m{index}")).collect();
    let mut inputs: Vec<_> = keys
        .iter()
        .zip(&models)
        .map(|(key, model)| SourceInput::supplied(key, model))
        .collect();
    inputs.push(SourceInput::supplied("sample", sample.as_bytes()).sample());
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let report = catalogue
        .check_sample(
            SampleRequest::new(catalogue.source_id("m0").unwrap(), kind, pointer)
                .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    Sampled {
        admission: report.sample_result().unwrap().admission(),
        complete: report.outcome().complete(),
        wire: serde_json::from_str(&report.to_json()).unwrap(),
    }
}

/// Checks the JSON text `sample` against the `kind` description selected at `pointer`.
fn check(document: &Value, kind: SampleKind, pointer: &str, sample: &str) -> Sampled {
    check_among(document, &[], kind, pointer, sample)
}

fn value(document: &Value, sample: &str) -> Sampled {
    check(
        document,
        SampleKind::Property,
        "/sdfObject/Temperature/sdfProperty/value",
        sample,
    )
}

#[test]
fn a_protocol_mapped_property_admits_and_rejects_on_its_base_description() {
    let mapped = temperature(json!({"sdfProtocolMap": {"ble": {"serviceID": "1809"}}}));
    let admitted = value(&mapped, "20");
    assert_eq!(admitted.admission, Admission::Admitted);
    assert!(admitted.complete);
    assert!(admitted.names_reading());
    let rejected = value(&mapped, "100");
    assert_eq!(rejected.admission, Admission::Rejected);
    assert!(rejected.complete);
    assert_eq!(rejected.codes("error"), ["sample.bound"]);
    assert!(rejected.names_reading());
}

#[test]
fn every_selected_description_kind_checks_its_base_description_beside_a_qualified_name() {
    let level = json!({"type": "integer", "maximum": 10, "acme:note": "x"});
    let document = json!({
        "info": {},
        "sdfData": {"Level": level},
        "sdfObject": {"Tank": {
            "sdfAction": {"fill": {"sdfInputData": level, "sdfOutputData": level}},
            "sdfEvent": {"full": {"sdfOutputData": level}}
        }}
    });
    for (kind, pointer) in [
        (SampleKind::Data, "/sdfData/Level"),
        (SampleKind::ActionInput, "/sdfObject/Tank/sdfAction/fill"),
        (SampleKind::ActionOutput, "/sdfObject/Tank/sdfAction/fill"),
        (SampleKind::EventOutput, "/sdfObject/Tank/sdfEvent/full"),
    ] {
        for (sample, admission, error) in [
            ("5", Admission::Admitted, None),
            ("11", Admission::Rejected, Some("sample.bound")),
            ("5.5", Admission::Rejected, Some("sample.integer")),
        ] {
            let checked = check(&document, kind, pointer, sample);
            let case = format!("{} {sample}", kind.as_str());
            assert_eq!(checked.admission, admission, "{case}");
            assert!(checked.complete, "{case}");
            assert_eq!(checked.codes("error"), Vec::from_iter(error), "{case}");
            assert!(checked.names_reading(), "{case}");
        }
    }
}

/// A document whose `p` Property has `description`.
fn property(description: Value) -> Value {
    json!({"info": {}, "sdfProperty": {"p": description}})
}

/// Checks the JSON text `sample` against a `p` Property with `description`.
fn check_description(description: Value, sample: &str) -> Sampled {
    check(
        &property(description),
        SampleKind::Property,
        "/sdfProperty/p",
        sample,
    )
}

#[test]
fn nested_descriptions_check_their_base_meaning_beside_extension_vocabulary() {
    let bounded = |extra: Value| {
        let mut description = json!({"type": "number", "maximum": 10});
        for (key, value) in extra.as_object().unwrap() {
            description[key] = value.clone();
        }
        description
    };
    for extra in [json!({"acme:note": "x"}), json!({"sdfProtocolMap": {}})] {
        let object = json!({"type": "object", "properties": {"level": bounded(extra.clone())}});
        let items = json!({"type": "array", "items": bounded(extra.clone())});
        let choice = json!({"sdfChoice": {
            "low": bounded(extra.clone()),
            "high": {"type": "number", "minimum": 100, "vendorHint": true}
        }});
        // Each alternative's excluded bound supplies the choice's rejection.
        for (description, admitted, rejected, errors) in [
            (
                &object,
                r#"{"level":5}"#,
                r#"{"level":50}"#,
                &["sample.bound"][..],
            ),
            (&items, "[5]", "[5,50]", &["sample.bound"]),
            (&choice, "5", "50", &["sample.bound", "sample.bound"]),
        ] {
            let case = format!("{extra} {description}");
            let checked = check_description(description.clone(), admitted);
            assert_eq!(checked.admission, Admission::Admitted, "{case}");
            assert!(checked.complete, "{case}");
            assert!(checked.names_reading(), "{case}");
            let checked = check_description(description.clone(), rejected);
            assert_eq!(checked.admission, Admission::Rejected, "{case}");
            assert!(checked.complete, "{case}");
            assert_eq!(checked.codes("error"), errors, "{case}");
            assert!(checked.names_reading(), "{case}");
        }
    }
}

#[test]
fn const_and_unique_items_comparisons_hold_beside_extension_vocabulary() {
    let constant = json!({"type": "number", "const": 7, "acme:note": "x"});
    let list = json!({
        "type": "array", "uniqueItems": true, "acme:note": "x",
        "items": {"type": "number", "sdfProtocolMap": {}}
    });
    let fixed = json!({
        "type": "array", "const": [1, 2], "vendorHint": true,
        "items": {"type": "number", "acme:note": "x"}
    });
    for (description, sample, admission, errors) in [
        (&constant, "7.0", Admission::Admitted, &[][..]),
        (&constant, "8", Admission::Rejected, &["sample.const"]),
        (&list, "[1,2]", Admission::Admitted, &[]),
        (&list, "[1,1.0]", Admission::Rejected, &["sample.unique"]),
        (&fixed, "[1,2]", Admission::Admitted, &[]),
        (&fixed, "[1,3]", Admission::Rejected, &["sample.const"]),
    ] {
        let case = format!("{description} {sample}");
        let checked = check_description(description.clone(), sample);
        assert_eq!(checked.admission, admission, "{case}");
        assert!(checked.complete, "{case}");
        assert_eq!(checked.codes("error"), errors, "{case}");
        assert!(checked.names_reading(), "{case}");
    }
}

#[test]
fn unknown_formats_and_base_qualities_outside_their_role_keep_their_outcomes() {
    let with_extension = |mut description: Value, pointer: &str| {
        description.pointer_mut(pointer).unwrap()["acme:note"] = json!("x");
        description
    };
    for (description, pointer, sample) in [
        (
            json!({"type": "string", "format": "email"}),
            "",
            r#""a@example.com""#,
        ),
        (
            json!({"type": "array", "items": {"type": "string", "format": "email"}}),
            "/items",
            r#"["a@example.com"]"#,
        ),
        (
            json!({"type": "array", "items": {"type": "integer", "exclusiveMinimum": 0}}),
            "/items",
            "[1]",
        ),
        (
            json!({"type": "number", "maximum": 10, "title": "t"}),
            "",
            "5",
        ),
    ] {
        let case = format!("{description} {sample}");
        let before = check_description(description.clone(), sample);
        let after = check_description(with_extension(description, pointer), sample);
        assert_eq!(after.admission, before.admission, "{case}");
        assert_eq!(after.admission, Admission::Undetermined, "{case}");
        assert_eq!(after.complete, before.complete, "{case}");
        assert_eq!(
            after.wire["coverage"]["rows"], before.wire["coverage"]["rows"],
            "{case}"
        );
        assert_eq!(after.codes("error"), before.codes("error"), "{case}");
        assert!(!after.names_reading(), "{case}");
    }
}

#[test]
fn recorded_extension_vocabulary_admits_and_rejects_on_the_base_description() {
    // `oma:id` on a Property is Recorded extension vocabulary, following ADR 0038: Recorded
    // extension vocabulary. A Sample report never consults its recorded evidence, so setting
    // it aside relies on the ADR 0037 reading.
    let recorded = temperature(json!({"oma:id": 5700}));
    let admitted = value(&recorded, "20");
    assert_eq!(admitted.admission, Admission::Admitted);
    assert!(admitted.complete);
    assert!(admitted.names_reading());
    assert_eq!(admitted.wire["baseline"]["registry_knowledge"], json!([]));
    let rejected = value(&recorded, "100");
    assert_eq!(rejected.admission, Admission::Rejected);
    assert!(rejected.complete);
    assert_eq!(rejected.codes("error"), ["sample.bound"]);
    assert!(rejected.names_reading());
}

#[test]
fn reports_omit_the_reading_when_no_sample_conclusion_relied_on_it() {
    let plain = value(&temperature(json!({})), "20");
    assert_eq!(plain.admission, Admission::Admitted);
    assert!(!plain.names_reading());
    // Extension vocabulary on the enclosing Object is outside the selected description.
    let mut object = temperature(json!({}));
    object["sdfObject"]["Temperature"]["acme:note"] = json!("x");
    object["sdfObject"]["Temperature"]["oma:id"] = json!(3303);
    let outside = value(&object, "20");
    assert_eq!(outside.admission, Admission::Admitted);
    assert!(!outside.names_reading());
    // So is vocabulary in an optional entry the sample omits.
    let optional = json!({"type": "object", "properties": {
        "level": {"type": "number", "acme:note": "x"}
    }});
    let absent = check_description(optional.clone(), "{}");
    assert_eq!(absent.admission, Admission::Admitted);
    assert!(absent.complete);
    assert!(!absent.names_reading());
    assert!(check_description(optional, r#"{"level":1}"#).names_reading());
    // A document declaring an Extension feature stays deferred as a whole.
    let mut declared = temperature(json!({"acme:note": "x"}));
    declared["info"]["features"] = json!(["acme"]);
    let deferred = value(&declared, "20");
    assert_eq!(deferred.admission, Admission::Undetermined);
    assert!(!deferred.names_reading());
    // So is a description composed from such a document.
    let donor = json!({
        "info": {"features": ["acme"]},
        "namespace": {"d": "https://example.com/d"},
        "defaultNamespace": "d",
        "sdfData": {"Level": {"type": "integer", "maximum": 10, "acme:x": 1}}
    });
    let user = json!({
        "info": {},
        "namespace": {"d": "https://example.com/d"},
        "sdfProperty": {"p": {"sdfRef": "d:#/sdfData/Level", "acme:x": 1}}
    });
    let composed =
        |donor: &Value| check_among(&user, &[donor], SampleKind::Property, "/sdfProperty/p", "5");
    let deferred = composed(&donor);
    assert_eq!(deferred.admission, Admission::Undetermined);
    assert!(!deferred.names_reading());
    let mut undeclared = donor.clone();
    undeclared["info"] = json!({});
    let established = composed(&undeclared);
    assert_eq!(established.admission, Admission::Admitted);
    assert!(established.names_reading());
}
