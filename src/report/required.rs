//! The fixed relationship summary survives zero optional DTO allowance.
//! Follows ADR 0026: Required declaration relationships and source evidence.
use super::*;
use crate::Class;
use crate::effective::{Owner, ViewNode};
use std::fmt::Write;

#[derive(Default)]
pub(super) struct Presentation {
    pub(super) owners: Vec<Value>,
    pub(super) omitted: u32,
    pub(super) reason: Option<&'static str>,
    pub(super) locations: LocationLimits,
}
#[derive(Default)]
pub(super) struct LocationLimits {
    pub(super) limited: bool,
    pub(super) unavailable: bool,
}
impl LocationLimits {
    fn observe(&mut self, reason: Option<&str>) {
        match reason {
            Some("location_limit") => self.limited = true,
            Some("not_available") => self.unavailable = true,
            _ => {}
        }
    }
}

fn count(value: usize) -> u32 {
    u32::try_from(value).expect("operation-bound Count")
}
fn retained(collection: &crate::effective::Collection<'_>) -> (u32, u32) {
    (collection.references_retained, collection.targets_retained)
}
#[derive(Clone, Copy)]
struct Counts {
    owners_known: u32,
    owners_retained: u32,
    references_known: u32,
    references_retained: u32,
    targets_known: u32,
    targets_retained: u32,
    owners_omitted: u32,
}
fn counts(collection: &crate::effective::Collection<'_>, presentation: &Presentation) -> Counts {
    let (references, targets) = retained(collection);
    Counts {
        owners_known: collection.owners_known,
        owners_retained: count(collection.owners.len()),
        references_known: collection.references_known,
        references_retained: references,
        targets_known: collection.targets_known,
        targets_retained: targets,
        owners_omitted: presentation.omitted,
    }
}
fn quoted(output: &mut impl Write, text: &str) {
    output.write_char('"').expect("String write");
    for byte in text.bytes() {
        assert!((32..=126).contains(&byte), "fixed printable reason");
        if matches!(byte, b'"' | b'\\') {
            output.write_char('\\').expect("String write");
        }
        output.write_char(char::from(byte)).expect("String write");
    }
    output.write_char('"').expect("String write");
}
fn prefix_json(
    output: &mut String,
    state: &str,
    complete: bool,
    reason: Option<&str>,
    counts: Counts,
) {
    output.push_str(",\"required_declarations\":{\"state\":");
    quoted(output, state);
    write!(output, ",\"complete\":{complete},\"unavailable_reason\":").expect("String write");
    if let Some(reason) = reason {
        assert!(reason.len() <= 96);
        quoted(output, reason);
    } else {
        output.push_str("null");
    }
    write!(output, ",\"owners_known\":{},\"owners_retained\":{},\"references_known\":{},\"references_retained\":{},\"targets_known\":{},\"targets_retained\":{},\"owners_omitted\":{},\"traversal_complete\":{complete},\"owners\":[", counts.owners_known, counts.owners_retained, counts.references_known, counts.references_retained, counts.targets_known, counts.targets_retained, counts.owners_omitted).expect("String write");
}
fn omission_json(output: &mut String, omitted: u32, reason: &str) {
    write!(output, ",\"omissions\":[{{\"field\":\"result.required_declarations.owners\",\"reason\":\"{reason}\",\"omitted_count\":{omitted}}}]").expect("String write");
}
fn prefix_human(
    output: &mut HumanText,
    state: &str,
    complete: bool,
    reason: Option<&str>,
    counts: Counts,
    omission: Option<&str>,
) {
    let start = output.len();
    output.heading("Required declarations:");
    output.push(' ');
    output.span(
        match state {
            "invalid" => Some(Role::Failed),
            "partial" | "unavailable" => Some(Role::Incomplete),
            _ => None,
        },
        state,
    );
    output.push_str("; complete: ");
    output.span(
        (!complete).then_some(Role::Incomplete),
        if complete { "true" } else { "false" },
    );
    output.push_str("; traversal: ");
    output.span(
        (!complete).then_some(Role::Incomplete),
        if complete { "true" } else { "false" },
    );
    output.push('\n');
    output.heading("Owners:");
    writeln!(
        output,
        " known {}; retained {}; omitted {}",
        counts.owners_known, counts.owners_retained, counts.owners_omitted
    )
    .expect("String write");
    output.heading("References:");
    writeln!(
        output,
        " known {}; retained {}",
        counts.references_known, counts.references_retained
    )
    .expect("String write");
    output.heading("Targets:");
    writeln!(
        output,
        " known {}; retained {}",
        counts.targets_known, counts.targets_retained
    )
    .expect("String write");
    output.heading("Unavailable reason:");
    output.push(' ');
    if let Some(reason) = reason {
        assert!(reason.len() <= 96);
        quoted(output, reason);
    } else {
        output.push_str("none");
    }
    output.push('\n');
    if let Some(reason) = omission {
        output.heading("Owner details omitted:");
        writeln!(output, " {} ({reason})", counts.owners_omitted).expect("String write");
    }
    assert!(output.len() - start <= 512, "fixed required human prefix");
}

impl Report<'_> {
    pub(super) fn write_required_json(&self, output: &mut String, presentation: &Presentation) {
        let collection = self.required.as_ref().expect("requested summary");
        prefix_json(
            output,
            collection.state.as_str(),
            collection.complete,
            collection.reason,
            counts(collection, presentation),
        );
        for (index, owner) in presentation.owners.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write!(output, "{owner}").expect("String write");
        }
        output.push(']');
        if let Some(reason) = presentation.reason {
            omission_json(output, presentation.omitted, reason);
        }
        output.push('}');
    }
    pub(super) fn write_required_human(&self, output: &mut HumanText, presentation: &Presentation) {
        let Some(collection) = &self.required else {
            return;
        };
        prefix_human(
            output,
            collection.state.as_str(),
            collection.complete,
            collection.reason,
            counts(collection, presentation),
            presentation.reason,
        );
    }
    pub(super) fn required_presentation(
        &self,
        budget: &mut DetailBudget,
        cap: usize,
        work: &mut crate::json::PointerBudget,
        no_optional_report: bool,
    ) -> Presentation {
        let mut out = Presentation::default();
        let Some(collection) = &self.required else {
            return out;
        };
        for owner in &collection.owners {
            let fail = if out.owners.len() >= cap {
                Some("count_limit")
            } else {
                let allowance = self.required_owner_wire(owner, work).and_then(|wire| {
                    wire.checked_mul(8)
                        .and_then(|bytes| bytes.checked_add(32768))
                        .map(|scratch| (wire, scratch))
                });
                let admitted = allowance.and_then(|(wire, scratch)| budget.admit(wire, scratch));
                if let Some(charge) = admitted {
                    let row = self.required_owner_dto(owner, work, &mut out.locations);
                    let actual = serde_json::to_string(&row)
                        .expect("bounded required owner")
                        .len()
                        + 64;
                    budget.settle(charge, actual);
                    out.owners.push(row);
                    None
                } else {
                    Some(if no_optional_report {
                        "report_limit"
                    } else {
                        "byte_limit"
                    })
                }
            };
            if let Some(reason) = fail {
                out.omitted = count(collection.owners.len() - out.owners.len());
                out.reason = Some(reason);
                break;
            }
        }
        out
    }
    fn required_owner_wire(
        &self,
        owner: &Owner<'_>,
        work: &mut crate::json::PointerBudget,
    ) -> Option<usize> {
        let mut wire = 1024usize
            .checked_add(self.required_source_wire(owner.source, work))?
            .checked_add(self.required_view_wire(owner.json, work))?;
        for row in &owner.references {
            wire = wire
                .checked_add(2048)?
                .checked_add(self.required_source_wire(row.source, work))?
                .checked_add(self.required_view_wire(row.json, work))?;
            wire = wire.checked_add(match row.form {
                Some(
                    crate::RequiredReferenceForm::DirectName(text)
                    | crate::RequiredReferenceForm::NameReference(text),
                ) => text.len().checked_mul(6)?,
                _ => 0,
            })?;
            if let ViewNode::Effective(node) = row.json
                && let Some(model) = self.effective.first()
                && let Some(consumer) = model.nodes[node].consumer
            {
                wire = wire.checked_add(self.required_source_wire(consumer, work))?;
            }
            for target in &row.targets {
                wire = wire
                    .checked_add(3072)?
                    .checked_add(target.name.len().checked_mul(6)?)?;
                if let Some(source) = target.source {
                    wire = wire.checked_add(self.required_source_wire(source, work))?;
                }
                if let Some(node) = target.effective {
                    wire =
                        wire.checked_add(self.required_view_wire(ViewNode::Effective(node), work))?;
                }
            }
            for source in &row.related {
                wire = wire
                    .checked_add(512)?
                    .checked_add(self.required_source_wire(*source, work))?;
            }
        }
        Some(wire)
    }
    fn required_source_wire(
        &self,
        json: JsonView<'_>,
        work: &mut crate::json::PointerBudget,
    ) -> usize {
        512 + 6 * json
            .pointer_size(self.catalogue.limits.pointer_bytes, work)
            .map_or(0, |size| size.bytes)
    }
    fn required_view_wire(
        &self,
        node: ViewNode<'_>,
        work: &mut crate::json::PointerBudget,
    ) -> usize {
        match (node, self.effective.first()) {
            (ViewNode::Effective(node), Some(model)) => {
                512 + 6 * crate::EffectiveJson { model, node }
                    .location()
                    .pointer_size(self.catalogue.limits.pointer_bytes, work)
                    .map_or(0, |size| size.bytes)
            }
            _ => 0,
        }
    }
    fn required_source_location(
        &self,
        json: JsonView<'_>,
        field: &'static str,
        work: &mut crate::json::PointerBudget,
        limits: &mut LocationLimits,
    ) -> Value {
        let pointer = json
            .pointer_size(self.catalogue.limits.pointer_bytes, work)
            .and_then(|size| {
                work.debit(size.render_work())?;
                Ok(json.render_pointer(size))
            });
        let (pointer, reason) = location_pointer(pointer);
        limits.observe(reason);
        let span = json.span();
        let mut location = json!({"kind":"source","source_id":json.source_id().get(),"pointer":pointer,"span":[span.start,span.end]});
        required_location_omission(&mut location, field, reason, Some(json.source_id()));
        location
    }
    fn required_effective_location(
        &self,
        node: ViewNode<'_>,
        field: &'static str,
        work: &mut crate::json::PointerBudget,
        limits: &mut LocationLimits,
    ) -> Value {
        match (node, self.effective.first()) {
            (ViewNode::Effective(node), Some(model)) => {
                let json = crate::EffectiveJson { model, node };
                let location = json.location();
                let pointer = location
                    .pointer_size(self.catalogue.limits.pointer_bytes, work)
                    .and_then(|size| {
                        work.debit(size.render_work())?;
                        Ok(location.render_pointer(size))
                    });
                let (pointer, reason) = location_pointer(pointer);
                limits.observe(reason);
                let mut location =
                    json!({"kind":"effective","root":model.source.get(),"pointer":pointer});
                required_location_omission(&mut location, field, reason, None);
                location
            }
            _ => Value::Null,
        }
    }
    fn required_owner_dto(
        &self,
        owner: &Owner<'_>,
        work: &mut crate::json::PointerBudget,
        limits: &mut LocationLimits,
    ) -> Value {
        let view = crate::RequiredDeclarations {
            collection: self.required.as_ref().expect("collection"),
            model: self.effective.first(),
        };
        let mut references = Vec::new();
        for row in &owner.references {
            let mut targets = Vec::new();
            for target in &row.targets {
                let capability = target.capability(self.effective.first()).as_str();
                targets.push(json!({"class":target.class.keyword(),"given_name":target.name,"complete":target.complete,"source_definition":target.source.map(|source|self.required_source_location(source,"result.required_declarations.owners.references.targets.source_definition.pointer",work,limits)),"effective_definition":target.effective.map(|node|self.required_effective_location(ViewNode::Effective(node),"result.required_declarations.owners.references.targets.effective_definition.pointer",work,limits)),"capability":capability}));
            }
            let (form, text) = match row.form {
                Some(crate::RequiredReferenceForm::Carrier) => (Some("carrier"), None),
                Some(crate::RequiredReferenceForm::DirectName(text)) => {
                    (Some("direct_name"), Some(text))
                }
                Some(crate::RequiredReferenceForm::NameReference(text)) => {
                    (Some("name_reference"), Some(text))
                }
                None => (None, None),
            };
            let consumer = match (row.json, view.model) {
                (ViewNode::Effective(node), Some(model)) => {
                    model.nodes[node].consumer.map(|source| {
                        self.required_source_location(
                            source,
                            "result.required_declarations.owners.references.consumer.pointer",
                            work,
                            limits,
                        )
                    })
                }
                _ => None,
            };
            let related: Vec<_> = row
                .related
                .iter()
                .map(|source| {
                    self.required_source_location(
                        *source,
                        "result.required_declarations.owners.references.related.pointer",
                        work,
                        limits,
                    )
                })
                .collect();
            references.push(json!({"occurrence":row.occurrence,"form":form,"text":text,"status":row.status.as_str(),"complete":row.complete,"source":self.required_source_location(row.source,"result.required_declarations.owners.references.source.pointer",work,limits),"effective":self.required_effective_location(row.json,"result.required_declarations.owners.references.effective.pointer",work,limits),"consumer":consumer,"targets":targets,"related":related}));
        }
        json!({"class":owner.class.map(Class::keyword),"state":owner.state.as_str(),"complete":owner.complete,"source_owner":self.required_source_location(owner.source,"result.required_declarations.owners.source_owner.pointer",work,limits),"effective_owner":self.required_effective_location(owner.json,"result.required_declarations.owners.effective_owner.pointer",work,limits),"references":references})
    }
    pub(super) fn retain_required_conditions(
        &self,
        items: &mut Vec<Value>,
        budget: &mut DetailBudget,
        cap: usize,
        total: &mut usize,
    ) {
        let Some(collection) = &self.required else {
            return;
        };
        *total += collection.conditions as usize;
        for owner in &collection.owners {
            if owner.state == crate::RequirednessState::Invalid && owner.references.is_empty() {
                let source = match (owner.syntax, self.effective.first()) {
                    (Some(ViewNode::Source(source)), _) => source,
                    (Some(ViewNode::Effective(node)), Some(model)) => model.nodes[node].source,
                    _ => owner.source,
                };
                required_condition(
                    items,
                    budget,
                    cap,
                    source,
                    (
                        "required.syntax",
                        "error",
                        "rfc",
                        "sdfRequired must be an array of name references or carrier markers",
                    ),
                );
            }
            for row in &owner.references {
                if row.error_already_counted {
                    continue;
                }
                let condition = match row.status {
                    crate::RequiredReferenceStatus::UndefinedContext => (
                        "context.required_undefined",
                        "incomplete",
                        "interpretation",
                        "The Context draft does not define Context use in sdfRequired",
                    ),
                    crate::RequiredReferenceStatus::InvalidSyntax => (
                        "required.syntax",
                        "error",
                        "rfc",
                        "Required declaration syntax is invalid",
                    ),
                    crate::RequiredReferenceStatus::WrongTarget => (
                        "required.target_role",
                        "error",
                        "rfc",
                        "Required target is not a declaration at this position",
                    ),
                    crate::RequiredReferenceStatus::Missing => (
                        "required.target_missing",
                        "notice",
                        "advice",
                        "Required target was not found in the established scope",
                    ),
                    crate::RequiredReferenceStatus::Resolved => continue,
                    _ => (
                        "required.unavailable",
                        "incomplete",
                        "interpretation",
                        "Required declaration identity or context is unavailable",
                    ),
                };
                required_condition(items, budget, cap, row.source, condition);
            }
        }
    }
}
fn required_location_omission(
    location: &mut Value,
    field: &'static str,
    reason: Option<&'static str>,
    source: Option<SourceId>,
) {
    if let Some(reason) = reason {
        let mut omission = json!({"field":field,"reason":reason,"omitted_count":1});
        if let Some(source) = source {
            omission["source_id"] = json!(source.get());
        }
        location["omission"] = omission;
    }
}
fn required_condition(
    items: &mut Vec<Value>,
    budget: &mut DetailBudget,
    cap: usize,
    source: JsonView<'_>,
    condition: (&'static str, &'static str, &'static str, &'static str),
) {
    let (code, severity, basis, message) = condition;
    retain(items, budget, cap, || {
        let span = source.span();
        required_condition_value(source.source_id(), span, (code, severity, basis, message))
    });
}
fn required_condition_value(
    source: SourceId,
    span: std::ops::Range<usize>,
    condition: (&'static str, &'static str, &'static str, &'static str),
) -> Value {
    let (code, severity, basis, message) = condition;
    let mut value = json!({"code":code,"severity":severity,"basis":{"kind":basis,"citation":"RFC 9880 Section 4.5"},"message":message,"assessment_unit":"inspection","primary":{"kind":"source","source_id":source.get(),"pointer":null,"span":[span.start,span.end]},"related":[],"arguments":{}});
    if basis != "rfc" && basis != "json" {
        value["basis"]["id"] = json!(code);
    }
    if code == "context.required_undefined" {
        value["basis"]["id"] = json!(crate::context::DRAFT);
        value["basis"]["citation"] = json!("draft-ietf-asdf-sdf-nonaffordance-06 Section 4.2");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn required_location_and_condition_fixed_metadata_fit_precharges() {
        let source = SourceId(u32::MAX);
        let mut location = json!({"kind":"source","source_id":source.get(),"pointer":null,"span":[usize::MAX,usize::MAX]});
        required_location_omission(
            &mut location,
            "result.required_declarations.owners.references.targets.source_definition.pointer",
            Some("location_limit"),
            Some(source),
        );
        let source_wire = serde_json::to_string(&location).unwrap().len();
        assert!(source_wire <= 512);
        let mut location = json!({"kind":"effective","root":source.get(),"pointer":null});
        required_location_omission(
            &mut location,
            "result.required_declarations.owners.references.targets.effective_definition.pointer",
            Some("location_limit"),
            None,
        );
        let effective_wire = serde_json::to_string(&location).unwrap().len();
        assert!(effective_wire <= 512);
        let mut condition_wire = 0;
        for condition in [
            (
                "required.syntax",
                "error",
                "rfc",
                "sdfRequired must be an array of name references or carrier markers",
            ),
            (
                "required.target_missing",
                "notice",
                "advice",
                "Required target was not found in the established scope",
            ),
            (
                "required.unavailable",
                "incomplete",
                "interpretation",
                "Required declaration identity or context is unavailable",
            ),
        ] {
            let value = required_condition_value(source, usize::MAX..usize::MAX, condition);
            condition_wire = condition_wire.max(serde_json::to_string(&value).unwrap().len() + 1);
        }
        assert!(condition_wire <= 512);
        println!(
            "required_repair_metadata source_wire={source_wire}; effective_wire={effective_wire}; condition_wire={condition_wire}; location_cap=512; condition_cap=512"
        );
    }
    #[test]
    fn fixed_emitter_inventory_covers_maximum_counts_and_printable_escaping() {
        let counts = Counts {
            owners_known: u32::MAX,
            owners_retained: u32::MAX,
            references_known: u32::MAX,
            references_retained: u32::MAX,
            targets_known: u32::MAX,
            targets_retained: u32::MAX,
            owners_omitted: u32::MAX,
        };
        for reason in ["\"".repeat(96), "\\".repeat(96)] {
            let mut wire = String::new();
            prefix_json(&mut wire, "unavailable", false, Some(&reason), counts);
            wire.push(']');
            omission_json(&mut wire, u32::MAX, "report_limit");
            wire.push('}');
            let wire_bytes = wire.len();
            assert!(wire_bytes + 40 <= 768);
            let document = format!("{{\"result\":{{{}}}}}", wire.strip_prefix(',').unwrap());
            let value: Value = serde_json::from_str(&document).unwrap();
            assert_eq!(
                value["result"]["required_declarations"]["owners_known"],
                u32::MAX
            );
            assert_eq!(
                value["result"]["required_declarations"]["unavailable_reason"],
                reason
            );
            let mut human = HumanText::new(HumanStyle::Plain);
            prefix_human(
                &mut human,
                "unavailable",
                false,
                Some(&reason),
                counts,
                Some("report_limit"),
            );
            assert!(human.len() <= 512);
            println!(
                "required_fixed_inventory wire_with_omission={wire_bytes}; diagnostic_addition=40; human_prefix={}; Counts_size={}; presentation_size={}",
                human.len(),
                std::mem::size_of::<Counts>(),
                std::mem::size_of::<Presentation>()
            );
        }
    }
}
