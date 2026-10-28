//! Vocabulary roles of document locations, deciding which qualities a key may carry there
//! and how each Quality Name affects base meaning.
//! Follows ADR 0003: Check findings and completeness,
//! ADR 0008: Reject the historical Action input quality,
//! ADR 0037: Undeclared extension vocabulary, and
//! ADR 0038: Recorded extension vocabulary.
use crate::Class;

/// How a Quality Name at a role affects the base meaning of its definition.
/// Base-meaning consumers ask this one question instead of testing names.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Vocabulary {
    /// A base quality the role supports, carrying base meaning there.
    Supported,
    /// A base Quality Name the role does not support, such as a wider Direct item quality.
    OutsideRole,
    /// A qualified Quality Name outside base SDF, such as `acme:note`.
    Qualified,
    /// An unqualified name outside base SDF, such as `sdfProtocolMap` or a misspelled base quality.
    Unqualified,
    /// Recorded extension vocabulary at a position its registration names, such as `oma:id` on
    /// an sdfObject definition. Its value domain comes from recorded registry evidence.
    Recorded,
}
impl Vocabulary {
    /// Whether the name is undeclared extension vocabulary: outside base SDF, not recorded at
    /// this position, and so without supported meaning of its own. Names in a document that
    /// declares an Extension feature never reach this question, because that whole document
    /// is deferred.
    pub(crate) fn undeclared(self) -> bool {
        matches!(self, Self::Qualified | Self::Unqualified)
    }
    /// Whether base conclusions about the definition proceed as if the quality were absent.
    /// Undeclared extension vocabulary is ignorable, and so is recorded extension vocabulary
    /// because its recorded meaning concerns only its own value. A base quality outside its
    /// role keeps stopping base meaning, because it is not extension vocabulary.
    pub(crate) fn ignorable(self) -> bool {
        match self {
            Self::Qualified | Self::Unqualified | Self::Recorded => true,
            Self::Supported | Self::OutsideRole => false,
        }
    }
    /// Whether the quality stops a base-meaning conclusion about its definition.
    pub(crate) fn blocks_base_meaning(self) -> bool {
        self != Self::Supported && !self.ignorable()
    }
}

/// Extension vocabulary that a base-meaning conclusion set aside. A conclusion classifies
/// each quality through it and passes it on only when the conclusion holds, so a report
/// names `undeclared-extensions-ignorable/1` from ADR 0037: Undeclared extension vocabulary
/// exactly when a result relied on that reading.
#[derive(Clone, Copy, Default)]
pub(crate) struct SetAside {
    undeclared: bool,
    recorded: bool,
}
impl SetAside {
    /// Classifies a Quality Name at `role` and notes extension vocabulary.
    pub(crate) fn classify(&mut self, role: Role, key: &str, object: bool) -> Vocabulary {
        let vocabulary = role.vocabulary(key, object);
        self.undeclared |= vocabulary.undeclared();
        self.recorded |= vocabulary == Vocabulary::Recorded;
        vocabulary
    }
    /// Whether a result relied on reading the noted vocabulary as ignorable. Recorded
    /// extension vocabulary relies on it only when the report did not consult its recorded
    /// evidence, as in a Sample report, an assessment other than Full or a Full check that
    /// withholds recorded evidence, following ADR 0038: Recorded extension vocabulary.
    pub(crate) fn relies_on_reading(self, recorded_consulted: bool) -> bool {
        self.undeclared || self.recorded && !recorded_consulted
    }
}
impl std::ops::BitOrAssign for SetAside {
    fn bitor_assign(&mut self, other: Self) {
        self.undeclared |= other.undeclared;
        self.recorded |= other.recorded;
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Role {
    Document,
    Information,
    Definition(Class),
    Data,
    DirectItem,
}
impl Role {
    pub(crate) const INFORMATION_STRINGS: u64 = 1 << 11;
    pub(crate) const NAMED_SHIFT: u32 = 12;
    pub(crate) const QUALITY_SHIFT: u32 = 24;
    pub(crate) fn mask(self) -> u64 {
        1 << match self {
            Self::Document => 0,
            Self::Information => 1,
            Self::Definition(Class::Thing) => 2,
            Self::Definition(Class::Object) => 3,
            Self::Definition(Class::Property) => 4,
            Self::Definition(Class::Action) => 5,
            Self::Definition(Class::Event) => 6,
            Self::Definition(Class::Data) => 7,
            Self::Data => 8,
            Self::DirectItem => 9,
            Self::Definition(Class::Context) => 10,
        }
    }
    pub(crate) fn reference_context(self) -> bool {
        matches!(self, Self::Definition(_) | Self::Data | Self::DirectItem)
    }
    /// Classifies a Quality Name at this role. `object` says whether the description's
    /// `type` is `object`, which decides `required` and `properties` in Direct items.
    pub(crate) fn vocabulary(self, key: &str, object: bool) -> Vocabulary {
        if self.supports(key, object) {
            Vocabulary::Supported
        } else if known_quality(key) {
            Vocabulary::OutsideRole
        } else if key == "oma:id" && self.oma_id_position() {
            Vocabulary::Recorded
        } else if key.contains(':') {
            Vocabulary::Qualified
        } else {
            Vocabulary::Unqualified
        }
    }
    pub(crate) fn supports(self, key: &str, object: bool) -> bool {
        if let Some(class) = class(key) {
            return self.allows(class);
        }
        if self.metadata(key) {
            return true;
        }
        match self {
            Self::Document => matches!(key, "info" | "namespace" | "defaultNamespace"),
            Self::Information => key == "features",
            Self::Definition(Class::Thing | Class::Object) => {
                matches!(key, "sdfRef" | "sdfRequired" | "minItems" | "maxItems")
            }
            Self::Definition(Class::Action) => matches!(
                key,
                "sdfRef" | "sdfRequired" | "sdfInputData" | "sdfOutputData"
            ),
            Self::Definition(Class::Event) => {
                matches!(key, "sdfRef" | "sdfRequired" | "sdfOutputData")
            }
            Self::Definition(Class::Property | Class::Data | Class::Context) | Self::Data => {
                matches!(
                    key,
                    "sdfRef"
                        | "sdfRequired"
                        | "type"
                        | "sdfType"
                        | "sdfChoice"
                        | "enum"
                        | "const"
                        | "default"
                        | "minimum"
                        | "maximum"
                        | "exclusiveMinimum"
                        | "exclusiveMaximum"
                        | "multipleOf"
                        | "minLength"
                        | "maxLength"
                        | "pattern"
                        | "format"
                        | "minItems"
                        | "maxItems"
                        | "uniqueItems"
                        | "items"
                        | "required"
                        | "properties"
                        | "nullable"
                        | "contentFormat"
                        | "unit"
                ) || matches!(self, Self::Definition(Class::Property))
                    && matches!(key, "readable" | "writable" | "observable")
            }
            Self::DirectItem => {
                matches!(
                    key,
                    "sdfRef"
                        | "type"
                        | "sdfChoice"
                        | "enum"
                        | "minimum"
                        | "maximum"
                        | "format"
                        | "minLength"
                        | "maxLength"
                ) || object && matches!(key, "required" | "properties")
            }
        }
    }
    pub(crate) fn metadata(self, key: &str) -> bool {
        match self {
            Self::Information => matches!(
                key,
                "title"
                    | "description"
                    | "version"
                    | "modified"
                    | "copyright"
                    | "license"
                    | "$comment"
            ),
            Self::Definition(_) | Self::Data => matches!(key, "description" | "label" | "$comment"),
            Self::DirectItem => matches!(key, "description" | "$comment"),
            Self::Document => false,
        }
    }
    pub(crate) fn allows(self, class: Class) -> bool {
        match self {
            Self::Document => class != Class::Context,
            Self::Definition(parent) => parent.allows_child(class),
            _ => false,
        }
    }
    pub(crate) fn full_data(self) -> bool {
        matches!(
            self,
            Self::Data | Self::Definition(Class::Property | Class::Data | Class::Context)
        )
    }
    /// The positions of the registered `oma:id` CDDL sockets: sdfObject and sdfAction
    /// definitions and every full data description.
    fn oma_id_position(self) -> bool {
        matches!(self, Self::Definition(Class::Object | Class::Action)) || self.full_data()
    }
    pub(crate) fn numeric_parameter(self, key: &str) -> bool {
        self.full_data()
            && matches!(
                key,
                "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf"
            )
            || matches!(self, Self::DirectItem) && matches!(key, "minimum" | "maximum")
    }
    pub(crate) fn data_description_role(self) -> bool {
        self.full_data() || matches!(self, Self::DirectItem)
    }
    pub(crate) fn map_parameter(self, key: &str, object: bool) -> bool {
        self.full_data() && matches!(key, "items" | "sdfChoice" | "properties")
            || matches!(self, Self::DirectItem)
                && (key == "sdfChoice" || object && key == "properties")
    }
    pub(crate) fn text_parameter(self, key: &str) -> bool {
        self.full_data()
            && matches!(
                key,
                "type" | "sdfType" | "pattern" | "format" | "unit" | "contentFormat"
            )
            || matches!(self, Self::DirectItem) && matches!(key, "type" | "format")
    }
    pub(crate) fn count_parameter(self, key: &str) -> bool {
        self.full_data() && matches!(key, "minLength" | "maxLength" | "minItems" | "maxItems")
            || matches!(self, Self::DirectItem) && matches!(key, "minLength" | "maxLength")
    }
}
fn known_quality(key: &str) -> bool {
    // RFC 9880 Sections 3.1, 3.2 and 5, and Table 13; familiarity is not registry knowledge.
    matches!(
        key,
        "info"
            | "namespace"
            | "defaultNamespace"
            | "title"
            | "version"
            | "modified"
            | "copyright"
            | "license"
            | "features"
            | "sdfThing"
            | "sdfObject"
            | "sdfProperty"
            | "sdfAction"
            | "sdfEvent"
            | "sdfData"
            | "sdfContext"
            | "$comment"
            | "const"
            | "contentFormat"
            | "default"
            | "description"
            | "enum"
            | "exclusiveMaximum"
            | "exclusiveMinimum"
            | "format"
            | "items"
            | "label"
            | "maxItems"
            | "maxLength"
            | "maximum"
            | "minItems"
            | "minLength"
            | "minimum"
            | "multipleOf"
            | "nullable"
            | "observable"
            | "pattern"
            | "properties"
            | "readable"
            | "required"
            | "sdfChoice"
            | "sdfInputData"
            | "sdfOutputData"
            | "sdfRef"
            | "sdfRequired"
            | "sdfRequiredInputData"
            | "sdfType"
            | "type"
            | "uniqueItems"
            | "unit"
            | "writable"
    )
}
pub(crate) fn class(key: &str) -> Option<Class> {
    match key {
        "sdfThing" => Some(Class::Thing),
        "sdfObject" => Some(Class::Object),
        "sdfProperty" => Some(Class::Property),
        "sdfAction" => Some(Class::Action),
        "sdfEvent" => Some(Class::Event),
        "sdfData" => Some(Class::Data),
        "sdfContext" => Some(Class::Context),
        _ => None,
    }
}
