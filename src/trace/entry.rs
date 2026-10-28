//! One Trace entry: its line decoded on its own, its members read, its Sample selector
//! resolved and its value checked, with the conditions behind its outcome located in the
//! trace file. Decoding follows ADR 0012: Exact numbers and JSON input policies, so a
//! duplicate member name is refused and every number keeps its exact value.
use super::{Condition, Entry, LABEL_BYTES, Related, TRACE_CONTRACT, TraceOutcome};
use crate::allowance::OperationBudget;
use crate::budgeted::failure;
use crate::effective::EffectiveModel;
use crate::finding::Finding;
use crate::json::{self, Issue, JsonView};
use crate::pattern::MatchingWork;
use crate::report::WireBasis;
use crate::role::SetAside;
use crate::sample::{
    SELECTOR_BYTES, ValueAllowance, ValueInterpretations, check_value, description_root,
};
use crate::{
    Admission, DataRole, EffectiveDataDescription, EffectiveJson, Field, JsonKind, SampleKind,
    SampleValueView, Severity, Source, SourceId,
};
use std::collections::HashMap;
use std::ops::Range;

/// The most conditions kept for one entry.
const CONDITIONS: usize = 8;
/// The longest pointer a condition location renders.
const POINTER_BYTES: u32 = 8192;

/// The basis of a condition the trace contract itself establishes.
const TRACE_BASIS: WireBasis = WireBasis {
    kind: "operational",
    citation: None,
    id: Some(TRACE_CONTRACT),
};

/// One line of the trace file.
pub(super) struct Line<'t> {
    pub(super) id: SourceId,
    pub(super) number: u32,
    /// The line's first byte in the trace file.
    pub(super) offset: usize,
    pub(super) bytes: &'t [u8],
}

/// A checked entry and what its check relied on.
pub(super) struct Checked {
    pub(super) entry: Entry,
    /// The line could not be decoded completely.
    pub(super) partial: bool,
    pub(super) interpretations: ValueInterpretations,
    pub(super) set_aside: SetAside,
}

/// Where a Sample selector leads in the composed document.
#[derive(Clone, Copy)]
enum Resolution {
    /// The selected description's node.
    Selected(usize),
    /// The selector names no description of its kind.
    Nothing,
    /// The description could not be established, for this reason.
    Unavailable(&'static str),
}

/// Checks the entries of one Recorded trace against the composed document, resolving each
/// distinct Sample selector once.
pub(super) struct Selectors<'s, 'm> {
    model: &'s EffectiveModel<'m>,
    /// The resolution of each pointer for each kind, in `SampleKind::ALL` order.
    resolved: HashMap<String, [Option<Resolution>; SampleKind::ALL.len()]>,
}

impl<'s, 'm> Selectors<'s, 'm> {
    pub(super) fn new(model: &'s EffectiveModel<'m>) -> Self {
        Self {
            model,
            resolved: HashMap::new(),
        }
    }

    /// Where `kind` and `pointer` lead. A pointer longer than any selector selects nothing
    /// and is not kept.
    fn resolve(&mut self, kind: SampleKind, pointer: &str) -> Resolution {
        if pointer.len() > SELECTOR_BYTES {
            return Resolution::Nothing;
        }
        let slot = SampleKind::ALL
            .into_iter()
            .position(|listed| listed == kind)
            .expect("listed kind");
        if let Some(resolution) = self.resolved.get(pointer).and_then(|kinds| kinds[slot]) {
            return resolution;
        }
        let resolution = self.locate(kind, pointer);
        self.resolved
            .entry(pointer.to_owned())
            .or_insert([None; SampleKind::ALL.len()])[slot] = Some(resolution);
        resolution
    }

    /// Where `kind` and `pointer` lead in the composed document.
    fn locate(&self, kind: SampleKind, pointer: &str) -> Resolution {
        if json::validate_pointer(pointer).is_err() {
            return Resolution::Nothing;
        }
        let path = format!("{pointer}{}", kind.suffix());
        match self.model.explanation_node(&path) {
            Field::Present(node) => {
                let json = EffectiveJson {
                    model: self.model,
                    node,
                };
                if let Some(reason) = failure(json) {
                    return Resolution::Unavailable(reason.reason);
                }
                match description_root(json) {
                    Ok((found, _)) if found == kind => Resolution::Selected(node),
                    _ => Resolution::Nothing,
                }
            }
            Field::Absent => Resolution::Nothing,
            Field::Unavailable(reason) => Resolution::Unavailable(reason),
        }
    }

    /// Decode, read and check one line, lending the entry's allowance the pattern worker that
    /// `patterns` holds for the trace, so entries share one engine.
    pub(super) fn check(&mut self, line: Line<'_>, patterns: &mut MatchingWork) -> Checked {
        let catalogue = self.model.catalogue;
        let limits = &catalogue.limits;
        // The decoding allowances a Sample source would have in this Catalogue.
        let mut decoding = json::Budget::new(limits);
        decoding.nodes = catalogue.parsed_nodes_remaining;
        decoding.allocation = catalogue.parsed_allocation_remaining;
        decoding.work = catalogue.decoding_work_remaining;
        decoding.numeric = catalogue.numeric_remaining;
        let parsed = json::parse_value(line.bytes, true, limits, &mut decoding);
        let source = Source::trace_line(line.id, line.bytes.to_vec(), parsed);
        let mut checked = Checked {
            entry: Entry {
                line: line.number,
                label: None,
                selector: None,
                expected: None,
                admission: None,
                outcome: TraceOutcome::Failed,
                conditions: Vec::new(),
                conditions_omitted: 0,
            },
            partial: source.parsed.issue_count > 0 || source.parsed.root.is_none(),
            interpretations: ValueInterpretations::default(),
            set_aside: SetAside::default(),
        };
        let mut conditions = Conditions {
            kept: Vec::new(),
            omitted: 0,
            offset: line.offset,
        };
        // The check of the value has the allowance one Sample-value checking request
        // receives, with the numeric work its decoding left.
        let mut allowance = ValueAllowance::lend(
            OperationBudget::new(limits),
            decoding.numeric,
            limits.equality_pairs,
        );
        patterns.lend_worker(&mut allowance.budget.patterns);
        let outcome = self.settle(&source, &mut allowance, &mut checked, &mut conditions);
        patterns.reclaim_worker(&mut allowance.budget.patterns);
        checked.entry.outcome = outcome;
        if outcome != TraceOutcome::Passed {
            checked.entry.conditions = conditions.kept;
            checked.entry.conditions_omitted = conditions.omitted;
        }
        checked
    }

    /// The entry's outcome, recording its members, admission and conditions in `checked`.
    fn settle(
        &mut self,
        source: &Source,
        allowance: &mut ValueAllowance,
        checked: &mut Checked,
        conditions: &mut Conditions,
    ) -> TraceOutcome {
        let parsed = &source.parsed;
        for issue in parsed.issues.iter().chain(&parsed.allocation_issue) {
            conditions.push(issue_condition(issue, conditions.offset));
        }
        let shown = parsed.issues.len() + usize::from(parsed.allocation_issue.is_some());
        conditions.omitted += parsed.issue_count.saturating_sub(shown) as u32;
        if parsed.rejected {
            return TraceOutcome::Failed;
        }
        let Some(root) = source.json().present() else {
            // A line that is not UTF-8 or exceeds a decoding limit cannot be read.
            return TraceOutcome::Undetermined;
        };
        let Some(members) = members(root, conditions) else {
            return TraceOutcome::Failed;
        };
        checked.entry.label = members.label.map(str::to_owned);
        checked.entry.expected = members.expected.map(|(admission, _)| admission);
        if let (Some(kind), Some((pointer, _))) = (members.kind, &members.pointer)
            && pointer.len() <= SELECTOR_BYTES
        {
            checked.entry.selector = Some((kind, (*pointer).to_owned()));
        }
        let (Some(kind), Some((pointer, pointer_json)), Some(value)) =
            (members.kind, members.pointer, members.value)
        else {
            return TraceOutcome::Failed;
        };
        if !members.complete {
            return TraceOutcome::Failed;
        }
        let node = match self.resolve(kind, pointer) {
            Resolution::Selected(node) => node,
            Resolution::Nothing => {
                conditions.member(
                    "trace.unselectable",
                    "The Sample selector selects no description of its kind",
                    pointer_json,
                    None,
                );
                return TraceOutcome::Failed;
            }
            Resolution::Unavailable(reason) => {
                checked.entry.admission = Some(Admission::Undetermined);
                conditions.incomplete(
                    "trace.unavailable",
                    "The selected description could not be established",
                    "sample-establishment",
                    pointer_json,
                    Some(("reason", reason)),
                );
                return TraceOutcome::Undetermined;
            }
        };
        let model = self.model;
        let description = EffectiveDataDescription(EffectiveJson { model, node }, DataRole::Full);
        let settled = check_value(description, SampleValueView::new(source, value), allowance)
            .expect("a resolved description is a selectable root");
        checked.interpretations = settled.interpretations;
        checked.set_aside = allowance.budget.set_aside;
        let admission = settled.admission;
        checked.entry.admission = Some(admission);
        let outcome = match (admission, checked.entry.expected) {
            (Admission::Undetermined, _) => TraceOutcome::Undetermined,
            (admission, Some(expected)) if admission == expected => TraceOutcome::Passed,
            (Admission::Admitted, None) => TraceOutcome::Passed,
            _ => TraceOutcome::Failed,
        };
        if outcome == TraceOutcome::Passed {
            return outcome;
        }
        if let Some((expected, json)) = members.expected
            && admission != Admission::Undetermined
            && admission != expected
        {
            conditions.member(
                "trace.expected",
                "The definite admission differs from the Expected admission",
                json,
                None,
            );
        }
        // The exhausted allowance comes first, so the condition limit never hides it, unless a
        // finding already reports the limit itself.
        if settled.exhausted
            && !settled
                .findings
                .iter()
                .any(|finding| finding.code == "operation.limit")
        {
            conditions.incomplete(
                "operation.limit",
                "Sample-value checking exhausted the entry's allowance",
                "sample-work-limit",
                JsonView {
                    source,
                    node: value,
                },
                None,
            );
        }
        // Keep an explanation of unfinished checking within the condition limit, even
        // beside model errors. Some incomplete evaluations have no detailed finding.
        if outcome == TraceOutcome::Undetermined {
            for finding in settled.findings.iter().filter(|f| f.basis.incomplete()) {
                conditions.push(finding_condition(model, source, conditions.offset, finding));
            }
            if !conditions
                .kept
                .iter()
                .any(|condition| condition.severity == Severity::Incomplete)
            {
                conditions.incomplete(
                    "sample.coverage_unavailable",
                    "Sample admission requires checks that could not be completed",
                    "sample-establishment",
                    JsonView {
                        source,
                        node: value,
                    },
                    None,
                );
            }
        }
        for finding in &settled.findings {
            if outcome != TraceOutcome::Undetermined || !finding.basis.incomplete() {
                conditions.push(finding_condition(model, source, conditions.offset, finding));
            }
        }
        outcome
    }
}

/// The conditions of one entry, at most `CONDITIONS` of them kept, with spans moved from the
/// line to the trace file.
struct Conditions {
    kept: Vec<Condition>,
    omitted: u32,
    offset: usize,
}
impl Conditions {
    fn push(&mut self, condition: Condition) {
        if self.kept.len() < CONDITIONS {
            self.kept.push(condition);
        } else {
            self.omitted += 1;
        }
    }
    fn span(&self, span: Range<usize>) -> Range<usize> {
        span.start + self.offset..span.end + self.offset
    }
    /// Unfinished work, located at `at` within the entry.
    fn incomplete(
        &mut self,
        code: &'static str,
        message: &'static str,
        id: &'static str,
        at: JsonView<'_>,
        argument: Option<(&'static str, &'static str)>,
    ) {
        let basis = WireBasis {
            kind: "operational",
            citation: None,
            id: Some(id),
        };
        self.located(code, message, Severity::Incomplete, basis, at, argument);
    }
    /// An error the trace contract establishes about `member`.
    fn member(
        &mut self,
        code: &'static str,
        message: &'static str,
        member: JsonView<'_>,
        name: Option<&'static str>,
    ) {
        let argument = name.map(|name| ("member", name));
        self.located(
            code,
            message,
            Severity::Error,
            TRACE_BASIS,
            member,
            argument,
        );
    }
    fn located(
        &mut self,
        code: &'static str,
        message: &'static str,
        severity: Severity,
        basis: WireBasis,
        at: JsonView<'_>,
        argument: Option<(&'static str, &'static str)>,
    ) {
        let span = self.span(at.span());
        self.push(Condition {
            code,
            severity,
            basis,
            domain: if severity == Severity::Incomplete {
                "operation"
            } else {
                "sample"
            },
            message,
            pointer: at.pointer(POINTER_BYTES).present(),
            span: Some(span),
            argument,
            related: Vec::new(),
        });
    }
}

/// The members of a Trace entry. `complete` is false when a member is unknown, missing or
/// malformed, which the conditions record.
struct Members<'s> {
    kind: Option<SampleKind>,
    pointer: Option<(&'s str, JsonView<'s>)>,
    value: Option<usize>,
    expected: Option<(Admission, JsonView<'s>)>,
    label: Option<&'s str>,
    complete: bool,
}

/// Read the members of the entry map `root`, or `None` when it is not a map.
fn members<'s>(root: JsonView<'s>, conditions: &mut Conditions) -> Option<Members<'s>> {
    if root.kind() != JsonKind::Map {
        conditions.member("trace.entry", "A Trace entry is one JSON map", root, None);
        return None;
    }
    let mut members = Members {
        kind: None,
        pointer: None,
        value: None,
        expected: None,
        label: None,
        complete: true,
    };
    let mut seen = [false; 3];
    for member in root.children() {
        let text = member.text().present();
        let invalid = match member.member_name().present() {
            Some("kind") => {
                seen[0] = true;
                members.kind = text
                    .and_then(SampleKind::from_name)
                    .filter(|kind| *kind != SampleKind::Context);
                members.kind.is_none().then_some("kind")
            }
            Some("pointer") => {
                seen[1] = true;
                members.pointer = text.map(|text| (text, member));
                members.pointer.is_none().then_some("pointer")
            }
            Some("value") => {
                seen[2] = true;
                members.value = Some(member.node);
                None
            }
            Some("expected") => {
                members.expected = [Admission::Admitted, Admission::Rejected]
                    .into_iter()
                    .find(|admission| text == Some(admission.as_str()))
                    .map(|admission| (admission, member));
                members.expected.is_none().then_some("expected")
            }
            Some("label") => {
                members.label = text.filter(|label| label.len() <= LABEL_BYTES);
                members.label.is_none().then_some("label")
            }
            _ => {
                members.complete = false;
                conditions.member(
                    "trace.unknown_member",
                    "A Trace entry has no such member",
                    member,
                    None,
                );
                None
            }
        };
        if let Some(name) = invalid {
            members.complete = false;
            conditions.member(
                "trace.invalid_member",
                "A Trace entry member has an invalid value",
                member,
                Some(name),
            );
        }
    }
    for (name, seen) in ["kind", "pointer", "value"].into_iter().zip(seen) {
        if !seen {
            members.complete = false;
            conditions.member(
                "trace.missing_member",
                "A Trace entry lacks a required member",
                root,
                Some(name),
            );
        }
    }
    Some(members)
}

/// A decoding issue of the line, with the basis the shared report gives a source's decoding
/// issues.
fn issue_condition(issue: &Issue, offset: usize) -> Condition {
    Condition {
        code: issue.code,
        severity: if issue.error {
            Severity::Error
        } else {
            Severity::Incomplete
        },
        basis: WireBasis::decoding(issue.code),
        domain: if issue.code == "operation.limit" {
            "operation"
        } else {
            "sample"
        },
        message: issue.reason,
        pointer: None,
        span: issue
            .span
            .as_ref()
            .map(|span| span.start + offset..span.end + offset),
        argument: None,
        related: Vec::new(),
    }
}

/// A Sample-value checking finding about the entry's value, related to the constraint's
/// effective location and nearest source.
fn finding_condition(
    model: &EffectiveModel<'_>,
    source: &Source,
    offset: usize,
    finding: &Finding,
) -> Condition {
    let sample = JsonView {
        source,
        node: finding.sample_node,
    };
    let span = sample.span();
    let incomplete = finding.basis.incomplete();
    let mut related = Vec::new();
    if let Some(node) = finding.parameter {
        let json = EffectiveJson { model, node };
        let location = json.location();
        related.push(Related::Effective {
            root: location.root_document(),
            pointer: location.pointer(POINTER_BYTES).present(),
        });
        let origin = json.n().source;
        related.push(Related::Source {
            id: origin.source_id(),
            pointer: origin.pointer(POINTER_BYTES).present(),
            span: origin.span(),
        });
    }
    let citation = finding.basis.citation();
    Condition {
        code: finding.code,
        severity: if incomplete {
            Severity::Incomplete
        } else {
            Severity::Error
        },
        basis: WireBasis {
            kind: finding.basis.kind(),
            citation: (!citation.is_empty()).then_some(citation),
            id: finding.basis.id(),
        },
        domain: if incomplete { "operation" } else { "sample" },
        message: finding.message,
        pointer: sample.pointer(POINTER_BYTES).present(),
        span: Some(span.start + offset..span.end + offset),
        argument: Some(("constraint", finding.constraint)),
        related,
    }
}
