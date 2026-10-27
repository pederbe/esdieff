//! A `const`, a `pattern` and a checked `format` each only restrict the values of their
//! description, so an added restriction narrows and a removed one widens. A changed
//! restriction does neither. Each candidate is a representative value of the revision that
//! must admit it: the earlier revision's for Client-supplied data, when the later revision has
//! the restriction, and the later revision's for Thing-supplied data, when the earlier one has
//! it.
//!
//! A `const` beside an enum leaves it as it is, since each enum value replaces it, following
//! ADR 0024: Effective alternatives and nullable characteristics, so an added constant still
//! only narrows. A changed constant is the first representative value of its own revision.
//! Whether one pattern admits every string another admits is not decided, so a changed
//! pattern is settled by its candidates alone. A `format` restricts only the six formats that
//! Sample-value checking checks beside an explicit `type: "string"`, so a change involving any
//! other format is not classified. Of those, every `uri` is a `uri-reference`, which
//! Sample-value checking reads as the same URI, so `uri` replaced by `uri-reference` widens and
//! the reverse narrows.
use super::{Descriptions, Relation, Rule};
use crate::comparison::{CLIENT, DIRECTIONS, Revision, THING, UndeterminedReason};
use crate::scalar::{ResourceFormat, TemporalFormat};

pub(super) fn rule(
    key: &str,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Rule, UndeterminedReason> {
    let restricted = [
        descriptions.member(Revision::Earlier, key)?.is_some(),
        descriptions.member(Revision::Later, key)?.is_some(),
    ];
    let relation = match restricted {
        [false, true] => Relation::Narrower,
        [true, false] => Relation::Wider,
        _ => Relation::Unknown,
    };
    represented(relation, descriptions)
}

/// The restriction rule for `format`, when every revision that has one has a checked format.
pub(super) fn format(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let mut resources = [None, None];
    for revision in [Revision::Earlier, Revision::Later] {
        let Some(format) = descriptions.member(revision, "format")? else {
            continue;
        };
        let name = format.text().present();
        let checked = name.is_some_and(|name| {
            TemporalFormat::from_text(name).is_some() || ResourceFormat::from_text(name).is_some()
        });
        let typed = descriptions.member(revision, "type")?;
        if !checked || typed.and_then(|json| json.text().present()) != Some("string") {
            let mut rule = Rule::of(Relation::Unknown);
            rule.unclassified = CLIENT | THING;
            return Ok(rule);
        }
        resources[revision.index()] = name.and_then(ResourceFormat::from_text);
    }
    match resources {
        [Some(ResourceFormat::Uri), Some(ResourceFormat::Reference)] => {
            represented(Relation::Wider, descriptions)
        }
        [Some(ResourceFormat::Reference), Some(ResourceFormat::Uri)] => {
            represented(Relation::Narrower, descriptions)
        }
        _ => rule("format", descriptions),
    }
}

/// A rule with this relation whose candidates are the representative values of the revision
/// that must admit a candidate, in each direction the relation does not make safe: a later
/// restriction may reject an earlier value, and an earlier one a later value.
fn represented(
    relation: Relation,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Rule, UndeterminedReason> {
    let mut rule = Rule::of(relation);
    for direction in DIRECTIONS {
        if !relation.safe(direction.bit()) {
            descriptions.representatives(&mut rule, direction)?;
        }
    }
    Ok(rule)
}
