//! Enum values correspond by value, following ADR 0039: Client-side revision comparison, so
//! reordering them changes nothing. A value only the earlier revision lists may break
//! Client-supplied data, and one only the later revision lists Thing-supplied data, so each
//! is a candidate witness in that direction. An enum that only one revision has restricts
//! that revision alone.
use super::{Candidate, Descriptions, Relation, Rule};
use crate::EffectiveJson;
use crate::comparison::{CLIENT, Direction, Revision, THING, UndeterminedReason};

pub(super) fn rule(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let lists = [
        descriptions.member(Revision::Earlier, "enum")?,
        descriptions.member(Revision::Later, "enum")?,
    ];
    for revision in [Revision::Earlier, Revision::Later] {
        let Some(list) = lists[revision.index()] else {
            continue;
        };
        if !decoded(list, descriptions)? || beside(revision, descriptions)? {
            return Ok(Rule::of(Relation::Unknown));
        }
    }
    let [Some(earlier), Some(later)] = lists else {
        // An enum added as a whole may reject a representative value of the earlier
        // revision, and a removed one admit one of the later revision.
        let (relation, direction) = match lists {
            [None, Some(_)] => (Relation::Narrower, Direction::ClientSupplied),
            [Some(_), None] => (Relation::Wider, Direction::ThingSupplied),
            _ => return Ok(Rule::of(Relation::Same)),
        };
        let mut rule = Rule::of(relation);
        descriptions.representatives(&mut rule, direction)?;
        return Ok(rule);
    };
    let mut rule = Rule::of(Relation::Same);
    let mut directions = 0;
    for (values, others, direction) in [
        (earlier, later, Direction::ClientSupplied),
        (later, earlier, Direction::ThingSupplied),
    ] {
        for value in values.children() {
            let text = value.text().present().expect("decoded string");
            if descriptions.contains(Some(others), text)? {
                continue;
            }
            directions |= direction.bit();
            if let Some(value) = descriptions.text(text)? {
                descriptions.add(&mut rule, Candidate { value, direction })?;
            }
        }
    }
    rule.relation = match directions {
        0 => Relation::Same,
        CLIENT => Relation::Narrower,
        THING => Relation::Wider,
        _ => Relation::Unknown,
    };
    Ok(rule)
}

/// Whether an enum is an available nonempty array of decoded strings, the only enum that
/// derives alternatives.
fn decoded(
    json: EffectiveJson<'_>,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<bool, UndeterminedReason> {
    Ok(descriptions.strings(json)? && json.children().next().is_some())
}

/// Whether a revision's enum sits beside a `const`, which each of its alternatives replaces
/// with its own value, or beside an `sdfChoice`, which leaves no alternatives, following
/// ADR 0024: Effective alternatives and nullable characteristics. Neither enum restricts the
/// admitted values alone.
fn beside(
    revision: Revision,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<bool, UndeterminedReason> {
    Ok(descriptions.member(revision, "const")?.is_some()
        || descriptions.member(revision, "sdfChoice")?.is_some())
}
