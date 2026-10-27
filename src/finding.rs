//! Shared finding vocabulary: the basis a producer cites, the finding it records
//! and the verdict a bounded comparison returns.
//! Follows ADR 0015: Bounded value equality in a resolved data context, and
//! ADR 0016: Selected samples with explicit admission and coverage.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Basis {
    Numeric,
    Text,
    Boolean,
    Nullable,
    TypeNull,
    Const,
    Alternatives,
    Map,
    Array,
    Temporal,
    TemporalRepresentation,
    TemporalKnowledge,
    TemporalMeaning,
    TemporalCalendar,
    TemporalClock,
    TemporalSlot,
    TemporalLeap,
    TemporalWork,
    Resource,
    AbstractMixed,
    UriSyntax,
    HttpIdentity,
    UuidText,
    UuidValue,
    ResourceMeaning,
    ResourceWork,
    Abstract,
    Byte,
    Unix,
    AbstractUnavailable,
    AbstractWork,
    AbstractNumberWork,
    AbstractEstablishment,
    PatternCapability,
    PatternWork,
}
impl Basis {
    pub(crate) const fn support_mask(self) -> u32 {
        let index = match self {
            Self::Numeric => 0,
            Self::Text => 1,
            Self::Boolean => 2,
            Self::Nullable => 3,
            Self::TypeNull => 4,
            Self::Const => 5,
            Self::Alternatives => 6,
            Self::Map => 7,
            Self::Array => 8,
            Self::Temporal => 9,
            Self::TemporalRepresentation => 10,
            Self::TemporalCalendar => 11,
            Self::TemporalClock => 12,
            Self::TemporalSlot => 13,
            Self::TemporalLeap => 14,
            Self::Resource => 15,
            Self::AbstractMixed => 16,
            Self::UriSyntax => 17,
            Self::HttpIdentity => 18,
            Self::UuidText => 19,
            Self::UuidValue => 20,
            Self::Abstract => 21,
            Self::Byte => 22,
            Self::Unix => 23,
            _ => return 0,
        };
        1 << index
    }
    pub(crate) fn citation(self) -> &'static str {
        match self {
            Self::Numeric => "RFC 9880 Appendix C.1",
            Self::Text => "RFC 9880 Appendix C.2",
            Self::Boolean => "RFC 9880 Appendix C.3",
            Self::Nullable => "RFC 9880 Section 4.7 Table 4",
            Self::TypeNull => "RFC 9880 Section 4.7 and Appendix C",
            Self::Const => "RFC 9880 Appendix C",
            Self::Alternatives => "RFC 9880 Section 4.7.2",
            Self::Map => "RFC 9880 Appendix C.5",
            Self::Array => "RFC 9880 Appendix C.4",
            Self::Temporal => {
                "RFC 9880 Appendix C.2; ADR 0018: Civil date and time sample conventions"
            }
            Self::TemporalRepresentation => "RFC 9880 Appendix C.2; RFC 3339 Section 5.6",
            Self::TemporalCalendar => "RFC 3339 Section 5.7 and Appendix C; RFC 9880 Appendix C.2",
            Self::TemporalClock => "RFC 3339 Sections 5.6 and 5.7; RFC 9880 Appendix C.2",
            Self::TemporalSlot | Self::TemporalLeap | Self::TemporalKnowledge => {
                "RFC 3339 Section 5.7; RFC 9880 Appendix C.2"
            }
            Self::TemporalMeaning => {
                "ADR 0018: Civil date and time sample conventions; RFC 9880 Appendix C.2; RFC 3339 Section 5.6"
            }
            Self::TemporalWork
            | Self::AbstractUnavailable
            | Self::AbstractWork
            | Self::AbstractNumberWork
            | Self::AbstractEstablishment => "",
            Self::PatternCapability | Self::PatternWork => "",
            Self::Resource => {
                "RFC 9880 Appendix C.2; RFC 3986 Sections 3 and 4.1; RFC 9110 Sections 4.2.1 and 4.2.2"
            }
            Self::AbstractMixed => {
                "RFC 9880 Appendix C; ADR 0018: Civil date and time sample conventions; ADR 0019: URI, URI-reference and UUID sample conventions"
            }
            Self::UriSyntax => "RFC 3986 Sections 3 and 4.1; RFC 9880 Appendix C.2",
            Self::HttpIdentity => "RFC 9110 Sections 4.2.1 and 4.2.2; RFC 9880 Appendix C.2",
            Self::UuidText | Self::UuidValue => {
                "RFC 9562 Sections 4 and 6.12; RFC 9880 Appendix C.2"
            }
            Self::ResourceMeaning => {
                "ADR 0019: URI, URI-reference and UUID sample conventions; RFC 9880 Appendix C.2"
            }
            Self::ResourceWork => "",
            Self::Abstract | Self::Byte | Self::Unix => "RFC 9880 Section 4.7.1 Table 5",
        }
    }
    pub(crate) fn incomplete(self) -> bool {
        matches!(
            self,
            Self::TemporalKnowledge
                | Self::TemporalMeaning
                | Self::TemporalWork
                | Self::ResourceMeaning
                | Self::ResourceWork
                | Self::AbstractUnavailable
                | Self::AbstractWork
                | Self::AbstractNumberWork
                | Self::AbstractEstablishment
                | Self::PatternCapability
                | Self::PatternWork
        )
    }
    pub(crate) fn kind(self) -> &'static str {
        match self {
            Self::TemporalRepresentation => "interpretation",
            Self::TemporalKnowledge | Self::TemporalMeaning | Self::TemporalWork => "operational",
            Self::UuidText | Self::UuidValue | Self::Byte | Self::Unix => "interpretation",
            Self::ResourceMeaning
            | Self::ResourceWork
            | Self::AbstractUnavailable
            | Self::AbstractWork
            | Self::AbstractNumberWork
            | Self::AbstractEstablishment => "operational",
            Self::PatternCapability | Self::PatternWork => "operational",
            _ => "rfc",
        }
    }
    pub(crate) fn id(self) -> Option<&'static str> {
        match self {
            Self::TemporalRepresentation => Some("ADR0018-selected-representation"),
            Self::TemporalKnowledge => Some("temporal-knowledge"),
            Self::TemporalMeaning | Self::TemporalWork => Some("sample-establishment"),
            Self::TemporalLeap => Some(crate::scalar::KNOWLEDGE_ID),
            Self::UuidText => Some("uuid-bare-json/1"),
            Self::UuidValue => Some("uuid-opaque-value/1"),
            Self::ResourceMeaning | Self::ResourceWork => Some("sample-establishment"),
            Self::Byte => Some("byte-string-json/1"),
            Self::Unix => Some("unix-time-posix-coordinate/1"),
            Self::AbstractUnavailable => Some("sample-abstract-denotation-unavailable"),
            Self::AbstractWork => Some("sample-work-limit"),
            Self::AbstractNumberWork => Some("sample-number-work-limit"),
            Self::AbstractEstablishment => Some("sample-establishment"),
            Self::PatternCapability | Self::PatternWork => Some("pattern-matching-capability"),
            _ => None,
        }
    }
}
pub(crate) struct Finding {
    pub(crate) sample_node: usize,
    pub(crate) paired_sample: Option<usize>,
    pub(crate) basis: Basis,
    pub(crate) code: &'static str,
    pub(crate) constraint: &'static str,
    pub(crate) message: &'static str,
    pub(crate) parameter: Option<usize>,
    pub(crate) alternative: Option<usize>,
}
pub(crate) enum Verdict {
    Equal,
    Different,
    Unavailable,
}
