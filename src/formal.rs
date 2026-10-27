//! Exact imported CDDL count domains follow ADR0032: Formal model admission.
//! Every JSON number belongs to the imported `number`, so only `uint` needs work.
use crate::{ExactNumber, NumberWork};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Verdict {
    Admitted,
    Excluded,
    Unavailable,
}

pub(crate) fn uint(value: ExactNumber<'_>, work: &mut usize, numbers: &mut NumberWork) -> Verdict {
    if !debit(work, numbers, value.decimal.coefficient.len() + 22) {
        return Verdict::Unavailable;
    }
    if value.sign().is_lt() || !value.is_integer() {
        return Verdict::Excluded;
    }
    if within_u64(value) {
        Verdict::Admitted
    } else {
        Verdict::Excluded
    }
}

/// Compares a nonnegative integer with 2^64-1 digit by digit.
fn within_u64(value: ExactNumber<'_>) -> bool {
    if value.sign().is_eq() {
        return true;
    }
    let digits = value.decimal.coefficient.as_bytes();
    let length = digits.len() as i128 + i128::from(value.decimal.power);
    if length != 20 {
        return length < 20;
    }
    for (index, bound) in b"18446744073709551615".iter().copied().enumerate() {
        let digit = digits.get(index).copied().unwrap_or(b'0');
        if digit != bound {
            return digit < bound;
        }
    }
    true
}

fn debit(work: &mut usize, numbers: &mut NumberWork, cost: usize) -> bool {
    crate::allowance::charge(work, cost) && numbers.debit(cost).is_ok()
}
