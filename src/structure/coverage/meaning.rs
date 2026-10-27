//! Model-level interpretation, separate from formal admission and sample verdicts.
use super::*;
use crate::allowance::OperationBudget;
use crate::role::SetAside;
use crate::{EffectiveJson, NumberWork};

pub(super) struct Local {
    pub(super) scalar: [(Requirement, crate::AssessmentStatus); 4],
    context: crate::AssessmentStatus,
    ordinary: bool,
    kind: Option<&'static str>,
    needed: bool,
}

pub(super) struct Special {
    pub(super) rows: [(Requirement, crate::AssessmentStatus); 4],
    pub(super) pattern_condition: Option<(usize, crate::pattern::Condition)>,
}

pub(super) fn fixed_headers() -> usize {
    use std::mem::size_of;
    // The whole-description walk can call one equality-capability walk; that
    // walk never calls another walk. Const comparison can run with the outer
    // walk retained. Backing Vec capacities remain separately charged.
    4 * size_of::<Local>()
        + 4 * size_of::<Special>()
        + 32 * size_of::<EffectiveJson<'_>>()
        + 8 * size_of::<Vec<EffectiveJson<'_>>>()
        + 4 * size_of::<&mut OperationBudget>()
        + crate::equality::fixed_headers()
        + crate::civil::fixed_headers().max(crate::formats::fixed_headers())
}

/// Fixed-size observations keep interpretation in the traversal's original work budget.
/// Undeclared extension vocabulary is skipped, following ADR 0037: Undeclared extension
/// vocabulary, and an established context records that it relied on that reading.
pub(super) fn scalar(
    json: CheckView<'_, '_>,
    role: Role,
    object: bool,
    budget: &mut OperationBudget,
) -> Option<Local> {
    let CheckView::Effective { model, node } = json else {
        return None;
    };
    if !(EffectiveJson { model, node }).model_meaning_needed(&mut budget.work)? {
        return Some(Local {
            scalar: [
                (Requirement::M042, NotApplicable),
                (Requirement::M043, NotApplicable),
                (Requirement::M045, NotApplicable),
                (Requirement::M046, NotApplicable),
            ],
            context: Assessed,
            ordinary: true,
            kind: None,
            needed: false,
        });
    }
    let n = &model.nodes[node];
    let mut context = if model.clear_feature
        && n.unavailable.is_none()
        && n.meaning_unavailable.is_none()
        && n.donor_unavailable.is_none()
    {
        Assessed
    } else {
        Undetermined
    };
    let mut ordinary = true;
    let mut kind = None;
    let mut observations = [NotApplicable; 4];
    let mut byte_string = false;
    let mut set_aside = SetAside::default();
    for child in json.children() {
        let name = child.member_name().present();
        if !charge_exact(&mut budget.work, name.map_or(1, |key| key.len() + 1)) {
            return None;
        }
        let Some(key) = name else {
            context = Undetermined;
            continue;
        };
        let CheckView::Effective { node: child_id, .. } = child else {
            unreachable!();
        };
        let q = &model.nodes[child_id];
        let available = q.unavailable.is_none()
            && q.meaning_unavailable.is_none()
            && q.donor_unavailable.is_none();
        let vocabulary = set_aside.classify(role, key, object);
        if !available || vocabulary.blocks_base_meaning() {
            context = Undetermined;
        }
        if vocabulary != Vocabulary::Supported {
            continue;
        }
        match key {
            "type" => {
                let status = match child.text() {
                    Field::Present(
                        value @ ("number" | "integer" | "string" | "boolean" | "array" | "object"),
                    ) => {
                        kind = Some(match value {
                            "number" => "number",
                            "integer" => "integer",
                            "string" => "string",
                            "boolean" => "boolean",
                            "array" => "array",
                            _ => "object",
                        });
                        if role == Role::DirectItem && value == "array" {
                            Undetermined
                        } else {
                            Assessed
                        }
                    }
                    Field::Present(_) | Field::Unavailable(_) => Undetermined,
                    _ => {
                        // Selector shape has its own finding and requirement.
                        context = Undetermined;
                        continue;
                    }
                };
                observations[0] = status;
                if status == Undetermined {
                    context = Undetermined;
                }
            }
            "sdfType" | "format" | "contentFormat" => {
                ordinary = false;
                byte_string |=
                    key == "sdfType" && matches!(child.text(), Field::Present("byte-string"));
                let status = match (key, child.text()) {
                    ("sdfType", Field::Present("byte-string" | "unix-time"))
                    | (
                        "format",
                        Field::Present(
                            "date-time" | "date" | "time" | "uri" | "uri-reference" | "uuid",
                        ),
                    ) => Assessed,
                    // Declaration meaning is owned by M024; base constraints
                    // do not depend on a payload interpretation.
                    ("contentFormat", Field::Present(_)) => Assessed,
                    _ => Undetermined,
                };
                context = crate::inventory::join(context, status);
            }
            "const" => {
                observations[1] = match child.kind() {
                    JsonKind::Boolean | JsonKind::Null => Assessed,
                    JsonKind::String => match child.text() {
                        Field::Present(_) => Assessed,
                        _ => Undetermined,
                    },
                    JsonKind::Number => mathematical_number(child),
                    // Contextual compound equality is assessed separately below.
                    JsonKind::Array | JsonKind::Map => NotApplicable,
                };
            }
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf" => {
                observations[2] =
                    crate::inventory::join(observations[2], mathematical_number(child));
            }
            "minLength" | "maxLength" => {
                observations[3] =
                    crate::inventory::join(observations[3], mathematical_number(child));
            }
            _ => {}
        }
    }
    if !ordinary {
        // A recognized abstract selector alone cannot establish the actual
        // constant's denotation. The shared contextual comparator handles it.
        observations[1] = NotApplicable;
    }
    if byte_string && observations[3] != NotApplicable {
        // Neither encoded-text length nor decoded-byte length is adopted here.
        observations[3] = Undetermined;
    }
    for status in &mut observations {
        if *status != NotApplicable {
            *status = crate::inventory::join(*status, context);
        }
    }
    if context == Assessed {
        budget.set_aside |= set_aside;
    }
    Some(Local {
        scalar: [
            (Requirement::M042, observations[0]),
            (Requirement::M043, observations[1]),
            (Requirement::M045, observations[2]),
            (Requirement::M046, observations[3]),
        ],
        context,
        ordinary,
        kind,
        needed: true,
    })
}

/// Named model interpretations are independent of actual value denotation.
/// Matching capability compiles the real pattern without a subject.
pub(super) fn special(
    json: EffectiveJson<'_>,
    local: &Local,
    limits: &crate::Limits,
    budget: &mut OperationBudget,
) -> Option<Special> {
    if !local.needed {
        return Some(Special {
            rows: [
                (Requirement::M026, NotApplicable),
                (Requirement::M027, NotApplicable),
                (Requirement::M048, NotApplicable),
                (Requirement::M049, NotApplicable),
            ],
            pattern_condition: None,
        });
    }
    let role = json.n().role?;
    let mut sdf_type = None;
    let mut format = None;
    let mut pattern = None;
    for child in json.children() {
        let key = child.n().key;
        if !charge_exact(&mut budget.work, key.map_or(1, |key| key.len() + 1)) {
            return None;
        }
        if !key.is_some_and(|key| role.supports(key, local.kind == Some("object"))) {
            continue;
        }
        match key {
            Some("sdfType") => sdf_type = Some(child),
            Some("format") => format = Some(child),
            Some("pattern") => pattern = Some(child),
            _ => {}
        }
    }
    let mut observations = [NotApplicable; 4];
    let mut pattern_condition = None;
    if let Some(value) = sdf_type {
        let text = value.text().present();
        if !charge_exact(&mut budget.work, text.map_or(1, |text| text.len() + 1)) {
            return None;
        }
        let (index, compatible) = match text {
            Some("byte-string") => (0, local.kind.is_none_or(|kind| kind == "string")),
            Some("unix-time") => (
                1,
                local
                    .kind
                    .is_none_or(|kind| matches!(kind, "number" | "integer")),
            ),
            _ => (4, false),
        };
        if index < 4 {
            observations[index] = if compatible && format.is_none() {
                local.context
            } else {
                Undetermined
            };
        }
    }
    if let Some(value) = format {
        let text = value.text().present();
        if !charge_exact(&mut budget.work, text.map_or(1, |text| text.len() + 1)) {
            return None;
        }
        observations[3] = if text.is_some_and(|text| {
            crate::scalar::TemporalFormat::from_text(text).is_some()
                || crate::scalar::ResourceFormat::from_text(text).is_some()
        }) && local.kind.is_none_or(|kind| kind == "string")
            && sdf_type.is_none()
        {
            local.context
        } else {
            Undetermined
        };
    }
    if let Some(value) = pattern
        && value.kind() == JsonKind::String
    {
        let (status, condition) = if local.context != Assessed
            || matches!(
                sdf_type.and_then(|value| value.text().present()),
                Some("byte-string")
            ) {
            (Undetermined, Some(crate::pattern::Condition::Unavailable))
        } else if let Field::Present(text) = value.text() {
            match crate::pattern::matching_capability(text, limits, budget) {
                crate::pattern::Verdict::Valid => (Assessed, None),
                // M047 owns the syntax finding; it is not repeated here.
                crate::pattern::Verdict::Invalid => (Undetermined, None),
                crate::pattern::Verdict::Unsupported => {
                    (Undetermined, Some(crate::pattern::Condition::Unsupported))
                }
                crate::pattern::Verdict::Limit => {
                    (Undetermined, Some(crate::pattern::Condition::Limit))
                }
            }
        } else {
            (Undetermined, Some(crate::pattern::Condition::Unavailable))
        };
        observations[2] = status;
        pattern_condition = condition.map(|condition| (value.node, condition));
    }
    Some(Special {
        rows: [
            (Requirement::M026, observations[0]),
            (Requirement::M027, observations[1]),
            (Requirement::M048, observations[2]),
            (Requirement::M049, observations[3]),
        ],
        pattern_condition,
    })
}

pub(super) fn compound<'a>(
    json: EffectiveJson<'a>,
    local: &Local,
    budget: &mut OperationBudget,
    numbers: &mut NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
    limits: &crate::Limits,
) -> Option<[(Requirement, crate::AssessmentStatus); 6]> {
    if !local.needed {
        return Some([
            (Requirement::M043, NotApplicable),
            (Requirement::M050, NotApplicable),
            (Requirement::M051, NotApplicable),
            (Requirement::M052, NotApplicable),
            (Requirement::M053, NotApplicable),
            (Requirement::M054, NotApplicable),
        ]);
    }
    let mut observations = [NotApplicable; 6];
    if local.kind == Some("object") {
        // Map descriptions keep optional entries and remain open to other keys.
        observations[4] = local.context;
        observations[5] = local.context;
    }
    let final_owner = matches!(crate::sample::alternatives(json, budget), Field::Absent);
    let mut adapter = CompoundAdapter::Owner { final_owner };
    let mut work = CompoundWork {
        budget,
        numbers,
        equality,
        limits,
    };
    for child in json.children() {
        if !charge_exact(
            &mut work.budget.work,
            child.n().key.map_or(1, |key| key.len() + 1),
        ) {
            return None;
        }
        if let Some((index, status)) =
            compound_quality(json, child, local, &mut adapter, &mut work)?
        {
            observations[index] = crate::inventory::join(observations[index], status);
        }
    }
    Some([
        (Requirement::M043, observations[0]),
        (Requirement::M050, observations[1]),
        (Requirement::M051, observations[2]),
        (Requirement::M052, observations[3]),
        (Requirement::M053, observations[4]),
        (Requirement::M054, observations[5]),
    ])
}

struct CompoundWork<'a, 'work> {
    budget: &'work mut OperationBudget,
    numbers: &'work mut NumberWork,
    equality: &'work mut crate::equality::EqualityWork<'a>,
    limits: &'work crate::Limits,
}

/// Owner observations finish children inline. Parent capability schedules them
/// on its existing stack, including visits outside an established type guard.
enum CompoundAdapter<'a, 'jobs> {
    Owner { final_owner: bool },
    Descendants(&'jobs mut Vec<EffectiveJson<'a>>),
}

impl<'a> CompoundAdapter<'a, '_> {
    fn descendants(&self) -> bool {
        matches!(self, Self::Descendants(_))
    }

    fn description(
        &mut self,
        json: EffectiveJson<'a>,
        work: &mut CompoundWork<'a, '_>,
    ) -> Option<crate::AssessmentStatus> {
        match self {
            Self::Owner { .. } => subtree(json, work),
            Self::Descendants(jobs) => {
                crate::allowance::push(jobs, json, &mut work.budget.allocation).ok()?;
                Some(NotApplicable)
            }
        }
    }
}

/// Shared compound decisions, with separate requirement observations and parent
/// capability aggregation. Neither adapter owns formal-admission diagnostics.
fn compound_quality<'a>(
    json: EffectiveJson<'a>,
    child: EffectiveJson<'a>,
    local: &Local,
    adapter: &mut CompoundAdapter<'a, '_>,
    work: &mut CompoundWork<'a, '_>,
) -> Option<Option<(usize, crate::AssessmentStatus)>> {
    let (index, status) = match child.n().key {
        Some("const") => {
            if matches!(adapter, CompoundAdapter::Owner { final_owner: false }) {
                return Some(None);
            }
            let mut status = NotApplicable;
            if adapter.descendants()
                && super::formal::literal(
                    CheckView::Effective {
                        model: child.model,
                        node: child.node,
                    },
                    &mut work.budget.work,
                ) != super::formal::Literal::Base
            {
                // Extension-only literals leave needed child meaning unfinished
                // without changing their separate grammar-admission result.
                status = Undetermined;
            }
            if !local.ordinary || matches!(child.kind(), JsonKind::Array | JsonKind::Map) {
                status = crate::inventory::join(
                    status,
                    constant(json, child, work.budget, work.numbers, work.equality),
                );
            }
            // Actual denotation is independent of local type/count conclusions.
            return Some(Some((0, status)));
        }
        Some("items") => {
            let applicable = local.kind == Some("array");
            let mut status = if applicable { Assessed } else { Undetermined };
            if applicable || adapter.descendants() {
                status = crate::inventory::join(status, adapter.description(child, work)?);
            }
            (1, status)
        }
        Some("minItems" | "maxItems") => (
            2,
            mathematical_number(CheckView::Effective {
                model: child.model,
                node: child.node,
            }),
        ),
        Some("uniqueItems") => {
            let boolean = child.boolean();
            if adapter.descendants() && !matches!(boolean, Field::Present(true)) {
                return Some(None);
            }
            let status = match boolean {
                Field::Present(false) => Assessed,
                Field::Present(true) if local.kind == Some("array") => {
                    match json.lookup("items", work.budget) {
                        Field::Absent => Assessed,
                        Field::Present(items) => comparator(items, work.budget)?,
                        Field::Unavailable(_) => Undetermined,
                    }
                }
                Field::Present(true) => Undetermined,
                _ if child.kind() != JsonKind::Boolean => Assessed,
                _ => Undetermined,
            };
            (3, status)
        }
        Some("properties") => {
            let applicable = local.kind == Some("object");
            let mut status = if applicable { Assessed } else { Undetermined };
            if applicable || adapter.descendants() {
                for property in child.children() {
                    if !charge_exact(
                        &mut work.budget.work,
                        property.n().key.map_or(1, |key| key.len() + 1),
                    ) {
                        return None;
                    }
                    if property.n().key.is_none() {
                        status = Undetermined;
                    }
                    if property.n().key.is_some() || adapter.descendants() {
                        status =
                            crate::inventory::join(status, adapter.description(property, work)?);
                    }
                }
            }
            (4, status)
        }
        Some("required") => (
            5,
            if local.kind == Some("object") || adapter.descendants() {
                required(child, work.budget)?
            } else {
                Undetermined
            },
        ),
        _ => return Some(None),
    };
    Some(Some((index, crate::inventory::join(status, local.context))))
}

/// Actual const denotation reuses equality, without checking admission or inventing a sample.
pub(super) fn constant<'a>(
    description: EffectiveJson<'a>,
    candidate: EffectiveJson<'a>,
    budget: &mut OperationBudget,
    numbers: &mut NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
) -> crate::AssessmentStatus {
    let value = crate::value::BorrowedValue::Model(candidate);
    let result = equality.compare_constant(
        &crate::value::ValueContext {
            description,
            root: value,
        },
        value,
        candidate,
        numbers,
        budget,
    );
    match result.verdict {
        crate::finding::Verdict::Equal => Assessed,
        crate::finding::Verdict::Different | crate::finding::Verdict::Unavailable => Undetermined,
    }
}

fn required(
    json: EffectiveJson<'_>,
    budget: &mut OperationBudget,
) -> Option<crate::AssessmentStatus> {
    let mut status = Assessed;
    for child in json.children() {
        if !charge_exact(&mut budget.work, 1) {
            return None;
        }
        if child.kind() == JsonKind::String && !matches!(child.text(), Field::Present(_)) {
            status = Undetermined;
        }
    }
    Some(status)
}

/// Whole-model child descriptions cannot skip optional properties or empty-array items.
fn subtree<'a>(
    root: EffectiveJson<'a>,
    work: &mut CompoundWork<'a, '_>,
) -> Option<crate::AssessmentStatus> {
    let mut jobs = Vec::new();
    if crate::allowance::push(&mut jobs, root, &mut work.budget.allocation).is_err() {
        return None;
    }
    let mut status = Assessed;
    while let Some(json) = jobs.pop() {
        if !charge_exact(&mut work.budget.work, 1) {
            return None;
        }
        if !json.model.clear_feature
            || json.n().unavailable.is_some()
            || json.n().donor_unavailable.is_some()
            || !json.consistency_meaning_available()
        {
            status = Undetermined;
            continue;
        }
        match crate::sample::alternatives(json, work.budget) {
            Field::Present(rows) => {
                for (_, branch) in rows {
                    if !charge_exact(&mut work.budget.work, 1) {
                        return None;
                    }
                    if let Field::Present(branch) = branch {
                        if crate::allowance::push(
                            &mut jobs,
                            branch.json(),
                            &mut work.budget.allocation,
                        )
                        .is_err()
                        {
                            return None;
                        }
                    } else {
                        status = Undetermined;
                    }
                }
                continue;
            }
            Field::Unavailable(_) => {
                status = Undetermined;
                continue;
            }
            Field::Absent => {}
        }
        if json.kind() != JsonKind::Map {
            continue;
        }
        let Some(role) = json.n().role.filter(|role| role.data_description_role()) else {
            status = Undetermined;
            continue;
        };
        let object = matches!(
            json.lookup("type", work.budget)
                .present()
                .and_then(|value| value.text().present()),
            Some("object")
        );
        let local = scalar(
            CheckView::Effective {
                model: json.model,
                node: json.node,
            },
            role,
            object,
            work.budget,
        )?;
        status = crate::inventory::join(status, local.context);
        for (_, observation) in local.scalar {
            status = crate::inventory::join(status, observation);
        }
        for (_, observation) in special(json, &local, work.limits, work.budget)?.rows {
            status = crate::inventory::join(status, observation);
        }
        let mut adapter = CompoundAdapter::Descendants(&mut jobs);
        for child in json.children() {
            if !charge_exact(
                &mut work.budget.work,
                child.n().key.map_or(1, |key| key.len() + 1),
            ) {
                return None;
            }
            if let Some((_, observation)) =
                compound_quality(json, child, &local, &mut adapter, work)?
            {
                status = crate::inventory::join(status, observation);
            }
        }
    }
    Some(status)
}

/// Project every candidate interpretation at the same data location together.
/// Missing descriptions impose ordinary equality; they are not absent values.
fn comparator<'a>(
    root: EffectiveJson<'a>,
    budget: &mut OperationBudget,
) -> Option<crate::AssessmentStatus> {
    use crate::scalar::Context;
    let mut initial = Vec::new();
    crate::allowance::push(&mut initial, Some(root), &mut budget.allocation).ok()?;
    let mut jobs = Vec::new();
    crate::allowance::push(&mut jobs, initial, &mut budget.allocation).ok()?;
    while let Some(mut pending) = jobs.pop() {
        let mut candidates = Vec::new();
        let mut common = None;
        while let Some(candidate) = pending.pop() {
            if !charge_exact(&mut budget.work, 1) {
                return None;
            }
            let signature = if let Some(json) = candidate {
                if !json.model.clear_feature
                    || json.n().unavailable.is_some()
                    || json.n().donor_unavailable.is_some()
                    || !json.consistency_meaning_available()
                {
                    return Some(Undetermined);
                }
                match crate::sample::alternatives(json, budget) {
                    Field::Present(rows) => {
                        for (_, branch) in rows {
                            if !charge_exact(&mut budget.work, 1) {
                                return None;
                            }
                            let Field::Present(branch) = branch else {
                                return Some(Undetermined);
                            };
                            crate::allowance::push(
                                &mut pending,
                                Some(branch.json()),
                                &mut budget.allocation,
                            )
                            .ok()?;
                        }
                        continue;
                    }
                    Field::Unavailable(_) => return Some(Undetermined),
                    Field::Absent => {}
                }
                let Some(role) = json.n().role.filter(|role| role.data_description_role()) else {
                    return Some(Undetermined);
                };
                let object = matches!(
                    json.lookup("type", budget)
                        .present()
                        .and_then(|value| value.text().present()),
                    Some("object")
                );
                let local = scalar(
                    CheckView::Effective {
                        model: json.model,
                        node: json.node,
                    },
                    role,
                    object,
                    budget,
                )?;
                if local.context != Assessed {
                    return Some(Undetermined);
                }
                if local.ordinary {
                    Context::Ordinary
                } else {
                    match crate::sample::scalar_format_context(json, budget) {
                        Context::Bytes => Context::Bytes,
                        context @ (Context::Resource(crate::scalar::ResourceFormat::Uuid)
                        | Context::Temporal(
                            crate::scalar::TemporalFormat::Date
                            | crate::scalar::TemporalFormat::DateTime,
                        )) => {
                            if !role.full_data() {
                                return Some(Undetermined);
                            }
                            match json.lookup("nullable", budget) {
                                Field::Present(value) => match value.boolean() {
                                    Field::Present(_) => {}
                                    _ => return Some(Undetermined),
                                },
                                Field::Absent => {}
                                Field::Unavailable(_) => return Some(Undetermined),
                            }
                            context
                        }
                        _ => return Some(Undetermined),
                    }
                }
            } else {
                Context::Ordinary
            };
            if common.is_some_and(|previous| previous != signature) {
                return Some(Undetermined);
            }
            common = Some(signature);
            crate::allowance::push(&mut candidates, candidate, &mut budget.allocation).ok()?;
        }
        if common != Some(Context::Ordinary) {
            // Established abstract scalar classifiers admit no compound children.
            continue;
        }
        for owner in candidates.iter().flatten() {
            for quality in owner.children() {
                if !charge_exact(
                    &mut budget.work,
                    quality.n().key.map_or(1, |key| key.len() + 1),
                ) {
                    return None;
                }
                match quality.n().key {
                    Some("items") => {
                        if !matches!(
                            owner
                                .lookup("type", budget)
                                .present()
                                .and_then(|v| v.text().present()),
                            Some("array")
                        ) {
                            return Some(Undetermined);
                        }
                        let projection = project_comparators(&candidates, None, budget)?;
                        crate::allowance::push(&mut jobs, projection, &mut budget.allocation)
                            .ok()?;
                    }
                    Some("properties") => {
                        if !matches!(
                            owner
                                .lookup("type", budget)
                                .present()
                                .and_then(|v| v.text().present()),
                            Some("object")
                        ) {
                            return Some(Undetermined);
                        }
                        for property in quality.children() {
                            let Some(name) = property.n().key else {
                                return Some(Undetermined);
                            };
                            if !charge_exact(&mut budget.work, name.len() + 1) {
                                return None;
                            }
                            let projection = project_comparators(&candidates, Some(name), budget)?;
                            crate::allowance::push(&mut jobs, projection, &mut budget.allocation)
                                .ok()?;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Some(Assessed)
}

fn project_comparators<'a>(
    candidates: &[Option<EffectiveJson<'a>>],
    property: Option<&str>,
    budget: &mut OperationBudget,
) -> Option<Vec<Option<EffectiveJson<'a>>>> {
    let mut projected = Vec::new();
    for candidate in candidates {
        if !charge_exact(&mut budget.work, 1) {
            return None;
        }
        let child = if let Some(json) = candidate {
            let selected = json.lookup(
                if property.is_some() {
                    "properties"
                } else {
                    "items"
                },
                budget,
            );
            match (selected, property) {
                (Field::Present(properties), Some(name)) => properties.lookup(name, budget),
                (selected, _) => selected,
            }
        } else {
            Field::Absent
        };
        let selected = match child {
            Field::Present(json) => Some(json),
            Field::Absent => None,
            Field::Unavailable(_) => return None,
        };
        crate::allowance::push(&mut projected, selected, &mut budget.allocation).ok()?;
    }
    Some(projected)
}

fn mathematical_number(json: CheckView<'_, '_>) -> crate::AssessmentStatus {
    match json.number() {
        Field::Present(_) => Assessed,
        _ if json.kind() == JsonKind::Number => Undetermined,
        // Definite parameter shape failures are assessed work, with the
        // diagnostic retained by the shared parameter checker.
        _ => Assessed,
    }
}
