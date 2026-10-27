//! Operation allowances charged before work, traversal or derived allocation.
//! Follows ADR 0012: Exact numbers and JSON input policies,
//! ADR 0013: Shared workflow and report guarantees, and
//! ADR 0015: Bounded value equality in a resolved data context.
//! The `effective` module doc names the deliberate core that lends this budget.

/// The derived allocation allowance ran out while admitting one more value.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AllocationLimit;

pub(crate) fn charge(budget: &mut usize, cost: usize) -> bool {
    if let Some(left) = budget.checked_sub(cost) {
        *budget = left;
        true
    } else {
        *budget = 0;
        false
    }
}
pub(crate) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    allocation: &mut usize,
) -> Result<(), AllocationLimit> {
    admit(values, allocation)?;
    values.push(value);
    Ok(())
}
pub(crate) fn admit<T>(values: &mut Vec<T>, allocation: &mut usize) -> Result<(), AllocationLimit> {
    if values.len() == values.capacity() {
        let old = values.capacity();
        let new = old.max(4).checked_mul(2).ok_or(AllocationLimit)?;
        if !charge(allocation, (new - old) * std::mem::size_of::<T>()) {
            return Err(AllocationLimit);
        }
        values.reserve_exact(new - old);
    }
    Ok(())
}

#[derive(Default)]
pub(crate) struct OperationBudget {
    pub(crate) patterns: crate::pattern::MatchingWork,
    pub(crate) work: usize,
    pub(crate) allocation: usize,
    pub(crate) nodes: usize,
    pub(crate) references: usize,
    /// Extension vocabulary that established base conclusions set aside. Like the pattern
    /// sink, it records use, not allowance.
    pub(crate) set_aside: crate::role::SetAside,
}
impl OperationBudget {
    pub(crate) fn new(limits: &crate::Limits) -> Self {
        Self {
            patterns: Default::default(),
            work: limits.work,
            allocation: limits.effective_allocation,
            nodes: limits.effective_nodes,
            references: limits.references,
            set_aside: Default::default(),
        }
    }
    /// A budget lending only `work`, for a nested check that allocates nothing.
    pub(crate) fn lease(work: usize) -> Self {
        Self {
            work,
            ..Self::default()
        }
    }
}
