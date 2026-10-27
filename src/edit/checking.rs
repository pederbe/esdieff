//! Own ordinary checking without extending the evaluator's scope or inventories.
use super::*;
use crate::document_request::{RequestedChecking, RequestedCheckingView};
use crate::report::projection::Projection;
use serde_json::{Value, json};

// The independent inventory aggregate and its row omission records remain
// mandatory even when ordinary optional requirement details are omitted.
pub(super) const MANDATORY_RESERVE: usize = 2048;

pub(super) struct Checking {
    pub(super) wire: Value,
    pub(super) projection: Option<Projection>,
    requested: RequestedChecking,
}

/// Requested Model checking remains independent of editing fidelity.
pub struct EditChecking<'a>(RequestedCheckingView<'a>);
impl<'a> EditChecking<'a> {
    pub(super) fn new(value: Option<&'a Checking>) -> Self {
        Self(RequestedCheckingView::new(
            value.map(|check| &check.requested),
        ))
    }
    pub fn assessment(&self) -> Option<Assessment> {
        self.0.assessment()
    }
    pub fn outcome(&self) -> Option<&'a Outcome> {
        self.0.outcome()
    }
    pub fn requested_coverage(&self) -> Option<&'a crate::RequestedCoverage> {
        self.0.requested_coverage()
    }
    pub fn requirement_coverage(&self) -> crate::Field<&'a crate::RequestedCoverage> {
        self.0.requirement_coverage()
    }
    pub fn requested_source_ids(&self) -> Option<&'a [SourceId]> {
        self.0.requested_source_ids()
    }
}
impl Checking {
    pub(super) fn outcome(&self) -> &Outcome {
        self.requested.outcome()
    }
    pub(super) fn from_report(report: crate::Report<'_>) -> Self {
        let (mut wire, projection) = report.author_projection();
        let requirements = report.requirement_coverage().present();
        if let Some(coverage) = &requirements
            && !wire["coverage"]["requirements"].is_object()
        {
            // Editing retains authoritative inventory totals in its mandatory
            // envelope even when the ordinary optional detail summary did not fit.
            wire["coverage"]["requirements"] = json!({"inventory":coverage.inventory(),"count_unit":coverage.count_unit(),"total":coverage.total(),"assessed":coverage.assessed(),"not_applicable":coverage.not_applicable(),"unassessed":coverage.unassessed(),"undetermined":coverage.undetermined()});
            wire["coverage"]["requirement_rows"] = json!([]);
            wire["coverage"]["requirement_rows_retained"] = json!(0);
            wire["coverage"]["requirement_rows_omitted"] = json!(coverage.total());
            let omission = json!({"field":"coverage.requirement_rows","reason":"byte_limit","omitted_count":coverage.total()});
            wire["coverage"]["requirement_omissions"] = json!([omission]);
            wire["diagnostics"]["omissions"]
                .as_array_mut()
                .expect("ordinary omissions")
                .push(omission);
            wire["diagnostics"]["truncated"] = json!(true);
            let reasons = wire["diagnostics"]["reasons"]
                .as_array_mut()
                .expect("ordinary omission reasons");
            if !reasons.contains(&json!("byte_limit")) {
                reasons.push(json!("byte_limit"));
            }
        }
        Self {
            wire,
            projection: Some(projection),
            requested: RequestedChecking::from_report(&report),
        }
    }
    pub(super) fn unavailable(assessment: Assessment, source: SourceId, strict: bool) -> Self {
        Self {
            wire: RequestedChecking::unavailable_wire(assessment, source, strict),
            projection: None,
            requested: RequestedChecking::unavailable(assessment, source, strict),
        }
    }
}
