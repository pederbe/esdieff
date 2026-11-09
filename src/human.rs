//! Explicit human presentation, independent of the process environment.
use crate::{AssessmentStatus, Disposition, Severity, terminal_palette as palette};
use std::ops::{Deref, Range};

/// Presentation for human reports. Library rendering is plain unless opted in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum HumanStyle {
    #[default]
    Plain,
    /// Terminal colors, bold and status symbols, with each styled span followed immediately
    /// by a reset, and blank lines and separators that only aid reading.
    Ansi,
}

#[derive(Clone, Copy)]
pub(crate) enum Role {
    /// Report titles and section headings.
    Heading,
    /// Field labels within a line, which recede so their values stand out.
    Label,
    /// Secondary request, input and knowledge metadata.
    Muted,
    Passed,
    Failed,
    Warning,
    Incomplete,
    Notice,
}

impl Role {
    fn sequence(self) -> &'static str {
        match self {
            Self::Heading => palette::HEADING,
            Self::Label | Self::Muted => palette::MUTED,
            Self::Passed => palette::PASSED,
            Self::Failed => palette::FAILED,
            Self::Warning | Self::Incomplete => palette::ADVISORY,
            Self::Notice => palette::NOTICE,
        }
    }

    /// The symbol and space that start the line of a styled outcome, finding or section with
    /// this role.
    fn marker(self) -> &'static str {
        match self {
            Self::Heading | Self::Label | Self::Muted => "",
            Self::Passed => palette::PASSED_MARK,
            Self::Failed => palette::FAILED_MARK,
            Self::Warning | Self::Incomplete => palette::ADVISORY_MARK,
            Self::Notice => palette::NOTICE_MARK,
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

/// How styled output presents one range of the plain projection.
#[derive(Clone, Copy)]
enum Piece {
    Span(Role),
    /// A styled span that starts its line's text, introduced by its role's symbol.
    Status(Role),
    /// The role's symbol alone, at an empty range where its line's text starts, for a status
    /// later on that line.
    Mark(Role),
    /// Text in place of the plain range, which can be empty.
    Text(&'static str),
}

/// Record spans while building the plain projection so styling never changes retained
/// detail, omission accounting, or the existing JSON fallback choice. Styled output only
/// adds colors, symbols and blank lines, and replaces plain list separators.
pub(crate) struct HumanText {
    plain: String,
    pieces: Vec<(Range<usize>, Piece)>,
    /// Where the latest symbol was inserted, so each line receives at most one.
    marked: Option<usize>,
    style: HumanStyle,
}

impl HumanText {
    pub(crate) fn new(style: HumanStyle) -> Self {
        Self {
            plain: String::new(),
            pieces: Vec::new(),
            marked: None,
            style,
        }
    }

    fn piece(&mut self, piece: Option<Piece>, text: &str) {
        let start = self.plain.len();
        self.plain.push_str(text);
        if self.style == HumanStyle::Ansi
            && let Some(piece) = piece
        {
            self.pieces.push((start..self.plain.len(), piece));
        }
    }

    pub(crate) fn span(&mut self, role: impl Into<Option<Role>>, text: &str) {
        if !text.is_empty() {
            self.piece(role.into().map(Piece::Span), text);
        }
    }

    /// An outcome, finding or section heading. Styled output starts its line with a symbol,
    /// after any indentation. The first status on a line chooses the symbol.
    pub(crate) fn status(&mut self, role: Role, text: &str) {
        if text.is_empty() {
            return;
        }
        let line = self.plain.rfind('\n').map_or(0, |end| end + 1);
        let at = self.plain.len() - self.plain[line..].trim_start_matches(' ').len();
        if self.style != HumanStyle::Ansi || role.marker().is_empty() || self.marked == Some(at) {
            self.span(role, text);
            return;
        }
        self.marked = Some(at);
        if at == self.plain.len() {
            self.piece(Some(Piece::Status(role)), text);
            return;
        }
        // A blank line inserted at the same position comes first, then the symbol, then the
        // text already written on this line.
        let index = self
            .pieces
            .partition_point(|(range, _)| range.start < at || range == &(at..at));
        self.pieces.insert(index, (at..at, Piece::Mark(role)));
        self.span(role, text);
    }

    pub(crate) fn heading(&mut self, text: &str) {
        self.span(Role::Heading, text);
    }

    pub(crate) fn label(&mut self, text: &str) {
        self.span(Role::Label, text);
    }

    /// A whole line of secondary request, input or knowledge metadata.
    pub(crate) fn muted_line(&mut self, text: &str) {
        self.span(Role::Muted, text);
        self.push('\n');
    }

    pub(crate) fn assessment(&mut self, status: AssessmentStatus) {
        self.span(Role::assessment(status), status.as_str());
    }

    /// A blank line separating sections, in styled output only.
    pub(crate) fn gap(&mut self) {
        self.piece(Some(Piece::Text("\n")), "");
    }

    /// A list separator, which styled output replaces with a middle dot.
    pub(crate) fn separator(&mut self, plain: &str) {
        self.piece(Some(Piece::Text(palette::SEPARATOR)), plain);
    }

    /// Fixed wording and counts whose every `separator` divides list items.
    pub(crate) fn push_list(&mut self, text: &str, separator: &str) {
        for (index, item) in text.split(separator).enumerate() {
            if index != 0 {
                self.separator(separator);
            }
            self.push_str(item);
        }
    }

    pub(crate) fn push(&mut self, character: char) {
        self.plain.push(character);
    }

    pub(crate) fn push_str(&mut self, text: &str) {
        self.plain.push_str(text);
    }

    pub(crate) fn finish(self, limit: usize) -> String {
        let styled_len = self
            .pieces
            .iter()
            .try_fold(self.plain.len(), |size, (range, piece)| {
                let added = match piece {
                    Piece::Span(role) => role.sequence().len() + palette::RESET.len(),
                    Piece::Status(role) | Piece::Mark(role) => {
                        role.sequence().len() + role.marker().len() + palette::RESET.len()
                    }
                    Piece::Text(text) => text.len(),
                };
                let removed = match piece {
                    Piece::Text(_) => range.len(),
                    Piece::Span(_) | Piece::Status(_) | Piece::Mark(_) => 0,
                };
                size.checked_add(added).map(|size| size - removed)
            });
        let Some(styled_len) = styled_len.filter(|&size| size <= limit) else {
            return self.plain;
        };
        if self.pieces.is_empty() {
            return self.plain;
        }
        let mut styled = String::with_capacity(styled_len);
        let mut cursor = 0;
        for (range, piece) in self.pieces {
            styled.push_str(&self.plain[cursor..range.start]);
            match piece {
                Piece::Span(role) => {
                    styled.push_str(role.sequence());
                    styled.push_str(&self.plain[range.clone()]);
                    styled.push_str(palette::RESET);
                }
                Piece::Status(role) | Piece::Mark(role) => {
                    styled.push_str(role.sequence());
                    styled.push_str(role.marker());
                    styled.push_str(&self.plain[range.clone()]);
                    styled.push_str(palette::RESET);
                }
                Piece::Text(text) => styled.push_str(text),
            }
            cursor = range.end;
        }
        styled.push_str(&self.plain[cursor..]);
        styled
    }
}

// Ordinary writes remain plain; only explicit pieces receive generated styling.
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

    fn sample(style: HumanStyle) -> HumanText {
        let mut output = HumanText::new(style);
        output.heading("Outcome:");
        output.push(' ');
        output.status(Role::Passed, "passed");
        output.push('\n');
        output.label("Counts:");
        output.push(' ');
        output.span(Role::Warning, "1 warning");
        output.separator("; ");
        output.span(None, "0 notices");
        output.push('\n');
        output.gap();
        output.push_str("  ");
        output.status(Role::Warning, "warning");
        output.push_str(" detail\n");
        output
    }

    #[test]
    fn styled_output_only_adds_presentation_to_the_plain_projection() {
        let plain = "Outcome: passed\nCounts: 1 warning; 0 notices\n  warning detail\n";
        let styled = "\x1b[1;38;2;90;247;142m✔ \x1b[0m\
            \x1b[1;38;2;189;147;249mOutcome:\x1b[0m \
            \x1b[1;38;2;90;247;142mpassed\x1b[0m\n\
            \x1b[38;2;112;120;140mCounts:\x1b[0m \
            \x1b[1;38;2;243;156;18m1 warning\x1b[0m · 0 notices\n\
            \n  \x1b[1;38;2;243;156;18m⚠ warning\x1b[0m detail\n";
        assert_eq!(sample(HumanStyle::Plain).finish(usize::MAX), plain);
        let output = sample(HumanStyle::Ansi);
        assert_eq!(output.as_str(), plain);
        assert_eq!(output.finish(usize::MAX), styled);
    }

    #[test]
    fn a_line_receives_one_symbol_from_its_first_status() {
        let mut output = HumanText::new(HumanStyle::Ansi);
        output.status(Role::Failed, "failed");
        output.push(' ');
        output.status(Role::Passed, "passed");
        assert_eq!(
            output.finish(usize::MAX),
            "\x1b[1;38;2;255;92;87m✗ failed\x1b[0m \x1b[1;38;2;90;247;142mpassed\x1b[0m"
        );
    }

    #[test]
    fn style_overhead_never_drops_retained_plain_detail() {
        let styled_len = sample(HumanStyle::Ansi).finish(usize::MAX).len();
        assert_eq!(
            sample(HumanStyle::Ansi).finish(styled_len - 1),
            "Outcome: passed\nCounts: 1 warning; 0 notices\n  warning detail\n"
        );
        assert_eq!(
            sample(HumanStyle::Ansi).finish(styled_len).len(),
            styled_len
        );
    }
}
