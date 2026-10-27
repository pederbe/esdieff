//! Reads of effective content and small collections charged to an operation budget, which
//! revision comparison and value generation share, following ADR 0009: Bounded processing and
//! reporting. Each read charges its work before it looks, so the same read always costs the
//! same.
use crate::allowance::{OperationBudget, charge, push};
use crate::{EffectiveJson, JsonKind};

/// A read or a value ran out of the budget it was charged to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Exhausted {
    Work,
    Allocation,
}

/// Why an effective view could not establish this content, including content copied from
/// an unavailable donor.
pub(crate) fn failure(json: EffectiveJson<'_>) -> Option<crate::effective::Failure> {
    json.n().unavailable.or(json.n().donor_unavailable)
}

/// Charge `cost` units of work.
pub(crate) fn work(budget: &mut OperationBudget, cost: usize) -> Result<(), Exhausted> {
    if charge(&mut budget.work, cost) {
        Ok(())
    } else {
        Err(Exhausted::Work)
    }
}

/// The member of a map with this name, charging every scanned key.
pub(crate) fn member<'c>(
    json: EffectiveJson<'c>,
    key: &str,
    budget: &mut OperationBudget,
) -> Result<Option<EffectiveJson<'c>>, Exhausted> {
    for child in json.children() {
        let name = child.n().key;
        work(budget, 1 + key.len() + name.map_or(0, str::len))?;
        if name == Some(key) {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

/// Whether `json` is an available array of decoded strings, as `enum` values and `required`
/// names are, charging each element.
pub(crate) fn strings(
    json: EffectiveJson<'_>,
    budget: &mut OperationBudget,
) -> Result<bool, Exhausted> {
    if failure(json).is_some() || json.kind() != JsonKind::Array {
        return Ok(false);
    }
    for value in json.children() {
        work(budget, 1)?;
        if value.text().present().is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether `name` is among `names`, as a repeated required name is, charging each comparison.
pub(crate) fn listed<'n>(
    names: impl IntoIterator<Item = &'n str>,
    name: &str,
    budget: &mut OperationBudget,
) -> Result<bool, Exhausted> {
    for other in names {
        work(budget, name.len().min(other.len()) + 1)?;
        if other == name {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Append `item`, charging any growth of `items` to the allocation budget.
pub(crate) fn kept<T>(
    items: &mut Vec<T>,
    item: T,
    budget: &mut OperationBudget,
) -> Result<(), Exhausted> {
    push(items, item, &mut budget.allocation).map_err(|_| Exhausted::Allocation)
}
