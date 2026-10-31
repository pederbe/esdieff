//! Representative values of an effective data description: simple values it may admit.
//! Revision comparison offers them as candidate Breaking witnesses from the revision that must
//! admit them, following ADR 0039: Client-side revision comparison, and Generated values use
//! them as their first Generation targets, following ADR 0041: Generated values settled by
//! Sample-value checking. Sample-value checking settles every value, so a representative
//! value that its description rejects after all establishes nothing by itself.
//!
//! A map value holds every required entry, each filled with a representative value of its
//! own description, so generation recurses into nested descriptions, as deep as a generated
//! value may nest. Every lookup and value is charged to the allowance the generator is given.
pub(crate) mod bounds;
mod pattern;

use crate::allowance::{OperationBudget, charge};
pub(crate) use crate::budgeted::Exhausted;
use crate::budgeted::{failure, kept, listed, member, strings, work};
use crate::sample::ValueAllowance;
use crate::scalar::{ResourceFormat, TemporalFormat};
use crate::{EffectiveJson, ExactNumber, Field, JsonBuilder, JsonKind, JsonValue};
use bounds::{End, step, step_power};
use std::cmp::Ordering;

/// Where a representative value comes from, in the order they are generated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Representative {
    Const,
    Enum,
    Default,
    Type,
    Alternative,
    Null,
}

/// An exact numeric bound at one end of a description's range.
#[derive(Clone, Copy)]
pub(crate) struct Bound<'c> {
    pub(crate) value: ExactNumber<'c>,
    pub(crate) exclusive: bool,
}
impl Bound<'_> {
    /// The bound itself when it is inclusive and one unit inside it when it is exclusive, with
    /// the unit rules of `bounds::step_power`. An inclusive bound keeps its authored spelling.
    pub(crate) fn inside(self, end: End, description: EffectiveJson<'_>) -> String {
        if !self.exclusive {
            return self.value.lexeme().to_owned();
        }
        let power = step_power(self.value, description.model.catalogue.limits.coefficient);
        step(self.value, end.inward_up(), power, false)
    }
    /// The bound itself when it is exclusive and one unit outside it when it is inclusive.
    pub(crate) fn outside(self, end: End, description: EffectiveJson<'_>) -> String {
        if self.exclusive {
            return self.value.lexeme().to_owned();
        }
        let power = step_power(self.value, description.model.catalogue.limits.coefficient);
        step(self.value, !end.inward_up(), power, false)
    }
}

/// Up to `limit` representative values of `description`, in order: a copy of its `const`,
/// its enum values, a copy of its documentary `default`, one value per type it admits or, for
/// a choice, the first value of each effective alternative, then `null` when it is Nullable.
pub(crate) fn values(
    description: EffectiveJson<'_>,
    limit: usize,
    allowance: &mut ValueAllowance,
) -> Result<Vec<(Representative, JsonValue)>, Exhausted> {
    let Some(mut generator) = Generator::new(description, allowance) else {
        return Ok(Vec::new());
    };
    generator.labelled(description, 0, limit)
}

/// Whether another reading of the enclosing map describes its filled entry `name` as the
/// admitting one does, as revision comparison requires of both revisions.
pub(crate) type SameEntry<'f> =
    &'f mut dyn FnMut(&str, &mut ValueAllowance) -> Result<bool, Exhausted>;

/// The map that `owner` describes around `value` at its entry `name`: `value` beside a
/// representative value of every other entry that `owner` requires. With a `counterpart`,
/// another description of the same map, the map also holds every entry the counterpart
/// requires, and is `None` when `same` finds a filled entry described differently. `None` too
/// when a filled entry has no representative value or the map exceeds the value capabilities.
pub(crate) fn enclosing(
    owner: EffectiveJson<'_>,
    name: &str,
    value: JsonValue,
    counterpart: Option<(EffectiveJson<'_>, SameEntry<'_>)>,
    allowance: &mut ValueAllowance,
) -> Result<Option<JsonValue>, Exhausted> {
    let Some(mut generator) = Generator::new(owner, allowance) else {
        return Ok(None);
    };
    generator.map(owner, Some((name, value)), counterpart, 0)
}

/// An array of `length` copies of the first representative value of the `items` of
/// `description`, or of `null` without an `items` description, or `None` when that item has
/// no representative value or the array exceeds the value capabilities.
pub(crate) fn items(
    description: EffectiveJson<'_>,
    length: usize,
    allowance: &mut ValueAllowance,
) -> Result<Option<JsonValue>, Exhausted> {
    let Some(mut generator) = Generator::new(description, allowance) else {
        return Ok(None);
    };
    generator.items(description, length, 0)
}

/// The array of `length` copies of `value`, whose value capabilities `description` gives, or
/// `None` when it exceeds them.
pub(crate) fn repeated(
    description: EffectiveJson<'_>,
    value: JsonValue,
    length: usize,
    allowance: &mut ValueAllowance,
) -> Result<Option<JsonValue>, Exhausted> {
    let Some(mut generator) = Generator::new(description, allowance) else {
        return Ok(None);
    };
    generator.repeat(value, length)
}

type Build = fn(&JsonBuilder) -> Result<JsonValue, &'static str>;

/// One value of each JSON kind: one of each base type, in the order representative values try
/// the types, then `null`.
pub(crate) const KINDS: [Build; 6] = [
    |builder| builder.number("0"),
    |builder| builder.text(""),
    |builder| builder.boolean(true),
    |builder| builder.map(Vec::new()),
    |builder| builder.array(Vec::new()),
    JsonBuilder::null,
];

/// Arrays of `length` copies of one value of each base type that an item without a
/// description may be, in the order representative values try the types: `0`, `""`, `true`,
/// `{}` and `[]`. `description` gives the value capabilities.
pub(crate) fn untyped(
    description: EffectiveJson<'_>,
    length: usize,
    allowance: &mut ValueAllowance,
) -> Result<Vec<JsonValue>, Exhausted> {
    let Some(mut generator) = Generator::new(description, allowance) else {
        return Ok(Vec::new());
    };
    let mut arrays = Vec::new();
    for item in &KINDS[..5] {
        let array = match generator.build(4, item)? {
            Some(item) => generator.repeat(item, length)?,
            None => None,
        };
        if let Some(array) = array {
            generator.push(&mut arrays, array)?;
        }
    }
    Ok(arrays)
}

/// The base types whose values a description without a `type` admits, in the order their
/// representative values are tried, which is also the order of `KINDS`.
pub(crate) const TYPES: [&str; 5] = ["number", "string", "boolean", "object", "array"];

/// A conventional value of a format that Sample-value checking checks beside an explicit
/// `type: "string"`, or `None` for any other format.
fn conventional(format: &str) -> Option<&'static str> {
    if let Some(format) = TemporalFormat::from_text(format) {
        return Some(match format {
            TemporalFormat::DateTime => "1970-01-01T00:00:00Z",
            TemporalFormat::Date => "1970-01-01",
            TemporalFormat::Time => "00:00:00Z",
        });
    }
    ResourceFormat::from_text(format).map(|format| match format {
        ResourceFormat::Uri => "https://example.com",
        ResourceFormat::Reference => "",
        ResourceFormat::Uuid => "00000000-0000-0000-0000-000000000000",
    })
}

pub(crate) struct Generator<'s> {
    pub(crate) allowance: &'s mut ValueAllowance,
    builder: JsonBuilder,
    /// The deepest a generated value may nest.
    depth: usize,
    values: Vec<(Representative, JsonValue)>,
    limit: usize,
}
impl<'s> Generator<'s> {
    /// A generator within the value capabilities of `description`'s Catalogue, or `None` when
    /// they admit no generated value.
    pub(crate) fn new(
        description: EffectiveJson<'_>,
        allowance: &'s mut ValueAllowance,
    ) -> Option<Self> {
        let limits = &description.model.catalogue.limits;
        Some(Self {
            allowance,
            builder: JsonBuilder::with_limits(limits.clone()).ok()?,
            depth: limits.depth,
            values: Vec::new(),
            limit: 0,
        })
    }
    fn full(&self) -> bool {
        self.values.len() >= self.limit
    }

    /// Keep the representative values of `description`, whose values nest `depth` deep in
    /// the generated value, while there is room for them.
    fn description(
        &mut self,
        description: EffectiveJson<'_>,
        depth: usize,
    ) -> Result<(), Exhausted> {
        if failure(description).is_some()
            || description.kind() != JsonKind::Map
            || depth > self.depth
        {
            return Ok(());
        }
        for key in ["const", "enum", "default"] {
            let Some(json) = self.member(description, key)? else {
                continue;
            };
            if key != "enum" {
                let value = self.copy(json, depth)?;
                let source = if key == "const" {
                    Representative::Const
                } else {
                    Representative::Default
                };
                self.keep(source, value)?;
            } else if strings(json, &mut self.allowance.budget)? {
                for member in json.children() {
                    if self.full() {
                        return Ok(());
                    }
                    let text = member.text().present().expect("decoded string");
                    let value = self.build(text.len() + 2, |builder| builder.text(text))?;
                    self.keep(Representative::Enum, value)?;
                }
            }
        }
        let choice = self.member(description, "sdfChoice")?.is_some();
        match description.alternatives() {
            // A choice admits the values of its alternatives, so each gives its first value in
            // place of a value of each type.
            Field::Present(alternatives) if choice => {
                for (_, alternative) in alternatives {
                    self.work(1)?;
                    if let Field::Present(alternative) = alternative
                        && !self.full()
                    {
                        let value = self.first(alternative.0, depth)?;
                        self.keep(Representative::Alternative, value)?;
                    }
                }
            }
            _ => self.types(description, depth)?,
        }
        if !self.full() && self.nullable(description)? {
            let value = self.build(4, JsonBuilder::null)?;
            self.keep(Representative::Null, value)?;
        }
        Ok(())
    }

    /// Keep one value of each base type that `description` admits.
    fn types(&mut self, description: EffectiveJson<'_>, depth: usize) -> Result<(), Exhausted> {
        let typed = self.member(description, "type")?;
        let types = match typed {
            None => &TYPES[..],
            Some(json) => match json.text().present() {
                Some("integer") => &TYPES[..1],
                Some(name) => TYPES
                    .iter()
                    .position(|base| *base == name)
                    .map_or(&TYPES[..0], |at| &TYPES[at..=at]),
                None => &TYPES[..0],
            },
        };
        for name in types {
            if self.full() {
                return Ok(());
            }
            let value = match *name {
                "number" => self.number(description)?,
                "string" => self.string(description, typed.is_some())?,
                "boolean" => self.build(4, |builder| builder.boolean(true))?,
                "object" => self.map(description, None, None, depth)?,
                _ => self.array(description, depth)?,
            };
            self.keep(Representative::Type, value)?;
        }
        Ok(())
    }

    /// Up to `limit` representative values of `description`, whose values nest `depth` deep,
    /// each labelled with where it comes from.
    pub(crate) fn labelled(
        &mut self,
        description: EffectiveJson<'_>,
        depth: usize,
        limit: usize,
    ) -> Result<Vec<(Representative, JsonValue)>, Exhausted> {
        let values = std::mem::take(&mut self.values);
        let limit = std::mem::replace(&mut self.limit, limit);
        let found = self.description(description, depth);
        self.limit = limit;
        let labelled = std::mem::replace(&mut self.values, values);
        found.map(|()| labelled)
    }

    /// The first representative value of `description`, whose values nest `depth` deep.
    pub(crate) fn first(
        &mut self,
        description: EffectiveJson<'_>,
        depth: usize,
    ) -> Result<Option<JsonValue>, Exhausted> {
        let first = self.labelled(description, depth, 1)?.pop();
        Ok(first.map(|(_, value)| value))
    }

    /// A number at the description's lower bound, or else its upper bound: the bound itself
    /// when it is inclusive and one unit inside it when it is exclusive. Without a bound it
    /// is `0`.
    fn number(&mut self, description: EffectiveJson<'_>) -> Result<Option<JsonValue>, Exhausted> {
        for end in [End::Lower, End::Upper] {
            let Some(bound) = self.bound(description, end)? else {
                return Ok(None);
            };
            if let Some(bound) = bound {
                return self.lexeme(&bound.inside(end, description));
            }
        }
        self.lexeme("0")
    }

    /// The description's effective bound at `end`, the tighter of its inclusive and exclusive
    /// qualities, `Some(None)` without one, or `None` when a bound is not an exact number.
    pub(crate) fn bound<'c>(
        &mut self,
        description: EffectiveJson<'c>,
        end: End,
    ) -> Result<Option<Option<Bound<'c>>>, Exhausted> {
        let mut tightest: Option<Bound<'c>> = None;
        for (key, exclusive) in end.qualities() {
            let Some(json) = self.member(description, key)? else {
                continue;
            };
            let Field::Present(value) = json.number() else {
                return Ok(None);
            };
            let tighter = match tightest {
                None => true,
                Some(other) => {
                    let order = self
                        .allowance
                        .compare(value, other.value)
                        .map_err(|_| Exhausted::Work)?;
                    order == Ordering::Equal || (order == Ordering::Greater) == end.inward_up()
                }
            };
            if tighter {
                tightest = Some(Bound { value, exclusive });
            }
        }
        Ok(Some(tightest))
    }

    /// A string: a conventional value of a format that Sample-value checking checks beside
    /// an explicit `type`, otherwise a bounded pattern-aware candidate, falling back to
    /// `minLength` copies of `a`, which is `""` without one.
    fn string(
        &mut self,
        description: EffectiveJson<'_>,
        typed: bool,
    ) -> Result<Option<JsonValue>, Exhausted> {
        if typed
            && let Some(format) = self.member(description, "format")?
            && let Some(text) = format.text().present().and_then(conventional)
        {
            return self.build(text.len() + 2, |builder| builder.text(text));
        }
        let limit = description.model.catalogue.limits.document_bytes;
        let Some(length) = self.count_or_zero(description, "minLength", limit)? else {
            return Ok(None);
        };
        if let Some(pattern) = self.member(description, "pattern")?
            && failure(pattern).is_none()
            && let Some(pattern) = pattern.text().present()
            && pattern.len() <= description.model.catalogue.limits.pattern_bytes
        {
            let maximum = match self.member(description, "maxLength")? {
                None => Some(limit),
                Some(json) => json
                    .number()
                    .present()
                    .and_then(|number| count(number, limit)),
            };
            if let Some(maximum) = maximum
                && let Some(text) = pattern::synthesize(pattern, length, maximum, self)?
            {
                return self.build(text.len() + 2, |builder| builder.text(&text));
            }
        }
        self.build(length + 2, |builder| builder.text(&"a".repeat(length)))
    }

    /// An array of `minItems` representative values of its `items`, or of `null` without an
    /// `items` description.
    fn array(
        &mut self,
        description: EffectiveJson<'_>,
        depth: usize,
    ) -> Result<Option<JsonValue>, Exhausted> {
        let limit = description.model.catalogue.limits.document_nodes;
        let Some(length) = self.count_or_zero(description, "minItems", limit)? else {
            return Ok(None);
        };
        self.items(description, length, depth)
    }

    /// An array of `length` copies of the first representative value of the description's
    /// `items`, or of `null` without an `items` description.
    pub(crate) fn items(
        &mut self,
        description: EffectiveJson<'_>,
        length: usize,
        depth: usize,
    ) -> Result<Option<JsonValue>, Exhausted> {
        if depth >= self.depth {
            return Ok(None);
        }
        if length == 0 {
            return self.aggregate(Vec::new(), false);
        }
        let item = match self.member(description, "items")? {
            Some(items) => self.first(items, depth + 1)?,
            None => self.build(4, JsonBuilder::null)?,
        };
        match item {
            Some(item) => self.repeat(item, length),
            None => Ok(None),
        }
    }

    /// The count a `minLength` or `minItems` quality states, `0` without one, or `None` when
    /// it is not a count of at most `limit`, which a generated value cannot exceed.
    fn count_or_zero(
        &mut self,
        description: EffectiveJson<'_>,
        key: &str,
        limit: usize,
    ) -> Result<Option<usize>, Exhausted> {
        let Some(json) = self.member(description, key)? else {
            return Ok(Some(0));
        };
        let Field::Present(number) = json.number() else {
            return Ok(None);
        };
        Ok(count(number, limit))
    }

    /// A map holding `entry`, when given, and a representative value of every other entry
    /// that `description` requires, with `null` for a required entry without a description.
    /// With a `counterpart`, the corresponding description of the other revision, the map also
    /// holds every entry the counterpart requires, and is `None` when the two describe a filled
    /// entry differently.
    pub(crate) fn map(
        &mut self,
        description: EffectiveJson<'_>,
        entry: Option<(&str, JsonValue)>,
        mut counterpart: Option<(EffectiveJson<'_>, SameEntry<'_>)>,
        depth: usize,
    ) -> Result<Option<JsonValue>, Exhausted> {
        if depth >= self.depth {
            return Ok(None);
        }
        let mut entries = Vec::new();
        if let Some(entry) = entry {
            self.push(&mut entries, entry)?;
        }
        let other = counterpart.as_ref().map(|(other, _)| *other);
        for owner in std::iter::once(description).chain(other) {
            let Some(required) = self.member(owner, "required")? else {
                continue;
            };
            if !strings(required, &mut self.allowance.budget)? {
                continue;
            }
            let properties = self.member(description, "properties")?;
            for name in required.children() {
                let name = name.text().present().expect("decoded string");
                let names = entries.iter().map(|(other, _)| *other);
                if listed(names, name, &mut self.allowance.budget)? {
                    continue;
                }
                if let Some((_, same)) = &mut counterpart
                    && !same(name, self.allowance)?
                {
                    return Ok(None);
                }
                let described = match properties {
                    Some(properties) if properties.kind() == JsonKind::Map => {
                        self.member(properties, name)?
                    }
                    _ => None,
                };
                let value = match described {
                    Some(entry) => self.first(entry, depth + 1)?,
                    None => self.build(4, JsonBuilder::null)?,
                };
                let Some(value) = value else {
                    return Ok(None);
                };
                self.push(&mut entries, (name, value))?;
            }
        }
        self.aggregate(entries, true)
    }
    /// A copy of effective content, with each number in its authored spelling, or `None` when
    /// any of it is unavailable or the copy exceeds the value capabilities.
    fn copy(
        &mut self,
        json: EffectiveJson<'_>,
        depth: usize,
    ) -> Result<Option<JsonValue>, Exhausted> {
        if failure(json).is_some() || depth > self.depth {
            return Ok(None);
        }
        match json.kind() {
            JsonKind::Null => self.build(4, JsonBuilder::null),
            JsonKind::Boolean => match json.boolean().present() {
                Some(value) => self.build(5, |builder| builder.boolean(value)),
                None => Ok(None),
            },
            JsonKind::Number => match json.number() {
                Field::Present(number) => self.lexeme(number.lexeme()),
                _ => Ok(None),
            },
            JsonKind::String => match json.text().present() {
                Some(text) => self.build(text.len() + 2, |builder| builder.text(text)),
                None => Ok(None),
            },
            JsonKind::Array | JsonKind::Map => {
                let mut entries = Vec::new();
                for child in json.children() {
                    let name = match (json.kind(), child.n().key) {
                        (JsonKind::Map, None) => return Ok(None),
                        (_, name) => name.unwrap_or(""),
                    };
                    let Some(value) = self.copy(child, depth + 1)? else {
                        return Ok(None);
                    };
                    self.push(&mut entries, (name, value))?;
                }
                self.aggregate(entries, json.kind() == JsonKind::Map)
            }
        }
    }
    /// The map of `entries`, or the array of their values, charging what it adds to the
    /// values it holds.
    pub(crate) fn aggregate(
        &mut self,
        entries: Vec<(&str, JsonValue)>,
        map: bool,
    ) -> Result<Option<JsonValue>, Exhausted> {
        let held: usize = entries
            .iter()
            .map(|(_, value)| value.author_storage().1)
            .sum();
        let size = entries
            .iter()
            .map(|(name, _)| name.len() + 4)
            .sum::<usize>()
            + 2;
        self.work(size)?;
        let built = if map {
            self.builder.map(entries)
        } else {
            self.builder
                .array(entries.into_iter().map(|(_, value)| value).collect())
        };
        let Ok(value) = built else {
            return Ok(None);
        };
        if !charge(
            &mut self.allowance.budget.allocation,
            value.author_storage().1.saturating_sub(held),
        ) {
            return Err(Exhausted::Allocation);
        }
        Ok(Some(value))
    }
    /// The array of `length` copies of `item`, charging what it adds to the item it repeats.
    pub(crate) fn repeat(
        &mut self,
        item: JsonValue,
        length: usize,
    ) -> Result<Option<JsonValue>, Exhausted> {
        let held = item.author_storage().1;
        let size = (item.to_json().len() + 1).saturating_mul(length) + 2;
        self.work(size)?;
        let Ok(value) = self.builder.repeated(&item, length) else {
            return Ok(None);
        };
        if !charge(
            &mut self.allowance.budget.allocation,
            value.author_storage().1.saturating_sub(held),
        ) {
            return Err(Exhausted::Allocation);
        }
        Ok(Some(value))
    }
    /// The effective Nullable characteristic, with its Specified default `true` where
    /// `nullable` is absent. A Direct items description has neither, following ADR 0024:
    /// Effective alternatives and nullable characteristics.
    fn nullable(&mut self, description: EffectiveJson<'_>) -> Result<bool, Exhausted> {
        if description.n().role == Some(crate::role::Role::DirectItem) {
            return Ok(false);
        }
        Ok(match self.member(description, "nullable")? {
            None => true,
            Some(json) => json.boolean().present() == Some(true),
        })
    }

    pub(crate) fn member<'c>(
        &mut self,
        json: EffectiveJson<'c>,
        key: &str,
    ) -> Result<Option<EffectiveJson<'c>>, Exhausted> {
        member(json, key, &mut self.allowance.budget)
    }
    /// Keep a value while there is room for it.
    fn keep(&mut self, source: Representative, value: Option<JsonValue>) -> Result<(), Exhausted> {
        match value {
            Some(value) if !self.full() => kept(
                &mut self.values,
                (source, value),
                &mut self.allowance.budget,
            ),
            _ => Ok(()),
        }
    }
    pub(crate) fn push<T>(&mut self, items: &mut Vec<T>, item: T) -> Result<(), Exhausted> {
        kept(items, item, &mut self.allowance.budget)
    }
    /// Charge `cost` units of generation work.
    pub(crate) fn work(&mut self, cost: usize) -> Result<(), Exhausted> {
        work(&mut self.allowance.budget, cost)
    }
    pub(crate) fn lexeme(&mut self, lexeme: &str) -> Result<Option<JsonValue>, Exhausted> {
        self.build(lexeme.len(), |builder| builder.number(lexeme))
    }
    pub(crate) fn build(
        &mut self,
        size: usize,
        build: impl FnOnce(&JsonBuilder) -> Result<JsonValue, &'static str>,
    ) -> Result<Option<JsonValue>, Exhausted> {
        built(&self.builder, &mut self.allowance.budget, size, build)
    }
}

/// `number` as a count of at most `limit`, which a generated value cannot exceed, or `None`
/// when it is not a nonnegative integer within it.
pub(crate) fn count(number: ExactNumber<'_>, limit: usize) -> Option<usize> {
    let decimal = number.decimal;
    if decimal.coefficient == "0" {
        return Some(0);
    }
    if decimal.negative || decimal.power < 0 {
        return None;
    }
    let digits = decimal.coefficient.len() as u128 + decimal.power as u128;
    if digits > limit.ilog10() as u128 + 1 {
        return None;
    }
    let mut count: usize = decimal.coefficient.parse().expect("a decimal coefficient");
    for _ in 0..decimal.power {
        count *= 10;
    }
    (count <= limit).then_some(count)
}

/// The value `build` makes from about `size` bytes, charged as the generated value it
/// becomes, or `None` when it exceeds the value capabilities.
pub(crate) fn built(
    builder: &JsonBuilder,
    budget: &mut OperationBudget,
    size: usize,
    build: impl FnOnce(&JsonBuilder) -> Result<JsonValue, &'static str>,
) -> Result<Option<JsonValue>, Exhausted> {
    work(budget, size + 1)?;
    let Ok(value) = build(builder) else {
        return Ok(None);
    };
    if !charge(&mut budget.allocation, value.author_storage().1) {
        return Err(Exhausted::Allocation);
    }
    Ok(Some(value))
}
