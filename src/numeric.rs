//! Exact divisibility follows ADR0012: Exact numbers and JSON input policies,
//! and ADR0025: Selected numeric samples, without exponent-sized buffers.
use crate::{ExactNumber, NumberWork};
/// A capability numeric proof work can lack.
#[derive(Clone, Copy)]
pub(crate) enum NumericCapability {
    Meaning,
    Number,
    Work,
    Storage,
}
impl NumericCapability {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::Meaning => "numeric meaning unavailable",
            Self::Number => "exact numeric value unavailable",
            Self::Work => "exact numeric work limit",
            Self::Storage => "numeric derived storage limit",
        }
    }
}
pub(crate) const QUOTIENT_DIGITS: usize = 1025;
pub(crate) const FACTOR_DIGITS: usize = 513;
pub(crate) struct IntervalWorkspace {
    pub(crate) quotient: [Vec<u8>; 3],
    pub(crate) factor: [Vec<u8>; 3],
}

#[derive(Clone, Copy)]
pub(crate) struct Endpoint<'a> {
    pub(crate) value: ExactNumber<'a>,
    pub(crate) exclusive: bool,
}

#[derive(Clone, Copy)]
struct Lattice<'a> {
    coefficient: &'a [u8],
    power: i64,
}
#[derive(Clone, Copy)]
struct VirtualDigits<'a> {
    coefficient: &'a [u8],
    length: usize,
}
impl VirtualDigits<'_> {
    fn digit(self, index: usize) -> u8 {
        self.coefficient.get(index).copied().unwrap_or(b'0')
    }
}

fn capability(reason: &'static str) -> NumericCapability {
    if matches!(
        reason,
        "numeric derived storage limit" | "numeric derived width limit"
    ) {
        NumericCapability::Storage
    } else {
        NumericCapability::Work
    }
}

/// Compare rounded lattice indices without multiplying a candidate or expanding a power.
pub(crate) fn interval_gap(
    endpoints: [Endpoint<'_>; 2],
    step: Option<ExactNumber<'_>>,
    integer: bool,
    work: &mut NumberWork,
    allocation: &mut usize,
) -> Result<bool, NumericCapability> {
    interval_gap_exact(endpoints, step, integer, work, allocation).map_err(capability)
}

fn interval_gap_exact(
    endpoints: [Endpoint<'_>; 2],
    step: Option<ExactNumber<'_>>,
    integer: bool,
    work: &mut NumberWork,
    allocation: &mut usize,
) -> Result<bool, &'static str> {
    work.debit(1)?;
    *allocation = allocation
        .checked_sub(std::mem::size_of::<IntervalWorkspace>())
        .ok_or("numeric derived storage limit")?;
    let mut workspace = IntervalWorkspace {
        quotient: std::array::from_fn(|_| Vec::new()),
        factor: std::array::from_fn(|_| Vec::new()),
    };
    let lattice = match step {
        None => Lattice {
            coefficient: b"1",
            power: 0,
        },
        Some(step) if integer && step.decimal.power < 0 => {
            work.debit(3)?;
            *allocation = allocation
                .checked_sub(3 * FACTOR_DIGITS)
                .ok_or("numeric derived storage limit")?;
            workspace.factor = std::array::from_fn(|_| Vec::with_capacity(FACTOR_DIGITS));
            let [coefficient, trial, _] = &mut workspace.factor;
            work.debit(step.decimal.coefficient.len())?;
            coefficient.extend_from_slice(step.decimal.coefficient.as_bytes());
            let limit = i128::from(step.decimal.power).unsigned_abs();
            for prime in [2, 5] {
                let mut removed = 0;
                loop {
                    work.debit(1)?;
                    if removed >= limit {
                        break;
                    }
                    work.debit(coefficient.len())?;
                    if coefficient == b"1" {
                        break;
                    }
                    trial.clear();
                    trial.extend_from_slice(coefficient);
                    if divide_small(trial, prime, work)? != 0 {
                        break;
                    }
                    std::mem::swap(coefficient, trial);
                    removed += 1;
                }
            }
            Lattice {
                coefficient,
                power: 0,
            }
        }
        Some(step) => Lattice {
            coefficient: step.decimal.coefficient.as_bytes(),
            power: step.decimal.power,
        },
    };
    // Unsupported widths are refused before any quotient allocation or virtual scan.
    for endpoint in endpoints {
        virtual_ratio(endpoint.value, lattice, work)?;
    }
    work.debit(3)?;
    *allocation = allocation
        .checked_sub(3 * QUOTIENT_DIGITS)
        .ok_or("numeric derived storage limit")?;
    workspace.quotient = std::array::from_fn(|_| Vec::with_capacity(QUOTIENT_DIGITS));
    let [low, high, remainder] = &mut workspace.quotient;
    let low_negative = rounded_index(endpoints[0], lattice, true, low, remainder, work)?;
    let high_negative = rounded_index(endpoints[1], lattice, false, high, remainder, work)?;
    work.debit(low.len().max(high.len()) + 1)?;
    let order = if low_negative != high_negative {
        high_negative.cmp(&low_negative)
    } else {
        let order = low
            .len()
            .cmp(&high.len())
            .then_with(|| low.as_slice().cmp(high.as_slice()));
        if low_negative { order.reverse() } else { order }
    };
    Ok(order.is_gt())
}

fn rounded_index(
    endpoint: Endpoint<'_>,
    step: Lattice<'_>,
    lower: bool,
    quotient: &mut Vec<u8>,
    remainder: &mut Vec<u8>,
    work: &mut NumberWork,
) -> Result<bool, &'static str> {
    let number = endpoint.value;
    let integral = quotient_floor_magnitude(number, step, quotient, remainder, work)?;
    let mut negative = number.sign().is_lt();
    if !integral && lower != negative {
        increment_magnitude(quotient, work)?;
    }
    if quotient == b"0" {
        negative = false;
    }
    if integral && endpoint.exclusive {
        adjust_index(quotient, &mut negative, lower, work)?;
    }
    Ok(negative)
}

fn quotient_floor_magnitude(
    number: ExactNumber<'_>,
    step: Lattice<'_>,
    quotient: &mut Vec<u8>,
    remainder: &mut Vec<u8>,
    work: &mut NumberWork,
) -> Result<bool, &'static str> {
    let ratio = virtual_ratio(number, step, work)?;
    quotient.clear();
    remainder.clear();
    let Some((numerator, denominator)) = ratio else {
        work.debit(1)?;
        quotient.push(b'0');
        return Ok(number.sign().is_eq());
    };
    for index in 0..numerator.length {
        work.debit(1)?;
        remainder.push(numerator.digit(index));
        trim(remainder, work)?;
        let mut digit = b'0';
        loop {
            work.debit(remainder.len().max(denominator.length) + 1)?;
            let order = remainder.len().cmp(&denominator.length).then_with(|| {
                remainder
                    .iter()
                    .enumerate()
                    .map(|(i, digit)| digit.cmp(&denominator.digit(i)))
                    .find(|order| !order.is_eq())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            if order.is_lt() {
                break;
            }
            work.debit(remainder.len() + denominator.length)?;
            let mut borrow = 0i16;
            for offset in 0..remainder.len() {
                let at = remainder.len() - 1 - offset;
                let sub = denominator
                    .length
                    .checked_sub(offset + 1)
                    .map_or(0, |i| i16::from(denominator.digit(i) - b'0'));
                let value = i16::from(remainder[at] - b'0') - sub - borrow;
                remainder[at] = b'0' + value.rem_euclid(10) as u8;
                borrow = i16::from(value < 0);
            }
            trim(remainder, work)?;
            digit += 1;
        }
        work.debit(1)?;
        if digit != b'0' || !quotient.is_empty() {
            quotient.push(digit);
        }
    }
    work.debit(remainder.len() + 1)?;
    if quotient.is_empty() {
        quotient.push(b'0');
    }
    Ok(remainder.is_empty())
}

fn virtual_ratio<'a>(
    number: ExactNumber<'a>,
    step: Lattice<'a>,
    work: &mut NumberWork,
) -> Result<Option<(VirtualDigits<'a>, VirtualDigits<'a>)>, &'static str> {
    work.debit(number.decimal.coefficient.len().max(step.coefficient.len()) + 1)?;
    if number.decimal.coefficient == "0" {
        return Ok(None);
    }
    let coefficient = number.decimal.coefficient.as_bytes();
    let magnitude = (coefficient.len() as i128 + i128::from(number.decimal.power))
        .cmp(&(step.coefficient.len() as i128 + i128::from(step.power)))
        .then_with(|| {
            (0..coefficient.len().max(step.coefficient.len()))
                .map(|i| {
                    coefficient
                        .get(i)
                        .copied()
                        .unwrap_or(b'0')
                        .cmp(&step.coefficient.get(i).copied().unwrap_or(b'0'))
                })
                .find(|order| !order.is_eq())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    // This shortcut precedes virtual-width admission, including huge tiny ratios.
    if magnitude.is_lt() {
        return Ok(None);
    }
    let delta = i128::from(number.decimal.power) - i128::from(step.power);
    let length = |digits: usize, padding: i128| {
        usize::try_from(digits as i128 + padding.max(0))
            .ok()
            .filter(|length| *length < QUOTIENT_DIGITS)
            .ok_or("numeric derived width limit")
    };
    let numerator = VirtualDigits {
        coefficient,
        length: length(coefficient.len(), delta)?,
    };
    let denominator = VirtualDigits {
        coefficient: step.coefficient,
        length: length(step.coefficient.len(), -delta)?,
    };
    Ok(Some((numerator, denominator)))
}

fn increment_magnitude(digits: &mut Vec<u8>, work: &mut NumberWork) -> Result<(), &'static str> {
    work.debit(digits.len() + 1)?;
    for digit in digits.iter_mut().rev() {
        if *digit < b'9' {
            *digit += 1;
            return Ok(());
        }
        *digit = b'0';
    }
    if digits.len() == QUOTIENT_DIGITS {
        return Err("numeric derived width limit");
    }
    digits.insert(0, b'1');
    Ok(())
}

fn adjust_index(
    digits: &mut Vec<u8>,
    negative: &mut bool,
    up: bool,
    work: &mut NumberWork,
) -> Result<(), &'static str> {
    work.debit(1)?;
    if digits == b"0" {
        digits[0] = b'1';
        *negative = !up;
        return Ok(());
    }
    if up != *negative {
        return increment_magnitude(digits, work);
    }
    work.debit(digits.len())?;
    for digit in digits.iter_mut().rev() {
        if *digit > b'0' {
            *digit -= 1;
            break;
        }
        *digit = b'9';
    }
    trim(digits, work)?;
    if digits.is_empty() {
        digits.push(b'0');
        *negative = false;
    }
    Ok(())
}
pub(crate) fn multiple(
    value: ExactNumber<'_>,
    step: ExactNumber<'_>,
    work: &mut NumberWork,
    allocation: &mut usize,
) -> Result<bool, &'static str> {
    work.debit(1)?;
    if value.decimal.coefficient == "0" {
        return Ok(true);
    }
    let power = value.decimal.power - step.decimal.power;
    if power < 0 {
        return Ok(false);
    }
    let cost = (value.decimal.coefficient.len() + step.decimal.coefficient.len() + 2)
        .checked_mul(8)
        .ok_or("numeric buffer overflow")?;
    *allocation = allocation
        .checked_sub(cost)
        .ok_or("numeric derived storage limit")?;
    work.debit(value.decimal.coefficient.len() + step.decimal.coefficient.len())?;
    let mut denominator = step.decimal.coefficient.as_bytes().to_vec();
    let mut needed = [0usize; 2];
    for (index, prime) in [2, 5].into_iter().enumerate() {
        loop {
            work.debit(denominator.len())?;
            let mut divided = denominator.clone();
            if divide_small(&mut divided, prime, work)? != 0 {
                break;
            }
            needed[index] += 1;
            denominator = divided;
        }
    }
    for (index, prime) in [2, 5].into_iter().enumerate() {
        let required = needed[index].saturating_sub(usize::try_from(power).unwrap_or(usize::MAX));
        if required == 0 {
            continue;
        }
        work.debit(value.decimal.coefficient.len())?;
        let mut numerator = value.decimal.coefficient.as_bytes().to_vec();
        for _ in 0..required {
            if divide_small(&mut numerator, prime, work)? != 0 {
                return Ok(false);
            }
        }
    }
    if denominator == b"1" {
        return Ok(true);
    }
    let mut remainder = Vec::with_capacity(denominator.len() + 1);
    for digit in value.decimal.coefficient.bytes() {
        work.debit(1)?;
        remainder.push(digit);
        trim(&mut remainder, work)?;
        loop {
            work.debit(remainder.len().max(denominator.len()) + 1)?;
            if remainder.len() < denominator.len()
                || remainder.len() == denominator.len() && remainder < denominator
            {
                break;
            }
            subtract(&mut remainder, &denominator, work)?;
        }
    }
    Ok(remainder.is_empty() || remainder == b"0")
}
fn divide_small(
    digits: &mut Vec<u8>,
    divisor: u8,
    work: &mut NumberWork,
) -> Result<u8, &'static str> {
    work.debit(digits.len())?;
    let mut remainder = 0;
    for digit in digits.iter_mut() {
        let n = remainder * 10 + *digit - b'0';
        *digit = b'0' + n / divisor;
        remainder = n % divisor;
    }
    trim(digits, work)?;
    if digits.is_empty() {
        digits.push(b'0');
    }
    Ok(remainder)
}
fn trim(digits: &mut Vec<u8>, work: &mut NumberWork) -> Result<(), &'static str> {
    work.debit(digits.len())?;
    let leading = digits.iter().take_while(|digit| **digit == b'0').count();
    if leading > 0 {
        digits.drain(..leading);
    }
    Ok(())
}
fn subtract(a: &mut Vec<u8>, b: &[u8], work: &mut NumberWork) -> Result<(), &'static str> {
    work.debit(a.len() + b.len())?;
    let mut borrow = 0i16;
    for offset in 0..a.len() {
        let index = a.len() - 1 - offset;
        let sub = b
            .len()
            .checked_sub(offset + 1)
            .map_or(0, |index| i16::from(b[index] - b'0'));
        let n = i16::from(a[index] - b'0') - sub - borrow;
        a[index] = b'0' + n.rem_euclid(10) as u8;
        borrow = i16::from(n < 0);
    }
    trim(a, work)
}
