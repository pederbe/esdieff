//! Scans of every final effective coordinate for checks that consult recorded registry
//! evidence: unit names and Recorded extension vocabulary.
//! Follows ADR 0027: Unit names and recorded registry guidance, which checks inherited and
//! overriding values at each consuming coordinate, and ADR 0009: Bounded processing and
//! reporting, which charges every visited node to the original operation allowance.
use crate::EffectiveModel;
use crate::allowance::charge;

/// One scan's checks and conclusions, visited at each final effective coordinate.
pub(crate) trait Visitor<'a> {
    /// The operation allowance that pays traversal.
    fn work(&mut self) -> &mut usize;
    /// Leaves the scan's conclusions undetermined while it goes on.
    fn defer(&mut self);
    /// Traversal work ran out at `node`, so the scan stops.
    fn exhausted(&mut self, model: &EffectiveModel<'a>, node: usize);
    /// Checks `node`. `skip` says the walk does not descend into it, for example because it
    /// is deleted or unavailable. Returns false to stop the scan.
    fn visit(&mut self, model: &EffectiveModel<'a>, node: usize, skip: bool) -> bool;
}

/// Visits every selected model from its root, and then each final alternative as its own
/// tree. A missing selection, an unfinished composition or an incomplete scope defers.
pub(crate) fn scan<'a>(
    visitor: &mut impl Visitor<'a>,
    models: &[EffectiveModel<'a>],
    scope_incomplete: bool,
) {
    if scope_incomplete {
        visitor.defer();
    }
    for model in models {
        if !charge(visitor.work(), 1) {
            visitor.defer();
            return;
        }
        if !model.complete_apart_from_required() || model.dependency_unknown {
            visitor.defer();
        }
        let Some(root) = model.selected else {
            visitor.defer();
            continue;
        };
        if !tree(visitor, model, root) {
            return;
        }
        let mut cursor = (0, 0);
        loop {
            match model.next_assessment_alternative(&mut cursor, visitor.work()) {
                Ok(Some(branch)) => {
                    if !tree(visitor, model, branch) {
                        return;
                    }
                }
                Ok(None) => break,
                Err(()) => {
                    visitor.defer();
                    return;
                }
            }
        }
    }
}

/// Walks one tree in document order. It does not enter `sdfChoice`, whose final
/// alternatives `scan` visits separately. Returns false when the scan stops.
fn tree<'a>(visitor: &mut impl Visitor<'a>, model: &EffectiveModel<'a>, root: usize) -> bool {
    let mut current = Some(root);
    while let Some(id) = current {
        if !charge(
            visitor.work(),
            1 + model.nodes[id].key.map_or(0, str::len) + 9,
        ) {
            visitor.exhausted(model, id);
            return false;
        }
        let node = &model.nodes[id];
        let required = model.required_quality(id);
        let skip = node.deleted
            || required
            || node.sample_skipped
            || node.donor_unavailable.is_some()
            || node.unavailable.is_some();
        if !node.deleted
            && !required
            && (node.unavailable.is_some()
                || node.meaning_unavailable.is_some()
                || node.donor_unavailable.is_some())
        {
            visitor.defer();
        }
        if !visitor.visit(model, id, skip) {
            return false;
        }
        if !skip
            && node.key != Some("sdfChoice")
            && let Some(child) = node.first
        {
            current = Some(child);
            continue;
        }
        let mut up = id;
        loop {
            if !charge(visitor.work(), 1) {
                visitor.exhausted(model, id);
                return false;
            }
            if up == root {
                current = None;
                break;
            }
            let ancestor = &model.nodes[up];
            if let Some(next) = ancestor.next {
                current = Some(next);
                break;
            }
            up = ancestor.parent.expect("reachable effective ancestry");
        }
    }
    true
}

/// Counts a scan's finding with `severity` and reserves its retained slot, paying new
/// finding capacity from the scan's `detail` allowance. Returns whether the caller may
/// retain the finding. Past the condition limit or the detail allowance it stays counted
/// and the report says that findings were omitted.
pub(crate) fn admit(
    structure: &mut crate::structure::Structure<'_>,
    conditions: usize,
    detail: &mut usize,
    severity: &str,
) -> bool {
    structure.total += 1;
    structure.errors += u32::from(severity == "error");
    structure.notices += u32::from(severity == "notice");
    if structure.findings.len() >= conditions {
        structure.count_omitted = true;
        return false;
    }
    let old = structure.findings.capacity();
    if structure.findings.len() == old {
        let new = (old.max(4) * 2).min(conditions);
        if !charge(
            detail,
            (new - old) * std::mem::size_of::<crate::structure::Finding<'_>>(),
        ) {
            structure.bytes_omitted = true;
            return false;
        }
        structure.findings.reserve_exact(new - old);
    }
    true
}
