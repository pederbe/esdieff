//! Conventional JSON projections and sample views follow ADR0025: Selected numeric samples.
use crate::json::{Budget, Parsed, Value};
use crate::{ExactNumber, Field, JsonKind, Limits, Source, SourceId};
use std::ops::Range;

/// An owned, bounded conventional JSON value. Its projection is not original input bytes.
pub struct JsonValue {
    bytes: Vec<u8>,
    parsed: Parsed,
    depth: usize,
    storage: usize,
}
impl JsonValue {
    pub(crate) fn copy_authored(
        view: crate::JsonView<'_>,
        limits: &Limits,
        budget: &mut Budget,
    ) -> Result<Self, &'static str> {
        let raw = view.raw_json();
        let parsed = crate::json::parse_value(raw.as_bytes(), true, limits, budget);
        if parsed.rejected || parsed.root.is_none() || parsed.errors > 0 {
            return Err("authored JSON copying exceeded decoding capabilities");
        }
        budget.allocation = budget
            .allocation
            .checked_sub(raw.len())
            .ok_or("authored JSON byte allocation limit")?;
        let mut depth = 0;
        for node in &parsed.nodes {
            let mut parent = node.parent;
            let mut current = 0;
            while let Some(index) = parent {
                if budget.work == 0 {
                    return Err("authored JSON depth work limit");
                }
                budget.work -= 1;
                current += 1;
                parent = parsed.nodes[index].parent;
            }
            depth = depth.max(current);
        }
        let storage = raw.len() + parsed.nodes.len() * std::mem::size_of::<crate::json::Node>();
        Ok(Self {
            bytes: raw.as_bytes().to_vec(),
            parsed,
            depth,
            storage,
        })
    }
    pub(crate) fn author_storage(&self) -> (usize, usize, usize) {
        (self.parsed.nodes.len(), self.storage, self.depth)
    }
    pub fn view(&self) -> JsonValueView<'_> {
        JsonValueView(ValueRef {
            bytes: &self.bytes,
            parsed: &self.parsed,
            node: self.parsed.root.expect("constructed value"),
        })
    }
    pub fn to_json(&self) -> &str {
        std::str::from_utf8(&self.bytes).expect("projection UTF-8")
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// The projection and its value decoding, for use as a typed Sample source.
    pub(crate) fn into_parts(self) -> (Vec<u8>, Parsed) {
        (self.bytes, self.parsed)
    }
}
/// Constructors charge combined child storage, projection and decoding before retaining a value.
pub struct JsonBuilder {
    limits: Limits,
}
impl Default for JsonBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl JsonBuilder {
    pub fn new() -> Self {
        Self {
            limits: Limits::default(),
        }
    }
    pub(crate) fn with_limits(limits: Limits) -> Result<Self, &'static str> {
        limits.validate()?;
        Ok(Self { limits })
    }
    pub fn null(&self) -> Result<JsonValue, &'static str> {
        self.scalar("null")
    }
    pub fn boolean(&self, value: bool) -> Result<JsonValue, &'static str> {
        self.scalar(if value { "true" } else { "false" })
    }
    pub fn number(&self, lexeme: &str) -> Result<JsonValue, &'static str> {
        let value = self.scalar(lexeme)?;
        if value.view().kind() != JsonKind::Number
            || value.parsed.nodes[value.parsed.root.expect("value")].span != (0..lexeme.len())
        {
            return Err("number lexeme required");
        }
        value.view().number().require()?;
        Ok(value)
    }
    pub fn i64(&self, value: i64) -> Result<JsonValue, &'static str> {
        let mut budget = Budget::new(&self.limits);
        let size = if value == 0 {
            1
        } else {
            value.unsigned_abs().ilog10() as usize + 1 + usize::from(value < 0)
        };
        self.precharge(&mut budget, size, 0, 1, 0)?;
        let mut text = String::with_capacity(size);
        use std::fmt::Write;
        write!(text, "{value}").expect("precharged integer projection");
        self.decode(text.into_bytes(), 0, budget)
    }
    pub fn u64(&self, value: u64) -> Result<JsonValue, &'static str> {
        let mut budget = Budget::new(&self.limits);
        let size = if value == 0 {
            1
        } else {
            value.ilog10() as usize + 1
        };
        self.precharge(&mut budget, size, 0, 1, 0)?;
        let mut text = String::with_capacity(size);
        use std::fmt::Write;
        write!(text, "{value}").expect("precharged integer projection");
        self.decode(text.into_bytes(), 0, budget)
    }
    fn scalar(&self, text: &str) -> Result<JsonValue, &'static str> {
        let mut budget = Budget::new(&self.limits);
        self.precharge(&mut budget, text.len(), 0, 1, 0)?;
        self.decode(text.as_bytes().to_vec(), 0, budget)
    }
    pub fn text(&self, text: &str) -> Result<JsonValue, &'static str> {
        let mut budget = Budget::new(&self.limits);
        let size = self.string_size(&mut budget, text)?;
        self.precharge(&mut budget, size, 0, 1, 0)?;
        let mut bytes = Vec::with_capacity(size);
        escape(text, &mut bytes);
        self.decode(bytes, 0, budget)
    }
    pub fn array(&self, values: Vec<JsonValue>) -> Result<JsonValue, &'static str> {
        let mut budget = Budget::new(&self.limits);
        if values.len() >= self.limits.nodes.min(self.limits.document_nodes)
            || values.capacity() > self.limits.allocation / std::mem::size_of::<JsonValue>()
        {
            return Err("typed aggregate node or storage limit");
        }
        let mut size = 2usize;
        let mut storage = values
            .capacity()
            .checked_mul(std::mem::size_of::<JsonValue>())
            .ok_or("typed storage overflow")?;
        let mut nodes = 1usize;
        let mut depth = 0;
        for (index, value) in values.iter().enumerate() {
            construction_work(&mut budget, 1)?;
            size = size
                .checked_add(value.bytes.len() + usize::from(index > 0))
                .ok_or("typed size overflow")?;
            storage = storage
                .checked_add(value.storage)
                .ok_or("typed storage overflow")?;
            nodes = nodes
                .checked_add(value.parsed.nodes.len())
                .ok_or("typed node overflow")?;
            depth = depth.max(value.depth + 1);
        }
        self.precharge(&mut budget, size, storage, nodes, depth)?;
        let mut bytes = Vec::with_capacity(size);
        bytes.push(b'[');
        for (index, value) in values.iter().enumerate() {
            construction_work(&mut budget, 1)?;
            if index > 0 {
                bytes.push(b',');
            }
            bytes.extend_from_slice(&value.bytes);
        }
        bytes.push(b']');
        self.decode(bytes, depth, budget)
    }
    /// An array of `count` copies of `value`, without building each copy on its own.
    pub(crate) fn repeated(
        &self,
        value: &JsonValue,
        count: usize,
    ) -> Result<JsonValue, &'static str> {
        let mut budget = Budget::new(&self.limits);
        let size = (value.bytes.len() + 1)
            .checked_mul(count)
            .and_then(|size| size.checked_add(1 + usize::from(count == 0)))
            .ok_or("typed size overflow")?;
        let nodes = value
            .parsed
            .nodes
            .len()
            .checked_mul(count)
            .and_then(|nodes| nodes.checked_add(1))
            .ok_or("typed node overflow")?;
        let depth = if count == 0 { 0 } else { value.depth + 1 };
        self.precharge(&mut budget, size, value.storage, nodes, depth)?;
        let mut bytes = Vec::with_capacity(size);
        bytes.push(b'[');
        for index in 0..count {
            construction_work(&mut budget, 1)?;
            if index > 0 {
                bytes.push(b',');
            }
            bytes.extend_from_slice(&value.bytes);
        }
        bytes.push(b']');
        self.decode(bytes, depth, budget)
    }
    pub fn map(&self, entries: Vec<(&str, JsonValue)>) -> Result<JsonValue, &'static str> {
        let mut budget = Budget::new(&self.limits);
        if entries.len() >= self.limits.nodes.min(self.limits.document_nodes)
            || entries.capacity()
                > self.limits.allocation / std::mem::size_of::<(&str, JsonValue)>()
        {
            return Err("typed aggregate node or storage limit");
        }
        let mut size = 2usize;
        let mut storage = entries
            .capacity()
            .checked_mul(std::mem::size_of::<(&str, JsonValue)>())
            .ok_or("typed storage overflow")?;
        let mut nodes = 1usize;
        let mut depth = 0;
        for (index, (key, value)) in entries.iter().enumerate() {
            construction_work(&mut budget, 1)?;
            for (previous, _) in &entries[..index] {
                construction_work(
                    &mut budget,
                    key.len()
                        .max(previous.len())
                        .checked_add(1)
                        .ok_or("typed comparison work overflow")?,
                )?;
                if key == previous {
                    return Err("duplicate decoded map name (A002)");
                }
            }
            size = size
                .checked_add(
                    self.string_size(&mut budget, key)?
                        + 1
                        + value.bytes.len()
                        + usize::from(index > 0),
                )
                .ok_or("typed size overflow")?;
            storage = storage
                .checked_add(value.storage)
                .ok_or("typed storage overflow")?;
            nodes = nodes
                .checked_add(value.parsed.nodes.len())
                .ok_or("typed node overflow")?;
            depth = depth.max(value.depth + 1);
        }
        self.precharge(&mut budget, size, storage, nodes, depth)?;
        let mut bytes = Vec::with_capacity(size);
        bytes.push(b'{');
        for (index, (key, value)) in entries.iter().enumerate() {
            construction_work(&mut budget, 1)?;
            if index > 0 {
                bytes.push(b',');
            }
            escape(key, &mut bytes);
            bytes.push(b':');
            bytes.extend_from_slice(&value.bytes);
        }
        bytes.push(b'}');
        self.decode(bytes, depth, budget)
    }
    fn string_size(&self, budget: &mut Budget, text: &str) -> Result<usize, &'static str> {
        if text.len() > self.limits.document_bytes {
            return Err("typed text byte limit");
        }
        construction_work(budget, text.len())?;
        text.bytes().try_fold(2usize, |n, byte| {
            n.checked_add(match byte {
                b'"' | b'\\' => 2,
                0..=31 => 6,
                _ => 1,
            })
            .ok_or("typed string overflow")
        })
    }
    fn precharge(
        &self,
        budget: &mut Budget,
        bytes: usize,
        children: usize,
        nodes: usize,
        depth: usize,
    ) -> Result<(), &'static str> {
        if bytes > self.limits.document_bytes || bytes > self.limits.catalogue_bytes {
            return Err("typed projection byte or work limit");
        }
        if nodes > self.limits.nodes
            || nodes > self.limits.document_nodes
            || depth > self.limits.depth
        {
            return Err("typed node or depth limit");
        }
        construction_work(budget, bytes)?;
        budget.allocation = budget
            .allocation
            .checked_sub(
                children
                    .checked_add(bytes)
                    .ok_or("typed storage overflow")?,
            )
            .ok_or("typed construction storage limit")?;
        Ok(())
    }
    fn decode(
        &self,
        bytes: Vec<u8>,
        depth: usize,
        mut budget: Budget,
    ) -> Result<JsonValue, &'static str> {
        budget.allocation = budget
            .allocation
            .checked_sub(bytes.capacity() - bytes.len())
            .ok_or("typed construction storage limit")?;
        let before = budget.allocation;
        let parsed = crate::json::parse_value(&bytes, true, &self.limits, &mut budget);
        if parsed.rejected || parsed.root.is_none() || parsed.issue_count > 0 {
            return Err("typed value grammar or construction capability unavailable");
        }
        let storage = bytes.capacity() + before - budget.allocation;
        Ok(JsonValue {
            bytes,
            parsed,
            depth,
            storage,
        })
    }
}
fn construction_work(budget: &mut Budget, amount: usize) -> Result<(), &'static str> {
    budget.work = budget
        .work
        .checked_sub(amount)
        .ok_or("typed construction work limit")?;
    Ok(())
}
fn escape(text: &str, bytes: &mut Vec<u8>) {
    const HEX: &[u8] = b"0123456789abcdef";
    bytes.push(b'"');
    for byte in text.bytes() {
        match byte {
            b'"' | b'\\' => {
                bytes.push(b'\\');
                bytes.push(byte);
            }
            0..=31 => bytes.extend_from_slice(&[
                b'\\',
                b'u',
                b'0',
                b'0',
                HEX[(byte / 16) as usize],
                HEX[(byte % 16) as usize],
            ]),
            _ => bytes.push(byte),
        }
    }
    bytes.push(b'"');
}

#[derive(Clone, Copy)]
struct ValueRef<'a> {
    bytes: &'a [u8],
    parsed: &'a Parsed,
    node: usize,
}
impl<'a> ValueRef<'a> {
    fn kind(self) -> JsonKind {
        match self.parsed.nodes[self.node].value {
            Value::Null => JsonKind::Null,
            Value::Bool(_) => JsonKind::Boolean,
            Value::Number(_) => JsonKind::Number,
            Value::String(_) => JsonKind::String,
            Value::Array => JsonKind::Array,
            Value::Map => JsonKind::Map,
        }
    }
    fn raw_json(self) -> &'a str {
        std::str::from_utf8(&self.bytes[self.parsed.nodes[self.node].span.clone()])
            .expect("decoded UTF-8")
    }
    fn boolean(self) -> Field<bool> {
        match self.parsed.nodes[self.node].value {
            Value::Bool(v) => Field::Present(v),
            _ => Field::Unavailable("Boolean shape"),
        }
    }
    fn text(self) -> Field<&'a str> {
        match &self.parsed.nodes[self.node].value {
            Value::String(Some(v)) => Field::Present(v),
            Value::String(None) => Field::Unavailable("Unicode scalar decoding unavailable"),
            _ => Field::Unavailable("text shape"),
        }
    }
    fn number(self) -> Field<ExactNumber<'a>> {
        match &self.parsed.nodes[self.node].value {
            Value::Number(Some(decimal)) => Field::Present(ExactNumber {
                raw: self.raw_json(),
                decimal,
            }),
            _ => Field::Unavailable("exact number capability or shape"),
        }
    }
    fn children(self) -> ValueRefs<'a> {
        ValueRefs {
            parent: self,
            next: self.parsed.nodes[self.node].first,
        }
    }
    fn get(self, key: &str) -> Field<Self> {
        if self.kind() != JsonKind::Map {
            return Field::Unavailable("map shape");
        }
        let mut found = None;
        let mut unknown = false;
        for child in self.children() {
            match &child.parsed.nodes[child.node].key {
                Some(name) if name == key => found = Some(child),
                None => unknown = true,
                _ => {}
            }
        }
        if unknown {
            Field::Unavailable("map member identity unavailable")
        } else {
            found.map_or(Field::Absent, Field::Present)
        }
    }
    fn at(self, index: usize) -> Field<Self> {
        if self.kind() != JsonKind::Array {
            return Field::Unavailable("array shape");
        }
        self.children()
            .find(|child| child.parsed.nodes[child.node].index == Some(index))
            .map_or(Field::Absent, Field::Present)
    }
}
#[derive(Clone, Copy)]
struct ValueRefs<'a> {
    parent: ValueRef<'a>,
    next: Option<usize>,
}
impl<'a> Iterator for ValueRefs<'a> {
    type Item = ValueRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next?;
        self.next = self.parent.parsed.nodes[node].next;
        Some(ValueRef {
            node,
            ..self.parent
        })
    }
}
fn map_field<T, U>(field: Field<T>, f: impl FnOnce(T) -> U) -> Field<U> {
    match field {
        Field::Present(v) => Field::Present(f(v)),
        Field::Absent => Field::Absent,
        Field::Unavailable(r) => Field::Unavailable(r),
    }
}
#[derive(Clone, Copy)]
pub struct JsonValueView<'a>(ValueRef<'a>);
impl<'a> JsonValueView<'a> {
    pub fn kind(self) -> JsonKind {
        self.0.kind()
    }
    pub fn raw_json(self) -> &'a str {
        self.0.raw_json()
    }
    pub fn boolean(self) -> Field<bool> {
        self.0.boolean()
    }
    pub fn text(self) -> Field<&'a str> {
        self.0.text()
    }
    pub fn number(self) -> Field<ExactNumber<'a>> {
        self.0.number()
    }
    pub fn get(self, key: &str) -> Field<Self> {
        map_field(self.0.get(key), Self)
    }
    pub fn at(self, index: usize) -> Field<Self> {
        map_field(self.0.at(index), Self)
    }
    pub fn children(self) -> ValueChildren<'a> {
        ValueChildren(self.0.children())
    }
    pub fn member_name(self) -> Field<&'a str> {
        self.0.parsed.nodes[self.0.node]
            .key
            .as_deref()
            .map_or(Field::Absent, Field::Present)
    }
}
pub struct ValueChildren<'a>(ValueRefs<'a>);
impl<'a> Iterator for ValueChildren<'a> {
    type Item = JsonValueView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(JsonValueView)
    }
}
/// Sample access distinguishes original byte spans from conventional typed projections.
#[derive(Clone, Copy)]
pub struct SampleValueView<'a> {
    source: &'a Source,
    value: ValueRef<'a>,
}
impl<'a> SampleValueView<'a> {
    pub(crate) fn parent(self) -> Option<Self> {
        let parent = self.value.parsed.nodes.get(self.value.node)?.parent?;
        self.value.parsed.nodes.get(parent)?;
        Some(Self::new(self.source, parent))
    }
    pub(crate) fn node(self) -> usize {
        self.value.node
    }
    pub(crate) fn new(source: &'a Source, node: usize) -> Self {
        Self {
            source,
            value: ValueRef {
                bytes: &source.bytes,
                parsed: &source.parsed,
                node,
            },
        }
    }
    pub fn kind(self) -> JsonKind {
        self.value.kind()
    }
    pub fn raw_json(self) -> &'a str {
        self.value.raw_json()
    }
    pub fn source_id(self) -> SourceId {
        self.source.id
    }
    pub fn has_original_bytes(self) -> bool {
        self.source.original
    }
    pub fn span(self) -> Option<Range<usize>> {
        self.source
            .original
            .then(|| self.value.parsed.nodes[self.value.node].span.clone())
    }
    pub fn boolean(self) -> Field<bool> {
        self.value.boolean()
    }
    pub fn text(self) -> Field<&'a str> {
        self.value.text()
    }
    pub fn number(self) -> Field<ExactNumber<'a>> {
        self.value.number()
    }
    pub fn get(self, key: &str) -> Field<Self> {
        map_field(self.value.get(key), |value| Self { value, ..self })
    }
    pub fn at(self, index: usize) -> Field<Self> {
        map_field(self.value.at(index), |value| Self { value, ..self })
    }
    pub fn children(self) -> SampleChildren<'a> {
        SampleChildren {
            source: self.source,
            values: self.value.children(),
        }
    }
    pub fn member_name(self) -> Field<&'a str> {
        self.value.parsed.nodes[self.value.node]
            .key
            .as_deref()
            .map_or(Field::Absent, Field::Present)
    }
    /// Render an observation pointer using only the optional evidence allowance.
    pub(crate) fn evidence_pointer(
        self,
        budget: &mut crate::allowance::OperationBudget,
    ) -> Result<String, ()> {
        let json = crate::JsonView {
            source: self.source,
            node: self.value.node,
        };
        let mut work = crate::json::PointerBudget::new(budget.work);
        let size = json.pointer_size(8192, &mut work);
        budget.work = *work.counter();
        let size = size.map_err(|_| ())?;
        if !crate::allowance::charge(&mut budget.work, size.render_work())
            || !crate::allowance::charge(&mut budget.allocation, size.storage())
        {
            return Err(());
        }
        Ok(json.render_pointer(size))
    }
    pub fn pointer(self, bytes: u32) -> Field<String> {
        crate::JsonView {
            source: self.source,
            node: self.value.node,
        }
        .pointer(bytes)
    }
}
#[derive(Clone, Copy)]
pub struct SampleChildren<'a> {
    source: &'a Source,
    values: ValueRefs<'a>,
}

// Model candidates retain their effective coordinates; the public sample view stays role-typed.
#[derive(Clone, Copy)]
pub(crate) enum BorrowedValue<'a> {
    Sample(SampleValueView<'a>),
    Model(crate::EffectiveJson<'a>),
    Constraint(crate::EffectiveJson<'a>),
}
pub(crate) enum BorrowedChildren<'a> {
    Sample(SampleChildren<'a>),
    Model(crate::EffectiveChildren<'a>),
    Constraint(crate::EffectiveChildren<'a>),
}
impl<'a> Iterator for BorrowedChildren<'a> {
    type Item = BorrowedValue<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Sample(values) => values.next().map(BorrowedValue::Sample),
            Self::Model(values) => values.next().map(BorrowedValue::Model),
            Self::Constraint(values) => values.next().map(BorrowedValue::Constraint),
        }
    }
}
impl<'a> BorrowedValue<'a> {
    pub(crate) fn node(self) -> usize {
        match self {
            Self::Sample(v) => v.node(),
            Self::Model(v) | Self::Constraint(v) => v.node,
        }
    }
    pub(crate) fn kind(self) -> crate::JsonKind {
        match self {
            Self::Sample(v) => v.kind(),
            Self::Model(v) | Self::Constraint(v) => v.kind(),
        }
    }
    pub(crate) fn raw_json(self) -> &'a str {
        match self {
            Self::Sample(v) => v.raw_json(),
            Self::Model(v) | Self::Constraint(v) => v.n().source.raw_json(),
        }
    }
    pub(crate) fn children(self) -> BorrowedChildren<'a> {
        match self {
            Self::Sample(v) => BorrowedChildren::Sample(v.children()),
            Self::Model(v) => BorrowedChildren::Model(v.children()),
            Self::Constraint(v) => BorrowedChildren::Constraint(v.children()),
        }
    }
    pub(crate) fn member_name(self) -> Field<&'a str> {
        match self {
            Self::Sample(v) => v.member_name(),
            Self::Model(v) | Self::Constraint(v) => v.member_name(),
        }
    }
    pub(crate) fn text(self) -> Field<&'a str> {
        match self {
            Self::Sample(v) => v.text(),
            Self::Model(v) | Self::Constraint(v) => v.text(),
        }
    }
    pub(crate) fn boolean(self) -> Field<bool> {
        match self {
            Self::Sample(v) => v.boolean(),
            Self::Model(v) | Self::Constraint(v) => v.boolean(),
        }
    }
    pub(crate) fn number(self) -> Field<ExactNumber<'a>> {
        match self {
            Self::Sample(v) => v.number(),
            Self::Model(v) | Self::Constraint(v) => v.number(),
        }
    }
    pub(crate) fn available(self) -> bool {
        match self {
            Self::Sample(_) => true,
            Self::Model(v) | Self::Constraint(v) => {
                v.n().unavailable.is_none()
                    && v.n().donor_unavailable.is_none()
                    && v.n().meaning_unavailable.is_none()
            }
        }
    }
    pub(crate) fn parent(self, boundary: Self) -> Result<Option<Self>, ()> {
        let same_arena = match (self, boundary) {
            (Self::Sample(v), Self::Sample(root)) => std::ptr::eq(v.source, root.source),
            (Self::Model(v), Self::Model(root)) => std::ptr::eq(v.model, root.model),
            (Self::Constraint(v), Self::Constraint(root)) => std::ptr::eq(v.model, root.model),
            _ => false,
        };
        if !same_arena {
            return Err(());
        }
        if self.node() == boundary.node() {
            return Ok(None);
        }
        match self {
            Self::Sample(v) => v.parent().map(Self::Sample).map(Some).ok_or(()),
            Self::Model(v) => v
                .n()
                .parent
                .map(|node| {
                    Some(Self::Model(crate::EffectiveJson {
                        model: v.model,
                        node,
                    }))
                })
                .ok_or(()),
            Self::Constraint(v) => v
                .n()
                .parent
                .map(|node| {
                    Some(Self::Constraint(crate::EffectiveJson {
                        model: v.model,
                        node,
                    }))
                })
                .ok_or(()),
        }
    }
    pub(crate) fn member(self, key: &str, work: &mut usize) -> Field<Self> {
        if !self.available() || self.kind() != crate::JsonKind::Map {
            return Field::Unavailable("value map unavailable");
        }
        let mut found = None;
        for child in self.children() {
            let Field::Present(name) = child.member_name() else {
                return Field::Unavailable("value member identity unavailable");
            };
            if !crate::allowance::charge(work, name.len() + key.len() + 1) {
                return Field::Unavailable("value member work limit");
            }
            if name == key {
                found = Some(child);
            }
        }
        match found {
            Some(value) if !value.available() => Field::Unavailable("value member unavailable"),
            Some(value) => Field::Present(value),
            None => Field::Absent,
        }
    }
    pub(crate) fn map_count(self, budget: &mut crate::allowance::OperationBudget) -> Option<usize> {
        let mut count = 0usize;
        for child in self.children() {
            let Field::Present(name) = child.member_name() else {
                return None;
            };
            if !crate::allowance::charge(&mut budget.work, name.len() + 1) {
                return None;
            }
            count = count.checked_add(1)?;
        }
        Some(count)
    }
}
#[derive(Clone, Copy)]
pub(crate) struct ValueContext<'a> {
    pub(crate) description: crate::EffectiveJson<'a>,
    pub(crate) root: BorrowedValue<'a>,
}
impl ValueContext<'_> {
    pub(crate) fn valid(&self) -> bool {
        match self.root {
            BorrowedValue::Sample(_) => true,
            BorrowedValue::Model(root) => std::ptr::eq(root.model, self.description.model),
            BorrowedValue::Constraint(_) => false,
        }
    }
}
impl<'a> Iterator for SampleChildren<'a> {
    type Item = SampleValueView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.values.next().map(|value| SampleValueView {
            source: self.source,
            value,
        })
    }
}
