//! A map `properties` entry that only one revision describes. Clients use only what the
//! earlier revision describes or requires, following ADR 0039: Client-side revision
//! comparison, so an entry that only the later revision describes is part of no Client's
//! interaction unless the earlier revision requires it, itself or through an alternative of its
//! choice that inherits the entries. Maps are open, so an entry without a description admits
//! every value: a removed description widens its map, and a description added for an entry
//! that the earlier revision requires narrows it. The candidates hold the entry with one value
//! of each JSON kind, beside representative values of the other entries that either revision
//! requires, and Sample-value checking settles which of them the description rejects.
use super::choice::{self, Choices};
use super::{Candidate, Descriptions, Relation, Rule, enclosing};
use crate::EffectiveJson;
use crate::allowance::OperationBudget;
use crate::comparison::{Direction, Revision, UndeterminedReason, read};
use crate::representative::KINDS;

/// Whether Clients of the earlier revision use the entry `name` that only `revision`
/// describes: always when the earlier revision describes it, and otherwise when the earlier
/// revision requires it.
pub(super) fn used(
    descriptions: &mut Descriptions<'_, '_>,
    name: &str,
    revision: Revision,
) -> Result<bool, UndeterminedReason> {
    if revision == Revision::Earlier {
        return Ok(true);
    }
    required(
        descriptions.descriptions,
        name,
        &mut descriptions.allowance.budget,
    )
}

/// Whether the earlier of the corresponding descriptions `pair` requires the entry `name`,
/// itself or through a corresponding alternative of its choice that inherits `properties` in
/// both revisions, as a changed quality beside a choice is judged. Names that are not all
/// decoded strings may require it, and so may alternatives that are not established in both
/// revisions.
fn required(
    pair: [EffectiveJson<'_>; 2],
    name: &str,
    budget: &mut OperationBudget,
) -> Result<bool, UndeterminedReason> {
    if let Some(required) = read::member(pair[0], "required", budget)?
        && (!read::strings(required, budget)? || read::contains(Some(required), name, budget)?)
    {
        return Ok(true);
    }
    let alternatives = match choice::choices(pair, budget)? {
        Choices::Paired(alternatives) => alternatives,
        Choices::Unestablished => return Ok(true),
        // An earlier choice that the later revision lacks is a change of its own, but its
        // alternatives may still require the entry.
        Choices::Absent => return Ok(read::member(pair[0], "sdfChoice", budget)?.is_some()),
    };
    for [earlier, later] in alternatives {
        // An alternative that only one revision has is a change of its own.
        let (Some(earlier), Some(later)) = (earlier, later) else {
            continue;
        };
        if choice::inherits(earlier, "properties", budget)?
            && choice::inherits(later, "properties", budget)?
            && required([earlier, later], name, budget)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The rule of a used entry `name` that only `revision` describes.
pub(super) fn rule(
    descriptions: &mut Descriptions<'_, '_>,
    name: &str,
    revision: Revision,
) -> Result<Rule, UndeterminedReason> {
    let (relation, direction) = match revision {
        Revision::Earlier => (Relation::Wider, Direction::ThingSupplied),
        Revision::Later => (Relation::Narrower, Direction::ClientSupplied),
    };
    let mut rule = Rule::of(relation);
    for kind in KINDS {
        if descriptions.room(&rule, direction) == 0 {
            break;
        }
        let Some(value) = descriptions.value(4, kind)? else {
            continue;
        };
        let owners = descriptions.descriptions;
        let admitting = direction.admitting();
        let Some(map) = enclosing(owners, admitting, name, value, descriptions.allowance)? else {
            // The other entries are the same beside every kind.
            break;
        };
        descriptions.add(
            &mut rule,
            Candidate {
                value: map,
                direction,
            },
        )?;
    }
    Ok(rule)
}
