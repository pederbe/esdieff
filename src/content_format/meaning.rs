//! Bounded declaration meaning follows ADR 0033: Content-format model meaning.
//! Input is already wholly classified by the RFC 9193 grammar.

pub(super) fn fixed_headers() -> usize {
    use std::mem::size_of;
    let media = size_of::<(&str, &str, bool, Option<(&str, &str)>)>();
    let parameter = size_of::<([&str; 3], Option<(&str, &str)>, &[u8], usize)>();
    let comparison = size_of::<(&str, &[u8], std::str::Bytes<'_>, Option<u8>, &u8)>();
    media + parameter.max(comparison)
}

pub(super) fn media(name: &str, parameters: &str) -> bool {
    established(name, parameters).is_some()
}

/// The parameter meaning that the supported media rules establish.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Parameters {
    /// No parameter with an effect: parameterless JSON or CBOR, or JSON with its charset.
    None,
    /// Plain text in US-ASCII, its default, or in UTF-8.
    Text { utf8: bool },
}

/// The parameter meaning of a media name with these parameters, or `None` outside the
/// supported rules.
pub(super) fn established(name: &str, parameters: &str) -> Option<Parameters> {
    if name.eq_ignore_ascii_case("application/cbor") {
        return parameters.is_empty().then_some(Parameters::None);
    }
    let json = name.eq_ignore_ascii_case("application/json");
    if !json && !name.eq_ignore_ascii_case("text/plain") {
        return None;
    }
    // JSON has no required parameter, and plain text defaults to US-ASCII.
    let charset = if parameters.is_empty() {
        None
    } else {
        let (name, value) = parameter(parameters)?;
        if !name.eq_ignore_ascii_case("charset") {
            return None;
        }
        Some(value)
    };
    if json {
        // RFC 8259 defines no charset parameter and says it has no effect.
        return Some(Parameters::None);
    }
    match charset {
        None => Some(Parameters::Text { utf8: false }),
        Some(value) if value_equals(value, b"us-ascii") => Some(Parameters::Text { utf8: false }),
        Some(value) if value_equals(value, b"utf-8") => Some(Parameters::Text { utf8: true }),
        Some(_) => None,
    }
}

/// Borrow exactly one parameter. Further occurrences, including duplicates,
/// remain unsupported rather than choosing a first or last value.
fn parameter(text: &str) -> Option<(&str, &str)> {
    let text = text
        .trim_start_matches(' ')
        .strip_prefix(';')?
        .trim_start_matches(' ');
    let (name, rest) = text.split_once('=')?;
    let end = if rest.starts_with('"') {
        let bytes = rest.as_bytes();
        let mut index = 1;
        loop {
            match bytes.get(index)? {
                b'\\' => index += 2,
                b'"' => break index + 1,
                _ => index += 1,
            }
        }
    } else {
        rest.find([';', ' ']).unwrap_or(rest.len())
    };
    rest[end..].is_empty().then_some((name, &rest[..end]))
}

fn value_equals(text: &str, expected: &[u8]) -> bool {
    let text = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(text);
    let mut bytes = text.bytes();
    for expected in expected {
        let actual = match bytes.next() {
            Some(b'\\') => bytes.next(),
            byte => byte,
        };
        if actual.map(|byte| byte.to_ascii_lowercase()) != Some(*expected) {
            return false;
        }
    }
    bytes.next().is_none()
}

/// Numeric identifiers designate their complete recorded tuple, including
/// registered parameters and a coding, without an independent media query.
pub(super) fn numeric(row: super::table::Numeric) -> bool {
    !row.content_type.is_empty() && row.coding.is_none_or(|coding| !coding.is_empty())
}
