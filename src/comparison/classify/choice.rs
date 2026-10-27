//! Choices compare by the effective alternatives they derive, following ADR 0039:
//! Client-side revision comparison and ADR 0024: Effective alternatives and nullable
//! characteristics. Alternatives correspond by Given Name. A choice admits the values that
//! any of its alternatives admits, so a removed alternative narrows its description and an
//! added one widens it, with the representative values of that alternative as candidates.
//!
//! Each alternative inherits every quality of its description that its branch does not
//! replace. A changed quality of the description therefore changes each alternative that
//! inherits it in both revisions, beside that alternative's own qualities, so its rule is
//! judged on each of those alternatives, and a quality that every alternative replaces
//! changes nothing. A quality that a branch states in either revision is compared at that
//! alternative instead, so each change is judged once.
use super::{Descriptions, Relation, Rule, RuleFn};
use crate::allowance::{OperationBudget, push};
use crate::comparison::{CLIENT, Direction, Revision, THING, UndeterminedReason, read};
use crate::sample::ValueAllowance;
use crate::{AlternativeId, EffectiveJson, Field};

/// The derived alternatives of two corresponding descriptions' choices.
pub(in crate::comparison) enum Choices<'c> {
    /// A revision's description has no choice.
    Absent,
    /// Both have a choice, but a revision's alternatives are not established, as when an enum
    /// sits beside the choice or a branch is unavailable.
    Unestablished,
    /// Both revisions' alternatives, paired by Given Name: each earlier alternative with its
    /// later namesake, then the later alternatives without one.
    Paired(Vec<[Option<EffectiveJson<'c>>; 2]>),
}

/// The alternatives of the choices of corresponding descriptions, charged to the one
/// operation allowance.
pub(in crate::comparison) fn choices<'c>(
    pair: [EffectiveJson<'c>; 2],
    budget: &mut OperationBudget,
) -> Result<Choices<'c>, UndeterminedReason> {
    for description in pair {
        if read::member(description, "sdfChoice", budget)?.is_none() {
            return Ok(Choices::Absent);
        }
    }
    let mut collections = [Vec::new(), Vec::new()];
    for (description, collection) in pair.into_iter().zip(&mut collections) {
        let Field::Present(derived) = description.alternatives() else {
            return Ok(Choices::Unestablished);
        };
        for (id, alternative) in derived {
            read::work(budget, 1)?;
            let (AlternativeId::Choice(name), Field::Present(alternative)) = (id, alternative)
            else {
                return Ok(Choices::Unestablished);
            };
            kept(collection, (name, alternative.0), budget)?;
        }
    }
    let [earlier, later] = collections;
    let mut paired = Vec::new();
    for &(name, alternative) in &earlier {
        let namesake = named(&later, name, budget)?;
        kept(&mut paired, [Some(alternative), namesake], budget)?;
    }
    for &(name, alternative) in &later {
        if named(&earlier, name, budget)?.is_none() {
            kept(&mut paired, [None, Some(alternative)], budget)?;
        }
    }
    Ok(Choices::Paired(paired))
}

/// Whether corresponding alternatives both inherit the quality whose members they have, so
/// that their description compares it.
pub(in crate::comparison) fn inherited(
    alternatives: [EffectiveJson<'_>; 2],
    quality: &str,
    members: [Option<EffectiveJson<'_>>; 2],
    budget: &mut OperationBudget,
) -> Result<bool, UndeterminedReason> {
    for (alternative, member) in alternatives.into_iter().zip(members) {
        if member.is_some() && !inherits(alternative, quality, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The rule of an alternative that only `revision` has. A removed alternative narrows its
/// description and an added one widens it, so its representative values are the candidates
/// in the direction that breaks.
pub(in crate::comparison) fn alternative(
    alternative: EffectiveJson<'_>,
    revision: Revision,
    directions: u8,
    allowance: &mut ValueAllowance,
) -> Result<Rule, UndeterminedReason> {
    let (relation, direction) = match revision {
        Revision::Earlier => (Relation::Narrower, Direction::ClientSupplied),
        Revision::Later => (Relation::Wider, Direction::ThingSupplied),
    };
    let mut rule = Rule::of(relation);
    let mut descriptions = Descriptions::new([alternative; 2], directions, allowance);
    descriptions.representatives(&mut rule, direction)?;
    Ok(rule)
}

/// A choice that only one revision has may admit values the other revision's description
/// rejects, and reject ones it admits, so the change neither narrows nor widens, and
/// representative values of each revision are its candidates. A choice of both revisions
/// reaches this rule only when a revision's alternatives are not established, and it is not
/// classified.
pub(super) fn rule(descriptions: &mut Descriptions<'_, '_>) -> Result<Rule, UndeterminedReason> {
    let mut rule = Rule::of(Relation::Unknown);
    if descriptions
        .member(Revision::Earlier, "sdfChoice")?
        .is_some()
        && descriptions.member(Revision::Later, "sdfChoice")?.is_some()
    {
        rule.unclassified = CLIENT | THING;
        return Ok(rule);
    }
    for direction in [Direction::ClientSupplied, Direction::ThingSupplied] {
        descriptions.representatives(&mut rule, direction)?;
    }
    Ok(rule)
}

/// The rule of a changed quality, judged on each corresponding alternative that inherits it
/// in both revisions when both descriptions have a choice. A quality beside a choice whose
/// alternatives are not established in both revisions neither narrows nor widens.
pub(super) fn judged(
    rule: RuleFn,
    quality: &str,
    descriptions: &mut Descriptions<'_, '_>,
) -> Result<Rule, UndeterminedReason> {
    let budget = &mut descriptions.allowance.budget;
    let alternatives = match choices(descriptions.descriptions, budget)? {
        Choices::Absent => return rule(descriptions),
        Choices::Unestablished => {
            let mut found = rule(descriptions)?;
            found.relation = Relation::Unknown;
            return Ok(found);
        }
        Choices::Paired(alternatives) => alternatives,
    };
    let mut judged = Rule::of(Relation::Same);
    // An alternative that only one revision has is a change of its own.
    for [earlier, later] in alternatives {
        let (Some(earlier), Some(later)) = (earlier, later) else {
            continue;
        };
        let budget = &mut descriptions.allowance.budget;
        if !(inherits(earlier, quality, budget)? && inherits(later, quality, budget)?) {
            continue;
        }
        let mut alternative = Descriptions {
            descriptions: [earlier, later],
            directions: descriptions.directions,
            allowance: &mut *descriptions.allowance,
        };
        let found = self::judged(rule, quality, &mut alternative)?;
        judged.relation = combined(judged.relation, found.relation);
        judged.unclassified |= found.unclassified;
        for candidate in found.candidates {
            descriptions.add(&mut judged, candidate)?;
        }
    }
    Ok(judged)
}

/// Whether a quality of corresponding descriptions reaches their values in both revisions:
/// always without a choice, and beside one only through a corresponding alternative that
/// inherits it in both revisions and that it reaches in turn. `None` when a revision's
/// alternatives are not established.
pub(super) fn reaches(
    pair: [EffectiveJson<'_>; 2],
    quality: &str,
    budget: &mut OperationBudget,
) -> Result<Option<bool>, UndeterminedReason> {
    let alternatives = match choices(pair, budget)? {
        Choices::Absent => return Ok(Some(true)),
        Choices::Unestablished => return Ok(None),
        Choices::Paired(alternatives) => alternatives,
    };
    let mut reached = Some(false);
    // An alternative that only one revision has is a change of its own.
    for [earlier, later] in alternatives {
        let (Some(earlier), Some(later)) = (earlier, later) else {
            continue;
        };
        if !(inherits(earlier, quality, budget)? && inherits(later, quality, budget)?) {
            continue;
        }
        match reaches([earlier, later], quality, budget)? {
            Some(true) => return Ok(Some(true)),
            Some(false) => {}
            None => reached = None,
        }
    }
    Ok(reached)
}

/// How the later revision relates under two changes that each relate it so.
fn combined(a: Relation, b: Relation) -> Relation {
    match (a, b) {
        (Relation::Same, other) | (other, Relation::Same) => other,
        _ if a == b => a,
        _ => Relation::Unknown,
    }
}

/// Whether an alternative inherits `quality` from its description, because its authored
/// branch, the member of the choice with its Given Name, leaves it out. An alternative
/// without that branch is taken to state the quality itself.
pub(super) fn inherits(
    alternative: EffectiveJson<'_>,
    quality: &str,
    budget: &mut OperationBudget,
) -> Result<bool, UndeterminedReason> {
    let (Some(choice), Some(name)) = (alternative.n().parent, alternative.n().key) else {
        return Ok(false);
    };
    let choice = EffectiveJson {
        model: alternative.model,
        node: choice,
    };
    Ok(match read::member(choice, name, budget)? {
        Some(branch) => read::member(branch, quality, budget)?.is_none(),
        None => false,
    })
}

/// The alternative with this Given Name among `alternatives`.
fn named<'c>(
    alternatives: &[(&str, EffectiveJson<'c>)],
    name: &str,
    budget: &mut OperationBudget,
) -> Result<Option<EffectiveJson<'c>>, UndeterminedReason> {
    for &(other, alternative) in alternatives {
        read::work(budget, name.len().min(other.len()) + 1)?;
        if other == name {
            return Ok(Some(alternative));
        }
    }
    Ok(None)
}

fn kept<T>(
    items: &mut Vec<T>,
    item: T,
    budget: &mut OperationBudget,
) -> Result<(), UndeterminedReason> {
    push(items, item, &mut budget.allocation)
        .map_err(|_| UndeterminedReason::limit("comparison allocation limit"))
}
