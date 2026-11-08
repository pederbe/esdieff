//! The terminal palette shared by human reports and the command's help and diagnostics.
//! The library and the command each include this file and use part of it.
#![allow(dead_code)]

/// Bold purple titles, headings and labels.
pub(crate) const HEADING: &str = "\x1b[1;38;2;189;147;249m";
/// Blue command syntax.
pub(crate) const SYNTAX: &str = "\x1b[38;2;92;184;255m";
/// Bold blue notices.
pub(crate) const NOTICE: &str = "\x1b[1;38;2;92;184;255m";
/// Bold green passed outcomes.
pub(crate) const PASSED: &str = "\x1b[1;38;2;90;247;142m";
/// Bold amber warnings and unfinished work.
pub(crate) const ADVISORY: &str = "\x1b[1;38;2;243;156;18m";
/// Bold red failures.
pub(crate) const FAILED: &str = "\x1b[1;38;2;255;92;87m";
pub(crate) const RESET: &str = "\x1b[0m";

pub(crate) const PASSED_MARK: &str = "✔";
pub(crate) const FAILED_MARK: &str = "✗";
pub(crate) const ADVISORY_MARK: &str = "⚠";
pub(crate) const NOTICE_MARK: &str = "▶";
/// Replaces a plain list separator in styled output.
pub(crate) const SEPARATOR: &str = " · ";
