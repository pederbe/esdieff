use esdieff::{Assessment, Catalogue, CheckRequest, SourceInput};
use esdieff::{AssessmentStatus, Field, Limits, SampleKind, SampleRequest};
use serde_json::Value;

const CONST: &str = "const-consistency/1";
const DEFAULT: &str = "default-consistency/1";
const OPTIONAL_REFERENCE: &[u8] = br##"{"info":{},"sdfData":{"value":{"type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":{"first":{"properties":{"optional":{"sdfRef":"#/sdfData/Absent"}}},"second":{}}}}}
"##;
fn description(qualities: &str) -> Value {
    check(format!(r#"{{"info":{{}},"sdfData":{{"value":{{{qualities}}}}}}}"#).as_bytes())
}
fn status(wire: &Value, area: &str) -> String {
    wire["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == area)
        .unwrap()["status"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn check(bytes: &[u8]) -> Value {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", bytes)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        conditions(&wire, "json.syntax").is_empty(),
        "the fixture must be valid JSON"
    );
    for identity in [
        &wire["request"]["assessment"],
        &wire["request"]["inventory"],
        &wire["coverage"]["requested"]["inventory"],
    ] {
        assert_eq!(identity, "esdieff.expanded.consistency/1");
    }
    assert_eq!(wire["coverage"]["requested"]["total"], 10);
    let rows = wire["coverage"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    let unique: std::collections::BTreeSet<_> =
        rows.iter().map(|row| row["id"].as_str().unwrap()).collect();
    assert_eq!(unique.len(), 10);
    for area in [CONST, DEFAULT] {
        let typed = report.assessment_area_status(area).require().unwrap();
        assert_eq!(status(&wire, area), typed.as_str());
        let basis = &rows.iter().find(|row| row["id"] == area).unwrap()["basis"];
        if matches!(
            typed,
            AssessmentStatus::Undetermined | AssessmentStatus::Unassessed
        ) {
            assert_eq!(basis["kind"], "operational");
            assert!(basis.get("citation").is_none());
        } else {
            assert_eq!(basis["kind"], "advice");
        }
    }
    wire
}

fn incomplete_default_exclusion(wire: &Value) {
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(wire["outcome"]["complete"], false);
    assert!(!conditions(wire, "reference.unavailable").is_empty());
    assert!(!conditions(wire, "interpretation.unresolved").is_empty());
    let findings = conditions(wire, "consistency.default_excluded");
    assert_eq!(findings.len(), 1);
    let finding = findings[0];
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["basis"]["kind"], "advice");
    assert_eq!(finding["assessment_unit"], DEFAULT);
    assert_eq!(finding["primary"]["kind"], "source");
    assert_eq!(finding["primary"]["pointer"], "/sdfData/value/default");
    let related = finding["related"].as_array().unwrap();
    assert!(
        related
            .iter()
            .any(|row| row["kind"] == "source" && row["pointer"] == "/sdfData/value/required/0")
    );
    assert!(related.iter().any(|row| {
        row["kind"] == "effective"
            && row["pointer"].as_str().is_some_and(|pointer| {
                pointer.starts_with("/sdfData/value/sdfChoice/") && pointer.ends_with("/required/0")
            })
    }));
    assert!(related.iter().any(|row| {
        row["kind"] == "effective"
            && row["pointer"].as_str().is_some_and(|pointer| {
                pointer.starts_with("/sdfData/value/sdfChoice/")
                    && !pointer.ends_with("/required/0")
            })
    }));
}

#[test]
fn independent_missing_required_exclusion_survives_absent_optional_reference() {
    let wire = check(OPTIONAL_REFERENCE);
    incomplete_default_exclusion(&wire);
}

#[test]
fn an_admitting_required_override_clears_other_branch_exclusions() {
    let wire = description(
        r##""type":"object","default":{"b":1},"required":["a"],"properties":{"a":{"type":"number"},"b":{"type":"number"}},"sdfChoice":{"first":{"properties":{"optional":{"sdfRef":"#/sdfData/Absent"}}},"second":{"required":["b"]}}"##,
    );
    assert!(conditions(&wire, "consistency.default_excluded").is_empty());
    assert_eq!(wire["outcome"]["complete"], false);
    assert!(!conditions(&wire, "reference.unavailable").is_empty());
}

#[test]
fn unavailable_or_needed_unknown_branches_cannot_be_excluded() {
    for qualities in [
        r##""type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":{"first":{},"second":{"sdfRef":"#/sdfData/Absent"}}"##,
        r#""type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":{"first":{},"second":{"type":"vendor"}}"#,
        r#""type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":{"first":{},"second":false}"#,
        r#""type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":false"#,
    ] {
        let wire = description(qualities);
        assert!(
            conditions(&wire, "consistency.default_excluded").is_empty(),
            "{qualities}"
        );
        assert_eq!(status(&wire, DEFAULT), "undetermined", "{qualities}");
        assert_eq!(wire["outcome"]["complete"], false, "{qualities}");
    }
}

#[test]
fn nested_established_alternatives_preserve_one_owner_default_exclusion() {
    let wire = description(
        r##""type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":{"outer":{"sdfChoice":{"first":{"properties":{"optional":{"sdfRef":"#/sdfData/Absent"}}},"second":{}}},"other":{}}"##,
    );
    incomplete_default_exclusion(&wire);
}

#[test]
fn present_unknown_property_preserves_independent_required_exclusion() {
    let wire = description(
        r##""type":"object","default":{"optional":1},"required":["a"],"properties":{"a":{"type":"number"},"optional":{"sdfRef":"#/sdfData/Absent"}},"sdfChoice":{"first":{},"second":{}}"##,
    );
    incomplete_default_exclusion(&wire);
    assert_eq!(status(&wire, DEFAULT), "undetermined");
}

#[test]
fn aggregate_exclusion_count_survives_omission_and_strict_policy() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", OPTIONAL_REFERENCE)],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    for strict in [false, true] {
        let report = catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::ExpandedConsistency)
                    .strict_warnings(strict),
            )
            .unwrap();
        assert_eq!(report.outcome().warnings(), 1);
        assert_eq!(report.outcome().errors(), 0);
        assert!(!report.outcome().complete());
        assert_eq!(
            report.outcome().disposition().exit_code(),
            if strict { 1 } else { 3 }
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["requested"]["total"], 10);
        assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
        assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() >= 1);
    }
}

#[test]
fn local_owner_reference_failure_cannot_use_aggregate_eligibility() {
    let wire = description(
        r##""sdfRef":"#/sdfData/Absent","type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}},"sdfChoice":{"first":{},"second":{}}"##,
    );
    assert!(conditions(&wire, "consistency.default_excluded").is_empty());
    for area in [CONST, DEFAULT] {
        assert_eq!(status(&wire, area), "undetermined");
    }
    assert_eq!(wire["outcome"]["complete"], false);
    assert!(!conditions(&wire, "reference.unavailable").is_empty());
}

fn deferred_descendants(wire: &Value) {
    assert!(conditions(wire, "consistency.const_excluded").is_empty());
    assert!(conditions(wire, "consistency.default_excluded").is_empty());
    assert_eq!(wire["outcome"]["complete"], false);
    assert!(!conditions(wire, "interpretation.unresolved").is_empty());
    for area in [CONST, DEFAULT] {
        assert_eq!(status(wire, area), "undetermined");
    }
}

fn established_nested_default(wire: &Value, pointer: &str) {
    assert_eq!(status(wire, CONST), "not_applicable");
    assert_eq!(status(wire, DEFAULT), "assessed");
    assert!(conditions(wire, "consistency.const_excluded").is_empty());
    let findings = conditions(wire, "consistency.default_excluded");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["severity"], "warning");
    assert_eq!(findings[0]["basis"]["kind"], "advice");
    assert_eq!(findings[0]["primary"]["kind"], "source");
    assert_eq!(findings[0]["primary"]["pointer"], pointer);
}

#[test]
fn coverage_unavailable_direct_item_defers_hidden_full_properties() {
    let wire = description(
        r##""type":"array","items":{"sdfRef":"#/sdfData/Absent","type":"object","properties":{"a":{"type":"number","default":1,"minimum":2}}}"##,
    );
    deferred_descendants(&wire);
}

#[test]
fn coverage_established_direct_item_reaches_full_properties() {
    let wire = description(
        r#""type":"array","items":{"type":"object","properties":{"a":{"type":"number","default":1,"minimum":2}}}"#,
    );
    established_nested_default(&wire, "/sdfData/value/items/properties/a/default");
}

#[test]
fn coverage_unavailable_donor_defers_inherited_full_properties() {
    let wire = description(
        r#""type":"array","items":{"type":"string","properties":{"a":{"type":"number","default":1,"minimum":2}},"sdfChoice":{"branch":{"type":"object"}}}"#,
    );
    deferred_descendants(&wire);
}

#[test]
fn coverage_full_override_establishes_properties_after_unknown_donor() {
    let wire = description(
        r#""type":"array","items":{"type":"string","properties":{"a":{"type":"number","default":1,"minimum":2}},"sdfChoice":{"branch":{"type":"object","properties":{"a":{"type":"number","default":1,"minimum":2}}}}}"#,
    );
    established_nested_default(
        &wire,
        "/sdfData/value/items/sdfChoice/branch/properties/a/default",
    );
}

#[test]
fn coverage_failed_direct_item_alternatives_defer_hidden_full_candidates() {
    let wire = description(
        r#""type":"array","items":{"type":"string","enum":["x"],"sdfChoice":{"branch":{"type":"string","default":1}}}"#,
    );
    let coexistence = conditions(&wire, "data.enum_choice");
    assert_eq!(coexistence.len(), 1);
    assert_eq!(coexistence[0]["severity"], "error");
    assert_eq!(coexistence[0]["basis"]["kind"], "rfc");
    deferred_descendants(&wire);
}

#[test]
fn coverage_established_direct_item_alternatives_reach_full_candidates() {
    let wire = description(
        r#""type":"array","items":{"type":"string","sdfChoice":{"branch":{"type":"string","default":1}}}"#,
    );
    assert!(conditions(&wire, "data.enum_choice").is_empty());
    established_nested_default(&wire, "/sdfData/value/items/sdfChoice/branch/default");
}

#[test]
fn coverage_complete_absence_and_opaque_content_stay_not_applicable() {
    let absent = description(
        r#""type":"array","items":{"type":"object","properties":{"a":{"type":"number"}}}"#,
    );
    let opaque = check(br##"{"info":{},"x:opaque":{"sdfRef":"#/sdfData/Absent","default":1,"sdfChoice":false},"sdfData":{"value":{"type":"number"}}}"##);
    for wire in [absent, opaque] {
        for area in [CONST, DEFAULT] {
            assert_eq!(status(&wire, area), "not_applicable");
        }
        assert!(conditions(&wire, "consistency.const_excluded").is_empty());
        assert!(conditions(&wire, "consistency.default_excluded").is_empty());
        assert!(conditions(&wire, "reference.unavailable").is_empty());
    }
}

#[test]
fn coverage_unavailable_sibling_preserves_independent_warning_counts() {
    let wire = check(br##"{"info":{},"sdfData":{"a":{"type":"boolean","const":1},"b":{"sdfRef":"#/sdfData/Absent","type":"number","default":1}}}"##);
    assert_eq!(wire["outcome"]["warnings"], 2);
    assert_eq!(wire["outcome"]["complete"], false);
    assert!(!conditions(&wire, "reference.unavailable").is_empty());
    assert!(conditions(&wire, "consistency.default_excluded").is_empty());
    let independent = conditions(&wire, "consistency.const_excluded");
    assert_eq!(independent.len(), 1);
    assert_eq!(independent[0]["primary"]["pointer"], "/sdfData/a/const");
    for area in [CONST, DEFAULT] {
        assert_eq!(status(&wire, area), "undetermined");
    }
}

#[test]
fn null_and_documentary_defaults_remain_independent() {
    for (qualities, constants, defaults) in [
        (r#""type":"boolean","const":null"#, 0, 0),
        (r#""type":"boolean","nullable":false,"const":null"#, 1, 0),
        (r#""type":"boolean","default":1"#, 0, 1),
        (r#""type":"number","const":1,"default":2"#, 0, 1),
        (r#""type":"number","default":null"#, 0, 0),
        (r#""type":"number","nullable":false,"default":null"#, 0, 1),
        (r#""type":"number""#, 0, 0),
    ] {
        let wire = description(qualities);
        assert_eq!(
            conditions(&wire, "consistency.const_excluded").len(),
            constants,
            "{qualities}"
        );
        assert_eq!(
            conditions(&wire, "consistency.default_excluded").len(),
            defaults,
            "{qualities}"
        );
        assert_eq!(wire["outcome"]["errors"], 0, "{qualities}");
    }
    assert_eq!(
        status(&description(r#""type":"number""#), DEFAULT),
        "not_applicable"
    );
}

#[test]
fn exact_scalar_bounds_and_unicode_lengths_use_declared_values() {
    for (qualities, excluded) in [
        (
            r#""type":"integer","const":9007199254740993,"minimum":9007199254740994"#,
            true,
        ),
        (
            r#""type":"integer","const":10e-1,"minimum":1,"maximum":1"#,
            false,
        ),
        (r#""type":"number","const":-2,"exclusiveMinimum":-2"#, true),
        (r#""type":"number","const":0.3,"multipleOf":0.1"#, false),
        (r#""type":"number","const":0.2,"multipleOf":0.3"#, true),
        (r#""type":"string","const":"\u00e9","maxLength":1"#, false),
        (
            r#""type":"string","const":"\ud83d\ude00a","maxLength":1"#,
            true,
        ),
    ] {
        let wire = description(qualities);
        assert_eq!(
            conditions(&wire, "consistency.const_excluded").len(),
            usize::from(excluded),
            "{qualities}"
        );
        assert_eq!(status(&wire, CONST), "assessed", "{qualities}");
    }
}

#[test]
fn compound_failures_count_one_warning_and_preserve_decisive_leaves() {
    let wire = description(
        r#""type":"object","const":{"a":0,"b":0},"properties":{"a":{"type":"number","minimum":1},"b":{"type":"number","minimum":1}}"#,
    );
    let warnings = conditions(&wire, "consistency.const_excluded");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["primary"]["pointer"], "/sdfData/value/const/a");
    assert!(
        warnings[0]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|location| location["pointer"] == "/sdfData/value/properties/a/minimum")
    );
    let required = description(
        r#""type":"object","default":{},"required":["a"],"properties":{"a":{"type":"number"}}"#,
    );
    assert_eq!(
        conditions(&required, "consistency.default_excluded").len(),
        1
    );
    assert_eq!(
        conditions(&required, "consistency.default_excluded")[0]["primary"]["pointer"],
        "/sdfData/value/default"
    );
    let optional = description(
        r#""type":"object","default":{},"properties":{"a":{"type":"number","minimum":1}}"#,
    );
    assert!(conditions(&optional, "consistency.default_excluded").is_empty());
}

#[test]
fn array_items_counts_and_exact_uniqueness_retain_both_positions() {
    for qualities in [
        r#""type":"array","const":[0,0],"items":{"type":"number","minimum":1}"#,
        r#""type":"array","const":[1],"minItems":2"#,
    ] {
        assert_eq!(
            conditions(&description(qualities), "consistency.const_excluded").len(),
            1
        );
    }
    let wire = description(r#""type":"array","const":[1,1e0],"uniqueItems":true"#);
    let finding = conditions(&wire, "consistency.const_excluded")[0];
    assert_eq!(finding["primary"]["pointer"], "/sdfData/value/const/1");
    for pointer in [
        "/sdfData/value/const/0",
        "/sdfData/value/const/1",
        "/sdfData/value/uniqueItems",
    ] {
        assert!(
            finding["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["kind"] == "effective" && row["pointer"] == pointer),
            "{pointer}"
        );
    }
}

#[test]
fn a_choice_requires_one_complete_witness_and_unknown_can_admit() {
    let qualities = r#""type":"object","default":{"a":1,"b":1},"sdfChoice":{"first":{"properties":{"a":{"type":"number","minimum":2}}},"second":{"properties":{"b":{"type":"number","minimum":2}}}}"#;
    assert_eq!(
        conditions(&description(qualities), "consistency.default_excluded").len(),
        1
    );
    let admitting = description(
        r#""type":"number","default":1,"sdfChoice":{"reject":{"minimum":2},"admit":{"maximum":2}}"#,
    );
    assert!(conditions(&admitting, "consistency.default_excluded").is_empty());
    assert_eq!(status(&admitting, DEFAULT), "assessed");
    let unknown = description(
        r#""type":"number","default":1,"sdfChoice":{"reject":{"minimum":2},"unknown":{"type":"vendor"}}"#,
    );
    assert!(conditions(&unknown, "consistency.default_excluded").is_empty());
    assert_eq!(status(&unknown, DEFAULT), "undetermined");
}

#[test]
fn enum_owner_default_is_once_and_final_constants_are_distinct_candidates() {
    let wire = description(r#""type":"string","enum":["on","off"],"default":"on""#);
    assert!(conditions(&wire, "consistency.default_excluded").is_empty());
    assert_eq!(status(&wire, DEFAULT), "assessed");
    let excluded = description(r#""type":"string","enum":["on","off"],"default":"other""#);
    assert_eq!(
        conditions(&excluded, "consistency.default_excluded").len(),
        1
    );
    let constants = description(r#""type":"string","enum":["on","on"],"maxLength":1"#);
    let findings = conditions(&constants, "consistency.const_excluded");
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["primary"]["pointer"], "/sdfData/value/enum/0");
    assert_eq!(findings[1]["primary"]["pointer"], "/sdfData/value/enum/1");
    let overridden = description(
        r#""type":"string","const":1,"sdfChoice":{"a":{"const":"a"},"b":{"const":"b"}}"#,
    );
    assert!(conditions(&overridden, "consistency.const_excluded").is_empty());
}

#[test]
fn branch_reference_default_is_local_even_with_inherited_content_origin() {
    let wire = check(br##"{"info":{},"sdfData":{"donor":{"type":"number","default":1}},"sdfProperty":{"p":{"type":"number","sdfChoice":{"branch":{"sdfRef":"#/sdfData/donor","minimum":2}}}}}"##);
    let warnings = conditions(&wire, "consistency.default_excluded");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["primary"]["pointer"], "/sdfData/donor/default");
    assert!(
        warnings[0]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|location| location["pointer"] == "/sdfProperty/p/sdfChoice/branch/default")
    );
}

#[test]
fn merge_patch_values_use_actual_local_leaf_and_tombstones_restore_shared_default() {
    let wire = check(br##"{"info":{},"sdfData":{"Base":{"type":"object","const":{"a":1},"properties":{"b":{"type":"number","minimum":3}}}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Base","const":{"b":2}}}}"##);
    let warnings = conditions(&wire, "consistency.const_excluded");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["primary"]["pointer"], "/sdfProperty/p/const/b");
    assert!(
        warnings[0]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|location| location["pointer"] == "/sdfProperty/p/sdfRef")
    );
    let tombstone = check(br##"{"info":{},"sdfData":{"Base":{"type":"number","default":3}},"sdfProperty":{"p":{"type":"number","default":1,"minimum":2,"sdfChoice":{"branch":{"sdfRef":"#/sdfData/Base","default":null}}}}}"##);
    assert_eq!(
        conditions(&tombstone, "consistency.default_excluded").len(),
        1
    );
}

#[test]
fn unsupported_needed_meaning_is_visible_without_erasing_independent_advice() {
    for qualities in [
        r#""type":"string","default":"x","pattern":"(?<x>a)|(?<x>b)""#,
        r#""type":"string","default":"x","format":"future-format""#,
        r#""type":"array","default":[1],"items":{"type":"array"}"#,
    ] {
        let wire = description(qualities);
        assert_eq!(status(&wire, DEFAULT), "undetermined", "{qualities}");
        assert!(conditions(&wire, "consistency.default_excluded").is_empty());
    }
    // Undeclared extension vocabulary is not needed meaning, following ADR 0037:
    // Undeclared extension vocabulary.
    let extended = description(r#""type":"number","default":1,"x:meaning":true"#);
    assert_eq!(status(&extended, DEFAULT), "assessed");
    assert!(conditions(&extended, "consistency.default_excluded").is_empty());
    let independent =
        description(r#""type":"string","const":"xx","maxLength":1,"pattern":"(?<x>a)|(?<x>b)""#);
    assert_eq!(
        conditions(&independent, "consistency.const_excluded").len(),
        1
    );
    assert_eq!(status(&independent, CONST), "undetermined");
    let critical = check(br#"{"info":{"features":["urn:unknown:critical"]},"sdfData":{"value":{"type":"number","default":1}}}"#);
    assert_eq!(status(&critical, DEFAULT), "undetermined");
    let critical_exclusion = check(br#"{"info":{"features":["urn:unknown:critical"]},"sdfData":{"value":{"type":"boolean","default":1}}}"#);
    assert!(conditions(&critical_exclusion, "consistency.default_excluded").is_empty());
    assert_eq!(status(&critical_exclusion, DEFAULT), "undetermined");
}

#[test]
fn original_sample_role_and_source_bytes_stay_intact() {
    let bytes = br#"{"info":{},"sdfProperty":{"p":{"type":"number","default":1,"minimum":2}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", bytes),
        SourceInput::supplied("sample", b"1").sample(),
    ])
    .unwrap();
    let model = catalogue.source_id("model").unwrap();
    assert!(matches!(
        catalogue.source(model).unwrap().sample_value(),
        Field::Unavailable(_)
    ));
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert_eq!(report.outcome().warnings(), 1);
    assert_eq!(catalogue.sources().count(), 2);
    let sample = catalogue
        .check_sample(
            SampleRequest::new(model, SampleKind::Property, "/sdfProperty/p")
                .sample(catalogue.source_id("sample").unwrap()),
        )
        .unwrap();
    assert_eq!(
        sample.sample_result().unwrap().admission(),
        esdieff::Admission::Rejected
    );
    assert_eq!(
        catalogue
            .source(model)
            .unwrap()
            .json()
            .require()
            .unwrap()
            .raw_json()
            .as_bytes(),
        bytes
    );
}

#[test]
fn zero_detail_preserves_two_warnings_and_strict_changes_only_disposition() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"boolean","const":1}}}"#;
    for detail in [1, 1024 * 1024] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("model", bytes)],
            Limits::default().diagnostic_bytes(detail),
        )
        .unwrap();
        let normal = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        let strict = catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::ExpandedConsistency)
                    .strict_warnings(true),
            )
            .unwrap();
        assert_eq!(normal.outcome().warnings(), 2);
        assert_eq!(strict.outcome().warnings(), 2);
        assert_eq!(normal.outcome().complete(), strict.outcome().complete());
        assert_ne!(
            normal.outcome().disposition(),
            strict.outcome().disposition()
        );
        if detail == 1 {
            let wire: Value = serde_json::from_str(&normal.to_json()).unwrap();
            assert!(wire["diagnostics"]["items"].as_array().unwrap().is_empty());
            assert_eq!(wire["diagnostics"]["omitted"], 2);
        }
    }
}

#[test]
fn equality_pairs_are_shared_across_model_candidates_and_requests_restart_cleanly() {
    let bytes = br#"{"info":{},"sdfData":{"a":{"type":"number","const":1,"default":2},"b":{"type":"number","const":1,"default":2}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("m", bytes)],
        Limits::default().equality_pairs(2),
    )
    .unwrap();
    for _ in 0..2 {
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        assert_eq!(report.outcome().warnings(), 1);
        assert!(!report.outcome().complete());
        assert_eq!(
            report.assessment_area_status(DEFAULT).require().unwrap(),
            AssessmentStatus::Undetermined
        );
        assert_eq!(
            report.assessment_area_status(CONST).require().unwrap(),
            AssessmentStatus::Undetermined
        );
    }
    let enough = Catalogue::with_limits(
        &[SourceInput::supplied("m", bytes)],
        Limits::default().equality_pairs(4),
    )
    .unwrap();
    let report = enough
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert_eq!(report.outcome().warnings(), 2);
    assert_eq!(
        report.assessment_area_status(DEFAULT).require().unwrap(),
        AssessmentStatus::Assessed
    );
}

#[test]
fn limits_defer_needed_work_and_preserve_independent_warnings_and_denominator() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"boolean","const":1,"default":2}}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("m", bytes)],
        Limits::default().numeric_work(1),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert_eq!(report.outcome().warnings(), 3);
    assert!(!report.outcome().complete());
    assert_eq!(report.requested_coverage().total(), Some(10));
    assert_eq!(
        report.assessment_area_status(DEFAULT).require().unwrap(),
        AssessmentStatus::Undetermined
    );
    let source_limited = Catalogue::with_limits(
        &[SourceInput::supplied("m", bytes)],
        Limits::default().effective_nodes(1),
    )
    .unwrap();
    let report = source_limited
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert_eq!(report.requested_coverage().total(), Some(10));
    for area in [CONST, DEFAULT] {
        assert_eq!(
            report.assessment_area_status(area).require().unwrap(),
            AssessmentStatus::Undetermined
        );
    }
    let no_locations = Catalogue::with_limits(
        &[SourceInput::supplied("m", bytes)],
        Limits::default().pointer_output_bytes(1),
    )
    .unwrap();
    let report = no_locations
        .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
        .unwrap();
    assert_eq!(report.outcome().warnings(), 3);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 10);
    assert!(
        wire["diagnostics"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["field"] == "diagnostics.items.primary.pointer")
    );
}

#[test]
fn inherited_constants_and_consumers_preserve_distinct_candidate_cardinality() {
    let shared = description(
        r#""type":"number","const":1,"sdfChoice":{"a":{"minimum":2},"b":{"minimum":3}}"#,
    );
    let findings = conditions(&shared, "consistency.const_excluded");
    assert_eq!(findings.len(), 2);
    for finding in findings {
        assert_eq!(finding["primary"]["pointer"], "/sdfData/value/const");
    }
    let consumers = check(br##"{"info":{},"sdfData":{"Base":{"type":"number","default":1}},"sdfProperty":{"a":{"sdfRef":"#/sdfData/Base","minimum":2},"b":{"sdfRef":"#/sdfData/Base","minimum":3}}}"##);
    let findings = conditions(&consumers, "consistency.default_excluded");
    assert_eq!(findings.len(), 2);
    let mut seen = std::collections::BTreeSet::new();
    for finding in findings {
        assert_eq!(finding["primary"]["pointer"], "/sdfData/Base/default");
        let related = finding["related"].as_array().unwrap();
        let consumer = related
            .iter()
            .find(|row| {
                row["kind"] == "source"
                    && row["pointer"]
                        .as_str()
                        .is_some_and(|pointer| pointer.ends_with("/sdfRef"))
            })
            .unwrap()["pointer"]
            .as_str()
            .unwrap();
        let owner = consumer.strip_suffix("/sdfRef").unwrap();
        assert!(
            related
                .iter()
                .any(|row| row["kind"] == "effective" && row["pointer"] == owner)
        );
        assert!(seen.insert(consumer.to_owned()));
    }
    assert_eq!(
        seen,
        [
            "/sdfProperty/a/sdfRef".to_owned(),
            "/sdfProperty/b/sdfRef".to_owned()
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn consistency_selection_keeps_every_other_profile_inventory_unchanged() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"type":"boolean","const":1}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", bytes)]).unwrap();
    for (profile, total) in [
        (Assessment::Initial, 8),
        (Assessment::ExpandedModified, 9),
        (Assessment::ExpandedUnits, 10),
        (Assessment::ExpandedContentFormat, 10),
        (Assessment::ExpandedPatternSyntax, 9),
        (Assessment::ExpandedNumeric, 9),
        (Assessment::FullBaseline, 13),
    ] {
        let report = catalogue
            .check(CheckRequest::new().assessment(profile))
            .unwrap();
        assert_eq!(report.requested_coverage().total(), Some(total));
        for area in [CONST, DEFAULT] {
            assert!(matches!(report.assessment_area_status(area), Field::Absent));
        }
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert!(conditions(&wire, "consistency.const_excluded").is_empty());
        assert_eq!(report.outcome().warnings(), 1);
    }
}
fn conditions<'a>(wire: &'a Value, code: &str) -> Vec<&'a Value> {
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["code"] == code)
        .collect()
}
#[test]
fn primitive_exclusion_preserves_initial_warning_and_adds_one_candidate_warning() {
    let wire = check(br#"{"info":{},"sdfData":{"value":{"type":"boolean","const":1}}}"#);
    assert_eq!(conditions(&wire, "consistency.const_excluded").len(), 1);
    assert_eq!(wire["outcome"]["warnings"], 2);
    let finding = conditions(&wire, "consistency.const_excluded")[0];
    assert_eq!(finding["primary"]["kind"], "source");
    assert_eq!(finding["primary"]["pointer"], "/sdfData/value/const");
    assert_eq!(finding["basis"]["kind"], "advice");
}
