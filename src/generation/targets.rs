//! The Generation targets of one effective data description, in the order the contract
//! `esdieff.generation/2` fixes, each settled by Sample-value checking as soon as it is kept.
//! Settling each value before generating the next keeps every kept admission, even when later
//! generation runs out of the description's allowance.
//!
//! Two values whose maps hold the same entries in another order are the same value, and so
//! are kept once. Every kept value is charged the report bytes it needs in the largest output
//! format, so the request's retained values always fit the report byte limit.
//!
//! The targets of each `properties` entry and of `items` follow a description's own targets.
//! Each of their values is placed where that nested description stands, inside enclosing maps
//! whose other required entries hold Representative values and enclosing arrays that hold as
//! many items as they require, so the enclosing constraints still apply to it.
use crate::budgeted::{self, listed};
use crate::representative::bounds::{self, End, step, step_power};
use crate::representative::{self, Bound, Exhausted, Generator, KINDS, Representative, TYPES};
use crate::role::{Role, SetAside};
use crate::sample::{CheckedValue, ValueAllowance, ValueInterpretations, check_generated};
use crate::{
    Admission, DataRole, EffectiveDataDescription, EffectiveJson, Field, JsonBuilder, JsonKind,
    JsonValue, JsonValueView,
};
use std::collections::{HashMap, HashSet};

/// A kept value with its target and settled admission.
pub(super) struct Settled {
    pub(super) target: String,
    pub(super) value: CheckedValue,
    pub(super) admission: Admission,
    pub(super) codes: Vec<&'static str>,
    /// The target's precedence, which decides which of two targets with the same value
    /// keeps it.
    precedence: u8,
    /// The value's key, the same for maps with the same entries in another order.
    key: String,
    /// The report bytes charged for the value.
    size: usize,
}

/// The values generated for one description.
pub(super) struct Generated {
    pub(super) values: Vec<Settled>,
    /// Distinct values beyond the ceiling.
    pub(super) omitted: u32,
    /// Distinct values that did not fit the report byte limit.
    pub(super) oversized: u32,
    /// Generation or a check ran out of the allowance.
    pub(super) exhausted: bool,
    pub(super) interpretations: ValueInterpretations,
    /// The extension vocabulary the checks set aside.
    pub(super) set_aside: SetAside,
}

/// The most condition codes kept for one rejection.
const CODES: usize = 8;

/// The report bytes a value with this target, JSON text, admission and rejection codes needs
/// in the largest output format.
pub(super) type Size<'r> = &'r dyn Fn(&str, &str, Admission, &[&'static str]) -> usize;

/// The bytes of the Trace entry line a value with this target and JSON text gives, apart from
/// its LF.
pub(super) type Line<'r> = &'r dyn Fn(&str, &str) -> usize;

/// The report bytes left for generated values, how many a value needs, and how long its
/// Trace entry line is.
pub(super) struct Room<'r> {
    pub(super) bytes: &'r mut usize,
    pub(super) size: Size<'r>,
    pub(super) line: Line<'r>,
}

/// Generate and settle the values of `description`, keeping at most `ceiling` distinct ones
/// and only those that fit `room`. Building the values spends `allowance`, which belongs to
/// this description alone, and each check spends an allowance of its own.
pub(super) fn generate(
    description: EffectiveJson<'_>,
    ceiling: usize,
    allowance: &mut ValueAllowance,
    room: Room<'_>,
) -> Generated {
    let mut generated = Generated {
        values: Vec::new(),
        omitted: 0,
        oversized: 0,
        exhausted: false,
        interpretations: ValueInterpretations::default(),
        set_aside: SetAside::default(),
    };
    let Some(generator) = Generator::new(description, allowance) else {
        return generated;
    };
    let limits = &description.model.catalogue.limits;
    let mut run = Run {
        generator,
        root: EffectiveDataDescription(description, DataRole::Full),
        ceiling,
        depth: limits.depth,
        path: Vec::new(),
        seen: HashMap::new(),
        label: 0,
        room,
        generated: &mut generated,
    };
    let found = run.longest_label(description, 0, 0).and_then(|label| {
        run.label = label;
        run.targets(description)
    });
    if found.is_err() {
        run.generated.exhausted = true;
    }
    generated
}

/// A map description's described entries and its required names.
type Entries<'c> = (Vec<(&'c str, EffectiveJson<'c>)>, Vec<&'c str>);

/// Where a nested description's values stand within the selected description's value.
#[derive(Clone, Copy)]
enum Step<'m> {
    /// At the entry `name` of the map that `owner` describes.
    Entry(EffectiveJson<'m>, &'m str),
    /// Among the items of the array that `owner` describes.
    Items(EffectiveJson<'m>),
}

struct Run<'s, 'm, 'g> {
    generator: Generator<'s>,
    /// The selected description every value is checked against.
    root: EffectiveDataDescription<'m>,
    ceiling: usize,
    /// The deepest a generated value may nest.
    depth: usize,
    /// The steps from the selected description to the one whose targets are generated.
    path: Vec<Step<'m>>,
    /// The key of every distinct value offered so far, and whether it was kept.
    seen: HashMap<String, bool>,
    /// The JSON-escaped length of the longest target name this description can give.
    label: usize,
    room: Room<'g>,
    generated: &'g mut Generated,
}
impl<'m> Run<'_, 'm, '_> {
    /// Offer every target of `description`, in contract order, then the targets of its
    /// nested descriptions.
    fn targets(&mut self, description: EffectiveJson<'m>) -> Result<(), Exhausted> {
        let limits = &description.model.catalogue.limits;
        let (text_limit, item_limit) = (limits.document_bytes, limits.document_nodes);
        let unique = self.unique(description)?;
        let lower = self.generator.bound(description, End::Lower)?.flatten();
        let upper = self.generator.bound(description, End::Upper)?.flatten();
        let min_length = self.stated_count(description, "minLength", text_limit)?;
        let max_length = self.stated_count(description, "maxLength", text_limit)?;
        let min_items = self.stated_count(description, "minItems", item_limit)?;
        let max_items = self.stated_count(description, "maxItems", item_limit)?;
        self.representatives(description)?;
        // Boundaries: values at each edge that the description may admit.
        for (end, bound, target) in [
            (End::Lower, lower, "minimum"),
            (End::Upper, upper, "maximum"),
        ] {
            if let Some(bound) = bound {
                let value = self.generator.lexeme(&bound.inside(end, description))?;
                self.offer(target, value)?;
            }
        }
        for (length, target) in [(min_length, "min_length"), (max_length, "max_length")] {
            if let Some(length) = length {
                let value = self.text(length)?;
                self.offer(target, value)?;
            }
        }
        for (length, target) in [(min_items, "min_items"), (max_items, "max_items")] {
            if let Some(length) = length {
                let value = self.array(description, length, unique)?;
                self.offer(target, value)?;
            }
        }
        self.all_entries(description)?;
        // Just outside: values one unit, one step or one count across each edge.
        for (end, bound, target) in [
            (End::Lower, lower, "below_minimum"),
            (End::Upper, upper, "above_maximum"),
        ] {
            if let Some(bound) = bound {
                let value = self.generator.lexeme(&bound.outside(end, description))?;
                self.offer(target, value)?;
            }
        }
        self.not_multiple(description, lower)?;
        if let Some(length) = min_length.filter(|length| *length > 0) {
            let value = self.text(length - 1)?;
            self.offer("too_short", value)?;
        }
        if let Some(length) = max_length.filter(|length| *length < text_limit) {
            let value = self.text(length + 1)?;
            self.offer("too_long", value)?;
        }
        if let Some(length) = min_items.filter(|length| *length > 0) {
            let value = self.array(description, length - 1, unique)?;
            self.offer("too_few_items", value)?;
        }
        if let Some(length) = max_items.filter(|length| *length < item_limit) {
            let value = self.array(description, length + 1, unique)?;
            self.offer("too_many_items", value)?;
        }
        if unique {
            let depth = self.path.len();
            let length = min_items.unwrap_or(0).max(2);
            let value = self.generator.items(description, length, depth)?;
            self.offer("duplicate_items", value)?;
        }
        self.missing_entries(description)?;
        self.wrong_type(description)?;
        self.nested(description)
    }

    /// The targets of each `properties` entry, in order, then of `items`, while their values
    /// can still nest within the value capabilities.
    fn nested(&mut self, description: EffectiveJson<'m>) -> Result<(), Exhausted> {
        if self.path.len() + 1 >= self.depth {
            return Ok(());
        }
        if let Some(properties) = self.generator.member(description, "properties")?
            && budgeted::failure(properties).is_none()
            && properties.kind() == JsonKind::Map
        {
            for entry in properties.children() {
                self.generator.work(1)?;
                let Some(name) = entry.n().key else {
                    continue;
                };
                self.generator
                    .push(&mut self.path, Step::Entry(description, name))?;
                let found = self.targets(entry);
                self.path.pop();
                found?;
            }
        }
        if let Some(items) = self.generator.member(description, "items")? {
            self.generator
                .push(&mut self.path, Step::Items(description))?;
            let found = self.targets(items);
            self.path.pop();
            found?;
        }
        Ok(())
    }

    /// Whether the description requires unique items.
    fn unique(&mut self, description: EffectiveJson<'_>) -> Result<bool, Exhausted> {
        Ok(self
            .generator
            .member(description, "uniqueItems")?
            .is_some_and(|json| json.boolean().present() == Some(true)))
    }

    /// An array of `length` items: copies of the first representative value of `items`, or of
    /// `null` without an `items` description, or distinct items when the array requires
    /// unique items. `None` when there are not enough distinct items or the array exceeds the
    /// value capabilities.
    fn array(
        &mut self,
        description: EffectiveJson<'_>,
        length: usize,
        unique: bool,
    ) -> Result<Option<JsonValue>, Exhausted> {
        let depth = self.path.len();
        if !unique || length < 2 {
            return self.generator.items(description, length, depth);
        }
        let Some(items) = self.distinct(description, length, None, depth)? else {
            return Ok(None);
        };
        self.generator
            .aggregate(items.into_iter().map(|item| ("", item)).collect(), false)
    }

    /// `length` distinct items for the array `description` describes at `depth`, none the
    /// same value as `except`: the representative values of its `items`, then further values
    /// after the first of them, numbers one unit apart, strings that differ in their last
    /// character, and the other boolean. Without an `items` description they are the numbers
    /// from 0. `None` when there are not that many.
    fn distinct(
        &mut self,
        description: EffectiveJson<'_>,
        length: usize,
        except: Option<&JsonValue>,
        depth: usize,
    ) -> Result<Option<Vec<JsonValue>>, Exhausted> {
        let mut found: Vec<JsonValue> = Vec::new();
        let mut keys = HashSet::new();
        if let Some(except) = except {
            self.distinct_key(&mut keys, except)?;
        }
        let seeds = match self.generator.member(description, "items")? {
            Some(items) => self.generator.labelled(items, depth + 1, length + 1)?,
            None => match self.generator.lexeme("0")? {
                Some(zero) => vec![(Representative::Type, zero)],
                None => Vec::new(),
            },
        };
        let mut first = Seed::Other;
        for (index, (_, value)) in seeds.into_iter().enumerate() {
            if index == 0 {
                first = Seed::of(&value);
            }
            self.distinct_push(&mut found, &mut keys, value, length)?;
        }
        let digits = description.model.catalogue.limits.coefficient;
        let mut previous = match &first {
            Seed::Number(lexeme) => lexeme.clone(),
            _ => String::new(),
        };
        // Each further value is new, so `length` more attempts always suffice.
        for index in 0..length {
            if found.len() >= length {
                break;
            }
            let next = match &first {
                Seed::Number(_) => {
                    let built = self.generator.lexeme(&previous)?;
                    let Some(Field::Present(number)) =
                        built.as_ref().map(|value| value.view().number())
                    else {
                        break;
                    };
                    previous = step(number, true, step_power(number, digits), false);
                    self.generator.lexeme(&previous)?
                }
                Seed::Text(text) => {
                    let mut varied = text.clone();
                    varied.pop();
                    varied.push_str(&letters(index));
                    self.generator
                        .build(varied.len() + 2, |builder| builder.text(&varied))?
                }
                Seed::Boolean(value) => {
                    let other = !*value;
                    self.generator.build(5, |builder| builder.boolean(other))?
                }
                Seed::Other => break,
            };
            match next {
                Some(next) => self.distinct_push(&mut found, &mut keys, next, length)?,
                None => break,
            }
        }
        Ok((found.len() >= length).then_some(found))
    }

    /// Keep `value` among `found` items unless an earlier item, or the excluded value, has
    /// its key.
    fn distinct_push(
        &mut self,
        found: &mut Vec<JsonValue>,
        keys: &mut HashSet<String>,
        value: JsonValue,
        length: usize,
    ) -> Result<(), Exhausted> {
        if found.len() >= length || !self.distinct_key(keys, &value)? {
            return Ok(());
        }
        self.generator.push(found, value)
    }

    /// Add the key of `value` to `keys`, charging what it stores, and whether it is new.
    fn distinct_key(
        &mut self,
        keys: &mut HashSet<String>,
        value: &JsonValue,
    ) -> Result<bool, Exhausted> {
        let key = self.key(value)?;
        if keys.contains(&key) {
            return Ok(false);
        }
        self.charge(key.len())?;
        keys.insert(key);
        Ok(true)
    }

    /// The entries a map description describes, in order, and the names it requires, in
    /// order without repeats. `None` when its `properties` or `required` cannot be read.
    fn entries<'c>(
        &mut self,
        description: EffectiveJson<'c>,
    ) -> Result<Option<Entries<'c>>, Exhausted> {
        let mut described = Vec::new();
        if let Some(properties) = self.generator.member(description, "properties")? {
            if budgeted::failure(properties).is_some() || properties.kind() != JsonKind::Map {
                return Ok(None);
            }
            for entry in properties.children() {
                self.generator.work(1)?;
                let Some(name) = entry.n().key else {
                    return Ok(None);
                };
                self.generator.push(&mut described, (name, entry))?;
            }
        }
        let mut required = Vec::new();
        if let Some(names) = self.generator.member(description, "required")? {
            if !budgeted::strings(names, &mut self.generator.allowance.budget)? {
                return Ok(None);
            }
            for name in names.children() {
                let name = name.text().present().expect("decoded string");
                let budget = &mut self.generator.allowance.budget;
                if !listed(required.iter().copied(), name, budget)? {
                    self.generator.push(&mut required, name)?;
                }
            }
        }
        Ok(Some((described, required)))
    }

    /// A map holding every entry the description describes, then every other entry it
    /// requires, each filled with the first representative value of its description, or
    /// `null` without one.
    fn all_entries(&mut self, description: EffectiveJson<'_>) -> Result<(), Exhausted> {
        let Some((described, required)) = self.entries(description)? else {
            return Ok(());
        };
        if described.is_empty() {
            return Ok(());
        }
        let mut names: Vec<&str> = Vec::new();
        for (name, _) in &described {
            self.generator.push(&mut names, *name)?;
        }
        for name in &required {
            let budget = &mut self.generator.allowance.budget;
            if !listed(names.iter().copied(), name, budget)? {
                self.generator.push(&mut names, *name)?;
            }
        }
        let value = self.filled(&names, &described)?;
        self.offer("all_entries", value)
    }

    /// For each required entry, a map holding every other required entry and lacking it.
    fn missing_entries(&mut self, description: EffectiveJson<'_>) -> Result<(), Exhausted> {
        let Some((described, required)) = self.entries(description)? else {
            return Ok(());
        };
        for missing in 0..required.len() {
            let mut names = required.clone();
            names.remove(missing);
            let value = self.filled(&names, &described)?;
            self.offer("missing_entry", value)?;
        }
        Ok(())
    }

    /// The map of `names`, each filled with the first representative value of its
    /// description among `described`, or `null` without one. `None` when an entry has no
    /// representative value or the map exceeds the value capabilities.
    fn filled(
        &mut self,
        names: &[&str],
        described: &[(&str, EffectiveJson<'_>)],
    ) -> Result<Option<JsonValue>, Exhausted> {
        let depth = self.path.len();
        if depth >= self.depth {
            return Ok(None);
        }
        let mut entries = Vec::new();
        for name in names {
            let mut description = None;
            for (other, entry) in described {
                self.generator.work(name.len().min(other.len()) + 1)?;
                if other == name {
                    description = Some(*entry);
                    break;
                }
            }
            let value = match description {
                Some(entry) => self.generator.first(entry, depth + 1)?,
                None => self.generator.build(4, JsonBuilder::null)?,
            };
            let Some(value) = value else {
                return Ok(None);
            };
            self.generator.push(&mut entries, (*name, value))?;
        }
        self.generator.aggregate(entries, true)
    }

    /// Half a step above an inclusive minimum that is a multiple of a positive `multipleOf`,
    /// or else half a step above 0. Half a step past a multiple is never a multiple.
    fn not_multiple(
        &mut self,
        description: EffectiveJson<'_>,
        lower: Option<Bound<'_>>,
    ) -> Result<(), Exhausted> {
        let Some(json) = self.generator.member(description, "multipleOf")? else {
            return Ok(());
        };
        let Field::Present(step) = json.number() else {
            return Ok(());
        };
        if !step.sign().is_gt() {
            return Ok(());
        }
        let digits = description.model.catalogue.limits.coefficient;
        let mut lexeme = None;
        if let Some(bound) = lower
            && !bound.exclusive
            && self
                .generator
                .allowance
                .multiple(bound.value, step)
                .map_err(|_| Exhausted::Work)?
        {
            lexeme = bounds::half_past(bound.value, step, digits);
        }
        let lexeme = lexeme.unwrap_or_else(|| bounds::half(step));
        let value = self.generator.lexeme(&lexeme)?;
        self.offer("not_multiple", value)
    }

    /// One value of each base type that the description's `type` does not admit. Without a
    /// `type`, or with one that is not a base type, no value has a known wrong type.
    fn wrong_type(&mut self, description: EffectiveJson<'_>) -> Result<(), Exhausted> {
        let Some(json) = self.generator.member(description, "type")? else {
            return Ok(());
        };
        let admitted = match json.text().present() {
            Some("integer") => "number",
            Some(name @ ("number" | "string" | "boolean" | "object" | "array")) => name,
            _ => return Ok(()),
        };
        for (name, build) in TYPES.iter().zip(&KINDS) {
            if *name != admitted {
                let value = self.generator.build(4, build)?;
                self.offer("wrong_type", value)?;
            }
        }
        Ok(())
    }

    /// The count a `minLength`, `maxLength`, `minItems` or `maxItems` quality states, or
    /// `None` without one or when it is not a count of at most `limit`.
    fn stated_count(
        &mut self,
        description: EffectiveJson<'_>,
        key: &str,
        limit: usize,
    ) -> Result<Option<usize>, Exhausted> {
        let Some(json) = self.generator.member(description, key)? else {
            return Ok(None);
        };
        Ok(match json.number() {
            Field::Present(number) => representative::count(number, limit),
            _ => None,
        })
    }

    /// A string of `length` copies of `a`.
    fn text(&mut self, length: usize) -> Result<Option<JsonValue>, Exhausted> {
        self.generator
            .build(length + 2, |builder| builder.text(&"a".repeat(length)))
    }

    /// The Representative values, labelled by where they come from, then `null` for a Full
    /// data description that is not Nullable, so that settlement shows it is not.
    fn representatives(&mut self, description: EffectiveJson<'_>) -> Result<(), Exhausted> {
        let depth = self.path.len();
        let values = self.generator.labelled(description, depth, usize::MAX)?;
        let mut null = false;
        for (source, value) in values {
            let target = match source {
                Representative::Const => "const",
                Representative::Enum => "enum",
                Representative::Default => "default",
                Representative::Type => "type",
                Representative::Alternative => "alternative",
                Representative::Null => {
                    null = true;
                    "null"
                }
            };
            self.offer(target, Some(value))?;
        }
        if !null && description.n().role != Some(Role::DirectItem) {
            let value = self.generator.build(4, JsonBuilder::null)?;
            self.offer("null", value)?;
        }
        Ok(())
    }

    /// `value` placed where the current nested description stands in the selected
    /// description's value, or `None` when an enclosing value cannot hold it.
    fn place(&mut self, value: JsonValue) -> Result<Option<JsonValue>, Exhausted> {
        let mut value = value;
        for index in (0..self.path.len()).rev() {
            let placed = match self.path[index] {
                Step::Entry(owner, name) => {
                    self.generator
                        .map(owner, Some((name, value)), None, index)?
                }
                Step::Items(owner) => self.items(owner, value, index)?,
            };
            match placed {
                Some(placed) => value = placed,
                None => return Ok(None),
            }
        }
        Ok(Some(value))
    }

    /// The array `owner` describes, holding `item` as often as its `minItems` requires and at
    /// least once, or `item` beside other distinct items when it requires unique items.
    fn items(
        &mut self,
        owner: EffectiveJson<'_>,
        item: JsonValue,
        depth: usize,
    ) -> Result<Option<JsonValue>, Exhausted> {
        let limit = owner.model.catalogue.limits.document_nodes;
        let length = self
            .stated_count(owner, "minItems", limit)?
            .unwrap_or(0)
            .max(1);
        if length < 2 || !self.unique(owner)? {
            return self.generator.repeat(item, length);
        }
        let Some(others) = self.distinct(owner, length - 1, Some(&item), depth)? else {
            return Ok(None);
        };
        let mut entries = vec![("", item)];
        for other in others {
            self.generator.push(&mut entries, ("", other))?;
        }
        self.generator.aggregate(entries, false)
    }

    /// The name of `target` for the current nested description, such as `level.minimum`.
    fn name(&self, target: &str) -> String {
        let mut name = String::new();
        for step in &self.path {
            name.push_str(match step {
                Step::Entry(_, entry) => entry,
                Step::Items(_) => "items",
            });
            name.push('.');
        }
        name.push_str(target);
        name
    }

    /// Place `value` and keep it under `target`, then settle it. A value an earlier target
    /// already produced is kept once, under the target with the higher precedence. A distinct
    /// value beyond the ceiling is only counted.
    fn offer(&mut self, target: &str, value: Option<JsonValue>) -> Result<(), Exhausted> {
        let Some(value) = value else {
            return Ok(());
        };
        let Some(value) = self.place(value)? else {
            return Ok(());
        };
        let precedence = self.precedence(target);
        let target = self.name(target);
        let key = self.key(&value)?;
        match self.seen.get(&key) {
            Some(true) => return self.yield_to(&key, target, value, precedence),
            Some(false) => return Ok(()),
            None => {}
        }
        if self.generated.values.len() >= self.ceiling {
            self.generated.omitted += 1;
            return self.remember(key, false);
        }
        // A value too large for the report even without rejection codes, or whose Trace entry
        // would be longer than a Recorded trace line, is not checked.
        let text = value.to_json();
        let slack = self.label - target.len();
        if (self.room.size)(&target, text, Admission::Admitted, &[]) + slack > *self.room.bytes
            || (self.room.line)(&target, text) + slack > crate::trace::LINE_BYTES
        {
            self.generated.oversized += 1;
            return self.remember(key, false);
        }
        // Each check receives the allowance of one Sample-value checking request, so a heavy
        // check leaves only its own value undetermined.
        let value = CheckedValue::new(value);
        let mut lent = ValueAllowance::new(self.root.json().model.catalogue);
        let patterns = &mut self.generator.allowance.budget.patterns;
        patterns.lend_worker(&mut lent.budget.patterns);
        let checked = check_generated(self.root, &value, &mut lent);
        patterns.reclaim_worker(&mut lent.budget.patterns);
        let checked = checked.map_err(|_| Exhausted::Work)?;
        self.generated.interpretations |= checked.interpretations;
        self.generated.set_aside |= lent.budget.set_aside;
        let mut codes = Vec::new();
        if checked.admission == Admission::Rejected {
            for finding in &checked.findings {
                if codes.len() < CODES
                    && !finding.basis.incomplete()
                    && !codes.contains(&finding.code)
                {
                    self.generator.push(&mut codes, finding.code)?;
                }
            }
        }
        // Charged as if under the longest name, so a target of higher precedence always fits.
        let size = (self.room.size)(&target, value.to_json(), checked.admission, &codes) + slack;
        if size > *self.room.bytes {
            self.generated.oversized += 1;
            return self.remember(key, false);
        }
        *self.room.bytes -= size;
        self.remember(key.clone(), true)?;
        let settled = Settled {
            target,
            value,
            admission: checked.admission,
            codes,
            precedence,
            key,
            size,
        };
        self.generator.push(&mut self.generated.values, settled)
    }

    /// Give the kept value with `key` to a later `target` of higher `precedence`. The value
    /// moves to the later target's place and keeps that target's text, which has the same
    /// entries.
    fn yield_to(
        &mut self,
        key: &str,
        target: String,
        value: JsonValue,
        precedence: u8,
    ) -> Result<(), Exhausted> {
        let Some(index) = self
            .generated
            .values
            .iter()
            .position(|kept| kept.key == key)
        else {
            return Ok(());
        };
        let kept = &self.generated.values[index];
        if precedence <= kept.precedence {
            return Ok(());
        }
        // The value was charged as if under the longest target name, so this one fits.
        debug_assert!(
            (self.room.size)(&target, value.to_json(), kept.admission, &kept.codes) <= kept.size
        );
        let mut kept = self.generated.values.remove(index);
        kept.target = target;
        kept.value = CheckedValue::new(value);
        kept.precedence = precedence;
        self.generated.values.push(kept);
        Ok(())
    }

    /// The JSON-escaped length of the longest target name of `description`, whose name prefix
    /// so far is `prefix` escaped bytes long at nesting `depth`: the longest target, after the
    /// path of entry names and `items` to each nested description its values can reach.
    fn longest_label(
        &mut self,
        description: EffectiveJson<'_>,
        prefix: usize,
        depth: usize,
    ) -> Result<usize, Exhausted> {
        const LONGEST: &str = "duplicate_items";
        let mut longest = prefix + LONGEST.len() + 2;
        if depth + 1 >= self.depth {
            return Ok(longest);
        }
        if let Some(properties) = self.generator.member(description, "properties")?
            && budgeted::failure(properties).is_none()
            && properties.kind() == JsonKind::Map
        {
            for entry in properties.children() {
                let Some(name) = entry.n().key else {
                    continue;
                };
                self.generator.work(name.len() + 1)?;
                let escaped = serde_json::Value::from(name).to_string().len() - 2;
                let nested = self.longest_label(entry, prefix + escaped + 1, depth + 1)?;
                longest = longest.max(nested);
            }
        }
        if let Some(items) = self.generator.member(description, "items")? {
            let nested = self.longest_label(items, prefix + "items.".len(), depth + 1)?;
            longest = longest.max(nested);
        }
        Ok(longest)
    }

    /// The precedence of `target` at the current nesting: a boundary or outside target ranks
    /// above a Representative-value target or `all_entries`, and within each class a nested
    /// target ranks above a top-level one.
    fn precedence(&self, target: &str) -> u8 {
        let representative = matches!(
            target,
            "const" | "enum" | "default" | "type" | "alternative" | "null" | "all_entries"
        );
        2 * u8::from(!representative) + u8::from(!self.path.is_empty())
    }

    /// Record that the value with `key` was offered and whether it was kept, charging what the
    /// record stores to the description's allowance.
    fn remember(&mut self, key: String, kept: bool) -> Result<(), Exhausted> {
        self.charge(key.len())?;
        self.seen.insert(key, kept);
        Ok(())
    }

    /// Charge the storage of a recorded key of `bytes` bytes.
    fn charge(&mut self, bytes: usize) -> Result<(), Exhausted> {
        let entry = bytes + std::mem::size_of::<(String, bool)>();
        if crate::allowance::charge(&mut self.generator.allowance.budget.allocation, entry) {
            Ok(())
        } else {
            Err(Exhausted::Allocation)
        }
    }

    /// The key of `value`: its JSON text with every map's entries in name order, so that maps
    /// with the same entries have the same key. Numbers keep their spelling.
    fn key(&mut self, value: &JsonValue) -> Result<String, Exhausted> {
        let text = value.to_json();
        self.generator.work(text.len() + 1)?;
        let mut key = String::with_capacity(text.len());
        canonical(value.view(), &mut key);
        Ok(key)
    }
}

/// Write the key of `view` to `key`, with map entries in name order.
fn canonical(view: JsonValueView<'_>, key: &mut String) {
    match view.kind() {
        JsonKind::Map => {
            let mut entries: Vec<_> = view
                .children()
                .map(|entry| (entry.member_name().present().unwrap_or(""), entry))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            key.push('{');
            for (index, (name, entry)) in entries.into_iter().enumerate() {
                if index > 0 {
                    key.push(',');
                }
                key.push_str(&serde_json::Value::from(name).to_string());
                key.push(':');
                canonical(entry, key);
            }
            key.push('}');
        }
        JsonKind::Array => {
            key.push('[');
            for (index, item) in view.children().enumerate() {
                if index > 0 {
                    key.push(',');
                }
                canonical(item, key);
            }
            key.push(']');
        }
        _ => key.push_str(view.raw_json()),
    }
}

/// The first item a sequence of distinct items continues from.
enum Seed {
    Number(String),
    Text(String),
    Boolean(bool),
    Other,
}
impl Seed {
    fn of(value: &JsonValue) -> Self {
        let view = value.view();
        match view.kind() {
            JsonKind::Number => match view.number() {
                Field::Present(number) => Self::Number(number.lexeme().to_owned()),
                _ => Self::Other,
            },
            JsonKind::String => match view.text() {
                Field::Present(text) => Self::Text(text.to_owned()),
                _ => Self::Other,
            },
            JsonKind::Boolean => match view.boolean() {
                Field::Present(value) => Self::Boolean(value),
                _ => Self::Other,
            },
            _ => Self::Other,
        }
    }
}

/// The `index`th string of lowercase letters: `a` to `z`, then `aa`, `ab` and onwards.
fn letters(index: usize) -> String {
    let mut letters = Vec::new();
    let mut rest = index;
    loop {
        letters.push(b'a' + (rest % 26) as u8);
        rest /= 26;
        if rest == 0 {
            break;
        }
        rest -= 1;
    }
    letters.reverse();
    String::from_utf8(letters).expect("ASCII letters")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalogue, Class, EffectiveRequest, Lookup, SourceInput};

    /// The targets kept for the Property `level` under `ceiling`, and the omitted count.
    fn kept(description: &str, ceiling: usize) -> (Vec<(String, String)>, u32) {
        let model = format!(r#"{{"sdfProperty":{{"level":{description}}}}}"#);
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", model.as_bytes())]).unwrap();
        let model = catalogue
            .effective(EffectiveRequest::new(catalogue.source_id("model").unwrap()))
            .unwrap();
        let Lookup::Found(level) = model.definition(Class::Property, "level") else {
            panic!("level")
        };
        let mut allowance = ValueAllowance::new(&catalogue);
        let mut bytes = usize::MAX;
        let size = |_: &str, value: &str, _: Admission, _: &[&'static str]| value.len();
        let line = |_: &str, value: &str| value.len();
        let room = Room {
            bytes: &mut bytes,
            size: &size,
            line: &line,
        };
        let generated = generate(level.json(), ceiling, &mut allowance, room);
        assert!(!generated.exhausted);
        let values = generated
            .values
            .iter()
            .map(|value| (value.target.clone(), value.value.to_json().to_owned()))
            .collect();
        (values, generated.omitted)
    }

    #[test]
    fn distinct_values_beyond_the_ceiling_are_counted_not_kept() {
        let (values, omitted) = kept(r#"{"type":"string","enum":["a","b","c"]}"#, 4);
        let shown: Vec<_> = values.iter().map(|(target, _)| target.as_str()).collect();
        // Three enum values, the type's empty string, null and four wrong types.
        assert_eq!(shown, ["enum", "enum", "enum", "type"]);
        assert_eq!(omitted, 5);
    }

    #[test]
    fn values_beyond_the_ceiling_are_counted_once_each() {
        // Kept: the first enum value. Omitted: the second enum value, which the default
        // repeats, the type's empty string, null and four wrong types.
        let (values, omitted) = kept(r#"{"type":"string","enum":["a","b"],"default":"b"}"#, 1);
        assert_eq!(values, [("enum".to_owned(), "\"a\"".to_owned())]);
        assert_eq!(omitted, 7);
    }

    #[test]
    fn a_duplicate_is_kept_under_its_highest_precedence_target() {
        // The constant, the type and the minimum all give 3, and the minimum, a boundary
        // target, keeps it at its own place after null.
        let (values, _) = kept(r#"{"type":"integer","const":3,"minimum":3}"#, 64);
        let targets: Vec<_> = values.iter().map(|(target, _)| target.as_str()).collect();
        assert_eq!(&targets[..2], ["null", "minimum"]);
        assert!(!targets.contains(&"const") && !targets.contains(&"type"));
        // Two Representative-value targets of one class: the first keeps the value.
        let (values, _) = kept(r#"{"type":"integer","const":0}"#, 64);
        assert_eq!(values[0], ("const".to_owned(), "0".to_owned()));
        assert!(!values.iter().any(|(target, _)| target == "type"));
    }
}
