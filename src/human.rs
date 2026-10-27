//! Explicit human presentation, independent of the process environment.
use crate::{AssessmentStatus, Disposition, Severity};
use std::ops::{Deref, Range};

/// Presentation for human reports. Library rendering is plain unless opted in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum HumanStyle {
    #[default]
    Plain,
    /// ANSI foreground and bold spans, each followed immediately by a reset.
    Ansi,
}

#[derive(Clone, Copy)]
pub(crate) enum Role {
    Heading,
    Passed,
    Failed,
    Warning,
    Incomplete,
    Notice,
}

impl Role {
    fn sequence(self) -> &'static str {
        match self {
            Self::Heading => "\x1b[1m",
            Self::Passed => "\x1b[32m",
            Self::Failed => "\x1b[31m",
            Self::Warning | Self::Incomplete => "\x1b[33m",
            Self::Notice => "\x1b[36m",
        }
    }

    pub(crate) fn disposition(disposition: Disposition) -> Self {
        match disposition {
            Disposition::Passed => Self::Passed,
            Disposition::Failed => Self::Failed,
            Disposition::Undetermined => Self::Incomplete,
        }
    }

    pub(crate) fn severity(severity: Severity) -> Self {
        match severity {
            Severity::Error => Self::Failed,
            Severity::Warning => Self::Warning,
            Severity::Notice => Self::Notice,
            Severity::Incomplete => Self::Incomplete,
        }
    }

    pub(crate) fn assessment(status: AssessmentStatus) -> Option<Self> {
        match status {
            AssessmentStatus::Unassessed | AssessmentStatus::Undetermined => Some(Self::Incomplete),
            AssessmentStatus::Assessed | AssessmentStatus::NotApplicable => None,
        }
    }
}

/// Record spans while building the plain projection so styling never changes
/// retained detail, omission accounting, or the existing JSON fallback choice.
pub(crate) struct HumanText {
    plain: String,
    spans: Vec<(Range<usize>, Role)>,
    style: HumanStyle,
}

impl HumanText {
    pub(crate) fn new(style: HumanStyle) -> Self {
        Self {
            plain: String::new(),
            spans: Vec::new(),
            style,
        }
    }

    pub(crate) fn span(&mut self, role: impl Into<Option<Role>>, text: &str) {
        let start = self.plain.len();
        self.plain.push_str(text);
        if self.style == HumanStyle::Ansi
            && let Some(role) = role.into()
            && !text.is_empty()
        {
            self.spans.push((start..self.plain.len(), role));
        }
    }

    pub(crate) fn heading(&mut self, text: &str) {
        self.span(Role::Heading, text);
    }

    pub(crate) fn assessment(&mut self, status: AssessmentStatus) {
        self.span(Role::assessment(status), status.as_str());
    }

    pub(crate) fn push(&mut self, character: char) {
        self.plain.push(character);
    }

    pub(crate) fn push_str(&mut self, text: &str) {
        self.plain.push_str(text);
    }

    pub(crate) fn finish(self, limit: usize) -> String {
        const RESET: &str = "\x1b[0m";
        let styled_len = self
            .spans
            .iter()
            .try_fold(self.plain.len(), |size, (_, role)| {
                size.checked_add(role.sequence().len() + RESET.len())
            });
        let Some(styled_len) = styled_len.filter(|&size| size <= limit) else {
            return self.plain;
        };
        if self.spans.is_empty() {
            return self.plain;
        }
        let mut styled = String::with_capacity(styled_len);
        let mut cursor = 0;
        for (range, role) in self.spans {
            styled.push_str(&self.plain[cursor..range.start]);
            styled.push_str(role.sequence());
            styled.push_str(&self.plain[range.clone()]);
            styled.push_str(RESET);
            cursor = range.end;
        }
        styled.push_str(&self.plain[cursor..]);
        styled
    }
}

// Ordinary writes remain plain; only explicit spans receive generated styling.
impl Deref for HumanText {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.plain
    }
}
impl std::fmt::Write for HumanText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.push_str(text);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_overhead_never_drops_retained_plain_detail() {
        for (limit, expected) in [(14, "passed detail"), (22, "\x1b[32mpassed\x1b[0m detail")] {
            let mut output = HumanText::new(HumanStyle::Ansi);
            output.span(Role::Passed, "passed");
            output.push_str(" detail");
            assert_eq!(output.finish(limit), expected);
        }
    }
}
