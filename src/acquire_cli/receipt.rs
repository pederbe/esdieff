use super::Request;
use crate::cli_style::{Role, Style};
use serde_json::{Value, json};
use std::{borrow::Cow, ffi::OsStr, fs::File, path::PathBuf};

#[derive(Clone)]
pub(super) struct Failure {
    pub(super) phase: &'static str,
    pub(super) message: String,
}
impl Failure {
    pub(super) fn new(phase: &'static str, message: impl AsRef<str>) -> Self {
        let value = message.as_ref();
        let mut end = value.len().min(512);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        Self {
            phase,
            message: value[..end].to_owned(),
        }
    }
    fn host(&self) -> bool {
        matches!(self.phase, "stage" | "commit" | "cleanup" | "receipt")
    }
}
pub(super) struct Receipt<'a> {
    pub(super) request: &'a Request<'a>,
    pub(super) actual: Option<String>,
    pub(super) count: Option<usize>,
    pub(super) committed: bool,
    pub(super) verified_file: Option<File>,
    pub(super) staging: Option<PathBuf>,
    pub(super) failures: Vec<Failure>,
}
fn native(path: &OsStr) -> Value {
    json!({"label": label(path), "native_bytes": path.as_encoded_bytes()})
}
pub(super) fn label(path: &OsStr) -> String {
    let mut label = String::new();
    for character in path.to_string_lossy().chars() {
        if character.is_control() {
            label.extend(character.escape_default());
        } else {
            label.push(character);
        }
    }
    label
}
impl Receipt<'_> {
    pub(super) fn json(&self) -> String {
        json!({
            "schema": "esdieff.acquisition", "version": "1.0",
            "source": {"repository": self.request.repository, "commit": self.request.commit,
                "path": self.request.path, "uri": self.request.uri, "expected_sha256": self.request.expected},
            "result": {"status": if self.failures.is_empty() { "acquired" } else { "failed" },
                "complete": self.failures.is_empty(), "document_committed": self.committed,
                "actual_sha256": self.actual, "byte_count": self.count,
                "failures": self.failures.iter().map(|failure| json!({"phase": failure.phase, "message": failure.message})).collect::<Vec<_>>()},
            "document_output": native(self.request.document),
            "staging_output": self.staging.as_ref().map(|path| native(path.as_os_str()))
        }).to_string()
    }
    /// Styling that cannot fit within `limit` gives way to the identical plain receipt.
    pub(super) fn human_within(&self, style: Style, limit: usize) -> String {
        let styled = self.human(style);
        if style.enabled() && styled.len() > limit {
            self.human(Style::PLAIN)
        } else {
            styled
        }
    }
    /// Roles follow the recorded outcome and failure phases, never the reported values.
    fn human(&self, style: Style) -> String {
        use std::fmt::Write;
        let field = |output: &mut String, label: &str, value: &str| {
            writeln!(output, "{} {value}", style.span(Role::Heading, label)).expect("String write");
        };
        let complete = self.failures.is_empty();
        let mut output = format!(
            "{} {}\n",
            style.span(Role::Heading, "Acquisition"),
            if complete {
                style.span(Role::Passed, "acquired")
            } else {
                style.span(Role::Failure, "failed")
            }
        );
        field(&mut output, "Repository", self.request.repository);
        field(&mut output, "Commit", self.request.commit);
        field(&mut output, "Path", self.request.path);
        field(&mut output, "URI", &self.request.uri);
        field(&mut output, "Expected SHA-256", self.request.expected);
        field(&mut output, "Document", &label(self.request.document));
        let flag = |role: Option<Role>, value: bool| {
            let text = if value { "true" } else { "false" };
            role.map_or(Cow::Borrowed(text), |role| style.span(role, text))
        };
        let unfinished = (!complete).then_some(Role::Incomplete);
        field(&mut output, "Complete", &flag(unfinished, complete));
        // A document committed before a later failure stays available and needs notice.
        let notice = (self.committed && !complete).then_some(Role::Notice);
        field(
            &mut output,
            "Document committed",
            &flag(notice, self.committed),
        );
        if let Some(actual) = &self.actual {
            field(&mut output, "Actual SHA-256", actual);
        }
        if let Some(count) = self.count {
            field(&mut output, "Bytes", &count.to_string());
        }
        for failure in &self.failures {
            let phase = format!("{}:", failure.phase);
            writeln!(
                &mut output,
                "{} {}",
                style.span(Role::Failure, &phase),
                failure.message
            )
            .expect("String write");
        }
        if let Some(path) = &self.staging {
            field(&mut output, "Retained staging", &label(path.as_os_str()));
        }
        output
    }
    pub(super) fn code(&self) -> u8 {
        if self.failures.iter().any(Failure::host) {
            4
        } else if self.failures.is_empty() {
            0
        } else {
            1
        }
    }
}
