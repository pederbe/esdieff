//! Stable reports preserve scope and aggregate outcomes when optional presentation is omitted.
//! Follows ADR0011: Machine-readable report contracts, and
//! ADR0013: Shared workflow and report guarantees.
use crate::human::{HumanStyle, HumanText, Role};
use crate::{Acquisition, Catalogue, Field, JsonView, SourceId};
use serde_json::{Value, json};
pub(crate) mod projection;
mod required;
pub(crate) mod selected;
use crate::assessment::StatusSource;
use projection::{DetailBudget, Projection, retain};

struct JsonStringWriter<'a>(&'a mut String);
impl std::io::Write for JsonStringWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        // serde_json emits UTF-8 fragments; validate each fragment before appending.
        let text = std::str::from_utf8(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
        self.0.push_str(text);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

use crate::assessment::MODIFIED_AREA;
use crate::json::validate_pointer;
const REQUIRED_REPORT_RESERVE: usize = 1024;

/// Source-bound common ownership within the existing fixed C share. Five large
/// map slots cover base/outcome/current+retained inspection result and staging;
/// twelve small slots cover request/scope/baseline/policy/coverage/requested/
/// broader/diagnostics/selector and three omission copies. The prior human result
/// survives fallback. Optional detail, selectors and sample/registry extensions
/// retain their independent charges. Static key/value heaps fit8192 bytes.
pub(crate) fn common_owner_bytes() -> usize {
    use std::mem::size_of;
    5 * 4096
        + 12 * 1536
        + 8192
        + size_of::<Report<'_>>()
        + 8 * (size_of::<usize>() + size_of::<String>() + size_of::<Vec<Value>>() + 128)
        + 2 * size_of::<required::Presentation>()
        + size_of::<RequestedCoverage>()
        + projection::owner_bytes()
}

fn sample_basis_wire(basis: crate::SampleBasisRef) -> Value {
    let mut value = json!({"kind": match basis.kind() {
        BasisKind::Rfc => "rfc",
        BasisKind::Interpretation => "interpretation",
        _ => "operational",
    }});
    if let Some(id) = basis.id() {
        value["id"] = json!(id);
    }
    if let Some(citation) = basis.citation() {
        value["citation"] = json!(citation);
    }
    value
}
fn sample_meaning_wire(sample: &crate::SampleResult<'_>) -> Value {
    let meaning = sample.meaning();
    let area = |area: crate::SampleMeaningArea<'_>| {
        json!({"status":area.status().as_str(),
            "basis":area.basis().map(sample_basis_wire).collect::<Vec<_>>(),
            "limitations":area.limitations().map(sample_basis_wire).collect::<Vec<_>>()})
    };
    let admission = meaning.admission();
    json!({"abstract_values":area(meaning.abstract_values()), "const":area(meaning.const_values()),
        "admission":{"basis":admission.basis().map(sample_basis_wire).collect::<Vec<_>>(),
            "limitations":admission.limitations().map(sample_basis_wire).collect::<Vec<_>>()}})
}
fn human_sample_basis(
    output: &mut HumanText,
    label: &str,
    mut values: impl ExactSizeIterator<Item = crate::SampleBasisRef>,
) {
    output.heading(&format!("{label}:"));
    output.push(' ');
    if values.len() == 0 {
        output.push_str("none");
    }
    for (index, basis) in values.by_ref().enumerate() {
        if index != 0 {
            output.push_str("; ");
        }
        if let Some(id) = basis.id() {
            output.push_str(id);
        }
        if let Some(citation) = basis.citation() {
            if basis.id().is_some() {
                output.push_str(" (");
            }
            output.push_str(citation);
            if basis.id().is_some() {
                output.push(')');
            }
        }
    }
    output.push('\n');
}

fn sample_coverage_basis(
    area: &str,
    status: AssessmentStatus,
    state: &crate::sample::State,
) -> Value {
    match status {
        AssessmentStatus::Unassessed => {
            json!({"kind":"operational","id":"sample-implementation"})
        }
        AssessmentStatus::Undetermined => {
            json!({"kind":"operational","id":if state.abstract_unavailable(area) { "sample-abstract-denotation-unavailable" } else { "sample-establishment" }})
        }
        _ if area == crate::sample::AREAS[0] => {
            json!({"kind":"operational","id":"sample-establishment"})
        }
        _ => {
            let basis = if area == crate::sample::AREAS[1] {
                state.type_null_basis
            } else if area == crate::sample::AREAS[3] {
                state.text_boolean_basis
            } else if area == crate::sample::AREAS[4] {
                state.const_basis()
            } else if area == crate::sample::AREAS[5] {
                crate::finding::Basis::Alternatives
            } else if area == crate::sample::AREAS[6] {
                crate::finding::Basis::Map
            } else if area == crate::sample::AREAS[7] {
                crate::finding::Basis::Array
            } else if area == crate::sample::AREAS[8] {
                state.abstract_basis
            } else {
                crate::finding::Basis::Numeric
            };
            let mut value = json!({"kind":basis.kind(),"citation":basis.citation()});
            if let Some(id) = basis.id() {
                value["id"] = json!(id);
            }
            value
        }
    }
}

fn unfinished_check_basis(area: &str, status: AssessmentStatus) -> &'static str {
    if status == AssessmentStatus::Unassessed {
        "assessment-implementation"
    } else if area == MODIFIED_AREA {
        "modified-establishment"
    } else if area == crate::pattern::AREA {
        "pattern-establishment"
    } else if area == crate::proof::AREA {
        "advisory-establishment"
    } else if area == crate::proof::NUMERIC_AREA {
        "expanded-numeric-establishment"
    } else if crate::units::AREAS.contains(&area) {
        "unit-establishment"
    } else if crate::content_format::AREAS.contains(&area) {
        "content-format-establishment"
    } else {
        "assessment-establishment"
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Assessment {
    Initial,
    ExpandedModified,
    ExpandedUnits,
    ExpandedContentFormat,
    ExpandedPatternSyntax,
    ExpandedNumeric,
    ExpandedConsistency,
    FullBaseline,
}
impl Assessment {
    fn identity(self) -> &'static str {
        crate::assessment::profile(self).identity
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AssessmentStatus {
    Assessed,
    NotApplicable,
    Unassessed,
    Undetermined,
}
impl AssessmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Assessed => "assessed",
            Self::NotApplicable => "not_applicable",
            Self::Unassessed => "unassessed",
            Self::Undetermined => "undetermined",
        }
    }
}
pub struct RequestedCoverage {
    inventory: Option<&'static str>,
    count_unit: &'static str,
    total: Option<u32>,
    assessed: u32,
    not_applicable: u32,
    unassessed: u32,
    undetermined: u32,
}
impl RequestedCoverage {
    fn accumulate(&mut self, status: AssessmentStatus) {
        match status {
            AssessmentStatus::Assessed => self.assessed += 1,
            AssessmentStatus::NotApplicable => self.not_applicable += 1,
            AssessmentStatus::Unassessed => self.unassessed += 1,
            AssessmentStatus::Undetermined => self.undetermined += 1,
        }
    }
    pub fn inventory(&self) -> Option<&str> {
        self.inventory
    }
    pub fn count_unit(&self) -> &str {
        self.count_unit
    }
    pub fn total(&self) -> Option<u32> {
        self.total
    }
    pub fn assessed(&self) -> u32 {
        self.assessed
    }
    pub fn not_applicable(&self) -> u32 {
        self.not_applicable
    }
    pub fn unassessed(&self) -> u32 {
        self.unassessed
    }
    pub fn undetermined(&self) -> u32 {
        self.undetermined
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Disposition {
    Passed,
    Failed,
    Undetermined,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Severity {
    Error,
    Warning,
    Notice,
    Incomplete,
}
impl Severity {
    /// Decode a reported severity. Reports carry only the four fixed severities.
    pub(crate) fn from_report(severity: &str) -> Self {
        match severity {
            "error" => Self::Error,
            "warning" => Self::Warning,
            "notice" => Self::Notice,
            "incomplete" => Self::Incomplete,
            _ => unreachable!("fixed condition severity"),
        }
    }
    /// The severity's wire name, which `from_report` decodes.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Notice => "notice",
            Self::Incomplete => "incomplete",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BasisKind {
    Json,
    Rfc,
    Interpretation,
    Deviation,
    Acceptance,
    Advice,
    Operational,
    /// A rule from a recorded registration outside RFC 9880, such as a registered
    /// Quality Name's value domain. It carries the registration's citation and the
    /// registered identifier.
    Registration,
}
impl BasisKind {
    /// The basis kind's wire name, which `Basis::kind` decodes.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Rfc => "rfc",
            Self::Interpretation => "interpretation",
            Self::Deviation => "deviation",
            Self::Acceptance => "acceptance",
            Self::Advice => "advice",
            Self::Operational => "operational",
            Self::Registration => "registration",
        }
    }
}
pub struct Basis<'a>(&'a Value);
impl Basis<'_> {
    pub fn kind(&self) -> BasisKind {
        match self.0["kind"].as_str().expect("basis kind") {
            "json" => BasisKind::Json,
            "rfc" => BasisKind::Rfc,
            "interpretation" => BasisKind::Interpretation,
            "deviation" => BasisKind::Deviation,
            "acceptance" => BasisKind::Acceptance,
            "advice" => BasisKind::Advice,
            "operational" => BasisKind::Operational,
            "registration" => BasisKind::Registration,
            _ => unreachable!("fixed basis kind"),
        }
    }
    pub fn citation(&self) -> Option<&str> {
        self.0["citation"].as_str()
    }
    pub fn id(&self) -> Option<&str> {
        self.0["id"].as_str()
    }
}
pub struct SourceLocation<'a>(&'a Value);
impl SourceLocation<'_> {
    pub fn source_id(&self) -> SourceId {
        SourceId(self.0["source_id"].as_u64().expect("source id") as u32)
    }
    pub fn pointer(&self) -> Field<&str> {
        self.0["pointer"].as_str().map_or(
            Field::Unavailable("source pointer presentation unavailable"),
            Field::Present,
        )
    }
    pub fn span(&self) -> Option<std::ops::Range<u32>> {
        reported_span(self.0)
    }
}
/// A located byte span in report JSON. A null span is unavailable.
pub(crate) fn reported_span(location: &Value) -> Option<std::ops::Range<u32>> {
    let array = location["span"].as_array()?;
    Some(array[0].as_u64()? as u32..array[1].as_u64()? as u32)
}
/// A reported effective location. It names the root of its effective-document space and
/// claims no source identity or byte span. Unlike the live `EffectiveLocation` of an
/// effective model, it holds only what the report recorded.
pub struct EffectiveReportLocation<'a>(&'a Value);
impl EffectiveReportLocation<'_> {
    pub fn root_document(&self) -> SourceId {
        SourceId(self.0["root"].as_u64().expect("effective root") as u32)
    }
    pub fn pointer(&self) -> Field<&str> {
        self.0["pointer"].as_str().map_or(
            Field::Unavailable("effective pointer presentation unavailable"),
            Field::Present,
        )
    }
}
/// A primary or related location of a condition, by kind.
#[non_exhaustive]
pub enum ReportLocation<'a> {
    Source(SourceLocation<'a>),
    Effective(EffectiveReportLocation<'a>),
    Sample(SourceLocation<'a>),
    AuthorRequest(crate::AuthorRequestLocation<'a>),
    EditRequest(crate::EditRequestLocation<'a>),
}
impl<'a> ReportLocation<'a> {
    fn from_report(location: &'a Value) -> Self {
        match location["kind"].as_str() {
            Some("source") => Self::Source(SourceLocation(location)),
            Some("effective") => Self::Effective(EffectiveReportLocation(location)),
            Some("sample") => Self::Sample(SourceLocation(location)),
            Some("author_request") => {
                Self::AuthorRequest(crate::AuthorRequestLocation::from_report(location))
            }
            Some("edit_request") => {
                Self::EditRequest(crate::EditRequestLocation::from_report(location))
            }
            _ => unreachable!("fixed location kind"),
        }
    }
}
pub struct Condition(Value);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ConditionDomain {
    Model,
    Sample,
    Operation,
}
impl Condition {
    pub fn domain(&self) -> Option<ConditionDomain> {
        match self.0["domain"].as_str() {
            Some("model") => Some(ConditionDomain::Model),
            Some("sample") => Some(ConditionDomain::Sample),
            Some("operation") => Some(ConditionDomain::Operation),
            None => None,
            Some(_) => unreachable!("fixed condition domain"),
        }
    }
    pub fn code(&self) -> &str {
        self.0["code"].as_str().expect("condition code")
    }
    pub fn severity(&self) -> Severity {
        Severity::from_report(self.0["severity"].as_str().expect("condition severity"))
    }
    pub fn basis(&self) -> Basis<'_> {
        Basis(&self.0["basis"])
    }
    pub fn message(&self) -> &str {
        self.0["message"].as_str().expect("condition message")
    }
    pub fn assessment_unit(&self) -> Option<&str> {
        self.0["assessment_unit"].as_str()
    }
    /// Optional requested Full owner, preserving the producing assessment unit.
    pub fn coverage_owner(&self) -> Option<&str> {
        self.0["coverage_owner"].as_str()
    }
    /// The single location the condition is about, of any kind. It is absent when the
    /// condition has no meaningful single location and unavailable when the report could
    /// not establish one.
    pub fn primary(&self) -> Field<ReportLocation<'_>> {
        match self.0.get("primary") {
            None => Field::Absent,
            Some(Value::Null) => Field::Unavailable("primary location unavailable"),
            Some(location) => Field::Present(ReportLocation::from_report(location)),
        }
    }
    /// Related locations in report order. Source and sample locations carry a source id
    /// and, when available, a byte span. Effective locations carry the root of their
    /// effective-document space rather than a source id, and no span.
    pub fn related(&self) -> impl ExactSizeIterator<Item = ReportLocation<'_>> {
        self.0["related"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(ReportLocation::from_report)
    }
}
pub struct Diagnostics {
    conditions: Vec<Condition>,
    omitted: u32,
    truncated: bool,
}
impl Diagnostics {
    pub fn conditions(&self) -> impl ExactSizeIterator<Item = &Condition> {
        self.conditions.iter()
    }
    pub fn omitted(&self) -> u32 {
        self.omitted
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}
impl Disposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Undetermined => "undetermined",
        }
    }
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Passed => 0,
            Self::Failed => 1,
            Self::Undetermined => 3,
        }
    }
}
#[derive(Clone, Debug)]
pub struct CheckRequest {
    roots: Vec<SourceId>,
    assessment: Assessment,
    strict: bool,
    too_many_roots: bool,
    registry_policy: Option<crate::RecordedRegistryPolicy>,
}
impl Default for CheckRequest {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            assessment: Assessment::Initial,
            strict: false,
            too_many_roots: false,
            registry_policy: None,
        }
    }
}
impl CheckRequest {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn root(mut self, id: SourceId) -> Self {
        if self.roots.len() < 256 {
            self.roots.push(id);
        } else {
            self.too_many_roots = true;
        }
        self
    }
    pub fn assessment(mut self, value: Assessment) -> Self {
        self.assessment = value;
        self
    }
    pub fn strict_warnings(mut self, value: bool) -> Self {
        self.strict = value;
        self
    }
    /// Control whether this request may use the bundled dated registry evidence.
    pub fn recorded_registry_policy(mut self, value: crate::RecordedRegistryPolicy) -> Self {
        self.registry_policy = Some(value);
        self
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum View {
    Source,
    Effective,
}
impl View {
    fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Effective => "effective",
        }
    }
}
pub struct InspectRequest<'a> {
    source: SourceId,
    pointer: &'a str,
    view: View,
    strict: bool,
    required: bool,
}
impl<'a> InspectRequest<'a> {
    pub fn new(source: SourceId, pointer: &'a str) -> Self {
        Self {
            source,
            pointer,
            view: View::Source,
            strict: false,
            required: false,
        }
    }
    pub fn view(mut self, view: View) -> Self {
        self.view = view;
        self
    }
    pub fn strict_warnings(mut self, value: bool) -> Self {
        self.strict = value;
        self
    }
    /// Resolve required declaration relationships within this inspection scope.
    pub fn required_declarations(mut self, value: bool) -> Self {
        self.required = value;
        self
    }
}
#[derive(Clone, Debug)]
pub struct Outcome {
    complete: bool,
    errors: u32,
    warnings: u32,
    notices: u32,
    incomplete: u32,
    strict: bool,
}
impl Outcome {
    pub fn complete(&self) -> bool {
        self.complete
    }
    pub fn errors(&self) -> u32 {
        self.errors
    }
    pub fn warnings(&self) -> u32 {
        self.warnings
    }
    pub fn notices(&self) -> u32 {
        self.notices
    }
    pub fn incomplete_checks(&self) -> u32 {
        self.incomplete
    }
    pub fn strict_warnings(&self) -> bool {
        self.strict
    }
    pub fn disposition(&self) -> Disposition {
        if self.errors > 0 || self.strict && self.warnings > 0 {
            Disposition::Failed
        } else if !self.complete {
            Disposition::Undetermined
        } else {
            Disposition::Passed
        }
    }
}
enum Operation<'a> {
    Sample,
    Check(Assessment),
    Inspect {
        document: SourceId,
        pointer: &'a str,
        view: View,
        json: Option<JsonView<'a>>,
    },
}
pub struct Report<'a> {
    sample: Option<crate::sample::State>,
    catalogue: &'a Catalogue,
    operation: Operation<'a>,
    scope: Vec<SourceId>,
    selected: bool,
    outcome: Outcome,
    structure: crate::structure::Structure<'a>,
    effective: Vec<crate::EffectiveModel<'a>>,
    effective_limit: Option<(SourceId, crate::effective::Failure)>,
    dependencies: Vec<SourceId>,
    unavailable_dependencies: Vec<SourceId>,
    modified_sources: usize,
    required: Option<crate::effective::Collection<'a>>,
    units: Option<crate::units::State>,
    content: Option<crate::content_format::State>,
    registry: crate::registry::State,
}
impl Catalogue {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn check_sample(
        &self,
        request: crate::SampleRequest<'_>,
    ) -> Result<Report<'_>, &'static str> {
        let mut budget = crate::allowance::OperationBudget::new(&self.limits);
        let mut structure = crate::structure::Structure::default();
        let mut ledger = crate::structure::ParameterLedger {
            storage_remaining: self.limits.detail_bytes,
        };
        let (model, mut state) =
            crate::sample::select(self, &request, &mut budget, &mut structure, &mut ledger)?;
        let mut effective = crate::sample::mandatory_slot(&mut state)?;
        effective.push(model);
        let mut effective_limit = None;
        let mut scope = crate::sample::mandatory_slot(&mut state)?;
        scope.push(request.model);
        let (dependencies, unavailable_dependencies) =
            dependency_scope(self, &scope, &effective, &mut budget, &mut effective_limit);
        crate::sample::evaluate(self, &effective[0], &mut state, &mut budget);
        // Recorded on actual use, following ADR 0037: Undeclared extension vocabulary.
        structure.set_aside |= budget.set_aside;
        crate::sample::seal_meaning(
            &mut state,
            (effective_limit.is_some() || structure.work_limited).then_some(if budget.work == 0 {
                3
            } else {
                0
            }),
        );
        let mut input_errors = 0u32;
        let sample_id = state.sample;
        for id in scope
            .iter()
            .chain(dependencies.iter())
            .chain(std::iter::once(&sample_id))
        {
            let source = self.source(*id).expect("needed source");
            if source.parsed.rejected || source.parsed.root.is_none() {
                input_errors += source.parsed.errors as u32;
                if source.role.is_model() {
                    state.context_findings.errors += source.parsed.errors as u32;
                }
            }
        }
        state.context_findings.errors += structure.errors + effective[0].reference_errors;
        state.context_findings.warnings = structure.warnings;
        state.context_findings.notices = structure.notices;
        let incomplete = state
            .areas
            .iter()
            .filter(|area| {
                matches!(
                    area,
                    AssessmentStatus::Unassessed | AssessmentStatus::Undetermined
                )
            })
            .count() as u32;
        let errors = input_errors + structure.errors + effective[0].reference_errors + state.errors;
        Ok(Report {
            catalogue: self,
            operation: Operation::Sample,
            scope,
            selected: true,
            outcome: Outcome {
                complete: incomplete == 0,
                errors,
                warnings: structure.warnings,
                notices: structure.notices,
                incomplete,
                strict: request.strict,
            },
            structure,
            effective,
            effective_limit,
            dependencies,
            unavailable_dependencies,
            modified_sources: 0,
            sample: Some(state),
            required: None,
            units: None,
            content: None,
            registry: crate::registry::State::default(),
        })
    }
    pub fn check(&self, request: CheckRequest) -> Result<Report<'_>, &'static str> {
        let mut budget = crate::allowance::OperationBudget::new(&self.limits);
        self.check_with_budget(request, &mut budget)
    }
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(crate) fn check_with_budget(
        &self,
        request: CheckRequest,
        operation_budget: &mut crate::allowance::OperationBudget,
    ) -> Result<Report<'_>, &'static str> {
        if request.too_many_roots {
            return Err("selected roots exceed source-attempt ceiling");
        }
        validate_roots(self, &request.roots)?;
        let profile = crate::assessment::profile(request.assessment);
        profile.admit_reporting(
            self.reserve,
            self.limits.report_bytes,
            request.registry_policy.is_some(),
        )?;
        let units_requested = profile.enabled.units;
        let content_requested = profile.enabled.content_format;
        let registry_requested = profile.registry_requested();
        let pattern_requested = profile.enabled.pattern;
        // Charge the new assessment's source eligibility pass before it visits
        // any row, including rows excluded because their source role is Sample.
        let registry_scope_limited = registry_requested
            && !crate::allowance::charge(&mut operation_budget.work, self.sources.len());
        let selected = !request.roots.is_empty();
        let scope = if selected {
            request.roots
        } else {
            self.sources
                .iter()
                .filter(|s| s.role.is_model())
                .map(|s| s.id)
                .collect()
        };
        let errors: u32 = scope
            .iter()
            .map(|id| self.source(*id).expect("validated source").parsed.errors as u32)
            .sum();
        use crate::registry::{CONTENT_RECORDS, RECORDED_EXTENSION_RECORDS, UNIT_RECORDS, bits};
        let mut registry = crate::registry::State {
            policy: request.registry_policy.unwrap_or_default(),
            requested: (u8::from(units_requested) * bits(UNIT_RECORDS))
                | (u8::from(content_requested) * bits(CONTENT_RECORDS))
                | (u8::from(profile.enabled.full) * bits(RECORDED_EXTENSION_RECORDS)),
            ..Default::default()
        };
        let owners_admitted = !registry_requested
            || crate::allowance::charge(
                &mut operation_budget.allocation,
                16 + usize::from(units_requested) * 128 + usize::from(content_requested) * 64,
            );
        registry.owner_failed = !owners_admitted;
        let mut units = (units_requested && owners_admitted).then(crate::units::State::default);
        let mut content =
            (content_requested && owners_admitted).then(crate::content_format::State::default);
        let mut structure =
            crate::structure::Structure::check(self, &scope, operation_budget, request.assessment);
        let mut effective = Vec::new();
        let mut effective_limit = None;
        for id in &scope {
            if let Err(reason) =
                crate::allowance::admit(&mut effective, &mut operation_budget.allocation)
            {
                effective_limit = Some((*id, reason.into()));
                break;
            }
            let context = structure
                .contexts
                .iter_mut()
                .find(|context| context.id == *id);
            let gate = context
                .as_ref()
                .map_or(crate::structure::FeatureContext::Limited, |context| {
                    context.gate
                });
            let evaluated =
                context.map_or_else(Vec::new, |context| std::mem::take(&mut context.roles));
            let (mut model, required) = self.compose_inspected(
                crate::EffectiveRequest::new(*id),
                operation_budget,
                gate,
                evaluated,
                false,
                true,
            )?;
            if let Some(required) = required {
                structure.required(self, &model, &required, operation_budget);
            }
            model.leave_reported_references(&effective, &mut operation_budget.work);
            let composition = if model.complete() {
                AssessmentStatus::Assessed
            } else {
                AssessmentStatus::Undetermined
            };
            structure.coverage.initial(3, composition);
            if !model.complete() {
                for member in [
                    crate::inventory::Requirement::M016,
                    crate::inventory::Requirement::M017,
                    crate::inventory::Requirement::M018,
                    crate::inventory::Requirement::M019,
                ] {
                    structure
                        .coverage
                        .observe(member, AssessmentStatus::Undetermined);
                }
            }
            effective.push(model);
        }
        let (dependencies, unavailable_dependencies) = dependency_scope(
            self,
            &scope,
            &effective,
            operation_budget,
            &mut effective_limit,
        );
        let modified_sources = scope.len() + dependencies.len();
        if profile.enabled.modified {
            for id in &dependencies {
                let mut gate = crate::structure::FeatureContext::Limited;
                'models: for model in &effective {
                    for (source, context) in &model.dependency_contexts {
                        if !crate::allowance::charge(&mut operation_budget.work, 1) {
                            break 'models;
                        }
                        if source == id {
                            gate = *context;
                            break 'models;
                        }
                    }
                }
                structure.dependency_modified(self, gate, operation_budget);
            }
            if effective_limit.is_some() || effective.iter().any(|model| model.dependency_unknown) {
                structure.modified.defer();
            }
        }
        structure.pattern.enabled = pattern_requested;
        structure.effective(self, &effective, operation_budget, request.assessment);
        if profile.enabled.consistency
            && (structure.work_limited
                || effective_limit.is_some()
                || effective.len() < scope.len()
                || effective.iter().any(|model| {
                    model.dependency_unknown || model.failure.is_some() || !model.clear_feature
                }))
        {
            for scope in &mut structure.consistency {
                scope.unknown = true;
            }
        }
        if profile.enabled.numeric
            && (structure.work_limited
                || effective_limit.is_some()
                || effective.len() < scope.len()
                || effective.iter().any(|model| model.dependency_unknown))
        {
            structure.numeric.unknown = true;
        }
        if pattern_requested
            && (structure.work_limited
                || effective_limit.is_some()
                || effective.len() < scope.len())
        {
            structure.pattern.unknown = true;
        }
        if structure.work_limited || effective_limit.is_some() || effective.len() < scope.len() {
            structure.advisory.unknown = true;
        }
        if effective_limit.is_some() || effective.len() < scope.len() {
            for area in [1, 3, 4, 5, 6] {
                structure
                    .coverage
                    .initial(area, AssessmentStatus::Undetermined);
            }
            if structure.coverage.full_requested {
                for state in &mut structure.coverage.requirements[1..] {
                    *state = AssessmentStatus::Undetermined;
                }
            }
        }
        if let Some(units) = &mut units {
            crate::units::scan(
                self,
                &effective,
                units,
                &mut registry,
                &mut structure,
                operation_budget,
                registry_scope_limited
                    || effective_limit.is_some()
                    || effective.len() < scope.len(),
            );
        }
        if let Some(content) = &mut content {
            crate::content_format::scan(
                self,
                &effective,
                content,
                &mut registry,
                &mut structure,
                operation_budget,
                registry_scope_limited
                    || effective_limit.is_some()
                    || effective.len() < scope.len(),
            );
        }
        if profile.enabled.full {
            use crate::inventory::Requirement;
            if owners_admitted {
                crate::recorded_extension::scan(
                    self,
                    &effective,
                    &mut registry,
                    &mut structure,
                    operation_budget,
                    registry_scope_limited
                        || effective_limit.is_some()
                        || effective.len() < scope.len(),
                );
            } else {
                structure
                    .coverage
                    .observe(Requirement::M028, AssessmentStatus::Undetermined);
            }
            structure.coverage.observe(
                Requirement::M012,
                structure.modified.status(modified_sources),
            );
            let unit_status = units
                .as_ref()
                .map_or(AssessmentStatus::Undetermined, |state| {
                    crate::inventory::join(state.status(0), state.status(1))
                });
            structure.coverage.observe(Requirement::M023, unit_status);
            let content_status = content
                .as_ref()
                .map_or(AssessmentStatus::Undetermined, |state| {
                    crate::inventory::join(state.status(0), state.status(1))
                });
            structure
                .coverage
                .observe(Requirement::M024, content_status);
            structure
                .coverage
                .observe(Requirement::M047, structure.pattern.status());
        }
        let errors = errors
            + structure.errors
            + effective
                .iter()
                .map(|model| model.reference_errors)
                .sum::<u32>();
        let warnings = structure.warnings;
        let notices = structure.notices;
        let mut report = Report {
            sample: None,
            catalogue: self,
            operation: Operation::Check(request.assessment),
            scope,
            selected,
            outcome: Outcome {
                complete: false,
                errors,
                warnings,
                notices,
                incomplete: 0,
                strict: request.strict,
            },
            structure,
            effective,
            effective_limit,
            dependencies,
            unavailable_dependencies,
            modified_sources,
            required: None,
            units,
            content,
            registry,
        };
        let coverage = report.requested_coverage();
        report.outcome.incomplete = if coverage.total.is_some() {
            coverage.unassessed + coverage.undetermined
        } else {
            1
        };
        report.outcome.complete = report.outcome.incomplete == 0;
        Ok(report)
    }
    pub fn inspect<'a>(&'a self, request: InspectRequest<'a>) -> Result<Report<'a>, &'static str> {
        if request.required {
            return self.inspect_required(request);
        }
        let source = self
            .source(request.source)
            .ok_or("unknown document selector")?;
        validate_pointer(request.pointer)?;
        if self.reserve + request.pointer.len() * 12 > self.limits.report_bytes {
            return Err("report cannot reserve its mandatory inspection selector");
        }
        let json = if request.view == View::Source {
            match source.json() {
                Field::Present(json) => select(json, request.pointer, self.limits.work)?,
                _ => None,
            }
        } else {
            None
        };
        let effective =
            if request.view == View::Effective {
                vec![self.effective(
                    crate::EffectiveRequest::new(request.source).pointer(request.pointer),
                )?]
            } else {
                Vec::new()
            };
        let complete = source.complete
            && if request.view == View::Effective {
                effective[0].complete()
            } else {
                json.is_some_and(JsonView::capability_complete)
            };
        let mut scope_budget = crate::allowance::OperationBudget::new(&self.limits);
        if let Some(model) = effective.first() {
            scope_budget.allocation = model.derived_remaining;
            scope_budget.work = model.work_remaining;
        }
        let mut scope_limit = None;
        let (dependencies, unavailable_dependencies) = dependency_scope(
            self,
            &[request.source],
            &effective,
            &mut scope_budget,
            &mut scope_limit,
        );
        Ok(Report {
            sample: None,
            catalogue: self,
            operation: Operation::Inspect {
                document: request.source,
                pointer: request.pointer,
                view: request.view,
                json,
            },
            scope: vec![request.source],
            selected: true,
            outcome: Outcome {
                complete: complete && scope_limit.is_none(),
                errors: source.parsed.errors as u32
                    + effective
                        .iter()
                        .map(|model| model.reference_errors)
                        .sum::<u32>(),
                warnings: 0,
                notices: 0,
                incomplete: u32::from(!complete || scope_limit.is_some()),
                strict: request.strict,
            },
            structure: crate::structure::Structure::default(),
            effective,
            effective_limit: scope_limit,
            dependencies,
            unavailable_dependencies,
            modified_sources: 0,
            required: None,
            units: None,
            content: None,
            registry: crate::registry::State::default(),
        })
    }
    fn inspect_required<'a>(
        &'a self,
        request: InspectRequest<'a>,
    ) -> Result<Report<'a>, &'static str> {
        let source = self
            .source(request.source)
            .ok_or("unknown document selector")?;
        if !source.role().is_model() {
            return Err("sample source cannot be a model root");
        }
        validate_pointer(request.pointer)?;
        let reserve = self
            .reserve
            .checked_add(
                request
                    .pointer
                    .len()
                    .checked_mul(12)
                    .ok_or("inspection selector reserve overflow")?,
            )
            .and_then(|n| n.checked_add(REQUIRED_REPORT_RESERVE))
            .ok_or("required inspection reserve overflow")?;
        if reserve > self.limits.report_bytes {
            return Err("report cannot reserve required declaration summary");
        }
        let mut budget = crate::allowance::OperationBudget::new(&self.limits);
        let mut effective = Vec::new();
        if request.view == View::Effective
            && crate::allowance::charge(
                &mut budget.allocation,
                std::mem::size_of::<crate::EffectiveModel<'_>>(),
            )
        {
            effective.reserve_exact(1);
        }
        let gate = source
            .json()
            .present()
            .map_or(crate::structure::FeatureContext::Limited, |root| {
                crate::structure::Structure::preflight(self, root, &mut budget)
            });
        let (json, required, model) = if request.view == View::Effective {
            let (model, required) = self.compose_inspected(
                crate::EffectiveRequest::new(request.source).pointer(request.pointer),
                &mut budget,
                gate,
                Vec::new(),
                true,
                true,
            )?;
            (None, required.expect("requested collection"), model)
        } else {
            self.source_required(request.source, request.pointer, &mut budget, gate)?
        };
        let primary_complete = source.complete
            && if request.view == View::Effective {
                model.complete()
            } else {
                json.is_some_and(JsonView::capability_complete)
            };
        let mut scope_limit = None;
        let (dependencies, unavailable_dependencies) = dependency_scope(
            self,
            &[request.source],
            std::slice::from_ref(&model),
            &mut budget,
            &mut scope_limit,
        );
        let complete = primary_complete && required.complete && scope_limit.is_none();
        let errors = u32::try_from(source.parsed.errors)
            .expect("bounded decoding findings")
            .checked_add(model.reference_errors)
            .and_then(|errors| errors.checked_add(required.errors))
            .expect("bounded inspection findings");
        let notices = required.notices;
        if request.view == View::Effective {
            if effective.capacity() > 0 {
                effective.push(model);
            } else {
                scope_limit = Some((request.source, crate::effective::Failure::scope_limit()));
            }
        }
        Ok(Report {
            catalogue: self,
            operation: Operation::Inspect {
                document: request.source,
                pointer: request.pointer,
                view: request.view,
                json,
            },
            scope: vec![request.source],
            selected: true,
            outcome: Outcome {
                complete: complete && scope_limit.is_none(),
                errors,
                warnings: 0,
                notices,
                incomplete: u32::from(!complete || scope_limit.is_some()),
                strict: request.strict,
            },
            structure: crate::structure::Structure::default(),
            effective,
            effective_limit: scope_limit,
            dependencies,
            unavailable_dependencies,
            modified_sources: 0,
            required: Some(required),
            units: None,
            content: None,
            registry: crate::registry::State::default(),
            sample: None,
        })
    }
}
fn dependency_scope(
    catalogue: &Catalogue,
    roots: &[SourceId],
    models: &[crate::EffectiveModel<'_>],
    budget: &mut crate::allowance::OperationBudget,
    limited: &mut Option<(SourceId, crate::effective::Failure)>,
) -> (Vec<SourceId>, Vec<SourceId>) {
    let mut dependencies = Vec::new();
    let mut unavailable = Vec::new();
    'sources: for source in catalogue.sources() {
        for model in models {
            for (candidates, output, uncertain) in [
                (&model.dependencies, &mut dependencies, false),
                (&model.unavailable_dependencies, &mut unavailable, true),
            ] {
                for candidate in candidates {
                    if !crate::allowance::charge(&mut budget.work, 1 + output.len() + roots.len()) {
                        *limited = Some((model.source, crate::effective::Failure::scope_limit()));
                        break 'sources;
                    }
                    if *candidate == source.id()
                        && !output.contains(candidate)
                        && (uncertain || !roots.contains(candidate))
                        && let Err(reason) =
                            crate::allowance::push(output, *candidate, &mut budget.allocation)
                    {
                        *limited = Some((model.source, reason.into()));
                        break 'sources;
                    }
                }
            }
        }
    }
    (dependencies, unavailable)
}
fn validate_roots(catalogue: &Catalogue, roots: &[SourceId]) -> Result<(), &'static str> {
    if roots.len() > catalogue.sources.len() {
        return Err("selected roots exceed source count");
    }
    for (index, id) in roots.iter().enumerate() {
        if catalogue.source(*id).is_none_or(|s| !s.role.is_model()) || roots[..index].contains(id) {
            return Err("unknown or duplicate selected root");
        }
    }
    Ok(())
}
pub(crate) fn select<'a>(
    json: JsonView<'a>,
    pointer: &str,
    mut work: usize,
) -> Result<Option<JsonView<'a>>, &'static str> {
    select_with_budget(json, pointer, &mut work)
}
pub(crate) fn select_with_budget<'a>(
    mut json: JsonView<'a>,
    pointer: &str,
    work: &mut usize,
) -> Result<Option<JsonView<'a>>, &'static str> {
    if pointer.is_empty() {
        return Ok(Some(json));
    }
    for token in pointer[1..].split('/') {
        let name = token.replace("~1", "/").replace("~0", "~");
        let field = match json.kind() {
            crate::JsonKind::Array => {
                if name.is_empty()
                    || !name.bytes().all(|b| b.is_ascii_digit())
                    || name.len() > 1 && name.starts_with('0')
                {
                    return Err("unknown inspection selector");
                }
                let index = name
                    .parse::<usize>()
                    .map_err(|_| "unknown inspection selector")?;
                let mut found = Field::Absent;
                for child in json.children() {
                    let Some(left) = work.checked_sub(1) else {
                        return Ok(None);
                    };
                    *work = left;
                    if child.n().index == Some(index) {
                        found = Field::Present(child);
                        break;
                    }
                }
                found
            }
            crate::JsonKind::Map => {
                let mut found = Field::Absent;
                for child in json.children() {
                    let Some(key) = child.n().key.as_deref() else {
                        return Ok(None);
                    };
                    let Some(left) = work.checked_sub(key.len() + name.len() + 1) else {
                        return Ok(None);
                    };
                    *work = left;
                    if key == name {
                        found = Field::Present(child);
                    }
                }
                found
            }
            _ => return Err("unknown inspection selector"),
        };
        json = match field {
            Field::Present(json) => json,
            Field::Absent => return Err("unknown inspection selector"),
            Field::Unavailable(_) => return Ok(None),
        };
    }
    Ok(Some(json))
}
impl Report<'_> {
    fn assessment_profile(&self) -> Option<&'static crate::assessment::Profile> {
        self.assessment().map(crate::assessment::profile)
    }
    pub fn unit_guidance(&self) -> Field<crate::UnitGuidance<'_>> {
        if !self
            .assessment_profile()
            .is_some_and(|profile| profile.enabled.units)
        {
            return Field::Absent;
        }
        self.units
            .as_ref()
            .map_or(Field::Unavailable("unit summary owner limit"), |state| {
                Field::Present(crate::UnitGuidance {
                    state,
                    registry: &self.registry,
                })
            })
    }
    pub fn registry_knowledge(&self, id: &str) -> Field<crate::RegistryKnowledgeView> {
        self.registry.view(id)
    }
    /// Specification facts consulted by this selected sample, independent of its outcome.
    pub fn format_knowledge(&self, id: &str) -> Field<crate::FormatKnowledgeView> {
        self.sample.as_ref().map_or(Field::Absent, |state| {
            crate::formats::knowledge(id, state.format_use)
        })
    }
    pub fn sample_result(&self) -> Option<crate::SampleResult<'_>> {
        self.sample.as_ref().map(|state| crate::SampleResult {
            state,
            catalogue: self.catalogue,
            model: self.effective.first(),
        })
    }
    pub fn assessment_area_status(&self, id: &str) -> Field<AssessmentStatus> {
        match self.operation {
            Operation::Sample => self.sample_result().expect("sample state").area_status(id),
            Operation::Inspect { .. } => Field::Absent,
            Operation::Check(profile) => crate::assessment::profile(profile)
                .area(id)
                .map_or(Field::Absent, |area| Field::Present(self.area_status(area))),
        }
    }
    fn area_status(&self, area: &crate::assessment::Area) -> AssessmentStatus {
        match area.status {
            StatusSource::Initial(index) => self.structure.coverage.initial[index],
            StatusSource::Advisory => self.structure.advisory.status(),
            StatusSource::Modified => self.structure.modified.status(self.modified_sources),
            StatusSource::Units(index) => self
                .units
                .as_ref()
                .map_or(AssessmentStatus::Undetermined, |state| state.status(index)),
            StatusSource::ContentFormat(index) => self
                .content
                .as_ref()
                .map_or(AssessmentStatus::Undetermined, |state| state.status(index)),
            StatusSource::Pattern => self.structure.pattern.status(),
            StatusSource::Numeric => self.structure.numeric.status(),
            StatusSource::Consistency(index) => self.structure.consistency[index].status(),
            StatusSource::Requirements => self
                .structure
                .coverage
                .area_status(area.id, self.structure.advisory.status())
                .expect("fixed Full area"),
        }
    }
    fn requirement_counts(&self) -> RequestedCoverage {
        let mut coverage = RequestedCoverage {
            inventory: Some(crate::inventory::INVENTORY),
            count_unit: "requirement",
            total: Some(54),
            assessed: 0,
            not_applicable: 0,
            unassessed: 0,
            undetermined: 0,
        };
        for state in self.structure.coverage.requirements {
            coverage.accumulate(state);
        }
        coverage
    }
    fn broader_state(&self) -> &'static str {
        if self
            .assessment_profile()
            .is_some_and(|profile| profile.enabled.full)
        {
            let coverage = self.requirement_counts();
            if coverage.unassessed == 0 && coverage.undetermined == 0 {
                return "complete";
            }
        }
        "partial"
    }
    pub fn requested_coverage(&self) -> RequestedCoverage {
        let mut coverage = RequestedCoverage {
            inventory: None,
            count_unit: "assessment_area",
            total: None,
            assessed: 0,
            not_applicable: 0,
            unassessed: 0,
            undetermined: 0,
        };
        match self.operation {
            Operation::Sample => {
                coverage.inventory = Some("esdieff.sample/1");
                coverage.count_unit = "sample_area";
                coverage.total = Some(9);
                for area in self.sample.as_ref().expect("sample state").areas {
                    coverage.accumulate(area);
                }
            }
            Operation::Check(assessment) => {
                let profile = crate::assessment::profile(assessment);
                coverage.inventory = Some(profile.inventory);
                coverage.total = Some(profile.area_count() as u32);
                for area in profile.areas() {
                    coverage.accumulate(self.area_status(area));
                }
            }
            Operation::Inspect { .. } => {
                coverage.count_unit = "inspection";
                coverage.total = Some(1);
                coverage.assessed = u32::from(self.outcome.complete);
                coverage.undetermined = u32::from(!self.outcome.complete);
            }
        }
        coverage
    }
    /// The requested fine-grained inventory, independently of optional rendering.
    pub fn requirement_inventory(&self) -> Field<&'static str> {
        if self
            .assessment_profile()
            .is_some_and(|profile| profile.enabled.full)
        {
            Field::Present(crate::inventory::INVENTORY)
        } else {
            Field::Absent
        }
    }
    /// Authoritative requirement counts, whose count unit is `requirement`.
    pub fn requirement_coverage(&self) -> Field<RequestedCoverage> {
        if self
            .assessment_profile()
            .is_some_and(|profile| profile.enabled.full)
        {
            Field::Present(self.requirement_counts())
        } else {
            Field::Absent
        }
    }
    pub fn requirement_status(&self, id: &str) -> Field<AssessmentStatus> {
        if !self
            .assessment_profile()
            .is_some_and(|profile| profile.enabled.full)
        {
            return Field::Absent;
        }
        crate::inventory::DESCRIPTORS
            .iter()
            .position(|descriptor| descriptor.id == id)
            .map_or(Field::Absent, |index| {
                Field::Present(self.structure.coverage.requirements[index])
            })
    }
    pub fn requirement_descriptors(
        &self,
    ) -> Field<impl ExactSizeIterator<Item = &'static crate::RequirementDescriptor>> {
        if self
            .assessment_profile()
            .is_some_and(|profile| profile.enabled.full)
        {
            Field::Present(crate::inventory::DESCRIPTORS.iter())
        } else {
            Field::Absent
        }
    }
    pub fn assessment(&self) -> Option<Assessment> {
        match self.operation {
            Operation::Check(profile) => Some(profile),
            _ => None,
        }
    }
    pub fn diagnostics(&self) -> Diagnostics {
        let (mut metadata, _, _) = self.metadata(false);
        let omitted = metadata["diagnostics"]["omitted"]
            .as_u64()
            .expect("omitted count") as u32;
        let truncated = metadata["diagnostics"]["truncated"]
            .as_bool()
            .expect("truncation flag");
        let Value::Array(conditions) = metadata["diagnostics"]["items"].take() else {
            unreachable!("condition array")
        };
        Diagnostics {
            conditions: conditions.into_iter().map(Condition).collect(),
            omitted,
            truncated,
        }
    }
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    pub fn requested_source_ids(&self) -> &[SourceId] {
        &self.scope
    }
    pub fn inspection_json(&self) -> Option<JsonView<'_>> {
        match self.operation {
            Operation::Inspect { json, .. } => json,
            Operation::Check(_) | Operation::Sample => None,
        }
    }
    pub fn effective_model(&self) -> Option<&crate::EffectiveModel<'_>> {
        self.effective.first()
    }
    pub fn required_declarations(&self) -> Field<crate::RequiredDeclarations<'_>> {
        self.required.as_ref().map_or(Field::Absent, |collection| {
            Field::Present(crate::RequiredDeclarations {
                collection,
                model: self.effective.first(),
            })
        })
    }
    pub fn to_json(&self) -> String {
        self.encode(false)
    }
    pub fn to_human(&self) -> String {
        self.to_human_with_style(HumanStyle::Plain)
    }
    /// Render a check report as a SARIF 2.1.0 log within the report limit, following ADR 0043:
    /// SARIF and Markdown report projections. Artifact URIs of file sources are their paths
    /// relative to `directory`, the directory relative paths were resolved from.
    pub fn to_sarif(&self, directory: &std::path::Path) -> String {
        crate::sarif::check(self, self.catalogue, directory)
    }
    /// Render with an explicit presentation choice. Styling falls back to the
    /// identical plain projection when its ANSI spans exceed the report budget.
    pub fn to_human_with_style(&self, style: HumanStyle) -> String {
        self.render_human_with_style(self.catalogue.limits.report_bytes, style)
    }
    #[cfg(test)]
    fn render_human(&self, fallback_at: usize) -> String {
        self.render_human_with_style(fallback_at, HumanStyle::Plain)
    }
    fn render_human_with_style(&self, fallback_at: usize, style: HumanStyle) -> String {
        let (value, result, required) = self.metadata(true);
        let mut output = HumanText::new(style);
        output.heading("Esdieff:");
        output.push(' ');
        output.span(
            Role::disposition(self.outcome.disposition()),
            self.outcome.disposition().as_str(),
        );
        output.push('\n');
        output.heading("Complete:");
        output.push(' ');
        output.span(
            (!self.outcome.complete).then_some(Role::Incomplete),
            if self.outcome.complete {
                "true"
            } else {
                "false"
            },
        );
        output.push('\n');
        for (index, (label, count)) in [
            ("Errors:", self.outcome.errors),
            ("warnings:", self.outcome.warnings),
            ("notices:", self.outcome.notices),
            ("incomplete checks:", self.outcome.incomplete),
        ]
        .into_iter()
        .enumerate()
        {
            if index != 0 {
                output.push_str("; ");
            }
            output.heading(label);
            output.push_str(&format!(" {count}"));
        }
        output.push('\n');
        output.heading("Scope:");
        output.push_str(if self.selected {
            " selected\n"
        } else {
            " catalogue\n"
        });
        // Broader coverage is a scope disclosure, not a finding, so its state stays plain.
        output.heading("Broader RFC coverage:");
        output.push(' ');
        output.push_str(self.broader_state());
        output.push('\n');
        self.write_required_human(&mut output, &required);
        if self
            .assessment_profile()
            .is_some_and(|profile| profile.registry_requested())
        {
            use std::fmt::Write;
            output.heading("Recorded registry policy:");
            writeln!(output, " {}", self.registry.policy.as_str()).expect("String write");
            if let Some(state) = &self.units {
                let partial = state.spelling_unknown || state.guidance_unknown;
                output.heading("Unit guidance:");
                output.push(' ');
                output.span(
                    partial.then_some(Role::Incomplete),
                    if partial { "partial" } else { "complete" },
                );
                output.push('\n');
                for (area, id) in crate::units::AREAS.iter().enumerate() {
                    output.heading("Assessment area");
                    write!(output, " {id}: ").expect("String write");
                    output.assessment(state.status(area));
                    output.push('\n');
                }
                use crate::units::Counter;
                output.heading("Unit counts:");
                writeln!(output," known_primary {}; known_secondary {}; unknown {}; unavailable {}; legacy {}; uri_names {}",state.count(Counter::KnownPrimary),state.count(Counter::KnownSecondary),state.count(Counter::Unknown),state.count(Counter::Unavailable),state.count(Counter::Legacy),state.count(Counter::UriNames)).expect("String write");
                for reason in state.unavailable_reasons() {
                    output.heading("Unit query unavailable:");
                    writeln!(output, " {reason}").expect("String write");
                }
            } else if self
                .assessment_profile()
                .is_some_and(|profile| profile.enabled.units)
            {
                output.heading("Unit guidance:");
                output.push(' ');
                output.span(Role::Incomplete, "unavailable");
                output.push_str(" (owner_limit)\n");
                output.heading("Unit counts:");
                output.push(' ');
                output.span(Role::Incomplete, "unavailable");
                output.push('\n');
                for id in crate::units::AREAS {
                    output.heading("Assessment area");
                    write!(output, " {id}: ").expect("String write");
                    output.assessment(AssessmentStatus::Undetermined);
                    output.push('\n');
                }
            }
            if self
                .assessment_profile()
                .is_some_and(|profile| profile.enabled.content_format)
            {
                for (area, id) in crate::content_format::AREAS.iter().enumerate() {
                    let status = self
                        .content
                        .as_ref()
                        .map_or(AssessmentStatus::Undetermined, |state| state.status(area));
                    output.heading("Assessment area");
                    write!(output, " {id}: ").expect("String write");
                    output.assessment(status);
                    output.push('\n');
                }
            }
            self.registry.write_human(&mut output);
        }
        for owner in &required.owners {
            use std::fmt::Write;
            output.heading("Required owner:");
            writeln!(output, " {owner}").expect("String write");
        }
        output.heading("Requested assessment:");
        output.push_str(&format!(
            " {}\n",
            match self.operation {
                Operation::Sample => "esdieff.sample/1",
                Operation::Check(profile) => profile.identity(),
                Operation::Inspect { .. } => "none (one inspection operation)",
            }
        ));
        output.heading("Baseline:");
        output.push_str(" rfc9880/base-1 (RFC 9880)\n");
        output.heading("Acceptance restrictions:");
        output.push_str(if matches!(self.operation, Operation::Check(_)) {
            " A001, A002; "
        } else {
            " A002; "
        });
        output.heading("strict warnings:");
        output.push_str(&format!(" {}\n", self.outcome.strict));
        output.heading("Requested sources:");
        output.push_str(&format!(
            " {:?}\n",
            self.scope.iter().map(|id| id.get()).collect::<Vec<_>>()
        ));
        if let Operation::Check(profile) = self.operation {
            let coverage = self.requested_coverage();
            output.heading("Requested inventory:");
            output.push_str(&format!(
                " {}\n",
                coverage.inventory.expect("known inventory")
            ));
            output.heading("Requested coverage (assessment_area):");
            output.push_str(&format!(
                " total {}; assessed {}; not applicable {}; unassessed {}; undetermined {}\n",
                coverage.total.expect("known inventory"),
                coverage.assessed,
                coverage.not_applicable,
                coverage.unassessed,
                coverage.undetermined
            ));
            if crate::assessment::profile(profile).enabled.full {
                let requirements = self.requirement_counts();
                output.heading("Requirement inventory:");
                output.push_str(&format!(" {}\n", crate::inventory::INVENTORY));
                output.heading("Requirement coverage:");
                output.push_str(&format!(
                    " total 54; assessed {}; not applicable {}; unassessed {}; undetermined {}\n",
                    requirements.assessed,
                    requirements.not_applicable,
                    requirements.unassessed,
                    requirements.undetermined
                ));
            }
        }
        if let Some(state) = &self.sample {
            output.heading("Selected");
            output.push_str(&format!(
                " {} in model {} at {}\n",
                state.kind.as_str(),
                state.model.get(),
                serde_json::to_string(&state.pointer).expect("selector encoding"),
            ));
            output.heading("Sample");
            output.push_str(&format!(" {}: ", state.sample.get()));
            output.span(
                match state.admission {
                    crate::Admission::Admitted => Role::Passed,
                    crate::Admission::Rejected => Role::Failed,
                    crate::Admission::Undetermined => Role::Incomplete,
                },
                state.admission.as_str(),
            );
            output.push_str("; context ");
            output.span(
                (state.context != crate::ContextState::Available).then_some(Role::Incomplete),
                state.context.as_str(),
            );
            output.push('\n');
            let coverage = self.requested_coverage();
            output.heading("Requested coverage (sample_area):");
            output.push_str(&format!(
                " total 9; assessed {}; not applicable {}; unassessed {}; undetermined {}\n",
                coverage.assessed,
                coverage.not_applicable,
                coverage.unassessed,
                coverage.undetermined
            ));
            for (area, status) in crate::sample::AREAS.into_iter().zip(state.areas) {
                output.heading("Sample area");
                output.push_str(&format!(" {area}: "));
                output.assessment(status);
                output.push('\n');
            }
            let sample = self.sample_result().expect("sample state");
            let meaning = sample.meaning();
            for (name, area) in [
                ("abstract values", meaning.abstract_values()),
                ("const", meaning.const_values()),
            ] {
                output.heading(&format!("Sample {name} meaning:"));
                output.push(' ');
                output.assessment(area.status());
                output.push('\n');
                human_sample_basis(&mut output, &format!("Sample {name} basis"), area.basis());
                human_sample_basis(
                    &mut output,
                    &format!("Sample {name} limitations"),
                    area.limitations(),
                );
            }
            human_sample_basis(
                &mut output,
                "Sample admission basis",
                meaning.admission().basis(),
            );
            human_sample_basis(
                &mut output,
                "Sample admission limitations",
                meaning.admission().limitations(),
            );
            output.heading("Consulted sample interpretations:");
            output.push(' ');
            let consulted = sample.consulted_interpretations();
            if consulted.len() == 0 {
                output.push_str("none");
            }
            for (index, id) in consulted.enumerate() {
                if index != 0 {
                    output.push_str("; ");
                }
                output.push_str(id);
            }
            output.push('\n');
            human_location(&result["sample_location"], &mut output);
            output.push('\n');
        }
        if let Some(profile) = self.assessment_profile() {
            for area in profile
                .areas()
                .filter(|area| matches!(area.status, StatusSource::Modified))
            {
                output.heading("Assessment area");
                output.push_str(&format!(" {}: ", area.id));
                output.assessment(self.area_status(area));
                output.push('\n');
            }
        }
        if value["baseline"]["extensions"].is_array() {
            output.heading("Draft support:");
            output.push_str(&format!(
                " {} (Context checking, composition and inspection)\n",
                crate::context::DRAFT
            ));
            output.push_str("Context limits: absent nullable defaults unresolved, sdfRequired meaning undefined, reuse outside Context unavailable and general link meaning unavailable\n");
        }
        if value["baseline"]["temporal_knowledge"].is_object() {
            output.heading("Temporal knowledge:");
            output.push_str(&format!(
                " {} (1972-01-01 through 2026-12-31 UTC)\n",
                crate::scalar::KNOWLEDGE_ID
            ));
            output.heading("Temporal knowledge SHA256:");
            output.push_str(&format!(
                " transitions {}; bulletin {}\n",
                crate::scalar::TRANSITION_SHA256,
                crate::scalar::BULLETIN_SHA256
            ));
        }
        if let Some(records) = value["baseline"]["format_knowledge"].as_array() {
            for record in records {
                use std::fmt::Write;
                output.heading("Format knowledge:");
                writeln!(
                    output,
                    " {} (RFC {} Sections {}): {}",
                    record["id"].as_str().expect("format ID"),
                    record["rfc"].as_str().expect("format RFC"),
                    record["sections"].as_str().expect("format sections"),
                    record["scope"].as_str().expect("format scope")
                )
                .expect("String write");
            }
        } else if self.sample.is_some() {
            output.heading("Format knowledge:");
            output.push(' ');
            output.span(Role::Incomplete, "unavailable");
            output.push_str(" (initial owner)\n");
        }
        for interpretation in value["baseline"]["interpretations"]
            .as_array()
            .expect("interpretations")
        {
            use std::fmt::Write;
            output.heading("Interpretation:");
            writeln!(
                output,
                " {}",
                interpretation.as_str().expect("interpretation ID")
            )
            .expect("String write");
        }
        if let Some(profile) = self.assessment_profile() {
            for area in profile.areas().filter(|area| {
                matches!(
                    area.status,
                    StatusSource::Pattern | StatusSource::Numeric | StatusSource::Consistency(_)
                )
            }) {
                output.heading("Assessment area");
                output.push_str(&format!(" {}: ", area.id));
                output.assessment(self.area_status(area));
                output.push('\n');
            }
        }
        for source in &self.catalogue.sources {
            output.heading("Source");
            output.push_str(&format!(
                " {} [{}]: {} / {} ({} retained bytes, content complete: {})\n",
                source.id.get(),
                serde_json::to_string(&source.key).expect("source key encoding"),
                source.acquisition.as_str(),
                source_status(source),
                source.bytes.len(),
                source.complete
            ));
        }
        if self.selected && !self.dependencies.is_empty() {
            output.heading("Dependency sources:");
            output.push_str(&format!(
                " {:?}\n",
                self.dependencies
                    .iter()
                    .map(|id| id.get())
                    .collect::<Vec<_>>()
            ));
        }
        for condition in value["diagnostics"]["items"]
            .as_array()
            .expect("condition array")
        {
            output.span(
                Role::severity(Severity::from_report(
                    condition["severity"].as_str().expect("severity"),
                )),
                condition["code"].as_str().expect("code"),
            );
            output.push_str(&format!(
                ": {}\n",
                condition["message"].as_str().expect("message")
            ));
            if let Some(quality) = condition["arguments"]["quality"].as_str() {
                output.push_str(&format!(
                    "  Quality: {}\n",
                    serde_json::to_string(quality).expect("quality name encoding")
                ));
            }
            if let Some(location) = condition.get("primary") {
                human_location(location, &mut output);
                output.push('\n');
            }
            if let Some(replacement) = condition["arguments"]["replacement"].as_str() {
                output.push_str("  ");
                output.heading("Direct symbol if intended:");
                output.push_str(&format!(
                    " {}\n",
                    serde_json::to_string(replacement).expect("replacement encoding")
                ));
            }
            for location in condition["related"].as_array().expect("related locations") {
                output.push_str("  related ");
                human_location(location, &mut output);
                output.push('\n');
            }
        }
        output.heading("Diagnostic details omitted:");
        output.push_str(&format!(" {}\n", value["diagnostics"]["omitted"]));
        let reasons = value["diagnostics"]["reasons"]
            .as_array()
            .expect("limit reasons");
        if !reasons.is_empty() {
            output.heading("Presentation limits:");
            for reason in reasons {
                output.push(' ');
                output.push_str(reason.as_str().expect("limit reason"));
            }
            output.push('\n');
        }
        for omission in value["diagnostics"]["omissions"]
            .as_array()
            .expect("omission array")
        {
            output.heading("Presentation omitted");
            output.push_str(&format!(
                " {}: {}\n",
                omission["field"].as_str().expect("omitted field"),
                omission["reason"].as_str().expect("omission reason")
            ));
        }
        if let Operation::Inspect {
            json: Some(payload),
            view,
            pointer,
            ..
        } = self.operation
        {
            if output.len() + payload.raw_json().len() + pointer.len() * 6 + 128
                <= self.catalogue.limits.report_bytes
            {
                output.heading("Inspection");
                output.push_str(&format!(
                    " {} at {}:\n",
                    view.as_str(),
                    serde_json::to_string(pointer).expect("pointer encoding")
                ));
                output.push_str(payload.raw_json());
                output.push('\n');
            } else {
                output.heading("Inspection content omitted:");
                output.push_str(" report_limit\n");
            }
        }
        if let Operation::Inspect {
            json: None,
            view: View::Effective,
            pointer,
            ..
        } = self.operation
            && let Some(payload) = self
                .effective
                .first()
                .and_then(|model| model.generated.as_deref())
        {
            if output.len() + payload.len() + pointer.len() * 6 + 128
                <= self.catalogue.limits.report_bytes
            {
                output.heading("Inspection");
                output.push_str(&format!(
                    " effective at {}:\n",
                    serde_json::to_string(pointer).expect("pointer encoding")
                ));
                output.push_str(payload);
                output.push('\n');
            } else {
                output.heading("Inspection content omitted:");
                output.push_str(" report_limit\n");
            }
        }
        if let Some(owners) = result["alternatives"].as_array() {
            for owner in owners {
                output.heading("Alternatives");
                output.push(' ');
                let state = owner["state"].as_str().expect("collection state");
                output.span((state == "unavailable").then_some(Role::Incomplete), state);
                output.push_str(" at ");
                human_location(&owner["owner"], &mut output);
                output.push('\n');
                if let Some(reason) = owner["unavailable_reason"].as_str() {
                    output.push_str(reason);
                    output.push('\n');
                }
                for branch in owner["branches"].as_array().expect("alternative branches") {
                    use std::fmt::Write;
                    let id = &branch["id"];
                    if id["kind"] == "enum" {
                        output.heading("Alternative enum");
                        write!(output, " {}: ", id["index"]).expect("String write");
                    } else {
                        output.heading("Alternative choice");
                        write!(output, " {}: ", id["name"]).expect("String write");
                    }
                    let state = branch["state"].as_str().expect("branch state");
                    output.span((state == "unavailable").then_some(Role::Incomplete), state);
                    output.push('\n');
                    if let Some(raw) = branch["raw_json"].as_str() {
                        output.push_str(raw);
                        output.push('\n');
                    }
                    if let Some(reason) = branch["unavailable_reason"].as_str() {
                        output.push_str(reason);
                        output.push('\n');
                    }
                    for quality in branch["unavailable_qualities"]
                        .as_array()
                        .expect("unavailable qualities")
                    {
                        output.heading("Unavailable quality");
                        writeln!(
                            output,
                            " {}: {}",
                            quality["name"].as_str().expect("quality name"),
                            quality["reason"].as_str().expect("quality reason")
                        )
                        .expect("String write");
                    }
                    for explanation in branch["explanations"]
                        .as_array()
                        .expect("branch explanations")
                    {
                        human_explanation(explanation, &mut output);
                    }
                }
            }
        }
        if let Some(explanations) = result["explanations"].as_array() {
            for explanation in explanations {
                human_explanation(explanation, &mut output);
            }
            for omission in result["omissions"]
                .as_array()
                .expect("inspection omissions")
            {
                output.heading("Inspection detail omitted");
                output.push_str(&format!(
                    " {}: {}\n",
                    omission["field"].as_str().expect("omitted field"),
                    omission["reason"].as_str().expect("omission reason")
                ));
            }
        }
        // Human output has the same bounded metadata and retained details, without embedding source content.
        if output.len() > fallback_at {
            drop(value);
            drop(output);
            drop(required);
            if self
                .assessment_profile()
                .is_some_and(|profile| profile.enabled.numeric)
            {
                drop(result);
            }
            return self.encode(true);
        }
        output.finish(fallback_at)
    }
    fn metadata(&self, human: bool) -> (Value, Value, required::Presentation) {
        let (base, result, required, _) = self.metadata_with_budget(human);
        (base, result, required)
    }
    pub(crate) fn author_projection(&self) -> (Value, Projection) {
        let (base, _, _, budget) = self.metadata_with_budget(false);
        (base, budget)
    }
    fn metadata_with_budget(
        &self,
        human: bool,
    ) -> (Value, Value, required::Presentation, Projection) {
        let _mandatory_remaining = self.sample.as_ref().map(crate::sample::mandatory_dto);
        let cat = self.catalogue;
        let limits = &cat.limits;
        let reserve = cat.reserve
            + match self.operation {
                Operation::Inspect { pointer, .. } => pointer.len() * 12,
                Operation::Check(profile) => crate::assessment::profile(profile).report_reserve(),
                _ => 0,
            }
            + if self.required.is_some() {
                REQUIRED_REPORT_RESERVE
            } else {
                0
            };
        let mut projection = Projection::new(limits, reserve);
        let (budget, pointer_work) = projection.parts();
        let initial_budget = budget.remaining();
        let mut retained = Vec::new();
        let mut total = 0usize;
        let mut omissions = Vec::new();
        let mut required = required::Presentation::default();
        for source in &cat.sources {
            if !self.scope.contains(&source.id)
                && !self.sample.as_ref().is_some_and(|state| {
                    state.sample == source.id || self.dependencies.contains(&source.id)
                })
            {
                continue;
            }
            if self.sample.is_some()
                && source.role.is_model()
                && !source.parsed.rejected
                && source.parsed.root.is_some()
                && source.complete
            {
                continue;
            }
            total += source.parsed.issue_count;
            budget.observe_omitted(source.parsed.omitted_details.count, false);
            // Derived diagnostic storage exhaustion uses the contract's byte-limit reason.
            budget.observe_omitted(
                false,
                source.parsed.omitted_details.bytes || source.parsed.omitted_details.allocation,
            );
            for issue in source
                .parsed
                .issues
                .iter()
                .chain(source.parsed.allocation_issue.iter())
            {
                retain(&mut retained, budget, limits.conditions, || {
                    let basis = WireBasis::decoding(issue.code).to_json();
                    tag_condition(
                        json!({"code":issue.code,"severity":if issue.error { "error" } else { "incomplete" },"basis":basis,"message":issue.reason,
                    "primary":{"kind":if self.sample.is_some() && source.role == crate::SourceRole::Sample { "sample" } else { "source" },"source_id":source.id.get(),"pointer":null,"span":if source.original { issue.span.as_ref().map(|s|[s.start,s.end]) } else { None }},"related":[],"arguments":{}}),
                        self.sample.as_ref().map(|_| {
                            if issue.code == "operation.limit" {
                                "operation"
                            } else {
                                source.role.as_str()
                            }
                        }),
                    )
                });
            }
            if source.acquisition == Acquisition::Failed || !source.complete {
                total += 1;
                retain(&mut retained, budget, limits.conditions, || {
                    tag_condition(
                        json!({"code":if source.acquisition==Acquisition::Failed { "input.acquisition" } else { "operation.limit" },"severity":"incomplete","basis":{"kind":"operational","id":"source-acquisition"},"message":if source.acquisition==Acquisition::Failed { "Source content could not be acquired" } else { "Retained source content is incomplete" },"primary":{"kind":if self.sample.is_some() && source.role == crate::SourceRole::Sample { "sample" } else { "source" },"source_id":source.id.get(),"pointer":null,"span":null},"related":[],"arguments":{}}),
                        self.sample.as_ref().map(|_| {
                            if source.acquisition == Acquisition::Failed {
                                source.role.as_str()
                            } else {
                                "operation"
                            }
                        }),
                    )
                });
            }
        }
        total += self.structure.total;
        budget.observe_omitted(self.structure.count_omitted, self.structure.bytes_omitted);
        for finding in &self.structure.findings {
            retain_structure(
                &mut retained,
                budget,
                limits.conditions,
                finding,
                &self.effective,
                PointerPresentation {
                    cap: limits.pointer_bytes,
                    work: pointer_work,
                    omissions: &mut omissions,
                },
                Attribution {
                    domain: self.sample.as_ref().map(|_| {
                        if finding.rule.code() == "operation.limit" {
                            "operation"
                        } else {
                            "model"
                        }
                    }),
                    owner: self
                        .assessment_profile()
                        .is_some_and(|profile| profile.enabled.full)
                        .then(|| {
                            if finding.rule.assessment_unit() == Some(crate::proof::AREA) {
                                Some(crate::proof::AREA)
                            } else {
                                finding.coverage_requirement.map(|requirement| {
                                    crate::inventory::DESCRIPTORS[requirement as usize].area
                                })
                            }
                        })
                        .flatten(),
                },
            );
        }
        if let Some((document, reason)) = self.effective_limit {
            total += 1;
            retain(&mut retained, budget, limits.conditions, || {
                tag_condition(
                    json!({"code":reason.code(),"severity":"incomplete","basis":{"kind":"operational","id":"effective-composition"},"message":reason.reason,"primary":{"kind":"effective","root":document.get(),"pointer":""},"related":[],"arguments":{"document":document.get()}}),
                    self.sample.as_ref().map(|_| "operation"),
                )
            });
        }
        for model in &self.effective {
            if let Some(reason) = model.failure {
                total += 1;
                retain(&mut retained, budget, limits.conditions, || {
                    tag_condition(
                        json!({"code":reason.code(),"severity":"incomplete","basis":{"kind":"operational","id":"effective-composition"},"message":reason.reason,"related":[],"arguments":{"document":model.source.get()}}),
                        self.sample.as_ref().map(|_| {
                            if reason.code() == "operation.limit" {
                                "operation"
                            } else {
                                "model"
                            }
                        }),
                    )
                });
            }
            for (json, reason) in model.conditions() {
                total += 1;
                retain_effective_condition(
                    &mut retained,
                    budget,
                    limits.conditions,
                    json,
                    reason,
                    PointerPresentation {
                        cap: limits.pointer_bytes,
                        work: pointer_work,
                        omissions: &mut omissions,
                    },
                    self.sample.as_ref().map(|_| {
                        if reason.code() == "operation.limit" {
                            "operation"
                        } else {
                            "model"
                        }
                    }),
                );
            }
        }
        let (assessment, inventory, requested, selector, mut result) = match self.operation {
            Operation::Sample => {
                let state = self.sample.as_ref().expect("sample state");
                let model = self.effective.first();
                let sample_location =
                    sample_location(cat.source(state.sample).expect("sample source"));
                let selector = json!({"kind":state.kind.as_str(),"document":state.model.get(),"pointer":state.pointer});
                let mut witnesses = Vec::new();
                let mut explanations = Vec::new();
                total += state.total_findings;
                budget.observe_omitted(false, state.storage_omitted);
                if let Some(model) = model {
                    for finding in &state.findings {
                        retain_sample_finding(
                            &mut retained,
                            &mut witnesses,
                            budget,
                            limits.conditions,
                            SampleEvidence {
                                finding,
                                model,
                                sample: cat.source(state.sample).expect("sample source"),
                            },
                            PointerPresentation {
                                cap: limits.pointer_bytes,
                                work: pointer_work,
                                omissions: &mut omissions,
                            },
                        );
                    }
                    for key in [
                        "type",
                        "nullable",
                        "minimum",
                        "maximum",
                        "exclusiveMinimum",
                        "exclusiveMaximum",
                        "multipleOf",
                        "minLength",
                        "maxLength",
                        "pattern",
                        "format",
                        "const",
                    ] {
                        let owner = match state.witness_node() {
                            Field::Present(node) => Some(crate::EffectiveJson { model, node }),
                            Field::Absent => model.json().present(),
                            Field::Unavailable(_) => None,
                        };
                        if let Some(json) = owner.and_then(|json| {
                            for child in json.children() {
                                if pointer_work
                                    .debit(
                                        2 * (key.len()
                                            + child.member_name().present().map_or(0, str::len)
                                            + 1),
                                    )
                                    .is_err()
                                {
                                    return None;
                                }
                            }
                            json.get(key).present()
                        }) {
                            retain_explanation(
                                &mut explanations,
                                budget,
                                limits.conditions,
                                crate::effective::Explanation::selected(json),
                                false,
                                PointerPresentation {
                                    cap: limits.pointer_bytes,
                                    work: pointer_work,
                                    omissions: &mut omissions,
                                },
                            );
                        }
                    }
                    if state.areas[1] == AssessmentStatus::Assessed
                        && cat
                            .source(state.sample)
                            .and_then(|source| source.sample_value().present())
                            .is_some_and(|value| value.kind() == crate::JsonKind::Null)
                        && let Some(node) = match state.witness_node() {
                            Field::Present(node) => Some(node),
                            Field::Absent => model.selected,
                            Field::Unavailable(_) => None,
                        }
                        && let Field::Present(default) =
                            model.default_at(node, "nullable", pointer_work.counter())
                    {
                        retain_explanation(
                            &mut explanations,
                            budget,
                            limits.conditions,
                            default,
                            false,
                            PointerPresentation {
                                cap: limits.pointer_bytes,
                                work: pointer_work,
                                omissions: &mut omissions,
                            },
                        );
                    }
                }
                for (area, status) in crate::sample::AREAS.into_iter().zip(state.areas) {
                    if matches!(
                        status,
                        AssessmentStatus::Unassessed | AssessmentStatus::Undetermined
                    ) && !(area == crate::sample::AREAS[8] && state.abstract_unknown == 2)
                    {
                        total += 1;
                        retain(&mut retained, budget, limits.conditions, || {
                            json!({
                            "domain":"operation","code":if state.abstract_unavailable(area) { "sample.abstract_denotation_unavailable" } else { "sample.coverage_unavailable" },"severity":"incomplete",
                            "basis":sample_coverage_basis(area, status, state),
                            "message":"Requested sample area is unfinished","assessment_unit":area,
                            "related":[],"arguments":{}})
                        });
                    }
                }
                let description = model.and_then(|model| model.json().present()).map(|_| {
                    let shown = state.description_pointer.len() <= limits.pointer_bytes && state.description_pointer.len() <= limits.pointer_total;
                    if !shown { omissions.push(json!({"field":"result.description.pointer","reason":"location_limit","omitted_count":1})); }
                    json!({"kind":"effective","root":state.model.get(),"pointer":if shown { Some(state.description_pointer.as_str()) } else { None }})
                });
                if state.admission == crate::Admission::Admitted
                    && let Some(description) = &description
                    && !retain_admission_witness(
                        &mut witnesses,
                        budget,
                        limits.conditions,
                        description,
                        &sample_location,
                        state.witness_node().present().and_then(|node| {
                            model.map(|model| crate::EffectiveJson { model, node })
                        }),
                        PointerPresentation {
                            cap: limits.pointer_bytes,
                            work: pointer_work,
                            omissions: &mut omissions,
                        },
                    )
                {
                    omissions.push(json!({"field":"result.witnesses","reason":if witnesses.len() >= limits.conditions { "count_limit" } else { "byte_limit" },"omitted_count":1}));
                }
                if witnesses.len() < state.total_findings || state.storage_omitted {
                    omissions.push(json!({"field":"result.witnesses","reason":"byte_limit","omitted_count":state.total_findings.saturating_sub(witnesses.len())}));
                }
                let coverage = self.requested_coverage();
                (
                    Some("esdieff.sample/1"),
                    Some("esdieff.sample/1"),
                    json!({"inventory":coverage.inventory,"count_unit":"sample_area","total":9,
                        "assessed":coverage.assessed,"not_applicable":coverage.not_applicable,
                        "unassessed":coverage.unassessed,"undetermined":coverage.undetermined}),
                    Some(selector.clone()),
                    json!({"kind":"sample","admission":state.admission.as_str(),"sample":state.sample.get(),
                        "meaning":sample_meaning_wire(&self.sample_result().expect("sample state")),
                        "selector":selector,"description":description,"sample_location":sample_location,
                        "context":{"state":state.context.as_str(),"model_assessment":null,"findings":{
                            "errors":state.context_findings.errors,"warnings":state.context_findings.warnings,"notices":state.context_findings.notices}},
                        "witnesses":witnesses,"explanations":explanations,"omissions":omissions}),
                )
            }
            Operation::Check(profile) => {
                let coverage = self.requested_coverage();
                let description = crate::assessment::profile(profile);
                for selected in description.areas().filter(|area| area.unfinished_condition) {
                    let area = selected.id;
                    let status = self.area_status(selected);
                    if !matches!(
                        status,
                        AssessmentStatus::Unassessed | AssessmentStatus::Undetermined
                    ) {
                        continue;
                    }
                    total += 1;
                    retain(
                        &mut retained,
                        budget,
                        limits.conditions,
                        || json!({"code":if area==crate::proof::AREA {"coverage.advisory_unavailable"} else {"coverage.unavailable"},"severity":"incomplete","basis":{"kind":"operational","id":unfinished_check_basis(area,status)},"message":"Requested assessment area has unfinished applicable work","assessment_unit":area,"related":[],"arguments":{}}),
                    );
                }
                (
                    Some(profile.identity()),
                    coverage.inventory,
                    json!({"inventory":coverage.inventory,"count_unit":coverage.count_unit,"total":coverage.total,"assessed":coverage.assessed,"not_applicable":coverage.not_applicable,"unassessed":coverage.unassessed,"undetermined":coverage.undetermined}),
                    None,
                    json!({"kind":"check"}),
                )
            }
            Operation::Inspect {
                document,
                pointer,
                view,
                json: payload,
            } => {
                if !self.outcome.complete {
                    total += 1;
                    retain(
                        &mut retained,
                        budget,
                        limits.conditions,
                        || json!({"code":"inspection.unavailable","severity":"incomplete","basis":{"kind":"operational","id":"view-establishment"},"message":"Requested view establishment is incomplete","assessment_unit":"inspection","related":[],"arguments":{}}),
                    );
                }
                let mut explanations = Vec::new();
                let mut alternatives = Vec::new();
                if view == View::Effective
                    && let Some(model) = self.effective.first()
                {
                    let mut rows = model.explanations();
                    for explanation in rows.by_ref() {
                        retain_explanation(
                            &mut explanations,
                            budget,
                            limits.conditions,
                            explanation,
                            false,
                            PointerPresentation {
                                cap: limits.pointer_bytes,
                                work: pointer_work,
                                omissions: &mut omissions,
                            },
                        );
                    }
                    if rows.limited {
                        omissions.push(json!({"field":"result.explanations","reason":"not_available","omitted_count":1,"source_id":document.get()}));
                    }
                    if budget.count_omitted() || budget.byte_omitted() {
                        omissions.push(json!({"field":"result.explanations","reason":if budget.count_omitted(){"count_limit"}else{"byte_limit"},"omitted_count":1,"source_id":document.get()}));
                    }
                    retain_alternatives(
                        model,
                        &mut alternatives,
                        budget,
                        limits.conditions,
                        PointerPresentation {
                            cap: limits.pointer_bytes,
                            work: pointer_work,
                            omissions: &mut omissions,
                        },
                    );
                }
                required = self.required_presentation(
                    budget,
                    limits.conditions,
                    pointer_work,
                    limits.report_bytes == reserve,
                );
                self.retain_required_conditions(
                    &mut retained,
                    budget,
                    limits.conditions,
                    &mut total,
                );
                let raw = payload.map(JsonView::raw_json).or_else(|| {
                    self.effective
                        .first()
                        .and_then(|model| model.generated.as_deref())
                });
                let payload_budget =
                    limits.report_bytes - reserve - (initial_budget - budget.remaining());
                let present =
                    raw.is_some() && !human && raw.is_some_and(|s| s.len() <= payload_budget);
                if raw.is_some() && !present && !human {
                    omissions.push(json!({"field":"result.json","reason":"report_limit","omitted_count":1,"source_id":document.get()}));
                }
                if let Some(reason) = self
                    .effective
                    .first()
                    .and_then(|model| model.generation_failure)
                {
                    omissions.push(json!({"field":"result.json","reason":reason.omission_reason(),"omitted_count":1,"source_id":document.get(),"unavailable_reason":reason.reason}));
                }
                let shown_pointer =
                    pointer.len() <= limits.pointer_bytes && pointer.len() <= limits.pointer_total;
                if !shown_pointer {
                    omissions.push(json!({"field":"result.pointer","reason":"location_limit","omitted_count":1,"source_id":document.get()}));
                }
                let state = if payload.is_none()
                    && self
                        .effective
                        .first()
                        .is_none_or(|model| model.json().present().is_none())
                {
                    "unavailable"
                } else if self.outcome.complete && (present || human) && shown_pointer {
                    "available"
                } else {
                    "partial"
                };
                (
                    None,
                    None,
                    json!({"inventory":null,"count_unit":"inspection","total":1,"assessed":u32::from(self.outcome.complete),"not_applicable":0,"unassessed":0,"undetermined":u32::from(!self.outcome.complete)}),
                    Some(json!({"view":view.as_str(),"document":document.get(),"pointer":pointer})),
                    json!({"kind":"inspect","view":view.as_str(),"state":state,"root":document.get(),"pointer":if shown_pointer { Some(pointer) } else { None },"json_present":present,"explanations":explanations,"alternatives":alternatives,"omissions":omissions}),
                )
            }
        };
        let mut sources = Vec::new();
        for source in cat.retained_sources() {
            let (row, omission) = source_wire(source);
            sources.push(row);
            omissions.extend(omission);
        }
        let mut unavailable: Vec<_> = self
            .scope
            .iter()
            .filter(|id| {
                cat.source(**id)
                    .is_some_and(|s| s.json().present().is_none())
            })
            .map(|id| id.get())
            .collect();
        for id in &self.unavailable_dependencies {
            if !unavailable.contains(&id.get()) {
                unavailable.push(id.get());
            }
        }
        let ids: Vec<_> = self.scope.iter().map(|id| id.get()).collect();
        let dependencies: Vec<_> = self.dependencies.iter().map(|id| id.get()).collect();
        let mut assessed = ids.clone();
        assessed.extend(dependencies.iter().copied());
        let mut rows = Vec::new();
        let sample_areas: &[&str] = if matches!(self.operation, Operation::Sample) {
            &crate::sample::AREAS
        } else {
            &[]
        };
        let profile = self.assessment_profile();
        let row_total = sample_areas.len() + profile.map_or(0, |profile| profile.area_count());
        if row_total > 0 {
            let areas = sample_areas.iter().copied().chain(
                profile
                    .into_iter()
                    .flat_map(|profile| profile.areas())
                    .map(|area| area.id),
            );
            for area in areas {
                let Some(charge) = budget.admit(256, 2048) else {
                    continue;
                };
                let status = self
                    .assessment_area_status(area)
                    .present()
                    .expect("known requested inventory");
                let basis = if let Some(state) = &self.sample {
                    sample_coverage_basis(area, status, state)
                } else {
                    match status {
                        AssessmentStatus::Assessed | AssessmentStatus::NotApplicable => {
                            if area == crate::proof::AREA
                                || area == crate::proof::NUMERIC_AREA
                                || crate::sample::CONSISTENCY_AREAS.contains(&area)
                            {
                                json!({"kind":"advice","id":area,"citation":"RFC 9880 Appendix C"})
                            } else {
                                if area == crate::content_format::AREAS[1] {
                                    json!({"kind":"advice","id":"iana-content-format-positives/2"})
                                } else {
                                    json!({"kind":"rfc","citation":if area == crate::pattern::AREA {"RFC 9880 Appendix C.2; ECMA-262 2025 Section 22.2.1"} else if crate::units::AREAS.contains(&area) {"RFC 9880 Section 4.7"} else if area == crate::content_format::AREAS[0] {"RFC 9880 Appendix A; RFC 9193 Sections 3 and 6"} else {"RFC 9880 Appendix A"}})
                                }
                            }
                        }
                        AssessmentStatus::Unassessed | AssessmentStatus::Undetermined => {
                            json!({"kind":"operational","id":unfinished_check_basis(area,status)})
                        }
                    }
                };
                let mut row = json!({"id":area,"count_unit":if self.sample.is_some(){"sample_area"}else{"assessment_area"},"status":status.as_str()});
                // Move the basis to keep row staging within its precharged scratch allowance.
                row.as_object_mut()
                    .expect("coverage row")
                    .insert("basis".to_owned(), basis);
                let size = serde_json::to_string(&row)
                    .expect("bounded coverage row")
                    .len()
                    + 1;
                assert!(size <= 256, "coverage row exceeds precharge");
                budget.settle(charge, size);
                rows.push(row);
            }
        }
        let rows_omitted = row_total - rows.len();
        let full = profile.is_some_and(|profile| profile.enabled.full);
        let requirement_detail = full.then(|| self.requirement_detail(budget, &mut omissions));
        if rows_omitted > 0 {
            omissions.push(
                json!({"field":"coverage.rows","reason":"byte_limit","omitted_count":rows_omitted}),
            );
        }
        let (omitted, truncated, reasons) = budget.summary(
            total,
            retained.len(),
            &omissions,
            required.omitted > 0,
            [
                required.reason,
                required.locations.limited.then_some("location_limit"),
                required.locations.unavailable.then_some("not_available"),
            ],
        );
        let mut request = json!({"assessment":assessment,"inventory":inventory,"scope":{"kind":if self.selected { "selected" } else { "catalogue" },"roots":if self.selected { ids.clone() } else { Vec::new() },"dependencies":dependencies,"assessed":assessed,"unavailable_required":unavailable}});
        if let Some(selector) = selector {
            request
                .as_object_mut()
                .expect("request map")
                .insert("selector".into(), selector);
        }
        if let Some(state) = &self.sample {
            request["sample"] = json!(state.sample.get());
        }
        let mut baseline = json!({"id":"rfc9880/base-1","rfc":"9880","corrections":[],"interpretations":[],"deviations":[],"registry_knowledge":[]});
        if self.structure.context_used || matches!(self.operation, Operation::Inspect { .. }) {
            baseline["extensions"] = json!([{
                "id":crate::context::DRAFT,
                "sha256":crate::context::SHA256,
                "scope":"Context checking, composition and inspection",
                "limitations":["absent nullable defaults unresolved", "Context sdfRequired meaning undefined", "reference reuse outside Context unavailable", "general link meaning unavailable"]
            }]);
        }
        let temporal_use = self.structure.temporal_use
            | self.sample.as_ref().map_or(0, |state| state.temporal_use);
        if temporal_use & crate::civil::KNOWLEDGE_USED != 0
            || (profile.is_some_and(|profile| profile.enabled.modified)
                && self.structure.modified.knowledge_used)
        {
            // The additional mandatory reserve covers this fixed DTO and its encoding/growth buffers.
            baseline["temporal_knowledge"] = json!({"id":crate::scalar::KNOWLEDGE_ID,"from":"1972-01-01","through":"2026-12-31","transition_sha256":crate::scalar::TRANSITION_SHA256,"bulletin_sha256":crate::scalar::BULLETIN_SHA256});
        }
        let mut base = json!({"schema":"esdieff.report","version":"1.0","operation":if matches!(self.operation,Operation::Check(_)) { "check" } else { "inspect" },"request":null,"sources":[],"baseline":null,
            "policy":{"restrictions":if matches!(self.operation, Operation::Check(_)) { vec!["A001", "A002"] } else { vec!["A002"] },"strict_warnings":self.outcome.strict},"outcome":{"complete":self.outcome.complete,"errors":self.outcome.errors,"warnings":self.outcome.warnings,"notices":self.outcome.notices,"incomplete_checks":self.outcome.incomplete,"strict_warnings":self.outcome.strict,"disposition":self.outcome.disposition().as_str()},
            "coverage":{"requested":null,"broader":{"inventory":null,"state":"partial","inventory_status":"unavailable","omitted_areas":["unimplemented RFC requirements"]},"rows":[],"rows_omitted":rows_omitted,"omissions":if rows_omitted>0 { vec![json!({"field":"coverage.rows","reason":"byte_limit","omitted_count":rows_omitted})] } else { Vec::new() }},
            "diagnostics":{"retained":retained.len(),"omitted":omitted,"truncated":truncated,"reasons":reasons,"omissions":omissions,"items":[]}});
        // Move affected parent maps and profile text into their final graph.
        // A fallback may retain the previous graph while this one is built.
        base["request"] = request;
        base["baseline"] = baseline;
        base["coverage"]["requested"] = requested;
        base["sources"] = Value::Array(sources);
        base["coverage"]["rows"] = Value::Array(rows);
        base["diagnostics"]["items"] = Value::Array(retained);
        if full {
            base["coverage"]["broader"]["inventory"] = json!(crate::inventory::INVENTORY);
            base["coverage"]["broader"]["inventory_status"] = json!("available");
            base["coverage"]["broader"]["state"] = json!(self.broader_state());
            base["coverage"]["broader"]
                .as_object_mut()
                .expect("broader map")
                .remove("omitted_areas");
            if let Some(Some(mut detail)) = requirement_detail {
                if let Some(alias) = detail.remove("requirement_inventory") {
                    base["request"]
                        .as_object_mut()
                        .expect("request map")
                        .insert("requirement_inventory".into(), alias);
                }
                let coverage = base["coverage"].as_object_mut().expect("coverage map");
                for (key, value) in detail {
                    coverage.insert(key, value);
                }
            }
        }
        // Construct and move extensions after the existing metadata builders.
        // The independent reserve covers at most two complete DTO generations.
        if profile.is_some_and(|profile| profile.registry_requested()) {
            base["policy"].as_object_mut().expect("policy map").insert(
                "recorded_registry".to_owned(),
                Value::String(self.registry.policy.as_str().to_owned()),
            );
            base["baseline"]["registry_knowledge"] = self.registry.wire(full);
            base["baseline"]["interpretations"] = self.registry.interpretation_wire();
        }
        if let Some(state) = &self.sample {
            let usage = state.format_use;
            base["baseline"]["format_knowledge"] = crate::formats::wire(usage);
            base["version"] = json!("1.1");
            base["operation"] = json!("sample");
        }
        let used = crate::sample::ValueInterpretations {
            temporal: temporal_use,
            abstract_use: self.structure.abstract_use
                | self.sample.as_ref().map_or(0, |state| state.abstract_use),
            format: self.structure.format_use
                | self
                    .sample
                    .as_ref()
                    .and_then(|state| state.format_use)
                    .unwrap_or(0),
        };
        base["baseline"]["interpretations"]
            .as_array_mut()
            .expect("interpretation array")
            .extend(used.ids().map(|id| json!(id)));
        // Recorded on actual use, following ADR 0036: Copied reference relocation.
        if self
            .effective
            .iter()
            .any(|model| model.relocation_interpretation)
            || self
                .required
                .as_ref()
                .is_some_and(|required| required.relocation_interpretation)
        {
            base["baseline"]["interpretations"]
                .as_array_mut()
                .expect("interpretation array")
                .push(json!("copied-reference-relocation/1"));
        }
        // Recorded on actual use, following ADR 0037: Undeclared extension vocabulary.
        let recorded_consulted = self.registry.consulted
            & crate::registry::bits(crate::registry::RECORDED_EXTENSION_RECORDS)
            != 0;
        if self
            .structure
            .set_aside
            .relies_on_reading(recorded_consulted)
        {
            base["baseline"]["interpretations"]
                .as_array_mut()
                .expect("interpretation array")
                .push(json!("undeclared-extensions-ignorable/1"));
        }
        if profile.is_some_and(|profile| profile.enabled.units) {
            result.as_object_mut().expect("result map").insert(
                "unit_guidance".to_owned(),
                self.units
                    .as_ref()
                    .map_or_else(crate::units::unavailable_wire, crate::units::State::wire),
            );
        }
        (base, result, required, projection)
    }
    fn requirement_detail(
        &self,
        budget: &mut DetailBudget,
        omissions: &mut Vec<Value>,
    ) -> Option<serde_json::Map<String, Value>> {
        // The optional parent growth, alias, aggregate and omission records are
        // admitted before any new map or row is allocated.
        const WIRE: usize = 1024;
        const SCRATCH: usize = 32768;
        let charge = budget.admit(WIRE, SCRATCH)?;
        let coverage = self.requirement_counts();
        let mut result = serde_json::Map::new();
        result.insert(
            "requirement_inventory".into(),
            json!(crate::inventory::INVENTORY),
        );
        result.insert("requirements".into(), json!({"inventory":coverage.inventory,"count_unit":coverage.count_unit,"total":coverage.total,"assessed":coverage.assessed,"not_applicable":coverage.not_applicable,"unassessed":coverage.unassessed,"undetermined":coverage.undetermined}));
        let mut rows = Vec::new();
        for (index, status) in self.structure.coverage.requirements.into_iter().enumerate() {
            const ROW_WIRE: usize = 1024;
            const ROW_SCRATCH: usize = 8192;
            let Some(charge) = budget.admit(ROW_WIRE, ROW_SCRATCH) else {
                break;
            };
            let row = crate::inventory::requirement_row(index, status);
            let size = serde_json::to_string(&row)
                .expect("bounded requirement row")
                .len()
                + 1;
            assert!(size <= ROW_WIRE, "requirement row exceeds precharge");
            budget.settle(charge, size);
            rows.push(row);
        }
        let omitted = 54 - rows.len();
        result.insert("requirement_rows_retained".into(), json!(rows.len()));
        result.insert("requirement_rows_omitted".into(), json!(omitted));
        result.insert("requirement_omissions".into(), if omitted == 0 { json!([]) } else {
            let omission = json!({"field":"coverage.requirement_rows","reason":"byte_limit","omitted_count":omitted});
            omissions.push(omission.clone());
            json!([omission])
        });
        let size = serde_json::to_string(&result)
            .expect("bounded requirement summary")
            .len()
            + 1;
        assert!(size <= WIRE, "requirement summary exceeds precharge");
        budget.settle(charge, size);
        result.insert("requirement_rows".into(), Value::Array(rows));
        Some(result)
    }
    fn encode(&self, human: bool) -> String {
        let (base, mut result, required) = self.metadata(human);
        if matches!(self.operation, Operation::Sample) {
            assert!(
                self.required.is_none(),
                "Sample has no required presentation encoder"
            );
            assert!(
                result.get("alternatives").is_none() && result.get("json_present").is_none(),
                "Sample has no manual raw JSON fields"
            );
            let mut output = serde_json::to_string(&base).expect("bounded report metadata");
            output.pop();
            output.push_str(",\"result\":");
            serde_json::to_writer(JsonStringWriter(&mut output), &result)
                .expect("bounded Sample result");
            output.push('}');
            debug_assert!(
                output.len() <= self.catalogue.limits.report_bytes,
                "mandatory reserve and optional precharge"
            );
            return output;
        }
        let alternatives = result
            .as_object_mut()
            .expect("result map")
            .remove("alternatives");
        let mut result_text = serde_json::to_string(&result).expect("bounded report result");
        if let Some(Value::Array(owners)) = alternatives {
            result_text.pop();
            result_text.push_str(",\"alternatives\":[");
            for (index, mut owner) in owners.into_iter().enumerate() {
                if index > 0 {
                    result_text.push(',');
                }
                let branches = owner
                    .as_object_mut()
                    .expect("alternative owner")
                    .remove("branches")
                    .expect("branches");
                let mut owner_text = serde_json::to_string(&owner).expect("owner metadata");
                owner_text.pop();
                result_text.push_str(&owner_text);
                result_text.push_str(",\"branches\":[");
                let Value::Array(branches) = branches else {
                    unreachable!("branches array")
                };
                for (index, mut branch) in branches.into_iter().enumerate() {
                    if index > 0 {
                        result_text.push(',');
                    }
                    let raw = branch
                        .as_object_mut()
                        .expect("branch map")
                        .remove("raw_json")
                        .expect("raw branch payload");
                    let mut text = serde_json::to_string(&branch).expect("branch metadata");
                    text.pop();
                    result_text.push_str(&text);
                    result_text.push_str(",\"json\":");
                    result_text.push_str(raw.as_str().unwrap_or("null"));
                    result_text.push('}');
                }
                result_text.push_str("]}");
            }
            result_text.push_str("]}");
        }
        if let Operation::Inspect { json: payload, .. } = self.operation {
            result_text.pop();
            result_text.push_str(",\"json\":");
            if result["json_present"].as_bool() == Some(true) {
                result_text.push_str(
                    payload
                        .map(JsonView::raw_json)
                        .or_else(|| {
                            self.effective
                                .first()
                                .and_then(|model| model.generated.as_deref())
                        })
                        .expect("present inspection payload"),
                );
            } else {
                result_text.push_str("null");
            }
            result_text.push('}');
        }
        if self.required.is_some() {
            result_text.pop();
            self.write_required_json(&mut result_text, &required);
            result_text.push('}');
        }
        // Encode model JSON only from its validated original span, without a lossy numeric conversion.
        let mut output = serde_json::to_string(&base).expect("bounded report metadata");
        output.pop();
        output.push_str(",\"result\":");
        output.push_str(&result_text);
        output.push('}');
        debug_assert!(
            output.len() <= self.catalogue.limits.report_bytes,
            "mandatory reserve and optional precharge"
        );
        output
    }
}

fn human_location(location: &Value, output: &mut HumanText) {
    use std::fmt::Write;
    match location["kind"].as_str() {
        Some("source" | "sample") => write!(
            output,
            "source {} {}",
            location["source_id"], location["pointer"]
        )
        .expect("String write"),
        Some("effective") => write!(
            output,
            "effective {} {}",
            location["root"], location["pointer"]
        )
        .expect("String write"),
        _ => output.push_str("location unavailable"),
    }
}

fn sample_location(source: &crate::Source) -> Value {
    let root = source.parsed.root;
    json!({"kind":"sample","source_id":source.id.get(),"pointer":root.map(|_| ""),
        "span":if source.original { root.map(|node| { let span = &source.parsed.nodes[node].span; [span.start,span.end] }) } else { None }})
}
fn tag_condition(mut condition: Value, domain: Option<&'static str>) -> Value {
    if let Some(domain) = domain {
        condition["domain"] = json!(domain);
    }
    condition
}
struct SampleEvidence<'a, 'b> {
    finding: &'a crate::finding::Finding,
    model: &'a crate::EffectiveModel<'b>,
    sample: &'a crate::Source,
}
fn retain_sample_finding(
    items: &mut Vec<Value>,
    witnesses: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    evidence: SampleEvidence<'_, '_>,
    presentation: PointerPresentation<'_>,
) {
    let SampleEvidence {
        finding,
        model,
        sample,
    } = evidence;
    let PointerPresentation {
        cap: pointer_cap,
        work,
        omissions,
    } = presentation;
    if !budget.allow_count(items.len(), cap) {
        return;
    }
    let parameter = finding
        .parameter
        .map(|node| crate::EffectiveJson { model, node });
    let effective = parameter.map(|json| json.location());
    let effective_size = effective.map(|location| location.pointer_size(pointer_cap, work));
    let source = parameter.map(|json| json.n().source);
    let source_size = source.map(|source| source.pointer_size(pointer_cap, work));
    let alternative = finding
        .alternative
        .map(|node| crate::EffectiveJson { model, node }.location());
    let alternative_size = alternative.map(|location| location.pointer_size(pointer_cap, work));
    let paired_sample = finding.paired_sample.map(|node| crate::JsonView {
        source: sample,
        node,
    });
    let paired_size = paired_sample.map(|sample| {
        (sample.source.parsed.root != Some(sample.node))
            .then(|| sample.pointer_size(pointer_cap, work))
    });
    let sample = crate::JsonView {
        source: sample,
        node: finding.sample_node,
    };
    // The acquired root has the mandatory empty pointer; only nested locations need traversal.
    let sample_size = (sample.source.parsed.root != Some(sample.node))
        .then(|| sample.pointer_size(pointer_cap, work));
    let entry = (finding.constraint == "required")
        .then(|| parameter.and_then(|json| json.text().present()))
        .flatten();
    let entry = entry.filter(|entry| work.debit(entry.len() + 1).is_ok());
    let size_bytes = effective_size
        .as_ref()
        .and_then(|size| size.as_ref().ok())
        .map_or(0, |size| size.bytes)
        + source_size
            .as_ref()
            .and_then(|size| size.as_ref().ok())
            .map_or(0, |size| size.bytes)
        + alternative_size
            .as_ref()
            .and_then(|size| size.as_ref().ok())
            .map_or(0, |size| size.bytes)
        + sample_size
            .as_ref()
            .and_then(|size| size.as_ref().ok())
            .map_or(0, |size| size.bytes)
        + paired_size
            .as_ref()
            .and_then(Option::as_ref)
            .and_then(|size| size.as_ref().ok())
            .map_or(0, |size| size.bytes);
    let base_wire = 3072 + size_bytes * 12;
    let entry_wire = entry.map_or(0, |name| name.len() * 6);
    let entry =
        entry.filter(|_| budget.fits(base_wire + entry_wire, 32768 + (base_wire + entry_wire) * 8));
    let wire = base_wire + entry.map_or(0, |name| name.len() * 6);
    let scratch = 32768 + wire * 8;
    let Some(_charge) = budget.admit(wire, scratch) else {
        budget.observe_omitted(false, true);
        return;
    };
    let mut basis = json!({"kind":finding.basis.kind()});
    if !finding.basis.citation().is_empty() {
        basis["citation"] = json!(finding.basis.citation());
    }
    if let Some(id) = finding.basis.id() {
        basis["id"] = json!(id);
    }
    let incomplete = finding.basis.incomplete();
    let mut condition = json!({"domain":if incomplete {"operation"} else {"sample"},"code":finding.code,"severity":if incomplete {"incomplete"} else {"error"},"basis":basis,
        "message":finding.message,"primary":null,"related":[],"arguments":{"constraint":finding.constraint}});
    if incomplete {
        condition["assessment_unit"] = json!(if finding.constraint == "pattern" {
            "sample-text-boolean/1"
        } else {
            "sample-abstract-values/1"
        });
    }
    let pointer = sample_size.map_or_else(
        || Ok(String::new()),
        |size| {
            size.and_then(|size| {
                work.debit(size.render_work())?;
                Ok(sample.render_pointer(size))
            })
        },
    );
    let (pointer, reason) = location_pointer(pointer);
    condition["primary"] = json!({"kind":"sample","source_id":sample.source_id().get(),"pointer":pointer,
        "span":if sample.source.original { let span=sample.span(); Some([span.start,span.end]) } else { None }});
    if let Some(reason) = reason {
        let omission = json!({"field":"diagnostics.items.primary.pointer","reason":reason,"omitted_count":1,"source_id":sample.source_id().get()});
        condition["primary"]["omission"] = omission.clone();
        omissions.push(omission);
    }
    let mut related = Vec::with_capacity(2 + usize::from(paired_sample.is_some()));
    if let Some(location) = effective {
        let pointer = effective_size
            .expect("measured effective pointer")
            .and_then(|size| {
                work.debit(size.render_work())?;
                Ok(location.render_pointer(size))
            });
        let (pointer, reason) = location_pointer(pointer);
        if let Some(reason) = reason {
            omissions.push(json!({"field":"diagnostics.items.related.pointer","reason":reason,"omitted_count":1}));
        }
        related.push(
            json!({"kind":"effective","root":location.root_document().get(),"pointer":pointer}),
        );
    }
    if let Some(source) = source {
        let pointer = source_size
            .expect("measured source pointer")
            .and_then(|size| {
                work.debit(size.render_work())?;
                Ok(source.render_pointer(size))
            });
        related.push(source_location(
            source,
            pointer,
            "diagnostics.items.related.pointer",
            omissions,
        ));
    }
    if let Some(sample) = paired_sample {
        let pointer = paired_size.flatten().map_or_else(
            || Ok(String::new()),
            |size| {
                size.and_then(|size| {
                    work.debit(size.render_work())?;
                    Ok(sample.render_pointer(size))
                })
            },
        );
        let (pointer, reason) = location_pointer(pointer);
        let mut location = json!({"kind":"sample","source_id":sample.source_id().get(),"pointer":pointer,
            "span":if sample.source.original { let span=sample.span(); Some([span.start,span.end]) } else { None }});
        if let Some(reason) = reason {
            let omission = json!({"field":"diagnostics.items.related.pointer","reason":reason,"omitted_count":1,"source_id":sample.source_id().get()});
            location["omission"] = omission.clone();
            omissions.push(omission);
        }
        related.push(location);
    }
    condition["related"] = Value::Array(related);
    if finding.constraint == "required" {
        condition["arguments"]["entry"] = json!(entry);
        if entry.is_none() {
            omissions.push(json!({"field":"diagnostics.items.arguments.entry","reason":"byte_limit","omitted_count":1}));
        }
    }
    if effective.is_some() && !incomplete {
        if witnesses.len() < cap {
            let mut witness = json!({"kind":"rejection","effective":condition["related"][0],"sample":condition["primary"],"basis":condition["basis"],"constraint":finding.constraint});
            if witness["sample"].get("omission").is_some() {
                witness["sample"]["omission"]["field"] = json!("result.witnesses.sample.pointer");
            }
            if let Some(alternative) = alternative {
                witness["alternative"] = sample_alternative_location(
                    alternative,
                    alternative_size.expect("measured alternative pointer"),
                    work,
                    omissions,
                );
            }
            witnesses.push(witness);
        } else {
            omissions
                .push(json!({"field":"result.witnesses","reason":"count_limit","omitted_count":1}));
        }
    }
    items.push(condition);
}
fn sample_alternative_location(
    location: crate::EffectiveLocation<'_>,
    size: Result<crate::effective::EffectivePointerSize, crate::json::PointerError>,
    work: &mut crate::json::PointerBudget,
    omissions: &mut Vec<Value>,
) -> Value {
    let pointer = size.and_then(|size| {
        work.debit(size.render_work())?;
        Ok(location.render_pointer(size))
    });
    let (pointer, reason) = location_pointer(pointer);
    if let Some(reason) = reason {
        omissions.push(json!({"field":"result.witnesses.alternative.pointer","reason":reason,"omitted_count":1}));
    }
    json!({"kind":"effective","root":location.root_document().get(),"pointer":pointer})
}
fn retain_admission_witness(
    witnesses: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    description: &Value,
    sample: &Value,
    alternative: Option<crate::EffectiveJson<'_>>,
    presentation: PointerPresentation<'_>,
) -> bool {
    if !budget.allow_count(witnesses.len(), cap) {
        return false;
    }
    let pointer_bytes = description["pointer"].as_str().map_or(0, str::len);
    let alternative = alternative.map(|json| json.location());
    let alternative_size =
        alternative.map(|location| location.pointer_size(presentation.cap, presentation.work));
    let alternative_bytes = alternative_size
        .as_ref()
        .and_then(|size| size.as_ref().ok())
        .map_or(0, |size| size.bytes);
    let wire = 1024 + (pointer_bytes + alternative_bytes) * 6;
    let scratch = 32768 + wire * 8;
    let Some(charge) = budget.admit(wire, scratch) else {
        budget.observe_omitted(false, true);
        return false;
    };
    let mut witness = json!({"kind":"admission","effective":description,"sample":sample,
        "basis":{"kind":"rfc","citation":"RFC 9880 Appendix C"}});
    if let Some(alternative) = alternative {
        witness["alternative"] = sample_alternative_location(
            alternative,
            alternative_size.expect("measured admission alternative pointer"),
            presentation.work,
            presentation.omissions,
        );
    }
    let size = serde_json::to_string(&witness)
        .expect("witness encoding")
        .len()
        + 1;
    assert!(size <= wire, "witness exceeds its precharged wire bound");
    budget.settle(charge, size);
    witnesses.push(witness);
    true
}
/// A report's source row and, when its label was omitted, the omission record.
/// A condition basis as a report writes it.
#[derive(Clone, Copy)]
pub(crate) struct WireBasis {
    pub(crate) kind: &'static str,
    pub(crate) citation: Option<&'static str>,
    pub(crate) id: Option<&'static str>,
}
impl WireBasis {
    /// The basis of a source decoding issue with `code`.
    pub(crate) fn decoding(code: &str) -> Self {
        let (kind, citation, id) = match code {
            "json.syntax" => ("json", Some("RFC 8259"), None),
            "input.duplicate_name" => ("acceptance", None, Some("A002")),
            "document.envelope" => ("rfc", Some("RFC 9880 Section 3"), None),
            _ => ("operational", None, Some("source-decoding")),
        };
        Self { kind, citation, id }
    }
    pub(crate) fn to_json(self) -> Value {
        let mut basis = json!({"kind":self.kind});
        if let Some(citation) = self.citation {
            basis["citation"] = json!(citation);
        }
        if let Some(id) = self.id {
            basis["id"] = json!(id);
        }
        basis
    }
}
pub(crate) fn source_wire(source: &crate::Source) -> (Value, Option<Value>) {
    source_row(source, source_status(source), source.bytes.len())
}
/// A source row whose decode status and retained bytes the caller establishes, such as for a
/// Recorded trace, which is decoded line by line and not retained.
pub(crate) fn source_row(
    source: &crate::Source,
    decode: &str,
    retained_bytes: usize,
) -> (Value, Option<Value>) {
    let mut value = json!({"id":source.id.get(),"key":source.key,"role":source.role.as_str(),"acquisition":source.acquisition.as_str(),"decode":decode,"content_complete":source.complete,"retained_bytes":retained_bytes});
    let map = value.as_object_mut().expect("source map");
    for (name, field) in [
        ("label", &source.label),
        ("uri", &source.uri),
        ("version_token", &source.version),
        ("digest", &source.digest),
    ] {
        if let Some(field) = field {
            map.insert(name.into(), json!(field));
        }
    }
    let omission = source.label_omitted.then(|| {
        let omission = json!({"field":"sources.label","reason":"byte_limit","omitted_count":1,"source_id":source.id.get()});
        value["label"] = Value::Null;
        value["label_omitted"] = omission.clone();
        omission
    });
    (value, omission)
}
fn source_status(source: &crate::Source) -> &'static str {
    if source.acquisition == Acquisition::Failed {
        "unavailable"
    } else if source.parsed.rejected {
        "rejected"
    } else if source.parsed.root.is_none() {
        "unavailable"
    } else if !source.complete || source.parsed.issue_count > 0 {
        "partial"
    } else {
        "usable"
    }
}

struct PointerPresentation<'a> {
    cap: usize,
    work: &'a mut crate::json::PointerBudget,
    omissions: &'a mut Vec<Value>,
}
struct Attribution {
    domain: Option<&'static str>,
    owner: Option<&'static str>,
}
fn retain_structure(
    items: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    finding: &crate::structure::Finding<'_>,
    models: &[crate::EffectiveModel<'_>],
    presentation: PointerPresentation<'_>,
    attribution: Attribution,
) {
    if finding.proof.is_some() {
        retain_proof(
            items,
            budget,
            cap,
            finding,
            models,
            presentation,
            attribution.owner,
        );
        return;
    }
    let PointerPresentation {
        cap: pointer_cap,
        work: pointer_work,
        omissions,
    } = presentation;
    if !budget.allow_count(items.len(), cap) {
        return;
    }
    let pointer_size = finding.json.pointer_size(pointer_cap, pointer_work);
    let effective_location = finding.effective.and_then(|(root, node)| {
        models
            .iter()
            .find(|model| model.source == root)
            .map(|model| crate::EffectiveJson { model, node }.location())
    });
    let effective_size =
        effective_location.map(|location| location.pointer_size(pointer_cap, pointer_work));
    let quality = finding
        .rule
        .names_quality()
        .then(|| finding.json.member_name().present())
        .flatten();
    let observed_occurrences = finding
        .rule
        .observed_occurrences()
        .filter(|_| finding.effective.is_none());
    // Reserve escaped arguments before DTO construction, including names independent of pointer retention.
    let argument_wire = if observed_occurrences.is_some() {
        128
    } else {
        0
    } + quality.map_or(0, |name| 64 + name.len() * 6)
        + finding
            .replacement
            .as_ref()
            .map_or(0, |text| 64 + text.len() * 6);
    // Advice and registration bases retain both a stable identifier and a citation, so their
    // escaped bytes are precharged.
    let advice_wire = if matches!(finding.rule.basis_kind(), "advice" | "registration") {
        64 + finding.rule.basis_id().map_or(0, |id| id.len() * 6)
            + finding
                .rule
                .citation()
                .map_or(0, |citation| citation.len() * 6)
    } else {
        0
    };
    let wire = 1024
        + usize::from(attribution.domain.is_some()) * 64
        + attribution.owner.map_or(0, |id| 64 + id.len() * 6)
        + argument_wire
        + advice_wire
        + finding
            .rule
            .assessment_unit()
            .map_or(0, |id| id.len() * 6 + 64)
        + pointer_size.as_ref().map_or(0, |size| size.bytes) * 6
        + effective_size.as_ref().map_or(0, |size| {
            512 + size.as_ref().map_or(0, |size| size.bytes) * 6
        });
    let scratch = 32768 + wire * 8;
    let Some(charge) = budget.admit(wire, scratch) else {
        budget.observe_omitted(false, true);
        return;
    };
    let pointer = pointer_size.and_then(|size| {
        pointer_work.debit(size.render_work())?;
        Ok(finding.json.render_pointer(size))
    });
    let (pointer, omission_reason) = match pointer {
        Ok(pointer) => (Some(pointer), None),
        Err(crate::json::PointerError::NotAvailable) => (None, Some("not_available")),
        Err(crate::json::PointerError::Limit) => (None, Some("location_limit")),
    };
    let mut primary = json!({"kind":"source","source_id":finding.json.source_id().get(),"pointer":pointer,"span":[finding.json.span().start,finding.json.span().end]});
    let omission = omission_reason.map(|reason| json!({"field":if effective_location.is_some(){"diagnostics.items.related.pointer"}else{"diagnostics.items.primary.pointer"},"reason":reason,"omitted_count":1,"source_id":finding.json.source_id().get()}));
    if let Some(omission) = &omission {
        primary["omission"] = omission.clone();
    }
    let mut related = Vec::new();
    let mut effective_omission = None;
    if let Some(location) = effective_location {
        let pointer = effective_size
            .expect("effective location size")
            .and_then(|size| {
                pointer_work.debit(size.render_work())?;
                Ok(location.render_pointer(size))
            });
        let (pointer, reason) = location_pointer(pointer);
        related.push(primary);
        primary =
            json!({"kind":"effective","root":location.root_document().get(),"pointer":pointer});
        effective_omission=reason.map(|reason|json!({"field":"diagnostics.items.primary.pointer","reason":reason,"omitted_count":1,"source_id":location.root_document().get()}));
        if let Some(omission) = &effective_omission {
            primary["omission"] = omission.clone();
        }
    }
    let mut basis = json!({"kind":finding.rule.basis_kind()});
    if let Some(citation) = finding.rule.citation() {
        basis["citation"] = json!(citation);
    }
    if let Some(id) = finding.rule.basis_id() {
        basis["id"] = json!(id);
    }
    let mut arguments = observed_occurrences
        .map_or_else(|| json!({}), |count| json!({"observed_occurrences":count}));
    if let Some(quality) = quality {
        arguments["quality"] = json!(quality);
    }
    if let Some(replacement) = &finding.replacement {
        arguments["replacement"] = Value::String(replacement.clone());
    }
    let mut condition = tag_condition(
        json!({"code":finding.rule.code(),"severity":finding.rule.severity(),"basis":basis,"message":finding.rule.message(),"primary":primary,"related":related,"arguments":arguments}),
        attribution.domain,
    );
    if let Some(unit) = finding.rule.assessment_unit() {
        condition["assessment_unit"] = json!(unit);
    }
    if let Some(owner) = attribution.owner {
        condition["coverage_owner"] = json!(owner);
    }
    let size = serde_json::to_string(&condition)
        .expect("bounded structural condition")
        .len()
        + 1
        + omission.as_ref().map_or(0, |o| {
            serde_json::to_string(o)
                .expect("bounded pointer omission")
                .len()
                + 1
        })
        + effective_omission.as_ref().map_or(0, |omission| {
            serde_json::to_string(omission)
                .expect("effective pointer omission")
                .len()
                + 1
        });
    assert!(
        size <= wire,
        "structural condition exceeds its precharged wire bound"
    );
    budget.settle(charge, size);
    items.push(condition);
    if let Some(omission) = omission {
        omissions.push(omission);
    }
    if let Some(omission) = effective_omission {
        omissions.push(omission);
    }
}

fn retain_explanation(
    items: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    explanation: crate::Explanation<'_>,
    alternative: bool,
    presentation: PointerPresentation<'_>,
) -> bool {
    let PointerPresentation {
        cap: pointer_cap,
        work: pointer_work,
        omissions,
    } = presentation;
    if !budget.allow_count(items.len(), cap) {
        return false;
    }
    let effective = explanation.effective();
    let effective_size = effective.pointer_size(pointer_cap, pointer_work);
    let mut sources = [None, None];
    let mut sizes = [None, None];
    let mut size_bytes = effective_size.as_ref().map_or(0, |size| size.bytes);
    for (index, json) in explanation.sources().enumerate() {
        let size = json.pointer_size(pointer_cap, pointer_work);
        size_bytes += size.as_ref().map_or(0, |size| size.bytes);
        sources[index] = Some(json);
        sizes[index] = Some(size);
    }
    // Existing established Boolean defaults are true. Reserve the optional JSON
    // annotation and human value before constructing either presentation.
    let default_value = explanation.kind() == crate::OriginKind::Default;
    let default_wire = if default_value { 64 } else { 0 };
    let consumer = explanation.consumer();
    let consumer_size = consumer.map(|source| source.pointer_size(pointer_cap, pointer_work));
    size_bytes += consumer_size
        .as_ref()
        .and_then(|size| size.as_ref().ok())
        .map_or(0, |size| size.bytes);
    let wire = 3072 + default_wire + size_bytes * 6;
    let scratch = 32768 + wire * 8;
    let Some(_charge) = budget.admit(wire, scratch) else {
        budget.observe_omitted(false, true);
        return false;
    };
    let pointer = effective_size.and_then(|size| {
        pointer_work.debit(size.render_work())?;
        Ok(effective.render_pointer(size))
    });
    let (pointer, reason) = location_pointer(pointer);
    let mut effective_wire =
        json!({"kind":"effective","root":effective.root_document().get(),"pointer":pointer});
    if let Some(reason) = reason {
        let field = if alternative {
            "result.alternatives.explanations.effective.pointer"
        } else {
            "result.explanations.effective.pointer"
        };
        let omission = json!({"field":field,"reason":reason,"omitted_count":1,"source_id":effective.root_document().get()});
        effective_wire["omission"] = omission.clone();
        omissions.push(omission);
    }
    let mut source_wire = Vec::with_capacity(2);
    for (index, json) in sources
        .into_iter()
        .enumerate()
        .filter_map(|(index, json)| json.map(|json| (index, json)))
    {
        let pointer = sizes[index]
            .take()
            .expect("source pointer measurement")
            .and_then(|size| {
                pointer_work.debit(size.render_work())?;
                Ok(json.render_pointer(size))
            });
        let field = if alternative {
            "result.alternatives.explanations.sources.pointer"
        } else {
            "result.explanations.sources.pointer"
        };
        source_wire.push(source_location(json, pointer, field, omissions));
    }
    let mut value = json!({"kind":explanation.kind().as_str(),"effective":effective_wire,"sources":source_wire});
    if default_value {
        value["default_value"] = json!(true);
    }
    if let Some(source) = consumer {
        let pointer = consumer_size.expect("consumer measured").and_then(|size| {
            pointer_work.debit(size.render_work())?;
            Ok(source.render_pointer(size))
        });
        let field = if alternative {
            "result.alternatives.explanations.consumer.pointer"
        } else {
            "result.explanations.consumer.pointer"
        };
        value["consumer"] = source_location(source, pointer, field, omissions);
    }
    items.push(value);
    true
}
pub(crate) fn location_pointer(
    pointer: Result<String, crate::json::PointerError>,
) -> (Option<String>, Option<&'static str>) {
    match pointer {
        Ok(pointer) => (Some(pointer), None),
        Err(crate::json::PointerError::NotAvailable) => (None, Some("not_available")),
        Err(crate::json::PointerError::Limit) => (None, Some("location_limit")),
    }
}

pub(crate) fn source_location(
    source: JsonView<'_>,
    pointer: Result<String, crate::json::PointerError>,
    field: &'static str,
    omissions: &mut Vec<Value>,
) -> Value {
    let (pointer, reason) = location_pointer(pointer);
    let span = source.span();
    let mut location = json!({"kind":"source","source_id":source.source_id().get(),"pointer":pointer,"span":[span.start,span.end]});
    if let Some(reason) = reason {
        let omission = json!({"field":field,"reason":reason,"omitted_count":1,"source_id":source.source_id().get()});
        location["omission"] = omission.clone();
        omissions.push(omission);
    }
    location
}

fn retain_effective_condition(
    items: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    json: crate::EffectiveJson<'_>,
    reason: crate::effective::Failure,
    presentation: PointerPresentation<'_>,
    domain: Option<&'static str>,
) {
    let PointerPresentation {
        cap: pointer_cap,
        work: pointer_work,
        omissions,
    } = presentation;
    if !budget.allow_count(items.len(), cap) {
        return;
    }
    let location = json.location();
    let measured = location.pointer_size(pointer_cap, pointer_work);
    let sources = [json.n().needed_ref, Some(json.source_json())];
    let mut sizes = [None, None];
    let mut bytes = measured.as_ref().map_or(0, |size| size.bytes);
    for (index, source) in sources.iter().enumerate() {
        if let Some(source) = source {
            let size = source.pointer_size(pointer_cap, pointer_work);
            bytes += size.as_ref().map_or(0, |size| size.bytes);
            sizes[index] = Some(size);
        }
    }
    if reason.definite() {
        bytes += sizes[0]
            .as_ref()
            .and_then(|size| size.as_ref().ok())
            .map_or(0, |size| size.bytes);
    }
    let wire = 3072 + usize::from(domain.is_some()) * 64 + bytes * 6;
    let scratch = 32768 + wire * 8;
    let Some(_charge) = budget.admit(wire, scratch) else {
        budget.observe_omitted(false, true);
        return;
    };
    let pointer = measured.and_then(|size| {
        pointer_work.debit(size.render_work())?;
        Ok(location.render_pointer(size))
    });
    let (pointer, omitted) = location_pointer(pointer);
    let mut primary =
        json!({"kind":"effective","root":json.root_document().get(),"pointer":pointer});
    if let Some(reason) = omitted {
        let omission = json!({"field":"diagnostics.items.primary.pointer","reason":reason,"omitted_count":1,"source_id":json.root_document().get()});
        primary["omission"] = omission.clone();
        omissions.push(omission);
    }
    let mut related = Vec::with_capacity(2);
    for (index, source) in sources
        .into_iter()
        .enumerate()
        .filter_map(|(index, source)| source.map(|source| (index, source)))
    {
        let pointer = sizes[index]
            .take()
            .expect("source location size")
            .and_then(|size| {
                pointer_work.debit(size.render_work())?;
                Ok(source.render_pointer(size))
            });
        related.push(source_location(
            source,
            pointer,
            "diagnostics.items.related.pointer",
            omissions,
        ));
    }
    for (index, source) in json.n().conflicts.iter().enumerate() {
        let measured = source.pointer_size(pointer_cap, pointer_work);
        let wire = 512 + measured.as_ref().map_or(0, |size| size.bytes) * 6;
        let scratch = 32768 + wire * 8;
        if budget.admit(wire, scratch).is_none() {
            budget.observe_omitted(false, true);
            omissions.push(json!({"field":"diagnostics.items.related","reason":"byte_limit","omitted_count":json.n().conflicts.len()-index}));
            break;
        }
        let pointer = measured.and_then(|size| {
            pointer_work.debit(size.render_work())?;
            Ok(source.render_pointer(size))
        });
        related.push(source_location(
            *source,
            pointer,
            "diagnostics.items.related.pointer",
            omissions,
        ));
    }
    if reason.definite()
        && let Some(reference) = related.first()
    {
        primary = reference.clone();
    }
    items.push(tag_condition(json!({"code":reason.code(),"severity":if reason.definite(){"error"}else{"incomplete"},"basis":if reason.definite(){json!({"kind":"rfc","citation":"RFC 9880 Section 4.3; W3C CURIE Section 3"})}else{json!({"kind":"operational","id":"effective-composition"})},"message":reason.reason,"primary":primary,"related":related,"arguments":{}}), domain));
}

fn human_explanation(explanation: &Value, output: &mut HumanText) {
    output.push_str(explanation["kind"].as_str().expect("origin kind"));
    if let Some(value) = explanation["default_value"].as_bool() {
        output.push_str(if value { " true" } else { " false" });
    }
    output.push_str(" at ");
    human_location(&explanation["effective"], output);
    for source in explanation["sources"]
        .as_array()
        .expect("origin contributors")
    {
        output.push_str("; ");
        human_location(source, output);
    }
    if let Some(consumer) = explanation.get("consumer") {
        output.push_str("; imported at ");
        human_location(consumer, output);
    }
    output.push('\n');
}

fn retain_alternatives(
    model: &crate::EffectiveModel<'_>,
    owners: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    presentation: PointerPresentation<'_>,
) {
    let PointerPresentation {
        cap: pointer_cap,
        work: pointer_work,
        omissions,
    } = presentation;
    let mut render_work = model.catalogue.limits.work;
    for owner in model.alternative_owners() {
        if pointer_work.debit(1).is_err() {
            omissions.push(json!({"field":"result.alternatives","reason":"not_available","omitted_count":1,"source_id":model.source.get()}));
            break;
        }
        if !owner.n().alternative_scanned {
            continue;
        }
        if owners.len() >= cap {
            omissions.push(json!({"field":"result.alternatives","reason":"count_limit","omitted_count":1,"source_id":model.source.get()}));
            break;
        }
        let location = owner.location();
        let size = location.pointer_size(pointer_cap, pointer_work);
        let wire = 2048 + size.as_ref().map_or(0, |s| s.bytes * 6);
        let scratch = 32768 + wire * 8;
        if budget.admit(wire, scratch).is_none() {
            omissions.push(json!({"field":"result.alternatives","reason":"byte_limit","omitted_count":1,"source_id":model.source.get()}));
            break;
        }
        let pointer = size.and_then(|size| {
            pointer_work.debit(size.render_work())?;
            Ok(location.render_pointer(size))
        });
        let (pointer, reason) = location_pointer(pointer);
        let mut owner_location =
            json!({"kind":"effective","root":model.source.get(),"pointer":pointer});
        if let Some(reason) = reason {
            let omission = json!({"field":"result.alternatives.owner.pointer","reason":reason,"omitted_count":1,"source_id":model.source.get()});
            owner_location["omission"] = omission.clone();
            omissions.push(omission);
        }
        let mut branches = Vec::new();
        let (state, unavailable_reason) = match owner.alternatives() {
            Field::Absent => ("absent", None),
            Field::Unavailable(reason) => ("unavailable", Some(reason)),
            Field::Present(alternatives) => {
                for (id, branch) in alternatives {
                    if branches.len() >= cap {
                        omissions.push(json!({"field":"result.alternatives.branches","reason":"count_limit","omitted_count":1,"source_id":model.source.get()}));
                        break;
                    }
                    let name_bytes = match id {
                        crate::AlternativeId::Choice(name) => name.len(),
                        crate::AlternativeId::Enum(_) => 0,
                    };
                    let wire = 2048 + name_bytes * 6;
                    let scratch = 32768 + wire * 8;
                    if budget.admit(wire, scratch).is_none() {
                        omissions.push(json!({"field":"result.alternatives.branches","reason":"byte_limit","omitted_count":1,"source_id":model.source.get()}));
                        break;
                    }
                    let (kind, name, index) = match id {
                        crate::AlternativeId::Choice(name) => ("choice", Some(name), None),
                        crate::AlternativeId::Enum(index) => ("enum", None, Some(index)),
                    };
                    let mut explanations = Vec::new();
                    let mut unavailable_qualities = Vec::new();
                    let (state, complete, raw, reason) = match branch {
                        Field::Present(data) => {
                            let json = data.json();
                            for quality in json.children() {
                                if pointer_work.debit(1).is_err() {
                                    omissions.push(json!({"field":"result.alternatives.unavailable_qualities","reason":"not_available","omitted_count":1,"source_id":model.source.get()}));
                                    break;
                                }
                                let Some(reason) =
                                    quality.n().unavailable.or(quality.n().meaning_unavailable)
                                else {
                                    continue;
                                };
                                let Some(name) = quality.n().key else {
                                    continue;
                                };
                                let wire = 256 + 6 * (name.len() + reason.reason.len());
                                let scratch = 4096 + wire * 8;
                                if unavailable_qualities.len() >= cap
                                    || budget.admit(wire, scratch).is_none()
                                {
                                    omissions.push(json!({"field":"result.alternatives.unavailable_qualities","reason":"byte_or_count_limit","omitted_count":1,"source_id":model.source.get()}));
                                    break;
                                }
                                unavailable_qualities
                                    .push(json!({"name":name,"reason":reason.reason}));
                            }
                            let payload_cap = budget.remaining() / 6;
                            let raw = budget
                                .with_scratch(|scratch| {
                                    json.render_for_report(payload_cap, &mut render_work, scratch)
                                })
                                .present();
                            let raw = raw.filter(|raw| {
                                budget.admit(raw.len() * 6 + 64, raw.len() * 8).is_some()
                            });
                            let mut rows = model.explanations_from(json.node, render_work);
                            for explanation in rows.by_ref() {
                                if !retain_explanation(
                                    &mut explanations,
                                    budget,
                                    cap.max(1).min(model.catalogue.limits.conditions),
                                    explanation,
                                    true,
                                    PointerPresentation {
                                        cap: pointer_cap,
                                        work: pointer_work,
                                        omissions,
                                    },
                                ) {
                                    omissions.push(json!({"field":"result.alternatives.explanations","reason":if explanations.len() >= cap { "count_limit" } else { "byte_limit" },"omitted_count":1,"source_id":model.source.get()}));
                                    break;
                                }
                            }
                            render_work = rows.work;
                            if rows.limited {
                                omissions.push(json!({"field":"result.alternatives.explanations","reason":"not_available","omitted_count":1,"source_id":model.source.get()}));
                            }
                            let reason = if !json.capability_complete() {
                                Some("alternative meaning or capability incomplete")
                            } else {
                                raw.is_none().then_some("presentation_limit")
                            };
                            (
                                if json.capability_complete() {
                                    "available"
                                } else {
                                    "partial"
                                },
                                json.capability_complete(),
                                raw,
                                reason,
                            )
                        }
                        Field::Unavailable(reason) => ("unavailable", false, None, Some(reason)),
                        Field::Absent => unreachable!("known alternative identity"),
                    };
                    if raw.is_none() && matches!(state, "available" | "partial") {
                        omissions.push(json!({"field":"result.alternatives.json","reason":"not_available","omitted_count":1,"source_id":model.source.get()}));
                    }
                    branches.push(json!({"id":{"kind":kind,"name":name,"index":index},"state":state,"complete":complete,"json_present":raw.is_some(),"raw_json":raw,"unavailable_reason":reason,"unavailable_qualities":unavailable_qualities,"explanations":explanations}));
                }
                ("present", None)
            }
        };
        owners.push(json!({"owner":owner_location,"state":state,"unavailable_reason":unavailable_reason,"branches":branches}));
    }
}

// A witness retains one effective description, up to four operand pairs and one consumer.
// Measurement precedes pointer strings, JSON maps, duplicated omissions and vector growth.
fn retain_proof(
    items: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    finding: &crate::structure::Finding<'_>,
    models: &[crate::EffectiveModel<'_>],
    presentation: PointerPresentation<'_>,
    coverage_owner: Option<&'static str>,
) {
    if !budget.allow_count(items.len(), cap) {
        return;
    }
    let witness = finding.proof.expect("proof witness");
    let Some((root, node)) = finding.effective else {
        unreachable!("proofs require effective descriptions")
    };
    let model = models
        .iter()
        .find(|model| model.source == root)
        .expect("effective proof model");
    let json = crate::EffectiveJson { model, node };
    let PointerPresentation {
        cap: pointer_cap,
        work: pointer_work,
        omissions,
    } = presentation;
    let mut effective = [None; 5];
    effective[0] = Some(json.location());
    let mut source = [None; 5];
    for (index, operand) in witness.operands.iter().enumerate() {
        if let Some(node) = operand {
            let value = crate::EffectiveJson { model, node };
            effective[index + 1] = Some(value.location());
            source[index] = Some(value.source_json());
        }
    }
    source[4] = json.explanation().consumer();
    let mut effective_sizes: [_; 5] = std::array::from_fn(|_| None);
    let mut source_sizes: [_; 5] = std::array::from_fn(|_| None);
    let mut bytes = 0;
    for (index, location) in effective.iter().enumerate() {
        if let Some(location) = location {
            let size = location.pointer_size(pointer_cap, pointer_work);
            bytes += size.as_ref().map_or(0, |size| size.bytes);
            effective_sizes[index] = Some(size);
        }
    }
    for (index, location) in source.iter().enumerate() {
        if let Some(location) = location {
            let size = location.pointer_size(pointer_cap, pointer_work);
            bytes += size.as_ref().map_or(0, |size| size.bytes);
            source_sizes[index] = Some(size);
        }
    }
    let mut lexemes = [None; 4];
    for (index, operand) in witness.operands.iter().enumerate() {
        if !matches!(finding.rule, crate::structure::Rule::ConsistencyExcluded(_))
            && let Some(node) = operand
            && let Field::Present(number) = model.nodes[node].source.number()
            && number.lexeme().len() <= 1024
        {
            let raw = model.nodes[node].source.raw_json();
            bytes += raw.len();
            lexemes[index] = Some(raw);
        }
    }
    bytes += match witness.alternative {
        Some(crate::AlternativeId::Choice(name)) => name.len(),
        _ => 0,
    };
    let wire = 4096 + bytes * 6 + coverage_owner.map_or(0, |id| 64 + id.len() * 6);
    let scratch = 65536 + wire * 8;
    let Some(charge) = budget.admit(wire, scratch) else {
        budget.observe_omitted(false, true);
        return;
    };
    let omissions_before = omissions.len();
    let mut related = Vec::with_capacity(if witness.operands.at(3).is_some() {
        9
    } else {
        7
    });
    let mut primary = Value::Null;
    for (index, location) in effective.into_iter().enumerate() {
        let Some(location) = location else {
            continue;
        };
        let pointer = effective_sizes[index]
            .take()
            .expect("measured effective operand")
            .and_then(|size| {
                pointer_work.debit(size.render_work())?;
                Ok(location.render_pointer(size))
            });
        let (pointer, reason) = location_pointer(pointer);
        let mut value = json!({"kind":"effective","root":root.get(),"pointer":pointer});
        if let Some(reason) = reason {
            let omission = json!({"field":if index == 0 && !matches!(finding.rule, crate::structure::Rule::ConsistencyExcluded(_)) {"diagnostics.items.primary.pointer"} else {"diagnostics.items.related.pointer"},"reason":reason,"omitted_count":1,"source_id":root.get()});
            value["omission"] = omission.clone();
            omissions.push(omission);
        }
        if index == 0 && !matches!(finding.rule, crate::structure::Rule::ConsistencyExcluded(_)) {
            primary = value;
        } else {
            related.push(value);
        }
    }
    for (index, location) in source.into_iter().enumerate() {
        let Some(location) = location else {
            continue;
        };
        let pointer = source_sizes[index]
            .take()
            .expect("measured source operand")
            .and_then(|size| {
                pointer_work.debit(size.render_work())?;
                Ok(location.render_pointer(size))
            });
        let value = source_location(
            location,
            pointer,
            if index == 0 && matches!(finding.rule, crate::structure::Rule::ConsistencyExcluded(_))
            {
                "diagnostics.items.primary.pointer"
            } else {
                "diagnostics.items.related.pointer"
            },
            omissions,
        );
        if index == 0 && matches!(finding.rule, crate::structure::Rule::ConsistencyExcluded(_)) {
            primary = value;
        } else {
            related.push(value);
        }
    }
    let mut arguments = json!({});
    if let crate::structure::Rule::ConsistencyExcluded(default) = finding.rule {
        arguments["candidate"] = json!(if default { "default" } else { "const" });
    }
    if let Some(primitive) = witness.primitive {
        arguments["type"] = json!(crate::proof::primitive_name(primitive));
    }
    if let crate::structure::Rule::NumericCapability { class, capability } = finding.rule {
        arguments["proof_class"] = json!(class.code());
        arguments["reason"] = json!(capability.reason());
    }
    if let crate::structure::Rule::AdvisoryNumericLimit { class, operation } = finding.rule {
        arguments["proof_class"] = json!(class.code());
        arguments["operation"] = json!(operation.name());
        arguments["qualities"] = if matches!(class, crate::proof::ProofClass::PrimitiveConst) {
            json!(["const"])
        } else {
            let quality = |index: usize| {
                model.nodes[witness
                    .operands
                    .at(index)
                    .expect("numeric operation operand")]
                .key
                .expect("numeric operation quality")
            };
            json!([quality(0), quality(1)])
        };
    }
    if let Some(identity) = witness.alternative {
        arguments["alternative"] = match identity {
            crate::AlternativeId::Choice(name) => json!({"kind":"choice","name":name}),
            crate::AlternativeId::Enum(index) => json!({"kind":"enum","index":index}),
        };
    }
    for (index, raw) in lexemes.into_iter().enumerate() {
        if let Some(raw) = raw {
            let key = model.nodes[witness.operands.at(index).expect("numeric operand")]
                .key
                .expect("quality identity");
            arguments[key] = json!(raw);
        }
    }
    let mut condition = json!({"code":finding.rule.code(),"severity":finding.rule.severity(),"basis":{"kind":finding.rule.basis_kind(),"id":finding.rule.basis_id()},"message":finding.rule.message(),"assessment_unit":finding.rule.assessment_unit(),"primary":primary,"related":related,"arguments":arguments});
    if let Some(citation) = finding.rule.citation() {
        condition["basis"]["citation"] = json!(citation);
    }
    if let Some(owner) = coverage_owner {
        condition["coverage_owner"] = json!(owner);
    }
    let size = serde_json::to_string(&condition)
        .expect("bounded proof condition")
        .len()
        + 1
        + omissions[omissions_before..]
            .iter()
            .map(|omission| {
                serde_json::to_string(omission)
                    .expect("bounded proof omission")
                    .len()
                    + 1
            })
            .sum::<usize>();
    assert!(
        size <= wire,
        "proof condition exceeds its precharged wire bound"
    );
    assert!(
        related_capacity_bound(&condition),
        "proof witness exceeds its admitted location capacity"
    );
    budget.settle(charge, size);
    items.push(condition);
}
fn related_capacity_bound(condition: &Value) -> bool {
    let related = condition["related"]
        .as_array()
        .expect("proof related locations");
    related.len() <= 9 && related.capacity() <= 9
}

// Authoring reuses the common immutable envelope without initiating an assessment.
pub(crate) fn author_envelope(catalogue: &Catalogue, generated: SourceId, strict: bool) -> Value {
    let report = Report {
        sample: None,
        catalogue,
        operation: Operation::Inspect {
            document: generated,
            pointer: "",
            view: View::Source,
            json: None,
        },
        scope: vec![generated],
        selected: true,
        outcome: author_outcome(false, 0, 0, 0, strict),
        structure: crate::structure::Structure::default(),
        effective: Vec::new(),
        effective_limit: None,
        dependencies: Vec::new(),
        unavailable_dependencies: Vec::new(),
        modified_sources: 0,
        required: None,
        units: None,
        content: None,
        registry: crate::registry::State::default(),
    };
    report.metadata(false).0
}
pub(crate) fn author_outcome(
    complete: bool,
    errors: u32,
    warnings: u32,
    notices: u32,
    strict: bool,
) -> Outcome {
    Outcome {
        complete,
        errors,
        warnings,
        notices,
        incomplete: u32::from(!complete),
        strict,
    }
}
pub(crate) fn author_diagnostics(wire: Value) -> Diagnostics {
    Diagnostics {
        conditions: wire["items"]
            .as_array()
            .into_iter()
            .flatten()
            .cloned()
            .map(Condition)
            .collect(),
        omitted: wire["omitted"].as_u64().unwrap_or(0) as u32,
        truncated: wire["truncated"].as_bool().unwrap_or(false),
    }
}
pub(crate) fn author_coverage(complete: bool) -> RequestedCoverage {
    RequestedCoverage {
        inventory: None,
        count_unit: "authoring",
        total: Some(1),
        assessed: u32::from(complete),
        not_applicable: 0,
        unassessed: 0,
        undetermined: u32::from(!complete),
    }
}
pub(crate) fn edit_coverage(complete: bool) -> RequestedCoverage {
    let mut coverage = author_coverage(complete);
    coverage.count_unit = "editing";
    coverage
}
pub(crate) fn author_unavailable_check_coverage() -> RequestedCoverage {
    RequestedCoverage {
        inventory: None,
        count_unit: "assessment_area",
        total: None,
        assessed: 0,
        not_applicable: 0,
        unassessed: 0,
        undetermined: 0,
    }
}
impl Report<'_> {
    pub(crate) fn author_reference_errors(&self) -> u32 {
        self.effective
            .iter()
            .map(|model| model.reference_errors)
            .sum()
    }
}

/// Consume checking evidence after its projection, without a second graph or key heap.
pub(crate) struct AuthorReferences<'a>(Vec<crate::EffectiveModel<'a>>);
type ReferenceKey = (u32, u32, u32, &'static str);
fn reference_key(node: &crate::effective::Node<'_>) -> Option<ReferenceKey> {
    let failure = node.unavailable.or(node.meaning_unavailable)?;
    if node.deleted || node.sample_skipped || !failure.definite() || !failure.reported() {
        return None;
    }
    let source = node.needed_ref.unwrap_or(node.source);
    let span = source.span();
    Some((
        source.source_id().get(),
        span.start as u32,
        span.end as u32,
        failure.code(),
    ))
}
impl<'a> Report<'a> {
    pub(crate) fn into_author_references(
        self,
        lookups: usize,
        projection: &mut Projection,
    ) -> Option<AuthorReferences<'a>> {
        if !projection.admit_traversal(self.effective.len()) {
            return None;
        }
        // Each lookup visits at most every graph header and node once. Charge
        // that full bound before reading any node, without a separate key heap.
        let nodes: usize = self.effective.iter().map(|model| model.nodes.len()).sum();
        let work = (nodes + self.effective.len()).checked_mul(lookups)?;
        projection
            .admit_traversal(work)
            .then_some(AuthorReferences(self.effective))
    }
}
impl AuthorReferences<'_> {
    pub(crate) fn take(&mut self, source: SourceId, span: [u32; 2], code: &'static str) -> bool {
        let key = Some((source.get(), span[0], span[1], code));
        for model in &mut self.0 {
            if let Some(node) = model
                .nodes
                .iter_mut()
                .find(|node| reference_key(node) == key)
            {
                // One checking observation pairs with at most one author finding.
                node.unavailable = None;
                node.meaning_unavailable = None;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod content_owner_qualification {
    include!("../tests/fixtures/content-format/owner-qualification.rs");
}

#[cfg(test)]
mod condition_location_tests {
    use super::*;

    #[test]
    fn omitted_primary_is_absent_and_null_primary_is_unavailable() {
        let omitted = Condition(json!({"code":"c","related":[]}));
        assert!(matches!(omitted.primary(), Field::Absent));
        let null = Condition(json!({"code":"c","primary":null,"related":[]}));
        assert!(matches!(null.primary(), Field::Unavailable(_)));
        let request = Condition(
            json!({"code":"c","primary":{"kind":"edit_request","key":"k","pointer":null,"span":null},"related":[]}),
        );
        let Field::Present(ReportLocation::EditRequest(location)) = request.primary() else {
            panic!("edit request primary");
        };
        assert_eq!(location.key(), "k");
        assert!(matches!(location.pointer(), Field::Unavailable(_)));
    }
}

#[cfg(test)]
mod human_color_tests {
    use super::*;

    #[test]
    fn a_human_json_fallback_is_identical_and_unstyled() {
        let catalogue =
            Catalogue::from_inputs(&[crate::SourceInput::supplied("bad", b"[]")]).unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        let plain = report.render_human_with_style(0, HumanStyle::Plain);
        let ansi = report.render_human_with_style(0, HumanStyle::Ansi);
        assert_eq!(ansi, plain);
        assert!(!ansi.contains('\x1b'));
        let wire: Value = serde_json::from_str(&ansi).unwrap();
        assert_eq!(wire["outcome"]["disposition"], "failed");
    }
}
