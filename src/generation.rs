//! Generated values for selected effective data descriptions, following ADR 0041: Generated
//! values settled by Sample-value checking. Each value aims at one Generation target of the
//! contract `esdieff.generation/2`, and Sample-value checking settles it against the
//! description it was generated for, so its admission is the only statement made about it.
//! Composition and selection spend the request's allowance, the values of each description
//! are built on an allowance of the description's own, and each value is checked with the
//! ordinary allowance one Sample-value checking request receives, as ADR 0041 states.
//! A live pattern worker may be reused under ADR 0045: One pattern engine per trace or
//! generation request, while each checked value retains its own counters. The value and
//! description ceilings keep the request finite under ADR 0009: Bounded processing and
//! reporting and ADR 0013: Shared workflow and report guarantees.
mod report;
mod targets;

use crate::allowance::OperationBudget;
use crate::budgeted::{self, Exhausted, failure, kept};
use crate::pattern::MatchingWork;
use crate::report::selected::SelectedScope;
use crate::sample::{SELECTOR_BYTES, ValueAllowance, description_root};
use crate::{
    Admission, Catalogue, Disposition, EffectiveJson, EffectiveRequest, JsonKind, SampleKind,
    SourceId,
};

/// The identity of the fixed Generation target list, order and value rules.
pub const GENERATION_CONTRACT: &str = "esdieff.generation/2";

/// The most values kept for one description, after duplicates are removed.
const VALUES: usize = 64;
/// The most descriptions one request generates values for.
const DESCRIPTIONS: usize = 1024;

/// Selects the descriptions of one model document to generate values for: one description
/// chosen by a Sample selector, or every affordance description reachable in the document.
pub struct GenerationRequest<'a> {
    document: SourceId,
    selector: Option<(SampleKind, &'a str)>,
}
impl<'a> GenerationRequest<'a> {
    /// Every Property, Action input and output and Event output reachable from the
    /// document's top-level groupings and affordances, in document order.
    pub fn new(document: SourceId) -> Self {
        Self {
            document,
            selector: None,
        }
    }
    /// Only the description that `kind` and `pointer` select, as `esdieff sample` selects it.
    /// Context metadata requires this explicit selection and is excluded from the default
    /// affordance traversal.
    pub fn select(mut self, kind: SampleKind, pointer: &'a str) -> Self {
        self.selector = Some((kind, pointer));
        self
    }
}

/// How far generation got for one description.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GenerationState {
    /// Every applicable target was generated and every value settled.
    Complete,
    /// The allowance ran out, a ceiling omitted values, or a value stayed undetermined.
    Partial,
    /// The description's meaning is unavailable, so no value was generated.
    Unavailable,
}
impl GenerationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The Generated values of one request, owning every value and its settled admission.
pub struct GenerationResult<'a> {
    catalogue: &'a Catalogue,
    selector: Option<(SampleKind, String)>,
    descriptions: Vec<Description>,
    /// Selected descriptions beyond the per-request ceiling.
    unselected: u32,
    /// Selected descriptions whose details did not fit the report byte limit.
    unpresented: u32,
    /// The report bytes still free for description details and values.
    room: usize,
    /// Whether every grouping and affordance of the document could be enumerated.
    selection_complete: bool,
    /// The document, the sources its composition used, and what composition, generation and
    /// the checks relied on.
    scope: SelectedScope,
}

struct Description {
    kind: SampleKind,
    pointer: String,
    state: GenerationState,
    values: Vec<targets::Settled>,
    /// Distinct values beyond the per-description ceiling.
    omitted: u32,
    /// Distinct values that did not fit the report byte limit.
    oversized: u32,
}

/// One selected description and the values generated for it.
#[derive(Clone, Copy)]
pub struct GeneratedDescription<'a>(&'a Description);
impl<'a> GeneratedDescription<'a> {
    /// The Sample selector kind of the description.
    pub fn kind(self) -> SampleKind {
        self.0.kind
    }
    /// The effective definition pointer of the Sample selector.
    pub fn pointer(self) -> &'a str {
        &self.0.pointer
    }
    pub fn state(self) -> GenerationState {
        self.0.state
    }
    /// The kept values, in target order.
    pub fn values(self) -> impl ExactSizeIterator<Item = GeneratedValue<'a>> {
        self.0.values.iter().map(GeneratedValue)
    }
    /// Distinct values left out, beyond the per-description ceiling or because they did not
    /// fit the report byte limit.
    pub fn values_omitted(self) -> u32 {
        self.0.omitted + self.0.oversized
    }
}

/// A Generated value, the Generation target it aims at and its settled admission.
#[derive(Clone, Copy)]
pub struct GeneratedValue<'a>(&'a targets::Settled);
impl<'a> GeneratedValue<'a> {
    /// The Generation target, such as `minimum`, or `<entry>.<target>` for a nested target.
    pub fn target(self) -> &'a str {
        &self.0.target
    }
    /// The value as JSON text, with each number in the spelling it was generated with.
    pub fn to_json(self) -> &'a str {
        self.0.value.to_json()
    }
    /// What Sample-value checking established, whatever the target suggests.
    pub fn admission(self) -> Admission {
        self.0.admission
    }
    /// The condition codes behind a rejection, at most eight.
    pub fn codes(self) -> impl ExactSizeIterator<Item = &'static str> + 'a {
        self.0.codes.iter().copied()
    }
}

impl<'a> GenerationResult<'a> {
    /// The selected model document.
    pub fn document(&self) -> SourceId {
        self.scope.document()
    }
    /// The Sample selector of an explicitly selected description.
    pub fn selector(&self) -> Option<(SampleKind, &str)> {
        self.selector
            .as_ref()
            .map(|(kind, pointer)| (*kind, pointer.as_str()))
    }
    /// The JSON report 1.5 of operation `generate`.
    pub fn to_json(&self) -> String {
        report::to_json(self)
    }
    /// One Trace entry per admitted or rejected value, each on its own line, with the
    /// settled admission as its expected admission and the target as its label.
    pub fn to_json_lines(&self) -> String {
        report::to_json_lines(self)
    }
    pub fn to_human(&self) -> String {
        self.to_human_with_style(crate::HumanStyle::Plain)
    }
    pub fn to_human_with_style(&self, style: crate::HumanStyle) -> String {
        report::to_human(self, style)
    }
    pub fn descriptions(&self) -> impl ExactSizeIterator<Item = GeneratedDescription<'_>> {
        self.descriptions.iter().map(GeneratedDescription)
    }
    /// Selected descriptions left out, beyond the per-request ceiling or because their details
    /// did not fit the report byte limit. They have no values.
    pub fn descriptions_omitted(&self) -> u32 {
        self.unselected + self.unpresented
    }
    /// Whether every grouping and affordance of the document could be read, so that every
    /// description it reaches was selected.
    pub fn selection_complete(&self) -> bool {
        self.selection_complete
    }
    /// Whether generation completed for every selected description.
    pub fn complete(&self) -> bool {
        self.selection_complete
            && self.unselected == 0
            && self.unpresented == 0
            && self
                .descriptions
                .iter()
                .all(|description| description.state == GenerationState::Complete)
    }
    /// Passed when generation completed for every selected description, otherwise
    /// undetermined. Generation reports no model findings, so it never fails.
    pub fn disposition(&self) -> Disposition {
        if self.complete() {
            Disposition::Passed
        } else {
            Disposition::Undetermined
        }
    }
    /// How many kept values Sample-value checking settled with `admission`.
    pub fn count(&self, admission: Admission) -> u32 {
        self.descriptions
            .iter()
            .flat_map(|description| &description.values)
            .filter(|value| value.admission == admission)
            .count() as u32
    }
}

impl Catalogue {
    /// Generate values for the selected descriptions and settle each with Sample-value
    /// checking. A selector that selects nothing is a request error.
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn generate(
        &self,
        request: GenerationRequest<'_>,
    ) -> Result<GenerationResult<'_>, &'static str> {
        let source = self
            .source(request.document)
            .ok_or("unknown document selector")?;
        if !source.role().is_model() {
            return Err("sample source cannot be a model root");
        }
        if let Some((_, pointer)) = request.selector {
            if pointer.len() > SELECTOR_BYTES {
                return Err("invalid or oversized JSON pointer selector");
            }
            crate::json::validate_pointer(pointer)?;
        }
        let limits = &self.limits;
        let selector = request.selector.map_or(0, |(_, pointer)| pointer.len());
        let reserve = self.reserve + report::reserve(selector);
        if reserve > limits.report_bytes {
            return Err("report cannot retain its mandatory envelope");
        }
        let mut allowance = ValueAllowance::lend(
            OperationBudget::new(limits),
            self.numeric_remaining,
            limits.equality_pairs,
        );
        let mut result = GenerationResult {
            catalogue: self,
            selector: None,
            descriptions: Vec::new(),
            unselected: 0,
            unpresented: 0,
            room: limits.report_bytes - reserve,
            selection_complete: true,
            scope: SelectedScope::new(request.document),
        };
        let model = match request.selector {
            Some((kind, pointer)) => {
                let description = format!("{pointer}{}", kind.suffix());
                let request = EffectiveRequest::new(request.document).pointer(&description);
                self.compose_gated(request, &mut allowance.budget, false)?
            }
            None => self.compose_gated(
                EffectiveRequest::new(request.document),
                &mut allowance.budget,
                false,
            )?,
        };
        result.scope.composed(&model);
        let mut selected = Vec::new();
        match request.selector {
            Some((kind, pointer)) => {
                result.scope.context = kind == SampleKind::Context;
                let json = model.json().present();
                if let Some(json) = json
                    && description_root(json)?.0 != kind
                {
                    return Err("selected definition has the wrong sample kind");
                }
                result.selector = Some((kind, pointer.to_owned()));
                selected.push((kind, pointer.to_owned(), json));
            }
            None => {
                result.selection_complete = match model.json().present() {
                    Some(root) => {
                        affordances(root, &mut allowance.budget, &mut selected).unwrap_or(false)
                    }
                    None => false,
                };
            }
        }
        for (index, (kind, pointer, json)) in selected.into_iter().enumerate() {
            if index >= DESCRIPTIONS {
                result.unselected += 1;
                continue;
            }
            let patterns = &mut allowance.budget.patterns;
            if let Some(description) = settle(kind, pointer, json, &mut result, patterns) {
                result.descriptions.push(description);
            }
        }
        result.scope.set_aside |= allowance.budget.set_aside;
        Ok(result)
    }
}

/// A description a whole-document request selects: its kind, its selector pointer and its
/// effective content, or `None` when that is unavailable.
type Selected<'m> = (SampleKind, String, Option<EffectiveJson<'m>>);

enum Task<'m> {
    /// A grouping, or the document itself, and its pointer.
    Grouping(EffectiveJson<'m>, String),
    /// An affordance of the class its keyword names, and its pointer.
    Affordance(&'m str, EffectiveJson<'m>, String),
}

/// Collect the descriptions of every affordance reachable from the document's top-level
/// groupings and affordances, including nested groupings, in document order, and whether
/// every grouping and affordance could be read. An explicit stack bounds the depth, since
/// composed groupings can nest deeper than their sources, and every visit is charged.
fn affordances<'m>(
    root: EffectiveJson<'m>,
    budget: &mut OperationBudget,
    found: &mut Vec<Selected<'m>>,
) -> Result<bool, Exhausted> {
    let mut complete = true;
    let mut stack = vec![Task::Grouping(root, String::new())];
    while let Some(task) = stack.pop() {
        budgeted::work(budget, 1)?;
        match task {
            Task::Affordance(keyword, definition, pointer) => {
                let kinds: &[SampleKind] = match keyword {
                    "sdfProperty" => &[SampleKind::Property],
                    "sdfAction" => &[SampleKind::ActionInput, SampleKind::ActionOutput],
                    _ => &[SampleKind::EventOutput],
                };
                if failure(definition).is_some() || definition.kind() != JsonKind::Map {
                    // A Property is its own description, while an unreadable Action or
                    // Event hides whether it has any.
                    if keyword == "sdfProperty" {
                        kept(found, (SampleKind::Property, pointer, None), budget)?;
                    } else {
                        complete = false;
                    }
                    continue;
                }
                for &kind in kinds {
                    let description = match kind {
                        SampleKind::Property => Some(definition),
                        SampleKind::ActionInput => {
                            budgeted::member(definition, "sdfInputData", budget)?
                        }
                        _ => budgeted::member(definition, "sdfOutputData", budget)?,
                    };
                    let Some(description) = description else {
                        continue;
                    };
                    let usable = failure(description).is_none()
                        && description_root(description).is_ok_and(|(root, _)| root == kind);
                    let selected = (kind, pointer.clone(), usable.then_some(description));
                    kept(found, selected, budget)?;
                }
            }
            Task::Grouping(grouping, prefix) => {
                if failure(grouping).is_some() || grouping.kind() != JsonKind::Map {
                    complete = false;
                    continue;
                }
                let mut tasks = Vec::new();
                for collection in grouping.children() {
                    let Some(
                        keyword @ ("sdfThing" | "sdfObject" | "sdfProperty" | "sdfAction"
                        | "sdfEvent"),
                    ) = collection.n().key
                    else {
                        continue;
                    };
                    if failure(collection).is_some() || collection.kind() != JsonKind::Map {
                        complete = false;
                        continue;
                    }
                    for definition in collection.children() {
                        let pointer = pointer(&prefix, keyword, definition.n().key.unwrap_or(""));
                        budgeted::work(budget, pointer.len() + 1)?;
                        let task = if matches!(keyword, "sdfThing" | "sdfObject") {
                            Task::Grouping(definition, pointer)
                        } else {
                            Task::Affordance(keyword, definition, pointer)
                        };
                        kept(&mut tasks, task, budget)?;
                    }
                }
                // Reversed, so that the stack visits them in document order.
                while let Some(task) = tasks.pop() {
                    kept(&mut stack, task, budget)?;
                }
            }
        }
    }
    Ok(complete)
}

/// The pointer of the definition `name` in the `keyword` collection under `prefix`.
fn pointer(prefix: &str, keyword: &str, name: &str) -> String {
    let mut pointer = String::with_capacity(prefix.len() + keyword.len() + name.len() + 2);
    pointer.push_str(prefix);
    pointer.push('/');
    pointer.push_str(keyword);
    pointer.push('/');
    crate::json::push_pointer_token(&mut pointer, name);
    pointer
}

/// Generate and settle the values of one selected description, or `None` when its details do
/// not fit the report byte limit. Building its values spends an allowance of the description's
/// own, and each check spends one of its own, each the allowance one Sample-value checking
/// request receives. Request-wide selection and report limits can still omit later
/// descriptions or values. Each value is charged the report bytes it needs, so the values a request retains fit the report.
/// Each allowance can reuse the live pattern worker that `patterns` holds for the request.
fn settle(
    kind: SampleKind,
    pointer: String,
    json: Option<EffectiveJson<'_>>,
    result: &mut GenerationResult<'_>,
    patterns: &mut MatchingWork,
) -> Option<Description> {
    let overhead = report::description_bytes(result.scope.document(), kind, &pointer);
    if overhead > result.room {
        result.unpresented += 1;
        return None;
    }
    result.room -= overhead;
    let Some(json) = json.filter(|json| failure(*json).is_none() && json.kind() == JsonKind::Map)
    else {
        return Some(Description {
            kind,
            pointer,
            state: GenerationState::Unavailable,
            values: Vec::new(),
            omitted: 0,
            oversized: 0,
        });
    };
    let mut allowance = ValueAllowance::new(result.catalogue);
    let size = |target: &str, value: &str, admission: Admission, codes: &[&'static str]| {
        report::value_bytes(kind, &pointer, target, value, admission, codes)
    };
    let line = |target: &str, value: &str| report::line_bytes(kind, &pointer, target, value);
    let room = targets::Room {
        bytes: &mut result.room,
        size: &size,
        line: &line,
    };
    patterns.lend_worker(&mut allowance.budget.patterns);
    let generated = targets::generate(json, VALUES, &mut allowance, room);
    patterns.reclaim_worker(&mut allowance.budget.patterns);
    result.scope.interpretations |= generated.interpretations;
    result.scope.set_aside |= allowance.budget.set_aside;
    result.scope.set_aside |= generated.set_aside;
    let settled = !generated.exhausted
        && generated.omitted == 0
        && generated.oversized == 0
        && generated
            .values
            .iter()
            .all(|value| value.admission != Admission::Undetermined);
    Some(Description {
        kind,
        pointer,
        state: if settled {
            GenerationState::Complete
        } else {
            GenerationState::Partial
        },
        values: generated.values,
        omitted: generated.omitted,
        oversized: generated.oversized,
    })
}

/// Reference examples reuse settlement without recomposing each selected description.
/// The caller supplies the one request's pattern worker and effective views.
pub(crate) struct ReferenceExamples<'a> {
    result: GenerationResult<'a>,
    searched: usize,
}
pub(crate) struct ReferenceExample {
    pub(crate) value: Option<String>,
    pub(crate) status: &'static str,
}
impl<'a> ReferenceExamples<'a> {
    pub(crate) fn new(catalogue: &'a Catalogue, document: SourceId) -> Self {
        Self {
            result: GenerationResult {
                catalogue,
                selector: None,
                descriptions: Vec::new(),
                unselected: 0,
                unpresented: 0,
                room: catalogue.limits.report_bytes,
                selection_complete: true,
                scope: SelectedScope::new(document),
            },
            searched: 0,
        }
    }
    pub(crate) fn search(
        &mut self,
        kind: SampleKind,
        pointer: String,
        json: EffectiveJson<'_>,
        patterns: &mut MatchingWork,
    ) -> ReferenceExample {
        if self.searched >= DESCRIPTIONS {
            return ReferenceExample {
                value: None,
                status: "Examples not searched: request ceiling of 1,024 descriptions reached.",
            };
        }
        self.searched += 1;
        // Candidate retention is transient for one description. Presentation of the selected
        // example is charged independently by the reference renderer.
        self.result.room = self.result.catalogue.limits.report_bytes;
        let description = settle(kind, pointer, Some(json), &mut self.result, patterns);
        let value = description.as_ref().and_then(|description| {
            description
                .values
                .iter()
                .find(|value| {
                    value.admission == Admission::Admitted && value.value.to_json().len() <= 4096
                })
                .map(|value| value.value.to_json().to_owned())
        });
        let status = match description {
            _ if value.is_some() => "Illustrative value admitted by the model",
            Some(description) if description.state == GenerationState::Complete => {
                "No small admitted example was found. This does not establish an impossible description."
            }
            _ => {
                "Example generation unavailable or limited. No small admitted example was established."
            }
        };
        ReferenceExample { value, status }
    }
}
