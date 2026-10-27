//! Exact decimal steps for values just inside or just outside a bound, following ADR 0012:
//! Exact numbers and JSON input policies. Revision comparison and Generated values share
//! these unit rules, so a value one unit across a bound is the same value in both.
use crate::ExactNumber;

/// One end of a numeric or count range.
#[derive(Clone, Copy)]
pub(crate) enum End {
    Lower,
    Upper,
}
impl End {
    /// The inclusive and the exclusive bound quality.
    pub(crate) fn qualities(self) -> [(&'static str, bool); 2] {
        match self {
            Self::Lower => [("minimum", false), ("exclusiveMinimum", true)],
            Self::Upper => [("maximum", false), ("exclusiveMaximum", true)],
        }
    }
    /// Whether moving up goes further into the admitted range.
    pub(crate) fn inward_up(self) -> bool {
        matches!(self, Self::Lower)
    }
}

/// The power of one unit just across `value`: 1 when the digits that needs fit the
/// coefficient digit limit, otherwise the value's own last digit.
pub(crate) fn step_power(value: ExactNumber<'_>, digits: usize) -> i64 {
    let power = value.decimal.power;
    let needed = value.decimal.coefficient.len() as u128 + power.max(0) as u128;
    if power > 0 && needed > digits as u128 {
        power
    } else {
        power.min(0)
    }
}

/// `value` moved up or down by 10^`power`, or by half of that. `power` is at most the power
/// of the value's last digit, so the move is exact.
pub(crate) fn step(value: ExactNumber<'_>, up: bool, power: i64, half: bool) -> String {
    let decimal = value.decimal;
    if decimal.coefficient == "0" {
        return if half {
            render(!up, &[5], power - 1)
        } else {
            render(!up, &[1], power)
        };
    }
    let mut digits: Vec<u8> = decimal.coefficient.bytes().map(|b| b - b'0').collect();
    let padding = usize::try_from(decimal.power - power).expect("a step below the last digit");
    digits.resize(digits.len() + padding, 0);
    if up != decimal.negative {
        // Away from zero, where half a unit only appends a digit.
        if !half {
            increment(&mut digits);
        }
    } else {
        decrement(&mut digits);
    }
    let mut power = power;
    if half {
        digits.push(5);
        power -= 1;
    }
    let zero = digits.iter().all(|digit| *digit == 0);
    render(decimal.negative && !zero, &digits, power)
}

/// Half of a positive `step`: its coefficient times 5, one decimal place further down.
pub(crate) fn half(step: ExactNumber<'_>) -> String {
    render(false, &halved(step), step.decimal.power - 1)
}

/// The digits of a positive `step` times 5, whose last digit has the power one below the
/// step's last digit, so that they denote half the step.
fn halved(step: ExactNumber<'_>) -> Vec<u8> {
    let mut digits = Vec::with_capacity(step.decimal.coefficient.len() + 1);
    let mut carry = 0;
    for digit in step.decimal.coefficient.bytes().rev() {
        let product = (digit - b'0') * 5 + carry;
        digits.push(product % 10);
        carry = product / 10;
    }
    if carry > 0 {
        digits.push(carry);
    }
    digits.reverse();
    digits
}

/// `value`, a multiple of the positive `step`, plus half the step, exactly, or `None` when
/// the sum needs more than `limit` coefficient digits. A nonzero multiple is at least a whole
/// step from 0, so adding half a step never changes its sign.
pub(crate) fn half_past(
    value: ExactNumber<'_>,
    step: ExactNumber<'_>,
    limit: usize,
) -> Option<String> {
    let decimal = value.decimal;
    let half_power = step.decimal.power - 1;
    let power = decimal.power.min(half_power);
    let aligned = |mut digits: Vec<u8>, from: i64| -> Option<Vec<u8>> {
        let padding = usize::try_from(from - power).ok()?;
        (digits.len() + padding <= limit).then(|| {
            digits.resize(digits.len() + padding, 0);
            digits
        })
    };
    let magnitude = decimal.coefficient.bytes().map(|b| b - b'0').collect();
    let magnitude = aligned(magnitude, decimal.power)?;
    let half = aligned(halved(step), half_power)?;
    Some(if decimal.negative {
        render(true, &subtract(&magnitude, &half), power)
    } else {
        render(false, &add(&magnitude, &half), power)
    })
}

/// The sum of two magnitudes with the same last-digit power.
fn add(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut sum = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0;
    let (mut a, mut b) = (a.iter().rev(), b.iter().rev());
    loop {
        let (x, y) = (a.next(), b.next());
        if x.is_none() && y.is_none() {
            break;
        }
        let digit = x.copied().unwrap_or(0) + y.copied().unwrap_or(0) + carry;
        sum.push(digit % 10);
        carry = digit / 10;
    }
    if carry > 0 {
        sum.push(carry);
    }
    sum.reverse();
    sum
}

/// `a` minus `b` for magnitudes with the same last-digit power, where `a` is at least `b`.
fn subtract(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut difference = Vec::with_capacity(a.len());
    let mut borrow = 0;
    let mut b = b.iter().rev();
    for x in a.iter().rev() {
        let y = b.next().copied().unwrap_or(0) + borrow;
        if *x >= y {
            difference.push(x - y);
            borrow = 0;
        } else {
            difference.push(x + 10 - y);
            borrow = 1;
        }
    }
    difference.reverse();
    difference
}

fn increment(digits: &mut Vec<u8>) {
    for digit in digits.iter_mut().rev() {
        if *digit == 9 {
            *digit = 0;
        } else {
            *digit += 1;
            return;
        }
    }
    digits.insert(0, 1);
}

/// Subtract one from a nonzero magnitude.
fn decrement(digits: &mut [u8]) {
    for digit in digits.iter_mut().rev() {
        if *digit == 0 {
            *digit = 9;
        } else {
            *digit -= 1;
            return;
        }
    }
}

/// A JSON number lexeme for `digits` times 10^`power`, in plain decimal notation unless
/// that needs many more zeros than digits.
pub(crate) fn render(negative: bool, digits: &[u8], mut power: i64) -> String {
    let start = digits
        .iter()
        .position(|digit| *digit != 0)
        .unwrap_or(digits.len() - 1);
    let mut digits = &digits[start..];
    while power < 0 && digits.len() > 1 && digits.last() == Some(&0) {
        digits = &digits[..digits.len() - 1];
        power += 1;
    }
    let coefficient: String = digits
        .iter()
        .map(|digit| char::from(b'0' + digit))
        .collect();
    let length = coefficient.len() as i64;
    let mut text = String::from(if negative { "-" } else { "" });
    match power {
        0..=20 => {
            text.push_str(&coefficient);
            text.extend(std::iter::repeat_n('0', power as usize));
        }
        _ if power < 0 && -power < length => {
            let (whole, fraction) = coefficient.split_at((length + power) as usize);
            text.push_str(&format!("{whole}.{fraction}"));
        }
        _ if power < 0 && -power - length <= 20 => {
            text.push_str("0.");
            text.extend(std::iter::repeat_n('0', (-power - length) as usize));
            text.push_str(&coefficient);
        }
        _ => text.push_str(&format!("{coefficient}e{power}")),
    }
    text
}
