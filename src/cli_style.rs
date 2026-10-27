//! Destination-aware CLI presentation. Library renderers remain environment independent.
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
            Some("--no-color") => {
                *self = Self::Never;
                return Ok(true);
            }
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
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                terminal
                    && !std::env::var_os("TERM").is_some_and(|value| value == "dumb")
                    && !std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
            }
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Style(bool);

pub(crate) enum Role {
    Heading,
    Syntax,
    Notice,
    Passed,
    Incomplete,
    Failure,
}

impl Style {
    pub(crate) const PLAIN: Self = Self(false);

    pub(crate) fn enabled(self) -> bool {
        self.0
    }

    pub(crate) fn span(self, role: Role, text: &str) -> Cow<'_, str> {
        if !self.enabled() {
            return Cow::Borrowed(text);
        }
        let sequence = match role {
            Role::Heading => "\x1b[1m",
            Role::Syntax | Role::Notice => "\x1b[36m",
            Role::Passed => "\x1b[32m",
            Role::Incomplete => "\x1b[33m",
            Role::Failure => "\x1b[31m",
        };
        Cow::Owned(format!("{sequence}{text}\x1b[0m"))
    }

    /// The supplied help rows contain only fixed option syntax, never source content.
    fn syntax(self, text: &str) -> String {
        let mut rendered = String::new();
        let mut remaining = text;
        while let Some(start) = remaining.find("--") {
            rendered.push_str(&remaining[..start]);
            remaining = &remaining[start..];
            let end = remaining
                .find(|character: char| {
                    character != '-'
                        && !character.is_ascii_lowercase()
                        && !character.is_ascii_digit()
                })
                .unwrap_or(remaining.len());
            rendered.push_str(&self.span(Role::Syntax, &remaining[..end]));
            remaining = &remaining[end..];
        }
        rendered.push_str(remaining);
        rendered
    }
}

pub(crate) fn help(style: Style) -> String {
    let mut help = format!("{}\n", style.span(Role::Heading, "Usage:"));
    help.push_str(&format!(
        "{} {}\n",
        style.span(Role::Syntax, "esdieff"),
        style.span(Role::Syntax, "--version")
    ));
    for (command, options) in [
        ("check", "--plan PLAN [--format human|json]"),
        (
            "acquire",
            "--github OWNER/REPOSITORY --commit COMMIT --path REPOSITORY_PATH --sha256 DIGEST --document-output DEST [--report-output DEST] [--format human|json]",
        ),
        (
            "check",
            "[--directory DIR] [--recursive] [--exclude RELATIVE_PATH] [--root INPUT] [--assessment initial-v1|expanded-modified-v1|expanded-units-v1|expanded-content-format-v1|expanded-pattern-syntax-v1|expanded-numeric-v1|expanded-consistency-v1|full-baseline] [--registry-knowledge recorded|withheld] [--strict-warnings] [--format human|json|sarif] [INPUT...]",
        ),
        (
            "inspect",
            "--view source|effective --document INPUT --pointer POINTER [--required-declarations] [--format human|json] INPUT...",
        ),
        (
            "sample",
            "--document MODEL --kind property|data|action_input|action_output|event_output --pointer POINTER --sample SAMPLE [--strict-warnings] [--format human|json] MODEL_INPUT...",
        ),
        (
            "generate",
            "--document MODEL [--kind property|data|action_input|action_output|event_output --pointer POINTER] [--format human|json|jsonl] MODEL_INPUT...",
        ),
        (
            "trace",
            "--document MODEL --trace FILE [--format human|json] MODEL_INPUT...",
        ),
        (
            "author",
            "--plan PLAN --document-output DEST [--report-output DEST] [--check initial-v1|full-baseline] [--strict-warnings] [--format human|json] [--replace-document] [--replace-report] --source KEY PATH...",
        ),
        (
            "edit",
            "--plan PLAN --document-output DEST [--report-output DEST] [--check initial-v1|full-baseline] [--strict-warnings] [--format human|json] [--replace-document] [--replace-report] --source KEY PATH...",
        ),
        (
            "diff",
            "--old PATH --new PATH [--recursive] [--exclude RELATIVE_PATH] [--acknowledged FILE] [--acknowledgements-output FILE] [--replace-acknowledgements] [--format human|json|sarif|markdown]",
        ),
    ] {
        help.push_str(&format!(
            "{} {} {}\n",
            style.span(Role::Syntax, "esdieff"),
            style.span(Role::Syntax, command),
            style.syntax(options)
        ));
    }
    help.push_str("\nDirect check accepts files, one stdin document, and repeatable --directory DIR.\nDirectory selection includes immediate regular files ending exactly in .sdf.json.\n--recursive descends through all supplied directories, including hidden entries.\nRepeat --exclude RELATIVE_PATH for literal, case-sensitive file or subtree paths\nrelative to each directory. No globbing or parent/current components are allowed.\nTraversal options require directories and apply to direct check and diff.\nDiff takes two files or two directories and pairs documents by relative path.\nDescendant links and special files are skipped. Explicit directory links are allowed.\nRoots select exact joined paths relative to the invocation directory.\nDiscovery stops before checking on empty selections, overlaps or setup failures.\nFixed ceilings: 256 directories, 256 exclusions, 64 levels below each root,\n10,000 entries, 256 sources, 1 MiB path metadata.\n");
    help.push_str(&global_options(style));
    help.push_str("\nUse - once across model, sample and trace standard input.\n");
    help
}

pub(crate) fn acquire_help(style: Style) -> String {
    format!(
        "{}\n{} {} {}\n{}",
        style.span(Role::Heading, "Usage:"),
        style.span(Role::Syntax, "esdieff"),
        style.span(Role::Syntax, "acquire"),
        style.syntax("--github OWNER/REPOSITORY --commit COMMIT --path REPOSITORY_PATH --sha256 DIGEST --document-output DESTINATION [--report-output DESTINATION] [--format human|json]"),
        global_options(style)
    )
}

fn global_options(style: Style) -> String {
    format!(
        "\n{}\n  {} auto|always|never, {}=MODE  Choose styling (default: auto).\n  {}                         Alias for --color never.\n  {}, {}                         Show this help.\n",
        style.span(Role::Heading, "Global options:"),
        style.span(Role::Syntax, "--color"),
        style.span(Role::Syntax, "--color"),
        style.span(Role::Syntax, "--no-color"),
        style.span(Role::Syntax, "-h"),
        style.span(Role::Syntax, "--help"),
    )
}
