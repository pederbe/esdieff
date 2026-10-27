//! RFC 3986 Appendix A syntax without normalization, scheme policy or acquisition.
//! Follows ADR0002: Recorded standards baseline, and
//! ADR0009: Bounded processing and reporting.
//! The caller must charge a finite multiple of input bytes before scanning.

// Resource-only caller/callee staging. Borrowed search, component and literal
// phases end before the caller extracts resource meaning or constructs a finding.
pub(crate) fn resource_validation_fixed_headers() -> usize {
    use std::mem::size_of;
    // CharSearcher has haystack, two fingers, char and five encoding bytes;
    // its rounded containing width is covered by this larger public tuple.
    let search = size_of::<(&str, usize, usize, char, [u8; 5])>();
    let generic = 6 * size_of::<&str>()
        + 2 * size_of::<Option<&str>>()
        + size_of::<Option<(&str, &str)>>()
        + size_of::<std::str::CharIndices<'_>>()
        + search
        + size_of::<std::str::Bytes<'_>>()
        + 2 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + size_of::<Option<&u8>>()
        + 3 * size_of::<bool>()
        + 2 * size_of::<u8>()
        + 2 * size_of::<&str>(); // active forwarding/search receiver views
    let literal = 6 * size_of::<&str>() // enclosing grammar views, conservatively
        + size_of::<&[u8]>() + 2 * size_of::<usize>()
        + 2 * size_of::<u8>() + size_of::<Option<usize>>()
        + size_of::<Option<&u8>>()
        + size_of::<&[u8]>() + 2 * size_of::<usize>()
        + size_of::<u8>() + size_of::<u16>()
        + size_of::<&u8>() + size_of::<Option<&u8>>()
        + size_of::<Option<usize>>();
    generic.max(literal)
}

pub(crate) fn valid_uri(text: &str) -> bool {
    valid_uri_with(text, std_ipv6_literal)
}
pub(crate) fn valid_resource_uri(text: &str) -> bool {
    valid_uri_with(text, ipv6_literal_bounded)
}
fn std_ipv6_literal(text: &str) -> bool {
    text.parse::<std::net::Ipv6Addr>().is_ok()
}
fn valid_uri_with(text: &str, ipv6: impl Fn(&str) -> bool + Copy) -> bool {
    let Some((scheme, remainder)) = text.split_once(':') else {
        return false;
    };
    if !scheme
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        || !scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return false;
    }
    let (head, fragment) = remainder
        .split_once('#')
        .map_or((remainder, None), |(a, b)| (a, Some(b)));
    let (hier, query) = head
        .split_once('?')
        .map_or((head, None), |(a, b)| (a, Some(b)));
    if !fragment.is_none_or(|s| component(s, query_char))
        || !query.is_none_or(|s| component(s, query_char))
    {
        return false;
    }
    if let Some(authority_path) = hier.strip_prefix("//") {
        let (authority, path) = authority_path
            .find('/')
            .map_or((authority_path, ""), |i| authority_path.split_at(i));
        valid_authority(authority, ipv6) && component(path, path_char)
    } else {
        component(hier, path_char)
    }
}
/// RFC 3986 relative-ref, without resolving it against a base.
pub(crate) fn valid_relative_reference(text: &str) -> bool {
    valid_relative_reference_with(text, std_ipv6_literal)
}
pub(crate) fn valid_resource_reference(text: &str) -> bool {
    valid_relative_reference_with(text, ipv6_literal_bounded)
}
fn valid_relative_reference_with(text: &str, ipv6: impl Fn(&str) -> bool + Copy) -> bool {
    let (head, fragment) = text
        .split_once('#')
        .map_or((text, None), |(a, b)| (a, Some(b)));
    let (path, query) = head
        .split_once('?')
        .map_or((head, None), |(a, b)| (a, Some(b)));
    if !fragment.is_none_or(|s| component(s, query_char))
        || !query.is_none_or(|s| component(s, query_char))
    {
        return false;
    }
    if let Some(rest) = path.strip_prefix("//") {
        let (authority, path) = rest.find('/').map_or((rest, ""), |i| rest.split_at(i));
        valid_authority(authority, ipv6) && component(path, path_char)
    } else {
        (path.starts_with('/') || !path.split('/').next().unwrap_or("").contains(':'))
            && component(path, path_char)
    }
}
fn valid_authority(text: &str, ipv6: impl Fn(&str) -> bool) -> bool {
    let host_port = if let Some((userinfo, host_port)) = text.split_once('@') {
        if !component(userinfo, |b| basic(b) || b == b':') {
            return false;
        }
        host_port
    } else {
        text
    };
    if let Some(literal_port) = host_port.strip_prefix('[') {
        let Some((literal, port)) = literal_port.split_once(']') else {
            return false;
        };
        let valid_literal = if literal.starts_with(['v', 'V']) {
            literal[1..]
                .split_once('.')
                .is_some_and(|(version, address)| {
                    !version.is_empty()
                        && version.bytes().all(|b| b.is_ascii_hexdigit())
                        && !address.is_empty()
                        && address.bytes().all(|b| basic(b) || b == b':')
                })
        } else {
            ipv6(literal)
        };
        valid_literal
            && (port.is_empty()
                || port
                    .strip_prefix(':')
                    .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit())))
    } else {
        let (host, port) = host_port
            .split_once(':')
            .map_or((host_port, None), |(a, b)| (a, Some(b)));
        component(host, basic) && port.is_none_or(|s| s.bytes().all(|b| b.is_ascii_digit()))
    }
}
fn component(text: &str, admitted: impl Fn(u8) -> bool) -> bool {
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            if !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
                || !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
            {
                return false;
            }
        } else if !admitted(byte) {
            return false;
        }
    }
    true
}
fn basic(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=".contains(&byte)
}
fn path_char(byte: u8) -> bool {
    basic(byte) || matches!(byte, b':' | b'@' | b'/')
}
fn query_char(byte: u8) -> bool {
    path_char(byte) || byte == b'?'
}
fn ipv6_literal_bounded(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut cursor = 0_usize;
    let mut words = 0_u8;
    let mut compressed = false;
    if bytes[0] == b':' {
        if bytes.get(1) != Some(&b':') {
            return false;
        }
        compressed = true;
        cursor = 2;
        if cursor == bytes.len() {
            return true;
        }
    }
    loop {
        let start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_hexdigit) {
            let Some(next) = cursor.checked_add(1) else {
                return false;
            };
            cursor = next;
            if cursor - start > 4 {
                return false;
            }
        }
        if bytes.get(cursor) == Some(&b'.') {
            return words <= 6
                && ipv4_tail_bounded(&bytes[start..])
                && if compressed {
                    words + 2 < 8
                } else {
                    words + 2 == 8
                };
        }
        if cursor == start || words >= 8 {
            return false;
        }
        words += 1;
        if cursor == bytes.len() {
            return if compressed { words < 8 } else { words == 8 };
        }
        if bytes[cursor] != b':' {
            return false;
        }
        let Some(next) = cursor.checked_add(1) else {
            return false;
        };
        cursor = next;
        if bytes.get(cursor) == Some(&b':') {
            if compressed {
                return false;
            }
            compressed = true;
            let Some(next) = cursor.checked_add(1) else {
                return false;
            };
            cursor = next;
            if cursor == bytes.len() {
                return words < 8;
            }
        } else if cursor == bytes.len() {
            return false;
        }
    }
}
fn ipv4_tail_bounded(bytes: &[u8]) -> bool {
    let mut cursor = 0_usize;
    for index in 0_u8..4 {
        let start = cursor;
        let mut octet = 0_u16;
        while let Some(byte) = bytes.get(cursor).filter(|byte| byte.is_ascii_digit()) {
            // The preceding iteration rejected values above255. The next
            // multiply/add is at most2559 and is representable by u16.
            octet = octet * 10 + u16::from(*byte - b'0');
            let Some(next) = cursor.checked_add(1) else {
                return false;
            };
            cursor = next;
            if cursor - start > 3 || octet > 255 {
                return false;
            }
        }
        if cursor == start || cursor - start > 1 && bytes[start] == b'0' {
            return false;
        }
        if index < 3 {
            if bytes.get(cursor) != Some(&b'.') {
                return false;
            }
            let Some(next) = cursor.checked_add(1) else {
                return false;
            };
            cursor = next;
        }
    }
    cursor == bytes.len()
}

#[cfg(test)]
mod resource_seam_tests {
    use super::*;
    fn compare_literal(literal: &str) {
        let uri = format!("https://[{literal}]:443/device?q#r");
        let reference = format!("//[{literal}]:443/device?q#r");
        let expected = literal.parse::<std::net::Ipv6Addr>().is_ok();
        assert_eq!(valid_resource_uri(&uri), expected, "{literal}");
        assert_eq!(valid_resource_reference(&reference), expected, "{literal}");
        assert_eq!(valid_uri(&uri), expected, "old URI {literal}");
        assert_eq!(
            valid_relative_reference(&reference),
            expected,
            "old reference {literal}"
        );
    }
    #[test]
    fn bounded_literal_matches_std_through_actual_uri_and_reference_seams() {
        for literal in [
            "::",
            "::1",
            "2001:db8::1",
            "1:2:3:4:5:6:7:8",
            "::ffff:192.0.2.1",
            "1:2:3:4:5:6:192.0.2.1",
            "1:2:3:4:5::192.0.2.1",
            "",
            "1:2:3:4:5:6:7:8:9",
            "1:2:3:4:5:6:7:8::",
            "1::2::3",
            ":::1",
            "::ffff:192.00.2.1",
            "::ffff:256.0.0.1",
            "::ffff:1.2.3",
            "::ffff:1.2.3.4.5",
            "::ffff:1.2.3.-1",
            "::1%eth0",
            "::é",
            "::00000",
        ] {
            compare_literal(literal);
        }
        for count in 0..=9 {
            let full = vec!["aBcD"; count].join(":");
            compare_literal(&full);
            for left in 0..=count {
                let compressed = format!(
                    "{}::{}",
                    vec!["1"; left].join(":"),
                    vec!["2"; count - left].join(":")
                );
                compare_literal(&compressed);
            }
        }
        for octet in [
            "0", "1", "9", "10", "99", "100", "255", "256", "999", "1000", "01", "00", "-1", "é",
            "",
        ] {
            for position in 0..4 {
                let mut words = ["1", "2", "3", "4"];
                words[position] = octet;
                compare_literal(&format!("::ffff:{}", words.join(".")));
            }
        }
        let full = "2001:db8:abcd:1:2:3:4:5";
        for index in 0..=full.len() {
            compare_literal(&full[..index]);
            compare_literal(&format!("{}:{}", &full[..index], &full[index..]));
        }
        compare_literal(&"f".repeat(8192));
        for text in [
            "https://[v1.a:b]/",
            "//[vF.a:b]",
            "https://[v1.]/",
            "https://[v.a]/",
            "https://[::1]x/",
            "https://[::1]:port/",
            "https://a/%00?q=%FF#r",
            "https://u@a@b/",
        ] {
            assert_eq!(valid_resource_uri(text), valid_uri(text));
            assert_eq!(
                valid_resource_reference(text),
                valid_relative_reference(text)
            );
        }
    }
}
