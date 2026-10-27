//! Recorded extension vocabulary in Full checks, following ADR 0038: Recorded
//! extension vocabulary.
use esdieff::{
    Assessment, AssessmentStatus, BasisKind, Catalogue, CheckRequest, Condition, Diagnostics,
    Disposition, Field, RecordedRegistryPolicy, Report, ReportLocation, Severity, SourceInput,
};
use sha2::{Digest, Sha256};

const REGISTRY: &str = "omna.sdf-quality-names";
const TEMPERATURE: &[u8] = include_bytes!("fixtures/recorded-extension/temperature.sdf.json");
const EXCLUDED: &str = "extension.recorded_value_excluded";
const CITATION: &str = "OMNA SDF Quality Names registry";

fn model(text: &[u8]) -> Catalogue {
    Catalogue::from_inputs(&[SourceInput::supplied("model", text)]).unwrap()
}

fn full(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap()
}

fn status(report: &Report<'_>, member: u16) -> AssessmentStatus {
    report
        .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
        .require()
        .unwrap()
}

/// The `observed_occurrences` of each optional-vocabulary notice that carries one.
fn optional_occurrences(report: &Report<'_>) -> Vec<u64> {
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["code"] == "extension.optional_unsupported")
        .filter_map(|item| item["arguments"]["observed_occurrences"].as_u64())
        .collect()
}

/// Source and effective optional-vocabulary notices together.
fn optional_notices(report: &Report<'_>) -> usize {
    report
        .diagnostics()
        .conditions()
        .filter(|condition| condition.code() == "extension.optional_unsupported")
        .count()
}

/// Errors reporting a recorded extension value outside its registered domain.
fn excluded_errors(diagnostics: &Diagnostics) -> Vec<&Condition> {
    diagnostics
        .conditions()
        .filter(|condition| condition.code() == EXCLUDED)
        .collect()
}

fn effective_pointer(condition: &Condition) -> Option<String> {
    match condition.primary() {
        Field::Present(ReportLocation::Effective(location)) => {
            location.pointer().present().map(str::to_owned)
        }
        _ => None,
    }
}

fn with_id(template: &str, id: &str) -> Vec<u8> {
    format!(r#"{{"info":{{}},{}}}"#, template.replace("ID", id)).into_bytes()
}

/// Each position whose registered CDDL socket admits `oma:id`, with its effective pointer.
const RECORDED_POSITIONS: [(&str, &str); 10] = [
    (r#""sdfObject":{"O":{"oma:id":ID}}"#, "/sdfObject/O/oma:id"),
    (
        r#""sdfObject":{"O":{"sdfAction":{"A":{"oma:id":ID}}}}"#,
        "/sdfObject/O/sdfAction/A/oma:id",
    ),
    (r#""sdfAction":{"A":{"oma:id":ID}}"#, "/sdfAction/A/oma:id"),
    (
        r#""sdfData":{"D":{"type":"number","oma:id":ID}}"#,
        "/sdfData/D/oma:id",
    ),
    (
        r#""sdfProperty":{"P":{"type":"number","oma:id":ID}}"#,
        "/sdfProperty/P/oma:id",
    ),
    (
        r#""sdfAction":{"A":{"sdfInputData":{"type":"number","oma:id":ID}}}"#,
        "/sdfAction/A/sdfInputData/oma:id",
    ),
    (
        r#""sdfAction":{"A":{"sdfOutputData":{"type":"number","oma:id":ID}}}"#,
        "/sdfAction/A/sdfOutputData/oma:id",
    ),
    (
        r#""sdfEvent":{"E":{"sdfOutputData":{"type":"number","oma:id":ID}}}"#,
        "/sdfEvent/E/sdfOutputData/oma:id",
    ),
    (
        r#""sdfData":{"D":{"type":"object","properties":{"p":{"type":"number","oma:id":ID}}}}"#,
        "/sdfData/D/properties/p/oma:id",
    ),
    (
        r#""sdfData":{"D":{"sdfChoice":{"a":{"type":"number","oma:id":ID},"b":{"type":"string"}}}}"#,
        "/sdfData/D/sdfChoice/a/oma:id",
    ),
];

#[test]
fn oma_id_is_checked_at_every_registered_position() {
    for (position, pointer) in RECORDED_POSITIONS {
        let catalogue = model(&with_id(position, "7"));
        let report = full(&catalogue);
        assert_eq!(
            report.outcome().disposition(),
            Disposition::Passed,
            "{position}"
        );
        assert_eq!(
            status(&report, 28),
            AssessmentStatus::Assessed,
            "{position}"
        );
        assert_eq!(optional_notices(&report), 0, "{position}");
        assert_eq!(
            report
                .registry_knowledge(REGISTRY)
                .require()
                .unwrap()
                .state(),
            "available"
        );
        let catalogue = model(&with_id(position, r#""7""#));
        let report = full(&catalogue);
        assert_eq!(
            report.outcome().disposition(),
            Disposition::Failed,
            "{position}"
        );
        assert!(report.outcome().complete(), "{position}");
        assert_eq!(
            status(&report, 28),
            AssessmentStatus::Assessed,
            "{position}"
        );
        assert_eq!(optional_notices(&report), 0, "{position}");
        let diagnostics = report.diagnostics();
        let errors = excluded_errors(&diagnostics);
        assert_eq!(errors.len(), 1, "{position}");
        assert_eq!(effective_pointer(errors[0]).as_deref(), Some(pointer));
    }
}

#[test]
fn oma_id_elsewhere_and_other_oma_names_stay_undeclared_extension_vocabulary() {
    for position in [
        r#""sdfEvent":{"E":{"oma:id":ID}}"#,
        r#""sdfThing":{"T":{"oma:id":ID}}"#,
        r#""sdfData":{"D":{"type":"array","items":{"type":"number","oma:id":ID}}}"#,
        r#""sdfObject":{"O":{"oma:name":ID}}"#,
        r#""sdfProperty":{"P":{"type":"number","oma:other":ID}}"#,
    ] {
        for id in ["7", r#""7""#] {
            let catalogue = model(&with_id(position, id));
            let report = full(&catalogue);
            assert_eq!(
                status(&report, 28),
                AssessmentStatus::Undetermined,
                "{position}"
            );
            assert_eq!(optional_occurrences(&report), [1], "{position}");
            assert!(
                matches!(report.registry_knowledge(REGISTRY), Field::Absent),
                "{position}"
            );
            assert_eq!(report.outcome().errors(), 0, "{position} {id}");
        }
    }
}

#[test]
fn values_outside_the_uint_domain_are_registration_errors() {
    for (id, excluded) in [
        ("3303", false),
        ("3303.0", false),
        ("3.303e3", false),
        ("0", false),
        ("-0", false),
        ("18446744073709551615", false),
        (r#""3303""#, true),
        ("-1", true),
        ("3303.5", true),
        ("18446744073709551616", true),
        ("true", true),
        ("null", true),
        ("[3303]", true),
    ] {
        let catalogue = model(&with_id(
            r#""sdfObject":{"Temperature":{"oma:id":ID,"sdfProperty":{"Value":{"type":"number","oma:id":5700}}}}"#,
            id,
        ));
        let report = full(&catalogue);
        assert_eq!(status(&report, 28), AssessmentStatus::Assessed, "{id}");
        assert!(report.outcome().complete(), "{id}");
        assert_eq!(
            report.outcome().disposition(),
            if excluded {
                Disposition::Failed
            } else {
                Disposition::Passed
            },
            "{id}"
        );
        assert_eq!(report.outcome().errors(), u32::from(excluded), "{id}");
        let diagnostics = report.diagnostics();
        let errors = excluded_errors(&diagnostics);
        assert_eq!(errors.len(), usize::from(excluded), "{id}");
        for error in errors {
            assert_eq!(error.severity(), Severity::Error);
            assert_eq!(
                effective_pointer(error).as_deref(),
                Some("/sdfObject/Temperature/oma:id"),
                "{id}"
            );
            assert_eq!(error.assessment_unit(), Some("full.critical-extensions/1"));
            assert_eq!(error.coverage_owner(), Some("full.critical-extensions/1"));
            let basis = error.basis();
            assert_eq!(
                (basis.kind(), basis.citation(), basis.id()),
                (BasisKind::Registration, Some(CITATION), Some("oma:id")),
                "{id}"
            );
        }
        assert_eq!(
            report
                .registry_knowledge(REGISTRY)
                .require()
                .unwrap()
                .state(),
            "available",
            "{id}"
        );
    }
}

#[test]
fn withheld_evidence_leaves_the_oma_id_conclusion_unavailable() {
    let catalogue = model(TEMPERATURE);
    for id in ["3303", r#""3303""#] {
        let text = String::from_utf8(TEMPERATURE.to_vec())
            .unwrap()
            .replace(r#""oma:id": 3303"#, &format!(r#""oma:id": {id}"#));
        assert!(text.contains(&format!(r#""oma:id": {id},"#)));
        let catalogue = model(text.as_bytes());
        let report = catalogue
            .check(
                CheckRequest::new()
                    .assessment(Assessment::FullBaseline)
                    .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded),
            )
            .unwrap();
        assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(status(&report, 28), AssessmentStatus::Undetermined);
        for member in [42, 45, 46] {
            assert_eq!(status(&report, member), AssessmentStatus::Assessed);
        }
        assert_eq!(optional_notices(&report), 0);
        let record = report.registry_knowledge(REGISTRY).require().unwrap();
        assert_eq!(
            (record.state(), record.reason(), record.snapshot()),
            ("unavailable", Some("caller_withheld"), None)
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["policy"]["recorded_registry"], "withhold_recorded");
        let listed = wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["registry"] == REGISTRY)
            .unwrap();
        assert_eq!(listed.as_object().unwrap().len(), 4);
    }
    let report = catalogue
        .check(
            CheckRequest::new()
                .assessment(Assessment::FullBaseline)
                .recorded_registry_policy(RecordedRegistryPolicy::UseRecorded),
        )
        .unwrap();
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
}

#[test]
fn full_notice_counts_only_vocabulary_still_unsupported() {
    let text = String::from_utf8(TEMPERATURE.to_vec()).unwrap().replace(
        r#""oma:id": 5601,"#,
        r#""oma:id": 5601, "acme:note": true,"#,
    );
    assert_ne!(text.as_bytes(), TEMPERATURE);
    let catalogue = model(text.as_bytes());
    let report = full(&catalogue);
    assert_eq!(optional_occurrences(&report), [1]);
    assert_eq!(status(&report, 28), AssessmentStatus::Undetermined);
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(optional_occurrences(&report), [6]);
}

#[test]
fn initial_and_expanded_assessments_keep_their_inventories_and_notice() {
    let catalogue = model(TEMPERATURE);
    for (assessment, areas) in [
        (Assessment::Initial, 8),
        (Assessment::ExpandedModified, 9),
        (Assessment::ExpandedUnits, 10),
        (Assessment::ExpandedContentFormat, 10),
        (Assessment::ExpandedPatternSyntax, 9),
        (Assessment::ExpandedNumeric, 9),
        (Assessment::ExpandedConsistency, 10),
    ] {
        let report = catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        assert!(matches!(report.registry_knowledge(REGISTRY), Field::Absent));
        assert!(matches!(
            report.requirement_status("rfc9880.model.M028/1"),
            Field::Absent
        ));
        assert_eq!(optional_occurrences(&report), [5], "{assessment:?}");
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["requested"]["total"], areas);
        assert_eq!(report.outcome().disposition(), Disposition::Passed);
    }
    let report = catalogue
        .check(
            CheckRequest::new()
                .assessment(Assessment::ExpandedUnits)
                .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded),
        )
        .unwrap();
    assert!(matches!(report.registry_knowledge(REGISTRY), Field::Absent));
}

/// M028 and the excluded-value errors, as effective pointer and related source document
/// and pointer, when two Objects of a selected consumer copy a reusable Object whose own
/// `oma:id` is `provider_id`. `overrides` is appended to both copying definitions.
fn consumers(provider_id: &str, overrides: &str) -> (AssessmentStatus, Vec<[String; 3]>) {
    let provider = format!(
        r#"{{"info":{{}},"namespace":{{"shared":"https://example.test/oma-definitions"}},"defaultNamespace":"shared","sdfObject":{{"Temperature":{{"oma:id":{provider_id},"sdfProperty":{{"Value":{{"type":"number","oma:id":5700}}}}}}}}}}"#
    );
    let consumer = format!(
        r#"{{"info":{{}},"namespace":{{"s":"https://example.test/oma-definitions"}},"sdfObject":{{"Used":{{"sdfRef":"s:#/sdfObject/Temperature"{overrides}}},"Spare":{{"sdfRef":"s:#/sdfObject/Temperature"{overrides}}}}}}}"#
    );
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("provider", provider.as_bytes()),
        SourceInput::supplied("consumer", consumer.as_bytes()),
    ])
    .unwrap();
    let root = catalogue.source_id("consumer").unwrap();
    let report = catalogue
        .check(
            CheckRequest::new()
                .root(root)
                .assessment(Assessment::FullBaseline),
        )
        .unwrap();
    assert_eq!(optional_notices(&report), 0);
    let diagnostics = report.diagnostics();
    let mut errors: Vec<_> = excluded_errors(&diagnostics)
        .into_iter()
        .map(|error| {
            let Some(ReportLocation::Source(source)) = error.related().next() else {
                panic!("related source location");
            };
            let document = if source.source_id() == root {
                "consumer"
            } else {
                "provider"
            };
            [
                effective_pointer(error).unwrap(),
                document.to_owned(),
                source.pointer().present().unwrap().to_owned(),
            ]
        })
        .collect();
    errors.sort();
    assert_eq!(report.outcome().errors() as usize, errors.len());
    (status(&report, 28), errors)
}

#[test]
fn inherited_ids_are_checked_where_they_are_used_and_overrides_as_the_importers_value() {
    let each_consumer = |document: &str, written: &str| {
        ["Spare", "Used"]
            .map(|name| {
                [
                    format!("/sdfObject/{name}/oma:id"),
                    document.to_owned(),
                    written.replace("NAME", name),
                ]
            })
            .to_vec()
    };
    for (provider_id, overrides, errors) in [
        ("3303", "", Vec::new()),
        (
            r#""3303""#,
            "",
            each_consumer("provider", "/sdfObject/Temperature/oma:id"),
        ),
        (r#""3303""#, r#","oma:id":3303"#, Vec::new()),
        (
            "3303",
            r#","oma:id":-3303"#,
            each_consumer("consumer", "/sdfObject/NAME/oma:id"),
        ),
        (r#""3303""#, r#","oma:id":null"#, Vec::new()),
    ] {
        assert_eq!(
            consumers(provider_id, overrides),
            (AssessmentStatus::Assessed, errors),
            "{provider_id} {overrides}"
        );
    }
}

#[test]
fn oma_shaped_object_with_whole_number_ids_completes_full_with_the_recorded_evidence() {
    let catalogue = model(TEMPERATURE);
    let report = full(&catalogue);
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
    assert_eq!(status(&report, 28), AssessmentStatus::Assessed);
    for member in [42, 45, 46] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    let record = report.registry_knowledge(REGISTRY).require().unwrap();
    let canonical =
        include_bytes!("fixtures/recorded-extension/omna-sdf-quality-names.records.tsv");
    assert_eq!(
        (
            record.state(),
            record.snapshot(),
            record.scope(),
            record.entries_count(),
            record.source_updated(),
            record.retrieved_at(),
        ),
        (
            "available",
            Some("omna.sdf-quality-names/2026-10-16/58b2e81f1a27e35f"),
            Some("oma-quality-names"),
            Some(1),
            Some("2026-10-15"),
            Some("2026-10-16T08:38:45Z"),
        )
    );
    assert_eq!(
        record.digest(),
        Some(
            Sha256::digest(canonical)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                .as_str()
        )
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let listed: Vec<_> = wire["baseline"]["registry_knowledge"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["registry"] == REGISTRY)
        .collect();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["state"], "available");
    assert_eq!(listed[0]["entries_count"], 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .all(|condition| condition.code() != "extension.optional_unsupported")
    );
}
