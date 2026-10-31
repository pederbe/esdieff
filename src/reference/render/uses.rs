//! Reverse navigation over existing nearest-origin explanations, without resolving names again.
use super::*;

/// Match source ancestry by node identity, including generated/typed source trees whose
/// byte spans need not identify an individual member.
fn within(
    mut source: JsonView<'_>,
    selected: JsonView<'_>,
    budget: &mut OperationBudget,
) -> Result<bool, Exhausted> {
    if source.source_id() != selected.source_id() {
        return Ok(false);
    }
    loop {
        budgeted::work(budget, 1)?;
        if source.node == selected.node {
            return Ok(true);
        }
        match source.n().parent {
            Some(parent) => source.node = parent,
            None => return Ok(false),
        }
    }
}

pub(super) fn render(
    writer: &mut Writer<'_>,
    request: &ReferenceRequest<'_>,
    root: EffectiveJson<'_>,
) -> Result<(), Exhausted> {
    if !writer.row(&[
        "\n## Shared definition uses\n\n",
        "Scope: supplied model documents only. Effective occurrences below retain a nearest ",
        "source or previous source within the selected authored definition. Importing consumers ",
        "are authored reference locations, separately from effective occurrences. This is not ",
        "a complete transitive reference graph. Deleted content is excluded.\n\n",
    ])? {
        writer.omissions.unknown_remainder = true;
        return Ok(());
    }
    let selected = root.n().declaration_source;
    // An effective-only selection has no authored subtree at the requested coordinate.
    // Do not silently substitute its donor as the reverse-navigation query.
    budgeted::work(&mut writer.budget, request.pointer.len() + 1)?;
    if !charge(&mut writer.budget.allocation, request.pointer.len() + 1) {
        return Err(Exhausted::Allocation);
    }
    if selected.source_id() != request.document
        || selected
            .pointer(request.pointer.len() as u32)
            .present()
            .as_deref()
            != Some(request.pointer)
    {
        writer.unavailable("Use navigation requires a selected authored definition.")?;
        return Ok(());
    }
    let catalogue = writer.catalogue;
    let mut occurrences = 0usize;
    for source in catalogue
        .sources()
        .filter(|source| source.role().is_model())
    {
        budgeted::work(&mut writer.budget, 1)?;
        let Some(label) = writer.text(source.label().unwrap_or(source.key()))? else {
            continue;
        };
        if !writer.row(&["\n### Effective document: ", &label, "\n\n"])? {
            writer.omissions.entries += 1;
            continue;
        }
        let model = match catalogue.compose_gated(
            EffectiveRequest::new(source.id()),
            &mut writer.budget,
            false,
        ) {
            Ok(model) => model,
            Err(_) => {
                writer.unavailable("Supplied model could not be explored.")?;
                continue;
            }
        };
        if let Some(failure) = model.failure {
            writer.condition(failure.code(), failure.reason, failure.definite())?;
        }
        if !model.complete_apart_from_required() {
            writer.unavailable(
                "Use navigation could not establish every effective branch in this document.",
            )?;
        }
        if model.reference_errors > 0 {
            writer.condition(
                "reference.composition-failed",
                "Use navigation encountered a definite reference failure.",
                true,
            )?;
        }
        if source.parsed.errors > 0 || source.acquisition() == Acquisition::Failed {
            writer.condition(
                "reference.input-failed",
                "Supplied model acquisition or decoding failed during use navigation.",
                true,
            )?;
        }
        let mut stack = Vec::new();
        if let Field::Present(json) = model.json() {
            budgeted::kept(&mut stack, json, &mut writer.budget)?;
        } else {
            writer.unavailable("Supplied effective document unavailable for use navigation.")?;
        }
        let mut found = false;
        while let Some(json) = stack.pop() {
            budgeted::work(&mut writer.budget, 1)?;
            if let Some(failure) = budgeted::failure(json) {
                writer.condition(failure.code(), failure.reason, failure.definite())?;
            }
            let mut matched = false;
            for origin in json.explanation().sources() {
                matched |= within(origin, selected, &mut writer.budget)?;
            }
            if matched
                && budgeted::failure(json).is_none()
                && json.explanation().consumer().is_some()
                && let Some(pointer) = writer.pointer(json)?
            {
                budgeted::work(&mut writer.budget, pointer.len() + 1)?;
                // A selected alias has an importing consumer of its own. Its
                // effective subtree is still the source being queried, not a use.
                let selected_subtree = source.id() == request.document
                    && (pointer == request.pointer
                        || pointer
                            .strip_prefix(request.pointer)
                            .is_some_and(|suffix| suffix.starts_with('/')));
                if !selected_subtree {
                    found = true;
                    occurrences += 1;
                    if let Some(pointer) = writer.text(&pointer)? {
                        if !writer.row(&["- Effective occurrence: ", &pointer, "\n\n"])? {
                            continue;
                        }
                        if json.kind() != crate::JsonKind::Map
                            && json.kind() != crate::JsonKind::Array
                        {
                            writer.quality("Effective value", json, "")?;
                        } else {
                            writer.origin(json)?;
                        }
                    }
                }
            }
            for child in json.children() {
                budgeted::kept(&mut stack, child, &mut writer.budget)?;
            }
        }
        if !found {
            writer.row(&["No established effective uses retained in this document.\n\n"])?;
        }
    }
    if occurrences == 0 {
        writer.row(&["No established effective uses found within the stated scope.\n\n"])?;
    }
    Ok(())
}
