//! SARIF and Markdown projections stay within the report limit and count what they leave
//! out. Follows ADR 0043: SARIF and Markdown report projections, and ADR 0009: Bounded
//! processing and reporting.
use esdieff::{Catalogue, CheckRequest, ComparisonRequest, Limits, SourceInput};
use serde_json::Value;
use std::path::Path;

/// A model with an error and `count` unfamiliar qualities, each a warning.
fn unfamiliar(count: usize) -> String {
    let qualities: Vec<_> = (0..count).map(|n| format!("\"q{n}\": {n}")).collect();
    format!(
        r#"{{"info": {{"title": "T", "version": "1", "copyright": "x", "license": "x"}},
"sdfProperty": {{"p": {{"minimum": "low", {}}}}}}}"#,
        qualities.join(", ")
    )
}

#[test]
fn check_logs_stay_within_a_small_report_limit_and_count_omissions() {
    let model = unfamiliar(300);
    let limit = 32 * 1024;
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied("model", model.as_bytes())],
        Limits::default().report_bytes(limit),
    )
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    let outcome = report.outcome();
    let findings = u64::from(outcome.errors() + outcome.warnings() + outcome.notices());
    assert_eq!(findings, 301);

    let text = report.to_sarif(Path::new(""));
    assert!(text.len() <= limit, "{} > {limit}", text.len());
    let log: Value = serde_json::from_str(&text).unwrap();
    let run = &log["runs"][0];
    let results = run["results"].as_array().unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0]["level"], "error", "errors come first");
    let omitted = run["properties"]["omittedResults"].as_u64().unwrap();
    assert!(omitted > 0);
    assert_eq!(omitted + results.len() as u64, findings);
    let notifications = run["invocations"][0]["toolExecutionNotifications"]
        .as_array()
        .unwrap();
    let message = notifications.last().unwrap()["message"]["text"]
        .as_str()
        .unwrap();
    assert!(
        message.starts_with(&format!("{omitted} results ")),
        "{message}"
    );
}

/// Earlier and later revisions where the later one drops `count` Properties, each a breaking
/// difference.
fn removals(count: usize, limit: usize) -> (Catalogue, Catalogue) {
    let properties: Vec<_> = (0..count)
        .map(|n| format!("\"p{n}\": {{\"type\": \"number\"}}"))
        .collect();
    let earlier = format!(
        r#"{{"sdfObject": {{"Meter": {{"sdfProperty": {{{}}}}}}}}}"#,
        properties.join(", ")
    );
    let later = r#"{"sdfObject": {"Meter": {}}}"#;
    let catalogue = |text: &str| {
        Catalogue::with_limits(
            &[SourceInput::supplied("meter.sdf.json", text.as_bytes())],
            Limits::default().report_bytes(limit),
        )
        .unwrap()
    };
    (catalogue(&earlier), catalogue(later))
}

#[test]
fn diff_logs_stay_within_a_small_report_limit_and_count_omissions() {
    let limit = 48 * 1024;
    let (earlier, later) = removals(300, limit);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let text = comparison.to_sarif(Path::new(""));
    assert!(text.len() <= limit, "{} > {limit}", text.len());
    let log: Value = serde_json::from_str(&text).unwrap();
    let run = &log["runs"][0];
    let results = run["results"].as_array().unwrap().len() as u64;
    let omitted = run["properties"]["omittedResults"].as_u64().unwrap();
    assert!(results > 0 && omitted > 0);
    assert_eq!(results + omitted, 300);
}

#[test]
fn changelogs_stay_within_a_small_report_limit_and_end_with_the_omitted_count() {
    // A thousand list items are longer than the limit.
    let limit = 48 * 1024;
    let (earlier, later) = removals(1000, limit);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let text = comparison.to_markdown();
    assert!(text.len() <= limit, "{} > {limit}", text.len());
    let listed = text.lines().filter(|line| line.starts_with("- ")).count();
    assert!(listed > 0);
    assert!(
        text.ends_with(&format!(
            "\n{} further differences are not listed, because their details did not fit the report limit.\n",
            1000 - listed
        )),
        "{text}"
    );
}
