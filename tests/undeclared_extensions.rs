//! Undeclared extension vocabulary leaves the base meaning of its definition
//! intact, and reports name the reading whenever a conclusion relied on it.
//! Follows ADR 0037: Undeclared extension vocabulary.
mod undeclared_extension_support;
use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, InspectRequest, RecordedRegistryPolicy,
    SourceInput, View,
};
use serde_json::{Value, json};
use undeclared_extension_support::temperature;

/// The observable result of one check.
struct Checked {
    wire: Value,
    requirements: Vec<AssessmentStatus>,
}

impl Checked {
    fn status(&self, member: usize) -> AssessmentStatus {
        self.requirements[member - 1]
    }
    fn area(&self, id: &str) -> &Value {
        &self.wire["coverage"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("area {id}"))["status"]
    }
    fn disposition(&self) -> &Value {
        &self.wire["outcome"]["disposition"]
    }
    /// The codes of retained conditions with `severity`.
    fn codes(&self, severity: &str) -> Vec<&str> {
        undeclared_extension_support::codes(&self.wire, severity)
    }
    fn names_reading(&self) -> bool {
        undeclared_extension_support::names_reading(&self.wire)
    }
}

/// Checks `sources` with the request that `request` builds for their catalogue.
fn check_sources(
    sources: &[(&str, &Value)],
    request: impl FnOnce(&Catalogue) -> CheckRequest,
) -> Checked {
    let bytes: Vec<_> = sources
        .iter()
        .map(|(key, document)| (*key, serde_json::to_vec(document).unwrap()))
        .collect();
    let inputs: Vec<_> = bytes
        .iter()
        .map(|(key, text)| SourceInput::supplied(key, text))
        .collect();
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let report = catalogue.check(request(&catalogue)).unwrap();
    let requirements = (1..=54)
        .map(|member| {
            report
                .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
                .present()
                .unwrap_or(AssessmentStatus::NotApplicable)
        })
        .collect();
    Checked {
        wire: serde_json::from_str(&report.to_json()).unwrap(),
        requirements,
    }
}

fn check(document: &Value, assessment: Assessment) -> Checked {
    check_sources(&[("model", document)], |_| {
        CheckRequest::new().assessment(assessment)
    })
}

fn full(document: &Value) -> Checked {
    check(document, Assessment::FullBaseline)
}

#[test]
fn a_protocol_mapping_keeps_the_scalar_meaning_of_its_property() {
    let plain = full(&temperature(json!({})));
    assert_eq!(plain.disposition(), "passed");
    assert_eq!(plain.status(28), AssessmentStatus::NotApplicable);
    assert!(!plain.names_reading());

    let mapped = full(&temperature(
        json!({"sdfProtocolMap": {"ble": {"serviceID": "1809"}}}),
    ));
    for member in [42, 45] {
        assert_eq!(
            mapped.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert_eq!(mapped.area("full.scalar-meaning/1"), "assessed");
    // The mapping's own meaning stays unsupported.
    assert_eq!(mapped.status(28), AssessmentStatus::Undetermined);
    assert_eq!(mapped.area("full.critical-extensions/1"), "undetermined");
    assert_eq!(mapped.codes("warning"), ["quality.unfamiliar"]);
    assert!(mapped.codes("error").is_empty());
    assert_eq!(mapped.disposition(), "undetermined");
    assert!(mapped.names_reading());
}

#[test]
fn an_unregistered_qualified_name_keeps_base_meaning_with_its_notice() {
    let noted = full(&temperature(
        json!({"acme:note": "calibrated", "const": 20}),
    ));
    for member in [42, 43, 45] {
        assert_eq!(
            noted.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert_eq!(noted.status(28), AssessmentStatus::Undetermined);
    assert_eq!(noted.codes("notice"), ["extension.optional_unsupported"]);
    assert!(noted.codes("warning").is_empty());
    assert!(noted.names_reading());
}

#[test]
fn a_misspelled_base_quality_is_reported_without_blocking_other_conclusions() {
    let misspelled = temperature(json!({"minimun": 0}));
    let lenient = full(&misspelled);
    assert_eq!(lenient.codes("warning"), ["quality.unfamiliar"]);
    for member in [42, 45] {
        assert_eq!(
            lenient.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert_eq!(lenient.disposition(), "undetermined");
    assert!(lenient.names_reading());
    // Initial was never blocked and passes with the warning, or fails when warnings are strict.
    let initial = |strict| {
        check_sources(&[("model", &misspelled)], |_| {
            CheckRequest::new()
                .assessment(Assessment::Initial)
                .strict_warnings(strict)
        })
    };
    assert_eq!(initial(false).disposition(), "passed");
    let strict = initial(true);
    assert_eq!(strict.codes("warning"), ["quality.unfamiliar"]);
    assert_eq!(strict.disposition(), "failed");
    let strict_full = check_sources(&[("model", &misspelled)], |_| {
        CheckRequest::new()
            .assessment(Assessment::FullBaseline)
            .strict_warnings(true)
    });
    assert_eq!(strict_full.disposition(), "failed");
}

#[test]
fn a_declared_extension_feature_still_defers_its_document_and_importers() {
    let donor = json!({
        "info": {"features": ["acme"]},
        "namespace": {"d": "https://example.com/d"},
        "defaultNamespace": "d",
        "sdfData": {"D": {"type": "integer", "minimum": 0, "acme:x": 1}}
    });
    let declaring = full(&donor);
    let user = json!({
        "info": {},
        "namespace": {"d": "https://example.com/d"},
        "sdfData": {"U": {"sdfRef": "d:#/sdfData/D", "acme:x": 1}}
    });
    let importing = check_sources(&[("donor", &donor), ("user", &user)], |catalogue| {
        CheckRequest::new()
            .root(catalogue.source_id("user").unwrap())
            .assessment(Assessment::FullBaseline)
    });
    for checked in [&declaring, &importing] {
        for member in [42, 45] {
            assert_eq!(
                checked.status(member),
                AssessmentStatus::Undetermined,
                "M{member}"
            );
        }
        assert!(
            checked
                .codes("incomplete")
                .contains(&"structure.feature_context_unavailable")
        );
        assert!(!checked.names_reading());
    }
    assert!(
        declaring
            .codes("incomplete")
            .contains(&"extension.critical_unsupported")
    );
}

#[test]
fn base_qualities_outside_their_role_and_unknown_base_values_keep_their_outcomes() {
    let items =
        |item: Value| json!({"info": {}, "sdfData": {"A": {"type": "array", "items": item}}});
    let wider = full(&items(json!({"type": "integer", "exclusiveMinimum": 0})));
    for member in [41, 42, 50] {
        assert_eq!(
            wider.status(member),
            AssessmentStatus::Undetermined,
            "M{member}"
        );
    }
    assert_eq!(wider.codes("notice"), ["extension.optional_unsupported"]);
    assert!(!wider.names_reading());
    // The same Direct item position ignores undeclared extension vocabulary.
    let extended = full(&items(
        json!({"type": "integer", "minimum": 0, "acme:x": 1}),
    ));
    for member in [42, 45, 50] {
        assert_eq!(
            extended.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert_eq!(extended.status(28), AssessmentStatus::Undetermined);
    assert!(extended.names_reading());

    let email = full(&json!({"info": {}, "sdfData": {"N": {"type": "string", "format": "email"}}}));
    for member in [28, 42, 49] {
        assert_eq!(
            email.status(member),
            AssessmentStatus::Undetermined,
            "M{member}"
        );
    }
    assert_eq!(email.codes("notice"), ["extension.optional_unsupported"]);
    assert!(!email.names_reading());
}

#[test]
fn const_and_unique_items_comparisons_hold_beside_undeclared_vocabulary() {
    let list = json!({"info": {}, "sdfData": {"List": {
        "type": "array", "uniqueItems": true, "const": [1, 2], "acme:note": "x",
        "items": {"type": "number", "sdfProtocolMap": {}}
    }}});
    let checked = full(&list);
    for member in [42, 43, 50, 52] {
        assert_eq!(
            checked.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert_eq!(checked.area("full.compound-meaning/1"), "assessed");
    assert!(checked.names_reading());
}

#[test]
fn numeric_advice_and_contradictions_hold_beside_undeclared_vocabulary() {
    let document = json!({"info": {}, "sdfData": {
        "Empty": {"acme:note": "x", "type": "integer", "minimum": 5, "maximum": 3, "nullable": false},
        "Step": {"vendorHint": "x", "type": "integer", "minimum": 1, "maximum": 2, "multipleOf": 5, "nullable": false}
    }});
    // Contradiction warnings never consulted the vocabulary, so Initial relies on nothing.
    let initial = check(&document, Assessment::Initial);
    assert!(
        initial
            .codes("warning")
            .contains(&"proof.empty_numeric_interval")
    );
    assert!(!initial.names_reading());
    let numeric = check(&document, Assessment::ExpandedNumeric);
    assert_eq!(numeric.area("advisory-numeric-three/1"), "assessed");
    let warnings = numeric.codes("warning");
    for code in ["proof.empty_numeric_interval", "proof.step_interval_gap"] {
        assert!(warnings.contains(&code), "{code}");
    }
    assert!(numeric.names_reading());
}

#[test]
fn declared_value_consistency_holds_beside_undeclared_vocabulary() {
    let document = json!({"info": {}, "sdfData": {"K": {
        "acme:note": "x", "type": "integer", "const": 7, "minimum": 1, "maximum": 3
    }}});
    let consistency = check(&document, Assessment::ExpandedConsistency);
    assert_eq!(consistency.area("const-consistency/1"), "assessed");
    assert!(
        consistency
            .codes("warning")
            .contains(&"consistency.const_excluded")
    );
    assert!(consistency.names_reading());
}

#[test]
fn reports_name_the_reading_only_when_a_conclusion_relied_on_it() {
    let mapped = temperature(json!({"sdfProtocolMap": {}}));
    assert!(full(&mapped).names_reading());
    // Initial consults no base meaning that the vocabulary could have stopped.
    assert!(!check(&mapped, Assessment::Initial).names_reading());
    // No conclusion about an Object's own meaning consults its extension quality.
    let object = json!({"info": {}, "sdfObject": {"Temperature": {
        "acme:note": "x",
        "sdfProperty": {"value": {"type": "number"}}
    }}});
    let checked = full(&object);
    assert_eq!(checked.status(28), AssessmentStatus::Undetermined);
    assert_eq!(checked.status(42), AssessmentStatus::Assessed);
    assert!(!checked.names_reading());
    // A conclusion stopped by a base quality outside its role relies on nothing either.
    let stopped = json!({"info": {}, "sdfData": {"N": {
        "type": "integer", "minimum": 1, "maximum": 2, "multipleOf": 5, "nullable": false,
        "readable": true, "acme:x": 1
    }}});
    let numeric = check(&stopped, Assessment::ExpandedNumeric);
    assert_eq!(numeric.area("advisory-numeric-three/1"), "undetermined");
    assert!(!numeric.names_reading());
    let checked = full(&stopped);
    assert_eq!(checked.status(45), AssessmentStatus::Undetermined);
    assert!(!checked.names_reading());
}

#[test]
fn recorded_extension_vocabulary_names_the_reading_only_where_its_record_went_unconsulted() {
    // `oma:id` on a Property is Recorded extension vocabulary, following ADR 0038:
    // Recorded extension vocabulary. Full consults its recorded evidence.
    let document = temperature(json!({"oma:id": 5700}));
    let recorded = full(&document);
    for member in [28, 42, 45] {
        assert_eq!(
            recorded.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert!(!recorded.names_reading());
    // Without that evidence, conclusions that set it aside rely on the ADR 0037 reading.
    let withheld = check_sources(&[("model", &document)], |_| {
        CheckRequest::new()
            .assessment(Assessment::FullBaseline)
            .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded)
    });
    assert_eq!(withheld.status(28), AssessmentStatus::Undetermined);
    for member in [42, 45] {
        assert_eq!(
            withheld.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert!(withheld.names_reading());
    let stepped = json!({"info": {}, "sdfData": {"Step": {
        "oma:id": 7, "type": "integer", "minimum": 1, "maximum": 2, "multipleOf": 5,
        "nullable": false
    }}});
    let numeric = check(&stepped, Assessment::ExpandedNumeric);
    assert_eq!(numeric.area("advisory-numeric-three/1"), "assessed");
    assert!(
        numeric
            .codes("warning")
            .contains(&"proof.step_interval_gap")
    );
    assert!(numeric.names_reading());
    // Initial consults no base meaning that the vocabulary could have stopped.
    assert!(!check(&stepped, Assessment::Initial).names_reading());
    // Any other `oma:` name stays undeclared extension vocabulary.
    let other = full(&temperature(json!({"oma:other": 1})));
    assert_eq!(other.status(42), AssessmentStatus::Assessed);
    assert!(other.names_reading());
}

#[test]
fn composition_copies_merges_and_shows_extension_qualities() {
    let document = json!({"info": {}, "sdfObject": {
        "Base": {"sdfProperty": {"value": {
            "type": "number", "minimum": 0,
            "sdfProtocolMap": {"ble": {"serviceID": "1809"}},
            "acme:note": "base"
        }}},
        "Use": {"sdfRef": "#/sdfObject/Base", "sdfProperty": {"value": {"acme:note": "use"}}}
    }});
    let bytes = serde_json::to_vec(&document).unwrap();
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", &bytes)]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(id, "/sdfObject/Use/sdfProperty/value").view(View::Effective))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["result"]["json"],
        json!({
            "type": "number", "minimum": 0,
            "sdfProtocolMap": {"ble": {"serviceID": "1809"}},
            "acme:note": "use"
        })
    );
    let checked = full(&document);
    for member in [42, 45] {
        assert_eq!(
            checked.status(member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert!(checked.names_reading());
}
