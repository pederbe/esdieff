//! Numeric bounds compare as exact numbers, following ADR 0012: Exact numbers and JSON
//! input policies. `minimum` and `exclusiveMinimum` together set a description's lower
//! bound, and `maximum` and `exclusiveMaximum` its upper bound, so a change to either
//! quality of a pair is judged by the effective bound the pair leaves. An exclusive bound
//! replacing an inclusive one at the same value therefore narrows by that value alone.
use super::{Candidate, Descriptions, Relation, Rule};
use crate::comparison::{Direction, Revision, UndeterminedReason};
pub(super) use crate::representative::bounds::End;
use crate::representative::bounds::{step, step_power};
use crate::{ExactNumber, Field, JsonValue};
use std::cmp::Ordering;

#[derive(Clone, Copy)]
struct Bound<'c> {
    value: ExactNumber<'c>,
    exclusive: bool,
}

pub(super) fn rule(
    end: End,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Rule, UndeterminedReason> {
    let (Some(earlier), Some(later)) = (
        effective_bound(end, Revision::Earlier, descriptions)?,
        effective_bound(end, Revision::Later, descriptions)?,
    ) else {
        return Ok(Rule::of(Relation::Unknown));
    };
    // The tighter bound, the wider one when there is one, how the later revision relates,
    // and the direction that the values between them break.
    let (tighter, wider, relation, direction) = match (earlier, later) {
        (None, None) => return Ok(Rule::of(Relation::Same)),
        (None, Some(later)) => (later, None, Relation::Narrower, Direction::ClientSupplied),
        (Some(earlier), None) => (earlier, None, Relation::Wider, Direction::ThingSupplied),
        (Some(earlier), Some(later)) => {
            if is_tighter(end, later, earlier, descriptions)? {
                (
                    later,
                    Some(earlier),
                    Relation::Narrower,
                    Direction::ClientSupplied,
                )
            } else if is_tighter(end, earlier, later, descriptions)? {
                (
                    earlier,
                    Some(later),
                    Relation::Wider,
                    Direction::ThingSupplied,
                )
            } else {
                return Ok(Rule::of(Relation::Same));
            }
        }
    };
    // Values the wider bound admits and the tighter one rejects: the tighter bound when it
    // is exclusive, the wider bound when it is inclusive, each in its authored spelling,
    // and otherwise a value just across an open edge of that gap.
    let mut rule = Rule::of(relation);
    let edges = [
        Some(tighter).filter(|bound| bound.exclusive),
        wider.filter(|bound| !bound.exclusive),
    ];
    for bound in edges.into_iter().flatten() {
        if let Some(value) = descriptions.number(bound.value.lexeme())? {
            descriptions.add(&mut rule, Candidate { value, direction })?;
        }
    }
    if edges.iter().all(Option::is_none)
        && let Some(value) = inside(end, tighter, wider, descriptions)?
    {
        descriptions.add(&mut rule, Candidate { value, direction })?;
    }
    Ok(rule)
}

/// A number that is not an integer just inside one revision's bound at `end`, or `None`
/// without an exact bound: an inclusive bound that is not an integer itself, and otherwise
/// half a unit inside the bound, of 1 for an integer and of its last digit for another.
pub(super) fn fraction(
    end: End,
    revision: Revision,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Option<JsonValue>, UndeterminedReason> {
    let Some(Some(bound)) = effective_bound(end, revision, descriptions)? else {
        return Ok(None);
    };
    let value = bound.value;
    if !bound.exclusive && !value.is_integer() {
        return descriptions.number(value.lexeme());
    }
    let power = value.decimal.power.min(0);
    let digits = value.decimal.coefficient.len() as u128 + (value.decimal.power - power) as u128;
    if digits >= descriptions.coefficient_digits() as u128 {
        return Ok(None);
    }
    descriptions.number(&step(value, end.inward_up(), power, true))
}

/// One revision's effective bound at `end`, `Some(None)` when it has none, or `None` when a
/// bound is not an exact number.
fn effective_bound<'c>(
    end: End,
    revision: Revision,
    descriptions: &mut Descriptions<'_, 'c>,
) -> Result<Option<Option<Bound<'c>>>, UndeterminedReason> {
    let mut tightest = None;
    for (key, exclusive) in end.qualities() {
        let Some(json) = descriptions.member(revision, key)? else {
            continue;
        };
        let Field::Present(value) = json.number() else {
            return Ok(None);
        };
        let bound = Bound { value, exclusive };
        tightest = Some(match tightest {
            Some(other) if !is_tighter(end, bound, other, descriptions)? => other,
            _ => bound,
        });
    }
    Ok(Some(tightest))
}

/// Whether bound `a` admits fewer values than bound `b` at `end`.
fn is_tighter(
    end: End,
    a: Bound<'_>,
    b: Bound<'_>,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<bool, UndeterminedReason> {
    let order = descriptions.compare(a.value, b.value)?;
    let inward = if end.inward_up() {
        order
    } else {
        order.reverse()
    };
    Ok(match inward {
        Ordering::Greater => true,
        Ordering::Equal => a.exclusive && !b.exclusive,
        Ordering::Less => false,
    })
}

/// A value strictly inside a gap whose edges are both open: one unit of the finer edge's
/// last digit across that edge, or half a unit when one unit reaches the other edge. Without
/// a wider bound the gap is unbounded outward, and the unit is at most 1 where that fits, so
/// that integer data gets an integer.
fn inside(
    end: End,
    tighter: Bound<'_>,
    wider: Option<Bound<'_>>,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Option<JsonValue>, UndeterminedReason> {
    let outward = !end.inward_up();
    let Some(wider) = wider else {
        let power = step_power(tighter.value, descriptions.coefficient_digits());
        return descriptions.number(&step(tighter.value, outward, power, false));
    };
    let power = wider.value.decimal.power.min(tighter.value.decimal.power);
    let (from, other, up) = if wider.value.decimal.power == power {
        (wider, tighter, !outward)
    } else {
        (tighter, wider, outward)
    };
    let Some(value) = descriptions.number(&step(from.value, up, power, false))? else {
        return Ok(None);
    };
    let Field::Present(number) = value.view().number() else {
        return Ok(None);
    };
    if descriptions.compare(number, other.value)? != Ordering::Equal {
        return Ok(Some(value));
    }
    descriptions.number(&step(from.value, up, power, true))
}
