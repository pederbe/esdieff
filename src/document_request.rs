//! Request mechanics shared by Model authoring and Authored-quality editing, whose fidelity
//! contracts stay separate under ADR 0017, Reference-preserving sdfThing authoring, and
//! ADR 0034, Authored-quality editing with source substitution.
use crate::{Assessment, Field, JsonView, Outcome, Report, RequestedCoverage, SourceId};
use serde_json::{Value, json};
use std::{borrow::Cow, ops::Range};

/// Requested Model checking of a generated document, owned by the operation's result.
pub(crate) struct RequestedChecking {
    outcome: Outcome,
    assessment: Assessment,
    coverage: RequestedCoverage,
    requirements: Option<RequestedCoverage>,
    roots: Vec<SourceId>,
}
impl RequestedChecking {
    pub(crate) fn from_report(report: &Report<'_>) -> Self {
        Self {
            outcome: report.outcome().clone(),
            assessment: report.assessment().expect("generated document assessment"),
            coverage: report.requested_coverage(),
            requirements: report.requirement_coverage().present(),
            roots: report.requested_source_ids().to_owned(),
        }
    }
    /// Checking that could not run against the generated source stays undetermined.
    pub(crate) fn unavailable(assessment: Assessment, source: SourceId, strict: bool) -> Self {
        Self {
            outcome: crate::report::author_outcome(false, 0, 0, 0, strict),
            assessment,
            coverage: crate::report::author_unavailable_check_coverage(),
            requirements: None,
            roots: vec![source],
        }
    }
    /// The wire envelope of unavailable checking, matching `unavailable`.
    pub(crate) fn unavailable_wire(
        assessment: Assessment,
        source: SourceId,
        strict: bool,
    ) -> Value {
        json!({
            "request":{"assessment":if assessment == Assessment::Initial { "esdieff.initial/1" } else { "rfc9880/base-1" },"inventory":null,
                "scope":{"kind":"selected","roots":[source.get()],"dependencies":[],"assessed":[],"unavailable_required":[source.get()]}},
            "outcome":{"complete":false,"errors":0,"warnings":0,"notices":0,"incomplete_checks":1,"strict_warnings":strict,"disposition":"undetermined"},
            "coverage":{"requested":{"inventory":null,"count_unit":"assessment_area","total":null,"assessed":0,"not_applicable":0,"unassessed":0,"undetermined":0},"broader":{"inventory":null,"state":"partial","inventory_status":"unavailable"},"rows":[],"rows_omitted":0,"omissions":[]},
            "diagnostics":{"items":[],"omitted":0,"truncated":false,"reasons":[],"omissions":[]}
        })
    }
    pub(crate) fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    pub(crate) fn coverage(&self) -> &RequestedCoverage {
        &self.coverage
    }
    pub(crate) fn requirements(&self) -> Option<&RequestedCoverage> {
        self.requirements.as_ref()
    }
}

/// Public reading of optional requested checking, shared by both operations' wrappers.
pub(crate) struct RequestedCheckingView<'a>(Option<&'a RequestedChecking>);
impl<'a> RequestedCheckingView<'a> {
    pub(crate) fn new(value: Option<&'a RequestedChecking>) -> Self {
        Self(value)
    }
    pub(crate) fn assessment(&self) -> Option<Assessment> {
        self.0.map(|check| check.assessment)
    }
    pub(crate) fn outcome(&self) -> Option<&'a Outcome> {
        self.0.map(|check| &check.outcome)
    }
    pub(crate) fn requested_coverage(&self) -> Option<&'a RequestedCoverage> {
        self.0.map(|check| &check.coverage)
    }
    pub(crate) fn requirement_coverage(&self) -> Field<&'a RequestedCoverage> {
        if self.0.is_some_and(|check| {
            check.assessment == Assessment::FullBaseline && check.requirements.is_none()
        }) {
            return Field::Unavailable("requested requirement inventory unavailable");
        }
        self.0
            .and_then(|check| check.requirements.as_ref())
            .map_or(Field::Absent, Field::Present)
    }
    pub(crate) fn requested_source_ids(&self) -> Option<&'a [SourceId]> {
        self.0.map(|check| check.roots.as_slice())
    }
}

/// A decoded coordinate in a request plan, distinct from every SDF source identity. It is
/// built from the owned plan or read back from a reported condition.
pub(crate) struct RequestLocation<'a> {
    key: &'a str,
    pointer: Option<Cow<'a, str>>,
    span: Option<Range<u32>>,
}
impl<'a> RequestLocation<'a> {
    /// The span covers the located textual-plan value. Typed requests locate nothing.
    pub(crate) fn new(key: &'a str, pointer: String, located: Option<JsonView<'_>>) -> Self {
        let span = located.map(|value| {
            let span = value.span();
            span.start as u32..span.end as u32
        });
        Self {
            key,
            pointer: Some(Cow::Owned(pointer)),
            span,
        }
    }
    /// A location read back from report JSON, where a null pointer is an omitted presentation.
    pub(crate) fn from_report(location: &'a Value) -> Self {
        Self {
            key: location["key"].as_str().expect("request key"),
            pointer: location["pointer"].as_str().map(Cow::Borrowed),
            span: crate::report::reported_span(location),
        }
    }
    pub(crate) fn key(&self) -> &str {
        self.key
    }
    pub(crate) fn pointer(&self) -> Field<&str> {
        self.pointer.as_deref().map_or(
            Field::Unavailable("request pointer presentation unavailable"),
            Field::Present,
        )
    }
    pub(crate) fn span(&self) -> Option<Range<u32>> {
        self.span.clone()
    }
}
