use esdieff::{Catalogue, Disposition, HumanStyle, InspectRequest, Limits, SourceInput};
#[test]
fn a_tight_style_budget_preserves_the_same_retained_payload_and_omissions() {
    let text = format!("{{\"passed\":\"{}\"}}", "warning error ".repeat(1600));
    let inputs = [SourceInput::supplied("model", text.as_bytes())];
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let plain_report = catalogue
        .inspect(InspectRequest::new(id, ""))
        .unwrap()
        .to_human();
    let limit = plain_report.len() + 128;
    let limited = Catalogue::with_limits(&inputs, Limits::default().report_bytes(limit)).unwrap();
    let report = limited.inspect(InspectRequest::new(id, "")).unwrap();
    let ordinary = report.to_human();
    let requested = report.to_human_with_style(HumanStyle::Ansi);
    assert_eq!(
        requested, ordinary,
        "ANSI overhead must fall back to the same plain report"
    );
    assert!(ordinary.contains(&text));
    assert_eq!(ordinary, plain_report);
    assert!(ordinary.len() <= limit);
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
}
