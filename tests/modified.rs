//! Modified metadata is an explicit expanded assessment, independently of reference composition.
//! Follows ADR 0013: Shared workflow and report guarantees.
mod coverage_support;
mod location_support;
use esdieff::{
    Assessment, AssessmentStatus, Catalogue, CheckRequest, Field, Limits, ReportLocation,
    SourceInput,
};

#[test]
fn expanded_metadata_reports_the_gregorian_century_error_at_its_source() {
    let source = br#"{"info":{"modified":"1900-02-29"}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("revision", source)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    let diagnostics = report.diagnostics();
    let date = diagnostics
        .conditions()
        .find(|condition| condition.code() == "metadata.modified_calendar")
        .unwrap();
    let Field::Present(ReportLocation::Source(location)) = date.primary() else {
        panic!("metadata source location")
    };
    assert!(matches!(
        location.pointer(),
        Field::Present("/info/modified")
    ));
    assert_eq!(
        &source[location.span().unwrap().start as usize..location.span().unwrap().end as usize],
        br#""1900-02-29""#
    );
    assert_eq!(
        catalogue.source(location.source_id()).unwrap().raw_bytes(),
        source
    );
}

#[test]
fn early_nonstring_metadata_survives_later_work_exhaustion_and_all_detail_omission() {
    let selector = "x".repeat(1500);
    let definitions = format!(r#""sdfData":{{"Value":{{"type":"{selector}"}}}}"#);
    for source in [
        format!(r#"{{{definitions},"info":{{"modified":false}}}}"#),
        format!(r#"{{"info":{{"modified":false}},{definitions}}}"#),
    ] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("revision", source.as_bytes())],
            Limits::default().decoding_work(6000).diagnostic_bytes(1),
        )
        .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert!(matches!(
            report.assessment_area_status("modified-date-time/1"),
            Field::Present(AssessmentStatus::Assessed)
        ));
        coverage_support::assert_requested_counts(&report);
        assert_eq!(report.requested_coverage().unassessed(), 0);
        assert_eq!(report.diagnostics().conditions().len(), 0);
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["rows_omitted"], 9);
        assert_eq!(wire["outcome"]["errors"], 1);
    }
}

#[test]
fn numeric_offsets_are_outside_the_modified_utc_production() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "revision",
        br#"{"info":{"modified":"2024-02-29T12:34:56+00:00"}}"#,
    )])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "metadata.modified_syntax")
    );
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
}

#[test]
fn gregorian_date_controls_and_existing_metadata_shapes_keep_their_exact_meaning() {
    for date in ["0000-02-29", "2000-02-29", "9999-12-31", "1580-02-29"] {
        let source = format!(
            r#"{{"info":{{"modified":"{date}","version":"free revision","license":"custom license terms"}}}}"#
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("revision", source.as_bytes())])
                .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0, "{date}");
        assert!(matches!(
            report.assessment_area_status("modified-date-time/1"),
            Field::Present(AssessmentStatus::Assessed)
        ));
    }
    for value in ["false", "null", "42", "{}", "[]"] {
        let source = format!(r#"{{"info":{{"modified":{value}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("revision", source.as_bytes())])
                .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 1);
        assert_eq!(
            report
                .diagnostics()
                .conditions()
                .filter(|condition| condition.code() == "metadata.string")
                .count(),
            1
        );
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|condition| condition.code().starts_with("metadata.modified_"))
        );
    }
}

#[test]
fn impossible_future_insertion_slots_are_definite_without_leap_knowledge() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "revision",
        br#"{"info":{"modified":"2030-06-30T12:00:60Z"}}"#,
    )])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "metadata.modified_exceptional_slot")
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(wire["baseline"].get("temporal_knowledge").is_none());
}

#[test]
fn utc_case_exact_fractions_and_invalid_production_controls_use_metadata_rules() {
    for (date, code) in [
        ("2024-02-29t12:34:56z", None),
        ("2024-02-29t12:34:56Z", None),
        ("2024-02-29T12:34:56.000000000000000000001Z", None),
        ("2100-02-29", Some("metadata.modified_calendar")),
        ("2024-04-31", Some("metadata.modified_calendar")),
        ("2024-00-01", Some("metadata.modified_calendar")),
        ("2024-01-00", Some("metadata.modified_calendar")),
        ("2024-02-29T24:00:00Z", Some("metadata.modified_clock")),
        ("2024-02-29T12:60:00Z", Some("metadata.modified_clock")),
        ("2024-02-29T12:34:61Z", Some("metadata.modified_clock")),
        ("2024-02-29T12:34:56.Z", Some("metadata.modified_syntax")),
        ("2024-02-29 12:34:56Z", Some("metadata.modified_syntax")),
        (
            "2024-02-29T12:34:56-00:00",
            Some("metadata.modified_syntax"),
        ),
        (
            "2024-02-29T12:34:56+05:45",
            Some("metadata.modified_syntax"),
        ),
        ("2024-02-29T12:34:56", Some("metadata.modified_syntax")),
        ("12:34:56Z", Some("metadata.modified_syntax")),
        ("20240229", Some("metadata.modified_syntax")),
        ("2024-02-２９", Some("metadata.modified_syntax")),
    ] {
        let source = format!(r#"{{"info":{{"modified":"{date}"}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("revision", source.as_bytes())])
                .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(
            report.outcome().errors(),
            u32::from(code.is_some()),
            "{date}"
        );
        assert!(
            matches!(
                report.assessment_area_status("modified-date-time/1"),
                Field::Present(AssessmentStatus::Assessed)
            ),
            "{date}"
        );
        if let Some(code) = code {
            assert!(
                report
                    .diagnostics()
                    .conditions()
                    .any(|condition| condition.code() == code),
                "{date}"
            );
        }
        assert_eq!(
            catalogue.sources().next().unwrap().raw_bytes(),
            source.as_bytes()
        );
    }
}

#[test]
fn recorded_noninsertion_is_an_rfc_finding_with_its_knowledge_identity() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "revision",
        br#"{"info":{"modified":"2026-12-31T23:59:60Z"}}"#,
    )])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let diagnostics = report.diagnostics();
    let finding = diagnostics
        .conditions()
        .find(|condition| condition.code() == "metadata.modified_leap_second")
        .unwrap();
    assert_eq!(finding.basis().kind(), esdieff::BasisKind::Rfc);
    assert_eq!(finding.basis().id(), Some("iers-bulletin-c72/2026-07-06"));
    assert_eq!(finding.assessment_unit(), Some("modified-date-time/1"));
}

#[test]
fn actual_leap_knowledge_and_semantic_status_survive_every_detail_omission() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied(
            "revision",
            br#"{"info":{"modified":"2030-06-30T23:59:59.9Z"}}"#,
        )],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        wire["baseline"]["temporal_knowledge"]["id"],
        "iers-bulletin-c72/2026-07-06"
    );
    assert_eq!(wire["baseline"]["temporal_knowledge"]["from"], "1972-01-01");
    assert_eq!(
        wire["baseline"]["temporal_knowledge"]["through"],
        "2026-12-31"
    );
    assert_eq!(
        wire["baseline"]["temporal_knowledge"]["transition_sha256"],
        "6cb6f5d4b819f2e568e25db4b0b26d89dedf031fdffb18bc94d40f4e94e268d7"
    );
    assert_eq!(
        wire["baseline"]["temporal_knowledge"]["bulletin_sha256"],
        "310e172eadacacca3adf92cb9bd646fcc0d6320e35e996a106d2644ede3b52f0"
    );
    coverage_support::assert_wire_counts(&wire);
    assert_eq!(wire["coverage"]["rows_omitted"], 9);
    coverage_support::assert_requested_counts(&report);
    assert!(
        report
            .to_human()
            .contains("Assessment area modified-date-time/1: undetermined")
    );
    assert!(
        report
            .to_human()
            .contains("Temporal knowledge: iers-bulletin-c72/2026-07-06")
    );
}

#[test]
fn immutable_requested_inventories_distinguish_absence_from_unrequested_or_unknown_work() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("revision", br#"{"info":{}}"#)],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let request = CheckRequest::new().assessment(Assessment::ExpandedModified);
    let expanded = catalogue.check(request).unwrap();
    assert!(matches!(
        expanded.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::NotApplicable)
    ));
    assert!(matches!(
        expanded.assessment_area_status("document-envelope/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(matches!(
        expanded.assessment_area_status("future-area/1"),
        Field::Absent
    ));
    let coverage = expanded.requested_coverage();
    assert_eq!(coverage.inventory(), Some("esdieff.expanded.modified/1"));
    assert_eq!(coverage.count_unit(), "assessment_area");
    assert_eq!(coverage.total(), Some(9));
    assert_eq!(coverage.assessed(), 5);
    assert_eq!(coverage.not_applicable(), 4);
    assert_eq!(coverage.unassessed(), 0);
    assert_eq!(coverage.undetermined(), 0);
    assert_eq!(expanded.diagnostics().conditions().len(), 0);
    assert!(
        expanded
            .to_human()
            .contains("Assessment area modified-date-time/1: not_applicable")
    );
    let wire: serde_json::Value = serde_json::from_str(&expanded.to_json()).unwrap();
    assert_eq!(wire["coverage"]["rows_omitted"], 9);
    coverage_support::assert_wire_counts(&wire);

    let initial = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(
        initial.requested_coverage().inventory(),
        Some("esdieff.initial/1")
    );
    assert_eq!(initial.requested_coverage().total(), Some(8));
    assert!(matches!(
        initial.assessment_area_status("modified-date-time/1"),
        Field::Absent
    ));
    let full = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_eq!(
        full.requested_coverage().inventory(),
        Some("esdieff.full-model/1")
    );
    assert_eq!(full.requested_coverage().total(), Some(13));
    for area in [
        "modified-date-time/1",
        "document-envelope/1",
        "future-area/1",
    ] {
        assert!(matches!(full.assessment_area_status(area), Field::Absent));
    }
    let id = catalogue.source_id("revision").unwrap();
    let inspection = catalogue
        .inspect(esdieff::InspectRequest::new(id, ""))
        .unwrap();
    assert!(matches!(
        inspection.assessment_area_status("modified-date-time/1"),
        Field::Absent
    ));
    assert_eq!(inspection.requested_coverage().count_unit(), "inspection");
    assert_eq!(inspection.requested_coverage().total(), Some(1));
    coverage_support::assert_requested_counts(&inspection);
}

#[test]
fn required_unknown_metadata_context_keeps_one_aggregate_unknown_and_independent_errors() {
    for (source, _advisory_unknown) in [
        (
            br#"{"info":{"features":["future"],"modified":false}}"#.as_slice(),
            true,
        ),
        (
            br#"{"info":{"features":null,"modified":false}}"#.as_slice(),
            true,
        ),
        (
            br#"{"info":{"modified":"1900-02-29","\uD800":1}}"#.as_slice(),
            true,
        ),
        (
            br#"{"info":{"modified":"1900-02-29"},"\uD800":1}"#.as_slice(),
            true,
        ),
        (br#"{"info":{"modified":"\uD800"}}"#.as_slice(), false),
        (br#"{"info":[]}"#.as_slice(), true),
        (b"{unfinished".as_slice(), true),
        (b"\xff".as_slice(), true),
    ] {
        let catalogue = Catalogue::with_limits(
            &[
                SourceInput::supplied("unknown", source),
                SourceInput::supplied("definite", br#"{"info":{"modified":false}}"#),
            ],
            Limits::default().diagnostic_bytes(1),
        )
        .unwrap();
        let unknown = catalogue
            .check(
                CheckRequest::new()
                    .root(catalogue.source_id("unknown").unwrap())
                    .assessment(Assessment::ExpandedModified),
            )
            .unwrap();
        assert!(
            matches!(
                unknown.assessment_area_status("modified-date-time/1"),
                Field::Present(AssessmentStatus::Undetermined)
            ),
            "{source:?}"
        );
        coverage_support::assert_requested_counts(&unknown);
        let combined = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert!(
            matches!(
                combined.assessment_area_status("modified-date-time/1"),
                Field::Present(AssessmentStatus::Undetermined)
            ),
            "{source:?}"
        );
        coverage_support::assert_requested_counts(&combined);
        assert_eq!(combined.outcome().errors(), unknown.outcome().errors() + 1);
        assert_eq!(
            combined.outcome().disposition(),
            esdieff::Disposition::Failed
        );
        coverage_support::assert_requested_counts(&combined);
        assert_eq!(combined.diagnostics().conditions().len(), 0);
    }
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "critical",
        br#"{"info":{"features":["future"],"modified":false}}"#,
    )])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "metadata.string")
    );
}

#[test]
fn ordinary_information_metadata_is_independent_of_reference_and_opaque_content() {
    let source = br##"{"info":{"modified":"1900-02-29","sdfRef":"missing"},"sdfData":{"Ref":{"sdfRef":"#/sdfData/Absent"}},"x:extension":{"modified":"1900-02-29"}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("revision", source)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert_eq!(
        report
            .diagnostics()
            .conditions()
            .filter(|condition| condition.code() == "metadata.modified_calendar")
            .count(),
        1
    );
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "ordinary",
            br#"{"info":{},"x:extension":{"modified":"1900-02-29"}}"#,
        ),
        SourceInput::supplied("unused", br#"{"info":{"modified":"1900-02-29"}}"#),
    ])
    .unwrap();
    let selected = catalogue
        .check(
            CheckRequest::new()
                .root(catalogue.source_id("ordinary").unwrap())
                .assessment(Assessment::ExpandedModified),
        )
        .unwrap();
    assert_eq!(selected.outcome().errors(), 0);
    assert!(matches!(
        selected.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::NotApplicable)
    ));
    assert_eq!(
        catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap()
            .outcome()
            .errors(),
        1
    );
}

#[test]
fn finite_leap_interval_establishes_recorded_events_without_guessing_other_dates() {
    // Independently specified insertion labels from the recorded IERS transition history.
    for date in [
        "1972-06-30",
        "1972-12-31",
        "1973-12-31",
        "1974-12-31",
        "1975-12-31",
        "1976-12-31",
        "1977-12-31",
        "1978-12-31",
        "1979-12-31",
        "1981-06-30",
        "1982-06-30",
        "1983-06-30",
        "1985-06-30",
        "1987-12-31",
        "1989-12-31",
        "1990-12-31",
        "1992-06-30",
        "1993-06-30",
        "1994-06-30",
        "1995-12-31",
        "1997-06-30",
        "1998-12-31",
        "2005-12-31",
        "2008-12-31",
        "2012-06-30",
        "2015-06-30",
        "2016-12-31",
    ] {
        let source =
            format!(r#"{{"info":{{"modified":"{date}T23:59:60.000000000000000000001Z"}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("revision", source.as_bytes())])
                .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0, "{date}");
        assert!(
            matches!(
                report.assessment_area_status("modified-date-time/1"),
                Field::Present(AssessmentStatus::Assessed)
            ),
            "{date}"
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["baseline"]["temporal_knowledge"]["id"],
            "iers-bulletin-c72/2026-07-06"
        );
    }
    for (label, status, errors, knowledge) in [
        ("2026-12-31T23:59:59Z", AssessmentStatus::Assessed, 0, true),
        ("1972-01-31T23:59:60Z", AssessmentStatus::Assessed, 1, true),
        (
            "1971-12-31T23:59:59Z",
            AssessmentStatus::Undetermined,
            0,
            true,
        ),
        (
            "2030-01-31T23:59:60Z",
            AssessmentStatus::Undetermined,
            0,
            true,
        ),
        (
            "2031-02-28T23:59:59Z",
            AssessmentStatus::Undetermined,
            0,
            true,
        ),
        (
            "9999-12-31T23:59:60Z",
            AssessmentStatus::Undetermined,
            0,
            true,
        ),
        ("0000-02-29T23:59:58Z", AssessmentStatus::Assessed, 0, false),
        ("2030-06-29T23:59:60Z", AssessmentStatus::Assessed, 1, false),
        ("2030-06-30T23:58:60Z", AssessmentStatus::Assessed, 1, false),
        ("2030-06-30T23:59:58Z", AssessmentStatus::Assessed, 0, false),
    ] {
        let source = format!(r#"{{"info":{{"modified":"{label}"}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("revision", source.as_bytes())])
                .unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
            .unwrap();
        assert_eq!(report.outcome().errors(), errors, "{label}");
        assert!(
            matches!(report.assessment_area_status("modified-date-time/1"), Field::Present(actual) if actual == status),
            "{label}"
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(
            wire["baseline"].get("temporal_knowledge").is_some(),
            knowledge,
            "{label}"
        );
    }
}

#[test]
fn exact_escaped_tokens_and_unrelated_extreme_numbers_remain_source_values() {
    let source = br#"{"info":{"modified":"2016-12-31\u0054\u0032\u0033:59:60.000000000000000000001\u007a"},"x:opaque":{"number":10e1000000}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("revision", source)]).unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    let owned = catalogue.sources().next().unwrap();
    assert_eq!(owned.raw_bytes(), source);
    let root = owned.json().require().unwrap();
    let modified = root
        .get("info")
        .require()
        .unwrap()
        .get("modified")
        .require()
        .unwrap();
    assert_eq!(
        modified.text().require().unwrap(),
        "2016-12-31T23:59:60.000000000000000000001z"
    );
    assert_eq!(
        modified.raw_json(),
        r#""2016-12-31\u0054\u0032\u0033:59:60.000000000000000000001\u007a""#
    );
    let number = root
        .get("x:opaque")
        .require()
        .unwrap()
        .get("number")
        .require()
        .unwrap()
        .number()
        .require()
        .unwrap();
    assert_eq!(number.lexeme(), "10e1000000");
    let inspection = catalogue
        .inspect(esdieff::InspectRequest::new(owned.id(), "/x:opaque/number"))
        .unwrap();
    assert_eq!(
        inspection.inspection_json().unwrap().raw_json(),
        "10e1000000"
    );
}

#[test]
fn bounded_fraction_work_preserves_a_definite_companion_and_cannot_invent_a_date_error() {
    let source = format!(
        r#"{{"info":{{"modified":"2024-01-01T00:00:00.{}Z"}}}}"#,
        "0".repeat(1000)
    );
    let inputs = [
        SourceInput::supplied("definite", br#"{"info":{"modified":false}}"#),
        SourceInput::supplied("long", source.as_bytes()),
    ];
    let source_usable = |work| {
        let catalogue =
            Catalogue::with_limits(&inputs, Limits::default().decoding_work(work)).unwrap();
        matches!(
            catalogue
                .source(catalogue.source_id("long").unwrap())
                .unwrap()
                .json(),
            Field::Present(_)
        )
    };
    let (mut lower, mut upper) = (1, 10000);
    assert!(source_usable(upper));
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if source_usable(middle) {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    let catalogue =
        Catalogue::with_limits(&inputs, Limits::default().decoding_work(lower)).unwrap();
    let long = catalogue
        .source(catalogue.source_id("long").unwrap())
        .unwrap();
    assert!(
        matches!(long.json(), Field::Present(_)),
        "source decoding must be available for this semantic work control"
    );
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert_eq!(report.outcome().errors(), 1);
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Undetermined)
    ));
    coverage_support::assert_requested_counts(&report);
    let diagnostics = report.diagnostics();
    let limit = diagnostics
        .conditions()
        .find(|condition| condition.code() == "operation.limit")
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let wire_limit = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|condition| condition["code"] == "operation.limit")
        .unwrap();
    assert_eq!(
        (
            limit.assessment_unit(),
            wire_limit["assessment_unit"].as_str()
        ),
        (Some("modified-date-time/1"), Some("modified-date-time/1")),
    );
    let location = location_support::primary_source(limit);
    assert_eq!(location.source_id(), long.id());
    assert!(matches!(
        location.pointer(),
        Field::Present("/info/modified")
    ));
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code().starts_with("metadata.modified_"))
    );
    assert_eq!(long.raw_bytes(), source.as_bytes());
    println!(
        "fraction_digits=1000; minimum_decoding_work={lower}; decoded_source_available=true; modified_status=undetermined; preserved_companion_errors=1"
    );
}

#[test]
fn smallest_accepted_report_keeps_actual_temporal_metadata_and_native_status() {
    let source = br#"{"info":{"modified":"2016-12-31T23:59:60Z"}}"#;
    let accepted = |bytes| {
        Catalogue::with_limits(
            &[SourceInput::supplied("revision", source)],
            Limits::default().report_bytes(bytes),
        )
        .and_then(|catalogue| {
            catalogue
                .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
                .map(|_| ())
        })
        .is_ok()
    };
    let (mut lower, mut upper) = (1, 65536);
    assert!(accepted(upper));
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if accepted(middle) {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    let minimum = lower;
    assert!(!accepted(minimum - 1));
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("revision", source)],
        Limits::default().report_bytes(minimum),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    let json = report.to_json();
    let human = report.to_human();
    assert!(json.len() <= minimum);
    assert!(human.len() <= minimum);
    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        wire["baseline"]["temporal_knowledge"]["id"],
        "iers-bulletin-c72/2026-07-06"
    );
    assert_eq!(wire["coverage"]["rows_omitted"], 9);
    assert_eq!(wire["diagnostics"]["retained"], 0);
    coverage_support::assert_wire_counts(&wire);
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert!(human.contains("Assessment area modified-date-time/1: assessed"));
    assert!(human.contains("Temporal knowledge: iers-bulletin-c72/2026-07-06"));
    println!(
        "minimum_report_bytes={minimum}; actual_json_bytes={}; actual_human_bytes={}; temporal_knowledge_present=true; rows_omitted=9; retained_conditions=0",
        json.len(),
        human.len()
    );
}

#[test]
fn retained_rows_and_omitted_pointers_agree_with_native_semantic_status() {
    let source = br#"{"info":{"modified":"1900-02-29"}}"#;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("revision", source)],
        Limits::default().conditions(1).pointer_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    let diagnostics = report.diagnostics();
    assert_eq!(diagnostics.conditions().len(), 1);
    let finding = diagnostics.conditions().next().unwrap();
    assert_eq!(finding.code(), "metadata.modified_calendar");
    let location = location_support::primary_source(finding);
    assert!(matches!(location.pointer(), Field::Unavailable(_)));
    let span = location.span().unwrap();
    assert_eq!(
        &source[span.start as usize..span.end as usize],
        br#""1900-02-29""#
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let row = wire["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "modified-date-time/1")
        .unwrap();
    assert_eq!(row["status"], "assessed");
    assert_eq!(row["count_unit"], "assessment_area");
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::Assessed)
    ));
    assert_eq!(report.outcome().errors(), 1);
    coverage_support::assert_requested_counts(&report);
}

#[test]
fn reliable_absence_across_requested_sources_is_not_applicable() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("without-info", b"{}"),
        SourceInput::supplied("empty-info", br#"{"info":{}}"#),
        SourceInput::supplied(
            "empty-features",
            br#"{"info":{"features":[]},"x:opaque":{"modified":false}}"#,
        ),
    ])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedModified))
        .unwrap();
    assert!(matches!(
        report.assessment_area_status("modified-date-time/1"),
        Field::Present(AssessmentStatus::NotApplicable)
    ));
    coverage_support::assert_requested_counts(&report);
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().warnings(), 1);
    coverage_support::assert_requested_counts(&report);
}
