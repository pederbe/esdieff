//! The declarations mandatory in each grouping of one revision, read from its Required
//! declaration relationships under ADR 0026: Required declaration relationships and source
//! evidence, for the requiredness rule of ADR 0039: Client-side revision comparison.
//!
//! An `sdfRequired` entry makes the declaration it selects mandatory in a grouping. The
//! entries of a grouping belong to that grouping. The entries of an affordance, and a
//! `true` carrier, which selects the definition carrying it, belong to the grouping that
//! encloses that definition. A declaration required from an enclosing grouping can only be
//! represented within the groupings between them, so each of those is mandatory there too,
//! and the declaration is mandatory in each of them. Requiredness then closes through
//! mandatory groupings: whatever is mandatory in a mandatory grouping is mandatory in the
//! grouping that requires it.
use super::{UndeterminedReason, read};
use crate::allowance::{OperationBudget, admit, push};
use crate::effective::{Collection, RequiredReferenceForm, RequirednessState, ViewNode};
use crate::role::Role;
use crate::{Class, EffectiveModel};

const UNRESOLVED: UndeterminedReason = UndeterminedReason::Unavailable {
    code: "required.unavailable",
    message: "required declaration resolution is partial or unavailable",
};

/// The mandatory declarations of each grouping in one revision's document.
pub(super) struct Requiredness {
    /// Each grouping with the closure of its mandatory declarations, as effective nodes,
    /// sorted by grouping and then by declaration.
    mandatory: Vec<(usize, usize)>,
    /// Why some relationship is partial or unavailable, so that no grouping's requiredness
    /// is established, although the known mandatory declarations remain.
    unresolved: Option<UndeterminedReason>,
}
impl Requiredness {
    /// The requiredness of `model`'s groupings from the relationships collected with it.
    pub(super) fn new(
        model: &EffectiveModel<'_>,
        collection: Option<&Collection<'_>>,
        budget: &mut OperationBudget,
    ) -> Self {
        let mut requiredness = Self {
            mandatory: Vec::new(),
            unresolved: None,
        };
        if let Err(reason) = requiredness.collect(model, collection, budget) {
            requiredness.unresolved = Some(reason);
        }
        requiredness
    }
    /// The declarations mandatory in `grouping`, as `(grouping, declaration)` pairs.
    pub(super) fn mandatory(&self, grouping: usize) -> &[(usize, usize)] {
        let start = self
            .mandatory
            .partition_point(|&(owner, _)| owner < grouping);
        let end = self
            .mandatory
            .partition_point(|&(owner, _)| owner <= grouping);
        &self.mandatory[start..end]
    }
    pub(super) fn is_mandatory(&self, grouping: usize, declaration: usize) -> bool {
        self.mandatory
            .binary_search(&(grouping, declaration))
            .is_ok()
    }
    /// Whether every relationship was resolved, or why one was not.
    pub(super) fn resolution(&self) -> Result<(), UndeterminedReason> {
        self.unresolved.map_or(Ok(()), Err)
    }

    fn collect(
        &mut self,
        model: &EffectiveModel<'_>,
        collection: Option<&Collection<'_>>,
        budget: &mut OperationBudget,
    ) -> Result<(), UndeterminedReason> {
        let Some(collection) = collection else {
            return Err(UNRESOLVED);
        };
        if !collection.traversed {
            self.unresolved = Some(UNRESOLVED);
        }
        let mut direct = Vec::new();
        for owner in &collection.owners {
            // Data-only placements select no declaration of a grouping.
            let (Some(class), Some(_)) = (owner.class, owner.syntax) else {
                continue;
            };
            let ViewNode::Effective(node) = owner.json else {
                self.unresolved = Some(UNRESOLVED);
                continue;
            };
            if !owner.complete || owner.state != RequirednessState::Available {
                self.unresolved = Some(UNRESOLVED);
            }
            for reference in &owner.references {
                let carrier = reference.form == Some(RequiredReferenceForm::Carrier);
                let grouping = if carrier || !matches!(class, Class::Thing | Class::Object) {
                    enclosing(model, node, budget)?
                } else {
                    Some(node)
                };
                for target in &reference.targets {
                    let Some(declaration) = target.effective else {
                        self.unresolved = Some(UNRESOLVED);
                        continue;
                    };
                    if let Some(grouping) = grouping.filter(|&grouping| grouping != declaration) {
                        relate(model, grouping, declaration, &mut direct, budget)?;
                    }
                }
            }
        }
        read::work(budget, direct.len())?;
        direct.sort_unstable();
        direct.dedup();
        self.close(&direct, budget)
    }

    /// Close the direct requirements through mandatory groupings.
    fn close(
        &mut self,
        direct: &[(usize, usize)],
        budget: &mut OperationBudget,
    ) -> Result<(), UndeterminedReason> {
        let requirements = |grouping: usize| {
            let start = direct.partition_point(|&(owner, _)| owner < grouping);
            let end = direct.partition_point(|&(owner, _)| owner <= grouping);
            &direct[start..end]
        };
        let mut start = 0;
        while let Some(&(grouping, _)) = direct.get(start) {
            let own = requirements(grouping);
            start += own.len();
            let mut members: Vec<usize> = Vec::new();
            let mut pending = Vec::new();
            for &(_, declaration) in own {
                push(&mut members, declaration, &mut budget.allocation).map_err(limit)?;
                push(&mut pending, declaration, &mut budget.allocation).map_err(limit)?;
            }
            while let Some(member) = pending.pop() {
                for &(_, declaration) in requirements(member) {
                    read::work(budget, 1)?;
                    if declaration == grouping {
                        continue;
                    }
                    if let Err(at) = members.binary_search(&declaration) {
                        admit(&mut members, &mut budget.allocation).map_err(limit)?;
                        members.insert(at, declaration);
                        push(&mut pending, declaration, &mut budget.allocation).map_err(limit)?;
                    }
                }
            }
            for member in members {
                let pair = (grouping, member);
                push(&mut self.mandatory, pair, &mut budget.allocation).map_err(limit)?;
            }
        }
        Ok(())
    }
}

/// Record that `declaration` is mandatory in `grouping`. When the grouping encloses it, each
/// grouping between them is mandatory there too and holds the declaration as mandatory.
fn relate(
    model: &EffectiveModel<'_>,
    grouping: usize,
    declaration: usize,
    direct: &mut Vec<(usize, usize)>,
    budget: &mut OperationBudget,
) -> Result<(), UndeterminedReason> {
    // The declaration, then the groupings above it up to the requiring grouping.
    let mut chain = Vec::new();
    push(&mut chain, declaration, &mut budget.allocation).map_err(limit)?;
    let mut current = declaration;
    let enclosed = loop {
        read::work(budget, 1)?;
        let Some(parent) = model.nodes[current].parent else {
            break false;
        };
        if parent == grouping {
            break true;
        }
        if is_grouping(model, parent) {
            push(&mut chain, parent, &mut budget.allocation).map_err(limit)?;
        }
        current = parent;
    };
    if !enclosed {
        chain.truncate(1);
    }
    for (index, &member) in chain.iter().enumerate() {
        push(direct, (grouping, member), &mut budget.allocation).map_err(limit)?;
        for &inner in &chain[index + 1..] {
            push(direct, (inner, member), &mut budget.allocation).map_err(limit)?;
        }
    }
    Ok(())
}

/// The grouping that encloses a definition, if any does.
fn enclosing(
    model: &EffectiveModel<'_>,
    node: usize,
    budget: &mut OperationBudget,
) -> Result<Option<usize>, UndeterminedReason> {
    let mut current = node;
    while let Some(parent) = model.nodes[current].parent {
        read::work(budget, 1)?;
        if is_grouping(model, parent) {
            return Ok(Some(parent));
        }
        current = parent;
    }
    Ok(None)
}

fn is_grouping(model: &EffectiveModel<'_>, node: usize) -> bool {
    matches!(
        model.nodes[node].role,
        Some(Role::Definition(Class::Thing | Class::Object))
    )
}

fn limit(_: crate::allowance::AllocationLimit) -> UndeterminedReason {
    UndeterminedReason::limit("comparison allocation limit")
}
