//! Verify structural reports keep requested scope and outcomes independent of omitted detail.
//! Follows ADR0011: Machine-readable report contracts, and
//! ADR0013: Shared workflow and report guarantees.

mod coverage_support;
mod location_support;
use esdieff::{
    Assessment, Catalogue, CheckRequest, Disposition, InspectRequest, Limits, SourceInput, View,
};
use serde_json::Value;
fn json(report: &esdieff::Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}

#[test]
fn source_inspection_and_initial_assessment_are_independent() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("model", b"{\"opaque\":null}")]).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let inspection = catalogue
        .inspect(InspectRequest::new(id, "/opaque"))
        .unwrap();
    assert!(inspection.outcome().complete());
    assert_eq!(inspection.outcome().disposition(), Disposition::Passed);
    let wire = json(&inspection);
    assert_eq!(wire["request"]["assessment"], Value::Null);
    assert_eq!(wire["coverage"]["requested"]["count_unit"], "inspection");
    assert_eq!(wire["result"]["json"], Value::Null);
    assert_eq!(wire["result"]["json_present"], true);
    let check = catalogue.check(CheckRequest::new()).unwrap();
    let wire = json(&check);
    assert!(check.outcome().complete());
    coverage_support::assert_requested_counts(&check);
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert_eq!(wire["coverage"]["rows"].as_array().unwrap().len(), 8);
    assert_eq!(wire["result"]["kind"], "check");
    let full = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(!full.outcome().complete());
    assert_eq!(json(&full)["coverage"]["requested"]["total"], 13);
}

#[test]
fn failed_candidates_do_not_spoil_an_established_selected_source_view() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::failed("unused"),
        SourceInput::supplied("selected", b"{}"),
        SourceInput::supplied("bad", b"{\"x\":1,\"x\":2}"),
    ])
    .unwrap();
    let id = catalogue.source_id("selected").unwrap();
    let inspection = catalogue.inspect(InspectRequest::new(id, "")).unwrap();
    assert_eq!(inspection.outcome().disposition(), Disposition::Passed);
    assert_eq!(json(&inspection)["sources"].as_array().unwrap().len(), 3);
    assert_eq!(
        catalogue
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .disposition(),
        Disposition::Failed
    );
    assert_eq!(
        catalogue
            .check(CheckRequest::new().root(id))
            .unwrap()
            .outcome()
            .errors(),
        0
    );
}

#[test]
fn partial_acquisition_and_effective_work_cannot_report_completed_inspection() {
    let limits = Limits::default().document_bytes(2);
    let catalogue =
        Catalogue::with_limits(&[SourceInput::supplied("partial", b"{} trailing")], limits)
            .unwrap();
    let id = catalogue.source_id("partial").unwrap();
    let source = catalogue.inspect(InspectRequest::new(id, "")).unwrap();
    assert_eq!(source.inspection_json().unwrap().raw_json(), "{}");
    assert!(!source.outcome().complete());
    assert_eq!(source.outcome().disposition(), Disposition::Undetermined);
    let effective = catalogue
        .inspect(InspectRequest::new(id, "").view(View::Effective))
        .unwrap();
    assert!(effective.inspection_json().is_none());
    assert_eq!(json(&effective)["result"]["state"], "unavailable");
}

#[test]
fn undecodable_keys_leave_selection_unavailable_instead_of_an_invocation_error() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "unknown",
        br#"{"\uD800":1,"known":2}"#,
    )])
    .unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(
            catalogue.source_id("unknown").unwrap(),
            "/known",
        ))
        .unwrap();
    assert!(!report.outcome().complete());
    assert!(report.inspection_json().is_none());
    assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
}

#[test]
fn aggregate_errors_survive_omission_of_every_later_error_detail() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied(
            "bad",
            b"{\"x\":1,\"x\":2,\"y\":1,\"y\":2}",
        )],
        Limits::default().conditions(1),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let wire = json(&report);
    assert_eq!(report.outcome().errors(), 2);
    assert_eq!(report.outcome().disposition(), Disposition::Failed);
    assert_eq!(wire["diagnostics"]["retained"], 1);
    assert_eq!(wire["diagnostics"]["omitted"], 8);
    assert_eq!(wire["diagnostics"]["truncated"], true);
}

#[test]
fn pointer_identity_is_reserved_before_report_rendering() {
    let pointer = format!("/{}", "a".repeat(3000));
    let text = format!("{{\"{}\":null}}", "a".repeat(3000));
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("one", text.as_bytes())],
        Limits::default().report_bytes(22000),
    )
    .unwrap();
    assert!(
        catalogue
            .inspect(InspectRequest::new(
                catalogue.source_id("one").unwrap(),
                &pointer
            ))
            .is_err()
    );
}

#[test]
fn an_unreadable_required_source_is_incomplete_without_a_fabricated_syntax_error() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::failed("missing")]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
}

#[test]
fn a_truncated_json_token_is_a_limit_rather_than_a_syntax_violation() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("prefix", br#"{"x":"hello"}"#)],
        Limits::default().document_bytes(8),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
}

#[test]
fn payload_and_location_omissions_preserve_semantic_completion() {
    let text = format!("{{\"long\":\"{}\"}}", "x".repeat(10_000));
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("one", text.as_bytes())],
        Limits::default().report_bytes(22_000).pointer_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(
            catalogue.source_id("one").unwrap(),
            "/long",
        ))
        .unwrap();
    let wire = json(&report);
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
    assert_eq!(wire["result"]["state"], "partial");
    assert_eq!(wire["result"]["json_present"], false);
    assert_eq!(wire["result"]["pointer"], Value::Null);
    assert_eq!(wire["request"]["selector"]["pointer"], "/long");
    assert!(
        wire["diagnostics"]["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "location_limit")
    );
    assert!(report.to_human().contains("location_limit"));
    assert!(report.to_json().len() < 22_000);
}

fn consumer_summary(text: &str) -> Result<(String, bool, u64, u64), &'static str> {
    let wire: Value = serde_json::from_str(text).map_err(|_| "JSON")?;
    if wire["schema"] != "esdieff.report" || wire["version"] != "1.0" {
        return Err("version");
    }
    if wire["operation"] != "check" {
        return Err("unsupported operation");
    }
    let outcome = &wire["outcome"];
    Ok((
        outcome["disposition"]
            .as_str()
            .ok_or("disposition")?
            .to_owned(),
        outcome["complete"].as_bool().ok_or("completion")?,
        outcome["errors"].as_u64().ok_or("errors")?,
        outcome["incomplete_checks"].as_u64().ok_or("incomplete")?,
    ))
}

#[test]
fn a_structural_consumer_ignores_messages_additive_fields_and_detail_order() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("bad", b"[]")]).unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(
        consumer_summary(&report.to_json()).unwrap(),
        ("failed".to_owned(), false, 1, 7)
    );
    let mut wire = json(&report);
    wire["future_optional"] = serde_json::json!({"anything":true});
    let items = wire["diagnostics"]["items"].as_array_mut().unwrap();
    items.reverse();
    for condition in items {
        condition["message"] = serde_json::json!("Different human wording");
    }
    assert_eq!(
        consumer_summary(&wire.to_string()).unwrap(),
        ("failed".to_owned(), false, 1, 7)
    );
    wire["operation"] = serde_json::json!("future");
    assert!(consumer_summary(&wire.to_string()).is_err());
}

#[test]
fn rust_callers_read_conditions_without_rebuilding_or_parsing_report_json() {
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("duplicate", b"{\"x\":1,\"x\":2}")])
            .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let diagnostics = report.diagnostics();
    let duplicate = diagnostics
        .conditions()
        .find(|c| c.code() == "input.duplicate_name")
        .unwrap();
    assert_eq!(duplicate.severity(), esdieff::Severity::Error);
    assert_eq!(duplicate.basis().kind(), esdieff::BasisKind::Acceptance);
    assert_eq!(duplicate.basis().id(), Some("A002"));
    let location = location_support::primary_source(duplicate);
    assert_eq!(
        location.source_id(),
        catalogue.source_id("duplicate").unwrap()
    );
    assert!(location.span().is_some());
    assert_eq!(diagnostics.omitted(), 0);
    assert!(!diagnostics.truncated());
}

#[test]
fn a_tiny_diagnostic_budget_retains_only_mandatory_outcomes_and_scope() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("bad", b"[]")],
        Limits::default().diagnostic_bytes(1),
    )
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().strict_warnings(true))
        .unwrap();
    let wire = json(&report);
    assert_eq!(report.outcome().errors(), 1);
    coverage_support::assert_requested_counts(&report);
    assert_eq!(wire["diagnostics"]["retained"], 0);
    assert_eq!(wire["diagnostics"]["omitted"], 8);
    assert_eq!(wire["coverage"]["rows_omitted"], 8);
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert_eq!(wire["sources"].as_array().unwrap().len(), 1);
    assert_eq!(wire["policy"]["strict_warnings"], true);
    assert_eq!(wire["outcome"]["strict_warnings"], true);
}

#[test]
fn report_consumers_use_structural_outcomes_when_vocabulary_codes_are_unfamiliar() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("findings", br#"{"info":{"title":false,"versoin":"typo"},"sdfRequiredInputData":true,"example:quality":{"retained":true}}"#),
        SourceInput::supplied("critical", br#"{"info":{"features":["future"]},"sdfRequiredInputData":true}"#),
    ]).unwrap();
    for (key, expected) in [("findings", "failed"), ("critical", "undetermined")] {
        let report = catalogue
            .check(CheckRequest::new().root(catalogue.source_id(key).unwrap()))
            .unwrap();
        let wire = json(&report);
        assert_eq!(wire["schema"], "esdieff.report");
        assert_eq!(wire["version"], "1.0");
        assert_eq!(wire["operation"], "check");
        assert_eq!(wire["request"]["assessment"], "esdieff.initial/1");
        assert_eq!(wire["coverage"]["requested"]["total"], 8);
        assert_eq!(wire["outcome"]["disposition"], expected);
        assert_eq!(wire["outcome"]["complete"], key == "findings");
        // A reader that knows only older condition codes can still classify every new condition.
        let mut unfamiliar = 0;
        for condition in wire["diagnostics"]["items"].as_array().unwrap() {
            let code = condition["code"].as_str().unwrap();
            unfamiliar += usize::from(!matches!(
                code,
                "coverage.unimplemented" | "structure.feature_context_unavailable"
            ));
            assert!(matches!(
                condition["severity"].as_str().unwrap(),
                "error" | "warning" | "notice" | "incomplete"
            ));
            assert!(matches!(
                condition["basis"]["kind"].as_str().unwrap(),
                "rfc" | "advice" | "acceptance" | "operational"
            ));
            assert!(condition["arguments"].is_object());
        }
        assert!(unfamiliar > 0);
    }
    let report = catalogue
        .check(CheckRequest::new().root(catalogue.source_id("findings").unwrap()))
        .unwrap();
    let wire = json(&report);
    assert_eq!(wire["outcome"]["errors"], 2);
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["outcome"]["notices"], 1);
    coverage_support::assert_wire_counts(&wire);
}

#[test]
fn selected_parser_omissions_keep_their_count_cause_and_completed_view() {
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("earlier", br#"{"unknown":"\uD800"}"#),
            SourceInput::supplied("selected", br#"{"a":"\uD800","b":"\uD800","known":42}"#),
        ],
        Limits::default().conditions(1),
    )
    .unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(
            catalogue.source_id("selected").unwrap(),
            "/known",
        ))
        .unwrap();
    let wire = json(&report);
    assert_eq!(wire["diagnostics"]["retained"], 1);
    assert_eq!(wire["diagnostics"]["omitted"], 1);
    assert_eq!(
        wire["diagnostics"]["reasons"],
        serde_json::json!(["count_limit"])
    );
    assert_eq!(report.inspection_json().unwrap().raw_json(), "42");
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(report.outcome().incomplete_checks(), 0);
    assert!(report.to_human().contains("count_limit"));
    assert!(!report.to_human().contains("byte_limit"));
}

#[test]
fn selected_parser_omissions_keep_byte_causes_without_count_causes() {
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("earlier", br#"{"a":"\uD800","b":"\uD800","c":"\uD800"}"#),
            SourceInput::supplied(
                "selected",
                br#"{"a":"\uD800","b":"\uD800","c":"\uD800","known":42}"#,
            ),
        ],
        Limits::default().diagnostic_bytes(1024),
    )
    .unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(
            catalogue.source_id("selected").unwrap(),
            "/known",
        ))
        .unwrap();
    let wire = json(&report);
    assert_eq!(wire["diagnostics"]["retained"], 2);
    assert_eq!(wire["diagnostics"]["omitted"], 1);
    assert_eq!(
        wire["diagnostics"]["reasons"],
        serde_json::json!(["byte_limit"])
    );
    assert!(report.outcome().complete());
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
    assert!(report.to_human().contains("byte_limit"));
}

#[test]
fn unrelated_parser_omission_causes_do_not_leak_into_selected_reports() {
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("earlier", br#"{"a":"\uD800","b":"\uD800","c":"\uD800"}"#),
            SourceInput::supplied("selected", br#"{"unknown":"\uD800","known":42}"#),
        ],
        Limits::default().conditions(1),
    )
    .unwrap();
    let report = catalogue
        .inspect(InspectRequest::new(
            catalogue.source_id("selected").unwrap(),
            "/known",
        ))
        .unwrap();
    let wire = json(&report);
    assert_eq!(wire["diagnostics"]["retained"], 1);
    assert_eq!(wire["diagnostics"]["omitted"], 0);
    assert_eq!(wire["diagnostics"]["reasons"], serde_json::json!([]));
    assert_eq!(wire["diagnostics"]["truncated"], false);
    assert_eq!(
        wire["diagnostics"]["items"][0]["primary"]["source_id"],
        catalogue.source_id("selected").unwrap().get()
    );
    assert!(report.outcome().complete());
    let all = json(&catalogue.check(CheckRequest::new()).unwrap());
    assert!(
        all["diagnostics"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("count_limit"))
    );
}
