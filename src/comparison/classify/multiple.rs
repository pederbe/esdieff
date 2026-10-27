//! A `multipleOf` step admits the exact integer multiples of a positive number, and
//! `numeric::multiple` decides multiples exactly, following ADR 0012: Exact numbers and JSON
//! input policies. Every multiple of the earlier step is a multiple of the later one exactly
//! when the earlier step is a multiple of the later step, so a later step that divides the
//! earlier one widens and the converse narrows. Any other change does neither. An added step
//! narrows and a removed one widens.
//!
//! The candidates are the earlier step for Client-supplied data and the later step for
//! Thing-supplied data, each in its authored spelling. Half a step is never a multiple of it,
//! so half an added or removed step is the candidate, followed by `1` for an added or removed
//! step above 1, which integer data may admit. The representative values of the revision that
//! must admit a candidate come last.
use super::{Candidate, Descriptions, Relation, Rule};
use crate::comparison::{Direction, Revision, UndeterminedReason};
use crate::representative::bounds::half;
use crate::{ExactNumber, Field};
use std::cmp::Ordering;

pub(super) fn rule(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let (Some(earlier), Some(later)) = (
        step(Revision::Earlier, descriptions)?,
        step(Revision::Later, descriptions)?,
    ) else {
        return Ok(Rule::of(Relation::Unknown));
    };
    let (client, thing) = (Direction::ClientSupplied, Direction::ThingSupplied);
    let mut rule = Rule::of(Relation::Unknown);
    match (earlier, later) {
        (None, None) => rule.relation = Relation::Same,
        (None, Some(later)) => {
            rule.relation = Relation::Narrower;
            offer(&mut rule, &half(later), client, descriptions)?;
            if above_one(later, descriptions)? {
                offer(&mut rule, "1", client, descriptions)?;
            }
        }
        (Some(earlier), None) => {
            rule.relation = Relation::Wider;
            offer(&mut rule, &half(earlier), thing, descriptions)?;
            if above_one(earlier, descriptions)? {
                offer(&mut rule, "1", thing, descriptions)?;
            }
        }
        (Some(earlier), Some(later)) => {
            let wider = descriptions.multiple(earlier, later)?;
            let narrower = descriptions.multiple(later, earlier)?;
            rule.relation = match (wider, narrower) {
                (true, true) => Relation::Same,
                (true, false) => Relation::Wider,
                (false, true) => Relation::Narrower,
                (false, false) => Relation::Unknown,
            };
            if !wider {
                offer(&mut rule, earlier.lexeme(), client, descriptions)?;
            }
            if !narrower {
                offer(&mut rule, later.lexeme(), thing, descriptions)?;
            }
        }
    }
    for direction in [client, thing] {
        if !rule.relation.safe(direction.bit()) {
            descriptions.representatives(&mut rule, direction)?;
        }
    }
    Ok(rule)
}

/// One revision's step, `Some(None)` when it has none, or `None` when it is not a positive
/// exact number.
fn step<'c>(
    revision: Revision,
    descriptions: &mut Descriptions<'_, 'c>,
) -> Result<Option<Option<ExactNumber<'c>>>, UndeterminedReason> {
    let Some(json) = descriptions.member(revision, "multipleOf")? else {
        return Ok(Some(None));
    };
    Ok(match json.number() {
        Field::Present(step) if step.sign().is_gt() => Some(Some(step)),
        _ => None,
    })
}

/// Add the number with this lexeme as a candidate in `direction`, when it fits the number
/// capabilities.
fn offer(
    rule: &mut Rule,
    lexeme: &str,
    direction: Direction,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<(), UndeterminedReason> {
    if let Some(value) = descriptions.number(lexeme)? {
        descriptions.add(rule, Candidate { value, direction })?;
    }
    Ok(())
}

/// Whether `step` exceeds 1.
fn above_one(
    step: ExactNumber<'_>,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<bool, UndeterminedReason> {
    let Some(one) = descriptions.number("1")? else {
        return Ok(false);
    };
    let Field::Present(one) = one.view().number() else {
        return Ok(false);
    };
    Ok(descriptions.compare(step, one)? == Ordering::Greater)
}
