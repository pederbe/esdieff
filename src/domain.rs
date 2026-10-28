//! Borrowed source projections preserve authored absence and restricted data roles.
//! Follows ADR0010: Domain-oriented library and caller simplicity, and
//! ADR0013: Shared workflow and report guarantees.
use crate::{Children, ExactNumber, Field, JsonKind, JsonView, Source};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Class {
    Thing,
    Object,
    Property,
    Action,
    Event,
    Data,
    /// Non-affordance metadata from the recorded Context extension draft.
    Context,
}
impl Class {
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Thing => "sdfThing",
            Self::Object => "sdfObject",
            Self::Property => "sdfProperty",
            Self::Action => "sdfAction",
            Self::Event => "sdfEvent",
            Self::Data => "sdfData",
            Self::Context => "sdfContext",
        }
    }
    pub(crate) fn allows_child(self, class: Self) -> bool {
        match self {
            Self::Thing => true,
            Self::Object => matches!(
                class,
                Self::Property | Self::Action | Self::Event | Self::Data | Self::Context
            ),
            Self::Action | Self::Event => class == Self::Data,
            Self::Property | Self::Data | Self::Context => false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Lookup<T> {
    Found(T),
    NotFound,
    Ambiguous,
    Unavailable(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DataRole {
    Full,
    DirectItem,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BaseType {
    Number,
    Integer,
    String,
    Boolean,
    Array,
    Map,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DataType<'a> {
    Known(BaseType),
    Extended(&'a str),
    Undetermined(&'a str),
}

/// A Boolean Property or nullable characteristic, authored or supplied by its specified default.
/// Authored values include inherited values; contributor origins are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Characteristic {
    Declared(bool),
    SpecifiedDefault(bool),
}
impl Characteristic {
    pub fn value(self) -> bool {
        match self {
            Self::Declared(value) | Self::SpecifiedDefault(value) => value,
        }
    }
}

macro_rules! row_type {
    (source, $value:ty) => { $value };
    (effective, $value:ty) => { Field<$value> };
}
macro_rules! row_value {
    (source, $json:expr, $value:expr) => {
        $value
    };
    (effective, $json:expr, $value:expr) => {
        if let Some(reason) = $json.unavailable_reason() {
            Field::Unavailable(reason)
        } else if $json.kind() != JsonKind::Map {
            Field::Unavailable("definition or data-description map shape")
        } else {
            Field::Present($value)
        }
    };
}
// Both view families use the same class, data-role and scalar projection rules.
macro_rules! typed_access {
    ($JsonView:ident, $Children:ident, $effective:literal, $rows:ident) => {
        #[derive(Clone, Copy)]
        pub struct Definition<'a> {
            json: $JsonView<'a>,
            class: Class,
        }
        pub struct Definitions<'a> {
            children: $Children<'a>,
            class: Class,
            remaining: usize,
        }
        impl<'a> Iterator for Definitions<'a> {
            type Item = (&'a str, row_type!($rows, Definition<'a>));
            fn next(&mut self) -> Option<Self::Item> {
                let json = self.children.next()?;
                self.remaining -= 1;
                Some((
                    json.member_name().require().expect("validated member name"),
                    row_value!(
                        $rows,
                        json,
                        Definition {
                            json,
                            class: self.class,
                        }
                    ),
                ))
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.remaining, Some(self.remaining))
            }
        }
        impl ExactSizeIterator for Definitions<'_> {}
        fn definitions(json: $JsonView<'_>, class: Class) -> Field<Definitions<'_>> {
            let group = match json.get(class.keyword()) {
                Field::Present(value) => value,
                Field::Absent => return Field::Absent,
                Field::Unavailable(reason) => return Field::Unavailable(reason),
            };
            if group.kind() != JsonKind::Map {
                return Field::Unavailable("class group map shape");
            }
            let mut count = 0;
            for child in group.children() {
                if (!$effective && child.kind() != JsonKind::Map)
                    || !matches!(child.member_name(), Field::Present(_))
                {
                    return Field::Unavailable("definition map or name unavailable");
                }
                count += 1;
            }
            Field::Present(Definitions {
                children: group.children(),
                class,
                remaining: count,
            })
        }
        impl<'a> Definition<'a> {
            pub(crate) fn from_json(json: $JsonView<'a>, class: Class) -> Self {
                Self { json, class }
            }
            pub fn class(self) -> Class {
                self.class
            }
            pub fn json(self) -> $JsonView<'a> {
                self.json
            }
            pub fn description(self) -> Field<&'a str> {
                text_field(self.json, "description")
            }
            pub fn label(self) -> Field<&'a str> {
                text_field(self.json, "label")
            }
            pub fn children(self, class: Class) -> Field<Definitions<'a>> {
                if self.class.allows_child(class) {
                    definitions(self.json, class)
                } else {
                    match self.json.get(class.keyword()) {
                        Field::Absent => Field::Absent,
                        _ => Field::Unavailable("content has no established base class role here"),
                    }
                }
            }
            /// Authored grouping repetition, separate from array-data constraints and effective defaults.
            pub fn repetition_minimum(self) -> Field<ExactNumber<'a>> {
                self.repetition_count("minItems")
            }
            pub fn repetition_maximum(self) -> Field<ExactNumber<'a>> {
                self.repetition_count("maxItems")
            }
            fn repetition_count(self, key: &str) -> Field<ExactNumber<'a>> {
                if !matches!(self.class, Class::Thing | Class::Object) {
                    return Field::Absent;
                }
                match self.json.get(key) {
                    Field::Present(json) => json.number(),
                    Field::Absent => Field::Absent,
                    Field::Unavailable(reason) => Field::Unavailable(reason),
                }
            }
            pub fn data_description(self) -> Field<DataDescription<'a>> {
                if matches!(self.class, Class::Property | Class::Data | Class::Context) {
                    Field::Present(DataDescription(self.json, DataRole::Full))
                } else {
                    Field::Absent
                }
            }
            pub fn input(self) -> Field<DataDescription<'a>> {
                if self.class == Class::Action {
                    description_field(self.json, "sdfInputData")
                } else {
                    Field::Absent
                }
            }
            pub fn output(self) -> Field<DataDescription<'a>> {
                if matches!(self.class, Class::Action | Class::Event) {
                    description_field(self.json, "sdfOutputData")
                } else {
                    Field::Absent
                }
            }
            pub fn readable(self) -> Field<bool> {
                self.property_flag("readable")
            }
            pub fn writable(self) -> Field<bool> {
                self.property_flag("writable")
            }
            pub fn observable(self) -> Field<bool> {
                self.property_flag("observable")
            }
            pub fn readable_characteristic(self) -> Field<Characteristic> {
                self.property_characteristic("readable")
            }
            pub fn writable_characteristic(self) -> Field<Characteristic> {
                self.property_characteristic("writable")
            }
            pub fn observable_characteristic(self) -> Field<Characteristic> {
                self.property_characteristic("observable")
            }
            fn property_characteristic(self, name: &str) -> Field<Characteristic> {
                if self.class != Class::Property {
                    return Field::Absent;
                }
                match boolean_field(self.json, name) {
                    Field::Present(value) => Field::Present(Characteristic::Declared(value)),
                    Field::Absent if $effective => {
                        Field::Present(Characteristic::SpecifiedDefault(true))
                    }
                    Field::Absent => Field::Absent,
                    Field::Unavailable(reason) => Field::Unavailable(reason),
                }
            }
            fn property_flag(self, name: &str) -> Field<bool> {
                if self.class == Class::Property {
                    boolean_default(self.json, name)
                } else {
                    Field::Absent
                }
            }
            /// Authored requiredness syntax, without resolving references or declaration identities.
            pub fn declaration_requiredness(self) -> Field<$JsonView<'a>> {
                self.json.requiredness()
            }
        }
        fn text_field<'a>(json: $JsonView<'a>, key: &str) -> Field<&'a str> {
            match json.semantic_get(key) {
                Field::Present(value) => value.text(),
                Field::Absent => Field::Absent,
                Field::Unavailable(r) => Field::Unavailable(r),
            }
        }
        fn boolean_field(json: $JsonView<'_>, key: &str) -> Field<bool> {
            match json.semantic_get(key) {
                Field::Present(value) => value.boolean(),
                Field::Absent => Field::Absent,
                Field::Unavailable(r) => Field::Unavailable(r),
            }
        }
        fn boolean_default(json: $JsonView<'_>, key: &str) -> Field<bool> {
            match boolean_field(json, key) {
                Field::Absent if $effective => Field::Present(true),
                field => field,
            }
        }
        fn description_field<'a>(json: $JsonView<'a>, key: &str) -> Field<DataDescription<'a>> {
            match json.semantic_get(key) {
                Field::Present(value) if value.kind() == JsonKind::Map => {
                    Field::Present(DataDescription(value, DataRole::Full))
                }
                Field::Present(_) => Field::Unavailable("data description map shape"),
                Field::Absent => Field::Absent,
                Field::Unavailable(r) => Field::Unavailable(r),
            }
        }

        #[derive(Clone, Copy)]
        pub struct DataDescription<'a>(pub(crate) $JsonView<'a>, pub(crate) DataRole);
        pub struct DataDescriptions<'a> {
            children: $Children<'a>,
        }
        impl<'a> Iterator for DataDescriptions<'a> {
            type Item = (&'a str, row_type!($rows, DataDescription<'a>));
            fn next(&mut self) -> Option<Self::Item> {
                let json = self.children.next()?;
                Some((
                    json.member_name()
                        .require()
                        .expect("validated description name"),
                    row_value!($rows, json, DataDescription(json, DataRole::Full)),
                ))
            }
        }
        impl<'a> DataDescription<'a> {
            pub fn json(self) -> $JsonView<'a> {
                self.0
            }
            pub fn role(self) -> DataRole {
                self.1
            }
            pub fn data_type(self) -> Field<DataType<'a>> {
                match text_field(self.0, "type") {
                    Field::Present("array") if self.1 == DataRole::DirectItem => {
                        Field::Unavailable("direct-item array meaning is not established")
                    }
                    Field::Present(name) => Field::Present(match name {
                        "number" => DataType::Known(BaseType::Number),
                        "integer" => DataType::Known(BaseType::Integer),
                        "string" => DataType::Known(BaseType::String),
                        "boolean" => DataType::Known(BaseType::Boolean),
                        "array" => DataType::Known(BaseType::Array),
                        "object" => DataType::Known(BaseType::Map),
                        other if other.contains(':') => DataType::Extended(other),
                        other => DataType::Undetermined(other),
                    }),
                    Field::Absent => Field::Absent,
                    Field::Unavailable(r) => Field::Unavailable(r),
                }
            }
            pub fn number(self, key: &str) -> Field<ExactNumber<'a>> {
                match self.0.semantic_get(key) {
                    Field::Present(_)
                        if self.1 == DataRole::DirectItem
                            && !matches!(
                                key,
                                "minimum" | "maximum" | "minLength" | "maxLength"
                            ) =>
                    {
                        Field::Unavailable("direct-item quality meaning is not established")
                    }
                    Field::Present(value) => value.number(),
                    Field::Absent => Field::Absent,
                    Field::Unavailable(r) => Field::Unavailable(r),
                }
            }
            pub fn minimum(self) -> Field<ExactNumber<'a>> {
                self.number("minimum")
            }
            pub fn maximum(self) -> Field<ExactNumber<'a>> {
                self.number("maximum")
            }
            pub fn exclusive_minimum(self) -> Field<ExactNumber<'a>> {
                self.number("exclusiveMinimum")
            }
            pub fn exclusive_maximum(self) -> Field<ExactNumber<'a>> {
                self.number("exclusiveMaximum")
            }
            /// Exact positive step; malformed shape or unavailable meaning stays unavailable.
            pub fn multiple_of(self) -> Field<ExactNumber<'a>> {
                match self.number("multipleOf") {
                    Field::Present(number) if number.sign() != std::cmp::Ordering::Greater => {
                        Field::Unavailable("multipleOf must be positive")
                    }
                    field => field,
                }
            }
            pub fn minimum_length(self) -> Field<ExactNumber<'a>> {
                self.nonnegative_count("minLength")
            }
            pub fn maximum_length(self) -> Field<ExactNumber<'a>> {
                self.nonnegative_count("maxLength")
            }
            pub fn minimum_items(self) -> Field<ExactNumber<'a>> {
                self.nonnegative_count("minItems")
            }
            pub fn maximum_items(self) -> Field<ExactNumber<'a>> {
                self.nonnegative_count("maxItems")
            }
            fn nonnegative_count(self, key: &str) -> Field<ExactNumber<'a>> {
                match self.number(key) {
                    Field::Present(number)
                        if !number.is_integer() || number.sign() == std::cmp::Ordering::Less =>
                    {
                        Field::Unavailable("nonnegative mathematically integral count shape")
                    }
                    field => field,
                }
            }
            pub fn pattern(self) -> Field<&'a str> {
                self.textual_quality("pattern")
            }
            /// Retained selector text, without asserting support for its additional meaning.
            pub fn format(self) -> Field<&'a str> {
                text_field(self.0, "format")
            }
            /// Retained selector text, without asserting support for its additional meaning.
            pub fn sdf_type(self) -> Field<&'a str> {
                self.textual_quality("sdfType")
            }
            pub fn unit(self) -> Field<&'a str> {
                self.textual_quality("unit")
            }
            pub fn content_format(self) -> Field<&'a str> {
                self.textual_quality("contentFormat")
            }
            fn textual_quality(self, key: &str) -> Field<&'a str> {
                match self.full_quality(key) {
                    Field::Present(json) => json.text(),
                    Field::Absent => Field::Absent,
                    Field::Unavailable(reason) => Field::Unavailable(reason),
                }
            }
            pub fn unique_items(self) -> Field<bool> {
                match self.full_quality("uniqueItems") {
                    Field::Present(json) => json.boolean(),
                    Field::Absent => Field::Absent,
                    Field::Unavailable(reason) => Field::Unavailable(reason),
                }
            }
            pub fn nullable(self) -> Field<bool> {
                if self.1 == DataRole::DirectItem
                    && !matches!(self.0.semantic_get("nullable"), Field::Absent)
                {
                    Field::Unavailable("direct-item nullable meaning is not established")
                } else {
                    if $effective
                        && self.1 == DataRole::DirectItem
                        && matches!(self.0.semantic_get("nullable"), Field::Absent)
                    {
                        Field::Unavailable(
                            "direct-item nullable default meaning is not established",
                        )
                    } else {
                        boolean_default(self.0, "nullable")
                    }
                }
            }
            /// Nullable admission is independent of mandatory presence and other constraints.
            pub fn nullable_characteristic(self) -> Field<Characteristic> {
                match self.nullable() {
                    Field::Present(value)
                        if $effective && matches!(self.0.get("nullable"), Field::Absent) =>
                    {
                        Field::Present(Characteristic::SpecifiedDefault(value))
                    }
                    Field::Present(value) => Field::Present(Characteristic::Declared(value)),
                    Field::Absent => Field::Absent,
                    Field::Unavailable(reason) => Field::Unavailable(reason),
                }
            }
            pub fn declared_default(self) -> Field<$JsonView<'a>> {
                self.full_quality("default")
            }
            pub fn constant(self) -> Field<$JsonView<'a>> {
                self.full_quality("const")
            }
            fn full_quality(self, key: &str) -> Field<$JsonView<'a>> {
                match self.0.semantic_get(key) {
                    Field::Present(_) if self.1 == DataRole::DirectItem => {
                        Field::Unavailable("direct-item quality meaning is not established")
                    }
                    field => field,
                }
            }
            /// Nonempty authored map-entry syntax; presence alone does not establish object applicability.
            pub fn required_entries(self) -> Field<$Children<'a>> {
                match self.object_quality("required") {
                    Field::Present(value)
                        if value.kind() == JsonKind::Array
                            && value.children().next().is_some()
                            && value
                                .children()
                                .all(|v| matches!(v.text(), Field::Present(_))) =>
                    {
                        Field::Present(value.children())
                    }
                    Field::Present(_) => {
                        Field::Unavailable("required entries nonempty string-array shape")
                    }
                    Field::Absent => Field::Absent,
                    Field::Unavailable(r) => Field::Unavailable(r),
                }
            }
            pub fn properties(self) -> Field<DataDescriptions<'a>> {
                match self.object_quality("properties") {
                    Field::Unavailable(reason) => Field::Unavailable(reason),
                    _ => self.named_descriptions("properties"),
                }
            }
            fn object_quality(self, key: &str) -> Field<$JsonView<'a>> {
                let field = self.0.semantic_get(key);
                if self.1 == DataRole::DirectItem
                    && !matches!(
                        self.data_type(),
                        Field::Present(DataType::Known(BaseType::Map))
                    )
                    && !matches!(field, Field::Absent)
                {
                    Field::Unavailable("direct-item object structure is not established")
                } else {
                    field
                }
            }
            /// Authored choice branches, before composition or shared-quality inheritance.
            pub fn source_alternatives(self) -> Field<DataDescriptions<'a>> {
                self.named_descriptions("sdfChoice")
            }
            fn named_descriptions(self, key: &str) -> Field<DataDescriptions<'a>> {
                match self.0.semantic_get(key) {
                    Field::Present(value)
                        if value.kind() == JsonKind::Map
                            && value.children().all(|v| {
                                ($effective || v.kind() == JsonKind::Map)
                                    && matches!(v.member_name(), Field::Present(_))
                            }) =>
                    {
                        Field::Present(DataDescriptions {
                            children: value.children(),
                        })
                    }
                    Field::Present(_) => Field::Unavailable("named data descriptions map shape"),
                    Field::Absent => Field::Absent,
                    Field::Unavailable(r) => Field::Unavailable(r),
                }
            }
            pub fn items(self) -> Field<DataDescription<'a>> {
                match description_field(self.0, "items") {
                    Field::Present(_) if self.1 == DataRole::DirectItem => {
                        Field::Unavailable("nested direct-item items meaning is not established")
                    }
                    Field::Present(description) => {
                        Field::Present(DataDescription(description.0, DataRole::DirectItem))
                    }
                    Field::Absent => Field::Absent,
                    Field::Unavailable(r) => Field::Unavailable(r),
                }
            }
            pub fn source_enum(self) -> Field<$Children<'a>> {
                match self.0.semantic_get("enum") {
                    Field::Present(value)
                        if value.kind() == JsonKind::Array
                            && value.children().next().is_some()
                            && value
                                .children()
                                .all(|v| matches!(v.text(), Field::Present(_))) =>
                    {
                        Field::Present(value.children())
                    }
                    Field::Present(_) => {
                        Field::Unavailable("enum nonempty decoded string-array shape")
                    }
                    Field::Absent => Field::Absent,
                    Field::Unavailable(r) => Field::Unavailable(r),
                }
            }
        }
    };
}
mod source_views {
    use super::*;
    typed_access!(JsonView, Children, false, source);
    impl Source {
        pub fn definitions(&self, class: Class) -> Field<Definitions<'_>> {
            if !self.role().is_model() {
                return Field::Unavailable("sample sources have no model definitions");
            }
            match self.json() {
                Field::Present(json) if class != Class::Context => definitions(json, class),
                Field::Present(json) => match json.get(class.keyword()) {
                    Field::Absent => Field::Absent,
                    _ => Field::Unavailable("Context groups occur only under Objects and Things"),
                },
                Field::Unavailable(r) => Field::Unavailable(r),
                Field::Absent => Field::Unavailable("source unavailable"),
            }
        }
        pub fn definition(&self, class: Class, name: &str) -> Lookup<Definition<'_>> {
            match self.definitions(class) {
                Field::Present(mut rows) => rows
                    .find(|(n, _)| *n == name)
                    .map_or(Lookup::NotFound, |(_, d)| Lookup::Found(d)),
                Field::Absent => Lookup::NotFound,
                Field::Unavailable(r) => Lookup::Unavailable(r),
            }
        }
    }
}
mod effective_views {
    use super::*;
    use crate::{EffectiveChildren, EffectiveJson, EffectiveModel};
    typed_access!(EffectiveJson, EffectiveChildren, true, effective);
    impl<'a> DataDescription<'a> {
        /// Final shared qualities and explicit overrides, separate from authored syntax.
        pub fn alternatives(self) -> Field<crate::EffectiveAlternatives<'a>> {
            self.0.alternatives()
        }
        pub(crate) fn selected(json: EffectiveJson<'a>) -> Self {
            Self(json, DataRole::Full)
        }
    }
    impl EffectiveModel<'_> {
        pub fn definitions(&self, class: Class) -> Field<Definitions<'_>> {
            match self.json() {
                Field::Present(json) => match json.n().role {
                    Some(role) if role.allows(class) => definitions(json, class),
                    Some(_) => match json.get(class.keyword()) {
                        Field::Absent => Field::Absent,
                        _ => Field::Unavailable("content has no established base class role here"),
                    },
                    None => Field::Unavailable("selected view has no established class group role"),
                },
                Field::Unavailable(r) => Field::Unavailable(r),
                Field::Absent => Field::Absent,
            }
        }
        pub fn definition(&self, class: Class, name: &str) -> Lookup<Definition<'_>> {
            match self.definitions(class) {
                Field::Present(mut rows) => match rows.find(|(n, _)| *n == name) {
                    Some((_, Field::Present(value))) => Lookup::Found(value),
                    Some((_, Field::Unavailable(reason))) => Lookup::Unavailable(reason),
                    _ => Lookup::NotFound,
                },
                Field::Absent => Lookup::NotFound,
                Field::Unavailable(reason) => Lookup::Unavailable(reason),
            }
        }
    }
}
pub use effective_views::{
    DataDescription as EffectiveDataDescription, DataDescriptions as EffectiveDataDescriptions,
    Definition as EffectiveDefinition, Definitions as EffectiveDefinitions,
};
pub use source_views::{DataDescription, DataDescriptions, Definition, Definitions};

impl<'a> JsonView<'a> {
    pub(crate) fn semantic_get(self, key: &str) -> Field<Self> {
        self.get(key)
    }
    pub(crate) fn requiredness(self) -> Field<Self> {
        self.get("sdfRequired")
    }
}
