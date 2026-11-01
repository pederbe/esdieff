//! A Grouping array's `minItems` and `maxItems` bound its instance count inclusively.
//! For clients, a narrower later range is compatible and widening is breaking, following
//! ADR 0039: Client-side revision comparison. Implementer comparison reverses the internal
//! revision order, preserving every earlier count by accepting widening and rejecting
//! narrowing, following ADR 0058: Implementer grouping counts. Both revisions must establish
//! valid grouping-array ranges; other shapes stay unclassified. An absent minimum reads
//! as 0 and an absent maximum as unbounded. Each changed end is judged on its own.
use super::bounds::End;
use super::counts::{bound, relation};
use super::{Descriptions, Relation};
use crate::ExactNumber;
use crate::comparison::{Revision, UndeterminedReason, Verdict};
use std::cmp::Ordering;

/// One revision's instance count bound, absent or an exact nonnegative integer.
type Count<'c> = Option<ExactNumber<'c>>;

/// The changed count at `end`, `minItems` at the lower end and `maxItems` at the upper one.
pub(super) fn verdict(
    end: End,
    groupings: &mut Descriptions<'_, '_>,
) -> Result<Verdict, UndeterminedReason> {
    // Classified only when both revisions are arrays with counts that are exact
    // nonnegative integers and a minimum no greater than the maximum.
    let (Some(earlier), Some(later)) = (
        range(Revision::Earlier, groupings)?,
        range(Revision::Later, groupings)?,
    ) else {
        return Ok(Verdict::NOT_CLASSIFIED);
    };
    let index = match end {
        End::Lower => 0,
        End::Upper => 1,
    };
    Ok(
        match relation(end, [earlier[index], later[index]], groupings)? {
            Relation::Same | Relation::Narrower => Verdict::Compatible,
            _ => Verdict::Breaking,
        },
    )
}

/// One revision's minimum and maximum count, or `None` when the grouping is not an array or
/// the counts do not form a range.
fn range<'c>(
    revision: Revision,
    groupings: &mut Descriptions<'_, 'c>,
) -> Result<Option<[Count<'c>; 2]>, UndeterminedReason> {
    let (Some(minimum), Some(maximum)) = (
        bound("minItems", revision, groupings)?,
        bound("maxItems", revision, groupings)?,
    ) else {
        return Ok(None);
    };
    Ok(match (minimum, maximum) {
        (None, None) => None,
        (Some(low), Some(high)) if groupings.compare(low, high)? == Ordering::Greater => None,
        _ => Some([minimum, maximum]),
    })
}
