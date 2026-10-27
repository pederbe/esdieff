//! Established source structure, metadata and vocabulary findings retain compact locations under finite work budgets.
//! Follows ADR 0003: Check findings and completeness,
//! ADR 0008: Reject the historical Action input quality, and
//! ADR 0009: Bounded processing and reporting.
use crate::AssessmentStatus::{Assessed, NotApplicable, Unassessed, Undetermined};
use crate::inventory::Requirement;
use crate::role::{Role, Vocabulary, class};
use crate::{Catalogue, Class, EffectiveModel, ExactNumber, Field, JsonKind, JsonView, SourceId};
mod coverage;
#[derive(Clone, Copy)]
pub(crate) enum CheckView<'source, 'view> {
    Source(JsonView<'source>),
    Effective {
        model: &'view EffectiveModel<'source>,
        node: usize,
    },
}
pub(crate) enum CheckChildren<'source, 'view> {
    Source(crate::Children<'source>),
    Effective {
        model: &'view EffectiveModel<'source>,
        next: Option<usize>,
    },
}
impl<'source, 'view> Iterator for CheckChildren<'source, 'view> {
    type Item = CheckView<'source, 'view>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Source(children) => return children.next().map(CheckView::Source),
            Self::Effective { model, next } => {
                while let Some(node) = *next {
                    let value = &model.nodes[node];
                    *next = value.next;
                    if !value.deleted {
                        return Some(CheckView::Effective { model, node });
                    }
                }
            }
        }
        None
    }
}
impl<'source, 'view> CheckView<'source, 'view> {
    fn source(self) -> JsonView<'source> {
        match self {
            Self::Source(json) => json,
            Self::Effective { model, node } => model.nodes[node].source,
        }
    }
    fn effective(self) -> Option<(SourceId, usize)> {
        match self {
            Self::Source(_) => None,
            Self::Effective { model, node } => Some((model.source, node)),
        }
    }
    fn unavailable(self) -> bool {
        matches!(self,Self::Effective {model,node} if model.nodes[node].unavailable.is_some())
    }
    fn member_name(self) -> Field<&'source str> {
        match self {
            Self::Source(json) => json.member_name(),
            Self::Effective { model, node } => {
                model.nodes[node].key.map_or(Field::Absent, Field::Present)
            }
        }
    }
    fn kind(self) -> JsonKind {
        self.source().kind()
    }
    fn text(self) -> Field<&'source str> {
        if self.unavailable() {
            Field::Unavailable("effective contribution unavailable")
        } else {
            self.source().text()
        }
    }
    fn number(self) -> Field<ExactNumber<'source>> {
        if self.unavailable() {
            Field::Unavailable("effective contribution unavailable")
        } else {
            self.source().number()
        }
    }
    fn children(self) -> CheckChildren<'source, 'view> {
        match self {
            Self::Source(json) => CheckChildren::Source(json.children()),
            Self::Effective { model, node } => CheckChildren::Effective {
                model,
                next: if self.unavailable() {
                    None
                } else {
                    model.nodes[node].first
                },
            },
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Rule {
    Required(crate::RequiredReferenceStatus),
    Pattern(crate::pattern::Condition),
    PatternMeaning(crate::pattern::Condition),
    FormalUint,
    FormalUnavailable(Requirement),
    EmptyNumericInterval,
    InvertedTextLengths,
    InvertedArrayCounts,
    PrimitiveConstType,
    NumericProof(crate::proof::NumericClass),
    ConsistencyExcluded(bool),
    NumericCapability {
        class: crate::proof::NumericClass,
        capability: crate::numeric::NumericCapability,
    },
    AdvisoryNumericLimit {
        class: crate::proof::ProofClass,
        operation: crate::proof::NumericOperation,
    },
    Unit(crate::units::Rule),
    ContentFormat(crate::content_format::Rule),
    NameColon,
    WorkLimit,
    ModifiedWorkLimit,
    BlockShape,
    NamespaceUri,
    NamespaceDefault,
    GroupShape,
    DefinitionShape,
    RoleUnavailable,
    GroupingCount,
    PositiveStep,
    DataNumber,
    DataNumberUnavailable,
    DataCount,
    DataString,
    DataBoolean,
    DataEnum,
    DataRequired,
    DataMap,
    DataEnumChoice,
    DataShape,
    NumberUnavailable,
    FeatureContextUnavailable,
    MetadataString,
    InformationAbsent,
    HistoricalActionInput,
    QualityNameSyntax,
    QualityUnfamiliar,
    OptionalVocabulary(u32),
    EffectiveOptionalVocabulary,
    LocatedOptionalVocabulary,
    CriticalUnsupported,
    FeatureDeclarationUnavailable,
    RecordedValueExcluded,
    ModifiedSyntax,
    ModifiedCalendar,
    ModifiedClock,
    ModifiedExceptionalSlot,
    ModifiedLeapSecond,
    ModifiedKnowledgeUnavailable,
    ModifiedUnavailable,
}
impl Rule {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::ConsistencyExcluded(default) => {
                if default {
                    "consistency.default_excluded"
                } else {
                    "consistency.const_excluded"
                }
            }
            Self::Required(status) => match status {
                crate::RequiredReferenceStatus::InvalidSyntax => "required.syntax",
                crate::RequiredReferenceStatus::WrongTarget => "required.target_role",
                crate::RequiredReferenceStatus::Missing => "required.target_missing",
                _ => "required.unavailable",
            },
            Self::Pattern(condition) | Self::PatternMeaning(condition) => condition.code(),
            Self::FormalUint => "formal.uint",
            Self::FormalUnavailable(_) => "formal.unavailable",
            Self::EmptyNumericInterval => "proof.empty_numeric_interval",
            Self::InvertedTextLengths => "proof.inverted_text_lengths",
            Self::InvertedArrayCounts => "proof.inverted_array_counts",
            Self::PrimitiveConstType => "proof.primitive_const_type",
            Self::NumericProof(class) => class.code(),
            Self::NumericCapability { .. } => "proof.numeric_capability_unavailable",
            Self::Unit(rule) => rule.code(),
            Self::ContentFormat(rule) => rule.code(),
            Self::NameColon => "name.colon",
            Self::WorkLimit | Self::ModifiedWorkLimit | Self::AdvisoryNumericLimit { .. } => {
                "operation.limit"
            }
            Self::BlockShape => "structure.block_shape",
            Self::NamespaceUri => "namespace.uri",
            Self::NamespaceDefault => "namespace.default",
            Self::GroupShape => "structure.group_shape",
            Self::DefinitionShape => "structure.definition_shape",
            Self::RoleUnavailable => "structure.role_unavailable",
            Self::GroupingCount => "grouping.count",
            Self::PositiveStep => "parameter.positive_step",
            Self::DataNumber => "data.number",
            Self::DataNumberUnavailable => "data.number_unavailable",
            Self::DataCount => "data.count",
            Self::DataString => "data.string",
            Self::DataBoolean => "data.boolean",
            Self::DataEnum => "data.enum",
            Self::DataRequired => "data.required",
            Self::DataMap => "data.map_shape",
            Self::DataEnumChoice => "data.enum_choice",
            Self::DataShape => "structure.data_shape",
            Self::NumberUnavailable => "grouping.number_unavailable",
            Self::FeatureContextUnavailable => "structure.feature_context_unavailable",
            Self::MetadataString => "metadata.string",
            Self::InformationAbsent => "metadata.info_absent",
            Self::HistoricalActionInput => "policy.historical_action_input",
            Self::QualityNameSyntax => "quality.name_syntax",
            Self::QualityUnfamiliar => "quality.unfamiliar",
            Self::OptionalVocabulary(_)
            | Self::EffectiveOptionalVocabulary
            | Self::LocatedOptionalVocabulary => "extension.optional_unsupported",
            Self::CriticalUnsupported => "extension.critical_unsupported",
            Self::FeatureDeclarationUnavailable => "extension.feature_declaration_unavailable",
            Self::RecordedValueExcluded => "extension.recorded_value_excluded",
            Self::ModifiedSyntax => "metadata.modified_syntax",
            Self::ModifiedCalendar => "metadata.modified_calendar",
            Self::ModifiedClock => "metadata.modified_clock",
            Self::ModifiedExceptionalSlot => "metadata.modified_exceptional_slot",
            Self::ModifiedLeapSecond => "metadata.modified_leap_second",
            Self::ModifiedKnowledgeUnavailable => "metadata.modified_knowledge_unavailable",
            Self::ModifiedUnavailable => "metadata.modified_unavailable",
        }
    }
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::ConsistencyExcluded(default) => {
                if default {
                    "The documentary default is excluded by its effective description"
                } else {
                    "The declared constant is excluded by its effective description"
                }
            }
            Self::Required(status) => match status {
                crate::RequiredReferenceStatus::InvalidSyntax => {
                    "Required declaration syntax is invalid"
                }
                crate::RequiredReferenceStatus::WrongTarget => {
                    "Required target is not a declaration at this position"
                }
                crate::RequiredReferenceStatus::Missing => {
                    "Required target was not found in the established scope"
                }
                _ => "Required declaration identity or context is unavailable",
            },
            Self::Pattern(condition) => condition.message(),
            Self::FormalUint => "The exact count is outside the imported CDDL uint domain",
            Self::FormalUnavailable(_) => {
                "Formal model admission requires unavailable decoding, meaning or finite numeric work"
            }
            Self::PatternMeaning(condition) => match condition {
                crate::pattern::Condition::Unsupported => {
                    "Effective pattern matching requires a capability outside the qualified implementation"
                }
                crate::pattern::Condition::Limit => {
                    "Effective pattern matching reached its finite operation or compilation capacity"
                }
                _ => "Effective pattern matching requires unavailable text, role or interpretation",
            },
            Self::AdvisoryNumericLimit { .. } => {
                "Initial contradiction proof requires more exact numeric work"
            }
            Self::EmptyNumericInterval => "These established bounds admit no numeric value",
            Self::InvertedTextLengths => "The minimum text length exceeds the maximum text length",
            Self::InvertedArrayCounts => "The minimum array count exceeds the maximum array count",
            Self::PrimitiveConstType => {
                "The primitive constant is incompatible with the established type or null admission"
            }
            Self::NumericProof(crate::proof::NumericClass::IntegerGap) => {
                "These established bounds admit no integer"
            }
            Self::NumericProof(crate::proof::NumericClass::ConstBound) => {
                "The sole numeric constant is excluded by an applicable bound"
            }
            Self::NumericProof(crate::proof::NumericClass::StepGap) => {
                "These established bounds admit no multiple of the positive step"
            }
            Self::NumericCapability { .. } => {
                "Expanded numeric contradiction advice requires unavailable capability"
            }
            Self::Unit(rule) => rule.message(),
            Self::ContentFormat(rule) => rule.message(),
            Self::NameColon => "Given Names cannot contain a colon in base SDF",
            Self::WorkLimit | Self::ModifiedWorkLimit => {
                "Source structural checking reached its finite work limit"
            }
            Self::BlockShape => "A document information or namespace block must be a map",
            Self::NamespaceUri => {
                "A namespace mapping must contain a syntactically valid URI string"
            }
            Self::NamespaceDefault => {
                "defaultNamespace must be a string selecting an existing namespace mapping"
            }
            Self::GroupShape => "A base definition group must be a map",
            Self::DefinitionShape => "A base named definition must be a map",
            Self::RoleUnavailable => {
                "Retained content has no established base role at this position"
            }
            Self::GroupingCount => {
                "A grouping repetition count must be a nonnegative mathematically integral number"
            }
            Self::PositiveStep => "multipleOf must be a positive number",
            Self::DataNumber => "A numerical data constraint must contain a number",
            Self::DataNumberUnavailable => {
                "Data parameter assessment requires unavailable exact numeric capability"
            }
            Self::DataCount => {
                "A data length or item count must be a nonnegative mathematically integral number"
            }
            Self::DataString => "A textual data quality must contain a string",
            Self::DataBoolean => "A data or Property flag must contain a Boolean",
            Self::DataEnum => "enum must contain a nonempty array of strings",
            Self::DataRequired => "Map-data required must contain a nonempty array of strings",
            Self::DataMap => {
                "A data-description collection, item or named full description must be a map"
            }
            Self::DataEnumChoice => "A data description cannot contain both enum and sdfChoice",
            Self::DataShape => {
                "An Action input/output or Event output data description must be a map"
            }
            Self::NumberUnavailable => {
                "Grouping repetition assessment requires unavailable exact numeric capability"
            }
            Self::FeatureContextUnavailable => {
                "Structural assessment requires unavailable critical-feature context"
            }
            Self::MetadataString => "An ordinary metadata quality must contain a string",
            Self::InformationAbsent => {
                "The document has no information block; add info for document discovery and processing"
            }
            Self::HistoricalActionInput => {
                "sdfRequiredInputData is refused under A001; use sdfInputData and, for named input fields, type: object with properties and required. Review the input structure rather than renaming the quality"
            }
            Self::QualityNameSyntax => {
                "A Quality Name must use the base ASCII spelling, with an optional lowercase letter-and-digit prefix"
            }
            Self::QualityUnfamiliar => {
                "Unfamiliar unqualified Quality Name; check its spelling or the applicable extension vocabulary"
            }
            Self::OptionalVocabulary(_) | Self::EffectiveOptionalVocabulary => {
                "Recognized Quality Name spellings or extension selectors have unsupported optional meaning here; qualified spelling recognition does not establish registration"
            }
            Self::LocatedOptionalVocabulary => {
                "The meaning of this quality or selector is unsupported at this position"
            }
            Self::CriticalUnsupported => {
                "The readable critical-feature text list declares meaning that this processor does not understand; dependent conclusions are unavailable"
            }
            Self::FeatureDeclarationUnavailable => {
                "Critical-feature declaration meaning is unavailable for this wider or unreadable form; retained framework content is not rejected by a closed grammar"
            }
            Self::RecordedValueExcluded => {
                "The recorded OMNA registration defines oma:id as a CDDL uint, so its value must be a whole number from 0 through 18446744073709551615"
            }
            Self::ModifiedSyntax => {
                "modified must be a full date or a T-separated UTC date-time ending in Z"
            }
            Self::ModifiedCalendar => "modified contains a date outside the Gregorian calendar",
            Self::ModifiedClock => "modified contains a clock field outside its permitted range",
            Self::ModifiedExceptionalSlot => {
                "An inserted second requires UTC 23:59 on the final Gregorian day of a month"
            }
            Self::ModifiedLeapSecond => {
                "Recorded IERS knowledge establishes no inserted second at this UTC date"
            }
            Self::ModifiedKnowledgeUnavailable => {
                "This possible UTC adjustment slot needs leap knowledge outside the recorded interval"
            }
            Self::ModifiedUnavailable => {
                "Modified metadata assessment requires unavailable text, meaning or capability"
            }
        }
    }
    pub(crate) fn citation(self) -> Option<&'static str> {
        match self {
            Self::ConsistencyExcluded(_) => Some("RFC 9880 Section 4.7.2; Appendix C"),
            Self::Required(_) => Some("RFC 9880 Section 4.5"),
            Self::Pattern(crate::pattern::Condition::Invalid) => {
                Some("RFC 9880 Appendix C.2; ECMA-262 2025 Section 22.2.1")
            }
            Self::Pattern(_) | Self::PatternMeaning(_) | Self::FormalUnavailable(_) => None,
            Self::FormalUint => {
                Some("RFC 9880 Appendix A; RFC 8610 Sections 3.3 and 3.5.4; Appendices D and E")
            }
            Self::EmptyNumericInterval => Some("RFC 9880 Appendix C.1"),
            Self::NumericProof(_) => Some("RFC 9880 Appendix C.1"),
            Self::InvertedTextLengths => Some("RFC 9880 Appendix C.2"),
            Self::InvertedArrayCounts => Some("RFC 9880 Appendix C.4"),
            Self::PrimitiveConstType => Some("RFC 9880 Appendix C; Section 4.7"),
            Self::Unit(rule) => rule.citation(),
            Self::ContentFormat(rule) => rule.citation(),
            Self::NameColon => Some("RFC 9880 Section 2.3.1"),
            Self::QualityNameSyntax => Some("RFC 9880 Section 2.3.3; Appendix A quality-name"),
            Self::BlockShape => Some("RFC 9880 Sections 3.1 and 3.2"),
            Self::NamespaceUri => Some("RFC 9880 Section 3.2; RFC 3986 Section 3"),
            Self::NamespaceDefault => Some("RFC 9880 Section 3.2"),
            Self::GroupShape | Self::DefinitionShape => Some("RFC 9880 Section 3.3"),
            Self::GroupingCount => Some("RFC 9880 Sections 2.2.1 and 6.3; Appendix A"),
            Self::PositiveStep => Some("RFC 9880 Appendix C.1"),
            Self::DataNumber => Some("RFC 9880 Appendix A and Appendix C.1"),
            Self::DataCount => Some("RFC 9880 Appendix A and Appendix C"),
            Self::DataString => Some("RFC 9880 Appendix A"),
            Self::DataBoolean => Some("RFC 9880 Section 5.2 and Appendix A"),
            Self::DataEnum => Some("RFC 9880 Section 4.7.2 and Appendix A"),
            Self::DataRequired => Some("RFC 9880 Appendix A"),
            Self::DataMap => Some("RFC 9880 Appendix A"),
            Self::DataEnumChoice => Some("RFC 9880 Section 4.7.2 and Appendix A"),
            Self::DataShape => Some("RFC 9880 Sections 5.3 and 5.4"),
            Self::MetadataString => Some("RFC 9880 Sections 3.1 and 4.6; Appendix A"),
            Self::InformationAbsent => Some("RFC 9880 Section 3.1"),
            Self::QualityUnfamiliar => Some("RFC 9880 Sections 2.3.1 and 2.3.3"),
            Self::RecordedValueExcluded => Some("OMNA SDF Quality Names registry"),
            Self::ModifiedSyntax => Some("RFC 9880 Appendix A; RFC 5234 Section 2.3"),
            Self::ModifiedCalendar => {
                Some("RFC 9880 Appendix A; RFC 3339 Section 5.7 and Appendix C")
            }
            Self::ModifiedClock | Self::ModifiedExceptionalSlot | Self::ModifiedLeapSecond => {
                Some("RFC 9880 Appendix A; RFC 3339 Section 5.7")
            }
            Self::WorkLimit
            | Self::ModifiedWorkLimit
            | Self::AdvisoryNumericLimit { .. }
            | Self::NumericCapability { .. }
            | Self::RoleUnavailable
            | Self::NumberUnavailable
            | Self::DataNumberUnavailable
            | Self::HistoricalActionInput
            | Self::OptionalVocabulary(_)
            | Self::EffectiveOptionalVocabulary
            | Self::LocatedOptionalVocabulary
            | Self::CriticalUnsupported
            | Self::FeatureDeclarationUnavailable
            | Self::FeatureContextUnavailable
            | Self::ModifiedUnavailable
            | Self::ModifiedKnowledgeUnavailable => None,
        }
    }
    pub(crate) fn severity(self) -> &'static str {
        match self {
            Self::ConsistencyExcluded(_) => "warning",
            Self::Required(
                crate::RequiredReferenceStatus::WrongTarget
                | crate::RequiredReferenceStatus::InvalidSyntax,
            ) => "error",
            Self::Required(crate::RequiredReferenceStatus::Missing) => "notice",
            Self::Required(_) => "incomplete",
            Self::Pattern(crate::pattern::Condition::Invalid) => "error",
            Self::Pattern(_) | Self::PatternMeaning(_) | Self::FormalUnavailable(_) => "incomplete",
            Self::InformationAbsent
            | Self::QualityUnfamiliar
            | Self::EmptyNumericInterval
            | Self::InvertedTextLengths
            | Self::InvertedArrayCounts
            | Self::PrimitiveConstType => "warning",
            Self::NumericProof(_) => "warning",
            Self::Unit(rule) => rule.severity(),
            Self::ContentFormat(rule) => rule.severity(),
            Self::HistoricalActionInput | Self::RecordedValueExcluded => "error",
            Self::OptionalVocabulary(_)
            | Self::EffectiveOptionalVocabulary
            | Self::LocatedOptionalVocabulary => "notice",
            _ if self.citation().is_some() => "error",
            _ => "incomplete",
        }
    }
    pub(crate) fn basis_kind(self) -> &'static str {
        match self {
            Self::ConsistencyExcluded(_) => "advice",
            Self::Required(
                crate::RequiredReferenceStatus::WrongTarget
                | crate::RequiredReferenceStatus::InvalidSyntax,
            ) => "rfc",
            Self::Required(crate::RequiredReferenceStatus::Missing) => "advice",
            Self::Required(_) => "interpretation",
            Self::InformationAbsent
            | Self::QualityUnfamiliar
            | Self::EmptyNumericInterval
            | Self::InvertedTextLengths
            | Self::InvertedArrayCounts
            | Self::PrimitiveConstType => "advice",
            Self::NumericProof(_) => "advice",
            Self::Unit(rule) => rule.basis_kind(),
            Self::ContentFormat(rule) => rule.basis_kind(),
            Self::HistoricalActionInput => "acceptance",
            Self::RecordedValueExcluded => "registration",
            _ if self.citation().is_some() => "rfc",
            _ => "operational",
        }
    }
    pub(crate) fn basis_id(self) -> Option<&'static str> {
        match self {
            Self::ConsistencyExcluded(_) => Some(self.code()),
            Self::Required(
                crate::RequiredReferenceStatus::WrongTarget
                | crate::RequiredReferenceStatus::InvalidSyntax,
            ) => None,
            Self::Required(_) => Some(self.code()),
            Self::Pattern(condition) => condition.basis_id(),
            Self::PatternMeaning(_) => Some("pattern-matching-capability"),
            Self::FormalUint | Self::FormalUnavailable(_) => Some("formal-model-admission"),
            Self::EmptyNumericInterval
            | Self::InvertedTextLengths
            | Self::InvertedArrayCounts
            | Self::PrimitiveConstType => Some(self.code()),
            Self::NumericProof(_) => Some(self.code()),
            Self::NumericCapability { .. } => Some("expanded-numeric-capability"),
            Self::Unit(rule) => rule.basis_id(),
            Self::ContentFormat(rule) => rule.basis_id(),
            Self::InformationAbsent => Some("metadata.info_absent"),
            Self::QualityUnfamiliar => Some("quality.unfamiliar"),
            Self::HistoricalActionInput => Some("A001"),
            Self::RecordedValueExcluded => Some("oma:id"),
            Self::OptionalVocabulary(_)
            | Self::EffectiveOptionalVocabulary
            | Self::LocatedOptionalVocabulary => Some("optional-vocabulary"),
            Self::CriticalUnsupported | Self::FeatureDeclarationUnavailable => {
                Some("critical-feature-understanding")
            }
            Self::ModifiedUnavailable => Some("modified-establishment"),
            Self::ModifiedKnowledgeUnavailable => Some("temporal-knowledge"),
            Self::AdvisoryNumericLimit { .. } => Some("advisory-numeric-work"),
            Self::ModifiedLeapSecond => Some(crate::scalar::KNOWLEDGE_ID),
            _ if self.citation().is_none() => Some("source-structural-checking"),
            _ => None,
        }
    }
    pub(crate) fn observed_occurrences(self) -> Option<u32> {
        match self {
            Self::OptionalVocabulary(count) => Some(count),
            Self::LocatedOptionalVocabulary => Some(1),
            _ => None,
        }
    }
    pub(crate) fn names_quality(self) -> bool {
        matches!(
            self,
            Self::QualityUnfamiliar | Self::LocatedOptionalVocabulary
        )
    }
    pub(crate) fn assessment_unit(self) -> Option<&'static str> {
        if let Self::ConsistencyExcluded(default) = self {
            return Some(crate::sample::CONSISTENCY_AREAS[usize::from(default)]);
        }
        if matches!(self, Self::NumericProof(_) | Self::NumericCapability { .. }) {
            return Some(crate::proof::NUMERIC_AREA);
        }
        if matches!(self, Self::Required(_)) {
            return Some("inspection");
        }
        if matches!(self, Self::Pattern(_)) {
            return Some(crate::pattern::AREA);
        }
        if matches!(self, Self::PatternMeaning(_)) {
            return Some("full.pattern-format/1");
        }
        if matches!(self, Self::FormalUnavailable(Requirement::M035)) {
            return Some("full.data-grammar/1");
        }
        if matches!(self, Self::FormalUint | Self::FormalUnavailable(_)) {
            return Some("full.formal-numbers/1");
        }
        if matches!(
            self,
            Self::EmptyNumericInterval
                | Self::InvertedTextLengths
                | Self::InvertedArrayCounts
                | Self::PrimitiveConstType
                | Self::AdvisoryNumericLimit { .. }
        ) {
            return Some(crate::proof::AREA);
        }
        if let Self::Unit(rule) = self {
            return Some(rule.area());
        }
        if let Self::ContentFormat(rule) = self {
            return Some(rule.area());
        }
        if matches!(self, Self::RecordedValueExcluded) {
            return Some(crate::inventory::DESCRIPTORS[Requirement::M028 as usize].area);
        }
        matches!(
            self,
            Self::ModifiedSyntax
                | Self::ModifiedWorkLimit
                | Self::ModifiedCalendar
                | Self::ModifiedClock
                | Self::ModifiedExceptionalSlot
                | Self::ModifiedUnavailable
                | Self::ModifiedLeapSecond
                | Self::ModifiedKnowledgeUnavailable
        )
        .then_some(crate::assessment::MODIFIED_AREA)
    }
}
#[derive(Clone)]
pub(crate) struct Finding<'a> {
    pub(crate) json: JsonView<'a>,
    pub(crate) effective: Option<(SourceId, usize)>,
    pub(crate) rule: Rule,
    pub(crate) proof: Option<crate::proof::Witness<'a>>,
    pub(crate) replacement: Option<String>,
    pub(crate) coverage_requirement: Option<crate::inventory::Requirement>,
}
#[derive(Default)]
pub(crate) struct ModifiedScope {
    processed: usize,
    applicable: bool,
    unknown: bool,
    pub(crate) knowledge_used: bool,
}
impl ModifiedScope {
    pub(crate) fn defer(&mut self) {
        self.unknown = true;
    }
    pub(crate) fn status(&self, expected: usize) -> crate::AssessmentStatus {
        if self.unknown || self.processed < expected {
            crate::AssessmentStatus::Undetermined
        } else if self.applicable {
            crate::AssessmentStatus::Assessed
        } else {
            crate::AssessmentStatus::NotApplicable
        }
    }
}
#[derive(Default)]
pub(crate) struct Structure<'a> {
    pub(crate) temporal_use: u8,
    pub(crate) format_use: u8,
    pub(crate) abstract_use: u8,
    /// Extension vocabulary that reported conclusions set aside, which decides whether the
    /// report names `undeclared-extensions-ignorable/1` from ADR 0037: Undeclared extension
    /// vocabulary.
    pub(crate) set_aside: crate::role::SetAside,
    pub(crate) findings: Vec<Finding<'a>>,
    pub(crate) total: usize,
    pub(crate) errors: u32,
    pub(crate) warnings: u32,
    pub(crate) notices: u32,
    pub(crate) count_omitted: bool,
    pub(crate) bytes_omitted: bool,
    pub(crate) work_limited: bool,
    pub(crate) contexts: Vec<SourceContext<'a>>,
    pub(crate) modified: ModifiedScope,
    pub(crate) advisory: crate::proof::Scope,
    pub(crate) numeric: crate::proof::Scope,
    pub(crate) consistency: [crate::proof::Scope; 2],
    pub(crate) pattern: crate::pattern::Scope,
    pub(crate) coverage: crate::inventory::State,
}

pub(crate) struct SourceContext<'a> {
    pub(crate) id: SourceId,
    pub(crate) gate: FeatureContext<'a>,
    pub(crate) roles: Vec<u32>,
}
pub(crate) struct ParameterLedger {
    pub(crate) storage_remaining: usize,
}
pub(crate) fn sample_pair_fixed_headers() -> usize {
    use std::mem::size_of;
    size_of::<Checker<'_, '_, '_>>()
        + size_of::<CheckView<'_, '_>>()
        + size_of::<CheckChildren<'_, '_>>()
        + size_of::<SampleRules>()
        + size_of::<Role>()
        + size_of::<bool>()
}
impl<'a> Structure<'a> {
    // Keep the borrowed view and the two original storage owners explicit at this seam.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sample_pair(
        &mut self,
        catalogue: &'a Catalogue,
        model: &EffectiveModel<'a>,
        node: usize,
        role: Role,
        requested: SampleRules,
        budget: &mut crate::allowance::OperationBudget,
        ledger: &mut ParameterLedger,
    ) -> bool {
        let view = CheckView::Effective { model, node };
        let mut checker = Checker {
            budget,
            catalogue,
            result: std::mem::take(self),
            storage: &mut ledger.storage_remaining,
            optional: 0,
            jobs: Vec::new(),
            assessment: true,
            numeric_enabled: false,
            consistency_enabled: false,
            equality: None,
            information_strings: false,
            sample: true,
            sample_rules: requested,
            skip_local: false,
            direct: true,
            roles: Vec::new(),
            number_work: None,
            alternative: None,
        };
        let completed = checker.debit(view, 1) && checker.visit(view, role, false);
        *self = checker.result;
        completed
    }
    pub(crate) fn required(
        &mut self,
        catalogue: &'a Catalogue,
        model: &EffectiveModel<'a>,
        collection: &crate::effective::Collection<'a>,
        budget: &mut crate::allowance::OperationBudget,
    ) {
        let status = if !collection.complete {
            Undetermined
        } else if collection.owners_known == 0 {
            NotApplicable
        } else {
            Assessed
        };
        self.coverage.initial(5, status);
        self.coverage.observe(Requirement::M020, status);
        let before = (self.total, self.errors, self.notices);
        let mut storage = catalogue
            .limits
            .detail_bytes
            .saturating_sub(self.findings.capacity() * std::mem::size_of::<Finding<'_>>());
        let mut checker = Checker {
            budget,
            catalogue,
            result: std::mem::take(self),
            storage: &mut storage,
            optional: 0,
            jobs: Vec::new(),
            assessment: true,
            numeric_enabled: false,
            consistency_enabled: false,
            equality: None,
            information_strings: false,
            sample: false,
            sample_rules: SampleRules::default(),
            skip_local: false,
            direct: false,
            roles: Vec::new(),
            number_work: None,
            alternative: None,
        };
        let view = |node| match node {
            crate::effective::ViewNode::Source(json) => CheckView::Source(json),
            crate::effective::ViewNode::Effective(node) => CheckView::Effective { model, node },
        };
        'owners: for owner in &collection.owners {
            if !charge_exact(&mut checker.budget.work, 1) {
                checker.result.bytes_omitted = true;
                break;
            }
            if owner.state == crate::RequirednessState::Invalid && owner.references.is_empty() {
                checker.record(
                    view(owner.syntax.unwrap_or(owner.json)),
                    Rule::Required(crate::RequiredReferenceStatus::InvalidSyntax),
                );
            }
            for reference in &owner.references {
                if !charge_exact(&mut checker.budget.work, 1) {
                    checker.result.bytes_omitted = true;
                    break 'owners;
                }
                if reference.error_already_counted
                    || reference.status == crate::RequiredReferenceStatus::Resolved
                {
                    continue;
                }
                checker.record(view(reference.json), Rule::Required(reference.status));
            }
        }
        // Aggregate observations are authoritative even if optional folding stops.
        checker.result.total = before.0 + collection.conditions as usize;
        checker.result.errors = before.1 + collection.errors;
        checker.result.notices = before.2 + collection.notices;
        *self = checker.result;
    }
    pub(crate) fn dependency_modified(
        &mut self,
        catalogue: &'a Catalogue,
        gate: FeatureContext<'a>,
        budget: &mut crate::allowance::OperationBudget,
    ) {
        let mut storage = catalogue.limits.detail_bytes;
        let mut checker = Checker {
            budget,
            catalogue,
            result: std::mem::take(self),
            storage: &mut storage,
            optional: 0,
            jobs: Vec::new(),
            assessment: true,
            numeric_enabled: false,
            consistency_enabled: false,
            equality: None,
            information_strings: false,
            sample: false,
            sample_rules: SampleRules::default(),
            skip_local: false,
            direct: false,
            roles: Vec::new(),
            number_work: None,
            alternative: None,
        };
        match gate {
            FeatureContext::Clear(modified) => {
                checker.result.modified.processed += 1;
                if let Some(json) = modified {
                    if json.kind() != JsonKind::String {
                        checker.record(json, Rule::MetadataString);
                    }
                    checker.check_modified(json);
                }
            }
            _ => checker.result.modified.defer(),
        }
        *self = checker.result;
    }
    pub(crate) fn check(
        catalogue: &'a Catalogue,
        scope: &[SourceId],
        budget: &mut crate::allowance::OperationBudget,
        profile: crate::Assessment,
    ) -> Self {
        let mut storage = catalogue.limits.detail_bytes;
        let mut checker = Checker {
            budget,
            catalogue,
            result: Self::default(),
            storage: &mut storage,
            optional: 0,
            jobs: Vec::new(),
            assessment: true,
            numeric_enabled: false,
            consistency_enabled: false,
            equality: None,
            information_strings: false,
            sample: false,
            sample_rules: SampleRules::default(),
            skip_local: false,
            direct: false,
            roles: Vec::new(),
            number_work: None,
            alternative: None,
        };
        let enabled = &crate::assessment::profile(profile).enabled;
        checker.result.coverage = crate::inventory::State::start(enabled.full);
        checker.result.numeric.unknown = enabled.numeric;
        if enabled.consistency {
            for scope in &mut checker.result.consistency {
                scope.unknown = true;
            }
        }
        let expanded = enabled.modified;
        for id in scope {
            let source = catalogue.source(*id).expect("validated source");
            let envelope = if source.complete
                && source.acquisition != crate::Acquisition::Failed
                && (source.parsed.root.is_some() || source.parsed.rejected)
            {
                Assessed
            } else {
                Undetermined
            };
            checker.result.coverage.initial(0, envelope);
            checker.result.coverage.observe(Requirement::M001, envelope);
            checker.roles = Vec::new();
            let count = source.parsed.nodes.len();
            if crate::allowance::charge(
                &mut checker.budget.allocation,
                count * std::mem::size_of::<u32>(),
            ) {
                checker.roles = vec![0; count];
            }
            let gate = if let Field::Present(root) = source.json() {
                checker.optional = 0;
                checker.information_strings = false;
                let gate = checker.feature_context(root);
                match gate {
                    FeatureContext::Clear(modified) => {
                        let modified_complete = if expanded {
                            checker.result.modified.processed += 1;
                            modified.is_none_or(|json| checker.check_modified(json))
                        } else {
                            true
                        };
                        if modified_complete
                            && checker.schedule(CheckView::Source(root), Role::Document, false)
                        {
                            checker.drain();
                        }
                        if checker.optional > 0 {
                            checker.record(root, Rule::OptionalVocabulary(checker.optional));
                        }
                    }
                    FeatureContext::Deferred(json) => {
                        checker.coverage_unknown(Role::Document);
                        checker.record(json, Rule::FeatureContextUnavailable)
                    }
                    FeatureContext::Limited => checker.coverage_unknown(Role::Document),
                }
                gate
            } else {
                checker.coverage_unknown(Role::Document);
                FeatureContext::Limited
            };
            let roles = std::mem::take(&mut checker.roles);
            if crate::allowance::push(
                &mut checker.result.contexts,
                SourceContext {
                    id: *id,
                    gate,
                    roles,
                },
                &mut checker.budget.allocation,
            )
            .is_err()
            {
                checker.result.bytes_omitted = true;
                break;
            }
        }
        checker.result
    }
    pub(crate) fn preflight(
        catalogue: &'a Catalogue,
        root: JsonView<'a>,
        budget: &mut crate::allowance::OperationBudget,
    ) -> FeatureContext<'a> {
        let mut storage = 0;
        let mut checker = Checker {
            budget,
            catalogue,
            result: Self::default(),
            storage: &mut storage,
            optional: 0,
            jobs: Vec::new(),
            assessment: false,
            numeric_enabled: false,
            consistency_enabled: false,
            equality: None,
            information_strings: false,
            sample: false,
            sample_rules: SampleRules::default(),
            skip_local: false,
            direct: false,
            roles: Vec::new(),
            number_work: None,
            alternative: None,
        };
        checker.feature_context(root)
    }
    pub(crate) fn effective(
        &mut self,
        catalogue: &'a Catalogue,
        models: &[EffectiveModel<'a>],
        budget: &mut crate::allowance::OperationBudget,
        profile: crate::Assessment,
    ) {
        self.effective_mode(
            catalogue,
            models,
            budget,
            false,
            SampleRules::default(),
            profile,
        )
    }
    fn effective_mode(
        &mut self,
        catalogue: &'a Catalogue,
        models: &[EffectiveModel<'a>],
        budget: &mut crate::allowance::OperationBudget,
        sample: bool,
        sample_rules: SampleRules,
        profile: crate::Assessment,
    ) {
        let enabled = &crate::assessment::profile(profile).enabled;
        let numeric_enabled = enabled.numeric;
        let consistency_enabled = enabled.consistency;
        if enabled.full
            && !crate::allowance::charge(&mut budget.allocation, coverage::meaning_fixed_headers())
        {
            self.work_limited = true;
            self.coverage.requirements.fill(Undetermined);
            self.advisory.unknown = true;
            return;
        }
        let mut storage = catalogue.limits.detail_bytes;
        let mut checker = Checker {
            budget,
            catalogue,
            result: std::mem::take(self),
            storage: &mut storage,
            optional: 0,
            jobs: Vec::new(),
            assessment: true,
            numeric_enabled,
            consistency_enabled,
            equality: (consistency_enabled || enabled.full)
                .then(|| crate::equality::EqualityWork::new(catalogue.limits.equality_pairs)),
            information_strings: false,
            sample,
            sample_rules,
            skip_local: false,
            direct: false,
            roles: Vec::new(),
            number_work: (!sample)
                .then(|| crate::NumberWork::from_remaining(catalogue.numeric_remaining)),
            alternative: None,
        };
        if numeric_enabled {
            checker.result.numeric = crate::proof::Scope::default();
        }
        if consistency_enabled {
            checker.result.consistency = Default::default();
        }
        'models: for model in models {
            if let Some(node) = model.selected {
                checker.optional = 0;
                let role = model.nodes[node].role.unwrap_or(Role::Document);
                if !checker.schedule(CheckView::Effective { model, node }, role, false)
                    || !checker.drain()
                {
                    break;
                }
                for (branch, identity) in model.alternative_roots() {
                    checker.alternative = Some(identity);
                    if model.nodes[branch].source.kind() != JsonKind::Map {
                        checker.result.advisory.unknown = true;
                        checker.result.numeric.unknown |= numeric_enabled;
                        for scope in &mut checker.result.consistency {
                            scope.unknown |= consistency_enabled;
                        }
                    }
                    if !checker.schedule(
                        CheckView::Effective {
                            model,
                            node: branch,
                        },
                        Role::Data,
                        false,
                    ) || !checker.drain()
                    {
                        break 'models;
                    }
                }
                checker.alternative = None;
                if checker.optional > 0 {
                    checker.record(
                        CheckView::Effective { model, node },
                        Rule::EffectiveOptionalVocabulary,
                    );
                }
            } else {
                checker.result.advisory.unknown = true;
                checker.result.numeric.unknown |= numeric_enabled;
                for scope in &mut checker.result.consistency {
                    scope.unknown |= consistency_enabled;
                }
                if checker.result.pattern.enabled {
                    checker.result.pattern.unknown = true;
                }
            }
        }
        checker.result.set_aside |= std::mem::take(&mut checker.budget.set_aside);
        *self = checker.result;
    }
}
struct Checker<'a, 'view, 'run> {
    // Nested assessment uses the original allowance; nodes and references stay
    // resolver-owned. Finding capacity has its own workflow or parameter owner.
    budget: &'run mut crate::allowance::OperationBudget,
    catalogue: &'a Catalogue,
    result: Structure<'a>,
    storage: &'run mut usize,
    optional: u32,
    jobs: Vec<(CheckView<'a, 'view>, Role, bool)>,
    assessment: bool,
    numeric_enabled: bool,
    consistency_enabled: bool,
    equality: Option<crate::equality::EqualityWork<'view>>,
    information_strings: bool,
    sample: bool,
    sample_rules: SampleRules,
    skip_local: bool,
    direct: bool,
    roles: Vec<u32>,
    number_work: Option<crate::NumberWork>,
    alternative: Option<crate::AlternativeId<'a>>,
}
impl<'a, 'view> Checker<'a, 'view, '_> {
    fn defer_consistency(&mut self) {
        for scope in &mut self.result.consistency {
            scope.unknown |= self.consistency_enabled;
        }
    }
    fn check_consistency(&mut self, view: CheckView<'a, 'view>) {
        let CheckView::Effective { model, node } = view else {
            return;
        };
        let description = crate::EffectiveJson { model, node };
        if !model.clear_feature
            || description.n().unavailable.is_some()
            || description.n().donor_unavailable.is_some()
            || !description.consistency_meaning_available()
        {
            for scope in &mut self.result.consistency {
                scope.unknown = true;
            }
            return;
        }
        for (area, key) in ["const", "default"].into_iter().enumerate() {
            let candidate = description.lookup(key, self.budget);
            let local = if area == 1 {
                model.local_default(node, &mut self.budget.work)
            } else {
                Field::Present(true)
            };
            let alternatives = if area == 0 {
                crate::sample::alternatives(description, self.budget)
            } else {
                Field::Absent
            };
            // A choice replaces shared const restrictions; each final branch owns its candidate.
            if matches!(candidate, Field::Absent)
                || matches!(local, Field::Present(false))
                || area == 0 && matches!(alternatives, Field::Present(_))
            {
                continue;
            }
            self.result.consistency[area].applicable = true;
            if let Field::Present(candidate) = candidate
                && matches!(local, Field::Present(true))
            {
                let evaluation = crate::sample::evaluate_branches(
                    &crate::value::ValueContext {
                        description,
                        root: crate::value::BorrowedValue::Model(candidate),
                    },
                    self.budget,
                    self.number_work
                        .as_mut()
                        .expect("shared effective numeric counter"),
                    self.equality.as_mut().expect("request equality counter"),
                );
                self.result.temporal_use |= evaluation.temporal_use;
                self.result.abstract_use |= self
                    .equality
                    .as_ref()
                    .expect("request equality counter")
                    .abstract_use;
                self.result.format_use |= self
                    .equality
                    .as_ref()
                    .expect("request equality counter")
                    .format_use;
                self.result.consistency[area].unknown |= evaluation.areas.iter().any(|status| {
                    matches!(
                        status,
                        crate::AssessmentStatus::Unassessed | crate::AssessmentStatus::Undetermined
                    )
                });
                // The shared evaluator's model sink survives tentative sample
                // branch replacement and diagnostic-detail omission.
                for index in
                    self.budget.patterns.reported_models..self.budget.patterns.model_patterns.len()
                {
                    let issue = &self.budget.patterns.model_patterns[index];
                    if issue.invalid && !issue.reported {
                        debug_assert_eq!(issue.source, model.source);
                        let node = issue.node;
                        self.budget.patterns.model_patterns[index].reported = true;
                        self.record(
                            CheckView::Effective { model, node },
                            Rule::Pattern(crate::pattern::Condition::Invalid),
                        );
                    }
                }
                self.budget.patterns.reported_models = self.budget.patterns.model_patterns.len();
                if evaluation.admission == crate::sample::Admission::Rejected {
                    // Count one exclusion per candidate, independently of leaf count or retention.
                    let evidence = evaluation
                        .findings
                        .iter()
                        .find(|finding| !finding.basis.incomplete());
                    let Some(evidence) = evidence else {
                        self.result.total += 1;
                        self.result.warnings += 1;
                        self.result.bytes_omitted = true;
                        continue;
                    };
                    let operands = crate::proof::OperandIndices::new([
                        Some(evidence.sample_node),
                        evidence.parameter,
                        evidence.paired_sample,
                        evidence.alternative,
                    ]);
                    if let Ok(operands) = operands {
                        self.record_with_witness(
                            view,
                            Rule::ConsistencyExcluded(area == 1),
                            Some(crate::proof::Witness {
                                operands,
                                primitive: None,
                                alternative: self.alternative,
                            }),
                        );
                    } else {
                        self.result.consistency[area].unknown = true;
                        self.result.total += 1;
                        self.result.warnings += 1;
                        self.result.bytes_omitted = true;
                    }
                }
                continue;
            }
            self.result.consistency[area].unknown = true;
        }
    }
    fn feature_context(&mut self, root: JsonView<'a>) -> FeatureContext<'a> {
        if !self.debit(root, 1) {
            return FeatureContext::Limited;
        }
        if !root.source.complete {
            return FeatureContext::Deferred(root);
        }
        if root.kind() != JsonKind::Map {
            return FeatureContext::Clear(None);
        }
        let mut info = None;
        let mut unknown = None;
        for child in root.children() {
            if !self.debit(child, child.n().key.as_ref().map_or(1, |key| key.len() + 1)) {
                return FeatureContext::Limited;
            }
            match child.member_name() {
                Field::Present("info") => info = Some(child),
                Field::Present(_) => {}
                _ => unknown = Some(child),
            }
        }
        if let Some(unknown) = unknown {
            return FeatureContext::Deferred(unknown);
        }
        let Some(info) = info else {
            if self.assessment {
                self.record(root, Rule::InformationAbsent);
            }
            return FeatureContext::Clear(None);
        };
        if info.kind() != JsonKind::Map {
            return FeatureContext::Deferred(info);
        }
        let mut features = None;
        let mut modified = None;
        let mut unknown = None;
        for child in info.children() {
            if !self.debit(child, child.n().key.as_ref().map_or(1, |key| key.len() + 1)) {
                return FeatureContext::Limited;
            }
            match child.member_name() {
                Field::Present("features") => features = Some(child),
                Field::Present("modified") => modified = Some(child),
                Field::Present(_) => {}
                _ => unknown = Some(child),
            }
        }
        if let Some(unknown) = unknown {
            return FeatureContext::Deferred(unknown);
        }
        let context = match features {
            None => FeatureContext::Clear(modified),
            Some(features)
                if features.kind() == JsonKind::Array && features.n().first.is_none() =>
            {
                FeatureContext::Clear(modified)
            }
            Some(features) => {
                let mut readable_list = features.kind() == JsonKind::Array;
                if readable_list {
                    for feature in features.children() {
                        if !self.debit(feature, 1) {
                            return FeatureContext::Limited;
                        }
                        readable_list &= matches!(feature.text(), Field::Present(_));
                    }
                }
                self.record(
                    features,
                    if readable_list {
                        Rule::CriticalUnsupported
                    } else {
                        Rule::FeatureDeclarationUnavailable
                    },
                );
                FeatureContext::Deferred(features)
            }
        };
        if let FeatureContext::Clear(modified) = context {
            if self.assessment {
                for child in info.children() {
                    if !self.debit(child, child.n().key.as_ref().map_or(1, |key| key.len() + 1)) {
                        return FeatureContext::Limited;
                    }
                    if let Field::Present(key) = child.member_name()
                        && Role::Information.metadata(key)
                        && child.kind() != JsonKind::String
                    {
                        self.record(child, Rule::MetadataString);
                    }
                }
                self.information_strings = true;
                if let Some(mask) = self.roles.get_mut(info.node) {
                    *mask |= 1 << 10;
                }
            }
            return FeatureContext::Clear(modified);
        }
        context
    }
    fn debit(&mut self, json: impl Into<CheckView<'a, 'view>>, cost: usize) -> bool {
        self.debit_with_limit(json, cost, Rule::WorkLimit)
    }
    fn debit_with_limit(
        &mut self,
        json: impl Into<CheckView<'a, 'view>>,
        cost: usize,
        limit: Rule,
    ) -> bool {
        if charge_exact(&mut self.budget.work, cost) {
            true
        } else {
            if self.number_work.is_some() {
                self.result.advisory.unknown = true;
                self.result.numeric.unknown |= self.numeric_enabled;
            }
            self.record(json.into(), limit);
            false
        }
    }
    fn check_modified(&mut self, json: JsonView<'a>) -> bool {
        self.result.modified.applicable = true;
        // The authoritative Information pass already recorded any nonstring value.
        if json.kind() != JsonKind::String {
            return true;
        }
        let Field::Present(text) = json.text() else {
            self.result.modified.unknown = true;
            self.record(json, Rule::ModifiedUnavailable);
            return true;
        };
        if !self.debit_with_limit(json, text.len() * 2 + 256, Rule::ModifiedWorkLimit) {
            self.result.modified.unknown = true;
            return false;
        }
        let evaluation = crate::modified::evaluate(text);
        self.result.modified.knowledge_used |= evaluation.knowledge_used;
        match evaluation.verdict {
            crate::modified::Verdict::Established => {}
            crate::modified::Verdict::Syntax => self.record(json, Rule::ModifiedSyntax),
            crate::modified::Verdict::Calendar => self.record(json, Rule::ModifiedCalendar),
            crate::modified::Verdict::Clock => self.record(json, Rule::ModifiedClock),
            crate::modified::Verdict::ExceptionalSlot => {
                self.record(json, Rule::ModifiedExceptionalSlot)
            }
            crate::modified::Verdict::NoInsertedSecond => {
                self.record(json, Rule::ModifiedLeapSecond)
            }
            crate::modified::Verdict::KnowledgeUnavailable => {
                self.result.modified.unknown = true;
                self.record(json, Rule::ModifiedKnowledgeUnavailable);
            }
        }
        true
    }
    fn local_replay_proof(&mut self, json: CheckView<'a, 'view>, role: Role) -> bool {
        let CheckView::Effective { model, node } = json else {
            return false;
        };
        let n = &model.nodes[node];
        if n.changed_context
            || n.source.source_id() != model.source
            || n.inherited_context
            || !model.clear_feature
            || n.original_role != Some(role)
            || n.source.kind() != JsonKind::Map
            || model
                .evaluated_roles
                .get(n.source.node)
                .is_none_or(|mask| mask & role.mask() == 0)
        {
            return false;
        }
        let mut original = n.source.children();
        let mut effective = json.children();
        loop {
            let (source, current) = match (original.next(), effective.next()) {
                (None, None) => return true,
                (Some(source), Some(current)) => (source, current),
                _ => return false,
            };
            let Some(key) = source.n().key.as_deref() else {
                return false;
            };
            if !self.debit(
                current,
                key.len() + current.member_name().present().map_or(0, str::len) + 1,
            ) {
                return false;
            }
            if !matches!(current.member_name(),Field::Present(name)if name==key)
                || source.kind() != current.kind()
            {
                return false;
            }
            if !matches!(source.kind(), JsonKind::Map | JsonKind::Array) {
                let original = current.source();
                if original.source_id() != source.source_id()
                    || original.node != source.node
                    || !source.n().capable
                {
                    return false;
                }
            }
            if class(key).is_some_and(|class| role.allows(class))
                || matches!(key, "properties" | "sdfChoice")
            {
                let mut old = source.children();
                let mut derived = current.children();
                loop {
                    let (old, current) = match (old.next(), derived.next()) {
                        (None, None) => break,
                        (Some(old), Some(current)) => (old, current),
                        _ => return false,
                    };
                    let Some(name) = old.n().key.as_deref() else {
                        return false;
                    };
                    if !self.debit(
                        current,
                        name.len() + current.member_name().present().map_or(0, str::len) + 1,
                    ) {
                        return false;
                    }
                    if current.unavailable()
                        || !matches!(current.member_name(),Field::Present(value)if value==name)
                        || current.kind() != old.kind()
                    {
                        return false;
                    }
                }
            }
            if key == "namespace" || key == "info" {
                let CheckView::Effective { model, node } = current else {
                    unreachable!()
                };
                if model.nodes[node].changed_context
                    || model.nodes[node].source.source_id() != source.source_id()
                    || model.nodes[node].source.node != source.node
                {
                    return false;
                }
            }
        }
    }
    fn schedule(&mut self, json: CheckView<'a, 'view>, role: Role, patch: bool) -> bool {
        if !self.debit(json, 1) {
            return false;
        }
        if let CheckView::Source(source) = json {
            // Original source depth is admitted before this traversal. Derived
            // nesting uses the explicit work queue below.
            if !self.visit(json, role, patch) {
                return false;
            }
            if source.kind() == JsonKind::Map
                && let Some(mask) = self.roles.get_mut(source.node)
            {
                *mask |= role.mask();
            }
            return true;
        }
        if crate::allowance::push(
            &mut self.jobs,
            (json, role, patch),
            &mut self.budget.allocation,
        )
        .is_err()
        {
            if self.number_work.is_some() {
                self.result.advisory.unknown = true;
                self.result.numeric.unknown |= self.numeric_enabled;
            }
            self.record(json, Rule::WorkLimit);
            return false;
        }
        true
    }
    fn drain(&mut self) -> bool {
        while let Some((json, role, patch)) = self.jobs.pop() {
            if !self.visit(json, role, patch) {
                self.jobs.clear();
                return false;
            }
            if let CheckView::Source(json) = json
                && json.kind() == JsonKind::Map
                && let Some(mask) = self.roles.get_mut(json.node)
            {
                *mask |= role.mask();
            }
        }
        true
    }
    fn visit(&mut self, json: CheckView<'a, 'view>, role: Role, mut possible_patch: bool) -> bool {
        if self.consistency_enabled && role.full_data() {
            self.check_consistency(json);
        }
        if json.unavailable() {
            self.coverage_unknown(role);
            if self.consistency_enabled && role == Role::DirectItem {
                self.defer_consistency();
            }
        } else {
            self.coverage_role(role);
        }
        if self.result.pattern.enabled
            && role != Role::Information
            && matches!(json, CheckView::Effective {model,node} if model.nodes[node].unavailable.is_some() || model.nodes[node].meaning_unavailable.is_some())
        {
            self.pattern_unavailable(json);
        }
        if self.sample
            && self.sample_rules.common
            && !json.unavailable()
            && json.kind() != JsonKind::Map
        {
            self.record(json, Rule::DefinitionShape);
        }
        if self.number_work.is_some()
            && let CheckView::Effective { model, node } = json
        {
            if self.numeric_enabled
                && role != Role::Information
                && (model.nodes[node].unavailable.is_some()
                    || model.nodes[node].donor_unavailable.is_some()
                    || model.nodes[node].meaning_unavailable.is_some())
            {
                self.result.numeric.unknown = true;
            }
            if json.unavailable() && role != Role::Information {
                self.result.advisory.unknown = true;
                self.result.numeric.unknown |= self.numeric_enabled;
            } else if role.full_data() && self.number_work.is_some() {
                let evaluation = crate::proof::evaluate(
                    model,
                    node,
                    self.number_work
                        .as_mut()
                        .expect("effective numeric counter"),
                    &mut self.budget.work,
                    self.alternative,
                );
                self.result.advisory.applicable |= evaluation.scope.applicable;
                self.result.advisory.unknown |= evaluation.scope.unknown;
                for (rule, witness) in evaluation.findings.into_iter().flatten() {
                    self.record_proof(json, rule, witness);
                }
                if self.numeric_enabled {
                    let evaluation = crate::proof::evaluate_numeric(
                        model,
                        node,
                        self.number_work
                            .as_mut()
                            .expect("shared effective numeric counter"),
                        &mut self.budget.work,
                        &mut self.budget.allocation,
                        self.alternative,
                    );
                    self.result.numeric.applicable |= evaluation.scope.applicable;
                    self.result.numeric.unknown |= evaluation.scope.unknown;
                    if evaluation.scope.applicable {
                        self.budget.set_aside |= evaluation.set_aside;
                    }
                    for (rule, witness) in evaluation.findings.into_iter().flatten() {
                        self.record_proof(json, rule, witness);
                    }
                }
            }
        }
        if json.unavailable() || json.kind() != JsonKind::Map {
            return true;
        }
        self.skip_local = self.local_replay_proof(json, role);
        let mut object = false;
        let mut namespace = Field::Absent;
        let mut unknown_keys = false;
        let mut enumeration = None;
        let mut choice = None;
        let mut has_definitions = false;
        let skip_map = self.skip_local;
        for child in json.children() {
            if !self.debit(
                child,
                child.member_name().present().map_or(1, |key| key.len() + 1),
            ) {
                return false;
            }
            let Some(key) = child.member_name().present() else {
                unknown_keys = true;
                continue;
            };
            if key == "type" {
                object = matches!(child.text(), Field::Present("object"));
            }
            if key == "namespace" {
                namespace = Field::Present(child);
            }
            if key == "enum" {
                enumeration = Some(child);
            }
            if key == "sdfChoice" {
                choice = Some(child);
            }
            if self.result.coverage.full_requested && role == Role::Document && class(key).is_some()
            {
                if !self.debit(child, 1) {
                    return false;
                }
                has_definitions |= child.children().next().is_some();
            }
            // This records possible merge/deletion uncertainty, not a valid or resolved reference.
            if json.effective().is_none() && role.reference_context() && key == "sdfRef" {
                possible_patch = true;
            }
        }
        if role == Role::Document && json.effective().is_none() {
            self.result.coverage.observe(
                Requirement::M008,
                if has_definitions {
                    Assessed
                } else {
                    Undetermined
                },
            );
        }
        if unknown_keys {
            self.coverage_unknown(role);
            self.result.numeric.unknown |= self.numeric_enabled && role != Role::Information;
            if self.result.pattern.enabled
                && json.effective().is_some()
                && role != Role::Information
            {
                self.pattern_unavailable(json);
            }
            if matches!(namespace, Field::Absent) {
                namespace = Field::Unavailable("map member identity unavailable");
            }
            if json.effective().is_none() && role.reference_context() {
                possible_patch = true;
            }
        }
        if !self.coverage_meaning(json, role, object) {
            return false;
        }
        if self.number_work.is_some()
            && role.data_description_role()
            && let CheckView::Effective { model, node } = json
            && (unknown_keys
                || (enumeration.is_some() || choice.is_some())
                    && model.alternative_scope_unknown(node))
        {
            // Direct items are excluded from proofs, but their Full alternatives still need established traversal.
            self.result.advisory.unknown = true;
            self.result.numeric.unknown |= self.numeric_enabled;
            self.defer_consistency();
            if self.result.pattern.enabled {
                self.pattern_unavailable(json);
            }
        }
        let sample_rules = if self.sample {
            match json {
                CheckView::Effective { model, node } => crate::sample::parameter_rules(
                    crate::EffectiveJson { model, node },
                    self.sample_rules,
                    &mut self.budget.work,
                ),
                _ => self.sample_rules,
            }
        } else {
            self.sample_rules
        };
        // This relationship requires the complete parent-map presence evidence,
        // not a replay proof for either quality alone. Patch nulls may delete it.
        if role.data_description_role()
            && !self.sample
            && !skip_map
            && !unknown_keys
            && let (Some(enumeration), Some(choice)) = (enumeration, choice)
            && !(possible_patch
                && (enumeration.kind() == JsonKind::Null || choice.kind() == JsonKind::Null))
        {
            self.record(enumeration, Rule::DataEnumChoice);
        }
        for child in json.children() {
            if !self.debit(
                child,
                child.member_name().present().map_or(1, |key| key.len() + 1),
            ) {
                return false;
            }
            let Some(key) = child.member_name().present() else {
                continue;
            };
            if self.result.pattern.enabled
                && child.effective().is_some()
                && role.full_data()
                && key == "pattern"
            {
                let established = !unknown_keys
                    && !matches!(child, CheckView::Effective { model, node } if model.nodes[node].donor_unavailable.is_some() || model.nodes[node].meaning_unavailable.is_some());
                self.check_pattern(child, established);
            }
            if matches!(child, CheckView::Effective { model, node } if model.nodes[node].donor_unavailable.is_some())
            {
                self.coverage_unknown(role);
                if role.full_data() && matches!(key, "items" | "properties" | "sdfChoice")
                    || role == Role::DirectItem && key == "sdfChoice"
                    || class(key).is_some_and(|class| role.allows(class))
                {
                    self.result.advisory.unknown = true;
                    self.result.numeric.unknown |= self.numeric_enabled;
                    self.defer_consistency();
                    if self.result.pattern.enabled {
                        self.pattern_unavailable(child);
                    }
                }
                continue;
            }
            if self.sample && !crate::sample::relevant_quality(key) {
                continue;
            }
            let reliable_quality_member =
                // The selected sample supplies which shared parameter rules are needed.
                !unknown_keys && !(possible_patch && child.kind() == JsonKind::Null);
            if reliable_quality_member {
                self.coverage_field(child, key, role, object);
                self.formal_field(json, child, key, role, object);
            }
            if self.sample
                && (key == "nullable" && !sample_rules.nullable
                    || role.numeric_parameter(key) && !sample_rules.numeric
                    || matches!(key, "minLength" | "maxLength" | "pattern") && !sample_rules.text
                    || matches!(key, "properties" | "required") && !sample_rules.map
                    || matches!(key, "items" | "minItems" | "maxItems" | "uniqueItems")
                        && !sample_rules.array
                    || !matches!(
                        key,
                        "nullable"
                            | "minimum"
                            | "maximum"
                            | "exclusiveMinimum"
                            | "exclusiveMaximum"
                            | "multipleOf"
                            | "minLength"
                            | "maxLength"
                            | "pattern"
                            | "properties"
                            | "required"
                            | "items"
                            | "minItems"
                            | "maxItems"
                            | "uniqueItems"
                    ) && !sample_rules.common)
            {
                continue;
            }
            if self.sample
                && sample_rules.text
                && reliable_quality_member
                && role.full_data()
                && key == "pattern"
            {
                self.check_pattern(child, true);
            }
            self.skip_local =
                skip_map || reliable_quality_member && self.quality_replay_proof(json, child, role);
            if !self.skip_local
                && reliable_quality_member
                && !self.vocabulary(child, key, role, object)
            {
                return false;
            }
            // Only a known vocabulary map can establish that a non-null patch member survives as a quality.
            if !self.skip_local && reliable_quality_member && key == "sdfRequiredInputData" {
                self.record(child, Rule::HistoricalActionInput);
            }
            let information_shape_proved = matches!(role, Role::Information)
                && match json {
                    CheckView::Source(_) => self.information_strings,
                    CheckView::Effective { model, node } => {
                        !model.nodes[node].changed_context
                            && model.nodes[node].source.source_id() == model.source
                            && model.clear_feature
                            && model.nodes[node].original_role == Some(Role::Information)
                            && model
                                .evaluated_roles
                                .get(model.nodes[node].source.node)
                                .is_some_and(|mask| mask & (1 << 10) != 0)
                    }
                };
            if !self.skip_local
                && !information_shape_proved
                && reliable_quality_member
                && role.metadata(key)
                && child.kind() != JsonKind::String
            {
                self.record(child, Rule::MetadataString);
            }
            if let Some(class) = class(key) {
                if role.allows(class) {
                    if !self.direct
                        && !self.named(
                            child,
                            Role::Definition(class),
                            possible_patch,
                            true,
                            self.skip_local,
                        )
                    {
                        return false;
                    }
                } else if !self.skip_local && !(possible_patch && child.kind() == JsonKind::Null) {
                    self.record(child, Rule::RoleUnavailable);
                }
                if reliable_quality_member {
                    self.mark_quality(child, role);
                }
                continue;
            }
            if matches!(role, Role::Document) {
                if !self.document_field(child, key, namespace) {
                    return false;
                }
            } else if !self.skip_local
                && matches!(key, "info" | "namespace" | "defaultNamespace")
                && !(possible_patch && child.kind() == JsonKind::Null)
            {
                self.record(child, Rule::RoleUnavailable);
            }
            if matches!(role, Role::Definition(Class::Thing | Class::Object))
                && matches!(key, "minItems" | "maxItems")
                && !self.skip_local
            {
                self.grouping_count(child, possible_patch);
            }
            if role.map_parameter(key, object)
                && reliable_quality_member
                && !self.skip_local
                && child.kind() != JsonKind::Map
            {
                self.record(child, Rule::DataMap);
            }
            if role.data_description_role()
                && (key == "enum" || object && key == "required")
                && reliable_quality_member
                && !self.skip_local
                && !self.nonempty_string_array(
                    child,
                    if key == "enum" {
                        Rule::DataEnum
                    } else {
                        Rule::DataRequired
                    },
                )
            {
                return false;
            }
            if role.text_parameter(key)
                && reliable_quality_member
                && !self.skip_local
                && child.kind() != JsonKind::String
            {
                self.record(child, Rule::DataString);
            }
            if (role.full_data() && matches!(key, "nullable" | "uniqueItems")
                || matches!(role, Role::Definition(Class::Property))
                    && matches!(key, "readable" | "writable" | "observable"))
                && reliable_quality_member
                && !self.skip_local
                && child.kind() != JsonKind::Boolean
            {
                self.record(child, Rule::DataBoolean);
            }
            if role.numeric_parameter(key)
                && reliable_quality_member
                && !self.skip_local
                && child.kind() != JsonKind::Number
            {
                self.record(child, Rule::DataNumber);
            }
            if role.numeric_parameter(key)
                && reliable_quality_member
                && !self.skip_local
                && child.kind() == JsonKind::Number
                && !matches!(child.number(), Field::Present(_))
            {
                self.record(child, Rule::DataNumberUnavailable);
            }
            if role.count_parameter(key) && reliable_quality_member && !self.skip_local {
                match child.number() {
                    Field::Present(number)
                        if number.is_integer() && number.sign() != std::cmp::Ordering::Less => {}
                    Field::Present(_) => self.record(child, Rule::DataCount),
                    _ if child.kind() == JsonKind::Number => {
                        self.record(child, Rule::DataNumberUnavailable)
                    }
                    _ => self.record(child, Rule::DataCount),
                }
            }
            if role.full_data()
                && key == "multipleOf"
                && reliable_quality_member
                && !self.skip_local
                && let Field::Present(number) = child.number()
                && number.sign() != std::cmp::Ordering::Greater
            {
                self.record(child, Rule::PositiveStep);
            }
            let next = match (role, key) {
                (Role::Definition(Class::Action), "sdfInputData" | "sdfOutputData")
                | (Role::Definition(Class::Event), "sdfOutputData") => {
                    if !self.skip_local
                        && !(possible_patch && child.kind() == JsonKind::Null)
                        && child.kind() != JsonKind::Map
                    {
                        self.record(child, Rule::DataShape);
                    }
                    Some(Role::Data)
                }
                (r, "items") if r.full_data() => Some(Role::DirectItem),
                _ => None,
            };
            if self.direct {
                if reliable_quality_member {
                    self.mark_quality(child, role);
                }
                continue;
            }
            if let Some(next) = next {
                if !self.schedule(child, next, possible_patch) {
                    return false;
                }
            } else if key == "sdfChoice"
                && matches!(json, CheckView::Effective { model, node } if model.nodes[node].alternative_group.is_some())
            {
                // Final inherited alternatives use the same evaluator below, rather than the raw branch context.
            } else if ((role.full_data() || matches!(role, Role::DirectItem)) && key == "sdfChoice"
                || (role.full_data() || matches!(role, Role::DirectItem) && object)
                    && key == "properties")
                && !self.named(child, Role::Data, possible_patch, false, self.skip_local)
            {
                return false;
            } else if !self.skip_local
                && matches!(key, "sdfInputData" | "sdfOutputData")
                && !(possible_patch && child.kind() == JsonKind::Null)
            {
                self.record(child, Rule::RoleUnavailable);
            }
            if reliable_quality_member {
                self.mark_quality(child, role);
            }
        }
        true
    }
    fn nonempty_string_array(&mut self, json: CheckView<'a, 'view>, rule: Rule) -> bool {
        if json.kind() != JsonKind::Array || json.children().next().is_none() {
            self.record(json, rule);
        } else {
            for value in json.children() {
                if !self.debit(value, 1) {
                    return false;
                }
                if value.kind() != JsonKind::String {
                    self.record(json, rule);
                    break;
                }
            }
        }
        true
    }
    fn mark_quality(&mut self, json: CheckView<'a, 'view>, role: Role) {
        if let CheckView::Source(source) = json
            && let Some(mask) = self.roles.get_mut(source.node)
        {
            *mask |= role.mask() << 20;
        }
    }
    fn quality_replay_proof(
        &mut self,
        parent: CheckView<'a, 'view>,
        json: CheckView<'a, 'view>,
        role: Role,
    ) -> bool {
        let (
            CheckView::Effective {
                model,
                node: parent,
            },
            CheckView::Effective { node, .. },
        ) = (parent, json)
        else {
            return false;
        };
        let p = &model.nodes[parent];
        let n = &model.nodes[node];
        if p.changed_context
            || p.source.source_id() != model.source
            || n.source.source_id() != model.source
            || n.changed_context
            || n.unavailable.is_some()
            || !model.clear_feature
            || p.original_role != Some(role)
            || model
                .evaluated_roles
                .get(n.source.node)
                .is_none_or(|mask| mask & (role.mask() << 20) == 0)
            || n.source.source_id() != p.source.source_id()
            || n.source.n().parent != Some(p.source.node)
        {
            return false;
        }
        let Field::Present(name) = n.source.member_name() else {
            return false;
        };
        if p.inherited_context && matches!(name, "required" | "properties") {
            // Shared inheritance can change the containing object-type premise.
            return false;
        }
        if !self.debit(json, name.len() + 1)
            || !matches!(json.member_name(),Field::Present(current)if current==name)
        {
            return false;
        }
        if !matches!(json.kind(), JsonKind::Map | JsonKind::Array) {
            return n.source.n().capable;
        }
        let mut source = n.source.children();
        let mut effective = json.children();
        loop {
            let (original, current) = match (source.next(), effective.next()) {
                (None, None) => return true,
                (Some(original), Some(current)) => (original, current),
                _ => return false,
            };
            if !self.debit(
                current,
                original.n().key.as_deref().map_or(1, |key| key.len() + 1),
            ) {
                return false;
            }
            let CheckView::Effective { node, .. } = current else {
                unreachable!()
            };
            let child = &model.nodes[node];
            if child.changed_context
                || child.unavailable.is_some()
                || child.source.source_id() != original.source_id()
                || child.source.node != original.node
                || child.key != original.n().key.as_deref()
                || child.index != original.n().index
            {
                return false;
            }
        }
    }
    fn vocabulary(
        &mut self,
        json: CheckView<'a, 'view>,
        key: &str,
        role: Role,
        object: bool,
    ) -> bool {
        // Charge the additional name scan and qualification scan before inspecting bytes.
        if !self.debit(json, key.len() * 2 + 1) {
            return false;
        }
        if self.result.coverage.full_requested {
            // Full additionally establishes lexical admission, before vocabulary meaning.
            if !self.debit(json, key.len() * 3 + 1) {
                return false;
            }
            if !unqualified_quality(key) && !qualified_quality(key) {
                self.record(json, Rule::QualityNameSyntax);
                return true;
            }
        }
        if key == "sdfRequiredInputData" {
            return true;
        }
        match role.vocabulary(key, object) {
            Vocabulary::Supported => {
                if matches!(key, "type" | "format" | "sdfType")
                    && let Field::Present(value) = json.text()
                {
                    if !self.debit(json, value.len() * 8 + 1) {
                        return false;
                    }
                    let understood = match key {
                        "type" => {
                            matches!(
                                value,
                                "number" | "integer" | "string" | "boolean" | "object"
                            ) || value == "array" && !matches!(role, Role::DirectItem)
                        }
                        "format" => matches!(
                            value,
                            "date-time" | "date" | "time" | "uri" | "uri-reference" | "uuid"
                        ),
                        "sdfType" => matches!(value, "byte-string" | "unix-time"),
                        _ => unreachable!(),
                    };
                    if !understood {
                        self.optional_vocabulary(json);
                    }
                }
            }
            Vocabulary::OutsideRole => self.optional_vocabulary(json),
            Vocabulary::Qualified if qualified_quality(key) => self.optional_vocabulary(json),
            Vocabulary::Qualified => {}
            Vocabulary::Unqualified => self.record(json, Rule::QualityUnfamiliar),
            // Full consults the recorded meaning, so only other assessments count it.
            Vocabulary::Recorded => {
                self.optional += u32::from(!self.result.coverage.full_requested)
            }
        }
        true
    }
    fn optional_vocabulary(&mut self, json: CheckView<'a, 'view>) {
        if self.result.coverage.full_requested {
            self.record(json, Rule::LocatedOptionalVocabulary);
        } else {
            self.optional += 1;
        }
    }
    fn document_field(
        &mut self,
        json: CheckView<'a, 'view>,
        key: &str,
        namespace: Field<CheckView<'a, 'view>>,
    ) -> bool {
        match key {
            "info" if json.kind() != JsonKind::Map => self.record(json, Rule::BlockShape),
            "info" => return self.schedule(json, Role::Information, false),
            "namespace" if self.skip_local => {}
            "defaultNamespace" if self.skip_local => {}
            "namespace" => {
                if json.kind() != JsonKind::Map {
                    self.record(json, Rule::BlockShape);
                } else {
                    for mapping in json.children() {
                        if !self.debit(
                            mapping,
                            mapping
                                .member_name()
                                .present()
                                .map_or(1, |name| name.len() + 1),
                        ) {
                            return false;
                        }
                        if let Field::Present(name) = mapping.member_name()
                            && name.contains(':')
                        {
                            self.record(mapping, Rule::NameColon);
                        }
                        if mapping.kind() != JsonKind::String {
                            self.record(mapping, Rule::NamespaceUri);
                        } else if let Field::Present(uri) = mapping.text() {
                            // Bound the complete scanner, including authority and IP-literal parsing, before work.
                            if !self.debit(mapping, uri.len() * 8 + 1) {
                                return false;
                            }
                            if !crate::uri::valid_uri(uri) {
                                self.record(mapping, Rule::NamespaceUri);
                            }
                        }
                    }
                }
            }
            "defaultNamespace" => {
                if json.kind() != JsonKind::String {
                    self.record(json, Rule::NamespaceDefault);
                } else if let Field::Present(prefix) = json.text() {
                    match namespace {
                        Field::Absent => self.record(json, Rule::NamespaceDefault),
                        Field::Present(map) if map.kind() == JsonKind::Map => {
                            let mut mapped = false;
                            let mut unknown = false;
                            for entry in map.children() {
                                if !self.debit(entry, 1) {
                                    return false;
                                }
                                if let Field::Present(name) = entry.member_name() {
                                    if !self.debit(json, name.len() + prefix.len() + 1) {
                                        return false;
                                    }
                                    mapped |= name == prefix;
                                } else {
                                    unknown = true;
                                }
                            }
                            if !mapped && !unknown {
                                self.record(json, Rule::NamespaceDefault);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        true
    }
    fn grouping_count(&mut self, json: CheckView<'a, 'view>, possible_patch: bool) {
        if possible_patch && json.kind() == JsonKind::Null {
            return;
        }
        if self.result.coverage.full_requested {
            // The no-cut grouping entries admit extension fallback in Full.
            // Its final effective membership and meaning are assessed together.
            return;
        }
        match json.number() {
            Field::Present(number)
                if number.is_integer() && number.sign() != std::cmp::Ordering::Less => {}
            Field::Present(_) => self.record(json, Rule::GroupingCount),
            _ if json.kind() == JsonKind::Number => self.record(json, Rule::NumberUnavailable),
            _ => self.record(json, Rule::GroupingCount),
        }
    }
    fn named(
        &mut self,
        group: CheckView<'a, 'view>,
        role: Role,
        possible_patch: bool,
        class_group: bool,
        skip_local: bool,
    ) -> bool {
        if let CheckView::Effective { model, node } = group
            && (model.nodes[node].unavailable.is_some()
                || model.nodes[node].donor_unavailable.is_some()
                || model.nodes[node].meaning_unavailable.is_some())
        {
            self.coverage_unknown(role);
            self.result.advisory.unknown = true;
            self.result.numeric.unknown |= self.numeric_enabled;
            self.defer_consistency();
            if self.result.pattern.enabled {
                if !self.debit(group, 1) {
                    self.result.pattern.unknown = true;
                    return false;
                }
                self.pattern_unavailable(group);
            }
            return true;
        }
        if group.kind() != JsonKind::Map {
            if !skip_local && class_group && !(possible_patch && group.kind() == JsonKind::Null) {
                self.record(group, Rule::GroupShape);
            }
            return true;
        }
        for definition in group.children() {
            if definition.unavailable() {
                self.coverage_unknown(role);
                self.defer_consistency();
                if self.result.pattern.enabled {
                    if !self.debit(definition, 1) {
                        self.result.pattern.unknown = true;
                        return false;
                    }
                    self.pattern_unavailable(definition);
                }
                if self.number_work.is_some() {
                    self.result.advisory.unknown = true;
                    self.result.numeric.unknown |= self.numeric_enabled;
                }
                continue;
            }
            // A null nested patch entry is a deletion selector, not a surviving Given Name.
            if possible_patch && definition.kind() == JsonKind::Null {
                if !self.debit(definition, 1) {
                    return false;
                }
                continue;
            }
            if !self.debit(
                definition,
                definition
                    .member_name()
                    .present()
                    .map_or(1, |name| name.len() + 1),
            ) {
                return false;
            }
            let skip_entry = skip_local || self.named_replay_proof(definition, role);
            if !skip_entry
                && let Field::Present(name) = definition.member_name()
                && name.contains(':')
            {
                self.record(definition, Rule::NameColon);
            }
            if !skip_entry && class_group && definition.kind() != JsonKind::Map {
                self.record(definition, Rule::DefinitionShape);
            }
            if !skip_entry && !class_group && role.full_data() && definition.kind() != JsonKind::Map
            {
                self.record(definition, Rule::DataMap);
            }
            if let CheckView::Source(source) = definition
                && let Some(mask) = self.roles.get_mut(source.node)
            {
                *mask |= role.mask() << 10;
            }
            if !self.schedule(definition, role, possible_patch) {
                return false;
            }
        }
        true
    }
    fn named_replay_proof(&mut self, json: CheckView<'a, 'view>, role: Role) -> bool {
        let CheckView::Effective { model, node } = json else {
            return false;
        };
        let n = &model.nodes[node];
        if n.changed_context
            || n.source.source_id() != model.source
            || !model.clear_feature
            || n.original_role != Some(role)
            || model
                .evaluated_roles
                .get(n.source.node)
                .is_none_or(|mask| mask & (role.mask() << 10) == 0)
        {
            return false;
        }
        let Field::Present(original_name) = n.source.member_name() else {
            return false;
        };
        if !self.debit(json, original_name.len() + 1) {
            return false;
        }
        matches!(json.member_name(),Field::Present(name)if name==original_name)
            && json.kind() == n.source.kind()
    }
    fn record(&mut self, json: impl Into<CheckView<'a, 'view>>, rule: Rule) {
        self.record_with_witness(json.into(), rule, None);
    }
    fn pattern_unavailable(&mut self, json: CheckView<'a, 'view>) {
        self.result.pattern.unknown = true;
        self.record(json, Rule::Pattern(crate::pattern::Condition::Unavailable));
    }
    fn check_pattern(&mut self, json: CheckView<'a, 'view>, established: bool) {
        if !established || json.unavailable() {
            self.pattern_unavailable(json);
            return;
        }
        if json.kind() != JsonKind::String {
            return;
        }
        self.result.pattern.applicable = true;
        let Field::Present(text) = json.text() else {
            self.pattern_unavailable(json);
            return;
        };
        let verdict = crate::pattern::evaluate(text, &self.catalogue.limits, self.budget);
        let condition = match verdict {
            crate::pattern::Verdict::Valid => return,
            crate::pattern::Verdict::Invalid => crate::pattern::Condition::Invalid,
            crate::pattern::Verdict::Unsupported => {
                self.result.pattern.unknown = true;
                crate::pattern::Condition::Unsupported
            }
            crate::pattern::Verdict::Limit => {
                self.result.pattern.unknown = true;
                crate::pattern::Condition::Limit
            }
        };
        self.record(json, Rule::Pattern(condition));
    }
    fn record_proof(
        &mut self,
        json: CheckView<'a, 'view>,
        rule: Rule,
        witness: crate::proof::Witness<'a>,
    ) {
        self.record_with_witness(json, rule, Some(witness));
    }
    fn record_with_witness(
        &mut self,
        json: CheckView<'a, 'view>,
        rule: Rule,
        proof: Option<crate::proof::Witness<'a>>,
    ) {
        let owner = self.coverage_record(json, rule);
        let result = &mut self.result;
        result.work_limited |= matches!(rule, Rule::WorkLimit);
        result.total += 1;
        result.errors += u32::from(rule.severity() == "error");
        result.warnings += u32::from(rule.severity() == "warning");
        result.notices += u32::from(rule.severity() == "notice");
        if result.findings.len() >= self.catalogue.limits.conditions {
            result.count_omitted = true;
            return;
        }
        if result.findings.len() == result.findings.capacity() {
            let old = result.findings.capacity();
            let new = (old.max(4) * 2).min(self.catalogue.limits.conditions);
            if matches!(
                rule,
                Rule::Pattern(_)
                    | Rule::PatternMeaning(_)
                    | Rule::FormalUint
                    | Rule::FormalUnavailable(_)
            ) && !new
                .checked_mul(std::mem::size_of::<Finding<'_>>())
                .is_some_and(|bytes| charge_exact(&mut self.budget.allocation, bytes))
            {
                result.bytes_omitted = true;
                return;
            }
            if !charge_exact(
                self.storage,
                (new - old) * std::mem::size_of::<Finding<'_>>(),
            ) {
                result.bytes_omitted = true;
                return;
            }
            // Admit the complete requested capacity, including the first allocation, before growing.
            result.findings.reserve_exact(new - old);
        }
        result.findings.push(Finding {
            json: json.source(),
            effective: json.effective(),
            rule,
            proof,
            replacement: None,
            coverage_requirement: owner,
        });
    }
}
#[derive(Clone, Copy)]
pub(crate) struct SampleRules {
    pub(crate) common: bool,
    pub(crate) array: bool,
    pub(crate) map: bool,
    pub(crate) numeric: bool,
    pub(crate) nullable: bool,
    pub(crate) text: bool,
}
impl Default for SampleRules {
    fn default() -> Self {
        Self {
            common: true,
            array: false,
            map: false,
            numeric: false,
            nullable: false,
            text: false,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum FeatureContext<'a> {
    Clear(Option<JsonView<'a>>),
    Deferred(JsonView<'a>),
    Limited,
}
/// Charge only when the whole cost fits, leaving the counter untouched otherwise.
/// Unlike `allowance::charge`, a refused charge does not drain the counter, so a
/// later cheaper charge can still succeed.
fn charge_exact(counter: &mut usize, cost: usize) -> bool {
    match counter.checked_sub(cost) {
        Some(left) => {
            *counter = left;
            true
        }
        None => false,
    }
}

fn qualified_quality(key: &str) -> bool {
    let Some((prefix, quality)) = key.split_once(':') else {
        return false;
    };
    prefix
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_lowercase)
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && unqualified_quality(quality)
}
fn unqualified_quality(key: &str) -> bool {
    key.as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase() || *byte == b'$')
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'$')
}

impl<'source, 'view> From<JsonView<'source>> for CheckView<'source, 'view> {
    fn from(json: JsonView<'source>) -> Self {
        Self::Source(json)
    }
}
