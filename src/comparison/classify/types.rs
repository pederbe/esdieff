//! A changed `type` relates the revisions only when one type contains the other. Every
//! integer is a number, and a description without a type admits values of every type, so
//! adding a type narrows and removing one widens. A fraction separates number data from
//! integer data, so it is the candidate witness of that change. Any other change of type
//! has no monotone relation. Every change between base types other than integer and number
//! may be witnessed by a representative value of the revision that must admit it.
use super::bounds::{self, End};
use super::{Candidate, Descriptions, Relation, Rule};
use crate::JsonValue;
use crate::comparison::{Direction, Revision, UndeterminedReason};

/// The base types of RFC 9880 Section 4.7.
const TYPES: [&str; 6] = ["number", "integer", "string", "boolean", "array", "object"];

pub(super) fn rule(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let mut types = [None, None];
    for revision in [Revision::Earlier, Revision::Later] {
        let Some(json) = descriptions.member(revision, "type")? else {
            continue;
        };
        match json.text().present() {
            Some(name) if TYPES.contains(&name) => types[revision.index()] = Some(name),
            _ => return Ok(Rule::of(Relation::Unknown)),
        }
    }
    // A type added as a whole may reject an earlier value, one removed as a whole admit a
    // later value, and unrelated types, such as string and boolean, admit no common value.
    let (relation, directions): (_, &[Direction]) = match types {
        [None, Some(_)] => (Relation::Narrower, &[Direction::ClientSupplied]),
        [Some(_), None] => (Relation::Wider, &[Direction::ThingSupplied]),
        [Some("integer"), Some("number")] => {
            return fractional(Relation::Wider, Direction::ThingSupplied, descriptions);
        }
        [Some("number"), Some("integer")] => {
            return fractional(Relation::Narrower, Direction::ClientSupplied, descriptions);
        }
        _ => (
            Relation::Unknown,
            &[Direction::ClientSupplied, Direction::ThingSupplied],
        ),
    };
    let mut rule = Rule::of(relation);
    for &direction in directions {
        descriptions.representatives(&mut rule, direction)?;
    }
    Ok(rule)
}

/// The rule between integer and number data, whose candidates are fractions that the
/// revision admitting fractions may admit: `0.5`, then one just inside each of its bounds.
fn fractional(
    relation: Relation,
    direction: Direction,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Rule, UndeterminedReason> {
    let revision = direction.admitting();
    let fractions: [Option<JsonValue>; 3] = [
        descriptions.number("0.5")?,
        bounds::fraction(End::Lower, revision, descriptions)?,
        bounds::fraction(End::Upper, revision, descriptions)?,
    ];
    let mut rule = Rule::of(relation);
    for value in fractions.into_iter().flatten() {
        descriptions.add(&mut rule, Candidate { value, direction })?;
    }
    Ok(rule)
}
