//! Applicable scalar contexts, the temporal and resource formats they name, and the
//! recorded leap-second knowledge edition that temporal findings cite.
//! Follows ADR 0018: Civil date and time sample conventions, and
//! ADR 0019: URI, URI-reference and UUID sample conventions.

/// The recorded leap-second knowledge edition that temporal findings cite.
pub(crate) const KNOWLEDGE_ID: &str = "iers-bulletin-c72/2026-07-06";
pub(crate) const TRANSITION_SHA256: &str =
    "6cb6f5d4b819f2e568e25db4b0b26d89dedf031fdffb18bc94d40f4e94e268d7";
pub(crate) const BULLETIN_SHA256: &str =
    "310e172eadacacca3adf92cb9bd646fcc0d6320e35e996a106d2644ede3b52f0";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TemporalFormat {
    Date,
    Time,
    DateTime,
}
impl TemporalFormat {
    pub(crate) fn from_text(text: &str) -> Option<Self> {
        match text {
            "date" => Some(Self::Date),
            "time" => Some(Self::Time),
            "date-time" => Some(Self::DateTime),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceFormat {
    Uri,
    Reference,
    Uuid,
}
impl ResourceFormat {
    pub(crate) fn from_text(text: &str) -> Option<Self> {
        match text {
            "uri" => Some(Self::Uri),
            "uri-reference" => Some(Self::Reference),
            "uuid" => Some(Self::Uuid),
            _ => None,
        }
    }
}
/// Applicable scalar context only; a context does not establish a denotation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Context {
    Ordinary,
    Temporal(TemporalFormat),
    Resource(ResourceFormat),
    Bytes,
    Unix,
    Unavailable,
}
