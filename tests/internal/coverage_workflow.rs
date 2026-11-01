//! Public checks, requested boundaries and report evolution.
use super::coverage_support;
use coverage_support::semantic_signature;
use coverage_support::{
    INITIAL_AREAS,
    semantic::{allowed_omission, wire_projection},
};
use esdieff::{
    Assessment, AssessmentStatus, BasisKind, Catalogue, CheckRequest, Disposition, Limits,
    Severity, SourceInput,
};
use serde_json::Value;

fn one(text: &[u8], limits: Limits) -> Catalogue {
    Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap()
}

#[test]
fn competing_request_admission_failures_keep_root_policy_and_envelope_precedence() {
    use esdieff::RecordedRegistryPolicy;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("m", b"{}")],
        Limits::default().report_bytes(20_486),
    )
    .unwrap();
    let other = Catalogue::from_inputs(&[
        SourceInput::supplied("first", b"{}"),
        SourceInput::supplied("second", b"{}"),
    ])
    .unwrap();
    let unknown = other.source_id("second").unwrap();
    let incompatible = CheckRequest::new()
        .assessment(Assessment::ExpandedModified)
        .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded);
    assert_eq!(
        catalogue.check(incompatible.clone().root(unknown)).err(),
        Some("unknown or duplicate selected root")
    );
    let oversized = (0..257).fold(incompatible.clone(), |request, _| request.root(unknown));
    assert_eq!(
        catalogue.check(oversized).err(),
        Some("selected roots exceed source-attempt ceiling")
    );
    assert_eq!(
        catalogue.check(incompatible).err(),
        Some("recorded registry policy requires a registry assessment")
    );
    for (assessment, error) in [
        (
            Assessment::FullBaseline,
            "report cannot reserve the registry assessment envelope",
        ),
        (
            Assessment::ExpandedPatternSyntax,
            "report cannot reserve the pattern assessment envelope",
        ),
        (
            Assessment::ExpandedModified,
            "report cannot reserve the expanded assessment envelope",
        ),
    ] {
        assert_eq!(
            catalogue
                .check(CheckRequest::new().assessment(assessment))
                .err(),
            Some(error)
        );
    }
}

fn assert_unfinished_basis(report: &esdieff::Report<'_>, area: &str, code: &str, basis: &str) {
    assert_eq!(
        report.assessment_area_status(area).require().unwrap(),
        AssessmentStatus::Undetermined
    );
    assert!(!report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
    assert!(report.requested_coverage().undetermined() > 0);
    coverage_support::assert_requested_counts(report);
    let diagnostics = report.diagnostics();
    let condition = diagnostics
        .conditions()
        .find(|condition| condition.code() == code && condition.assessment_unit() == Some(area))
        .expect("actual unfinished-area condition");
    assert_eq!(condition.severity(), Severity::Incomplete);
    assert_eq!(condition.basis().kind(), BasisKind::Operational);
    assert_eq!(condition.basis().id(), Some(basis));
    assert_eq!(condition.basis().citation(), None);
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["outcome"]["complete"], false);
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(wire["request"]["scope"]["kind"], "catalogue");
    let condition = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|condition| condition["code"] == code && condition["assessment_unit"] == area)
        .unwrap();
    assert_eq!(condition["severity"], "incomplete");
    assert_eq!(
        condition["basis"],
        serde_json::json!({"kind":"operational","id":basis})
    );
    let row = wire["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == area)
        .unwrap();
    assert_eq!(row["status"], "undetermined");
    assert_eq!(row["basis"], condition["basis"]);
}

#[test]
fn unfinished_advice_retains_its_established_basis() {
    let catalogue = one(
        include_bytes!("../fixtures/advisory/critical.json"),
        Limits::default(),
    );
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_unfinished_basis(
        &report,
        "advisory-four/1",
        "coverage.advisory_unavailable",
        "advisory-establishment",
    );
    assert_eq!(
        report.requested_coverage().inventory(),
        Some("esdieff.initial/1")
    );
    assert_eq!(report.requested_coverage().total(), Some(8));
}

#[test]
fn unfinished_full_document_work_has_a_generic_establishment_basis() {
    let catalogue = one(b"{}", Limits::default());
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_unfinished_basis(
        &report,
        "full.document-syntax/1",
        "coverage.unavailable",
        "assessment-establishment",
    );
    assert_eq!(
        report.requested_coverage().inventory(),
        Some("esdieff.full-model/1")
    );
    assert_eq!(report.requested_coverage().total(), Some(13));
    assert_eq!(
        report.requirement_coverage().require().unwrap().total(),
        Some(54)
    );
    assert_eq!(
        report
            .requirement_status("rfc9880.model.M008/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
}

#[test]
fn unfinished_initial_feature_work_has_a_generic_establishment_basis() {
    let catalogue = one(
        include_bytes!("../fixtures/advisory/critical.json"),
        Limits::default(),
    );
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_unfinished_basis(
        &report,
        "extensions-policies/1",
        "coverage.unavailable",
        "assessment-establishment",
    );
    assert_eq!(
        report.requested_coverage().inventory(),
        Some("esdieff.initial/1")
    );
    assert_eq!(report.requested_coverage().total(), Some(8));
}

#[test]
fn expanded_human_coverage_is_emitted_once_with_specialized_status() {
    for (bytes, profile, area, errors) in [
        (br#"{"info":{"modified":"2016-12-31T23:59:60Z"},"sdfProperty":{"p":{"type":"number"}}}"#.as_slice(), Assessment::ExpandedModified, "modified-date-time/1", 0),
        (br#"{"info":{},"sdfProperty":{"p":{"pattern":"["}}}"#.as_slice(), Assessment::ExpandedPatternSyntax, "pattern-syntax/1", 1),
    ] {
        let catalogue = one(bytes, Limits::default());
        let report = catalogue.check(CheckRequest::new().assessment(profile)).unwrap();
        assert_eq!(report.assessment_area_status(area).require().unwrap(), AssessmentStatus::Assessed);
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().errors(), errors);
        assert_eq!(report.outcome().disposition(), if errors == 0 { Disposition::Passed } else { Disposition::Failed });
        assert_eq!(report.requested_coverage().total(), Some(9));
        coverage_support::assert_requested_counts(&report);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["outcome"]["complete"], true);
        assert_eq!(wire["outcome"]["errors"], errors);
        let human = report.to_human();
        assert_eq!(human.matches("Requested coverage (assessment_area): total ").count(), 1);
        assert!(human.contains(&format!("Assessment area {area}: assessed")));
        if profile == Assessment::ExpandedModified {
            assert!(wire["baseline"]["temporal_knowledge"].is_object());
            assert!(human.contains("Temporal knowledge: "));
            assert!(human.contains("Temporal knowledge SHA256: transitions "));
        }
    }
}

#[test]
fn workflow() {
    let ordinary = one(
        br#"{"info":{},"sdfData":{"D":{"type":"string","pattern":"["}}}"#,
        Limits::default(),
    );
    let initial = ordinary.check(CheckRequest::new()).unwrap();
    assert!(
        initial.outcome().complete(),
        "pattern specialization remains outside initial"
    );
    let full = ordinary
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(!full.outcome().complete());

    // Initial excludes specialized unit grammar; Full requests it.
    let unit = one(
        br#"{"info":{},"sdfData":{"D":{"unit":"urn:ietf:params:unit:Cel"}}}"#,
        Limits::default(),
    );
    assert_eq!(
        unit.check(CheckRequest::new()).unwrap().outcome().errors(),
        0
    );
    let full = unit
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(
        full.diagnostics()
            .conditions()
            .any(|condition| condition.code() == "unit.urn_forbidden")
    );
    assert_eq!(
        full.requirement_status("rfc9880.model.M023/1")
            .require()
            .unwrap(),
        esdieff::AssessmentStatus::Assessed
    );

    let date = one(br#"{"info":{"modified":"2026-02-30"}}"#, Limits::default());
    assert_eq!(
        date.check(CheckRequest::new()).unwrap().outcome().errors(),
        0
    );
    let full = date
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(
        full.diagnostics()
            .conditions()
            .any(|c| c.code() == "metadata.modified_calendar"),
        "Full executes already available requested expanded modified rule"
    );
    assert!(full.outcome().errors() > 0);
    assert!(!full.outcome().complete());

    let proof = one(br#"{"info":{},"sdfData":{"D":{"type":"number","nullable":false,"minimum":2,"exclusiveMaximum":2}}}"#, Limits::default());
    let normal = proof.check(CheckRequest::new()).unwrap();
    let strict = proof
        .check(CheckRequest::new().strict_warnings(true))
        .unwrap();
    assert!(normal.outcome().complete());
    assert!(strict.outcome().complete());
    assert_eq!(normal.outcome().disposition(), Disposition::Passed);
    assert_eq!(strict.outcome().disposition(), Disposition::Failed);
    for report in [&normal, &strict] {
        let diagnostics = report.diagnostics();
        let warning = diagnostics
            .conditions()
            .find(|c| c.code() == "proof.empty_numeric_interval")
            .unwrap();
        assert_eq!(warning.severity(), Severity::Warning);
        assert_eq!(warning.basis().kind(), BasisKind::Advice);
    }
    let again = proof.check(CheckRequest::new()).unwrap();
    assert_eq!(semantic_signature(&normal), semantic_signature(&again));
    let wire: Value = serde_json::from_str(&normal.to_json()).unwrap();
    let mut evolved = wire.clone();
    evolved["future_optional"] = serde_json::json!({"detail":true});
    if let Some(rows) = evolved["diagnostics"]["items"].as_array_mut() {
        rows.reverse();
        for row in rows {
            row["message"] = Value::String("Reworded guidance".into());
        }
    }
    evolved["coverage"]["rows"]
        .as_array_mut()
        .unwrap()
        .reverse();
    for row in evolved["coverage"]["rows"].as_array_mut().unwrap() {
        row["basis"]["future_optional"] = serde_json::json!(true);
    }
    assert_eq!(wire_projection(&wire), wire_projection(&evolved));
    let mut duplicate_area = wire.clone();
    let rows = duplicate_area["coverage"]["rows"].as_array_mut().unwrap();
    rows[1] = rows[0].clone();
    assert!(
        !allowed_omission(&wire, &duplicate_area),
        "duplicate area cannot replace another retained identity"
    );
    let mut coverage_only = wire.clone();
    coverage_only["coverage"]["rows"]
        .as_array_mut()
        .unwrap()
        .pop();
    coverage_only["coverage"]["rows_omitted"] = serde_json::json!(1);
    let record =
        serde_json::json!({"field":"coverage.rows", "reason":"byte_limit", "omitted_count":1});
    coverage_only["coverage"]["omissions"]
        .as_array_mut()
        .unwrap()
        .push(record.clone());
    assert!(
        !allowed_omission(&wire, &coverage_only),
        "coverage-only loss needs truncation/reason linkage"
    );
    coverage_only["diagnostics"]["truncated"] = serde_json::json!(true);
    coverage_only["diagnostics"]["reasons"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("byte_limit"));
    coverage_only["diagnostics"]["omissions"]
        .as_array_mut()
        .unwrap()
        .push(record);
    assert!(allowed_omission(&wire, &coverage_only));
    let mut missing_reason = coverage_only.clone();
    missing_reason["diagnostics"]["reasons"] = serde_json::json!([]);
    assert!(!allowed_omission(&wire, &missing_reason));
    let mut missing_link = coverage_only.clone();
    missing_link["diagnostics"]["omissions"] = serde_json::json!([]);
    assert!(!allowed_omission(&wire, &missing_link));
    assert_eq!(
        wire["diagnostics"]["items"][0]["primary"]["kind"],
        "effective"
    );
    assert!(
        wire["diagnostics"]["items"][0]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|location| location["kind"] == "source")
    );
    for change in [
        "code",
        "basis",
        "coordinate",
        "source_coordinate",
        "arguments",
        "unit",
    ] {
        let mut changed = wire.clone();
        let condition = &mut changed["diagnostics"]["items"][0];
        match change {
            "code" => condition["code"] = Value::String("different.semantic_code".into()),
            "basis" => condition["basis"]["kind"] = Value::String("rfc".into()),
            "coordinate" => {
                condition["primary"]["pointer"] = Value::String("/sdfData/Other".into())
            }
            "source_coordinate" => {
                let location = condition["related"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|location| location["kind"] == "source")
                    .unwrap();
                location["pointer"] = Value::String("/sdfData/Other".into());
            }
            "arguments" => condition["arguments"]["observed_occurrences"] = serde_json::json!(999),
            "unit" => condition["assessment_unit"] = Value::String("other-area/1".into()),
            _ => unreachable!(),
        }
        assert_ne!(
            wire_projection(&wire),
            wire_projection(&changed),
            "{change}"
        );
    }
    let mut pointer_omitted = wire.clone();
    let primary = &mut pointer_omitted["diagnostics"]["items"][0]["primary"];
    assert!(primary["pointer"].is_string());
    let source = primary.get("source_id").unwrap_or(&primary["root"]).clone();
    primary["pointer"] = Value::Null;
    primary["omission"] = serde_json::json!({"field":"diagnostics.items.primary.pointer", "reason":"location_limit", "omitted_count":1, "source_id":source});
    let pointer_record = primary["omission"].clone();
    pointer_omitted["diagnostics"]["truncated"] = serde_json::json!(true);
    pointer_omitted["diagnostics"]["omissions"]
        .as_array_mut()
        .unwrap()
        .push(pointer_record);
    pointer_omitted["diagnostics"]["reasons"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("location_limit"));
    assert!(allowed_omission(&wire, &pointer_omitted));
    pointer_omitted["diagnostics"]["items"][0]["primary"]
        .as_object_mut()
        .unwrap()
        .remove("omission");
    assert!(!allowed_omission(&wire, &pointer_omitted));

    let truncation_input = br#"{"info":{},"sdfData":{"A":{"multipleOf":0},"B":{"multipleOf":0}}}"#;
    let verbose = one(truncation_input, Limits::default());
    let terse = one(truncation_input, Limits::default().diagnostic_bytes(1));
    let verbose = verbose.check(CheckRequest::new()).unwrap();
    let terse = terse.check(CheckRequest::new()).unwrap();
    for area in INITIAL_AREAS {
        assert_eq!(
            format!(
                "{:?}",
                verbose.assessment_area_status(area).require().unwrap()
            ),
            format!(
                "{:?}",
                terse.assessment_area_status(area).require().unwrap()
            )
        );
    }
    let verbose_wire: Value = serde_json::from_str(&verbose.to_json()).unwrap();
    let terse_wire: Value = serde_json::from_str(&terse.to_json()).unwrap();
    assert_eq!(terse_wire["diagnostics"]["retained"], 0);
    assert_eq!(terse_wire["coverage"]["rows_omitted"], 8);
    assert!(allowed_omission(&verbose_wire, &terse_wire));
    let mut erased_without_accounting = verbose_wire.clone();
    erased_without_accounting["diagnostics"]["items"] = serde_json::json!([]);
    assert!(!allowed_omission(&verbose_wire, &erased_without_accounting));

    let independent = one(br##"{"info":{},"namespace":{"lib":"https://example.com/missing"},"sdfData":{"Dependent":{"sdfRef":"lib:#/sdfData/X"}},"sdfProperty":{"Bad":{"type":"number","multipleOf":0}}}"##, Limits::default());
    let report = independent.check(CheckRequest::new()).unwrap();
    assert!(!report.outcome().complete());
    assert!(report.outcome().errors() > 0);
    assert_eq!(report.outcome().disposition(), Disposition::Failed);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|c| c.code() == "parameter.positive_step")
    );

    let omitted = one(
        br#"{"info":{},"sdfData":{"A":{"multipleOf":0},"B":{"multipleOf":0}}}"#,
        Limits::default().conditions(1),
    );
    let report = omitted.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(report.outcome().disposition(), Disposition::Failed);
    assert!(report.diagnostics().truncated());
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(wire["outcome"]["errors"], 2);
    assert_eq!(wire["coverage"]["requested"]["total"], 8);

    for (text, code) in [
        (br#"{"info":{},"sdfData":{"D":{"multipleOf":0}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfProperty":{"P":{"multipleOf":0}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfAction":{"A":{"sdfInputData":{"multipleOf":0}}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfAction":{"A":{"sdfOutputData":{"multipleOf":0}}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfEvent":{"E":{"sdfOutputData":{"multipleOf":0}}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfData":{"D":{"type":"object","properties":{"x":{"multipleOf":0}}}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfData":{"D":{"sdfChoice":{"x":{"multipleOf":0}}}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfData":{"D":{"items":42}}}"#.as_slice(), "data.map_shape"),
        (br#"{"info":{},"sdfData":{"D":{"items":{"minimum":false}}}}"#.as_slice(), "data.number"),
        (br#"{"info":{},"sdfData":{"D":{"items":{"format":false}}}}"#.as_slice(), "data.string"),
        (br#"{"info":{},"sdfData":{"D":{"items":{"enum":[]}}}}"#.as_slice(), "data.enum"),
        (br#"{"info":{},"sdfData":{"D":{"items":{"type":"object","properties":{"x":{"multipleOf":0}}}}}}"#.as_slice(), "parameter.positive_step"),
        (br#"{"info":{},"sdfData":{"D":{"items":{"sdfChoice":{"a":{"type":"array","items":{"type":"object","properties":{"x":{"multipleOf":0}}}}}}}}}"#.as_slice(), "parameter.positive_step"),
    ] {
        let catalogue = one(text, Limits::default());
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert!(report.outcome().errors() > 0, "{code}");
        assert!(report.diagnostics().conditions().any(|c| c.code() == code), "{code}");
    }
    println!(
        "initial/Full boundary, strict advice, repeatability, evolution, independent findings and context controls passed"
    );
}

#[test]
fn requirement_detail_evolution_and_omission() {
    let input=br#"{"info":{},"sdfData":{"D":{"type":"number","minimum":2,"maximum":1,"unit":"urn:ietf:params:unit:Cel"}}}"#;
    let catalogue = one(input, Limits::default());
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(allowed_omission(&wire, &wire));
    let mut evolved = wire.clone();
    evolved["coverage"]["requirements"]["future_optional"] = serde_json::json!(true);
    evolved["coverage"]["requirement_rows"]
        .as_array_mut()
        .unwrap()
        .reverse();
    for row in evolved["coverage"]["requirement_rows"]
        .as_array_mut()
        .unwrap()
    {
        row["basis"]["future_optional"] = serde_json::json!(true);
    }
    for row in evolved["baseline"]["registry_knowledge"]
        .as_array_mut()
        .unwrap()
    {
        row["future_optional"] = serde_json::json!({"hint":true});
    }
    evolved["policy"]["future_optional"] = serde_json::json!(true);
    assert_eq!(wire_projection(&wire), wire_projection(&evolved));
    let mut duplicate = wire.clone();
    let rows = duplicate["coverage"]["requirement_rows"]
        .as_array_mut()
        .unwrap();
    rows[1] = rows[0].clone();
    assert!(!allowed_omission(&wire, &duplicate));
    let mut limited = wire.clone();
    limited["coverage"]["requirement_rows"]
        .as_array_mut()
        .unwrap()
        .pop();
    limited["coverage"]["requirement_rows_retained"] = serde_json::json!(53);
    limited["coverage"]["requirement_rows_omitted"] = serde_json::json!(1);
    let record = serde_json::json!({"field":"coverage.requirement_rows","reason":"byte_limit","omitted_count":1});
    limited["coverage"]["requirement_omissions"] = serde_json::json!([record.clone()]);
    assert!(!allowed_omission(&wire, &limited));
    limited["diagnostics"]["truncated"] = serde_json::json!(true);
    limited["diagnostics"]["reasons"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("byte_limit"));
    limited["diagnostics"]["omissions"]
        .as_array_mut()
        .unwrap()
        .push(record);
    assert!(allowed_omission(&wire, &limited));
    for field in ["requirement_rows_retained", "requirement_rows_omitted"] {
        let mut inconsistent = limited.clone();
        inconsistent["coverage"][field] = serde_json::json!(0);
        assert!(!allowed_omission(&wire, &inconsistent));
    }
    let mut missing_link = limited.clone();
    missing_link["diagnostics"]["omissions"] = serde_json::json!([]);
    assert!(!allowed_omission(&wire, &missing_link));
    let mut wrong_owner = wire.clone();
    wrong_owner["coverage"]["requirement_rows"][0]["coverage_owner"] =
        serde_json::json!("full.metadata/1");
    assert!(!allowed_omission(&wire, &wrong_owner));
    let mut wrong_alias = wire.clone();
    wrong_alias["request"]["requirement_inventory"] = serde_json::json!("rfc9880.model/2");
    assert!(!allowed_omission(&wire, &wrong_alias));
    for change in ["policy", "registry", "condition_owner"] {
        let mut changed = wire.clone();
        match change {
            "policy" => changed["policy"]["recorded_registry"] = serde_json::json!("withheld"),
            "registry" => {
                changed["baseline"]["registry_knowledge"][0]["scope"] =
                    serde_json::json!("different")
            }
            "condition_owner" => {
                let row = changed["diagnostics"]["items"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|row| row.get("coverage_owner").is_some())
                    .unwrap();
                row["coverage_owner"] = serde_json::json!("full.document-syntax/1");
            }
            _ => unreachable!(),
        }
        assert_ne!(
            wire_projection(&wire),
            wire_projection(&changed),
            "{change}"
        );
        assert!(!allowed_omission(&wire, &changed), "{change}");
    }
    let terse = one(input, Limits::default().diagnostic_bytes(1));
    let terse = terse
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let terse: Value = serde_json::from_str(&terse.to_json()).unwrap();
    assert!(terse["coverage"].get("requirements").is_none());
    assert!(allowed_omission(&wire, &terse));
    assert_eq!(
        coverage_support::requirement_binding(terse["request"]["inventory"].as_str())
            .require()
            .unwrap(),
        "rfc9880.model/1"
    );
}
