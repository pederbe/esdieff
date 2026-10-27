//! Normative revision-metadata syntax and calendar meaning, separate from sample formats.
//! Follows ADR 0021: Explicit modified metadata assessment, and
//! ADR 0018: Civil date and time sample conventions for deliberately recorded leap facts.

#[derive(Clone, Copy)]
pub(crate) enum Verdict {
    Established,
    Syntax,
    Calendar,
    Clock,
    ExceptionalSlot,
    NoInsertedSecond,
    KnowledgeUnavailable,
}
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

// The caller precharges two text passes plus bounded calendar and 27-event knowledge work.
// No fractional digits are copied, rounded, normalized or converted to a fixed-scale number.
pub(crate) fn evaluate(text: &str) -> Evaluation {
    let bytes = text.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Verdict::Syntax.into();
    }
    let (Some(year), Some(month), Some(day)) = (
        decimal(&bytes[..4]),
        decimal(&bytes[5..7]),
        decimal(&bytes[8..10]),
    ) else {
        return Verdict::Syntax.into();
    };
    let Some(last_day) = crate::civil::days_in_month(i32::from(year), month) else {
        return Verdict::Calendar.into();
    };
    if day == 0 || day > last_day {
        return Verdict::Calendar.into();
    }
    if bytes.len() == 10 {
        return Verdict::Established.into();
    }
    if bytes.len() < 20
        || !matches!(bytes[10], b'T' | b't')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return Verdict::Syntax.into();
    }
    let (Some(hour), Some(minute), Some(second)) = (
        decimal(&bytes[11..13]),
        decimal(&bytes[14..16]),
        decimal(&bytes[17..19]),
    ) else {
        return Verdict::Syntax.into();
    };
    if hour > 23 || minute > 59 || second > 60 {
        return Verdict::Clock.into();
    }
    let mut end = 19;
    if bytes[end] == b'.' {
        end += 1;
        let start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == start {
            return Verdict::Syntax.into();
        }
    }
    if !matches!(bytes.get(end..), Some([b'Z' | b'z'])) {
        return Verdict::Syntax.into();
    }
    let boundary = hour == 23 && minute == 59 && day == last_day;
    if second == 60 && !boundary {
        return Verdict::ExceptionalSlot.into();
    }
    if second < 59 || !boundary {
        return Verdict::Established.into();
    }
    let knowledge = crate::civil::knowledge(i32::from(year), month, day, second);
    let verdict = match knowledge.verdict {
        crate::civil::Verdict::KnowledgeUnavailable => Verdict::KnowledgeUnavailable,
        crate::civil::Verdict::NoInsertedSecond => Verdict::NoInsertedSecond,
        _ => Verdict::Established,
    };
    Evaluation {
        verdict,
        knowledge_used: knowledge.knowledge_used,
    }
}
fn decimal(bytes: &[u8]) -> Option<u16> {
    bytes.iter().try_fold(0, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + u16::from(byte - b'0'))
    })
}
