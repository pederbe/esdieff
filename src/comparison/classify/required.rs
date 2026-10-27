//! A map data description's `required` names the `properties` entries that every value
//! must have. Names compare as decoded strings, so a later revision that requires every name
//! the earlier one requires admits only values the earlier one admits, and the reverse. A
//! representative map holds exactly the entries its revision requires, so one of the earlier
//! revision lacks every newly required entry, and one of the later revision every dropped one.
use super::{Descriptions, Relation, Rule};
use crate::EffectiveJson;
use crate::comparison::{Direction, Revision, UndeterminedReason};

pub(super) fn rule(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let (Some(earlier), Some(later)) = (
        names(Revision::Earlier, descriptions)?,
        names(Revision::Later, descriptions)?,
    ) else {
        return Ok(Rule::of(Relation::Unknown));
    };
    let added = !descriptions.within(later, earlier)?;
    let dropped = !descriptions.within(earlier, later)?;
    let mut rule = Rule::of(match (added, dropped) {
        (false, false) => Relation::Same,
        (true, false) => Relation::Narrower,
        (false, true) => Relation::Wider,
        (true, true) => Relation::Unknown,
    });
    // Other constraints, such as an entry whose representative value its own description
    // rejects, may still leave the representative map without a witness.
    for (changed, direction) in [
        (added, Direction::ClientSupplied),
        (dropped, Direction::ThingSupplied),
    ] {
        if changed {
            descriptions.representatives(&mut rule, direction)?;
        }
    }
    Ok(rule)
}

/// One revision's `required` names, `Some(None)` when it has none, or `None` when they are
/// not all decoded strings.
fn names<'c>(
    revision: Revision,
    descriptions: &mut Descriptions<'_, 'c>,
) -> Result<Option<Option<EffectiveJson<'c>>>, UndeterminedReason> {
    let Some(json) = descriptions.member(revision, "required")? else {
        return Ok(Some(None));
    };
    Ok(descriptions.strings(json)?.then_some(Some(json)))
}
