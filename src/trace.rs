//! Recorded traces, following ADR 0042: Recorded traces with independent entries. A Recorded
//! trace is a JSON Lines file of Trace entries, each naming a Sample selector of one selected
//! document, a value and optionally an Expected admission and a label. The document is
//! composed once, each distinct selector is resolved once, and every entry is decoded and
//! checked with its own Sample-value checking allowance. A live pattern worker may be
//! reused under ADR 0045: One pattern engine per trace or generation request, while each
//! entry retains its own counters. Malformed entries do not stop later entries.
//! The file, line and entry ceilings keep the request finite under ADR 0009: Bounded processing and reporting, and the JSON input
//! policies of ADR 0012: Exact numbers and JSON input policies apply to every line.
mod entry;
mod report;

use crate::allowance::OperationBudget;
use crate::report::WireBasis;
use crate::report::selected::SelectedScope;
use crate::{Admission, Catalogue, Disposition, EffectiveRequest, Severity, Source, SourceId};
use std::ops::Range;

/// The identity of the Trace entry format and the rules that settle each entry's outcome.
pub const TRACE_CONTRACT: &str = "esdieff.trace/1";

/// The longest line of a Recorded trace, apart from its line ending. Generation writes no
/// longer Trace entry, so its JSON Lines output fits this line ceiling.
pub(crate) const LINE_BYTES: usize = 1024 * 1024;
/// The longest label a Trace entry may carry, which generation also respects.
pub(crate) const LABEL_BYTES: usize = 256;
/// The most entries one Recorded trace holds.
const ENTRIES: usize = 100_000;
/// The longest key and label of the trace source, as for any other source.
const KEY_BYTES: usize = 256;
const SOURCE_LABEL_BYTES: usize = 512;

/// Checks a Recorded trace against one model document of a Catalogue.
pub struct TraceRequest<'a> {
    document: SourceId,
    trace: &'a [u8],
    key: &'a str,
    label: Option<&'a str>,
    label_omitted: bool,
}
impl<'a> TraceRequest<'a> {
    /// The largest Recorded trace, in bytes.
    pub const FILE_BYTES: usize = 64 * 1024 * 1024;
    /// Check the Trace entries in `trace` against `document`. The trace becomes a source with
    /// role `sample` and the key `trace`, numbered after the Catalogue's sources.
    pub fn new(document: SourceId, trace: &'a [u8]) -> Self {
        Self {
            document,
            trace,
            key: "trace",
            label: None,
            label_omitted: false,
        }
    }
    /// The trace source's caller key, which must differ from every Catalogue key.
    pub fn key(mut self, key: &'a str) -> Self {
        self.key = key;
        self
    }
    /// A display label for the trace source, such as its path, of at most 512 bytes.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self.label_omitted = false;
        self
    }
    /// Record that the trace source's label was left out, as for a path too long to show.
    pub fn omit_label(mut self) -> Self {
        self.label = None;
        self.label_omitted = true;
        self
    }
}

/// How a Trace entry settled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TraceOutcome {
    /// The value is admitted with nothing else expected, or its definite admission is the
    /// expected one.
    Passed,
    /// The value is rejected with nothing expected, its definite admission differs from the
    /// expected one, or the entry itself is malformed or selects nothing.
    Failed,
    /// The admission, or the entry itself, could not be established.
    Undetermined,
}
impl TraceOutcome {
    const ALL: [Self; 3] = [Self::Passed, Self::Failed, Self::Undetermined];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Undetermined => "undetermined",
        }
    }
}

/// The admissions a checked entry can have, in report order.
const ADMISSIONS: [Admission; 3] = [
    Admission::Admitted,
    Admission::Rejected,
    Admission::Undetermined,
];

/// The position of `value` in `all`.
fn slot<T: PartialEq>(all: [T; 3], value: T) -> usize {
    all.into_iter()
        .position(|listed| listed == value)
        .expect("listed value")
}

/// The checked entries of one Recorded trace. Passing entries are counted, and failed and
/// undetermined entries are kept in line order while they fit the report byte limit.
pub struct TraceResult<'a> {
    catalogue: &'a Catalogue,
    /// The document, the sources its composition used, and what composition and the checks
    /// relied on.
    scope: SelectedScope,
    /// The trace file's identity and metadata, without its content.
    file: Source,
    /// The trace file's length.
    bytes: usize,
    /// Some line could not be decoded completely.
    partial: bool,
    entries: Vec<Entry>,
    /// Failed and undetermined entries left out because they did not fit the report.
    omitted: u32,
    outcomes: [u32; 3],
    admissions: [u32; 3],
}

struct Entry {
    line: u32,
    label: Option<String>,
    selector: Option<(crate::SampleKind, String)>,
    expected: Option<Admission>,
    admission: Option<Admission>,
    outcome: TraceOutcome,
    conditions: Vec<Condition>,
    conditions_omitted: u32,
}

/// One condition behind an entry's outcome, located in the trace file.
struct Condition {
    code: &'static str,
    severity: Severity,
    basis: WireBasis,
    /// What was diagnosed: `sample` for the entry and its value, `operation` for unfinished
    /// work.
    domain: &'static str,
    message: &'static str,
    /// The pointer within the entry's JSON map, or `None` when it cannot be rendered.
    pointer: Option<String>,
    /// The byte range in the trace file.
    span: Option<Range<usize>>,
    argument: Option<(&'static str, &'static str)>,
    related: Vec<Related>,
}

/// A model location related to a condition: the effective location of the constraint and
/// its nearest source.
enum Related {
    Effective {
        root: SourceId,
        pointer: Option<String>,
    },
    Source {
        id: SourceId,
        pointer: Option<String>,
        span: Range<usize>,
    },
}

/// A failed or undetermined Trace entry.
#[derive(Clone, Copy)]
pub struct TraceEntry<'a>(&'a Entry);
impl<'a> TraceEntry<'a> {
    /// The line number, counting from 1.
    pub fn line(self) -> u32 {
        self.0.line
    }
    pub fn label(self) -> Option<&'a str> {
        self.0.label.as_deref()
    }
    /// The Sample selector the entry names: a valid kind and a pointer string of at most
    /// 8192 bytes, whether or not it selects anything.
    pub fn selector(self) -> Option<(crate::SampleKind, &'a str)> {
        self.0
            .selector
            .as_ref()
            .map(|(kind, pointer)| (*kind, pointer.as_str()))
    }
    pub fn expected(self) -> Option<Admission> {
        self.0.expected
    }
    /// What Sample-value checking established, or `None` when the entry was not checked.
    pub fn admission(self) -> Option<Admission> {
        self.0.admission
    }
    pub fn outcome(self) -> TraceOutcome {
        self.0.outcome
    }
    /// The conditions behind the outcome, at most eight.
    pub fn conditions(self) -> impl ExactSizeIterator<Item = TraceCondition<'a>> {
        self.0.conditions.iter().map(TraceCondition)
    }
    /// Conditions beyond the eight kept.
    pub fn conditions_omitted(self) -> u32 {
        self.0.conditions_omitted
    }
}

/// A condition of a Trace entry.
#[derive(Clone, Copy)]
pub struct TraceCondition<'a>(&'a Condition);
impl<'a> TraceCondition<'a> {
    pub fn code(self) -> &'a str {
        self.0.code
    }
    pub fn severity(self) -> Severity {
        self.0.severity
    }
    pub fn message(self) -> &'a str {
        self.0.message
    }
    /// The location's pointer within the entry's JSON map, such as `/value`.
    pub fn pointer(self) -> Option<&'a str> {
        self.0.pointer.as_deref()
    }
    /// The location's byte range in the trace file.
    pub fn span(self) -> Option<Range<usize>> {
        self.0.span.clone()
    }
}

impl<'a> TraceResult<'a> {
    /// The selected model document.
    pub fn document(&self) -> SourceId {
        self.scope.document()
    }
    /// The identity of the trace source in the report.
    pub fn trace_source(&self) -> SourceId {
        self.file.id
    }
    /// The failed and undetermined entries that fit the report, in line order.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = TraceEntry<'_>> {
        self.entries.iter().map(TraceEntry)
    }
    /// Failed and undetermined entries left out because they did not fit the report byte
    /// limit.
    pub fn entries_omitted(&self) -> u32 {
        self.omitted
    }
    /// How many entries settled with `outcome`.
    pub fn count(&self, outcome: TraceOutcome) -> u32 {
        self.outcomes[slot(TraceOutcome::ALL, outcome)]
    }
    /// How many checked entries Sample-value checking settled with `admission`.
    pub fn admissions(&self, admission: Admission) -> u32 {
        self.admissions[slot(ADMISSIONS, admission)]
    }
    /// Failed for any failed entry, otherwise undetermined for any undetermined entry,
    /// otherwise passed.
    pub fn disposition(&self) -> Disposition {
        if self.count(TraceOutcome::Failed) > 0 {
            Disposition::Failed
        } else if self.count(TraceOutcome::Undetermined) > 0 {
            Disposition::Undetermined
        } else {
            Disposition::Passed
        }
    }
    /// The JSON report 1.5 of operation `trace`.
    pub fn to_json(&self) -> String {
        report::to_json(self)
    }
    pub fn to_human(&self) -> String {
        self.to_human_with_style(crate::HumanStyle::Plain)
    }
    pub fn to_human_with_style(&self, style: crate::HumanStyle) -> String {
        report::to_human(self, style)
    }
}

impl Catalogue {
    /// Check every Trace entry of a Recorded trace against one model document. An unknown or
    /// non-model document, a trace over 64 MiB, a line over 1 MiB, more than 100,000 entries
    /// and trace source metadata that cannot be reported are request errors. A malformed
    /// entry fails alone.
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn check_trace(&self, request: TraceRequest<'_>) -> Result<TraceResult<'_>, &'static str> {
        let source = self
            .source(request.document)
            .ok_or("unknown document selector")?;
        if !source.role().is_model() {
            return Err("sample source cannot be a model root");
        }
        if request.trace.len() > TraceRequest::FILE_BYTES {
            return Err("trace exceeds 64 MiB");
        }
        if request.key.len() > KEY_BYTES {
            return Err("trace key exceeds 256 bytes");
        }
        if request
            .label
            .is_some_and(|label| label.len() > SOURCE_LABEL_BYTES)
        {
            return Err("trace label exceeds 512 bytes");
        }
        if self
            .retained_sources()
            .any(|source| source.key == request.key)
        {
            return Err("duplicate source key");
        }
        if self.attempt_count() >= self.limits.documents {
            return Err("trace exceeds the source attempt ceiling");
        }
        let lines = lines(request.trace)?;
        let reserve =
            self.reserve + report::reserve(request.key.len() + request.label.map_or(0, str::len));
        if reserve > self.limits.report_bytes {
            return Err("report cannot retain its mandatory envelope");
        }
        let id = SourceId(self.attempt_count() as u32 + 1);
        let mut file = Source::trace_line(id, Vec::new(), crate::json::Parsed::unavailable());
        file.key = request.key.to_owned();
        file.label = request.label.map(str::to_owned);
        file.label_omitted = request.label_omitted;
        let mut result = TraceResult {
            catalogue: self,
            scope: SelectedScope::new(request.document),
            file,
            bytes: request.trace.len(),
            partial: false,
            entries: Vec::new(),
            omitted: 0,
            outcomes: [0; 3],
            admissions: [0; 3],
        };
        let mut budget = OperationBudget::new(&self.limits);
        let model =
            self.compose_gated(EffectiveRequest::new(request.document), &mut budget, false)?;
        result.scope.composed(&model);
        result.scope.set_aside |= budget.set_aside;
        let mut room = self.limits.report_bytes - reserve;
        let mut selectors = entry::Selectors::new(&model);
        // The trace's own allowance holds the pattern worker that each entry borrows in turn.
        for (index, range) in lines.into_iter().enumerate() {
            let checked = selectors.check(
                entry::Line {
                    id,
                    number: index as u32 + 1,
                    offset: range.start,
                    bytes: &request.trace[range],
                },
                &mut budget.patterns,
            );
            result.partial |= checked.partial;
            result.scope.interpretations |= checked.interpretations;
            result.scope.set_aside |= checked.set_aside;
            let entry = checked.entry;
            result.scope.context |= entry
                .selector
                .as_ref()
                .is_some_and(|(kind, _)| *kind == crate::SampleKind::Context);
            result.outcomes[slot(TraceOutcome::ALL, entry.outcome)] += 1;
            if let Some(admission) = entry.admission {
                result.admissions[slot(ADMISSIONS, admission)] += 1;
            }
            if entry.outcome == TraceOutcome::Passed {
                continue;
            }
            // Entries are listed in line order while they fit, and every later one is counted.
            if result.omitted == 0 {
                let size = report::entry_bytes(&entry, request.document, id);
                if size <= room {
                    room -= size;
                    result.entries.push(entry);
                    continue;
                }
            }
            result.omitted += 1;
        }
        Ok(result)
    }
}

/// The byte range of each line of `trace`, apart from its LF and a CR before it. A last line
/// may omit its LF, and an empty last line is no entry.
fn lines(trace: &[u8]) -> Result<Vec<Range<usize>>, &'static str> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < trace.len() {
        let end = trace[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(trace.len(), |at| start + at);
        let next = end + 1;
        let content = if end > start && trace[end - 1] == b'\r' && end < trace.len() {
            start..end - 1
        } else {
            start..end
        };
        if content.len() > LINE_BYTES {
            return Err("trace line exceeds 1 MiB");
        }
        if content.is_empty() && next >= trace.len() {
            break;
        }
        if lines.len() == ENTRIES {
            return Err("trace exceeds 100,000 entries");
        }
        lines.push(content);
        start = next;
    }
    Ok(lines)
}
