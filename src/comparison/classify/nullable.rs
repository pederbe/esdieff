//! The effective Nullable characteristic decides each revision, with the `true` of its
//! Specified default characteristic where `nullable` is absent. Data that stops being
//! Nullable narrows and data that becomes Nullable widens, and `null` is the candidate
//! witness of either change.
use super::{Candidate, Descriptions, Relation, Rule};
use crate::comparison::{Direction, Revision, UndeterminedReason};

pub(super) fn rule(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let mut nullable = [Some(true); 2];
    for revision in [Revision::Earlier, Revision::Later] {
        if let Some(json) = descriptions.member(revision, "nullable")? {
            nullable[revision.index()] = json.boolean().present();
        }
    }
    let (relation, direction) = match nullable {
        [Some(true), Some(false)] => (Relation::Narrower, Direction::ClientSupplied),
        [Some(false), Some(true)] => (Relation::Wider, Direction::ThingSupplied),
        _ => return Ok(Rule::of(Relation::Unknown)),
    };
    let mut rule = Rule::of(relation);
    if let Some(value) = descriptions.null()? {
        descriptions.add(&mut rule, Candidate { value, direction })?;
    }
    Ok(rule)
}
