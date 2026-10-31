//! Bounded ASCII pattern candidates for Representative values. This deliberately small
//! synthesizer does not decide ECMAScript validity or sample admission. Unsupported syntax
//! and impossible lengths supply no candidate, leaving the ordinary representative fallback.
use super::{Exhausted, Generator};
use crate::allowance::charge;

struct Atom {
    byte: u8,
    minimum: usize,
    maximum: usize,
}

pub(super) fn synthesize(
    pattern: &str,
    minimum: usize,
    maximum: usize,
    generator: &mut Generator<'_>,
) -> Result<Option<String>, Exhausted> {
    if minimum > maximum {
        return Ok(None);
    }
    // Parsing visits each byte a bounded number of times. Pay before scanning or allocating.
    generator.work(pattern.len().saturating_mul(8).saturating_add(1))?;
    if !pattern.is_ascii() {
        return Ok(None);
    }
    let capacity = pattern.len().saturating_mul(std::mem::size_of::<Atom>());
    if !charge(&mut generator.allowance.budget.allocation, capacity) {
        return Err(Exhausted::Allocation);
    }
    let mut parser = Parser {
        bytes: pattern.as_bytes(),
        at: 0,
    };
    let anchored_start = parser.take(b'^');
    let mut atoms = Vec::with_capacity(pattern.len());
    while parser.at < parser.bytes.len() && parser.peek() != Some(b'$') {
        let Some(byte) = parser.atom() else {
            return Ok(None);
        };
        let Some((minimum, maximum)) = parser.repetition(maximum) else {
            return Ok(None);
        };
        atoms.push(Atom {
            byte,
            minimum,
            maximum,
        });
    }
    let anchored_end = parser.take(b'$');
    if parser.at != parser.bytes.len() {
        return Ok(None);
    }
    let Some(shortest) = atoms
        .iter()
        .try_fold(0usize, |sum, atom| sum.checked_add(atom.minimum))
    else {
        return Ok(None);
    };
    if shortest > maximum {
        return Ok(None);
    }
    let mut extra = minimum.saturating_sub(shortest);
    for atom in &mut atoms {
        let added = extra.min(atom.maximum - atom.minimum);
        atom.minimum += added;
        extra -= added;
    }
    // Unanchored matching can meet a length bound with padding outside the matched text.
    if extra > 0 && anchored_start && anchored_end {
        return Ok(None);
    }
    let length = shortest.max(minimum);
    generator.work(length + 1)?;
    if !charge(&mut generator.allowance.budget.allocation, length) {
        return Err(Exhausted::Allocation);
    }
    let mut text = String::with_capacity(length);
    if anchored_end {
        text.extend(std::iter::repeat_n('a', extra));
    }
    for atom in atoms {
        text.extend(std::iter::repeat_n(char::from(atom.byte), atom.minimum));
    }
    if !anchored_end {
        text.extend(std::iter::repeat_n('a', extra));
    }
    Ok(Some(text))
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.peek() != Some(byte) {
            return false;
        }
        self.at += 1;
        true
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.at += 1;
        Some(byte)
    }

    fn atom(&mut self) -> Option<u8> {
        match self.next()? {
            b'[' => self.class(),
            b'\\' => self.escape().map(|(byte, _)| byte),
            b'^' | b'$' | b'.' | b'|' | b'(' | b')' | b']' | b'{' | b'}' | b'?' | b'*' | b'+' => {
                None
            }
            byte => Some(byte),
        }
    }

    /// The first ASCII character of an escape, and whether it denotes a single character.
    fn escape(&mut self) -> Option<(u8, bool)> {
        Some(match self.next()? {
            b'd' | b'w' => (b'0', false),
            b's' => (b' ', false),
            b'n' => (b'\n', true),
            b'r' => (b'\r', true),
            b't' => (b'\t', true),
            b'f' => (12, true),
            b'v' => (11, true),
            byte @ (b'^' | b'$' | b'\\' | b'.' | b'*' | b'+' | b'?' | b'(' | b')' | b'[' | b']'
            | b'{' | b'}' | b'|' | b'/') => (byte, true),
            _ => return None,
        })
    }

    fn class_character(&mut self) -> Option<(u8, bool)> {
        match self.next()? {
            b'\\' if self.take(b'-') => Some((b'-', true)),
            b'\\' => self.escape(),
            b']' => None,
            byte => Some((byte, true)),
        }
    }

    fn class(&mut self) -> Option<u8> {
        if self.peek() == Some(b'^') {
            return None;
        }
        let mut first = None;
        while self.peek()? != b']' {
            let (byte, single) = self.class_character()?;
            first = Some(first.map_or(byte, |earlier: u8| earlier.min(byte)));
            if self.take(b'-') {
                if self.peek() == Some(b']') {
                    first = Some(first?.min(b'-'));
                    break;
                }
                let (end, end_single) = self.class_character()?;
                if !single || !end_single || byte > end {
                    return None;
                }
            }
        }
        self.take(b']');
        first
    }

    fn decimal(&mut self) -> Option<usize> {
        let start = self.at;
        let mut number = 0usize;
        while let Some(byte @ b'0'..=b'9') = self.peek() {
            number = number
                .checked_mul(10)?
                .checked_add(usize::from(byte - b'0'))?;
            self.at += 1;
        }
        (self.at > start).then_some(number)
    }

    fn repetition(&mut self, limit: usize) -> Option<(usize, usize)> {
        let (minimum, maximum) = match self.peek() {
            Some(b'?') => {
                self.at += 1;
                (0, 1)
            }
            Some(b'*') => {
                self.at += 1;
                (0, limit)
            }
            Some(b'+') => {
                self.at += 1;
                (1, limit)
            }
            Some(b'{') => {
                self.at += 1;
                let minimum = self.decimal()?;
                let maximum = if self.take(b',') {
                    if self.peek() == Some(b'}') {
                        limit
                    } else {
                        self.decimal()?
                    }
                } else {
                    minimum
                };
                if !self.take(b'}') {
                    return None;
                }
                (minimum, maximum)
            }
            _ => (1, 1),
        };
        // Lazy repeats choose the same shortest candidate as greedy repeats.
        self.take(b'?');
        (minimum <= maximum).then_some((minimum, maximum))
    }
}
