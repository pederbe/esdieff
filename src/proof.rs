//! Four local contradictions in established effective Full Data Descriptions.
//! Follows ADR 0003: Check findings and completeness, and ADR 0009: Bounded processing and reporting.
use crate::numeric::NumericCapability;
use crate::structure::Rule;
use crate::{BaseType, EffectiveModel, Field, JsonKind, NumberWork};
use std::num::NonZeroUsize;

pub(crate) const AREA: &str = "advisory-four/1";
pub(crate) const NUMERIC_AREA: &str = "advisory-numeric-three/1";

#[derive(Clone, Copy)]
pub(crate) enum NumericClass {
    IntegerGap,
    ConstBound,
    StepGap,
}
impl NumericClass {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::IntegerGap => "proof.integer_interval_gap",
            Self::ConstBound => "proof.numeric_const_bound",
            Self::StepGap => "proof.step_interval_gap",
        }
    }
}

const TYPE: usize = 0;
const NULLABLE: usize = 1;
const ENUM: usize = 2;
const CHOICE: usize = 3;
const SDF_TYPE: usize = 4;
const FORMAT: usize = 5;
const MINIMUM: usize = 6;
const MAXIMUM: usize = 7;
const EXCLUSIVE_MINIMUM: usize = 8;
const EXCLUSIVE_MAXIMUM: usize = 9;
const MIN_LENGTH: usize = 10;
const MAX_LENGTH: usize = 11;
const MIN_ITEMS: usize = 12;
const MAX_ITEMS: usize = 13;
const CONST: usize = 14;
const MULTIPLE_OF: usize = 15;

#[derive(Clone, Copy)]
pub(crate) enum ProofClass {
    NumericInterval,
    TextLengths,
    ArrayCounts,
    PrimitiveConst,
}
impl ProofClass {
    pub(crate) fn code(self) -> &'static str {
        self.warning_rule().code()
    }
    fn warning_rule(self) -> Rule {
        match self {
            Self::NumericInterval => Rule::EmptyNumericInterval,
            Self::TextLengths => Rule::InvertedTextLengths,
            Self::ArrayCounts => Rule::InvertedArrayCounts,
            Self::PrimitiveConst => Rule::PrimitiveConstType,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum NumericOperation {
    Compare,
    Integrality,
}
impl NumericOperation {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Compare => "compare",
            Self::Integrality => "integrality",
        }
    }
}

pub(crate) fn primitive_name(primitive: BaseType) -> &'static str {
    match primitive {
        BaseType::Number => "number",
        BaseType::Integer => "integer",
        BaseType::String => "string",
        BaseType::Boolean => "boolean",
        BaseType::Array => "array",
        BaseType::Map => "object",
    }
}
#[derive(Clone, Copy)]
pub(crate) struct OperandIndices([Option<NonZeroUsize>; 4]);
impl OperandIndices {
    pub(crate) fn new(nodes: [Option<usize>; 4]) -> Result<Self, &'static str> {
        let mut encoded = [None; 4];
        for (slot, node) in encoded.iter_mut().zip(nodes) {
            if let Some(node) = node {
                *slot = Some(
                    node.checked_add(1)
                        .and_then(NonZeroUsize::new)
                        .ok_or("proof operand identity unavailable")?,
                );
            }
        }
        Ok(Self(encoded))
    }

    pub(crate) fn at(self, position: usize) -> Option<usize> {
        self.0
            .get(position)
            .copied()
            .flatten()
            .map(|index| index.get() - 1)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = Option<usize>> + '_ {
        self.0
            .iter()
            .map(|index| index.map(|index| index.get() - 1))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Witness<'a> {
    pub(crate) operands: OperandIndices,
    pub(crate) primitive: Option<BaseType>,
    pub(crate) alternative: Option<crate::AlternativeId<'a>>,
}
impl<'a> Witness<'a> {
    fn for_scope(
        nodes: [Option<usize>; 4],
        primitive: BaseType,
        alternative: Option<crate::AlternativeId<'a>>,
        scope: &mut Scope,
    ) -> Option<Self> {
        let operands = match OperandIndices::new(nodes) {
            Ok(operands) => operands,
            Err(_) => {
                scope.unknown = true;
                return None;
            }
        };
        Some(Self {
            operands,
            primitive: Some(primitive),
            alternative,
        })
    }
}
#[derive(Default)]
pub(crate) struct Scope {
    pub(crate) applicable: bool,
    pub(crate) unknown: bool,
}
impl Scope {
    pub(crate) fn status(&self) -> crate::AssessmentStatus {
        if self.unknown {
            crate::AssessmentStatus::Undetermined
        } else if self.applicable {
            crate::AssessmentStatus::Assessed
        } else {
            crate::AssessmentStatus::NotApplicable
        }
    }
}
pub(crate) struct Evaluation<'a> {
    pub(crate) scope: Scope,
    // Each attempted pair or constant occupies at most one warning or limit slot.
    pub(crate) findings: [Option<(Rule, Witness<'a>)>; 5],
}
pub(crate) struct NumericEvaluation<'a> {
    pub(crate) scope: Scope,
    pub(crate) findings: [Option<(Rule, Witness<'a>)>; 6],
    /// Extension vocabulary the established evaluation set aside.
    pub(crate) set_aside: crate::role::SetAside,
}
#[derive(Clone, Copy)]
enum Quality {
    Absent,
    Known(usize),
    Unknown,
}
impl Quality {
    fn node(self) -> Option<usize> {
        if let Self::Known(node) = self {
            Some(node)
        } else {
            None
        }
    }
}
fn quality(model: &EffectiveModel<'_>, node: usize) -> Quality {
    let n = &model.nodes[node];
    if n.unavailable.is_some() || n.donor_unavailable.is_some() || n.meaning_unavailable.is_some() {
        Quality::Unknown
    } else {
        Quality::Known(node)
    }
}

pub(crate) fn evaluate<'a>(
    model: &EffectiveModel<'a>,
    node: usize,
    numeric: &mut NumberWork,
    work: &mut usize,
    alternative: Option<crate::AlternativeId<'a>>,
) -> Evaluation<'a> {
    let mut result = Evaluation {
        scope: Scope::default(),
        findings: [None; 5],
    };
    let n = &model.nodes[node];
    if n.unavailable.is_some() || n.donor_unavailable.is_some() {
        result.scope.unknown = true;
        return result;
    }
    if n.source.kind() != JsonKind::Map {
        return result;
    }
    let mut fields = [Quality::Absent; 15];
    if (crate::EffectiveJson { model, node }).context_data() {
        fields[NULLABLE] = Quality::Unknown;
    }
    let names = [
        "type",
        "nullable",
        "enum",
        "sdfChoice",
        "sdfType",
        "format",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
        "const",
    ];
    let mut child = n.first;
    while let Some(index) = child {
        let q = &model.nodes[index];
        child = q.next;
        if q.deleted {
            continue;
        }
        let cost = q
            .key
            .map_or(1, |key| key.len().saturating_mul(15).saturating_add(1));
        if !crate::allowance::charge(work, cost) {
            result.scope.unknown = true;
            return result;
        }
        let Some(key) = q.key else {
            result.scope.unknown = true;
            return result;
        };
        if let Some(position) = names.iter().position(|name| *name == key) {
            fields[position] = quality(model, index);
        }
    }
    // Presence excludes owners, including malformed selectors. Established derived branches are separate descriptions.
    if !matches!(fields[ENUM], Quality::Absent) || !matches!(fields[CHOICE], Quality::Absent) {
        result.scope.unknown = model.alternative_scope_unknown(node);
        return result;
    }
    let candidate = (!matches!(fields[MINIMUM], Quality::Absent)
        || !matches!(fields[EXCLUSIVE_MINIMUM], Quality::Absent))
        && (!matches!(fields[MAXIMUM], Quality::Absent)
            || !matches!(fields[EXCLUSIVE_MAXIMUM], Quality::Absent))
        || (!matches!(fields[MIN_LENGTH], Quality::Absent)
            && !matches!(fields[MAX_LENGTH], Quality::Absent))
        || (!matches!(fields[MIN_ITEMS], Quality::Absent)
            && !matches!(fields[MAX_ITEMS], Quality::Absent))
        || match fields[CONST] {
            Quality::Absent => false,
            Quality::Unknown => true,
            Quality::Known(node) => !matches!(
                model.nodes[node].source.kind(),
                JsonKind::Map | JsonKind::Array
            ),
        };
    if !candidate {
        return result;
    }
    let primitive = match fields[TYPE] {
        Quality::Known(index) => match model.nodes[index].source.text() {
            Field::Present("number") => BaseType::Number,
            Field::Present("integer") => BaseType::Integer,
            Field::Present("string") => BaseType::String,
            Field::Present("array") => BaseType::Array,
            Field::Present("boolean") => BaseType::Boolean,
            Field::Present("object") => return result,
            _ => {
                result.scope.unknown = true;
                return result;
            }
        },
        Quality::Unknown => {
            result.scope.unknown = true;
            return result;
        }
        Quality::Absent => return result,
    };
    if primitive != BaseType::Array {
        result.findings[4] = constant(
            model,
            &fields,
            primitive,
            numeric,
            &mut result.scope,
            alternative,
        );
    }
    if matches!(primitive, BaseType::String | BaseType::Array) {
        let (low_field, high_field, class) = if primitive == BaseType::String {
            (
                fields[MIN_LENGTH],
                fields[MAX_LENGTH],
                ProofClass::TextLengths,
            )
        } else {
            (
                fields[MIN_ITEMS],
                fields[MAX_ITEMS],
                ProofClass::ArrayCounts,
            )
        };
        if matches!(low_field, Quality::Absent) || matches!(high_field, Quality::Absent) {
            return result;
        }
        let Some(nullable_false) = nullable_false(model, fields[NULLABLE], &mut result.scope)
        else {
            return result;
        };
        if !nullable_false {
            return result;
        }
        result.scope.applicable = true;
        let known_format = match fields[FORMAT] {
            Quality::Absent => true,
            Quality::Known(index) => matches!(
                model.nodes[index].source.text(),
                Field::Present("uri" | "uri-reference" | "uuid" | "date" | "time" | "date-time")
            ),
            Quality::Unknown => false,
        };
        if !matches!(fields[SDF_TYPE], Quality::Absent)
            || (primitive == BaseType::String && !known_format)
        {
            result.scope.unknown = true;
            return result;
        }
        let (Some(a), Some(b)) = (low_field.node(), high_field.node()) else {
            result.scope.unknown = true;
            return result;
        };
        let (Field::Present(low), Field::Present(high)) = (
            model.nodes[a].source.number(),
            model.nodes[b].source.number(),
        ) else {
            result.scope.unknown = true;
            return result;
        };
        let Some(witness) = Witness::for_scope(
            [Some(a), Some(b), fields[NULLABLE].node(), None],
            primitive,
            alternative,
            &mut result.scope,
        ) else {
            return result;
        };
        if numeric.debit(2).is_err() {
            result.scope.unknown = true;
            result.findings[0] = Some((
                Rule::AdvisoryNumericLimit {
                    class,
                    operation: NumericOperation::Integrality,
                },
                witness,
            ));
            return result;
        }
        if !low.is_integer() || !high.is_integer() || low.sign().is_lt() || high.sign().is_lt() {
            result.scope.unknown = true;
            return result;
        }
        match low.compare(high, numeric) {
            Ok(order) if order.is_gt() => {
                result.findings[0] = Some((class.warning_rule(), witness))
            }
            Ok(_) => {}
            Err(_) => {
                result.scope.unknown = true;
                result.findings[0] = Some((
                    Rule::AdvisoryNumericLimit {
                        class,
                        operation: NumericOperation::Compare,
                    },
                    witness,
                ));
            }
        }
        return result;
    }
    if !matches!(primitive, BaseType::Number | BaseType::Integer) {
        return result;
    }
    let lower = [(fields[MINIMUM], false), (fields[EXCLUSIVE_MINIMUM], true)];
    let upper = [(fields[MAXIMUM], false), (fields[EXCLUSIVE_MAXIMUM], true)];
    let mut output = 0;
    for (low, exclusive_low) in lower {
        for (high, exclusive_high) in upper {
            if matches!(low, Quality::Absent) || matches!(high, Quality::Absent) {
                continue;
            }
            let Some(no_null) = nullable_false(model, fields[NULLABLE], &mut result.scope) else {
                continue;
            };
            if !no_null {
                continue;
            }
            result.scope.applicable = true;
            if !matches!(fields[SDF_TYPE], Quality::Absent)
                || matches!(low, Quality::Unknown)
                || matches!(high, Quality::Unknown)
            {
                result.scope.unknown = true;
                continue;
            }
            let (Some(a), Some(b)) = (low.node(), high.node()) else {
                unreachable!()
            };
            let (Field::Present(a_number), Field::Present(b_number)) = (
                model.nodes[a].source.number(),
                model.nodes[b].source.number(),
            ) else {
                result.scope.unknown = true;
                continue;
            };
            let Some(witness) = Witness::for_scope(
                [Some(a), Some(b), fields[NULLABLE].node(), None],
                primitive,
                alternative,
                &mut result.scope,
            ) else {
                continue;
            };
            match a_number.compare(b_number, numeric) {
                Ok(order)
                    if order.is_gt() || (order.is_eq() && (exclusive_low || exclusive_high)) =>
                {
                    result.findings[output] =
                        Some((ProofClass::NumericInterval.warning_rule(), witness));
                    output += 1;
                }
                Ok(_) => {}
                Err(_) => {
                    result.scope.unknown = true;
                    result.findings[output] = Some((
                        Rule::AdvisoryNumericLimit {
                            class: ProofClass::NumericInterval,
                            operation: NumericOperation::Compare,
                        },
                        witness,
                    ));
                    output += 1;
                }
            }
        }
    }
    result
}

fn constant<'a>(
    model: &EffectiveModel<'a>,
    fields: &[Quality; 15],
    primitive: BaseType,
    numeric: &mut NumberWork,
    scope: &mut Scope,
    alternative: Option<crate::AlternativeId<'a>>,
) -> Option<(Rule, Witness<'a>)> {
    let node = match fields[CONST] {
        Quality::Absent => return None,
        Quality::Unknown => {
            scope.applicable = true;
            scope.unknown = true;
            return None;
        }
        Quality::Known(node) => node,
    };
    let json = model.nodes[node].source;
    if matches!(json.kind(), JsonKind::Map | JsonKind::Array) {
        return None;
    }
    scope.applicable = true;
    if json.kind() == JsonKind::Null {
        return nullable_false(model, fields[NULLABLE], scope)
            .filter(|value| *value)
            .and_then(|_| {
                Witness::for_scope(
                    [Some(node), fields[NULLABLE].node(), None, None],
                    primitive,
                    alternative,
                    scope,
                )
                .map(|witness| (ProofClass::PrimitiveConst.warning_rule(), witness))
            });
    }
    if !matches!(fields[SDF_TYPE], Quality::Absent)
        || (primitive == BaseType::String && !matches!(fields[FORMAT], Quality::Absent))
    {
        scope.unknown = true;
        return None;
    }
    let expected = match primitive {
        BaseType::Number | BaseType::Integer => JsonKind::Number,
        BaseType::String => JsonKind::String,
        BaseType::Boolean => JsonKind::Boolean,
        _ => unreachable!(),
    };
    let mismatch = if json.kind() != expected {
        true
    } else if primitive == BaseType::Integer {
        let number = match json.number() {
            Field::Present(number) => number,
            _ => {
                scope.unknown = true;
                return None;
            }
        };
        if numeric.debit(1).is_err() {
            scope.unknown = true;
            return Some((
                Rule::AdvisoryNumericLimit {
                    class: ProofClass::PrimitiveConst,
                    operation: NumericOperation::Integrality,
                },
                Witness::for_scope(
                    [Some(node), fields[TYPE].node(), None, None],
                    primitive,
                    alternative,
                    scope,
                )?,
            ));
        }
        !number.is_integer()
    } else {
        false
    };
    mismatch
        .then(|| {
            Witness::for_scope(
                [Some(node), fields[TYPE].node(), None, None],
                primitive,
                alternative,
                scope,
            )
            .map(|witness| (ProofClass::PrimitiveConst.warning_rule(), witness))
        })
        .flatten()
}

fn nullable_false(model: &EffectiveModel<'_>, field: Quality, scope: &mut Scope) -> Option<bool> {
    match field {
        Quality::Absent => Some(false),
        Quality::Known(node) => match model.nodes[node].source.boolean() {
            Field::Present(value) => Some(!value),
            _ => {
                scope.unknown = true;
                None
            }
        },
        Quality::Unknown => {
            scope.unknown = true;
            None
        }
    }
}

/// Numeric advice uses final role and meaning, rather than a quality-name whitelist.
pub(crate) fn evaluate_numeric<'a>(
    model: &EffectiveModel<'a>,
    node: usize,
    numeric: &mut NumberWork,
    work: &mut usize,
    allocation: &mut usize,
    alternative: Option<crate::AlternativeId<'a>>,
) -> NumericEvaluation<'a> {
    let mut result = NumericEvaluation {
        scope: Scope::default(),
        findings: [None; 6],
        set_aside: Default::default(),
    };
    let n = &model.nodes[node];
    if n.unavailable.is_some()
        || n.donor_unavailable.is_some()
        || n.meaning_unavailable.is_some()
        || !model.clear_feature
    {
        result.scope.unknown = true;
        return result;
    }
    if n.source.kind() != JsonKind::Map {
        return result;
    }
    let Some(role) = n.role.filter(|role| role.full_data()) else {
        result.scope.unknown = true;
        return result;
    };
    let names = [
        "type",
        "nullable",
        "enum",
        "sdfChoice",
        "sdfType",
        "format",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
        "const",
        "multipleOf",
    ];
    let mut fields = [Quality::Absent; 16];
    if (crate::EffectiveJson { model, node }).context_data() {
        fields[NULLABLE] = Quality::Unknown;
    }
    let mut blocked = false;
    let mut set_aside = crate::role::SetAside::default();
    let mut child = n.first;
    while let Some(index) = child {
        let q = &model.nodes[index];
        child = q.next;
        if q.deleted {
            continue;
        }
        if !crate::allowance::charge(
            work,
            q.key
                .map_or(64, |key| key.len().saturating_mul(64).saturating_add(64)),
        ) {
            result.scope.unknown = true;
            return result;
        }
        let Some(key) = q.key else {
            result.scope.unknown = true;
            return result;
        };
        if let Some(position) = names.iter().position(|name| *name == key) {
            fields[position] = quality(model, index);
        }
        let vocabulary = set_aside.classify(role, key, false);
        // Payload capability can be irrelevant, but interpretation and donor identity cannot.
        blocked |= key != "nullable"
            && (q.meaning_unavailable.is_some() || q.donor_unavailable.is_some())
            || vocabulary.blocks_base_meaning()
            || matches!(key, "items" | "properties")
            || key == "sdfRequired"
                && role != crate::role::Role::Definition(crate::Class::Property);
    }
    if !matches!(fields[ENUM], Quality::Absent) || !matches!(fields[CHOICE], Quality::Absent) {
        result.scope.unknown = model.alternative_scope_unknown(node);
        return result;
    }
    let has_bound = [MINIMUM, MAXIMUM, EXCLUSIVE_MINIMUM, EXCLUSIVE_MAXIMUM]
        .into_iter()
        .any(|index| !matches!(fields[index], Quality::Absent));
    if !has_bound {
        return result;
    }
    let constant = fields[CONST].node();
    let numeric_const =
        constant.is_some_and(|node| model.nodes[node].source.kind() == JsonKind::Number);
    let const_candidate = numeric_const || matches!(fields[CONST], Quality::Unknown);
    let finite = [MINIMUM, EXCLUSIVE_MINIMUM]
        .into_iter()
        .any(|i| !matches!(fields[i], Quality::Absent))
        && [MAXIMUM, EXCLUSIVE_MAXIMUM]
            .into_iter()
            .any(|i| !matches!(fields[i], Quality::Absent));
    if !const_candidate && !finite {
        return result;
    }
    let primitive = match fields[TYPE] {
        Quality::Absent => None,
        Quality::Known(index) => match model.nodes[index].source.text() {
            Field::Present("integer") => Some(BaseType::Integer),
            Field::Present("number") => Some(BaseType::Number),
            Field::Present("string" | "boolean" | "array" | "object") => return result,
            _ => {
                result.scope.unknown = true;
                return result;
            }
        },
        Quality::Unknown => {
            result.scope.unknown = true;
            return result;
        }
    };
    let interval_candidate = finite
        && (primitive == Some(BaseType::Integer)
            || primitive == Some(BaseType::Number)
                && !matches!(fields[MULTIPLE_OF], Quality::Absent));
    if !const_candidate && !interval_candidate {
        return result;
    }
    let format_ordinary = match fields[FORMAT] {
        Quality::Absent => true,
        Quality::Known(index) => matches!(
            model.nodes[index].source.text(),
            Field::Present("uri" | "uri-reference" | "uuid" | "date" | "time" | "date-time")
        ),
        Quality::Unknown => false,
    };
    if blocked || !matches!(fields[SDF_TYPE], Quality::Absent) {
        result.scope.unknown = true;
        return result;
    }
    result.set_aside = set_aside;
    if const_candidate && !format_ordinary {
        result.scope.unknown = true;
    }
    for (slot, field) in [MINIMUM, MAXIMUM, EXCLUSIVE_MINIMUM, EXCLUSIVE_MAXIMUM]
        .into_iter()
        .enumerate()
        .filter(|_| const_candidate && format_ordinary)
    {
        if matches!(fields[field], Quality::Absent) {
            continue;
        }
        result.scope.applicable = true;
        let bound = fields[field].node();
        let operands = match OperandIndices::new([constant, bound, None, None]) {
            Ok(operands) => operands,
            Err(_) => {
                result.scope.unknown = true;
                continue;
            }
        };
        let witness = Witness {
            operands,
            primitive,
            alternative,
        };
        let compared = if numeric.debit(2).is_err() {
            Some(Err("numeric work limit"))
        } else {
            constant.zip(bound).and_then(|(constant, bound)| {
                match (
                    model.nodes[constant].source.number(),
                    model.nodes[bound].source.number(),
                ) {
                    (Field::Present(value), Field::Present(bound)) => {
                        Some(value.compare(bound, numeric))
                    }
                    _ => None,
                }
            })
        };
        match compared {
            Some(Ok(order)) => {
                let violates = match field {
                    MINIMUM => order.is_lt(),
                    MAXIMUM => order.is_gt(),
                    EXCLUSIVE_MINIMUM => !order.is_gt(),
                    EXCLUSIVE_MAXIMUM => !order.is_lt(),
                    _ => unreachable!(),
                };
                if violates {
                    result.findings[slot] =
                        Some((Rule::NumericProof(NumericClass::ConstBound), witness));
                }
            }
            failure => {
                result.scope.unknown = true;
                result.findings[slot] = Some((
                    Rule::NumericCapability {
                        class: NumericClass::ConstBound,
                        capability: if failure.is_some() {
                            NumericCapability::Work
                        } else {
                            NumericCapability::Number
                        },
                    },
                    witness,
                ));
            }
        }
    }
    if !interval_candidate {
        return result;
    }
    let constant_nonnull =
        constant.is_some_and(|node| model.nodes[node].source.kind() != JsonKind::Null);
    let no_null = if constant_nonnull {
        Some(true)
    } else {
        match fields[NULLABLE] {
            Quality::Absent if matches!(fields[CONST], Quality::Unknown) => None,
            Quality::Absent => Some(false),
            Quality::Known(node) => match model.nodes[node].source.boolean() {
                Field::Present(false) => Some(true),
                Field::Present(true) if !matches!(fields[CONST], Quality::Unknown) => Some(false),
                _ => None,
            },
            Quality::Unknown => None,
        }
    };
    if no_null == Some(false) {
        return result;
    }
    let guard = if constant_nonnull {
        constant
    } else {
        fields[NULLABLE].node()
    };
    let lower = strongest_bound(model, &fields, true, numeric);
    let upper = strongest_bound(model, &fields, false, numeric);
    for (slot, class) in [(4, NumericClass::IntegerGap), (5, NumericClass::StepGap)] {
        let step = match class {
            NumericClass::IntegerGap if primitive == Some(BaseType::Integer) => Ok(None),
            NumericClass::IntegerGap => continue,
            NumericClass::StepGap => match fields[MULTIPLE_OF] {
                Quality::Absent => continue,
                Quality::Unknown => Err(NumericCapability::Meaning),
                Quality::Known(node) => match model.nodes[node].source.number() {
                    Field::Present(value) if value.sign().is_gt() => Ok(Some(value)),
                    Field::Present(_) => continue,
                    _ if model.nodes[node].source.kind() == JsonKind::Number => {
                        Err(NumericCapability::Number)
                    }
                    _ => continue,
                },
            },
            NumericClass::ConstBound => unreachable!(),
        };
        result.scope.applicable = true;
        let operands = match class {
            NumericClass::IntegerGap => [
                lower.0.map(|bound| bound.0),
                upper.0.map(|bound| bound.0),
                guard,
                None,
            ],
            _ => [
                lower.0.map(|bound| bound.0),
                upper.0.map(|bound| bound.0),
                fields[MULTIPLE_OF].node(),
                guard,
            ],
        };
        let Ok(operands) = OperandIndices::new(operands) else {
            result.scope.unknown = true;
            continue;
        };
        let witness = Witness {
            operands,
            primitive,
            alternative,
        };
        let limitation = lower.1.or(upper.1);
        result.scope.unknown |= limitation.is_some();
        let evaluated = if no_null.is_none() || !matches!(fields[FORMAT], Quality::Absent) {
            Err(NumericCapability::Meaning)
        } else if let Err(capability) = step {
            Err(capability)
        } else if let (Some((_, lower)), Some((_, upper)), Ok(step)) = (lower.0, upper.0, step) {
            crate::numeric::interval_gap(
                [lower, upper],
                step,
                primitive == Some(BaseType::Integer),
                numeric,
                allocation,
            )
        } else {
            Err(limitation.unwrap_or(NumericCapability::Number))
        };
        match evaluated {
            Ok(true) => result.findings[slot] = Some((Rule::NumericProof(class), witness)),
            Ok(false) => {
                if let Some(capability) = limitation {
                    result.findings[slot] =
                        Some((Rule::NumericCapability { class, capability }, witness));
                }
            }
            Err(capability) => {
                result.scope.unknown = true;
                result.findings[slot] =
                    Some((Rule::NumericCapability { class, capability }, witness));
            }
        }
    }
    result
}

type SelectedBound<'a> = (usize, crate::numeric::Endpoint<'a>);
fn strongest_bound<'a>(
    model: &EffectiveModel<'a>,
    fields: &[Quality; 16],
    lower: bool,
    numeric: &mut NumberWork,
) -> (Option<SelectedBound<'a>>, Option<NumericCapability>) {
    let mut selected: Option<SelectedBound<'a>> = None;
    let mut limitation = None;
    for index in if lower {
        [MINIMUM, EXCLUSIVE_MINIMUM]
    } else {
        [MAXIMUM, EXCLUSIVE_MAXIMUM]
    } {
        if matches!(fields[index], Quality::Absent) {
            continue;
        }
        if numeric.debit(1).is_err() {
            limitation = Some(NumericCapability::Work);
            continue;
        }
        let Some(node) = fields[index].node() else {
            limitation = Some(NumericCapability::Meaning);
            continue;
        };
        let Field::Present(value) = model.nodes[node].source.number() else {
            limitation = Some(NumericCapability::Number);
            continue;
        };
        let candidate = (
            node,
            crate::numeric::Endpoint {
                value,
                exclusive: index == EXCLUSIVE_MINIMUM || index == EXCLUSIVE_MAXIMUM,
            },
        );
        let replace = match selected {
            None => true,
            Some((_, previous)) => match value.compare(previous.value, numeric) {
                Ok(order) => {
                    (if lower { order.is_gt() } else { order.is_lt() })
                        || order.is_eq() && candidate.1.exclusive && !previous.exclusive
                }
                Err(_) => {
                    limitation = Some(NumericCapability::Work);
                    false
                }
            },
        };
        if replace {
            selected = Some(candidate);
        }
    }
    (selected, limitation)
}
