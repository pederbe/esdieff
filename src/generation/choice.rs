//! Optional observations of successful admission proofs, on a separate allowance.
use crate::allowance::{OperationBudget, charge, push};
use crate::budgeted::{self, Exhausted};
use crate::value::BorrowedValue;
use crate::{Admission, AlternativeId, EffectiveJson, Field, JsonKind, Limits};

/// Whether all identifiable named alternatives and their observations were collected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ChoiceEvidenceState {
    Complete,
    Partial,
    Unavailable,
}
impl ChoiceEvidenceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}

/// A semantic step from the selected description, not an authored JSON pointer.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ChoiceCoordinate {
    Property(String),
    Items,
    Choice(String),
    Enum(usize),
}

/// One retained, wholly admitted Generated value and its actual value occurrence.
pub struct ChoiceWitness {
    value_index: usize,
    sample_pointer: String,
}
impl ChoiceWitness {
    /// Zero-based index in the description's final retained values.
    pub fn value_index(&self) -> usize {
        self.value_index
    }
    pub fn sample_pointer(&self) -> &str {
        &self.sample_pointer
    }
}

/// One named Effective alternative in model order.
pub struct ChoiceAlternativeEvidence {
    owner: Vec<ChoiceCoordinate>,
    name: String,
    witness: Option<ChoiceWitness>,
    owner_node: usize,
    branch_node: Option<usize>,
}
impl ChoiceAlternativeEvidence {
    pub fn owner(&self) -> &[ChoiceCoordinate] {
        &self.owner
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn status(&self) -> &'static str {
        if self.witness.is_some() {
            "witnessed"
        } else {
            "no_witness_established"
        }
    }
    pub fn witness(&self) -> Option<&ChoiceWitness> {
        self.witness.as_ref()
    }
}

/// Choice admission observations, independent of generation completeness and rendering.
pub struct ChoiceEvidence {
    state: ChoiceEvidenceState,
    rows: Vec<ChoiceAlternativeEvidence>,
    omitted: usize,
}
impl ChoiceEvidence {
    pub fn state(&self) -> ChoiceEvidenceState {
        self.state
    }
    pub fn alternatives(&self) -> impl ExactSizeIterator<Item = &ChoiceAlternativeEvidence> {
        self.rows.iter()
    }
    /// Known named rows omitted by collection limits. Unknown membership has no total.
    pub fn alternatives_omitted(&self) -> usize {
        self.omitted
    }
}

struct Observation {
    row: usize,
    value_index: usize,
    pointer: String,
}

pub(crate) struct Collection {
    evidence: ChoiceEvidence,
    budget: OperationBudget,
    pending: Vec<Observation>,
    proofs: Vec<Observation>,
    checkpoints: Vec<usize>,
    active: bool,
}
impl Collection {
    pub(crate) fn new(json: Option<EffectiveJson<'_>>, limits: &Limits) -> Self {
        let mut this = Self {
            evidence: ChoiceEvidence {
                state: ChoiceEvidenceState::Unavailable,
                rows: Vec::new(),
                omitted: 0,
            },
            budget: OperationBudget::new(limits),
            pending: Vec::new(),
            proofs: Vec::new(),
            checkpoints: Vec::new(),
            active: true,
        };
        if let Some(json) =
            json.filter(|json| budgeted::failure(*json).is_none() && json.kind() == JsonKind::Map)
        {
            this.evidence.state = ChoiceEvidenceState::Complete;
            if this.enumerate(json, &mut Vec::new(), limits.depth).is_err() {
                this.partial();
            }
        }
        if this.evidence.rows.is_empty() && this.evidence.state != ChoiceEvidenceState::Partial {
            this.active = false;
        }
        this
    }
    fn partial(&mut self) {
        self.evidence.state = ChoiceEvidenceState::Partial;
    }
    fn stop(&mut self) {
        self.partial();
        self.active = false;
        self.pending.clear();
        self.checkpoints.clear();
    }
    fn copy_text(&mut self, text: &str) -> Result<String, Exhausted> {
        budgeted::work(&mut self.budget, text.len() + 1)?;
        if !charge(&mut self.budget.allocation, text.len()) {
            return Err(Exhausted::Allocation);
        }
        Ok(text.to_owned())
    }
    fn copy_path(&mut self, path: &[ChoiceCoordinate]) -> Result<Vec<ChoiceCoordinate>, Exhausted> {
        let mut copied = Vec::new();
        for step in path {
            budgeted::work(&mut self.budget, 1)?;
            let step = match step {
                ChoiceCoordinate::Property(name) => {
                    ChoiceCoordinate::Property(self.copy_text(name)?)
                }
                ChoiceCoordinate::Choice(name) => ChoiceCoordinate::Choice(self.copy_text(name)?),
                other => other.clone(),
            };
            budgeted::kept(&mut copied, step, &mut self.budget)?;
        }
        Ok(copied)
    }
    fn enumerate(
        &mut self,
        json: EffectiveJson<'_>,
        path: &mut Vec<ChoiceCoordinate>,
        depth: usize,
    ) -> Result<(), Exhausted> {
        budgeted::work(&mut self.budget, 1)?;
        if depth == 0 || budgeted::failure(json).is_some() || json.kind() != JsonKind::Map {
            self.partial();
            return Ok(());
        }
        match crate::sample::alternatives(json, &mut self.budget) {
            Field::Present(rows) => {
                let mut rows = rows;
                while let Some((id, branch)) = rows.next() {
                    let named = matches!(id, AlternativeId::Choice(_));
                    let mut retained = false;
                    let processed = (|| {
                        budgeted::work(&mut self.budget, 1)?;
                        let step = match id {
                            AlternativeId::Choice(name) => {
                                let owner = self.copy_path(path)?;
                                let owned_name = self.copy_text(name)?;
                                budgeted::kept(
                                    &mut self.evidence.rows,
                                    ChoiceAlternativeEvidence {
                                        owner,
                                        name: owned_name,
                                        witness: None,
                                        owner_node: json.node,
                                        branch_node: match &branch {
                                            Field::Present(branch) => Some(branch.json().node),
                                            _ => None,
                                        },
                                    },
                                    &mut self.budget,
                                )?;
                                retained = true;
                                ChoiceCoordinate::Choice(self.copy_text(name)?)
                            }
                            AlternativeId::Enum(index) => ChoiceCoordinate::Enum(index),
                        };
                        budgeted::kept(path, step, &mut self.budget)?;
                        if let Field::Present(branch) = branch {
                            self.enumerate(branch.json(), path, depth - 1)?;
                        } else {
                            self.partial();
                        }
                        path.pop();
                        Ok(())
                    })();
                    if processed.is_err() {
                        // An established group is entirely named Choices or enum occurrences.
                        // Its remaining length is known without scanning unreadable descendants.
                        if named {
                            self.evidence.omitted += rows.len() + usize::from(!retained);
                        }
                        return processed;
                    }
                }
                // Shared qualities are checked through the final alternatives only.
                return Ok(());
            }
            Field::Unavailable(_) => {
                self.partial();
                return Ok(());
            }
            Field::Absent => {}
        }
        let kind = json
            .lookup("type", &mut self.budget)
            .present()
            .and_then(|value| value.text().present());
        let structural_type = matches!(kind, Some("object" | "array"));
        let direct_array =
            kind == Some("array") && json.n().role == Some(crate::role::Role::DirectItem);
        if !structural_type || direct_array {
            if !matches!(kind, Some("number" | "integer" | "string" | "boolean")) {
                // Unavailable type or restricted Direct-item structure can hide applicable
                // descendants, but supplies no established context to enumerate them in.
                for key in ["properties", "items"] {
                    if budgeted::member(json, key, &mut self.budget)?.is_some() {
                        self.partial();
                    }
                }
            }
            return Ok(());
        }
        if kind == Some("object")
            && let Some(properties) = budgeted::member(json, "properties", &mut self.budget)?
        {
            if budgeted::failure(properties).is_some() || properties.kind() != JsonKind::Map {
                self.partial();
            } else {
                for property in properties.children() {
                    let Some(name) = property.n().key else {
                        self.partial();
                        continue;
                    };
                    let step = ChoiceCoordinate::Property(self.copy_text(name)?);
                    budgeted::kept(path, step, &mut self.budget)?;
                    self.enumerate(property, path, depth - 1)?;
                    path.pop();
                }
            }
        }
        if kind == Some("array")
            && let Some(items) = budgeted::member(json, "items", &mut self.budget)?
        {
            budgeted::kept(path, ChoiceCoordinate::Items, &mut self.budget)?;
            self.enumerate(items, path, depth - 1)?;
            path.pop();
        }
        Ok(())
    }
    pub(crate) fn begin(&mut self) {
        self.pending.clear();
        self.checkpoints.clear();
    }
    pub(crate) fn enter(&mut self) {
        if self.active
            && (!charge(&mut self.budget.work, 1)
                || push(
                    &mut self.checkpoints,
                    self.pending.len(),
                    &mut self.budget.allocation,
                )
                .is_err())
        {
            self.stop();
        }
    }
    pub(crate) fn leave(&mut self, admission: Admission) {
        if !self.active {
            return;
        }
        if !charge(&mut self.budget.work, 1) {
            self.stop();
            return;
        }
        let Some(start) = self.checkpoints.pop() else {
            self.stop();
            return;
        };
        if admission != Admission::Admitted {
            self.pending.truncate(start);
        }
    }
    pub(crate) fn observe(&mut self, owner: usize, branch: usize, sample: BorrowedValue<'_>) {
        if !self.active {
            return;
        }
        for index in 0..self.evidence.rows.len() {
            if !charge(&mut self.budget.work, 1) {
                self.stop();
                return;
            }
            let row = &self.evidence.rows[index];
            if row.owner_node != owner || row.branch_node != Some(branch) {
                continue;
            }
            // Keep the first actual occurrence for each row within this value.
            for observed in &self.pending {
                if !charge(&mut self.budget.work, 1) {
                    self.stop();
                    return;
                }
                if observed.row == index {
                    return;
                }
            }
            let BorrowedValue::Sample(sample) = sample else {
                self.stop();
                return;
            };
            let pointer = sample.evidence_pointer(&mut self.budget);
            let Ok(pointer) = pointer else {
                self.stop();
                return;
            };
            if push(
                &mut self.pending,
                Observation {
                    row: index,
                    value_index: 0,
                    pointer,
                },
                &mut self.budget.allocation,
            )
            .is_err()
            {
                self.stop();
            }
            return;
        }
    }
    pub(crate) fn retain(&mut self, value_index: usize, admission: Admission) {
        if !self.active || admission != Admission::Admitted {
            self.pending.clear();
            return;
        }
        for mut observation in self.pending.drain(..) {
            observation.value_index = value_index;
            if !charge(&mut self.budget.work, 1)
                || push(&mut self.proofs, observation, &mut self.budget.allocation).is_err()
            {
                self.evidence.state = ChoiceEvidenceState::Partial;
                self.active = false;
                break;
            }
        }
    }
    pub(crate) fn moved(&mut self, index: usize, last: usize) {
        for observation in &mut self.proofs {
            if !charge(&mut self.budget.work, 1) {
                self.proofs.clear();
                self.stop();
                return;
            }
            observation.value_index = if observation.value_index == index {
                last
            } else if observation.value_index > index {
                observation.value_index - 1
            } else {
                observation.value_index
            };
        }
    }
    pub(crate) fn finish(mut self) -> ChoiceEvidence {
        for proof in self.proofs {
            if !charge(&mut self.budget.work, 1) {
                self.evidence.state = ChoiceEvidenceState::Partial;
                break;
            }
            let row = &mut self.evidence.rows[proof.row];
            if row
                .witness
                .as_ref()
                .is_none_or(|old| proof.value_index < old.value_index)
            {
                row.witness = Some(ChoiceWitness {
                    value_index: proof.value_index,
                    sample_pointer: proof.pointer,
                });
            }
        }
        self.evidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::{GeneratedValue, targets};
    use crate::sample::ValueAllowance;
    use crate::{Catalogue, EffectiveRequest, SampleKind, SourceInput};

    #[test]
    fn exhausted_optional_bookkeeping_preserves_candidates_checks_and_allowances() {
        let model = br#"{"sdfProperty":{"a":{"nullable":false,"sdfChoice":{"one":{"const":1},"two":{"const":9}}}}}"#;
        let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", model)]).unwrap();
        let effective = catalogue
            .effective(
                EffectiveRequest::new(catalogue.source_id("model").unwrap())
                    .pointer("/sdfProperty/a"),
            )
            .unwrap();
        let json = effective.json().present().unwrap();
        let mut expected = None;
        for (work, allocation) in [
            (usize::MAX, usize::MAX),
            (0, usize::MAX),
            (usize::MAX, 0),
            (12, 256),
        ] {
            let mut collection = Collection::new(Some(json), &catalogue.limits);
            collection.budget.work = work;
            collection.budget.allocation = allocation;
            let mut allowance = ValueAllowance::new(&catalogue);
            let mut bytes = catalogue.limits.report_bytes;
            let size = |target: &str, value: &str, admission, codes: &[&'static str]| {
                super::super::report::value_bytes(
                    SampleKind::Property,
                    "/sdfProperty/a",
                    target,
                    value,
                    admission,
                    codes,
                )
            };
            let line = |target: &str, value: &str| {
                super::super::report::line_bytes(
                    SampleKind::Property,
                    "/sdfProperty/a",
                    target,
                    value,
                )
            };
            let generated = targets::generate(
                json,
                64,
                &mut allowance,
                targets::Room {
                    bytes: &mut bytes,
                    size: &size,
                    line: &line,
                },
                &mut collection,
            );
            let observed = (
                generated
                    .values
                    .iter()
                    .map(|value| {
                        let value = GeneratedValue(value);
                        (
                            value.target().to_owned(),
                            value.to_json().to_owned(),
                            value.admission(),
                            value.codes().collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>(),
                generated.exhausted,
                generated.omitted,
                generated.oversized,
                allowance.budget.work,
                allowance.budget.allocation,
                bytes,
            );
            if let Some(expected) = &expected {
                assert_eq!(&observed, expected);
            } else {
                expected = Some(observed);
            }
            let evidence = collection.finish();
            assert_eq!(
                evidence.state(),
                if work == usize::MAX && allocation == usize::MAX {
                    ChoiceEvidenceState::Complete
                } else {
                    ChoiceEvidenceState::Partial
                }
            );
        }
    }
}
