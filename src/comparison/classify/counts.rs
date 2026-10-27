//! `minLength` and `maxLength` bound how many Unicode scalar values a string holds, and
//! `minItems` and `maxItems` how many items an array holds. Each is an inclusive nonnegative
//! count that compares as an exact number, following ADR 0022: Scalar constraints and
//! characteristic states. An absent minimum reads as 0 and an absent maximum as unbounded, so
//! a raised minimum, a lowered maximum or an added maximum narrows, and the reverse widens.
//!
//! The first candidate holds the count just across the tighter bound, one short of a minimum
//! and one over a maximum: a string of `a`, or an array of copies of the first representative
//! value of the `items` of the revision that must admit it. A count beyond the value
//! capabilities is never generated. A string of `a` often meets another constraint of the
//! description, such as an enum, only by chance, so the representative values of the
//! revision that must admit a candidate follow a length candidate.
use super::bounds::End;
use super::{Candidate, Descriptions, Relation, Rule};
use crate::comparison::{Direction, Revision, UndeterminedReason};
use crate::representative::count;
use crate::{ExactNumber, Field, JsonValue};
use std::cmp::Ordering;

/// The rule for a changed `minLength` at the lower end or `maxLength` at the upper end.
pub(super) fn length(
    end: End,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Rule, UndeterminedReason> {
    let key = match end {
        End::Lower => "minLength",
        End::Upper => "maxLength",
    };
    let limit = descriptions.limits().document_bytes;
    let mut rule = counted(key, end, limit, descriptions, |descriptions, _, length| {
        descriptions.text(&"a".repeat(length))
    })?;
    let direction = match rule.relation {
        Relation::Narrower => Direction::ClientSupplied,
        Relation::Wider => Direction::ThingSupplied,
        _ => return Ok(rule),
    };
    descriptions.representatives(&mut rule, direction)?;
    Ok(rule)
}

/// The rule for a changed `minItems` at the lower end or `maxItems` at the upper end.
pub(super) fn items(
    end: End,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Rule, UndeterminedReason> {
    let key = match end {
        End::Lower => "minItems",
        End::Upper => "maxItems",
    };
    let limit = descriptions.limits().document_nodes;
    counted(
        key,
        end,
        limit,
        descriptions,
        |descriptions, direction, length| descriptions.items(direction.admitting(), length),
    )
}

/// The rule for a changed count `key` at `end`, whose candidate `candidate` builds with a
/// count of at most `limit` for the revision that must admit it in a direction.
fn counted(
    key: &str,
    end: End,
    limit: usize,
    descriptions: &mut Descriptions<'_, '_>,
    candidate: impl FnOnce(
        &mut Descriptions<'_, '_>,
        Direction,
        usize,
    ) -> Result<Option<JsonValue>, UndeterminedReason>,
) -> Result<Rule, UndeterminedReason> {
    let (Some(earlier), Some(later)) = (
        bound(key, Revision::Earlier, descriptions)?,
        bound(key, Revision::Later, descriptions)?,
    ) else {
        return Ok(Rule::of(Relation::Unknown));
    };
    let relation = relation(end, [earlier, later], descriptions)?;
    let (tighter, direction) = match relation {
        Relation::Narrower => (later, Direction::ClientSupplied),
        Relation::Wider => (earlier, Direction::ThingSupplied),
        _ => return Ok(Rule::of(relation)),
    };
    let mut rule = Rule::of(relation);
    let length = tighter
        .and_then(|bound| count(bound, limit))
        .and_then(|count| match end {
            End::Lower => count.checked_sub(1),
            End::Upper => count.checked_add(1),
        });
    if let Some(length) = length
        && let Some(value) = candidate(descriptions, direction, length)?
    {
        descriptions.add(&mut rule, Candidate { value, direction })?;
    }
    Ok(rule)
}

/// How the later revision's count at `end` relates its admitted values to the earlier
/// revision's, with an absent minimum read as 0 and an absent maximum as unbounded: `Same`,
/// `Narrower` or `Wider`.
pub(super) fn relation(
    end: End,
    counts: [Option<ExactNumber<'_>>; 2],
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Relation, UndeterminedReason> {
    // The later count against the earlier one.
    let order = match (counts, end) {
        ([Some(earlier), Some(later)], _) => descriptions.compare(later, earlier)?,
        ([None, None], _) => Ordering::Equal,
        ([None, Some(later)], End::Lower) => later.sign(),
        ([Some(earlier), None], End::Lower) => earlier.sign().reverse(),
        ([None, Some(_)], End::Upper) => Ordering::Less,
        ([Some(_), None], End::Upper) => Ordering::Greater,
    };
    let inward = if end.inward_up() {
        order
    } else {
        order.reverse()
    };
    Ok(match inward {
        Ordering::Equal => Relation::Same,
        Ordering::Greater => Relation::Narrower,
        Ordering::Less => Relation::Wider,
    })
}

/// One revision's count, `Some(None)` when it has none, or `None` when it is not an exact
/// nonnegative integer.
pub(super) fn bound<'c>(
    key: &str,
    revision: Revision,
    descriptions: &mut Descriptions<'_, 'c>,
) -> Result<Option<Option<ExactNumber<'c>>>, UndeterminedReason> {
    let Some(json) = descriptions.member(revision, key)? else {
        return Ok(Some(None));
    };
    Ok(match json.number() {
        Field::Present(count) if count.is_integer() && count.sign() != Ordering::Less => {
            Some(Some(count))
        }
        _ => None,
    })
}
