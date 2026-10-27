//! Borrowed civil representations follow ADR 0018: Civil date and time sample conventions.
use crate::scalar::{Context, TemporalFormat as Format};

pub(crate) const CONVENTION_USED: u8 = 1;
pub(crate) const KNOWLEDGE_USED: u8 = 2;

#[derive(Clone, Copy)]
pub(crate) enum Invalid {
    Representation,
    Calendar,
    Clock,
    ExceptionalSlot,
    NoInsertedSecond,
}

#[derive(Clone, Copy)]
pub(crate) enum Unavailable {
    Input,
    Work,
    Knowledge,
    Meaning,
}

#[derive(Clone, Copy)]
pub(crate) enum Establishment {
    Established,
    Invalid(Invalid),
    Unavailable(Unavailable),
}

// Some callers attempt the convention before a text or parameter lookup.
pub(crate) fn begin_attempt(consulted: &mut u8) {
    *consulted |= CONVENTION_USED;
}

// Establishment shares the original request counter. Refused work performs
// neither a representation scan nor a Temporal knowledge query.
pub(crate) fn establish(
    format: Format,
    text: crate::Field<&str>,
    work: &mut usize,
    consulted: &mut u8,
) -> Establishment {
    begin_attempt(consulted);
    let crate::Field::Present(text) = text else {
        return Establishment::Unavailable(Unavailable::Input);
    };
    if !text
        .len()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(256))
        .is_some_and(|cost| crate::allowance::charge(work, cost))
    {
        return Establishment::Unavailable(Unavailable::Work);
    }
    let result = evaluate(format, text);
    *consulted |= u8::from(result.knowledge_used) * KNOWLEDGE_USED;
    match result.verdict {
        Verdict::Established => Establishment::Established,
        Verdict::Representation => Establishment::Invalid(Invalid::Representation),
        Verdict::Calendar => Establishment::Invalid(Invalid::Calendar),
        Verdict::Clock => Establishment::Invalid(Invalid::Clock),
        Verdict::ExceptionalSlot => Establishment::Invalid(Invalid::ExceptionalSlot),
        Verdict::NoInsertedSecond => Establishment::Invalid(Invalid::NoInsertedSecond),
        Verdict::KnowledgeUnavailable => Establishment::Unavailable(Unavailable::Knowledge),
        Verdict::MeaningUnavailable => Establishment::Unavailable(Unavailable::Meaning),
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Date {
    year: i32,
    month: u16,
    day: u16,
}
#[derive(Clone, Copy)]
struct Clock {
    hour: u16,
    minute: u16,
    second: u16,
    offset: i16,
}
// A UTC calendar slot keeps an inserted second distinct from the next day.
// The borrowed fraction has no precision cutoff or trailing-zero significance.
#[derive(Eq, PartialEq)]
struct DatedInstant<'a> {
    date: Date,
    minute: u16,
    second: u16,
    fraction: &'a str,
}

// Both texts must already have established DateTime denotation. The caller
// precharges two text passes and bounded calendar work for this comparison.
pub(crate) fn same_dated_instant(left: &str, right: &str) -> bool {
    dated_instant(left) == dated_instant(right)
}

fn dated_instant(text: &str) -> DatedInstant<'_> {
    let bytes = text.as_bytes();
    let component = |range| decimal(&bytes[range]).expect("established date-time component");
    let date = Date {
        year: i32::from(component(0..4)),
        month: component(5..7),
        day: component(8..10),
    };
    let offset_start = bytes.len()
        - if matches!(bytes.last(), Some(b'Z' | b'z')) {
            1
        } else {
            6
        };
    let offset = if offset_start == bytes.len() - 1 {
        0
    } else {
        let minutes = i32::from(component(offset_start + 1..offset_start + 3)) * 60
            + i32::from(component(offset_start + 4..offset_start + 6));
        if bytes[offset_start] == b'-' {
            -minutes
        } else {
            minutes
        }
    };
    let minute = i32::from(component(11..13)) * 60 + i32::from(component(14..16)) - offset;
    DatedInstant {
        date: utc_date(date, minute),
        minute: minute.rem_euclid(1440) as u16,
        second: component(17..19),
        fraction: text[19..offset_start]
            .strip_prefix('.')
            .unwrap_or("")
            .trim_end_matches('0'),
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Verdict {
    Established,
    Representation,
    Calendar,
    Clock,
    ExceptionalSlot,
    NoInsertedSecond,
    KnowledgeUnavailable,
    MeaningUnavailable,
}
#[derive(Clone, Copy)]
pub(crate) struct Evaluation {
    pub(crate) verdict: Verdict,
    pub(crate) knowledge_used: bool,
}
impl From<Verdict> for Evaluation {
    fn from(verdict: Verdict) -> Self {
        Self {
            verdict,
            knowledge_used: false,
        }
    }
}
// The request precharges two text passes and bounded calendar/27-event work.
// Fractional digits stay borrowed; no normalized temporal value is retained.
fn evaluate(format: Format, text: &str) -> Evaluation {
    let bytes = text.as_bytes();
    let date = if format != Format::Time {
        if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Verdict::Representation.into();
        }
        let (Some(year), Some(month), Some(day)) = (
            decimal(&bytes[..4]),
            decimal(&bytes[5..7]),
            decimal(&bytes[8..10]),
        ) else {
            return Verdict::Representation.into();
        };
        let date = Date {
            year: i32::from(year),
            month,
            day,
        };
        if !days_in_month(date.year, month).is_some_and(|last| day > 0 && day <= last) {
            return Verdict::Calendar.into();
        }
        if format == Format::Date {
            return if bytes.len() == 10 {
                Verdict::Established
            } else {
                Verdict::Representation
            }
            .into();
        }
        if !matches!(bytes.get(10), Some(b'T' | b't')) {
            return Verdict::Representation.into();
        }
        Some(date)
    } else {
        None
    };
    let start = if format == Format::Time { 0 } else { 11 };
    let Some(bytes) = bytes.get(start..) else {
        return Verdict::Representation.into();
    };
    if bytes.len() < 9 || bytes[2] != b':' || bytes[5] != b':' {
        return Verdict::Representation.into();
    }
    let (Some(hour), Some(minute), Some(second)) = (
        decimal(&bytes[..2]),
        decimal(&bytes[3..5]),
        decimal(&bytes[6..8]),
    ) else {
        return Verdict::Representation.into();
    };
    let mut end = 8;
    if bytes[end] == b'.' {
        end += 1;
        let start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == start {
            return Verdict::Representation.into();
        }
    }
    let offset = match &bytes[end..] {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), a, b, b':', c, d] => {
            let (Some(hours), Some(minutes)) = (decimal(&[*a, *b]), decimal(&[*c, *d])) else {
                return Verdict::Representation.into();
            };
            if hours > 23 || minutes > 59 {
                return Verdict::Clock.into();
            }
            let offset = (hours * 60 + minutes) as i16;
            if *sign == b'-' { -offset } else { offset }
        }
        _ => return Verdict::Representation.into(),
    };
    let clock = Clock {
        hour,
        minute,
        second,
        offset,
    };
    if clock.hour > 23 || clock.minute > 59 || clock.second > 60 {
        return Verdict::Clock.into();
    }
    if clock.second < 59 {
        return Verdict::Established.into();
    }
    let utc_minute = i32::from(clock.hour) * 60 + i32::from(clock.minute) - i32::from(clock.offset);
    let boundary = utc_minute.rem_euclid(1440) == 1439;
    if clock.second == 60 && !boundary {
        return Verdict::ExceptionalSlot.into();
    }
    let Some(date) = date else {
        return if clock.second == 60 {
            Verdict::MeaningUnavailable
        } else {
            Verdict::Established
        }
        .into();
    };
    let date = utc_date(date, utc_minute);
    let boundary = boundary && Some(date.day) == days_in_month(date.year, date.month);
    if clock.second == 60 && !boundary {
        return Verdict::ExceptionalSlot.into();
    }
    if !boundary {
        return Verdict::Established.into();
    }
    knowledge(date.year, date.month, date.day, clock.second)
}
fn utc_date(mut date: Date, minute: i32) -> Date {
    match minute.div_euclid(1440) {
        -1 => {
            if date.day > 1 {
                date.day -= 1;
            } else {
                if date.month == 1 {
                    date.year -= 1;
                    date.month = 12;
                } else {
                    date.month -= 1;
                }
                date.day = days_in_month(date.year, date.month).expect("previous Gregorian month");
            }
        }
        1 => {
            if date.day < days_in_month(date.year, date.month).expect("Gregorian month") {
                date.day += 1;
            } else {
                date.day = 1;
                if date.month == 12 {
                    date.year += 1;
                    date.month = 1;
                } else {
                    date.month += 1;
                }
            }
        }
        _ => {}
    }
    date
}
fn decimal(bytes: &[u8]) -> Option<u16> {
    bytes.iter().try_fold(0, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + u16::from(byte - b'0'))
    })
}
pub(crate) fn days_in_month(year: i32, month: u16) -> Option<u16> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            29
        } else {
            28
        }),
        _ => None,
    }
}
pub(crate) fn knowledge(year: i32, month: u16, day: u16, second: u16) -> Evaluation {
    let date = (year, month, day);
    let verdict = if !((1972, 1, 1)..=(2026, 12, 31)).contains(&date) {
        Verdict::KnowledgeUnavailable
    } else if second == 60 && !INSERTIONS.contains(&date) {
        Verdict::NoInsertedSecond
    } else {
        Verdict::Established
    };
    Evaluation {
        verdict,
        knowledge_used: true,
    }
}
pub(crate) fn fixed_headers() -> usize {
    use std::mem::size_of;
    size_of::<Context>()
        + size_of::<Format>()
        + 2 * size_of::<Evaluation>()
        + size_of::<Date>()
        + size_of::<Clock>()
        + size_of::<Option<Date>>()
        + size_of::<Option<Clock>>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<&[u8]>()
        + size_of::<[usize; 8]>()
        + 4 * size_of::<i32>()
        + 6 * size_of::<u16>()
        + size_of::<crate::Field<crate::EffectiveJson<'_>>>()
        + size_of::<Option<usize>>()
        + size_of::<(crate::finding::Basis, &str, &str)>()
        + size_of::<Establishment>()
        + 2 * size_of::<crate::Field<&str>>()
        + size_of::<Option<Context>>()
        + size_of::<std::array::IntoIter<&str, 2>>()
        + 4 * size_of::<bool>()
        + size_of::<crate::role::Role>()
        + size_of::<Option<&str>>()
        + size_of::<Option<Format>>()
        // Instant comparison follows establishment and retains no evaluator
        // frame. Its existing storage also funds establish's argument copies
        // and counter reborrows, without increasing this containing reserve.
        + 2 * size_of::<DatedInstant<'_>>()
        + 2 * size_of::<Date>()
        + 4 * size_of::<usize>()
        + 4 * size_of::<i32>()
        + 2 * size_of::<std::ops::Range<usize>>()
        + 4 * size_of::<&str>()
}
const INSERTIONS: [(i32, u16, u16); 27] = [
    (1972, 6, 30),
    (1972, 12, 31),
    (1973, 12, 31),
    (1974, 12, 31),
    (1975, 12, 31),
    (1976, 12, 31),
    (1977, 12, 31),
    (1978, 12, 31),
    (1979, 12, 31),
    (1981, 6, 30),
    (1982, 6, 30),
    (1983, 6, 30),
    (1985, 6, 30),
    (1987, 12, 31),
    (1989, 12, 31),
    (1990, 12, 31),
    (1992, 6, 30),
    (1993, 6, 30),
    (1994, 6, 30),
    (1995, 12, 31),
    (1997, 6, 30),
    (1998, 12, 31),
    (2005, 12, 31),
    (2008, 12, 31),
    (2012, 6, 30),
    (2015, 6, 30),
    (2016, 12, 31),
];
