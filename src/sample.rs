//! Selected admission follows ADR0016: Selected samples with explicit admission and coverage,
//! ADR0022: Scalar constraints and characteristic states, and ADR0025: Selected numeric samples.
use crate::allowance::{OperationBudget, charge, push};
use crate::finding::{Basis, Finding, Verdict};
use crate::role::SetAside;
use crate::scalar::{Context, ResourceFormat, TemporalFormat};
use crate::value::{BorrowedChildren, BorrowedValue, ValueContext};
use crate::{
    AssessmentStatus as Status, Catalogue, Class, EffectiveDataDescription, EffectiveJson,
    EffectiveModel, Field, JsonKind, SampleValueView, SourceId, SourceRole,
};
pub(crate) mod abstract_value;
mod generated;
pub(crate) use generated::{
    CheckedValue, ValueAdmission, ValueAllowance, ValueInterpretations, check_generated,
    check_generated_observed, check_value,
};
mod meaning;
pub use meaning::{SampleBasisRef, SampleMeaning, SampleMeaningArea, SampleMeaningConclusion};

pub(crate) const AREAS: [&str; 9] = [
    "sample-context/1",
    "sample-type-null/1",
    "sample-numeric/1",
    "sample-text-boolean/1",
    "sample-const/1",
    "sample-alternatives/1",
    "sample-map/1",
    "sample-array/1",
    "sample-abstract-values/1",
];
pub(crate) const CONSISTENCY_AREAS: [&str; 2] = ["const-consistency/1", "default-consistency/1"];
pub(crate) fn relevant_quality(key: &str) -> bool {
    matches!(
        key,
        "type"
            | "nullable"
            | "minimum"
            | "maximum"
            | "exclusiveMinimum"
            | "exclusiveMaximum"
            | "multipleOf"
            | "minLength"
            | "maxLength"
            | "pattern"
            | "sdfType"
            | "format"
            | "contentFormat"
            | "sdfRef"
            | "const"
            | "enum"
            | "sdfChoice"
            | "properties"
            | "required"
            | "items"
            | "minItems"
            | "maxItems"
            | "uniqueItems"
    )
}
pub(crate) const SELECTOR_BYTES: usize = 8192;
pub(crate) const DESCRIPTION_BYTES: usize = SELECTOR_BYTES + 14;
pub(crate) const MANDATORY_WIRE: usize =
    2 * (6 * SELECTOR_BYTES + 2) + (6 * DESCRIPTION_BYTES + 2) + 16384;
const IDENTITY_WORK: usize = 8 * SELECTOR_BYTES + 128;
const MANDATORY_OWNERSHIP: usize = 4 * MANDATORY_WIRE;
// Bounded map slots/backend storage, simultaneous scope arrays, pointer copies and ID vectors.
pub(crate) const MANDATORY_DTO: usize = 512 * 512
    + 2 * (3 * 256 + 1) * 128
    + 3 * SELECTOR_BYTES
    + 2 * DESCRIPTION_BYTES
    + 6 * 256 * std::mem::size_of::<u32>();
pub(crate) fn mandatory_storage_supported() -> bool {
    let owned = SELECTOR_BYTES
        + DESCRIPTION_BYTES
        + SELECTOR_BYTES * std::mem::size_of::<usize>()
        + std::mem::size_of::<State>()
        + std::mem::size_of::<crate::Report<'_>>()
        + std::mem::size_of::<EffectiveModel<'_>>()
        + std::mem::size_of::<SourceId>();
    owned
        .checked_add(MANDATORY_DTO)
        .and_then(|n| n.checked_add(mandatory_staging_headers()))
        .is_some_and(|n| n <= MANDATORY_OWNERSHIP)
}
fn mandatory_staging_headers() -> usize {
    use std::mem::size_of;
    let evaluation = size_of::<crate::structure::Structure<'_>>()
        + size_of::<crate::structure::ParameterLedger>()
        + 2 * size_of::<Evaluation>()
        + size_of::<crate::NumberWork>()
        + size_of::<OperationBudget>()
        + size_of::<Vec<Frame<'_>>>()
        + size_of::<Vec<EffectiveJson<'_>>>()
        + size_of::<Vec<Finding>>()
        + size_of::<Frame<'_>>()
        + size_of::<BorrowedChildren<'_>>()
        + 2 * size_of::<EffectiveJson<'_>>()
        + size_of::<BorrowedValue<'_>>()
        + 2 * size_of::<ValueContext<'_>>();
    crate::effective::sample_staging_fixed_headers()
        .max(evaluation + crate::equality::fixed_headers())
        + crate::civil::fixed_headers()
            .max(crate::formats::fixed_headers())
            .max(abstract_value::fixed_headers())
}
fn mandatory_charge(remaining: &mut usize, bytes: usize) -> Result<(), &'static str> {
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or("sample mandatory storage limit")?;
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SampleKind {
    Property,
    Data,
    ActionInput,
    ActionOutput,
    EventOutput,
    /// Non-interactive metadata declared under an `sdfObject` or `sdfThing`.
    Context,
}
impl SampleKind {
    /// Every kind, in the order the selector contract lists them.
    pub(crate) const ALL: [Self; 6] = [
        Self::Property,
        Self::Data,
        Self::ActionInput,
        Self::ActionOutput,
        Self::EventOutput,
        Self::Context,
    ];
    /// The kind whose name `as_str` gives, such as `action_input`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Property => "property",
            Self::Data => "data",
            Self::ActionInput => "action_input",
            Self::ActionOutput => "action_output",
            Self::EventOutput => "event_output",
            Self::Context => "context",
        }
    }
    fn class(self) -> Class {
        match self {
            Self::Property => Class::Property,
            Self::Data => Class::Data,
            Self::ActionInput | Self::ActionOutput => Class::Action,
            Self::EventOutput => Class::Event,
            Self::Context => Class::Context,
        }
    }
    pub(crate) fn suffix(self) -> &'static str {
        match self {
            Self::ActionInput => "/sdfInputData",
            Self::ActionOutput | Self::EventOutput => "/sdfOutputData",
            _ => "",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Admission {
    Admitted,
    Rejected,
    Undetermined,
}
impl Admission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Rejected => "rejected",
            Self::Undetermined => "undetermined",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContextState {
    Available,
    Partial,
    Unavailable,
}
impl ContextState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct ContextFindings {
    pub(crate) errors: u32,
    pub(crate) warnings: u32,
    pub(crate) notices: u32,
}
impl ContextFindings {
    pub fn errors(self) -> u32 {
        self.errors
    }
    pub fn warnings(self) -> u32 {
        self.warnings
    }
    pub fn notices(self) -> u32 {
        self.notices
    }
}
enum Selection<'a> {
    Pointer(&'a str),
    Description(EffectiveDataDescription<'a>),
}
pub struct SampleRequest<'a> {
    pub(crate) model: SourceId,
    kind: SampleKind,
    selection: Selection<'a>,
    pub(crate) sample: Option<SourceId>,
    pub(crate) strict: bool,
}
impl<'a> SampleRequest<'a> {
    pub fn new(model: SourceId, kind: SampleKind, pointer: &'a str) -> Self {
        Self {
            model,
            kind,
            selection: Selection::Pointer(pointer),
            sample: None,
            strict: false,
        }
    }
    pub fn from_description(
        description: EffectiveDataDescription<'a>,
    ) -> Result<Self, &'static str> {
        let (kind, _) = description_root(description.json())?;
        Ok(Self {
            model: description.json().model.source,
            kind,
            selection: Selection::Description(description),
            sample: None,
            strict: false,
        })
    }
    pub fn sample(mut self, sample: SourceId) -> Self {
        self.sample = Some(sample);
        self
    }
    pub fn strict_warnings(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}
pub(crate) fn description_root(
    json: EffectiveJson<'_>,
) -> Result<(SampleKind, usize), &'static str> {
    use crate::role::Role;
    match json.n().role {
        Some(Role::Definition(Class::Property)) => Ok((SampleKind::Property, json.node)),
        Some(Role::Definition(Class::Data)) => Ok((SampleKind::Data, json.node)),
        Some(Role::Definition(Class::Context)) => Ok((SampleKind::Context, json.node)),
        Some(Role::Data) => {
            let parent = json
                .n()
                .parent
                .ok_or("description is not a supported selected root")?;
            let role = json.model.nodes[parent].role;
            match (role, json.n().key) {
                (Some(Role::Definition(Class::Action)), Some("sdfInputData")) => {
                    Ok((SampleKind::ActionInput, parent))
                }
                (Some(Role::Definition(Class::Action)), Some("sdfOutputData")) => {
                    Ok((SampleKind::ActionOutput, parent))
                }
                (Some(Role::Definition(Class::Event)), Some("sdfOutputData")) => {
                    Ok((SampleKind::EventOutput, parent))
                }
                _ => Err("description is not a supported selected root"),
            }
        }
        _ => Err("description is not a supported selected root"),
    }
}
pub(crate) struct State {
    pub(crate) mandatory_remaining: usize,
    pub(crate) model: SourceId,
    pub(crate) sample: SourceId,
    pub(crate) kind: SampleKind,
    pub(crate) pointer: String,
    pub(crate) description_pointer: String,
    pub(crate) format_use: Option<u8>,
    pub(crate) abstract_use: u8,
    evaluation: Evaluation,
}
pub(crate) struct Evaluation {
    meaning: meaning::MeaningRecord,
    sample_node: usize,
    pub(crate) admission: Admission,
    pub(crate) context: ContextState,
    pub(crate) context_findings: ContextFindings,
    pub(crate) areas: [Status; 9],
    pub(crate) type_null_basis: Basis,
    pub(crate) text_boolean_basis: Basis,
    pub(crate) abstract_basis: Basis,
    pub(crate) abstract_unknown: u8,
    pub(crate) temporal_use: u8,
    pub(crate) findings: Vec<Finding>,
    pub(crate) total_findings: usize,
    pub(crate) errors: u32,
    pub(crate) storage_omitted: bool,
    witness: Option<std::num::NonZeroUsize>,
}
impl std::ops::Deref for State {
    type Target = Evaluation;
    fn deref(&self) -> &Self::Target {
        &self.evaluation
    }
}
impl std::ops::DerefMut for State {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.evaluation
    }
}
enum FrameKind<'a> {
    Leaf,
    Choice,
    Array {
        items: Field<EffectiveJson<'a>>,
        children: Option<BorrowedChildren<'a>>,
        awaiting: bool,
        established: bool,
    },
    Map {
        required: Field<EffectiveJson<'a>>,
        properties: Field<EffectiveJson<'a>>,
        children: Option<crate::EffectiveChildren<'a>>,
        awaiting: bool,
        established: bool,
    },
}
struct Frame<'a> {
    json: EffectiveJson<'a>,
    pair_root: EffectiveJson<'a>,
    sample: BorrowedValue<'a>,
    kind: FrameKind<'a>,
    alternatives: Field<crate::EffectiveAlternatives<'a>>,
    evaluation: Evaluation,
    unknown: bool,
    seen: bool,
}
#[allow(dead_code)]
pub(crate) fn pair_finding(second: usize, first: usize, parameter: usize) -> Finding {
    Finding {
        sample_node: second,
        paired_sample: Some(first),
        basis: Basis::Array,
        code: "sample.unique",
        constraint: "uniqueItems",
        message: "Array elements have an equal value",
        parameter: Some(parameter),
        alternative: None,
    }
}
pub(crate) fn select<'a>(
    catalogue: &'a Catalogue,
    request: &SampleRequest<'_>,
    budget: &mut OperationBudget,
    parameters: &mut crate::structure::Structure<'a>,
    ledger: &mut crate::structure::ParameterLedger,
) -> Result<(EffectiveModel<'a>, State), &'static str> {
    let mut identity_work = crate::json::PointerBudget::new(IDENTITY_WORK);
    let mut mandatory_remaining = MANDATORY_OWNERSHIP;
    mandatory_charge(
        &mut mandatory_remaining,
        std::mem::size_of::<State>() + std::mem::size_of::<crate::Report<'_>>(),
    )?;
    mandatory_charge(&mut mandatory_remaining, mandatory_staging_headers())?;
    identity_work
        .debit(catalogue.sources.len())
        .map_err(|_| "sample identity work limit")?;
    let source = catalogue
        .source(request.model)
        .ok_or("unknown selected model")?;
    if !source.role.is_model() {
        return Err("sample source cannot be a model root");
    }
    let sample = request.sample.ok_or("explicit sample input required")?;
    identity_work
        .debit(catalogue.sources.len())
        .map_err(|_| "sample identity work limit")?;
    if catalogue
        .source(sample)
        .is_none_or(|s| s.role != SourceRole::Sample)
    {
        return Err("selected sample must identify a supplied sample source");
    }
    let pointer = match request.selection {
        Selection::Pointer(pointer) => {
            if pointer.len() > SELECTOR_BYTES {
                return Err("invalid or oversized JSON pointer selector");
            }
            identity_work
                .debit(2 * pointer.len() + 1)
                .map_err(|_| "sample identity work limit")?;
            crate::json::validate_pointer(pointer)?;
            mandatory_charge(&mut mandatory_remaining, pointer.len())?;
            let mut owned = String::with_capacity(pointer.len());
            owned.push_str(pointer);
            owned
        }
        Selection::Description(description) => {
            identity_work
                .debit(16)
                .map_err(|_| "sample identity work limit")?;
            let json = description.json();
            if !std::ptr::eq(json.model.catalogue, catalogue) {
                return Err("description belongs to another catalogue");
            }
            let (_, node) = description_root(json)?;
            let location = EffectiveJson {
                model: json.model,
                node,
            }
            .location();
            let size = location
                .pointer_size(SELECTOR_BYTES, &mut identity_work)
                .map_err(|_| "invalid or oversized description selector")?;
            mandatory_charge(
                &mut mandatory_remaining,
                size.storage_bytes()
                    .ok_or("sample identity storage overflow")?,
            )?;
            identity_work
                .debit(size.render_work())
                .map_err(|_| "sample identity work limit")?;
            let pointer = location.render_pointer(size);
            identity_work
                .debit(pointer.len() + 1)
                .map_err(|_| "sample identity work limit")?;
            crate::json::validate_pointer(&pointer)?;
            pointer
        }
    };
    let description_size = pointer
        .len()
        .checked_add(request.kind.suffix().len())
        .filter(|size| *size <= DESCRIPTION_BYTES)
        .ok_or("sample description selector limit")?;
    mandatory_charge(&mut mandatory_remaining, description_size)?;
    identity_work
        .debit(description_size + 1)
        .map_err(|_| "sample identity work limit")?;
    let mut description_pointer = String::with_capacity(description_size);
    description_pointer.push_str(&pointer);
    description_pointer.push_str(request.kind.suffix());
    let gate = source
        .json()
        .present()
        .map_or(crate::structure::FeatureContext::Limited, |root| {
            crate::structure::Structure::preflight(catalogue, root, budget)
        });
    let model = catalogue.compose_sample(
        crate::EffectiveRequest::new(request.model).pointer(&description_pointer),
        budget,
        gate,
        catalogue
            .source(sample)
            .and_then(|source| source.sample_value().present()),
        parameters,
        ledger,
    )?;
    if let Some(json) = model.json().present() {
        let (kind, _) = description_root(json)?;
        if kind != request.kind || kind.class() != request.kind.class() {
            return Err("selected definition has the wrong sample kind");
        }
    }
    let state = State {
        mandatory_remaining,
        model: request.model,
        sample,
        kind: request.kind,
        pointer,
        description_pointer,
        format_use: None,
        abstract_use: 0,
        evaluation: Evaluation {
            meaning: meaning::MeaningRecord::default(),
            abstract_basis: Basis::Numeric,
            abstract_unknown: 0,
            temporal_use: 0,
            sample_node: 0,
            admission: Admission::Undetermined,
            context: ContextState::Unavailable,
            context_findings: ContextFindings::default(),
            areas: [Status::Undetermined; 9],
            type_null_basis: Basis::Numeric,
            text_boolean_basis: Basis::Numeric,
            findings: Vec::new(),
            total_findings: 0,
            errors: 0,
            storage_omitted: false,
            witness: None,
        },
    };
    Ok((model, state))
}
pub(crate) fn mandatory_slot<T>(state: &mut State) -> Result<Vec<T>, &'static str> {
    mandatory_charge(&mut state.mandatory_remaining, std::mem::size_of::<T>())?;
    Ok(Vec::with_capacity(1))
}
pub(crate) fn mandatory_dto(state: &State) -> usize {
    let mut remaining = state.mandatory_remaining;
    mandatory_charge(&mut remaining, MANDATORY_DTO).expect("admitted sample DTO storage");
    remaining
}
fn exists(json: EffectiveJson<'_>, key: &str, budget: &mut OperationBudget) -> bool {
    !matches!(json.lookup(key, budget), Field::Absent)
}
pub(crate) fn ordinary_context(json: EffectiveJson<'_>, budget: &mut OperationBudget) -> bool {
    if json.n().unavailable.is_some()
        || json.n().meaning_unavailable.is_some()
        || json.n().donor_unavailable.is_some()
    {
        return false;
    }
    for child in json.children() {
        let Some(key) = child.n().key else {
            return false;
        };
        if !charge(&mut budget.work, 5 * (key.len() + 1) + 40) {
            return false;
        }
        if matches!(
            key,
            "enum" | "sdfChoice" | "sdfType" | "format" | "contentFormat"
        ) {
            return false;
        }
        if child.n().donor_unavailable.is_some() && !matches!(key, "items" | "properties") {
            return false;
        }
    }
    true
}
pub(crate) fn ordinary_map_context(json: EffectiveJson<'_>, budget: &mut OperationBudget) -> bool {
    if !ordinary_context(json, budget) {
        return false;
    }
    let mut set_aside = SetAside::default();
    for child in json.children() {
        let Some(key) = child.n().key else {
            return false;
        };
        if !charge(&mut budget.work, key.len() + 1) {
            return false;
        }
        let established = match key {
            "$comment" | "label" | "description" | "default" | "unit" | "sdfRequired" | "type"
            | "nullable" | "const" | "minimum" | "maximum" | "exclusiveMinimum"
            | "exclusiveMaximum" | "multipleOf" | "minLength" | "maxLength" | "pattern"
            | "properties" | "required" | "items" | "minItems" | "maxItems" | "uniqueItems" => true,
            "readable" | "writable" | "observable" => matches!(
                json.n().role,
                Some(crate::role::Role::Definition(Class::Property))
            ),
            // `object` only matters for Direct item `required` and `properties`, listed above.
            _ => json
                .n()
                .role
                .is_some_and(|role| set_aside.classify(role, key, false).ignorable()),
        };
        if !established {
            return false;
        }
    }
    budget.set_aside |= set_aside;
    true
}
pub(crate) fn scalar_format_context(
    json: EffectiveJson<'_>,
    budget: &mut OperationBudget,
) -> Context {
    if json.n().unavailable.is_some()
        || json.n().meaning_unavailable.is_some()
        || json.n().donor_unavailable.is_some()
        || json.n().sample_skipped
    {
        return Context::Unavailable;
    }
    let Some(role) = json.n().role.filter(|role| role.data_description_role()) else {
        return Context::Unavailable;
    };
    let type_parameter = json.lookup("type", budget);
    let kind = type_parameter
        .present()
        .and_then(|value| value.text().present());
    let convention = json.lookup("sdfType", budget);
    let abstract_context = match convention {
        Field::Absent => None,
        Field::Present(value) => match value.text().present() {
            Some("byte-string")
                if matches!(type_parameter, Field::Absent) || kind == Some("string") =>
            {
                Some(Context::Bytes)
            }
            Some("unix-time")
                if matches!(type_parameter, Field::Absent)
                    || matches!(kind, Some("number" | "integer")) =>
            {
                Some(Context::Unix)
            }
            _ => return Context::Unavailable,
        },
        Field::Unavailable(_) => return Context::Unavailable,
    };
    let format = if let Some(context) = abstract_context {
        if !matches!(json.lookup("format", budget), Field::Absent)
            || !matches!(json.lookup("contentFormat", budget), Field::Absent)
        {
            return Context::Unavailable;
        }
        context
    } else {
        if kind != Some("string") {
            return Context::Unavailable;
        }
        let Some(format) = json
            .lookup("format", budget)
            .present()
            .and_then(|value| value.text().present())
            .and_then(|text| {
                TemporalFormat::from_text(text)
                    .map(Context::Temporal)
                    .or_else(|| ResourceFormat::from_text(text).map(Context::Resource))
            })
        else {
            return Context::Unavailable;
        };
        format
    };
    let mut set_aside = SetAside::default();
    for child in json.children() {
        let Some(key) = child.n().key else {
            return Context::Unavailable;
        };
        if !charge(&mut budget.work, 64 * (key.len() + 1)) {
            return Context::Unavailable;
        }
        let vocabulary = set_aside.classify(role, key, false);
        if vocabulary.blocks_base_meaning() || child.n().donor_unavailable.is_some() {
            return Context::Unavailable;
        }
        if matches!(key, "items" | "properties" | "contentFormat")
            || abstract_context.is_none() && matches!(key, "sdfType" | "sdfRef")
        {
            return Context::Unavailable;
        }
    }
    budget.set_aside |= set_aside;
    format
}
pub(crate) fn ordinary_untyped_container(
    json: EffectiveJson<'_>,
    budget: &mut OperationBudget,
) -> bool {
    json.n().role.is_some_and(crate::role::Role::full_data)
        && ordinary_map_context(json, budget)
        && ["type", "items", "properties", "required"]
            .into_iter()
            .all(|key| matches!(json.lookup(key, budget), Field::Absent))
}
pub(crate) fn alternatives<'a>(
    json: EffectiveJson<'a>,
    budget: &mut OperationBudget,
) -> Field<crate::EffectiveAlternatives<'a>> {
    for child in json.children() {
        if !charge(
            &mut budget.work,
            2 * (12 + child.n().key.map_or(0, str::len)),
        ) {
            return Field::Unavailable("sample alternative lookup work limit");
        }
    }
    json.alternatives()
}
pub(crate) fn needed_rules(value: Option<SampleValueView<'_>>) -> crate::structure::SampleRules {
    let nullable = value.is_some_and(|value| value.kind() == JsonKind::Null);
    let numeric = value.is_some_and(|value| value.kind() == JsonKind::Number);
    let text = value.is_some_and(|value| value.kind() == JsonKind::String);
    crate::structure::SampleRules {
        map: value.is_some_and(|value| value.kind() == JsonKind::Map),
        array: value.is_some_and(|value| value.kind() == JsonKind::Array),
        numeric,
        nullable,
        text,
        ..crate::structure::SampleRules::default()
    }
}
pub(crate) fn parameter_rules(
    json: EffectiveJson<'_>,
    requested: crate::structure::SampleRules,
    work: &mut usize,
) -> crate::structure::SampleRules {
    let mut budget = OperationBudget::lease(*work);
    let ordinary = ordinary_context(json, &mut budget)
        || matches!(
            scalar_format_context(json, &mut budget),
            Context::Temporal(_) | Context::Resource(_) | Context::Bytes | Context::Unix
        );
    let kind = json.lookup("type", &mut budget);
    let numeric = ordinary && requested.numeric && matches!(kind, Field::Absent)
        || ordinary
            && requested.numeric
            && matches!(
                kind.present().and_then(|v| v.text().present()),
                Some("number" | "integer")
            );
    let text = ordinary
        && requested.text
        && (matches!(kind, Field::Absent)
            || kind.present().and_then(|v| v.text().present()) == Some("string"));
    let map = ordinary
        && requested.map
        && kind.present().and_then(|v| v.text().present()) == Some("object")
        && ordinary_map_context(json, &mut budget);
    let array = ordinary
        && requested.array
        && (kind.present().and_then(|v| v.text().present()) == Some("array")
            || ordinary_untyped_container(json, &mut budget))
        && ordinary_map_context(json, &mut budget);
    *work = budget.work;
    crate::structure::SampleRules {
        map,
        array,
        numeric,
        text,
        nullable: requested.nullable,
        ..requested
    }
}
pub(crate) fn sample_member<'a>(
    sample: SampleValueView<'a>,
    key: &str,
    work: &mut usize,
) -> Field<SampleValueView<'a>> {
    for child in sample.children() {
        if !charge(
            work,
            2 * (key.len() + child.member_name().present().map_or(0, str::len) + 1),
        ) {
            return Field::Unavailable("sample member lookup work limit");
        }
    }
    sample.get(key)
}
pub(crate) fn sample_keys_known(sample: SampleValueView<'_>, work: &mut usize) -> bool {
    value_keys_known(BorrowedValue::Sample(sample), work)
}
fn value_member<'a>(
    sample: BorrowedValue<'a>,
    key: &str,
    work: &mut usize,
) -> Field<BorrowedValue<'a>> {
    match sample {
        BorrowedValue::Sample(sample) => match sample_member(sample, key, work) {
            Field::Present(value) => Field::Present(BorrowedValue::Sample(value)),
            Field::Absent => Field::Absent,
            Field::Unavailable(reason) => Field::Unavailable(reason),
        },
        BorrowedValue::Model(_) | BorrowedValue::Constraint(_) => sample.member(key, work),
    }
}
fn value_keys_known(sample: BorrowedValue<'_>, work: &mut usize) -> bool {
    if sample.kind() != JsonKind::Map {
        return false;
    }
    let mut known = true;
    for child in sample.children() {
        if !charge(
            work,
            child
                .member_name()
                .present()
                .map_or(child.raw_json().len(), str::len)
                + 1,
        ) {
            return false;
        }
        known &= matches!(child.member_name(), Field::Present(_));
    }
    known
}
pub(crate) fn description_members_known(json: EffectiveJson<'_>, work: &mut usize) -> bool {
    if json.kind() != JsonKind::Map
        || json.n().meaning_unavailable.is_some()
        || json.n().donor_unavailable.is_some()
    {
        return false;
    }
    let mut known = true;
    for child in json.children() {
        if !charge(
            work,
            child
                .n()
                .key
                .map_or(child.n().source.raw_json().len(), str::len)
                + 1,
        ) {
            return false;
        }
        known &= child.n().key.is_some();
    }
    known
}
fn required_meaning(required: &Field<EffectiveJson<'_>>, work: &mut usize) -> bool {
    match required {
        Field::Absent => true,
        Field::Unavailable(_) => false,
        Field::Present(list) => {
            if list.kind() != JsonKind::Array
                || list.n().meaning_unavailable.is_some()
                || list.n().donor_unavailable.is_some()
            {
                return false;
            }
            let mut seen = false;
            let mut known = true;
            for name in list.children() {
                if !charge(work, name.n().source.raw_json().len() + 1) {
                    return false;
                }
                seen = true;
                known &= name.n().meaning_unavailable.is_none()
                    && name.n().donor_unavailable.is_none()
                    && matches!(name.text(), Field::Present(_));
            }
            seen && known
        }
    }
}
pub(crate) fn evaluate(
    catalogue: &Catalogue,
    model: &EffectiveModel<'_>,
    state: &mut State,
    budget: &mut OperationBudget,
) {
    let Some(json) = model.json().present() else {
        return;
    };
    if model.failure.is_some() || json.kind() != JsonKind::Map {
        return;
    }
    state.context = ContextState::Available;
    let sample_source = catalogue.source(state.sample).expect("validated sample");
    if !sample_source.complete {
        return;
    }
    let Some(sample) = sample_source.sample_value().present() else {
        return;
    };
    let mut work = crate::NumberWork::from_remaining(catalogue.numeric_remaining);
    let mut equality = crate::equality::EqualityWork::new(catalogue.limits.equality_pairs);
    state.evaluation = evaluate_branches(
        &ValueContext {
            description: json,
            root: BorrowedValue::Sample(sample),
        },
        budget,
        &mut work,
        &mut equality,
    );
    state.format_use = equality.format_established.then_some(equality.format_use);
    state.abstract_use |= equality.abstract_use;
}
pub(crate) fn evaluate_branches<'a>(
    context: &ValueContext<'a>,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
) -> Evaluation {
    let mut evaluation = evaluate_branches_inner(context, budget, work, equality, None);
    evaluation.temporal_use |= equality.temporal_use;
    evaluation
}
pub(crate) fn evaluate_branches_observed<'a>(
    context: &ValueContext<'a>,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
    observations: &mut crate::generation::choice::Collection,
) -> Evaluation {
    let mut evaluation =
        evaluate_branches_inner(context, budget, work, equality, Some(observations));
    evaluation.temporal_use |= equality.temporal_use;
    evaluation
}
fn evaluate_branches_inner<'a>(
    context: &ValueContext<'a>,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
    mut observations: Option<&mut crate::generation::choice::Collection>,
) -> Evaluation {
    if !context.valid() {
        return Evaluation::unavailable();
    }
    if !charge(
        &mut budget.allocation,
        2 * std::mem::size_of::<Evaluation>(),
    ) {
        return Evaluation::unavailable_cause(5);
    }
    let mut frames = Vec::new();
    if crate::allowance::admit(&mut frames, &mut budget.allocation).is_err() {
        return Evaluation::unavailable_cause(5);
    }
    equality.format_established = true;
    let root = context.description;
    let sample = context.root;
    let initial = pair_frame(context, root, root, sample, budget, work, equality);
    if let Some(observations) = observations.as_deref_mut() {
        observations.enter();
    }
    frames.push(initial);
    while !frames.is_empty() {
        if !matches!(
            frames.last().expect("active sample pair").kind,
            FrameKind::Leaf
        ) && !charge(&mut budget.work, 1)
        {
            return interrupted(frames, budget, 3);
        }
        let frame = frames.last_mut().expect("active sample pair");
        let completed = match &mut frame.kind {
            FrameKind::Leaf => true,
            FrameKind::Array {
                items,
                children,
                awaiting,
                established,
            } => {
                *awaiting = false;
                if let Some(value) = children.as_mut().and_then(Iterator::next) {
                    if let Field::Present(item) = *items {
                        *awaiting = true;
                        if crate::allowance::admit(&mut frames, &mut budget.allocation).is_err() {
                            return interrupted(frames, budget, 5);
                        }
                        let child = pair_frame(context, item, item, value, budget, work, equality);
                        if let Some(observations) = observations.as_deref_mut() {
                            observations.enter();
                        }
                        frames.push(child);
                    }
                    continue;
                }
                if !*established {
                    frame.evaluation.areas[7] = Status::Undetermined;
                    frame.evaluation.context = ContextState::Partial;
                }
                frame.evaluation.finish_value();
                true
            }
            FrameKind::Map {
                required,
                properties,
                children,
                awaiting,
                established,
            } => {
                *awaiting = false;
                if let Some(child) = children.as_mut().and_then(Iterator::next) {
                    if !charge(
                        &mut budget.work,
                        child.n().key.map_or(1, |key| key.len() + 1),
                    ) {
                        return interrupted(frames, budget, 3);
                    }
                    let value = child
                        .n()
                        .key
                        .map_or(Field::Unavailable("property identity unavailable"), |key| {
                            value_member(frame.sample, key, &mut budget.work)
                        });
                    match value {
                        Field::Present(value) => {
                            *awaiting = true;
                            if crate::allowance::admit(&mut frames, &mut budget.allocation).is_err()
                            {
                                return interrupted(frames, budget, 5);
                            }
                            let child =
                                pair_frame(context, child, child, value, budget, work, equality);
                            if let Some(observations) = observations.as_deref_mut() {
                                observations.enter();
                            }
                            frames.push(child);
                        }
                        Field::Absent => {}
                        Field::Unavailable(_) => {
                            frame.evaluation.areas[6] = Status::Undetermined;
                            frame.evaluation.context = ContextState::Partial;
                        }
                    }
                    continue;
                }
                if !*established
                    || matches!(required, Field::Unavailable(_))
                    || matches!(properties, Field::Unavailable(_))
                {
                    frame.evaluation.areas[6] = Status::Undetermined;
                }
                frame.evaluation.finish_value();
                true
            }
            FrameKind::Choice => match &mut frame.alternatives {
                Field::Unavailable(_) => {
                    frame.evaluation = Evaluation::unavailable();
                    true
                }
                Field::Absent => true,
                Field::Present(rows) => {
                    if let Some((_, branch)) = rows.next() {
                        frame.seen = true;
                        match branch {
                            Field::Present(branch) => {
                                let pair_root = frame.pair_root;
                                let sample = frame.sample;
                                if crate::allowance::admit(&mut frames, &mut budget.allocation)
                                    .is_err()
                                {
                                    return interrupted(frames, budget, 5);
                                }
                                let child = pair_frame(
                                    context,
                                    pair_root,
                                    branch.json(),
                                    sample,
                                    budget,
                                    work,
                                    equality,
                                );
                                if let Some(observations) = observations.as_deref_mut() {
                                    observations.enter();
                                }
                                frames.push(child);
                            }
                            _ => frame.unknown = true,
                        }
                        continue;
                    }
                    if !frame.seen {
                        reject(
                            &mut frame.evaluation,
                            "sample.alternatives",
                            "sdfChoice",
                            "Every effective alternative excludes the sample",
                            Some(frame.json.node),
                            budget,
                        );
                    }
                    frame.evaluation.finish_choice(frame.unknown);
                    true
                }
            },
        };
        if completed {
            let frame = frames.pop().expect("completed frame");
            let mut result = frame.evaluation;
            if let Some(observations) = observations.as_deref_mut() {
                observations.leave(result.admission);
            }
            if frame.json.node != frame.pair_root.node {
                if charge(&mut budget.work, result.findings.len()) {
                    for finding in &mut result.findings {
                        finding.alternative.get_or_insert(frame.json.node);
                    }
                } else {
                    result.findings.clear();
                    result.storage_omitted = true;
                }
                result.assign_witness(frame.json.node);
            }
            let Some(parent) = frames.last_mut() else {
                return result;
            };
            if matches!(parent.kind, FrameKind::Choice) && result.admission == Admission::Admitted {
                if let Some(observations) = observations.as_deref_mut() {
                    observations.observe(parent.json.node, frame.json.node, frame.sample);
                }
                parent.evaluation.accept_choice_witness(result);
                parent.kind = FrameKind::Leaf;
            } else {
                if let FrameKind::Map { awaiting, .. } | FrameKind::Array { awaiting, .. } =
                    &mut parent.kind
                {
                    debug_assert!(*awaiting);
                    *awaiting = false;
                }
                merge(&mut parent.evaluation, result, &mut parent.unknown, budget);
            }
        }
    }
    Evaluation::unavailable()
}
fn pair_frame<'a>(
    context: &ValueContext<'a>,
    pair_root: EffectiveJson<'a>,
    json: EffectiveJson<'a>,
    sample: BorrowedValue<'a>,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
) -> Frame<'a> {
    let alternatives = alternatives(json, budget);
    let mut evaluation = Evaluation::empty();
    evaluation.sample_node = sample.node();
    let mut kind = FrameKind::Choice;
    if matches!(alternatives, Field::Absent) {
        if charge(&mut budget.work, 1) {
            evaluate_leaf(
                context,
                json,
                sample,
                &mut evaluation,
                budget,
                work,
                equality,
            );
        } else {
            evaluation = Evaluation::unavailable_cause(3);
        }
        kind = FrameKind::Leaf;
        if evaluation.areas[6] == Status::Assessed && sample.kind() == JsonKind::Map {
            let required = json.lookup("required", budget);
            let properties = json.lookup("properties", budget);
            let identities = value_keys_known(sample, &mut budget.work);
            let presence = identities && required_meaning(&required, &mut budget.work);
            if presence {
                if let Field::Present(list) = required {
                    for name in list.children() {
                        let text = name.text().present().expect("established required name");
                        if !charge(&mut budget.work, text.len() + 1) {
                            evaluation.areas[6] = Status::Undetermined;
                            break;
                        }
                        let mut duplicate = false;
                        for previous in list.children() {
                            if previous.node == name.node {
                                break;
                            }
                            if !charge(
                                &mut budget.work,
                                text.len() + previous.text().present().map_or(0, str::len) + 1,
                            ) {
                                evaluation.areas[6] = Status::Undetermined;
                                break;
                            }
                            duplicate |= previous.text().present() == Some(text);
                        }
                        if budget.work == 0 {
                            evaluation.areas[6] = Status::Undetermined;
                            break;
                        }
                        if duplicate {
                            continue;
                        }
                        match value_member(sample, text, &mut budget.work) {
                            Field::Absent => reject(
                                &mut evaluation,
                                "sample.required",
                                "required",
                                "Required map entry is absent",
                                Some(name.node),
                                budget,
                            ),
                            Field::Present(_) => {}
                            Field::Unavailable(_) => {
                                evaluation.areas[6] = Status::Undetermined;
                                break;
                            }
                        }
                    }
                }
            } else {
                evaluation.areas[6] = Status::Undetermined;
                evaluation.context = ContextState::Partial;
            }
            let property_meaning = match properties {
                Field::Absent => true,
                Field::Present(properties) => {
                    description_members_known(properties, &mut budget.work)
                }
                Field::Unavailable(_) => false,
            };
            if !property_meaning {
                evaluation.areas[6] = Status::Undetermined;
                evaluation.context = ContextState::Partial;
            }
            let children = properties
                .present()
                .filter(|_| identities && property_meaning)
                .map(EffectiveJson::children);
            kind = FrameKind::Map {
                required,
                properties,
                children,
                awaiting: false,
                established: presence && property_meaning,
            };
            evaluation.finish_value();
        }
        if evaluation.areas[1] == Status::Assessed
            && evaluation.type_null_basis == Basis::Array
            && sample.kind() == JsonKind::Array
        {
            let items = json.lookup("items", budget);
            // Item meaning is requested by actual elements, including each choice witness.
            let established = match items {
                Field::Absent => true,
                Field::Present(item) => {
                    sample.children().next().is_none() || item.kind() == JsonKind::Map
                }
                Field::Unavailable(_) => sample.children().next().is_none(),
            };
            kind = FrameKind::Array {
                items,
                children: items
                    .present()
                    .filter(|item| item.kind() == JsonKind::Map)
                    .map(|_| sample.children()),
                awaiting: false,
                established,
            };
            if !established {
                evaluation.areas[7] = Status::Undetermined;
                evaluation.context = ContextState::Partial;
            }
            evaluation.finish_value();
        }
    }
    Frame {
        json,
        pair_root,
        sample,
        kind,
        alternatives,
        evaluation,
        unknown: false,
        seen: false,
    }
}
fn interrupted(mut frames: Vec<Frame<'_>>, budget: &mut OperationBudget, cause: u8) -> Evaluation {
    let mut result = Evaluation::unavailable_cause(cause);
    while let Some(mut frame) = frames.pop() {
        if matches!(frame.kind, FrameKind::Choice) {
            frame.evaluation.choice_unfinished();
        } else {
            merge(&mut frame.evaluation, result, &mut frame.unknown, budget);
        }
        frame
            .evaluation
            .admission_unfinished(0, Status::Undetermined, cause);
        frame.evaluation.finish_value();
        result = frame.evaluation;
    }
    result
}
fn merge(
    target: &mut Evaluation,
    source: Evaluation,
    unknown: &mut bool,
    budget: &mut OperationBudget,
) {
    target.conjoin_child(&source);
    *unknown |= source.admission == Admission::Undetermined;
    if !charge(&mut budget.work, source.findings.len()) {
        target.storage_omitted = true;
        return;
    }
    for finding in source.findings {
        if push(&mut target.findings, finding, &mut budget.allocation).is_err() {
            target.storage_omitted = true;
        }
    }
}
fn evaluate_leaf<'a>(
    context: &ValueContext<'a>,
    json: EffectiveJson<'a>,
    sample: BorrowedValue<'a>,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
) {
    state.sample_node = sample.node();
    if !sample.available() {
        *state = Evaluation::unavailable();
        state.sample_node = sample.node();
        return;
    }
    if json.n().role == Some(crate::role::Role::DirectItem) {
        let object = json
            .lookup("type", budget)
            .present()
            .and_then(|v| v.text().present())
            == Some("object");
        let mut known = true;
        // Undeclared extension vocabulary passes this gate. The contexts established below
        // record the ADR 0037 reading only when the conclusion then proceeds.
        for child in json.children() {
            if !charge(&mut budget.work, child.n().key.map_or(1, str::len) + 1) {
                known = false;
                break;
            }
            known &= child.n().key.is_some_and(|key| {
                !crate::role::Role::DirectItem
                    .vocabulary(key, object)
                    .blocks_base_meaning()
            });
        }
        known &= json
            .lookup("type", budget)
            .present()
            .and_then(|v| v.text().present())
            != Some("array");
        if !known || sample.kind() == JsonKind::Null {
            state.areas[0] = Status::Undetermined;
            state.areas[1] = Status::Undetermined;
            state.context = ContextState::Partial;
            return state.finish_value();
        }
    }
    let scalar_context = if ordinary_context(json, budget) {
        Context::Ordinary
    } else {
        scalar_format_context(json, budget)
    };
    let scalar_format_known = matches!(
        scalar_context,
        Context::Temporal(_) | Context::Resource(_) | Context::Bytes | Context::Unix
    );
    let ordinary = scalar_context != Context::Unavailable;
    let abstract_value = ["sdfType", "format", "contentFormat"]
        .into_iter()
        .any(|key| exists(json, key, budget));
    match json.lookup("const", budget) {
        Field::Absent => {}
        Field::Present(constant) => {
            let decision = equality.compare_constant(context, sample, constant, work, budget);
            state.const_comparison(equality.supporting);
            match decision.verdict {
                Verdict::Equal => {
                    state.const_established();
                }
                Verdict::Different => {
                    if let Some((decisive_sample, decisive_constant)) = decision.constant_pair() {
                        state.const_established();
                        let original = state.sample_node;
                        state.sample_node = decisive_sample.node();
                        reject(
                            state,
                            "sample.const",
                            "const",
                            "Sample differs from the effective constant",
                            Some(decisive_constant.node),
                            budget,
                        );
                        state.sample_node = original;
                    } else {
                        state.areas[4] = Status::Undetermined;
                    }
                }
                Verdict::Unavailable => {
                    state.const_unfinished(equality.limitations);
                }
            }
        }
        Field::Unavailable(_) => state.areas[4] = Status::Undetermined,
    }
    if abstract_value && !scalar_format_known {
        state.areas[8] = Status::Unassessed;
        state.abstract_unknown |= 1;
    }
    let type_parameter = json.lookup("type", budget);
    let kind = type_parameter
        .present()
        .and_then(|value| value.text().present());
    let known_type = matches!(
        kind,
        Some("number" | "integer" | "boolean" | "string" | "object" | "array")
    );
    let absent_type = matches!(type_parameter, Field::Absent);
    let unsupported_structure = absent_type
        && (exists(json, "items", budget)
            || exists(json, "properties", budget)
            || exists(json, "required", budget));
    let vocabulary_ordinary = ordinary_map_context(json, budget) || scalar_format_known;
    let map_ordinary = !matches!(kind, Some("object" | "array")) || vocabulary_ordinary;
    if !vocabulary_ordinary && !matches!(kind, Some("object" | "array")) {
        state.areas[8] = Status::Undetermined;
        state.context = ContextState::Partial;
    }
    if (!known_type && !absent_type)
        || abstract_value && !scalar_format_known
        || !ordinary
        || unsupported_structure
        || !map_ordinary
    {
        state.areas[1] = Status::Undetermined;
        state.areas[2] = Status::Undetermined;
        if matches!(kind, Some("string" | "boolean")) {
            state.areas[3] = Status::Undetermined;
        }
        if !known_type && !absent_type || !ordinary || unsupported_structure || !map_ordinary {
            for area in [3, 6, 7, 8] {
                state.areas[area] = Status::Undetermined;
            }
        }
        state.context = ContextState::Partial;
        return state.finish_value();
    }
    let root = context.description;
    if root
        .n()
        .parent
        .is_some_and(|parent| root.model.nodes[parent].key == Some("properties"))
        && !ordinary_map_context(json, budget)
        && !scalar_format_known
    {
        state.areas[8] = Status::Undetermined;
        state.context = ContextState::Partial;
    }
    state.areas[1] = if absent_type {
        Status::NotApplicable
    } else {
        Status::Assessed
    };
    state.type_null_basis = match kind.or(match sample.kind() {
        JsonKind::String => Some("string"),
        JsonKind::Boolean => Some("boolean"),
        _ => None,
    }) {
        Some("string") => Basis::Text,
        Some("boolean") => Basis::Boolean,
        Some("object") => Basis::Map,
        Some("array") => Basis::Array,
        _ => Basis::Numeric,
    };
    if sample.kind() == JsonKind::Null {
        state.areas[1] = Status::Assessed;
        state.type_null_basis = Basis::TypeNull;
        match json.lookup("nullable", budget) {
            Field::Absent => {}
            Field::Present(value) => match value.boolean() {
                Field::Present(true) => {}
                Field::Present(false) => reject(
                    state,
                    "sample.null",
                    "nullable",
                    "Null is not admitted",
                    Some(value.node),
                    budget,
                ),
                _ => {
                    state.areas[1] = Status::Undetermined;
                    state.context = ContextState::Partial;
                }
            },
            _ => {
                state.areas[1] = Status::Undetermined;
                state.context = ContextState::Partial;
            }
        }
        return state.finish_value();
    }
    // A malformed nullable quality is needed only for a supplied null value.
    let abstract_context = matches!(scalar_context, Context::Bytes | Context::Unix);
    if abstract_context {
        abstract_sample_value(json, sample, scalar_context, state, budget, work, equality);
        if scalar_context == Context::Bytes
            && [
                "minimum",
                "maximum",
                "exclusiveMinimum",
                "exclusiveMaximum",
                "multipleOf",
            ]
            .into_iter()
            .any(|key| exists(json, key, budget))
        {
            state.admission_unfinished(2, Status::Unassessed, 1);
        }
        if scalar_context == Context::Unix
            && ["minLength", "maxLength", "pattern"]
                .into_iter()
                .any(|key| exists(json, key, budget))
        {
            state.admission_unfinished(3, Status::Unassessed, 1);
        }
    }
    let applicable = kind
        .or(match scalar_context {
            Context::Bytes => Some("string"),
            Context::Unix => Some("number"),
            _ => None,
        })
        .or(match sample.kind() {
            JsonKind::Number => Some("number"),
            JsonKind::String => Some("string"),
            JsonKind::Boolean => Some("boolean"),
            JsonKind::Map => Some("object"),
            JsonKind::Array => Some("array"),
            JsonKind::Null => None,
        });
    match applicable {
        Some("number" | "integer") => {
            if sample.kind() != JsonKind::Number {
                if !absent_type {
                    reject(
                        state,
                        "sample.type",
                        "type",
                        "Sample does not have the numerical base type",
                        type_parameter.present().map(|v| v.node),
                        budget,
                    );
                }
            } else if let Field::Present(number) = sample.number() {
                if kind == Some("integer") {
                    if work.debit(1).is_err() {
                        state.admission_unfinished(1, Status::Undetermined, 4);
                    } else if !number.is_integer() {
                        reject(
                            state,
                            "sample.integer",
                            "type",
                            "Sample is not mathematically integral",
                            type_parameter.present().map(|v| v.node),
                            budget,
                        );
                    }
                }
                let has_numeric = [
                    "minimum",
                    "maximum",
                    "exclusiveMinimum",
                    "exclusiveMaximum",
                    "multipleOf",
                ]
                .into_iter()
                .any(|key| exists(json, key, budget));
                state.areas[2] = if has_numeric {
                    Status::Assessed
                } else {
                    Status::NotApplicable
                };
                // Bounds on POSIX coordinates need both nonnegative denotations.
                // Integer integrality above is an independent base-type rule.
                if scalar_context == Context::Unix && state.areas[8] != Status::Assessed {
                    if has_numeric {
                        state.areas[2] = Status::Undetermined;
                    }
                    return state.finish_value();
                }
                for (key, lower, exclusive) in [
                    ("minimum", true, false),
                    ("maximum", false, false),
                    ("exclusiveMinimum", true, true),
                    ("exclusiveMaximum", false, true),
                ] {
                    match json.lookup(key, budget) {
                        Field::Absent => {}
                        Field::Present(parameter) => match parameter.number() {
                            Field::Present(bound)
                                if scalar_context == Context::Unix && bound.sign().is_lt() =>
                            {
                                state.admission_unfinished(2, Status::Undetermined, 2);
                            }
                            Field::Present(bound) => match number.compare_with_work(bound, work) {
                                Ok(order)
                                    if if lower {
                                        order.is_lt() || exclusive && order.is_eq()
                                    } else {
                                        order.is_gt() || exclusive && order.is_eq()
                                    } =>
                                {
                                    reject(
                                        state,
                                        "sample.bound",
                                        key,
                                        "Sample violates an exact numerical bound",
                                        Some(parameter.node),
                                        budget,
                                    )
                                }
                                Ok(_) => {}
                                Err(_) => {
                                    state.admission_unfinished(2, Status::Undetermined, 4);
                                }
                            },
                            _ => {
                                state.areas[2] = Status::Undetermined;
                                state.context = ContextState::Partial;
                            }
                        },
                        _ => {
                            state.areas[2] = Status::Undetermined;
                            state.context = ContextState::Partial;
                        }
                    }
                }
                match json.lookup("multipleOf", budget) {
                    Field::Absent => {}
                    Field::Present(parameter) => match parameter.number() {
                        Field::Present(step) if step.sign().is_gt() => {
                            match crate::numeric::multiple(
                                number,
                                step,
                                work,
                                &mut budget.allocation,
                            ) {
                                Ok(false) => reject(
                                    state,
                                    "sample.multiple",
                                    "multipleOf",
                                    "Sample is not an exact multiple of the positive step",
                                    Some(parameter.node),
                                    budget,
                                ),
                                Ok(true) => {}
                                Err(_) => {
                                    state.admission_unfinished(2, Status::Undetermined, 4);
                                }
                            }
                        }
                        _ => {
                            state.areas[2] = Status::Undetermined;
                            state.context = ContextState::Partial;
                        }
                    },
                    _ => {
                        state.areas[2] = Status::Undetermined;
                        state.context = ContextState::Partial;
                    }
                }
            } else {
                state.areas[0] = Status::Undetermined;
                state.areas[1] = Status::Undetermined;
                state.areas[2] = Status::Undetermined;
            }
        }
        Some("string" | "boolean") => {
            state.areas[3] = Status::Assessed;
            state.text_boolean_basis = if applicable == Some("string") {
                Basis::Text
            } else {
                Basis::Boolean
            };
            let expected = if applicable == Some("string") {
                JsonKind::String
            } else {
                JsonKind::Boolean
            };
            if sample.kind() != expected {
                if !absent_type {
                    reject(
                        state,
                        "sample.type",
                        "type",
                        "Sample does not have the selected base type",
                        type_parameter.present().map(|value| value.node),
                        budget,
                    );
                }
            } else if applicable == Some("string") {
                if scalar_context == Context::Bytes {
                    if ["minLength", "maxLength", "pattern"]
                        .into_iter()
                        .any(|key| exists(json, key, budget))
                    {
                        state.admission_unfinished(3, Status::Unassessed, 1);
                    }
                } else {
                    text_lengths(json, sample, state, budget, work);
                    text_pattern(json, sample, state, budget);
                }
                match scalar_context {
                    Context::Temporal(format) => {
                        temporal_value(json, sample, format, state, budget, equality);
                    }
                    Context::Resource(format) => {
                        resource_value(json, sample, format, state, budget, equality);
                    }
                    _ => {}
                }
            }
        }
        Some("object" | "array") => {
            let expected = if applicable == Some("object") {
                JsonKind::Map
            } else {
                JsonKind::Array
            };
            if sample.kind() == expected {
                if expected == JsonKind::Map && !absent_type {
                    state.areas[6] = Status::Assessed;
                } else if !absent_type {
                    state.areas[7] = Status::Assessed;
                    array_counts(json, sample, state, budget, work);
                    unique_items(context, json, sample, state, budget, work, equality);
                } else if ordinary_untyped_container(json, budget) {
                    state.areas[if expected == JsonKind::Map { 6 } else { 7 }] = Status::Assessed;
                    if expected == JsonKind::Array {
                        array_counts(json, sample, state, budget, work);
                        unique_items(context, json, sample, state, budget, work, equality);
                    }
                } else {
                    state.areas[if expected == JsonKind::Map { 6 } else { 7 }] =
                        Status::Undetermined;
                    state.context = ContextState::Partial;
                }
            } else {
                reject(
                    state,
                    "sample.type",
                    "type",
                    "Sample does not have the selected base type",
                    type_parameter.present().map(|v| v.node),
                    budget,
                );
            }
        }
        _ => {}
    }
    state.finish_value()
}
fn text_lengths(
    json: EffectiveJson<'_>,
    sample: BorrowedValue<'_>,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
) {
    let Some(text) = sample.text().present() else {
        state.areas[0] = Status::Undetermined;
        state.areas[1] = Status::Undetermined;
        state.areas[3] = Status::Undetermined;
        return;
    };
    let bounds = [
        ("minLength", true, json.lookup("minLength", budget)),
        ("maxLength", false, json.lookup("maxLength", budget)),
    ];
    if bounds
        .iter()
        .all(|(_, _, bound)| matches!(bound, Field::Absent))
    {
        return;
    }
    // One shared semantic debit covers every UTF-8 byte before scalar traversal.
    if !charge(&mut budget.work, text.len()) {
        state.areas[3] = Status::Undetermined;
        return;
    }
    let Some(count) = text.chars().try_fold(0u32, |count, _| count.checked_add(1)) else {
        state.areas[3] = Status::Undetermined;
        return;
    };
    for (key, lower, parameter) in bounds {
        match parameter {
            Field::Absent => {}
            Field::Present(parameter) => match parameter.number() {
                Field::Present(bound) if bound.is_integer() && !bound.sign().is_lt() => {
                    match compare_count(count, bound, work) {
                        Ok(order) if if lower { order.is_lt() } else { order.is_gt() } => reject(
                            state,
                            "sample.length",
                            key,
                            "Sample violates a Unicode scalar text length bound",
                            Some(parameter.node),
                            budget,
                        ),
                        Ok(_) => {}
                        Err(_) => state.areas[3] = Status::Undetermined,
                    }
                }
                _ => {
                    state.areas[3] = Status::Undetermined;
                    state.context = ContextState::Partial;
                }
            },
            _ => {
                state.areas[3] = Status::Undetermined;
                state.context = ContextState::Partial;
            }
        }
    }
}
fn text_pattern(
    json: EffectiveJson<'_>,
    sample: BorrowedValue<'_>,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
) {
    let parameter = match json.lookup("pattern", budget) {
        Field::Absent => return,
        Field::Present(parameter) => parameter,
        Field::Unavailable(_) => {
            state.areas[3] = Status::Undetermined;
            return;
        }
    };
    let (Some(pattern), Some(text)) = (parameter.text().present(), sample.text().present()) else {
        state.areas[3] = Status::Undetermined;
        return;
    };
    let model_pattern = if matches!(sample, BorrowedValue::Model(_)) {
        let Some(index) = budget.patterns.model_pattern(
            json.model.source,
            parameter.node,
            &mut budget.work,
            &mut budget.allocation,
        ) else {
            state.areas[3] = Status::Undetermined;
            return;
        };
        Some(index)
    } else {
        None
    };
    match crate::pattern::matches(pattern, text, &json.model.catalogue.limits, budget) {
        crate::pattern::MatchVerdict::Matched => {}
        crate::pattern::MatchVerdict::Mismatched => reject(
            state,
            "sample.pattern",
            "pattern",
            "Sample text does not match the effective Unicode ECMAScript pattern",
            Some(parameter.node),
            budget,
        ),
        verdict => {
            state.areas[3] = Status::Undetermined;
            state.context = ContextState::Partial;
            let (basis, code, message) = match verdict {
                crate::pattern::MatchVerdict::Invalid => {
                    if let Some(index) = model_pattern {
                        budget.patterns.model_patterns[index].invalid = true;
                        budget.patterns.reported_models =
                            budget.patterns.reported_models.min(index);
                    }
                    return;
                }
                crate::pattern::MatchVerdict::Unsupported => (
                    Basis::PatternCapability,
                    "pattern.unsupported",
                    "Pattern matching requires a capability outside the qualified Unicode ECMAScript subset",
                ),
                _ => (
                    Basis::PatternWork,
                    "operation.limit",
                    "Pattern matching reached its finite operation or engine capacity",
                ),
            };
            state.total_findings += 1;
            if push(
                &mut state.findings,
                Finding {
                    sample_node: sample.node(),
                    paired_sample: None,
                    basis,
                    code,
                    constraint: "pattern",
                    message,
                    parameter: Some(parameter.node),
                    alternative: None,
                },
                &mut budget.allocation,
            )
            .is_err()
            {
                state.storage_omitted = true;
            }
        }
    }
}
fn array_counts(
    json: EffectiveJson<'_>,
    sample: BorrowedValue<'_>,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
) {
    let mut count = 0u32;
    for _ in sample.children() {
        if !charge(&mut budget.work, 1) {
            state.areas[7] = Status::Undetermined;
            return;
        }
        let Some(next) = count.checked_add(1) else {
            state.areas[7] = Status::Undetermined;
            return;
        };
        count = next;
    }
    for (key, lower) in [("minItems", true), ("maxItems", false)] {
        match json.lookup(key, budget) {
            Field::Absent => {}
            Field::Present(parameter) => match parameter.number() {
                Field::Present(bound) if bound.is_integer() && !bound.sign().is_lt() => {
                    match compare_count(count, bound, work) {
                        Ok(order) if if lower { order.is_lt() } else { order.is_gt() } => reject(
                            state,
                            "sample.count",
                            key,
                            "Sample violates an inclusive array count bound",
                            Some(parameter.node),
                            budget,
                        ),
                        Ok(_) => {}
                        Err(_) => state.areas[7] = Status::Undetermined,
                    }
                }
                _ => {
                    state.areas[7] = Status::Undetermined;
                    state.context = ContextState::Partial;
                }
            },
            Field::Unavailable(_) => {
                state.areas[7] = Status::Undetermined;
                state.context = ContextState::Partial;
            }
        }
    }
}
fn unique_items<'a>(
    context: &ValueContext<'a>,
    json: EffectiveJson<'a>,
    sample: BorrowedValue<'a>,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
    work: &mut crate::NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
) {
    match json.lookup("uniqueItems", budget) {
        Field::Absent => {}
        Field::Present(value) => match value.boolean() {
            Field::Present(false) => {}
            Field::Present(true) => {
                let mut unknown = false;
                let mut prepared = crate::equality::PreparedItems::new(*context, sample);
                for second in sample.children() {
                    if !charge(&mut budget.work, 1) {
                        state.areas[7] = Status::Undetermined;
                        return;
                    }
                    for first in sample.children() {
                        if first.node() == second.node() {
                            break;
                        }
                        if equality.remaining() == 0 || budget.work == 0 {
                            state.areas[7] = Status::Undetermined;
                            return;
                        }
                        let decision =
                            equality.compare_items(&mut prepared, first, second, work, budget);
                        match decision.verdict {
                            Verdict::Equal => {
                                if let Some((first, second)) = decision.item_pair() {
                                    state.exclude_equal_items(equality.supporting);
                                    if push(
                                        &mut state.findings,
                                        pair_finding(second.node(), first.node(), value.node),
                                        &mut budget.allocation,
                                    )
                                    .is_err()
                                    {
                                        state.storage_omitted = true;
                                    }
                                    return;
                                }
                                unknown = true;
                            }
                            Verdict::Unavailable => {
                                state.items_comparison_unfinished(equality.limitations);
                                unknown = true;
                            }
                            Verdict::Different => {}
                        }
                    }
                }
                if unknown {
                    state.areas[7] = Status::Undetermined;
                }
            }
            _ => {
                state.areas[7] = Status::Undetermined;
                state.context = ContextState::Partial;
            }
        },
        Field::Unavailable(_) => {
            state.areas[7] = Status::Undetermined;
            state.context = ContextState::Partial;
        }
    }
}
fn compare_count(
    count: u32,
    bound: crate::ExactNumber<'_>,
    work: &mut crate::NumberWork,
) -> Result<std::cmp::Ordering, &'static str> {
    // Ten stack digits cover a checked count without narrowing the retained exact bound.
    work.debit(10 + bound.decimal.coefficient.len().max(10) + 1)?;
    if count == 0 {
        return Ok(bound.sign().reverse());
    }
    if bound.sign().is_eq() {
        return Ok(std::cmp::Ordering::Greater);
    }
    let mut digits = [b'0'; 10];
    let mut first = digits.len();
    let mut remaining = count;
    while remaining > 0 {
        first -= 1;
        digits[first] += (remaining % 10) as u8;
        remaining /= 10;
    }
    let digits = &digits[first..];
    let coefficient = bound.decimal.coefficient.as_bytes();
    let magnitude =
        (digits.len() as i128).cmp(&(coefficient.len() as i128 + i128::from(bound.decimal.power)));
    if !magnitude.is_eq() {
        return Ok(magnitude);
    }
    for index in 0..digits.len().max(coefficient.len()) {
        let order = digits
            .get(index)
            .copied()
            .unwrap_or(b'0')
            .cmp(&coefficient.get(index).copied().unwrap_or(b'0'));
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(std::cmp::Ordering::Equal)
}
fn abstract_sample_value<'a>(
    json: EffectiveJson<'a>,
    sample: BorrowedValue<'a>,
    context: Context,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
    number: &mut crate::NumberWork,
    equality: &mut crate::equality::EqualityWork<'a>,
) {
    use abstract_value::Established;
    state.begin_abstract_meaning(abstract_value::basis(context));
    let parameter = json
        .lookup("sdfType", budget)
        .present()
        .map(|value| value.node);
    let (basis, code, message) = match abstract_value::establish(
        context,
        sample,
        number,
        budget,
        &mut equality.abstract_use,
    ) {
        Established::Value => {
            state.abstract_established();
            return;
        }
        Established::Representation => (
            state.abstract_basis,
            if context == Context::Bytes {
                "sample.byte_string_representation"
            } else {
                "sample.unix_time_representation"
            },
            "Sample does not use the selected JSON representation",
        ),
        Established::Unavailable(cause) => {
            state.abstract_unfinished(cause);
            match cause {
                2 => (
                    Basis::AbstractUnavailable,
                    "sample.abstract_denotation_unavailable",
                    "The selected convention does not establish this abstract value",
                ),
                4 => (
                    Basis::AbstractNumberWork,
                    "operation.limit",
                    "Abstract numerical work limit reached",
                ),
                0 => (
                    Basis::AbstractEstablishment,
                    "sample.coverage_unavailable",
                    "Abstract scalar input is unavailable",
                ),
                _ => (
                    Basis::AbstractWork,
                    "operation.limit",
                    "Abstract representation work limit reached",
                ),
            }
        }
    };
    if !basis.incomplete() {
        state.abstract_excluded(basis);
    }
    if push(
        &mut state.findings,
        Finding {
            sample_node: sample.node(),
            paired_sample: None,
            basis,
            code,
            constraint: "sdfType",
            message,
            parameter,
            alternative: None,
        },
        &mut budget.allocation,
    )
    .is_err()
    {
        state.storage_omitted = true;
    }
}
fn resource_value<'a>(
    json: EffectiveJson<'a>,
    sample: BorrowedValue<'a>,
    format: ResourceFormat,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
    equality: &mut crate::equality::EqualityWork<'a>,
) {
    use crate::formats::Evaluation as Result;
    state.begin_abstract_meaning(if format == ResourceFormat::Uuid {
        Basis::UuidValue
    } else {
        Basis::Resource
    });
    let parameter = json
        .lookup("format", budget)
        .present()
        .map(|value| value.node);
    let (basis, code, message) = match sample.text() {
        Field::Present(text) => {
            match crate::formats::evaluate(format, text, &mut budget.work, &mut equality.format_use)
            {
                Result::Established(_) => {
                    state.abstract_established();
                    return;
                }
                Result::Syntax => (
                    Basis::UriSyntax,
                    "sample.uri_representation",
                    "String does not use the selected URI or reference representation",
                ),
                Result::Http => (
                    Basis::HttpIdentity,
                    "sample.http_identity",
                    "HTTP resource identifier requires authority and a nonempty host",
                ),
                Result::UuidText => (
                    Basis::UuidText,
                    "sample.uuid_representation",
                    "String does not use the selected bare UUID representation",
                ),
                Result::MeaningUnavailable => (
                    Basis::ResourceMeaning,
                    "sample.resource_meaning_unavailable",
                    "Needed scheme-specific resource meaning is unavailable",
                ),
                Result::WorkUnavailable => (
                    Basis::ResourceWork,
                    "operation.limit",
                    "Resource representation work limit reached",
                ),
            }
        }
        _ => (
            Basis::ResourceWork,
            "operation.limit",
            "Resource representation input unavailable",
        ),
    };
    if basis.incomplete() {
        state.abstract_unfinished(if basis == Basis::ResourceMeaning {
            8
        } else {
            3
        });
    } else {
        state.abstract_excluded(basis);
    }
    if push(
        &mut state.findings,
        Finding {
            sample_node: sample.node(),
            paired_sample: None,
            basis,
            code,
            constraint: "format",
            message,
            parameter,
            alternative: None,
        },
        &mut budget.allocation,
    )
    .is_err()
    {
        state.storage_omitted = true;
    }
}

fn temporal_value<'a>(
    json: EffectiveJson<'a>,
    sample: BorrowedValue<'a>,
    format: TemporalFormat,
    state: &mut Evaluation,
    budget: &mut OperationBudget,
    equality: &mut crate::equality::EqualityWork<'a>,
) {
    use crate::civil::{Establishment, Invalid, Unavailable};
    state.begin_abstract_meaning(Basis::Temporal);
    crate::civil::begin_attempt(&mut equality.temporal_use);
    let parameter = json
        .lookup("format", budget)
        .present()
        .map(|value| value.node);
    let result = crate::civil::establish(
        format,
        sample.text(),
        &mut budget.work,
        &mut equality.temporal_use,
    );
    let (basis, code, message) = match result {
        Establishment::Established => {
            state.abstract_established();
            return;
        }
        Establishment::Invalid(Invalid::Representation) => (
            Basis::TemporalRepresentation,
            "sample.temporal_representation",
            "String does not use the selected civil representation",
        ),
        Establishment::Invalid(Invalid::Calendar) => (
            Basis::TemporalCalendar,
            "sample.temporal_calendar",
            "Civil date does not exist in the proleptic Gregorian calendar",
        ),
        Establishment::Invalid(Invalid::Clock) => (
            Basis::TemporalClock,
            "sample.temporal_clock",
            "Civil clock or offset is out of range",
        ),
        Establishment::Invalid(Invalid::ExceptionalSlot) => (
            Basis::TemporalSlot,
            "sample.temporal_exceptional_slot",
            "Second 60 is outside a possible UTC exceptional slot",
        ),
        Establishment::Invalid(Invalid::NoInsertedSecond) => (
            Basis::TemporalLeap,
            "sample.temporal_leap_second",
            "No positive leap second is recorded for this UTC slot",
        ),
        Establishment::Unavailable(Unavailable::Knowledge) => (
            Basis::TemporalKnowledge,
            "sample.temporal_knowledge_unavailable",
            "Civil-time knowledge does not establish this slot",
        ),
        Establishment::Unavailable(Unavailable::Meaning) => (
            Basis::TemporalMeaning,
            "sample.temporal_meaning_unavailable",
            "A date-free time cannot establish a positive leap second",
        ),
        Establishment::Unavailable(Unavailable::Input | Unavailable::Work) => (
            Basis::TemporalWork,
            "operation.limit",
            "Civil representation work limit reached",
        ),
    };
    if basis.incomplete() {
        state.abstract_unfinished(match basis {
            Basis::TemporalKnowledge => 6,
            Basis::TemporalMeaning => 7,
            _ => 3,
        });
    } else {
        state.abstract_excluded(basis);
    }
    // The format was established by the same charged semantic lookup. Keep its
    // genuine token even if the subsequent parser has exhausted shared work.
    if push(
        &mut state.findings,
        Finding {
            sample_node: sample.node(),
            paired_sample: None,
            basis,
            code,
            constraint: "format",
            message,
            parameter,
            alternative: None,
        },
        &mut budget.allocation,
    )
    .is_err()
    {
        state.storage_omitted = true;
    }
}
fn reject(
    state: &mut Evaluation,
    code: &'static str,
    constraint: &'static str,
    message: &'static str,
    parameter: Option<usize>,
    budget: &mut OperationBudget,
) {
    let basis = if code == "sample.const" {
        Basis::Const
    } else if code == "sample.alternatives" {
        Basis::Alternatives
    } else if code == "sample.null" {
        Basis::Nullable
    } else {
        state.type_null_basis
    };
    state.exclude_constraint(basis);
    if push(
        &mut state.findings,
        Finding {
            sample_node: state.sample_node,
            paired_sample: None,
            basis,
            code,
            constraint,
            message,
            parameter,
            alternative: None,
        },
        &mut budget.allocation,
    )
    .is_err()
    {
        state.storage_omitted = true;
    }
}
pub(crate) fn seal_meaning(state: &mut State, context_cause: Option<u8>) {
    state.seal_request(context_cause);
}
pub struct SampleResult<'a> {
    pub(crate) state: &'a State,
    pub(crate) catalogue: &'a Catalogue,
    pub(crate) model: Option<&'a EffectiveModel<'a>>,
}
impl<'a> SampleResult<'a> {
    pub fn admission(&self) -> Admission {
        self.state.admission
    }
    pub fn sample_id(&self) -> SourceId {
        self.state.sample
    }
    pub fn selected_kind(&self) -> SampleKind {
        self.state.kind
    }
    pub fn selected_document(&self) -> SourceId {
        self.state.model
    }
    pub fn selected_pointer(&self) -> &str {
        &self.state.pointer
    }
    pub fn context_state(&self) -> ContextState {
        self.state.context
    }
    pub fn context_findings(&self) -> ContextFindings {
        self.state.context_findings
    }
    pub fn sample_value(&self) -> Field<SampleValueView<'a>> {
        self.catalogue
            .source(self.state.sample)
            .expect("sample identity")
            .sample_value()
    }
    pub fn description(&self) -> Field<EffectiveDataDescription<'a>> {
        self.model.and_then(|model| model.json().present()).map_or(
            Field::Unavailable("selected description unavailable"),
            |json| Field::Present(EffectiveDataDescription::selected(json)),
        )
    }
    pub fn area_status(&self, id: &str) -> Field<Status> {
        AREAS
            .iter()
            .position(|area| *area == id)
            .map_or(Field::Absent, |index| {
                Field::Present(self.state.areas[index])
            })
    }
}
