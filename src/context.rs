//! Recorded draft policy for non-affordance Context definitions, following ADR 0046.
use crate::{Class, EffectiveJson, role::Role};

pub(crate) const DRAFT: &str = "draft-ietf-asdf-sdf-nonaffordance-06";
pub(crate) const SHA256: &str = "404e5d06747cf5c78ae10e2867729cb751a40d3d35af58e3fdc7f5876a922661";
pub(crate) const DEFAULT_UNAVAILABLE: &str =
    "Context nullable default is unresolved under draft-ietf-asdf-sdf-nonaffordance-06 Section 4.3";

impl EffectiveJson<'_> {
    /// Context data and its nested descriptions retain the draft's default uncertainty.
    /// A different enclosing definition establishes its own role.
    pub(crate) fn context_data(self) -> bool {
        if !self.n().role.is_some_and(Role::full_data) {
            return false;
        }
        let mut next = Some(self.node);
        while let Some(node) = next {
            let node = &self.model.nodes[node];
            match node.role {
                Some(Role::Definition(class)) => return class == Class::Context,
                _ => next = node.parent,
            }
        }
        false
    }
}
