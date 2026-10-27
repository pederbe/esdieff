//! `uniqueItems` reads as `false` where it is absent, following ADR 0039: Client-side revision
//! comparison, and its candidate is an array that repeats one item, made of copies of the
//! first representative value of the `items` of the revision that must admit it. An array
//! without an `items` description admits items of every type, so an added description narrows
//! and a removed one widens, and arrays of one value of each base type are its candidates.
//! Item counts follow the count rule in `counts`.
use super::counts::bound;
use super::generating;
use super::{Candidate, Descriptions, Relation, Rule};
use crate::comparison::{CLIENT, Direction, Revision, THING, UndeterminedReason};
use crate::representative::{self, count};
use crate::sample::ValueAllowance;
use crate::{EffectiveJson, JsonValue};

/// The rule for a changed `uniqueItems`, which reads as `false` where it is absent. Arrays that
/// must hold unique items are fewer, and an array repeating one item separates the revisions:
/// at least two copies, and as many as the minimum count of the revision that must admit it.
pub(super) fn unique(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let unique = [
        unique_items(Revision::Earlier, descriptions)?,
        unique_items(Revision::Later, descriptions)?,
    ];
    let (relation, direction) = match unique {
        [Some(false), Some(true)] => (Relation::Narrower, Direction::ClientSupplied),
        [Some(true), Some(false)] => (Relation::Wider, Direction::ThingSupplied),
        _ => return Ok(Rule::of(Relation::Unknown)),
    };
    let mut rule = Rule::of(relation);
    let revision = direction.admitting();
    if let Some(minimum) = minimum(revision, descriptions)?
        && let Some(value) = descriptions.items(revision, minimum.max(2))?
    {
        descriptions.add(&mut rule, Candidate { value, direction })?;
    }
    Ok(rule)
}

/// The rule for an `items` description added to or removed from array data. The revision
/// with the description rejects the items of other types, so the candidates are arrays of
/// `0`, `""`, `true`, `{}` and `[]`, each repeated as often as the minimum count of the revision
/// that must admit it requires, and at least once. An `items` of both revisions that are not
/// both descriptions is not classified.
pub(super) fn items(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let described = [
        descriptions.member(Revision::Earlier, "items")?.is_some(),
        descriptions.member(Revision::Later, "items")?.is_some(),
    ];
    let (relation, direction) = match described {
        [false, true] => (Relation::Narrower, Direction::ClientSupplied),
        [true, false] => (Relation::Wider, Direction::ThingSupplied),
        _ => {
            let mut rule = Rule::of(Relation::Unknown);
            rule.unclassified = CLIENT | THING;
            return Ok(rule);
        }
    };
    let mut rule = Rule::of(relation);
    let revision = direction.admitting();
    if let Some(minimum) = minimum(revision, descriptions)? {
        let description = descriptions.description(revision);
        let arrays = representative::untyped(description, minimum.max(1), descriptions.allowance)
            .map_err(generating)?;
        for value in arrays {
            descriptions.add(&mut rule, Candidate { value, direction })?;
        }
    }
    Ok(rule)
}

/// The array that both revisions' corresponding `owners` admit around `value` as their items,
/// when the `admitting` revision admits `value` there: `value` repeated as often as the
/// minimum count of either revision requires, and at least once. `None` when a count is not a
/// count a generated value can hold, or when the other revision would reject the array for its
/// length alone: a `maxItems` below it, or unique items required of two or more.
pub(in crate::comparison) fn enclosing_items(
    owners: [EffectiveJson<'_>; 2],
    admitting: Revision,
    value: JsonValue,
    allowance: &mut ValueAllowance,
) -> Result<Option<JsonValue>, UndeterminedReason> {
    let mut descriptions = Descriptions::new(owners, 0, allowance);
    let mut length = 1;
    for revision in [Revision::Earlier, Revision::Later] {
        match minimum(revision, &mut descriptions)? {
            Some(count) => length = length.max(count),
            None => return Ok(None),
        }
    }
    let other = admitting.other();
    let limit = descriptions.limits().document_nodes;
    // A maximum count beyond the value capabilities exceeds any generated length.
    let fits = match bound("maxItems", other, &mut descriptions)? {
        Some(Some(maximum)) => count(maximum, limit).is_none_or(|maximum| maximum >= length),
        Some(None) => true,
        None => false,
    };
    if !fits || length > 1 && unique_items(other, &mut descriptions)? != Some(false) {
        return Ok(None);
    }
    representative::repeated(owners[admitting.index()], value, length, allowance)
        .map_err(generating)
}

/// One revision's effective `uniqueItems`, `false` where it is absent, or `None` when it is not
/// a boolean.
fn unique_items(
    revision: Revision,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Option<bool>, UndeterminedReason> {
    Ok(match descriptions.member(revision, "uniqueItems")? {
        Some(json) => json.boolean().present(),
        None => Some(false),
    })
}

/// One revision's minimum item count, `0` without one, or `None` when it is not a count a
/// generated value can hold.
fn minimum(
    revision: Revision,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Option<usize>, UndeterminedReason> {
    Ok(match bound("minItems", revision, descriptions)? {
        Some(Some(minimum)) => count(minimum, descriptions.limits().document_nodes),
        Some(None) => Some(0),
        None => None,
    })
}
