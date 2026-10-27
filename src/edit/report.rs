//! The additive edit 1.2 envelope preserves aggregate operation conclusions.
use super::*;
use crate::human::{HumanText, Role};
use crate::report::projection::{DiagnosticProjection, Projection};
use serde_json::{Value, json};

pub(super) fn wire(result: &EditResult) -> Value {
    let cat = result.catalogue();
    let mut value = crate::report::author_envelope(cat, result.generated, result.request.strict);
    value["version"] = json!("1.2");
    value["operation"] = json!("edit");
    let mut scope = value["request"]["scope"].take();
    scope["dependencies"] = json!([]);
    scope["assessed"] = if result.generated_document().is_some() {
        json!([result.generated.get()])
    } else {
        json!([])
    };
    scope["unavailable_required"] = if result.generation == Generation::Complete {
        json!([])
    } else {
        json!([result.generated.get()])
    };
    let checking = if let Some(check) = &result.checking {
        scope = check.wire["request"]["scope"].clone();
        for field in ["baseline", "policy"] {
            if check.wire[field].is_object() {
                value[field] = check.wire[field].clone();
            }
        }
        json!({"state":"requested","assessment":check.wire["request"]["assessment"],"inventory":check.wire["request"]["inventory"],"scope":scope,"outcome":check.wire["outcome"],"coverage":check.wire["coverage"]})
    } else {
        json!({"state":"not_requested"})
    };
    let summary = json!({"key":result.request.key,"original_source":result.request.selected.get(),"generated_source":result.generated.get(),"edit_count":result.request.edits.len(),"checking":if result.checking.is_some(){"requested"}else{"not_requested"}});
    value["request"] = json!({"assessment":null,"inventory":null,"scope":scope,"editing":summary});
    value["outcome"] = json!({"complete":result.outcome.complete(),"errors":result.outcome.errors(),"warnings":result.outcome.warnings(),"notices":result.outcome.notices(),"incomplete_checks":result.outcome.incomplete_checks(),"strict_warnings":result.outcome.strict_warnings(),"disposition":result.outcome.disposition().as_str()});
    value["coverage"]["requested"] = json!({"inventory":null,"count_unit":"editing","total":1,"assessed":u32::from(result.outcome.complete()),"not_applicable":0,"unassessed":0,"undetermined":u32::from(!result.outcome.complete())});
    value["coverage"]["rows"] = json!([]);
    value["coverage"]["rows_omitted"] = json!(0);
    value["coverage"]["omissions"] = json!([]);
    let copied = result
        .checking
        .as_ref()
        .map(|check| &check.wire["diagnostics"]);
    let allowance = result
        .checking
        .as_ref()
        .and_then(|check| check.projection.clone())
        .unwrap_or_else(|| Projection::new(&cat.limits, cat.reserve));
    let mut projection = DiagnosticProjection::new(copied, allowance);
    if let Some(items) = copied.and_then(|diagnostics| diagnostics["items"].as_array()) {
        for condition in items {
            let mut condition = condition.clone();
            relate_edit_origins(result, &mut condition, &mut projection);
            projection.push(condition);
        }
    }
    for condition in result.conditions.iter() {
        if !projection.allow_item(cat.limits.conditions) {
            continue;
        }
        let original_pointer_bound = result
            .request
            .edits
            .get(condition.index)
            .map_or(0, |edit| edit.owner.len() + edit.quality.len() * 2 + 1);
        let cost = 2048 + (result.request.key.len() + original_pointer_bound) * 6;
        let Some(detail_charge) = projection.admit_item(cost, 32768 + cost * 8) else {
            continue;
        };
        let pointer = format!("/edits/{}", condition.index);
        let presented = projection.present_pointer(&pointer, cat.limits.pointer_bytes);
        if !presented {
            projection.record("diagnostics.primary.pointer", "location_limit", 1);
        }
        let mut related = Vec::new();
        if let crate::Field::Present(target) = result.target(condition.index) {
            let original = target.original_member();
            let span = original.span();
            let original_pointer = projection.lookup(|work, bytes| {
                let mut allowance = crate::json::PointerBudget::new((*work).min(*bytes));
                let size = original
                    .pointer_size(cat.limits.pointer_bytes, &mut allowance)
                    .ok();
                let pointer = size.and_then(|size| {
                    charge(allowance.counter(), size.render_work() + size.bytes * 6 + 1)
                        .then(|| original.render_pointer(size))
                });
                *work = *allowance.counter();
                *bytes = *work;
                pointer
            });
            if original_pointer.is_none() {
                projection.record("diagnostics.related.pointer", "location_limit", 1);
            }
            let mut location = json!({"kind":"source","source_id":result.request.selected.get(),"pointer":original_pointer,"span":[span.start,span.end]});
            if original_pointer.is_none() {
                location["omission"] = json!({"field":"diagnostics.related.pointer","reason":"location_limit","omitted_count":1});
            }
            related.push(location);
        }
        let span = result.request.plan.as_ref().and_then(|plan| {
            projection.lookup(|work, _| {
                plan.json()
                    .present()
                    .and_then(|root| {
                        crate::report::select_with_budget(root, &pointer, work)
                            .ok()
                            .flatten()
                    })
                    .map(|value| {
                        let span = value.span();
                        [span.start, span.end]
                    })
            })
        });
        let mut item = json!({"code":condition.code,"severity":if condition.error {"error"}else{"incomplete"},"basis":{"kind":"operational","id":FIDELITY_CONTRACT},"message":condition.message,"primary":{"kind":"edit_request","key":result.request.key,"pointer":if presented {Some(pointer)}else{None},"span":span},"related":related,"arguments":{},"domain":"operation","assessment_unit":FIDELITY_CONTRACT});
        if !presented {
            item["primary"]["omission"] = json!({"field":"diagnostics.primary.pointer","reason":"location_limit","omitted_count":1});
        }
        if result.request.plan.is_some() && span.is_none() {
            item["primary"]["span_omission"] = json!({"field":"diagnostics.primary.span","reason":"location_limit","omitted_count":1});
            projection.record("diagnostics.primary.span", "location_limit", 1);
        }
        let actual = serde_json::to_string(&item).expect("edit diagnostic").len() + 1;
        projection.settle(detail_charge, actual);
        projection.push(item);
    }
    value["diagnostics"] = projection.finish();
    let source = cat.source(result.generated).expect("generated source");
    value["result"] = json!({"kind":"edit","generation":result.generation.as_str(),"fidelity":{"state":result.fidelity.as_str(),"contract":FIDELITY_CONTRACT},"documents":[{"source_id":result.generated.get(),"state":if source.bytes.is_empty(){"unavailable"}else if source.complete{"available"}else{"partial"},"content_complete":source.complete,"retained_bytes":source.bytes.len(),"faithful":result.fidelity==Fidelity::Established}],"substitution":{"original_source":result.request.selected.get(),"generated_source":result.generated.get()},"active_sources":cat.sources().map(|source|source.id.get()).collect::<Vec<_>>(),"checking":checking,"omissions":[]});
    value
}
pub(super) fn to_json(result: &EditResult) -> String {
    crate::report::projection::encode_author(wire(result), result.catalogue().limits.report_bytes)
}
pub(super) fn to_human(result: &EditResult, style: crate::HumanStyle) -> String {
    let value: Value = serde_json::from_str(&to_json(result)).expect("edit report");
    let mut text = HumanText::new(style);
    text.heading("Edit");
    text.push_str(&format!(" {}: ", result.request.key));
    text.span(
        Role::disposition(result.outcome.disposition()),
        result.outcome.disposition().as_str(),
    );
    text.push('\n');
    text.heading("Generation:");
    text.push(' ');
    text.span(
        if result.generation == Generation::Complete {
            Role::Passed
        } else {
            Role::Incomplete
        },
        result.generation.as_str(),
    );
    text.push('\n');
    text.heading("Fidelity:");
    text.push(' ');
    text.span(
        if result.fidelity == Fidelity::Established {
            Role::Passed
        } else {
            Role::Incomplete
        },
        result.fidelity.as_str(),
    );
    text.push_str(&format!(" ({FIDELITY_CONTRACT})\n"));
    text.heading("Substitution:");
    text.push_str(&format!(
        " source {} with generated source {}\n",
        result.request.selected.get(),
        result.generated.get()
    ));
    for (label, field) in [
        ("Root sources:", "roots"),
        ("Dependency sources:", "dependencies"),
        ("Assessed sources:", "assessed"),
        ("Unavailable required sources:", "unavailable_required"),
    ] {
        text.heading(label);
        text.push_str(&format!(" {}\n", value["request"]["scope"][field]));
    }
    text.heading("Requested editing coverage:");
    text.push_str(&format!(
        " total 1, assessed {}, undetermined {}\n",
        u32::from(result.outcome.complete()),
        u32::from(!result.outcome.complete())
    ));
    text.heading("Checking:");
    if let Some(check) = &result.checking {
        text.push_str(&format!(
            " {}: {}\n",
            value["result"]["checking"]["assessment"]
                .as_str()
                .expect("assessment"),
            check.outcome().disposition().as_str()
        ));
        let coverage = &value["result"]["checking"]["coverage"]["requested"];
        text.heading("Requested model coverage:");
        text.push_str(&format!(
            " total {}, assessed {}, not applicable {}, unassessed {}, undetermined {}\n",
            coverage["total"],
            coverage["assessed"],
            coverage["not_applicable"],
            coverage["unassessed"],
            coverage["undetermined"]
        ));
        if value["result"]["checking"]["coverage"]["requirements"].is_object() {
            let coverage = &value["result"]["checking"]["coverage"]["requirements"];
            text.heading("Requested requirements coverage:");
            text.push_str(&format!(
                " total {}, assessed {}, not applicable {}, unassessed {}, undetermined {}\n",
                coverage["total"],
                coverage["assessed"],
                coverage["not_applicable"],
                coverage["unassessed"],
                coverage["undetermined"]
            ));
        }
    } else {
        text.push_str(" not requested\n");
    }
    text.heading("Broader RFC coverage:");
    text.push_str(" partial\n");
    text.push_str(&format!(
        "Errors: {}, unfinished editing units: {}\n",
        result.outcome.errors(),
        result.outcome.incomplete_checks()
    ));
    for condition in value["diagnostics"]["items"]
        .as_array()
        .expect("edit conditions")
    {
        text.push_str(&format!(
            "{} {}: {}\n",
            condition["severity"].as_str().expect("severity"),
            condition["code"].as_str().expect("code"),
            condition["message"].as_str().expect("message")
        ));
        for location in std::iter::once(&condition["primary"])
            .chain(condition["related"].as_array().expect("related locations"))
        {
            if let Some(pointer) = location["pointer"].as_str() {
                if location["kind"] == "edit_request" {
                    text.push_str(&format!(
                        "  request {} at {pointer}\n",
                        location["key"].as_str().expect("request key")
                    ));
                } else if let Some(source) =
                    location.get("source_id").or_else(|| location.get("root"))
                {
                    text.push_str(&format!(
                        "  {} {source} at {pointer}\n",
                        location["kind"].as_str().expect("location kind")
                    ));
                }
            }
        }
    }
    if value["diagnostics"]["truncated"] == true {
        text.push_str(&format!(
            "Diagnostic details omitted: {}\n",
            value["diagnostics"]["omitted"]
        ));
    }
    text.finish(result.catalogue().limits.report_bytes)
}

struct Origin {
    original: String,
    request: String,
    original_span: [usize; 2],
    request_span: Option<[usize; 2]>,
    span_limited: bool,
}
fn edit_origin(
    result: &EditResult,
    pointer: &str,
    work: &mut usize,
    bytes: &mut usize,
) -> crate::Field<Origin> {
    for (index, edit) in result.request.edits.iter().enumerate() {
        if !charge(
            work,
            edit.owner.len() + edit.quality.len() * 2 + pointer.len() + 1,
        ) {
            return crate::Field::Unavailable("edit request origin work limit");
        }
        let encoded_quality = edit.quality.bytes().flat_map(|byte| {
            let (encoded, len) = match byte {
                b'~' => (*b"~0", 2),
                b'/' => (*b"~1", 2),
                byte => ([byte, 0], 1),
            };
            encoded.into_iter().take(len)
        });
        let member_len = edit.owner.len()
            + 1
            + edit.quality.len()
            + edit
                .quality
                .bytes()
                .filter(|byte| matches!(byte, b'~' | b'/'))
                .count();
        let matches = if edit.operation == EditOperation::Remove {
            pointer == edit.owner
        } else {
            pointer.len() >= member_len
                && edit
                    .owner
                    .bytes()
                    .chain(std::iter::once(b'/'))
                    .chain(encoded_quality)
                    .eq(pointer.bytes().take(member_len))
                && (pointer.len() == member_len || pointer.as_bytes()[member_len] == b'/')
        };
        if !matches {
            continue;
        }
        let Some(target) = result.targets.get(index).and_then(|target| *target) else {
            return crate::Field::Unavailable("original target unavailable");
        };
        let suffix = if edit.operation == EditOperation::Remove {
            ""
        } else {
            &pointer[member_len..]
        };
        let request_len = 32 + suffix.len();
        if !charge(bytes, member_len + request_len + 2)
            || !charge(work, member_len + request_len + 2)
            || member_len > result.catalogue().limits.pointer_bytes
            || request_len > result.catalogue().limits.pointer_bytes
        {
            return crate::Field::Unavailable("edit origin pointer limit");
        }
        let original = format!(
            "{}/{}",
            edit.owner,
            edit.quality.replace('~', "~0").replace('/', "~1")
        );
        let request = if edit.operation == EditOperation::Remove {
            format!("/edits/{index}")
        } else {
            format!("/edits/{index}/value{suffix}")
        };
        let source = JsonView {
            source: result.original_source(),
            node: target.member,
        };
        let span = source.span();
        let (request_span, span_limited) = if let Some(plan) = &result.request.plan {
            match plan.json().present().and_then(|root| {
                crate::report::select_with_budget(root, &request, work)
                    .ok()
                    .flatten()
            }) {
                Some(value) => {
                    let span = value.span();
                    (Some([span.start, span.end]), false)
                }
                None => (None, true),
            }
        } else {
            (None, false)
        };
        return crate::Field::Present(Origin {
            original,
            request,
            original_span: [span.start, span.end],
            request_span,
            span_limited,
        });
    }
    crate::Field::Absent
}
fn relate_edit_origins(
    result: &EditResult,
    condition: &mut Value,
    projection: &mut DiagnosticProjection,
) {
    let existing = condition["related"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut added = Vec::new();
    for location in std::iter::once(&condition["primary"]).chain(existing) {
        if location["kind"] != "source"
            || location["source_id"].as_u64() != Some(u64::from(result.generated.get()))
        {
            continue;
        }
        let Some(pointer) = location["pointer"].as_str() else {
            projection.record("diagnostics.related.edit_origin", "location_limit", 1);
            continue;
        };
        let original_bound = result
            .request
            .edits
            .iter()
            .map(|edit| edit.owner.len() + edit.quality.len() * 2 + 1)
            .max()
            .unwrap_or(0);
        let wire = 1024 + (result.request.key.len() + original_bound + pointer.len() + 32) * 6;
        let Some(charge) = projection.admit(wire, 16384 + wire * 8) else {
            projection.record("diagnostics.related.edit_origin", "byte_limit", 1);
            continue;
        };
        let origin = projection.lookup(|work, bytes| edit_origin(result, pointer, work, bytes));
        let origin = match origin {
            crate::Field::Present(origin) => origin,
            crate::Field::Absent => {
                projection.settle(charge, 0);
                continue;
            }
            crate::Field::Unavailable(reason) => {
                projection.settle(charge, 0);
                projection.record(
                    "diagnostics.related.edit_origin",
                    if reason.contains("limit") {
                        "location_limit"
                    } else {
                        "not_available"
                    },
                    1,
                );
                continue;
            }
        };
        let original = json!({"kind":"source","source_id":result.request.selected.get(),"pointer":origin.original,"span":origin.original_span});
        let mut request = json!({"kind":"edit_request","key":result.request.key,"pointer":origin.request,"span":origin.request_span});
        if origin.span_limited {
            request["omission"] = json!({"field":"diagnostics.related.edit_request.span","reason":"location_limit","omitted_count":1});
            projection.record("diagnostics.related.edit_request.span", "location_limit", 1);
        }
        let mut cost = 0;
        for location in [original, request] {
            if !existing.contains(&location) && !added.contains(&location) {
                cost += serde_json::to_vec(&location)
                    .expect("bounded edit origin")
                    .len()
                    + 1;
                added.push(location);
            }
        }
        projection.settle(charge, cost);
    }
    if let Some(related) = condition["related"].as_array_mut() {
        related.extend(added);
    }
}
