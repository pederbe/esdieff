//! The `trace` report 1.5 and its human form. The report keeps every source, count, the
//! outcome and the disposition when entry details do not fit, following ADR 0009: Bounded
//! processing and reporting and ADR 0011: Machine-readable report contracts.
//!
//! Each listed entry is charged the bytes its larger rendering needs, measured by the same
//! renderers that write the outputs, so both formats fit the report byte limit by
//! construction.
use super::{Condition, Entry, Related, TRACE_CONTRACT, TraceOutcome, TraceResult};
use crate::human::{HumanText, Role};
use crate::report::selected;
use crate::{Admission, SourceId};
use serde_json::{Value, json};

/// The report bytes the mandatory envelope needs beyond the Catalogue's own reserve, for trace
/// source metadata of `metadata` bytes, each of which may need escaping. It covers the trace
/// source row, the JSON envelope, the human report's opening lines and the omission records.
pub(super) fn reserve(metadata: usize) -> usize {
    4096 + 6 * metadata
}

/// The bytes one listed entry needs in its larger rendering.
pub(super) fn entry_bytes(entry: &Entry, document: SourceId, trace: SourceId) -> usize {
    let mut json = String::new();
    entry_json(&mut json, entry, document, trace);
    (json.len() + 1).max(human_entry(entry).len())
}

pub(super) fn to_json(result: &TraceResult<'_>) -> String {
    let mut details = String::new();
    for (index, entry) in result.entries.iter().enumerate() {
        if index > 0 {
            details.push(',');
        }
        entry_json(&mut details, entry, result.document(), result.file.id);
    }
    selected::splice(envelope(result), "entries", &details)
}

/// Everything but the entry details.
fn envelope(result: &TraceResult<'_>) -> Value {
    let document = result.document().get();
    let decode = if result.partial { "partial" } else { "usable" };
    let row = crate::report::source_row(&result.file, decode, result.bytes);
    let (sources, mut presentation) = selected::sources(result.catalogue, [row]);
    let passed = result.count(TraceOutcome::Passed);
    let failed = result.count(TraceOutcome::Failed);
    let undetermined = result.count(TraceOutcome::Undetermined);
    let mut omissions = Vec::new();
    if result.omitted > 0 {
        let omission = json!({"field":"result.entries","reason":"report_limit","omitted_count":result.omitted});
        presentation.push(omission.clone());
        omissions.push(omission);
    }
    let conditions: u32 = result.entries.iter().map(|e| e.conditions_omitted).sum();
    if conditions > 0 {
        omissions.push(json!({"field":"result.entries.conditions","reason":"count_limit","omitted_count":conditions}));
    }
    let reasons = selected::reasons(&presentation);
    let admission = |admission| result.admissions(admission);
    json!({
        "schema":"esdieff.report","version":"1.5","operation":"trace",
        "request":{"assessment":null,"inventory":null,
            "trace":{"contract":TRACE_CONTRACT,"document":document,"source":result.file.id.get()},
            "scope":result.scope.scope_json()},
        "sources":sources,
        "baseline":result.scope.baseline_json(),
        "policy":{"restrictions":["A002"],"strict_warnings":false},
        "outcome":{"complete":undetermined == 0,"errors":failed,"warnings":0,"notices":0,"incomplete_checks":undetermined,"strict_warnings":false,"disposition":result.disposition().as_str()},
        "coverage":{"requested":{"inventory":null,"count_unit":"trace_entry","total":passed + failed + undetermined,"assessed":passed + failed,"not_applicable":0,"unassessed":0,"undetermined":undetermined},
            "broader":{"inventory":null,"state":"partial","inventory_status":"unavailable"},"rows":[],"rows_omitted":0,"omissions":[]},
        "diagnostics":{"retained":0,"omitted":0,"truncated":!presentation.is_empty(),"reasons":reasons,"omissions":presentation,"items":[]},
        "result":{"kind":"trace",
            "summary":{"passed":passed,"failed":failed,"undetermined":undetermined,
                "admissions":{"admitted":admission(Admission::Admitted),"rejected":admission(Admission::Rejected),"undetermined":admission(Admission::Undetermined)}},
            "entries_omitted":result.omitted,
            "omissions":omissions}
    })
}

/// One entry's wire record.
fn entry_json(output: &mut String, entry: &Entry, document: SourceId, trace: SourceId) {
    let selector = entry.selector.as_ref().map(
        |(kind, pointer)| json!({"kind":kind.as_str(),"document":document.get(),"pointer":pointer}),
    );
    let conditions: Vec<Value> = entry
        .conditions
        .iter()
        .map(|condition| condition_json(condition, trace))
        .collect();
    let record = json!({"line":entry.line,"label":entry.label,"selector":selector,
        "expected":entry.expected.map(Admission::as_str),"admission":entry.admission.map(Admission::as_str),
        "outcome":entry.outcome.as_str(),"conditions":conditions,"conditions_omitted":entry.conditions_omitted});
    output.push_str(&record.to_string());
}

fn condition_json(condition: &Condition, trace: SourceId) -> Value {
    let related: Vec<Value> = condition
        .related
        .iter()
        .map(|related| match related {
            Related::Effective { root, pointer } => {
                json!({"kind":"effective","root":root.get(),"pointer":pointer})
            }
            Related::Source { id, pointer, span } => {
                json!({"kind":"source","source_id":id.get(),"pointer":pointer,"span":[span.start,span.end]})
            }
        })
        .collect();
    let mut arguments = json!({});
    if let Some((name, value)) = condition.argument {
        arguments[name] = json!(value);
    }
    json!({"domain":condition.domain,"code":condition.code,"severity":condition.severity.as_str(),
        "basis":condition.basis.to_json(),"message":condition.message,
        "primary":{"kind":"sample","source_id":trace.get(),"pointer":condition.pointer,
            "span":condition.span.as_ref().map(|span| [span.start, span.end])},
        "related":related,"arguments":arguments})
}

pub(super) fn to_human(result: &TraceResult<'_>, style: crate::HumanStyle) -> String {
    human_within(result, style, result.catalogue.limits.report_bytes)
}

/// The human report within `limit` bytes, or the JSON report if it would not fit.
fn human_within(result: &TraceResult<'_>, style: crate::HumanStyle, limit: usize) -> String {
    let mut text = HumanText::new(style);
    text.heading("Recorded trace");
    text.push_str(&format!(" {TRACE_CONTRACT}: "));
    let disposition = result.disposition();
    text.span(Role::disposition(disposition), disposition.as_str());
    text.push('\n');
    text.heading("Entries:");
    for (index, outcome) in TraceOutcome::ALL.into_iter().enumerate() {
        text.push_str(if index == 0 { " " } else { ", " });
        text.push_str(&format!("{} ", result.count(outcome)));
        text.span(outcome_role(outcome), outcome.as_str());
    }
    text.push('\n');
    text.heading("Admissions:");
    for (index, admission) in super::ADMISSIONS.into_iter().enumerate() {
        text.push_str(if index == 0 { " " } else { ", " });
        text.push_str(&format!(
            "{} {}",
            result.admissions(admission),
            admission.as_str()
        ));
    }
    text.push('\n');
    for entry in &result.entries {
        let lines = human_entry(entry);
        // The outcome ending the heading line is styled, and the rest is plain.
        let end = lines[1..].find('\n').map_or(lines.len(), |end| end + 1);
        let (heading, rest) = lines.split_at(end);
        let outcome = entry.outcome.as_str();
        text.push_str(&heading[..heading.len() - outcome.len()]);
        text.span(outcome_role(entry.outcome), outcome);
        text.push_str(rest);
    }
    if result.omitted > 0 {
        text.push_str(&format!(
            "\n{} more entries omitted for the report size\n",
            result.omitted
        ));
    }
    if text.len() > limit {
        return to_json(result);
    }
    text.finish(limit)
}

/// One entry's human lines, which begin with an empty line and a heading ending in its
/// outcome.
fn human_entry(entry: &Entry) -> String {
    let mut lines = format!("\nline {}", entry.line);
    if let Some((kind, pointer)) = &entry.selector {
        lines.push_str(&format!(" {} {}", kind.as_str(), json!(pointer)));
    }
    lines.push_str(&format!(": {}", entry.outcome.as_str()));
    if let Some(label) = &entry.label {
        lines.push_str(&format!("\n  label {}", json!(label)));
    }
    let admission = entry
        .admission
        .map(|admission| format!("admission {}", admission.as_str()));
    let expected = entry
        .expected
        .map(|expected| format!("expected {}", expected.as_str()));
    let settled: Vec<String> = admission.into_iter().chain(expected).collect();
    if !settled.is_empty() {
        lines.push_str(&format!("\n  {}", settled.join(", ")));
    }
    for condition in &entry.conditions {
        lines.push_str(&format!(
            "\n  {} {}",
            condition.severity.as_str(),
            condition.code
        ));
        if let Some(pointer) = &condition.pointer {
            lines.push_str(&format!(" at {}", json!(pointer)));
        }
        if let Some(span) = &condition.span {
            lines.push_str(&format!(" bytes {}..{}", span.start, span.end));
        }
        lines.push_str(&format!(": {}", condition.message));
    }
    if entry.conditions_omitted > 0 {
        lines.push_str(&format!(
            "\n  {} more conditions omitted",
            entry.conditions_omitted
        ));
    }
    lines.push('\n');
    lines
}

fn outcome_role(outcome: TraceOutcome) -> Role {
    match outcome {
        TraceOutcome::Passed => Role::Passed,
        TraceOutcome::Failed => Role::Failed,
        TraceOutcome::Undetermined => Role::Incomplete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalogue, HumanStyle, SourceInput, TraceRequest};

    #[test]
    fn a_human_report_that_does_not_fit_gives_the_json_report() {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
            "model",
            br#"{"sdfProperty":{"level":{"type":"integer","maximum":10}}}"#,
        )])
        .unwrap();
        let trace = b"{\"kind\":\"property\",\"pointer\":\"/sdfProperty/level\",\"value\":11}\n";
        let document = catalogue.source_id("model").unwrap();
        let result = catalogue
            .check_trace(TraceRequest::new(document, trace))
            .unwrap();
        let human = to_human(&result, HumanStyle::Plain);
        assert!(human.starts_with("Recorded trace"));
        let fallback = human_within(&result, HumanStyle::Plain, human.len() - 1);
        let wire: Value = serde_json::from_str(&fallback).unwrap();
        assert_eq!(wire["operation"], "trace");
    }
}
