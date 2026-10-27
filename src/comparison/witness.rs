//! Settle the changed qualities of one data description together, following ADR 0039:
//! Client-side revision comparison.
//!
//! A change is compatible only with a Compatibility proof: every changed quality of its
//! description loosens in the safe direction for each Client direction, by its monotone
//! rule, and nothing else in the description changed apart from documentary qualities.
//! Otherwise the candidate Breaking witnesses of the change are settled by Sample-value
//! checking against both revisions on the one operation allowance. Only admission by one
//! revision and rejection by the other, in the breaking direction, establishes a witness.
//! Everything else stays undetermined, including a check that runs out of the allowance.
//! A candidate for a map `properties` entry or an array's `items` is placed there inside its
//! enclosing maps and arrays, beside representative values of the entries that either
//! revision requires and repeated as often as the `minItems` of either revision requires, and
//! checked against the whole root description, so enclosing constraints still apply to it.
//! Only enclosing content that both revisions read alike may surround it, so that its witness
//! shows the change it was made for. A candidate for a choice
//! alternative stands where the description holding the choice does. Data that the two
//! revisions carry in no common Client direction reaches no Client in both, so all its
//! changes are compatible.
use super::classify::{self, Candidate, DataDirections, Descriptions, Rule};
use super::{
    DifferenceKind, DifferenceSubject, Direction, Revision, Side, UndeterminedReason, Verdict,
};
use crate::allowance::charge;
use crate::role::Role;
use crate::sample::{
    CheckedValue, ValueAdmission, ValueAllowance, ValueInterpretations, check_generated,
};
use crate::{Admission, EffectiveDataDescription, EffectiveJson, JsonValue};

/// A value that one revision admits and the other rejects, in the direction that makes its
/// difference breaking, as Sample-value checking establishes against both revisions.
#[derive(Clone, Debug)]
pub struct Witness {
    value: String,
    direction: Direction,
    admissions: [Admission; 2],
    /// The value interpretations both admissions relied on, which the report names.
    pub(super) interpretations: ValueInterpretations,
}
impl Witness {
    /// The value as JSON text. A number taken from either revision keeps its spelling there.
    pub fn value(&self) -> &str {
        &self.value
    }
    /// Client-supplied when the earlier revision admits the value and the later one rejects
    /// it, and Thing-supplied for the reverse.
    pub fn direction(&self) -> Direction {
        self.direction
    }
    /// Sample-value checking's admission of the value in one revision.
    pub fn admission(&self, revision: Revision) -> Admission {
        self.admissions[revision.index()]
    }
}

/// A changed data quality, or a map `properties` entry that only one revision has, held until
/// the rest of its description is compared.
pub(super) struct Change<'a> {
    /// `Changed` for a quality, and `Removed` or `Added` for an entry.
    pub(super) kind: DifferenceKind,
    pub(super) subject: DifferenceSubject<'a>,
    pub(super) directions: u8,
    pub(super) sides: [Option<Side<'a>>; 2],
    /// The classification the quality has without a monotone rule.
    pub(super) verdict: Verdict,
    pub(super) witness: Option<Witness>,
    rule: Option<Result<Rule, UndeterminedReason>>,
}
impl<'a> Change<'a> {
    pub(super) fn new(
        subject: DifferenceSubject<'a>,
        directions: u8,
        verdict: Verdict,
        sides: [Option<Side<'a>>; 2],
    ) -> Self {
        Self {
            kind: DifferenceKind::Changed,
            subject,
            directions,
            sides,
            verdict,
            witness: None,
            rule: None,
        }
    }
    /// The change with the rule its walk already established, such as for an alternative
    /// that only one revision's choice has.
    pub(super) fn ruled(mut self, rule: Result<Rule, UndeterminedReason>) -> Self {
        self.rule = Some(rule);
        self
    }
}

/// The data description under comparison.
#[derive(Default)]
pub(super) struct Batch<'a> {
    pub(super) changes: Vec<Change<'a>>,
    /// Content of the description was unavailable, so nothing in it is proven.
    pub(super) unavailable: bool,
    /// The description is a Direct items description carrying a quality outside its
    /// vocabulary, so its meaning is not established and none of its qualities has a monotone
    /// rule, following ADR 0024: Effective alternatives and nullable characteristics.
    pub(super) opaque: bool,
}

/// Classify every change of a description whose two revisions are `descriptions`, inside the
/// root descriptions `roots` that Sample-value checking selects.
pub(super) fn settle(
    batch: &mut Batch<'_>,
    roots: [EffectiveJson<'_>; 2],
    descriptions: [EffectiveJson<'_>; 2],
    data: DataDirections,
    allowance: &mut ValueAllowance,
) {
    // Data that the revisions carry in no common direction reaches no Client, so none of
    // its changes can break one. The characteristic changes decide.
    if data.directions == 0 && data.unknown.is_none() {
        for change in &mut batch.changes {
            change.verdict = Verdict::Compatible;
        }
        return;
    }
    let directions = data.directions;
    let mut proven = !batch.unavailable;
    let mut reading = Descriptions::new(descriptions, directions, allowance);
    for change in &mut batch.changes {
        if change.rule.is_none()
            && change.verdict == Verdict::NOT_CLASSIFIED
            && !batch.opaque
            && let DifferenceSubject::Quality(quality) = change.subject
        {
            change.rule = classify::ruled(quality, &mut reading);
        }
        proven &= match &change.rule {
            Some(Ok(rule)) => rule.relation.safe(directions),
            Some(Err(_)) => false,
            // Only a documentary change leaves the admitted values as they are.
            None => change.verdict == Verdict::Compatible,
        };
    }
    let mut checks = Checks {
        roots,
        descriptions,
        allowance,
    };
    for change in &mut batch.changes {
        match change.rule.take() {
            None => {}
            Some(Err(reason)) => change.verdict = Verdict::Undetermined(reason),
            Some(Ok(_)) if proven => change.verdict = Verdict::Compatible,
            Some(Ok(rule)) => (change.verdict, change.witness) = checks.search(rule, directions),
        }
    }
}

/// Leave the changes of a description whose comparison stopped for `reason` unsettled. A
/// quality without a monotone rule stays not classified, as settling would leave it while
/// its data reaches a Client. Any other change that waited on a proof or a witness is
/// undetermined for the stop.
pub(super) fn stop(batch: &mut Batch<'_>, reason: UndeterminedReason) {
    for change in &mut batch.changes {
        let ruled = change.rule.is_some()
            || match change.subject {
                DifferenceSubject::Quality(quality) => {
                    !batch.opaque && classify::rule(quality).is_some()
                }
                _ => false,
            };
        if change.verdict == Verdict::NOT_CLASSIFIED && (ruled || change.directions == 0) {
            change.verdict = Verdict::Undetermined(reason);
        }
    }
}

/// Both revisions' root descriptions, the descriptions inside them whose changes are
/// settled, and the allowance each witness check borrows.
struct Checks<'s, 'c> {
    roots: [EffectiveJson<'c>; 2],
    descriptions: [EffectiveJson<'c>; 2],
    allowance: &'s mut ValueAllowance,
}
impl Checks<'_, '_> {
    /// The first candidate that is a Breaking witness in one of `directions`, or why there
    /// is none. Only a witness relies on the extension vocabulary its checks set aside,
    /// following ADR 0037: Undeclared extension vocabulary.
    fn search(&mut self, rule: Rule, directions: u8) -> (Verdict, Option<Witness>) {
        let aside = self.allowance.budget.set_aside;
        let found = self.candidates(rule, directions);
        if found.1.is_none() {
            self.allowance.budget.set_aside = aside;
        }
        found
    }
    fn candidates(&mut self, rule: Rule, directions: u8) -> (Verdict, Option<Witness>) {
        let mut reason = if rule.unclassified & directions != 0 {
            UndeterminedReason::NotClassified
        } else {
            UndeterminedReason::Unproven
        };
        for Candidate { value, direction } in rule.candidates {
            if directions & direction.bit() == 0 {
                continue;
            }
            let value = match self.placed(value, direction.admitting()) {
                Ok(Some(value)) => CheckedValue::new(value),
                Ok(None) => continue,
                Err(Exhausted) => return (exhaustion(), None),
            };
            // The revision that must admit the value, then the one that must reject it.
            let order = match direction {
                Direction::ClientSupplied => [Revision::Earlier, Revision::Later],
                Direction::ThingSupplied => [Revision::Later, Revision::Earlier],
            };
            let mut admissions = [Admission::Undetermined; 2];
            let mut interpretations = ValueInterpretations::default();
            for (revision, needed) in order
                .into_iter()
                .zip([Admission::Admitted, Admission::Rejected])
            {
                // A description Sample-value checking cannot select settles nothing.
                let Ok(checked) = self.check(revision, &value) else {
                    break;
                };
                if checked.exhausted {
                    return (exhaustion(), None);
                }
                let admission = checked.admission;
                admissions[revision.index()] = admission;
                interpretations |= checked.interpretations;
                if admission == Admission::Undetermined {
                    reason = UndeterminedReason::WitnessUnsettled;
                }
                if admission != needed {
                    break;
                }
            }
            if admissions[order[0].index()] == Admission::Admitted
                && admissions[order[1].index()] == Admission::Rejected
            {
                let text = value.to_json();
                if !charge(&mut self.allowance.budget.allocation, text.len()) {
                    return (exhaustion(), None);
                }
                let witness = Witness {
                    value: text.to_owned(),
                    direction,
                    admissions,
                    interpretations,
                };
                return (Verdict::Breaking, Some(witness));
            }
        }
        (Verdict::Undetermined(reason), None)
    }

    /// The candidate at its `properties` entry, among an array's `items` or at a choice
    /// alternative, up to the root description. Each enclosing map also holds a representative
    /// value of every other entry it requires in `revision`, the revision that must admit the
    /// candidate, and each enclosing array repeats it as often as its `minItems` requires
    /// there. `None` when that exceeds the value capabilities or a required entry has no
    /// representative value. Corresponding entries and alternatives share their Given Names,
    /// so the candidate lies at the same path in both revisions.
    ///
    /// The other revision must read each enclosing value as `revision` does, or it could
    /// reject the candidate for a change the candidate was not made for. So the candidate is
    /// also `None` where the corresponding enclosing descriptions differ in what the enclosing
    /// value relies on, such as the description of an entry it fills.
    fn placed(
        &mut self,
        mut value: JsonValue,
        revision: Revision,
    ) -> Result<Option<JsonValue>, Exhausted> {
        let index = revision.index();
        let (root, description) = (self.roots[index], self.descriptions[index]);
        let other = self.descriptions[1 - index];
        let mut nodes = [description.node, other.node];
        while nodes[0] != root.node {
            let (Some((nesting, owner)), Some((_, counterpart))) =
                (nested(description, nodes[0]), nested(other, nodes[1]))
            else {
                return Ok(None);
            };
            nodes = [owner.node, counterpart.node];
            let mut owners = [owner, counterpart];
            if revision == Revision::Later {
                owners.reverse();
            }
            let allowance = &mut *self.allowance;
            let enclosed = match nesting {
                // An alternative's values are values of the description whose choice derives
                // it.
                Nesting::Alternative => continue,
                Nesting::Entry(name) => {
                    classify::enclosing(owners, revision, name, value, allowance)
                }
                Nesting::Items => classify::enclosing_items(owners, revision, value, allowance),
            };
            value = match enclosed {
                Ok(Some(enclosing)) => enclosing,
                Ok(None) => return Ok(None),
                Err(_) => return Err(Exhausted),
            };
        }
        Ok(Some(value))
    }

    /// Check `value` against one revision's root description on the lent allowance.
    fn check(
        &mut self,
        revision: Revision,
        value: &CheckedValue,
    ) -> Result<ValueAdmission, &'static str> {
        let description = EffectiveDataDescription::selected(self.roots[revision.index()]);
        check_generated(description, value, self.allowance)
    }
}

/// How a nested description sits in the description that encloses it.
#[derive(Clone, Copy)]
enum Nesting<'c> {
    /// A choice alternative, whose values are values of the description holding the choice.
    Alternative,
    /// An array's `items`, a Direct items description whose owner is its array description.
    Items,
    /// A map `properties` entry with this name, whose `properties` map belongs to the
    /// enclosing description.
    Entry(&'c str),
}

/// How the description at `node` of `json`'s model is nested, and the description enclosing
/// it, or `None` at a root.
fn nested<'c>(json: EffectiveJson<'c>, node: usize) -> Option<(Nesting<'c>, EffectiveJson<'c>)> {
    let nodes = &json.model.nodes;
    let parent = nodes[node].parent?;
    let (nesting, owner) = match (nodes[parent].key, nodes[node].role, nodes[node].key) {
        (Some("sdfChoice"), ..) => (Nesting::Alternative, nodes[parent].parent?),
        (_, Some(Role::DirectItem), _) => (Nesting::Items, parent),
        (_, _, Some(name)) => (Nesting::Entry(name), nodes[parent].parent?),
        _ => return None,
    };
    let owner = EffectiveJson {
        model: json.model,
        node: owner,
    };
    Some((nesting, owner))
}

/// The operation allowance ran out.
struct Exhausted;

fn exhaustion() -> Verdict {
    Verdict::Undetermined(UndeterminedReason::limit(
        "witness checking exhausted the operation allowance",
    ))
}
