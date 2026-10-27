//! The `generate` report 1.5, its human form and its JSON Lines Trace entries. The report
//! keeps every source, count, the outcome and the disposition when description details do not
//! fit, following ADR 0009: Bounded processing and reporting and ADR 0011: Machine-readable
//! report contracts. Generated values are written as JSON text, so every number keeps the
//! spelling it was generated with.
//!
//! Generation charges each description and each kept value the bytes its largest rendering
//! needs, measured by the same renderers that write the outputs, so every format fits the
//! report byte limit by construction. The JSON report also stops writing description details
//! once they would not fit, and the human report then gives the JSON report instead. The JSON
//! Lines entries are written whole, since their bytes were charged.
use super::targets::Settled;
use super::{Description, GENERATION_CONTRACT, GenerationResult, GenerationState};
use crate::human::{HumanText, Role};
use crate::report::selected;
use crate::{Admission, SampleKind, SourceId};
use serde_json::{Value, json};

/// The report bytes the mandatory envelope needs beyond the Catalogue's own reserve, for a
/// selector pointer of `selector` bytes, each of which may need escaping. It covers the JSON
/// envelope, the human report's opening lines and every omission record.
pub(super) fn reserve(selector: usize) -> usize {
    4096 + 6 * selector
}

/// Room left for the omission records that description details may need.
const OMISSION_SLACK: usize = 512;

/// The widest a human report aligns targets, so a long target pads no other line further.
const TARGET_WIDTH: usize = 40;

/// The indent of a human value line, and the width its admission is padded to, which fits
/// the longest admission, `undetermined`.
const INDENT: &str = "  ";
const ADMISSION_WIDTH: usize = 12;

/// The bytes one description needs in its largest rendering, apart from its values: its JSON
/// record with the longest state and count, or its human heading and omission lines.
pub(super) fn description_bytes(document: SourceId, kind: SampleKind, pointer: &str) -> usize {
    let json = description_open(document, kind, pointer, GenerationState::Unavailable).len()
        + description_close(u32::MAX).len()
        + 1;
    let human = human_heading(kind, pointer, GenerationState::Unavailable).len()
        + human_omitted(u32::MAX, u32::MAX).len();
    json.max(human)
}

/// The bytes one kept value needs in its largest rendering: its JSON record, its Trace entry
/// line or its human line.
pub(super) fn value_bytes(
    kind: SampleKind,
    pointer: &str,
    target: &str,
    value: &str,
    admission: Admission,
    codes: &[&'static str],
) -> usize {
    let mut json = String::new();
    value_json(&mut json, target, value, admission, codes);
    let mut line = String::new();
    trace_line(&mut line, kind, pointer, target, value, admission);
    let human = human_value(target, value, admission, codes, TARGET_WIDTH).len();
    (json.len() + 1).max(line.len()).max(human)
}

pub(super) fn to_json(result: &GenerationResult<'_>) -> String {
    within(result, result.catalogue.limits.report_bytes)
}

/// The JSON report within `limit` bytes. Description details are written in selection order
/// while they fit, and those that do not are counted with a `report_limit` record.
fn within(result: &GenerationResult<'_>, limit: usize) -> String {
    let size = serde_json::to_string(&envelope(result, 0))
        .expect("bounded generate envelope")
        .len();
    let room = limit.saturating_sub(size + OMISSION_SLACK);
    let mut details = String::new();
    let mut kept = 0;
    for description in &result.descriptions {
        let start = details.len();
        if kept > 0 {
            details.push(',');
        }
        description_json(&mut details, result.document(), description);
        if details.len() > room {
            details.truncate(start);
            break;
        }
        kept += 1;
    }
    let dropped = result.descriptions.len() - kept;
    selected::splice(envelope(result, dropped), "descriptions", &details)
}

/// Everything but the description details, with `dropped` descriptions whose details did not
/// fit the report.
fn envelope(result: &GenerationResult<'_>, dropped: usize) -> Value {
    let document = result.document().get();
    let (sources, mut presentation) = selected::sources(result.catalogue, None);
    let mut generation = json!({"contract":GENERATION_CONTRACT,"document":document});
    if let Some((kind, pointer)) = result.selector() {
        generation["selector"] =
            json!({"kind":kind.as_str(),"document":document,"pointer":pointer});
    }
    let complete = result
        .descriptions
        .iter()
        .filter(|description| description.state == GenerationState::Complete)
        .count();
    let unfinished = result.descriptions.len() - complete;
    let unassessed = (result.unselected + result.unpresented) as usize;
    let selected = result.descriptions.len() + unassessed;
    let incomplete = unfinished + unassessed + usize::from(!result.selection_complete);
    let total = result.selection_complete.then_some(selected);
    // Ceilings and the retained-value byte limit leave work undone, while the report limit
    // only leaves details unpresented, so presentation omissions also go to diagnostics.
    let mut omissions = Vec::new();
    let unpresented = result.unpresented as usize + dropped;
    let values =
        |count: fn(&Description) -> u32| -> u32 { result.descriptions.iter().map(count).sum() };
    for (field, reason, count) in [
        (
            "result.descriptions",
            "count_limit",
            result.unselected as usize,
        ),
        ("result.descriptions", "report_limit", unpresented),
        (
            "result.descriptions.values",
            "count_limit",
            values(|d| d.omitted) as usize,
        ),
        (
            "result.descriptions.values",
            "byte_limit",
            values(|d| d.oversized) as usize,
        ),
    ] {
        if count == 0 {
            continue;
        }
        let omission = json!({"field":field,"reason":reason,"omitted_count":count});
        if reason != "count_limit" {
            presentation.push(omission.clone());
        }
        omissions.push(omission);
    }
    let truncated = !presentation.is_empty();
    let reasons = selected::reasons(&presentation);
    json!({
        "schema":"esdieff.report","version":"1.5","operation":"generate",
        "request":{"assessment":null,"inventory":null,"generation":generation,
            "scope":result.scope.scope_json()},
        "sources":sources,
        "baseline":result.scope.baseline_json(),
        "policy":{"restrictions":["A002"],"strict_warnings":false},
        "outcome":{"complete":result.complete(),"errors":0,"warnings":0,"notices":0,"incomplete_checks":incomplete,"strict_warnings":false,"disposition":result.disposition().as_str()},
        "coverage":{"requested":{"inventory":null,"count_unit":"description","total":total,"assessed":complete,"not_applicable":0,"unassessed":unassessed,"undetermined":unfinished},
            "broader":{"inventory":null,"state":"partial","inventory_status":"unavailable"},"rows":[],"rows_omitted":0,"omissions":[]},
        "diagnostics":{"retained":0,"omitted":0,"truncated":truncated,"reasons":reasons,"omissions":presentation,"items":[]},
        "result":{"kind":"generate",
            "summary":{"admitted":result.count(Admission::Admitted),"rejected":result.count(Admission::Rejected),"undetermined":result.count(Admission::Undetermined)},
            "selection":if result.selection_complete { "complete" } else { "partial" },
            "descriptions_omitted":result.unselected as usize + unpresented,
            "omissions":omissions}
    })
}

/// A description's wire record, with each value spliced in as its own JSON text.
fn description_json(output: &mut String, document: SourceId, description: &Description) {
    output.push_str(&description_open(
        document,
        description.kind,
        &description.pointer,
        description.state,
    ));
    for (index, value) in description.values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        value_json(
            output,
            &value.target,
            value.value.to_json(),
            value.admission,
            &value.codes,
        );
    }
    output.push_str(&description_close(
        description.omitted + description.oversized,
    ));
}

/// A description record up to its first value.
fn description_open(
    document: SourceId,
    kind: SampleKind,
    pointer: &str,
    state: GenerationState,
) -> String {
    let document = document.get();
    let selector = json!({"kind":kind.as_str(),"document":document,"pointer":pointer});
    let location = json!({"kind":"effective","root":document,
        "pointer":format!("{pointer}{}", kind.suffix())});
    format!(
        "{{\"selector\":{selector},\"description\":{location},\"state\":\"{}\",\"values\":[",
        state.as_str()
    )
}

/// A description record after its last value.
fn description_close(omitted: u32) -> String {
    format!("],\"values_omitted\":{omitted}}}")
}

/// One value's wire record.
fn value_json(
    output: &mut String,
    target: &str,
    value: &str,
    admission: Admission,
    codes: &[&'static str],
) {
    output.push_str("{\"target\":");
    output.push_str(&json!(target).to_string());
    output.push_str(",\"value\":");
    output.push_str(value);
    output.push_str(&format!(
        ",\"admission\":\"{}\",\"codes\":{}}}",
        admission.as_str(),
        json!(codes)
    ));
}

/// The bytes of a value's Trace entry line, apart from its LF, which a Recorded trace limits.
pub(super) fn line_bytes(kind: SampleKind, pointer: &str, target: &str, value: &str) -> usize {
    let mut line = String::new();
    trace_line(&mut line, kind, pointer, target, value, Admission::Admitted);
    line.len() - 1
}

pub(super) fn to_json_lines(result: &GenerationResult<'_>) -> String {
    let mut lines = String::new();
    for description in &result.descriptions {
        for value in &description.values {
            if value.admission == Admission::Undetermined {
                continue;
            }
            let text = value.value.to_json();
            trace_line(
                &mut lines,
                description.kind,
                &description.pointer,
                &value.target,
                text,
                value.admission,
            );
        }
    }
    lines
}

/// One Trace entry line, ending in LF.
fn trace_line(
    output: &mut String,
    kind: SampleKind,
    pointer: &str,
    target: &str,
    value: &str,
    admission: Admission,
) {
    output.push_str(&format!(
        "{{\"kind\":\"{}\",\"pointer\":{},\"value\":",
        kind.as_str(),
        json!(pointer)
    ));
    output.push_str(value);
    output.push_str(&format!(",\"expected\":\"{}\"", admission.as_str()));
    // A target too long for a Trace entry label is left out rather than shortened.
    if target.len() <= crate::trace::LABEL_BYTES {
        output.push_str(",\"label\":");
        output.push_str(&json!(target).to_string());
    }
    output.push_str("}\n");
}

pub(super) fn to_human(result: &GenerationResult<'_>, style: crate::HumanStyle) -> String {
    human_within(result, style, result.catalogue.limits.report_bytes)
}

/// The human report within `limit` bytes, written one description at a time, or the JSON
/// report as soon as it does not fit.
fn human_within(result: &GenerationResult<'_>, style: crate::HumanStyle, limit: usize) -> String {
    let mut text = HumanText::new(style);
    text.heading("Generated values");
    text.push_str(&format!(" {GENERATION_CONTRACT}: "));
    let disposition = result.disposition();
    text.span(Role::disposition(disposition), disposition.as_str());
    text.push('\n');
    text.heading("Values:");
    for (index, admission) in [
        Admission::Admitted,
        Admission::Rejected,
        Admission::Undetermined,
    ]
    .into_iter()
    .enumerate()
    {
        text.push_str(if index == 0 { " " } else { ", " });
        text.push_str(&format!("{} ", result.count(admission)));
        text.span(admission_role(admission), admission.as_str());
    }
    text.push('\n');
    text.heading("Descriptions:");
    text.push_str(&format!(" {}", result.descriptions.len()));
    if result.unselected > 0 {
        text.push_str(&format!(
            ", {} omitted beyond the ceiling",
            result.unselected
        ));
    }
    if result.unpresented > 0 {
        text.push_str(&format!(
            ", {} omitted for the report size",
            result.unpresented
        ));
    }
    if !result.selection_complete {
        text.push_str(", ");
        text.span(Role::Incomplete, "selection partial");
    }
    text.push('\n');
    for description in &result.descriptions {
        text.push('\n');
        text.heading(&format!(
            "{} {}",
            description.kind.as_str(),
            description.pointer
        ));
        text.push_str(": ");
        let role = match description.state {
            GenerationState::Complete => Role::Passed,
            _ => Role::Incomplete,
        };
        text.span(role, description.state.as_str());
        text.push('\n');
        let width = description
            .values
            .iter()
            .map(|value| value.target.len())
            .max()
            .unwrap_or(0)
            .min(TARGET_WIDTH);
        for value in &description.values {
            human_value_line(&mut text, value, width);
        }
        text.push_str(&human_omitted(description.omitted, description.oversized));
        if text.len() > limit {
            return within(result, limit);
        }
    }
    text.finish(limit)
}

/// A description's human heading, which begins with an empty line.
fn human_heading(kind: SampleKind, pointer: &str, state: GenerationState) -> String {
    format!("\n{} {pointer}: {}\n", kind.as_str(), state.as_str())
}

/// A description's human lines for values omitted beyond the ceiling and for the report size.
fn human_omitted(omitted: u32, oversized: u32) -> String {
    let mut lines = String::new();
    if omitted > 0 {
        lines.push_str(&format!(
            "  {omitted} more values omitted beyond the ceiling\n"
        ));
    }
    if oversized > 0 {
        lines.push_str(&format!(
            "  {oversized} more values omitted for the report size\n"
        ));
    }
    lines
}

/// One value's human line, with its target padded to `width`.
fn human_value(
    target: &str,
    value: &str,
    admission: Admission,
    codes: &[&'static str],
    width: usize,
) -> String {
    let mut line = format!(
        "{INDENT}{:<ADMISSION_WIDTH$} {target:<width$} {value}",
        admission.as_str()
    );
    if !codes.is_empty() {
        line.push_str(&format!(" ({})", codes.join(", ")));
    }
    line.push('\n');
    line
}

fn human_value_line(text: &mut HumanText, value: &Settled, width: usize) {
    let line = human_value(
        &value.target,
        value.value.to_json(),
        value.admission,
        &value.codes,
        width,
    );
    // The admission is styled, and the rest of the line is plain.
    let (admission, rest) = line.split_at(INDENT.len() + ADMISSION_WIDTH);
    text.push_str(INDENT);
    text.span(admission_role(value.admission), &admission[INDENT.len()..]);
    text.push_str(rest);
}

fn admission_role(admission: Admission) -> Role {
    match admission {
        Admission::Admitted => Role::Passed,
        Admission::Rejected => Role::Failed,
        Admission::Undetermined => Role::Incomplete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalogue, GenerationRequest, HumanStyle, SourceInput};

    const MODEL: &[u8] =
        br#"{"sdfProperty":{"a":{"type":"integer","minimum":1},"b":{"type":"boolean"}}}"#;

    fn rendered(test: impl FnOnce(&GenerationResult<'_>)) {
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", MODEL)]).unwrap();
        let document = catalogue.source_id("model").unwrap();
        test(
            &catalogue
                .generate(GenerationRequest::new(document))
                .unwrap(),
        );
    }

    #[test]
    fn details_that_do_not_fit_are_counted_with_a_report_limit_record() {
        rendered(|result| {
            let envelope = serde_json::to_string(&envelope(result, 0)).unwrap().len();
            let mut first = String::new();
            description_json(&mut first, result.document(), &result.descriptions[0]);
            // Room for the first description's details and not the second's.
            let limit = envelope + OMISSION_SLACK + first.len();
            let json = within(result, limit);
            assert!(json.len() <= limit);
            let wire: Value = serde_json::from_str(&json).unwrap();
            let report = &wire["result"];
            assert_eq!(report["descriptions"].as_array().unwrap().len(), 1);
            assert_eq!(report["descriptions_omitted"], 1);
            let omission =
                json!({"field":"result.descriptions","reason":"report_limit","omitted_count":1});
            assert_eq!(report["omissions"], json!([omission]));
            assert_eq!(wire["diagnostics"]["truncated"], true);
            assert_eq!(wire["diagnostics"]["reasons"], json!(["report_limit"]));
            assert_eq!(wire["diagnostics"]["omissions"], json!([omission]));
            // The typed result still holds both descriptions and its counts.
            assert_eq!(
                report["summary"]["rejected"],
                result.count(Admission::Rejected)
            );
        });
    }

    #[test]
    fn a_human_report_that_does_not_fit_gives_the_json_report() {
        rendered(|result| {
            let human = to_human(result, HumanStyle::Plain);
            assert!(human.starts_with("Generated values"));
            let fallback = human_within(result, HumanStyle::Plain, human.len() - 1);
            let wire: Value = serde_json::from_str(&fallback).unwrap();
            assert_eq!(wire["operation"], "generate");
        });
    }
}
