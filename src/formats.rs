//! Resource representations follow ADR 0019: URI, URI-reference and UUID sample conventions.
use crate::Field;
use crate::scalar::{Context, ResourceFormat as Format};

pub(crate) const URI_USED: u8 = 1;
pub(crate) const REFERENCE_USED: u8 = 2;
pub(crate) const HTTP_USED: u8 = 4;
pub(crate) const HTTPS_USED: u8 = 8;
pub(crate) const UUID_TEXT_USED: u8 = 16;
pub(crate) const UUID_VALUE_USED: u8 = 32;

struct Record {
    id: &'static str,
    rfc: &'static str,
    sections: &'static str,
    scope: &'static str,
}

const RECORDS: [Record; 6] = [
    Record {
        id: "rfc3986-uri-syntax/1",
        rfc: "3986",
        sections: "3, 4.1",
        scope: "Generic URI representation; scheme-specific resource meaning is separate.",
    },
    Record {
        id: "rfc3986-reference-expression/1",
        rfc: "3986",
        sections: "4.1, 4.2",
        scope: "Relative reference expression; a target requires an explicit applicable base.",
    },
    Record {
        id: "rfc9110-http-identity/1",
        rfc: "9110",
        sections: "4.2.1",
        scope: "HTTP resource identifier with authority and nonempty host; no retrieval or normalization.",
    },
    Record {
        id: "rfc9110-https-identity/1",
        rfc: "9110",
        sections: "4.2.2",
        scope: "HTTPS resource identifier with authority and nonempty host; no retrieval or normalization.",
    },
    Record {
        id: "rfc9562-bare-text/1",
        rfc: "9562",
        sections: "4",
        scope: "Selected bare 8-4-4-4-12 hexadecimal JSON string representation, accepting either letter case.",
    },
    Record {
        id: "uuid-opaque-value/1",
        rfc: "9562",
        sections: "4, 6.12",
        scope: "Opaque 128-bit value equality, including Nil, Max and reserved layouts; no layout inference.",
    },
];

/// Immutable specification facts actually consulted by a selected sample request.
#[derive(Clone, Copy)]
pub struct FormatKnowledgeView(&'static Record);
impl FormatKnowledgeView {
    pub fn id(self) -> &'static str {
        self.0.id
    }
    pub fn rfc(self) -> &'static str {
        self.0.rfc
    }
    pub fn sections(self) -> &'static str {
        self.0.sections
    }
    pub fn scope(self) -> &'static str {
        self.0.scope
    }
}

pub(crate) fn knowledge(id: &str, usage: Option<u8>) -> Field<FormatKnowledgeView> {
    let Some(index) = RECORDS.iter().position(|record| record.id == id) else {
        return Field::Absent;
    };
    match usage {
        None => Field::Unavailable("initial format consultation owner unavailable"),
        Some(mask) if mask & (1 << index) != 0 => {
            Field::Present(FormatKnowledgeView(&RECORDS[index]))
        }
        Some(_) => Field::Absent,
    }
}
pub(crate) fn wire(usage: Option<u8>) -> serde_json::Value {
    let Some(mask) = usage else {
        return serde_json::Value::Null;
    };
    let mut records = Vec::new();
    for (index, record) in RECORDS.iter().enumerate() {
        if mask & (1 << index) != 0 {
            records.push(serde_json::json!({"id":record.id,"rfc":record.rfc,"sections":record.sections,"scope":record.scope}));
        }
    }
    serde_json::Value::Array(records)
}

pub(crate) enum Denotation<'a> {
    Identity(&'a str),
    Uuid([u8; 16]),
}
pub(crate) enum Evaluation<'a> {
    Established(Denotation<'a>),
    Syntax,
    Http,
    UuidText,
    MeaningUnavailable,
    WorkUnavailable,
}

// Complete exclusive scalar dispatch. Shared sample/equality frames remain in
// their existing containing-owner declaration, outside this maximum. Retained
// metadata and consultation records have separate, simultaneous report owners.
pub(crate) fn fixed_headers() -> usize {
    use std::mem::size_of;
    let selected = size_of::<Context>()
        + size_of::<Field<crate::EffectiveJson<'_>>>()
        + 2 * size_of::<Option<&str>>()
        + 8 * size_of::<bool>()
        + size_of::<crate::JsonKind>();
    let admission = selected
        + size_of::<Option<usize>>()
        + size_of::<Format>()
        + size_of::<&str>()
        + size_of::<&mut u8>();
    // During the right scan the first Option survives; the second Evaluation
    // appears only after the scanner returns. Both Options and moved pattern
    // payloads coexist later, without the active parser's storage.
    let pair_scan = 2 * size_of::<Context>()
        + 3 * size_of::<bool>()
        + size_of::<Option<Denotation<'_>>>()
        + size_of::<Format>()
        + 2 * size_of::<&str>()
        + size_of::<&mut usize>()
        + size_of::<&mut u8>()
        + size_of::<u16>();
    let pair_return = pair_scan + size_of::<Evaluation<'_>>();
    let post_scan = pair_scan
        + size_of::<Option<Denotation<'_>>>()
        + 2 * size_of::<[u8; 16]>()
        + size_of::<(crate::finding::Verdict, u16)>();
    let uri = admission.max(pair_scan) + crate::uri::resource_validation_fixed_headers();
    let uuid = admission.max(pair_scan)
        + size_of::<[u8; 16]>()
        + size_of::<&[u8]>()
        + size_of::<std::iter::Enumerate<std::slice::IterMut<'_, u8>>>()
        + 2 * size_of::<usize>()
        + size_of::<&mut u8>()
        + 2 * size_of::<u8>()
        + size_of::<Option<u8>>();
    let extract = admission.max(pair_scan)
        + 4 * size_of::<&str>()
        + size_of::<Option<(&str, &str)>>()
        + size_of::<Option<usize>>()
        + size_of::<bool>();
    // The parser payload is consumed before constructing a finding. The actual
    // finding temporary is paid here while the selected context survives.
    let diagnostic = selected
        + size_of::<Option<usize>>()
        + size_of::<(crate::finding::Basis, &str, &str)>()
        + size_of::<crate::finding::Finding>();
    uri.max(uuid)
        .max(extract)
        .max(diagnostic)
        .max(pair_return)
        .max(post_scan)
}

pub(crate) fn scan_charge(format: Format, bytes: usize) -> Option<usize> {
    // URI syntax, reference classification and authority extraction together
    // visit at most 64 times each input byte. The additional fixed allowance
    // includes every bounded borrowed IPv6/embedded IPv4 parser attempt.
    // UUID visits at most 36 bytes once, with at most 16 fixed tests per byte.
    let (multiple, fixed) = if format == Format::Uuid {
        (16, 64)
    } else {
        (64, 8192)
    };
    bytes.checked_mul(multiple)?.checked_add(fixed)
}

pub(crate) fn evaluate<'a>(
    format: Format,
    text: &'a str,
    work: &mut usize,
    usage: &mut u8,
) -> Evaluation<'a> {
    if !scan_charge(format, text.len()).is_some_and(|cost| crate::allowance::charge(work, cost)) {
        return Evaluation::WorkUnavailable;
    }
    evaluate_charged(format, text, usage)
}

// Consumes the first returned record before entering the second scanner. The
// first successful payload is retained until both valid operands are compared;
// no decoded resource value is stored in an equality or sample frame.
pub(crate) fn compare_meaning(
    format: Format,
    left: &str,
    right: &str,
    work: &mut usize,
    usage: &mut u8,
) -> (crate::finding::Verdict, u16) {
    use crate::finding::Verdict;
    let mut limitations = 0;
    let first = match evaluate(format, left, work, usage) {
        Evaluation::Established(value) => Some(value),
        Evaluation::WorkUnavailable => {
            limitations |= 1 << 3;
            None
        }
        Evaluation::MeaningUnavailable => {
            limitations |= 1 << 8;
            None
        }
        Evaluation::Syntax | Evaluation::Http | Evaluation::UuidText => {
            limitations |= 1 << 2;
            None
        }
    };
    let second = match evaluate(format, right, work, usage) {
        Evaluation::Established(value) => Some(value),
        Evaluation::WorkUnavailable => {
            limitations |= 1 << 3;
            None
        }
        Evaluation::MeaningUnavailable => {
            limitations |= 1 << 8;
            None
        }
        Evaluation::Syntax | Evaluation::Http | Evaluation::UuidText => {
            limitations |= 1 << 2;
            None
        }
    };
    let verdict = match (first, second) {
        (Some(Denotation::Uuid(a)), Some(Denotation::Uuid(b))) => {
            if !crate::allowance::charge(work, 17) {
                limitations |= 1 << 3;
                Verdict::Unavailable
            } else if a == b {
                Verdict::Equal
            } else {
                Verdict::Different
            }
        }
        (Some(Denotation::Identity(a)), Some(Denotation::Identity(b))) => {
            if a.len()
                .checked_add(b.len())
                .and_then(|n| n.checked_add(1))
                .is_some_and(|cost| crate::allowance::charge(work, cost))
            {
                if a == b {
                    Verdict::Equal
                } else {
                    limitations |= 1 << 8;
                    Verdict::Unavailable
                }
            } else {
                limitations |= 1 << 3;
                Verdict::Unavailable
            }
        }
        _ => Verdict::Unavailable,
    };
    (verdict, limitations)
}

fn evaluate_charged<'a>(format: Format, text: &'a str, usage: &mut u8) -> Evaluation<'a> {
    if format == Format::Uuid {
        *usage |= UUID_TEXT_USED;
        let bytes = text.as_bytes();
        if bytes.len() != 36 {
            return Evaluation::UuidText;
        }
        if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
            return Evaluation::UuidText;
        }
        let mut value = [0; 16];
        for (index, octet) in value.iter_mut().enumerate() {
            let start = 2 * index
                + usize::from(index >= 4)
                + usize::from(index >= 6)
                + usize::from(index >= 8)
                + usize::from(index >= 10);
            let Some(high) = hex(bytes[start]) else {
                return Evaluation::UuidText;
            };
            let Some(low) = hex(bytes[start + 1]) else {
                return Evaluation::UuidText;
            };
            *octet = high << 4 | low;
        }
        *usage |= UUID_VALUE_USED;
        return Evaluation::Established(Denotation::Uuid(value));
    }
    // A colon before the first path/query/fragment delimiter selects a URI.
    // Relative input is never scanned as a failed URI first.
    let absolute = format == Format::Uri || {
        let colon = text.find(':');
        let delimiter = text.find(['/', '?', '#']).unwrap_or(text.len());
        colon.is_some_and(|index| index < delimiter)
    };
    if !absolute {
        *usage |= REFERENCE_USED;
        return if crate::uri::valid_resource_reference(text) {
            Evaluation::Established(Denotation::Identity(text))
        } else {
            Evaluation::Syntax
        };
    }
    *usage |= URI_USED;
    if !crate::uri::valid_resource_uri(text) {
        return Evaluation::Syntax;
    }
    let rest = {
        let (scheme, rest) = text.split_once(':').expect("validated URI scheme");
        if scheme.eq_ignore_ascii_case("http") {
            *usage |= HTTP_USED;
        } else if scheme.eq_ignore_ascii_case("https") {
            *usage |= HTTPS_USED;
        } else {
            return Evaluation::MeaningUnavailable;
        }
        rest
    };
    let Some(authority) = rest.strip_prefix("//") else {
        return Evaluation::Http;
    };
    let authority = &authority[..authority.find(['/', '?', '#']).unwrap_or(authority.len())];
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let nonempty = if host_port.starts_with('[') {
        // The preceding generic scan established a nonempty IP literal.
        true
    } else {
        !host_port
            .split_once(':')
            .map_or(host_port, |(host, _)| host)
            .is_empty()
    };
    if !nonempty {
        Evaluation::Http
    } else {
        Evaluation::Established(Denotation::Identity(text))
    }
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
