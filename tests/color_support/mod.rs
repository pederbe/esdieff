//! Terminal styling only adds colors and status symbols, and in human reports also blank
//! lines and middle-dot list separators. Each test crate uses part of this module.
#![allow(dead_code)]

const RESET: &str = "\x1b[0m";

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

/// Asserts that `styled` presents `ordinary` with nothing changed except styling, status
/// symbols, added blank lines and middle dots where the plain report separates a list with
/// `; ` or `, `. Output without style sequences must equal the plain report exactly.
pub fn assert_presents(styled: &[u8], ordinary: &[u8]) {
    let ordinary = plain(ordinary);
    if !styled.contains(&0x1b) {
        assert_eq!(plain(styled), ordinary);
        return;
    }
    let styled = plain(styled);
    let (mut rest, mut expected) = (styled.as_str(), ordinary.as_str());
    let mut line_start = true;
    while !rest.is_empty() || !expected.is_empty() {
        if let Some(after) = rest.strip_prefix(" · ")
            && let Some(plain_after) = expected
                .strip_prefix("; ")
                .or_else(|| expected.strip_prefix(", "))
        {
            (rest, expected) = (after, plain_after);
            line_start = false;
        } else if let Some(character) = rest.chars().next()
            && expected.starts_with(character)
        {
            rest = &rest[character.len_utf8()..];
            expected = &expected[character.len_utf8()..];
            line_start = character == '\n';
        } else if line_start && rest.starts_with('\n') {
            rest = &rest[1..];
        } else {
            let excerpt = |text: &str| text.chars().take(80).collect::<String>();
            panic!(
                "styled output differs from the plain report\n  styled: {:?}\n  plain:  {:?}",
                excerpt(rest),
                excerpt(expected)
            );
        }
    }
}
