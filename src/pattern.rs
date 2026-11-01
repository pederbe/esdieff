//! Fixed Corvus pattern safeguards follow ADR 0056. Effective scopes and findings
//! retain ADR 0028: Bounded effective model pattern syntax.
use crate::AssessmentStatus;

pub(crate) const AREA: &str = "pattern-syntax/1";
pub(crate) const REPORT_RESERVE: usize = 171;

#[derive(Clone, Copy)]
pub(crate) enum Condition {
    Invalid,
    Unsupported,
    Unavailable,
    Limit,
}
impl Condition {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Invalid => "pattern.syntax",
            Self::Unsupported => "pattern.unsupported",
            Self::Unavailable => "pattern.unavailable",
            Self::Limit => "operation.limit",
        }
    }
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Invalid => {
                "The effective pattern has invalid Unicode ECMAScript regular-expression syntax"
            }
            Self::Unsupported => {
                "Effective pattern syntax requires a capability outside the qualified parser subset"
            }
            Self::Unavailable => {
                "Effective pattern syntax requires unavailable text, role or interpretation"
            }
            Self::Limit => {
                "Effective pattern syntax reached its finite operation or parser capacity"
            }
        }
    }
    pub(crate) fn basis_id(self) -> Option<&'static str> {
        match self {
            Self::Invalid => None,
            Self::Unavailable => Some("pattern-establishment"),
            Self::Unsupported | Self::Limit => Some("pattern-syntax-capability"),
        }
    }
}

#[derive(Default)]
pub(crate) struct Scope {
    pub(crate) enabled: bool,
    pub(crate) applicable: bool,
    pub(crate) unknown: bool,
}
impl Scope {
    pub(crate) fn status(&self) -> AssessmentStatus {
        if self.unknown {
            AssessmentStatus::Undetermined
        } else if self.applicable {
            AssessmentStatus::Assessed
        } else {
            AssessmentStatus::NotApplicable
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Verdict {
    Valid,
    Invalid,
    Unsupported,
    Limit,
}
#[derive(Default)]
pub(crate) struct MatchingWork {
    pub(crate) model_patterns: Vec<ModelPattern>,
    pub(crate) reported_models: usize,
}
pub(crate) struct ModelPattern {
    pub(crate) source: crate::SourceId,
    pub(crate) node: usize,
    pub(crate) invalid: bool,
    pub(crate) reported: bool,
}
impl MatchingWork {
    /// Admit independent model-condition storage before proving syntax. Its
    /// identity is the actual effective context, never a retained diagnostic.
    pub(crate) fn model_pattern(
        &mut self,
        source: crate::SourceId,
        node: usize,
        work: &mut usize,
        allocation: &mut usize,
    ) -> Option<usize> {
        // Prepay a later independent-condition drain as well as this lookup.
        if !crate::allowance::charge(work, self.model_patterns.len() + 1) {
            return None;
        }
        for (index, known) in self.model_patterns.iter().enumerate() {
            if !crate::allowance::charge(work, 1) {
                return None;
            }
            if known.source == source && known.node == node {
                return Some(index);
            }
        }
        if !crate::allowance::charge(work, 3)
            || crate::allowance::admit(&mut self.model_patterns, allocation).is_err()
        {
            return None;
        }
        let index = self.model_patterns.len();
        self.model_patterns.push(ModelPattern {
            source,
            node,
            invalid: false,
            reported: false,
        });
        Some(index)
    }
}
pub(crate) enum MatchVerdict {
    Matched,
    Mismatched,
    Invalid,
    Unsupported,
    Limit,
}

/// Compile each supplied pattern using the selected Unicode ECMAScript engine.
/// Fixed admission checks share the operation's counters. Engine-private capacity
/// and a cooperative backtracking timeout replace JavaScript runtime quotas.
#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn compile(
    pattern: &str,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> Result<corvus_regex_ecma262::Regex, Verdict> {
    let mut meter = Meter {
        work: &mut budget.work,
        allocation: &mut budget.allocation,
    };
    guards(pattern, limits, &mut meter)?;
    meter.visit(pattern.len() + 1)?;
    corvus_regex_ecma262::Regex::with_timeout(
        pattern,
        corvus_regex_ecma262::Flags {
            unicode: true,
            ..Default::default()
        },
        Some(std::time::Duration::from_millis(100)),
    )
    .map_err(|error| match error {
        corvus_regex_ecma262::Error::Invalid(_) => Verdict::Invalid,
        corvus_regex_ecma262::Error::TooLarge(_) => Verdict::Limit,
        _ => Verdict::Unsupported,
    })
}
pub(crate) fn evaluate(
    pattern: &str,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> Verdict {
    match compile(pattern, limits, budget) {
        Ok(_) => Verdict::Valid,
        Err(verdict) => verdict,
    }
}
pub(crate) fn matching_capability(
    pattern: &str,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> Verdict {
    evaluate(pattern, limits, budget)
}
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub(crate) fn matches(
    pattern: &str,
    subject: &str,
    limits: &crate::Limits,
    budget: &mut crate::allowance::OperationBudget,
) -> MatchVerdict {
    if subject.len() > limits.pattern_subject
        || !crate::allowance::charge(&mut budget.work, subject.len() + 1)
    {
        return MatchVerdict::Limit;
    }
    let regex = match compile(pattern, limits, budget) {
        Ok(regex) => regex,
        Err(Verdict::Invalid) => return MatchVerdict::Invalid,
        Err(Verdict::Unsupported) => return MatchVerdict::Unsupported,
        Err(_) => return MatchVerdict::Limit,
    };
    match regex.is_match(subject) {
        Ok(true) => MatchVerdict::Matched,
        Ok(false) => MatchVerdict::Mismatched,
        Err(_) => MatchVerdict::Limit,
    }
}

struct Meter<'a> {
    work: &'a mut usize,
    allocation: &'a mut usize,
}
impl Meter<'_> {
    fn visit(&mut self, bytes: usize) -> Result<(), Verdict> {
        crate::allowance::charge(self.work, bytes)
            .then_some(())
            .ok_or(Verdict::Limit)
    }
    fn allocate(&mut self, bytes: usize) -> Result<(), Verdict> {
        crate::allowance::charge(self.allocation, bytes)
            .then_some(())
            .ok_or(Verdict::Limit)
    }
    fn find<'a>(
        &mut self,
        text: &'a str,
        delimiter: char,
    ) -> Result<Option<(&'a str, &'a str)>, Verdict> {
        self.visit(text.len() + 1)?;
        Ok(text.split_once(delimiter))
    }
    fn decimal<'a>(&mut self, text: &'a str) -> Result<&'a str, Verdict> {
        self.visit(text.len() + 1)?;
        let normalized = text.trim_start_matches('0');
        Ok(if normalized.is_empty() {
            "0"
        } else {
            normalized
        })
    }
    fn compare_decimal(&mut self, a: &str, b: &str) -> Result<std::cmp::Ordering, Verdict> {
        self.visit(a.len() + b.len() + 1)?;
        Ok(a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
    }
}

fn guards(pattern: &str, limits: &crate::Limits, meter: &mut Meter<'_>) -> Result<(), Verdict> {
    if pattern.len() > limits.pattern_bytes {
        return Err(Verdict::Limit);
    }
    let mut names: Vec<String> = Vec::new();
    let mut in_class = false;
    let mut escaped = false;
    let mut backreference_name_end = 0;
    let mut depth = 0usize;
    let mut alternatives = 1usize;
    let mut index = 0;
    while index < pattern.len() {
        // Charge the maximum UTF-8 scalar decoder visit before reading it.
        meter.visit(5)?;
        let c = pattern[index..].chars().next().expect("remaining scalar");
        let i = index;
        index += c.len_utf8();
        if escaped {
            escaped = false;
            if c == 'u' {
                index = if i < backreference_name_end {
                    name_unicode_escape_end(pattern, index, meter)?
                } else {
                    unicode_escape_end(pattern, index, meter)?
                };
            } else if c == 'k' && !in_class {
                meter.visit(1)?;
                if pattern[index..].starts_with('<') {
                    let name = &pattern[index + 1..];
                    backreference_name_end = index
                        + 1
                        + meter
                            .find(name, '>')?
                            .map_or(name.len(), |(name, _)| name.len());
                }
            }
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        match c {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => {
                depth += 1;
                if depth > limits.pattern_nesting {
                    return Err(Verdict::Limit);
                }
                meter.visit(8)?;
                if let Some(tail) = pattern[i..]
                    .strip_prefix("(?<")
                    .filter(|tail| !tail.starts_with(['=', '!']))
                {
                    let raw = meter
                        .find(tail, '>')?
                        .map(|(name, _)| name)
                        .ok_or(Verdict::Unsupported)?;
                    let decoded = decode_name(raw, meter)?;
                    for name in &names {
                        meter.visit(name.len() + decoded.len() + 1)?;
                        if name == &decoded {
                            return Err(Verdict::Unsupported);
                        }
                    }
                    if names.len() == names.capacity() {
                        let old = names.capacity();
                        let new = old.max(4).checked_mul(2).ok_or(Verdict::Limit)?;
                        meter.allocate(
                            new.checked_mul(std::mem::size_of::<String>())
                                .ok_or(Verdict::Limit)?,
                        )?;
                        names.reserve_exact(new - old);
                    }
                    names.push(decoded);
                }
            }
            ')' if !in_class => depth = depth.saturating_sub(1),
            '|' if !in_class => {
                alternatives += 1;
                if alternatives > limits.pattern_alternatives {
                    return Err(Verdict::Limit);
                }
            }
            '{' if !in_class => {
                if let Some((body, _)) = meter.find(&pattern[index..], '}')? {
                    let (lo, hi) = meter
                        .find(body, ',')?
                        .map_or((body, None), |(lo, hi)| (lo, Some(hi)));
                    meter.visit(lo.len() + hi.map_or(0, str::len) + 1)?;
                    if !lo.is_empty()
                        && lo.bytes().all(|c| c.is_ascii_digit())
                        && hi.is_none_or(|v| v.bytes().all(|c| c.is_ascii_digit()))
                    {
                        let lo = meter.decimal(lo)?;
                        let hi = hi
                            .filter(|v| !v.is_empty())
                            .map(|v| meter.decimal(v))
                            .transpose()?;
                        if let Some(hi) = hi
                            && meter.compare_decimal(lo, hi)?.is_gt()
                        {
                            return Err(Verdict::Invalid);
                        }
                        if !meter.compare_decimal(lo, "1000000")?.is_lt()
                            || if let Some(hi) = hi {
                                !meter.compare_decimal(hi, "1000000")?.is_lt()
                            } else {
                                false
                            }
                        {
                            return Err(Verdict::Limit);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn unicode_escape_end(
    pattern: &str,
    digits_start: usize,
    meter: &mut Meter<'_>,
) -> Result<usize, Verdict> {
    let tail = &pattern[digits_start..];
    meter.visit(1)?;
    if let Some(braced) = tail.strip_prefix('{') {
        let (digits, _) = meter.find(braced, '}')?.ok_or(Verdict::Invalid)?;
        meter.visit(digits.len() + 1)?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Verdict::Invalid);
        }
        Ok(digits_start + digits.len() + 2)
    } else {
        meter.visit(4)?;
        let digits = tail.get(..4).ok_or(Verdict::Invalid)?;
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Verdict::Invalid);
        }
        Ok(digits_start + 4)
    }
}
fn unicode_escape_digits(pattern: &str, start: usize, end: usize) -> &str {
    if pattern[start..].starts_with('{') {
        &pattern[start + 1..end - 1]
    } else {
        &pattern[start..end]
    }
}
fn name_unicode_escape_end(
    pattern: &str,
    start: usize,
    meter: &mut Meter<'_>,
) -> Result<usize, Verdict> {
    let end = unicode_escape_end(pattern, start, meter)?;
    meter.visit(1)?;
    let digits = unicode_escape_digits(pattern, start, end);
    meter.visit(digits.len() + 3)?;
    if digits.trim_start_matches('0').eq_ignore_ascii_case("3e") {
        return Err(Verdict::Invalid);
    }
    Ok(end)
}
fn decode_name(raw: &str, meter: &mut Meter<'_>) -> Result<String, Verdict> {
    // Unicode escape decoding never expands the UTF-8 source length. Pay the
    // complete requested String capacity before decoding or copying any scalar.
    meter.allocate(raw.len())?;
    let mut output = String::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        meter.visit(5)?;
        let c = raw[index..].chars().next().expect("remaining name scalar");
        index += c.len_utf8();
        if c != '\\' {
            meter.visit(c.len_utf8())?;
            output.push(c);
            continue;
        }
        meter.visit(1)?;
        if !raw[index..].starts_with('u') {
            return Err(Verdict::Unsupported);
        }
        index += 1;
        let end = name_unicode_escape_end(raw, index, meter)?;
        meter.visit(1)?;
        let digits = unicode_escape_digits(raw, index, end);
        meter.visit(digits.len() + 1)?;
        let scalar = u32::from_str_radix(digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or(Verdict::Unsupported)?;
        meter.visit(scalar.len_utf8())?;
        output.push(scalar);
        index = end;
    }
    Ok(output)
}
