//! Destination-aware CLI presentation. Library renderers remain environment independent.
use crate::terminal_palette as palette;
use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    io::IsTerminal,
};

#[derive(Clone, Copy, Default)]
pub(crate) enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorMode {
    /// Called only for the current option, after its token has been consumed.
    /// Values belonging to another option and literal inputs never reach this seam.
    pub(crate) fn consume(
        &mut self,
        flag: &OsStr,
        args: &[OsString],
        index: &mut usize,
    ) -> Result<bool, (u8, String)> {
        let value = match flag.to_str() {
            Some("--color") => {
                let value = args
                    .get(*index)
                    .ok_or_else(|| (2, "--color requires auto, always or never".to_owned()))?;
                *index += 1;
                value.to_str()
            }
            Some(flag) if flag.starts_with("--color=") => flag.strip_prefix("--color="),
            _ => return Ok(false),
        };
        let mode = match value {
            Some("auto") => Self::Auto,
            Some("always") => Self::Always,
            Some("never") => Self::Never,
            _ => return Err((2, "color must be auto, always or never".to_owned())),
        };
        *self = mode;
        Ok(true)
    }

    pub(crate) fn stdout(self) -> Style {
        self.for_terminal(std::io::stdout().is_terminal())
    }

    pub(crate) fn stderr(self) -> Style {
        self.for_terminal(std::io::stderr().is_terminal())
    }

    pub(crate) fn for_terminal(self, terminal: bool) -> Style {
        Style(match self {
            Self::Always => {
                // Forced styling is written even where the console cannot show it.
                enable_console_ansi();
                true
            }
            Self::Never => false,
            Self::Auto => {
                terminal
                    && !std::env::var_os("TERM").is_some_and(|value| value == "dumb")
                    && !std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
                    && enable_console_ansi()
            }
        })
    }
}

/// Turns on virtual terminal processing in a Windows console, once, and reports whether
/// the console now interprets ANSI styling. Other terminals interpret it directly.
fn enable_console_ansi() -> bool {
    #[cfg(windows)]
    {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        // This reaches the console through CONOUT$, so it works while stdout or stderr is redirected.
        *ENABLED.get_or_init(|| nu_ansi_term::enable_ansi_support().is_ok())
    }
    #[cfg(not(windows))]
    true
}

#[derive(Clone, Copy)]
pub(crate) struct Style(bool);

pub(crate) enum Role {
    Heading,
    /// Field names within a line, which recede so their values stand out.
    Label,
    /// Command and option names.
    Command,
    /// Placeholders for values the user supplies.
    Placeholder,
}

impl Style {
    pub(crate) fn enabled(self) -> bool {
        self.0
    }

    pub(crate) fn span(self, role: Role, text: &str) -> Cow<'_, str> {
        if !self.enabled() {
            return Cow::Borrowed(text);
        }
        let sequence = match role {
            Role::Heading => palette::HEADING,
            Role::Command => palette::COMMAND,
            Role::Label | Role::Placeholder => palette::MUTED,
        };
        Cow::Owned(format!("{sequence}{text}{}", palette::RESET))
    }

    /// A diagnostic or failure label. The explanation that follows stays in the default
    /// foreground.
    pub(crate) fn failure(self, text: &str) -> Cow<'_, str> {
        self.colored(palette::FAILED, text)
    }

    /// A case disposition in its outcome colour.
    pub(crate) fn disposition(self, disposition: esdieff::Disposition) -> Cow<'static, str> {
        self.colored(
            Self::disposition_palette(disposition).0,
            disposition.as_str(),
        )
    }

    /// The symbol that starts the line of a case disposition, or nothing when plain.
    pub(crate) fn disposition_mark(self, disposition: esdieff::Disposition) -> Cow<'static, str> {
        if !self.enabled() {
            return Cow::Borrowed("");
        }
        let (sequence, mark) = Self::disposition_palette(disposition);
        self.colored(sequence, mark)
    }

    fn disposition_palette(disposition: esdieff::Disposition) -> (&'static str, &'static str) {
        match disposition {
            esdieff::Disposition::Passed => (palette::PASSED, palette::PASSED_MARK),
            esdieff::Disposition::Failed => (palette::FAILED, palette::FAILED_MARK),
            _ => (palette::ADVISORY, palette::ADVISORY_MARK),
        }
    }

    fn colored<'a>(self, sequence: &str, text: &'a str) -> Cow<'a, str> {
        if !self.enabled() {
            return Cow::Borrowed(text);
        }
        Cow::Owned(format!("{sequence}{text}{}", palette::RESET))
    }

    /// The supplied help rows contain only fixed option syntax, never source content.
    /// Options are bold and uppercase placeholders are gray.
    pub(crate) fn syntax(self, text: &str) -> String {
        let mut rendered = String::new();
        let mut remaining = text;
        while !remaining.is_empty() {
            let word = |character: char| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            };
            let start = remaining.find(word).unwrap_or(remaining.len());
            rendered.push_str(&remaining[..start]);
            remaining = &remaining[start..];
            let end = remaining
                .find(|character: char| !word(character))
                .unwrap_or(remaining.len());
            let token = &remaining[..end];
            let role = if token.starts_with('-') {
                Some(Role::Command)
            } else if token.starts_with(|character: char| character.is_ascii_uppercase())
                && token
                    .chars()
                    .all(|character| matches!(character, 'A'..='Z' | '_' | '.'))
            {
                Some(Role::Placeholder)
            } else {
                None
            };
            match role {
                Some(role) => rendered.push_str(&self.span(role, token)),
                None => rendered.push_str(token),
            }
            remaining = &remaining[end..];
        }
        rendered
    }
}
