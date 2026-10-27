//! Read corresponding effective content on the one operation allowance. The walk and the
//! monotone rules share these reads, so the same read always costs the same work.
use super::{UndeterminedReason, failure};
use crate::allowance::{OperationBudget, push};
use crate::budgeted;
use crate::role::Role;
use crate::sample::ValueAllowance;
use crate::{EffectiveJson, Field, JsonKind};
use std::cmp::Ordering;

/// Charge comparison work to the operation allowance.
pub(super) fn work(budget: &mut OperationBudget, cost: usize) -> Result<(), UndeterminedReason> {
    budgeted::work(budget, cost).map_err(limited)
}

/// The member of a map with this name, charging every scanned key.
pub(super) fn member<'c>(
    json: EffectiveJson<'c>,
    key: &str,
    budget: &mut OperationBudget,
) -> Result<Option<EffectiveJson<'c>>, UndeterminedReason> {
    budgeted::member(json, key, budget).map_err(limited)
}

/// Whether `json` is an available array of decoded strings, as `enum` values and `required`
/// names compare, charging each element.
pub(super) fn strings(
    json: EffectiveJson<'_>,
    budget: &mut OperationBudget,
) -> Result<bool, UndeterminedReason> {
    budgeted::strings(json, budget).map_err(limited)
}

/// Reading charges only work.
fn limited(_: budgeted::Exhausted) -> UndeterminedReason {
    UndeterminedReason::limit("comparison work limit")
}

/// Whether `text` is among the decoded strings of `values`, charging each comparison. An
/// absent array holds no strings.
pub(super) fn contains(
    values: Option<EffectiveJson<'_>>,
    text: &str,
    budget: &mut OperationBudget,
) -> Result<bool, UndeterminedReason> {
    for value in values.into_iter().flat_map(EffectiveJson::children) {
        let other = value.text().present().expect("decoded string");
        work(budget, text.len().min(other.len()) + 1)?;
        if text == other {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether every decoded string of `values` is among those of `others`, charging each
/// comparison. An absent array holds no strings.
pub(super) fn within(
    values: Option<EffectiveJson<'_>>,
    others: Option<EffectiveJson<'_>>,
    budget: &mut OperationBudget,
) -> Result<bool, UndeterminedReason> {
    for value in values.into_iter().flat_map(EffectiveJson::children) {
        let text = value.text().present().expect("decoded string");
        if !contains(others, text, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether a structural comparison finds corresponding content equal.
pub(super) enum Equality {
    Equal,
    Different,
    Unavailable(UndeterminedReason),
}

/// A structural comparison with exact numbers, decoded text, ordered arrays and unordered
/// maps. Content it calls equal has equal values under ADR 0015: Bounded value equality in a
/// resolved data context, but it reads no data context, so a difference may be one of
/// representation alone. A pair it calls different is therefore classified conservatively,
/// as a change. One definite difference decides, and otherwise unavailable content leaves
/// equality unknown.
pub(super) fn equal(
    earlier: EffectiveJson<'_>,
    later: EffectiveJson<'_>,
    allowance: &mut ValueAllowance,
) -> Result<Equality, UndeterminedReason> {
    let mut stack = Vec::new();
    kept(&mut stack, [earlier, later], &mut allowance.budget)?;
    let mut unknown = None;
    while let Some([a, b]) = stack.pop() {
        work(&mut allowance.budget, 1)?;
        if let Some(json) = [a, b].into_iter().find(|json| failure(*json).is_some()) {
            unknown.get_or_insert(unavailable_reason(json));
            continue;
        }
        if a.kind() != b.kind() {
            return Ok(Equality::Different);
        }
        match a.kind() {
            JsonKind::Null => {}
            JsonKind::Boolean => {
                if a.boolean().present() != b.boolean().present() {
                    return Ok(Equality::Different);
                }
            }
            JsonKind::Number => match (a.number(), b.number()) {
                (Field::Present(x), Field::Present(y)) => match allowance.compare(x, y) {
                    Ok(Ordering::Equal) => {}
                    Ok(_) => return Ok(Equality::Different),
                    Err(message) => {
                        unknown.get_or_insert(UndeterminedReason::limit(message));
                    }
                },
                _ => {
                    unknown.get_or_insert(UndeterminedReason::Unavailable {
                        code: "data.number_unavailable",
                        message: "exact number capability",
                    });
                }
            },
            JsonKind::String => match (a.text(), b.text()) {
                (Field::Present(x), Field::Present(y)) => {
                    work(&mut allowance.budget, x.len().min(y.len()) + 1)?;
                    if x != y {
                        return Ok(Equality::Different);
                    }
                }
                (Field::Unavailable(message), _) | (_, Field::Unavailable(message)) => {
                    unknown.get_or_insert(UndeterminedReason::Unavailable {
                        code: "input.unicode_unavailable",
                        message,
                    });
                }
                _ => unreachable!("string text is present or unavailable"),
            },
            JsonKind::Array => {
                let (mut left, mut right) = (a.children(), b.children());
                loop {
                    work(&mut allowance.budget, 1)?;
                    match (left.next(), right.next()) {
                        (Some(x), Some(y)) => kept(&mut stack, [x, y], &mut allowance.budget)?,
                        (None, None) => break,
                        _ => return Ok(Equality::Different),
                    }
                }
            }
            JsonKind::Map => {
                let mut count = 0usize;
                for child in a.children() {
                    count += 1;
                    let Field::Present(key) = child.member_name() else {
                        unknown.get_or_insert(identity_unavailable());
                        continue;
                    };
                    match member(b, key, &mut allowance.budget)? {
                        Some(other) => kept(&mut stack, [child, other], &mut allowance.budget)?,
                        None => return Ok(Equality::Different),
                    }
                }
                for _ in b.children() {
                    work(&mut allowance.budget, 1)?;
                    count = match count.checked_sub(1) {
                        Some(count) => count,
                        None => return Ok(Equality::Different),
                    };
                }
                if count != 0 {
                    return Ok(Equality::Different);
                }
            }
        }
    }
    Ok(unknown.map_or(Equality::Equal, Equality::Unavailable))
}

/// Whether corresponding map data descriptions both leave their entry `name` undescribed, or
/// describe it alike.
pub(super) fn same_entry(
    owners: [EffectiveJson<'_>; 2],
    name: &str,
    allowance: &mut ValueAllowance,
) -> Result<bool, UndeterminedReason> {
    let mut described = [None, None];
    for (described, owner) in described.iter_mut().zip(owners) {
        if let Some(properties) = member(owner, "properties", &mut allowance.budget)? {
            *described = member(properties, name, &mut allowance.budget)?;
        }
    }
    match described {
        [None, None] => Ok(true),
        [Some(earlier), Some(later)] => alike(earlier, later, allowance),
        _ => Ok(false),
    }
}

/// Whether corresponding data descriptions are structurally equal apart from their documentary
/// qualities, at any depth of their `properties` entries, `items` and choice branches, so that
/// both revisions read their values alike. `description` and `$comment` are documentary, and
/// so are `label` and `default` outside a Direct items description, where they would leave its
/// meaning unestablished.
fn alike(
    earlier: EffectiveJson<'_>,
    later: EffectiveJson<'_>,
    allowance: &mut ValueAllowance,
) -> Result<bool, UndeterminedReason> {
    let mut stack = Vec::new();
    kept(&mut stack, [earlier, later], &mut allowance.budget)?;
    while let Some(pair) = stack.pop() {
        work(&mut allowance.budget, 1)?;
        let role = pair[0].n().role;
        let described = role == pair[1].n().role
            && matches!(role, Some(Role::Data | Role::DirectItem))
            && pair
                .iter()
                .all(|json| failure(*json).is_none() && json.kind() == JsonKind::Map);
        let Some(role) = role.filter(|_| described) else {
            if !matches!(equal(pair[0], pair[1], allowance)?, Equality::Equal) {
                return Ok(false);
            }
            continue;
        };
        let documentary = |key: &str| role.metadata(key) || role == Role::Data && key == "default";
        let Some(members) = paired(pair, documentary, &mut allowance.budget)? else {
            return Ok(false);
        };
        for (key, members) in members {
            let nested = match key {
                "items" => Some(vec![members]),
                "properties" | "sdfChoice" => paired(members, |_| false, &mut allowance.budget)?
                    .map(|entries| entries.into_iter().map(|(_, entry)| entry).collect()),
                _ => None,
            };
            match nested {
                Some(nested) => {
                    for pair in nested {
                        kept(&mut stack, pair, &mut allowance.budget)?;
                    }
                }
                None if !matches!(equal(members[0], members[1], allowance)?, Equality::Equal) => {
                    return Ok(false);
                }
                None => {}
            }
        }
    }
    Ok(true)
}

/// The members of corresponding maps that share a name.
type Paired<'c> = Vec<(&'c str, [EffectiveJson<'c>; 2])>;

/// The members of corresponding maps with the same name, apart from those `skipped` names, or
/// `None` when either map is unavailable or has a member the other lacks.
fn paired<'c>(
    maps: [EffectiveJson<'c>; 2],
    skipped: impl Fn(&str) -> bool,
    budget: &mut OperationBudget,
) -> Result<Option<Paired<'c>>, UndeterminedReason> {
    if maps
        .iter()
        .any(|json| failure(*json).is_some() || json.kind() != JsonKind::Map)
    {
        return Ok(None);
    }
    let mut members = Vec::new();
    for (index, map) in maps.into_iter().enumerate() {
        for child in map.children() {
            let Some(key) = child.n().key else {
                return Ok(None);
            };
            work(budget, key.len() + 1)?;
            if skipped(key) {
                continue;
            }
            let Some(other) = member(maps[1 - index], key, budget)? else {
                return Ok(None);
            };
            if index == 0 {
                kept(&mut members, (key, [child, other]), budget)?;
            }
        }
    }
    Ok(Some(members))
}

/// Why corresponding content is unavailable, or why its shape is not the definition or data
/// description map it should be.
pub(super) fn unavailable_reason(json: EffectiveJson<'_>) -> UndeterminedReason {
    match failure(json) {
        Some(failure) => UndeterminedReason::Unavailable {
            code: failure.code(),
            message: failure.reason,
        },
        None => UndeterminedReason::Unavailable {
            code: "interpretation.unresolved",
            message: "definition or data-description map shape",
        },
    }
}

pub(super) fn identity_unavailable() -> UndeterminedReason {
    UndeterminedReason::Unavailable {
        code: "interpretation.unresolved",
        message: "map member identity unavailable",
    }
}

fn kept<T>(
    items: &mut Vec<T>,
    item: T,
    budget: &mut OperationBudget,
) -> Result<(), UndeterminedReason> {
    push(items, item, &mut budget.allocation)
        .map_err(|_| UndeterminedReason::limit("comparison allocation limit"))
}
