//! Esdieff's selected JSON representations for the semantic types in RFC 9880 Section 4.7.1
//! Table 5: canonical unpadded base64url and nonnegative POSIX-coordinate numbers. These are
//! recorded sample-value interpretations, not a device wire-protocol binding.
use crate::allowance::{OperationBudget, charge};
use crate::finding::{Basis, Finding, Verdict};
use crate::scalar::Context;
use crate::value::BorrowedValue;
use crate::{Field, JsonKind, NumberWork};

#[derive(Clone, Copy)]
pub(super) enum Established {
    Value,
    Representation,
    Unavailable(u8),
}

pub(super) fn basis(context: Context) -> Basis {
    if context == Context::Bytes {
        Basis::Byte
    } else {
        Basis::Unix
    }
}

pub(super) fn establish(
    context: Context,
    value: BorrowedValue<'_>,
    number: &mut NumberWork,
    budget: &mut OperationBudget,
    consulted: &mut u8,
) -> Established {
    if context == Context::Bytes {
        *consulted |= 1;
        match value.text() {
            Field::Present(text) => canonical(text, &mut budget.work),
            _ if value.kind() == JsonKind::String => Established::Unavailable(0),
            _ => Established::Representation,
        }
    } else {
        *consulted |= 2;
        match value.number() {
            Field::Present(value) => {
                if number.debit(1).is_err() {
                    Established::Unavailable(4)
                } else if value.sign().is_lt() {
                    Established::Unavailable(2)
                } else {
                    Established::Value
                }
            }
            _ if value.kind() == JsonKind::Number => Established::Unavailable(0),
            _ => Established::Representation,
        }
    }
}

fn canonical(text: &str, work: &mut usize) -> Established {
    if !text
        .len()
        .checked_add(1)
        .is_some_and(|cost| charge(work, cost))
    {
        return Established::Unavailable(3);
    }
    let mut last = 0;
    for byte in text.bytes() {
        last = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Established::Representation,
        };
    }
    if match text.len() % 4 {
        0 => true,
        2 => last & 15 == 0,
        3 => last & 3 == 0,
        _ => false,
    } {
        Established::Value
    } else {
        Established::Representation
    }
}

pub(crate) fn compare(
    context: Context,
    left: BorrowedValue<'_>,
    right: BorrowedValue<'_>,
    number: &mut NumberWork,
    budget: &mut OperationBudget,
    consulted: &mut u8,
) -> (Verdict, u16, u32) {
    let left_null = left.kind() == JsonKind::Null;
    let right_null = right.kind() == JsonKind::Null;
    if left_null && right_null {
        return (Verdict::Equal, 0, Basis::TypeNull.support_mask());
    }
    let a = if left_null {
        Established::Value
    } else {
        establish(context, left, number, budget, consulted)
    };
    let b = if right_null {
        Established::Value
    } else {
        establish(context, right, number, budget, consulted)
    };
    let limitations = limitation(a) | limitation(b);
    let supporting = if matches!(a, Established::Value) && !left_null
        || matches!(b, Established::Value) && !right_null
    {
        basis(context).support_mask()
    } else {
        0
    } | if left_null || right_null {
        Basis::TypeNull.support_mask()
    } else {
        0
    };
    if limitations != 0 {
        return (Verdict::Unavailable, limitations, supporting);
    }
    if left_null || right_null {
        return (
            Verdict::Different,
            0,
            supporting | Basis::TypeNull.support_mask(),
        );
    }
    if context == Context::Bytes {
        let (Field::Present(left), Field::Present(right)) = (left.text(), right.text()) else {
            return (Verdict::Unavailable, 1, supporting);
        };
        if !left
            .len()
            .checked_add(right.len())
            .and_then(|cost| cost.checked_add(1))
            .is_some_and(|cost| charge(&mut budget.work, cost))
        {
            return (Verdict::Unavailable, 1 << 3, supporting);
        }
        (
            if left == right {
                Verdict::Equal
            } else {
                Verdict::Different
            },
            0,
            supporting,
        )
    } else {
        let (Field::Present(left), Field::Present(right)) = (left.number(), right.number()) else {
            return (Verdict::Unavailable, 1, supporting);
        };
        match left.compare(right, number) {
            Ok(order) => (
                if order.is_eq() {
                    Verdict::Equal
                } else {
                    Verdict::Different
                },
                0,
                supporting,
            ),
            Err(_) => (Verdict::Unavailable, 1 << 4, supporting),
        }
    }
}

fn limitation(result: Established) -> u16 {
    match result {
        Established::Value => 0,
        Established::Representation => 1 << 2,
        Established::Unavailable(cause) => 1 << cause,
    }
}

// Exclusive scalar phases reuse the existing parser dispatch maximum. These
// are real borrowed operands/cursors and exact numbers; no decoded buffer exists.
pub(crate) fn fixed_headers() -> usize {
    use std::mem::size_of;
    let selected = size_of::<Context>()
        + 2 * size_of::<Field<crate::EffectiveJson<'_>>>()
        + 2 * size_of::<Option<&str>>()
        + 8 * size_of::<bool>()
        + size_of::<JsonKind>();
    let scan = 3 * size_of::<BorrowedValue<'_>>()
        + 2 * size_of::<&str>()
        + size_of::<std::str::Bytes<'_>>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<u8>()
        + 2 * size_of::<Established>()
        + size_of::<&mut u8>()
        + size_of::<&mut OperationBudget>()
        + size_of::<&mut NumberWork>();
    let numeric = 2 * size_of::<BorrowedValue<'_>>()
        + 2 * size_of::<Field<crate::ExactNumber<'_>>>()
        + size_of::<&mut NumberWork>()
        + size_of::<(Verdict, u16, u32)>()
        + 3 * size_of::<bool>()
        + size_of::<&mut u8>()
        + size_of::<&mut OperationBudget>();
    let diagnostic =
        size_of::<Finding>() + size_of::<Option<usize>>() + size_of::<(Basis, &str, &str)>();
    selected + scan.max(numeric).max(diagnostic)
}
