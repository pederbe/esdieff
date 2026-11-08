//! Terminal styling only adds colors and status symbols, and in human reports also blank
//! lines and middle-dot list separators. Each test crate uses part of this module.
#![allow(dead_code)]

pub const HEADING: &str = "\x1b[1;38;2;189;147;249m";
pub const SYNTAX: &str = "\x1b[38;2;92;184;255m";
pub const NOTICE: &str = "\x1b[1;38;2;92;184;255m";
pub const PASSED: &str = "\x1b[1;38;2;90;247;142m";
pub const ADVISORY: &str = "\x1b[1;38;2;243;156;18m";
pub const FAILED: &str = "\x1b[1;38;2;255;92;87m";
pub const RESET: &str = "\x1b[0m";

/// Text without its style sequences and the status symbol a styled span can start with.
pub fn plain(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace("\r\n", "\n");
    let mut plain = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(start) = rest.find("\x1b[") {
        plain.push_str(&rest[..start]);
        let end = start + rest[start..].find('m').expect("SGR sequence") + 1;
        let sequence = &rest[start..end];
        rest = &rest[end..];
        if sequence != RESET
            && let Some(mark) = ["✔ ", "✗ ", "⚠ ", "▶ "]
                .into_iter()
                .find(|mark| rest.starts_with(mark))
        {
            rest = &rest[mark.len()..];
        }
    }
    plain.push_str(rest);
    plain
}

/// A human report's lines without styling, blank lines or the choice of list separator.
pub fn readable(bytes: &[u8]) -> String {
    plain(bytes)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.replace(" · ", "; ").replace(", ", "; "))
        .collect::<Vec<_>>()
        .join("\n")
}
