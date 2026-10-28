//! The Markdown changelog of a Revision comparison lists its version warnings and groups its
//! retained differences by classification, following ADR 0043: SARIF and Markdown report
//! projections, ADR 0044: Versions of breaking revisions and ADR 0009: Bounded processing and
//! reporting. Model text such as Given Names appears only in code spans, so it cannot inject
//! Markdown.
use super::{Classification, Difference, RevisionComparison, wording};

/// Sections in order, each listing the unacknowledged differences of one classification, or
/// every acknowledged difference.
const SECTIONS: [(&str, Option<Classification>); 4] = [
    ("Breaking differences", Some(Classification::Breaking)),
    (
        "Undetermined differences",
        Some(Classification::Undetermined),
    ),
    ("Compatible differences", Some(Classification::Compatible)),
    ("Acknowledged differences", None),
];

const MARKDOWN: wording::Style = wording::Style { code, text: escape };

pub(super) fn render(comparison: &RevisionComparison<'_>) -> String {
    let count = |classification| comparison.count(classification);
    let mut text = format!(
        "# Model changes\n\nDisposition: {}, with {} breaking, {} undetermined and {} compatible differences.",
        comparison.disposition().as_str(),
        count(Classification::Breaking),
        count(Classification::Undetermined),
        count(Classification::Compatible),
    );
    if comparison
        .acknowledgements
        .is_some_and(|supplied| !supplied.entries().is_empty())
    {
        text.push_str(&format!(
            " {} breaking and {} undetermined differences are acknowledged.",
            comparison.acknowledged_count(Classification::Breaking),
            comparison.acknowledged_count(Classification::Undetermined),
        ));
    }
    text.push_str("\n\n");
    text.push_str(super::SCOPE_NOTICE);
    text.push('\n');

    // Keep sections and items in order while they fit with the closing line's reserve. Version
    // warnings come first, at most one per compared document pair.
    let total = comparison.total();
    let warnings = comparison.version_warnings().len() as u64;
    let limit = comparison.catalogues[0].limits.report_bytes;
    let mut remaining = limit.saturating_sub(text.len() + closing(u64::MAX, u64::MAX).len());
    let mut listed = 0;
    let mut warned = 0;
    let mut keep = |text: &mut String, section: &mut String, line: &str| {
        let cost = line.len() + section.len();
        if cost > remaining {
            return false;
        }
        remaining -= cost;
        text.push_str(section);
        section.clear();
        text.push_str(line);
        true
    };
    'sections: {
        let mut section = String::from("\n## Warnings\n\n");
        for warning in comparison.version_warnings() {
            let words = wording::describe_warning(comparison, warning, &MARKDOWN);
            if !keep(
                &mut text,
                &mut section,
                &format!("- {}\n", capitalized(&words)),
            ) {
                break 'sections;
            }
            warned += 1;
        }
        for (heading, classification) in SECTIONS {
            let mut section = format!("\n## {heading}\n\n");
            let members = comparison.differences().filter(|difference| {
                difference.acknowledged() == classification.is_none()
                    && classification
                        .is_none_or(|classification| difference.classification() == classification)
            });
            for difference in members {
                let line = item(comparison, &difference, classification.is_none());
                if !keep(&mut text, &mut section, &line) {
                    break 'sections;
                }
                listed += 1;
            }
        }
    }
    if total > listed || warnings > warned {
        text.push_str(&closing(total - listed, warnings - warned));
    }
    debug_assert!(text.len() <= limit, "changelog within the report limit");
    text
}

/// The closing line that counts differences and warnings the changelog does not list.
fn closing(differences: u64, warnings: u64) -> String {
    let unlisted = match warnings {
        0 => format!("{differences} further differences are"),
        1 => format!("{differences} further differences and 1 warning are"),
        _ => format!("{differences} further differences and {warnings} warnings are"),
    };
    format!("\n{unlisted} not listed, because their details did not fit the report limit.\n")
}

/// One difference as a list item. An acknowledged item starts with its classification.
fn item(
    comparison: &RevisionComparison<'_>,
    difference: &Difference<'_>,
    acknowledged: bool,
) -> String {
    let words = wording::describe(comparison, difference, &MARKDOWN);
    if acknowledged {
        format!(
            "- {}: {words}\n",
            capitalized(difference.classification().as_str())
        )
    } else {
        format!("- {}\n", capitalized(&words))
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A code span that shows `text` literally. Line breaks and other control characters become
/// `\uXXXX` escapes, so the span stays on its line, and the fence is longer than any run of
/// backticks inside it.
fn code(text: &str) -> String {
    let mut content = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
            content.push_str(&format!("\\u{:04X}", ch as u32));
        } else {
            content.push(ch);
        }
    }
    let mut longest = 0;
    let mut run = 0;
    for ch in content.chars() {
        run = if ch == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest + 1);
    // A space on each side keeps a leading or trailing backtick or space intact.
    let pad = if content.starts_with(['`', ' ']) || content.ends_with(['`', ' ']) {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{content}{pad}{fence}")
}

/// Plain text with Markdown punctuation escaped and line breaks as spaces.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '&' | '~' | '|' | '#' | '!' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            ch if ch.is_control() => escaped.push(' '),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_spans_keep_backticks_and_edge_spaces_literal() {
        assert_eq!(code("a"), "`a`");
        assert_eq!(code("a``b"), "```a``b```");
        assert_eq!(code("`a"), "`` `a ``");
        assert_eq!(code(" a "), "`  a  `");
        assert_eq!(code("a\r\nb\u{2028}"), "`a\\u000D\\u000Ab\\u2028`");
    }

    #[test]
    fn text_escapes_markdown_punctuation() {
        assert_eq!(escape("*a* <b> [c](d)\n"), "\\*a\\* \\<b\\> \\[c\\](d) ");
    }
}
