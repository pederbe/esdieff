//! Retain exact source tokens while bounding decoding and unavailable typed meaning.
//! Follows ADR0012: Exact numbers and JSON input policies, and
//! ADR0009: Bounded processing and reporting.

use crate::Limits;
use std::{cmp::Ordering, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum JsonKind {
    Null,
    Boolean,
    Number,
    String,
    Array,
    Map,
}
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Field<T> {
    Absent,
    Present(T),
    Unavailable(&'static str),
}
impl<T> Field<T> {
    pub fn present(self) -> Option<T> {
        match self {
            Self::Present(value) => Some(value),
            _ => None,
        }
    }
    pub fn require(self) -> Result<T, &'static str> {
        match self {
            Self::Present(v) => Ok(v),
            Self::Absent => Err("absent"),
            Self::Unavailable(r) => Err(r),
        }
    }
}

pub(crate) enum Value {
    Null,
    Bool(bool),
    Number(Option<Decimal>),
    String(Option<String>),
    Array,
    Map,
}
pub(crate) struct Node {
    pub(crate) value: Value,
    pub(crate) span: Range<usize>,
    pub(crate) parent: Option<usize>,
    pub(crate) key: Option<String>,
    pub(crate) index: Option<usize>,
    pub(crate) first: Option<usize>,
    pub(crate) next: Option<usize>,
    pub(crate) capable: bool,
}
pub(crate) struct Parsed {
    pub(crate) nodes: Vec<Node>,
    pub(crate) root: Option<usize>,
    pub(crate) issues: Vec<Issue>,
    pub(crate) rejected: bool,
    pub(crate) issue_count: usize,
    pub(crate) errors: usize,
    pub(crate) allocation_issue: Option<Issue>,
    pub(crate) omitted_details: OmissionCauses,
}
#[derive(Default)]
pub(crate) struct OmissionCauses {
    pub(crate) count: bool,
    pub(crate) bytes: bool,
    pub(crate) allocation: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Issue {
    pub(crate) code: &'static str,
    pub(crate) error: bool,
    pub(crate) span: Option<Range<usize>>,
    pub(crate) reason: &'static str,
}
pub(crate) struct Budget {
    pub(crate) nodes: usize,
    pub(crate) allocation: usize,
    pub(crate) work: usize,
    pub(crate) numeric: usize,
    conditions: usize,
    detail: usize,
    path_tokens: usize,
}
impl Budget {
    pub(crate) fn new(l: &Limits) -> Self {
        Self {
            nodes: l.nodes,
            allocation: l.allocation,
            work: l.work,
            numeric: l.number_work,
            conditions: l.conditions,
            detail: l.detail_bytes,
            path_tokens: 16 * 1024 * 1024,
        }
    }
}

pub(crate) struct Decimal {
    pub(crate) negative: bool,
    pub(crate) coefficient: String,
    pub(crate) power: i64,
}
#[derive(Clone, Copy)]
pub struct ExactNumber<'a> {
    pub(crate) raw: &'a str,
    pub(crate) decimal: &'a Decimal,
}
impl ExactNumber<'_> {
    pub fn lexeme(&self) -> &str {
        self.raw
    }
    pub fn is_integer(&self) -> bool {
        self.decimal.coefficient == "0" || self.decimal.power >= 0
    }
    pub fn sign(&self) -> Ordering {
        if self.decimal.coefficient == "0" {
            Ordering::Equal
        } else if self.decimal.negative {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
    pub fn compare(
        &self,
        other: ExactNumber<'_>,
        work: &mut NumberWork,
    ) -> Result<Ordering, &'static str> {
        work.debit(
            self.decimal
                .coefficient
                .len()
                .max(other.decimal.coefficient.len())
                + 1,
        )?;
        let a = self.decimal;
        let b = other.decimal;
        if self.sign() != other.sign() {
            return Ok(self.sign().cmp(&other.sign()));
        }
        if self.sign() == Ordering::Equal {
            return Ok(Ordering::Equal);
        }
        let mut order = (a.coefficient.len() as i128 + i128::from(a.power))
            .cmp(&(b.coefficient.len() as i128 + i128::from(b.power)));
        if order == Ordering::Equal {
            for index in 0..a.coefficient.len().max(b.coefficient.len()) {
                order = a
                    .coefficient
                    .as_bytes()
                    .get(index)
                    .copied()
                    .unwrap_or(b'0')
                    .cmp(&b.coefficient.as_bytes().get(index).copied().unwrap_or(b'0'));
                if order != Ordering::Equal {
                    break;
                }
            }
        }
        Ok(if a.negative { order.reverse() } else { order })
    }
    pub fn to_i64(&self) -> Result<i64, &'static str> {
        if !self.is_integer() || self.decimal.power > 19 {
            return Err("not representable");
        }
        let mut number = 0i64;
        for digit in self.decimal.coefficient.bytes() {
            number = number
                .checked_mul(10)
                .and_then(|n| {
                    if self.decimal.negative {
                        n.checked_sub(i64::from(digit - b'0'))
                    } else {
                        n.checked_add(i64::from(digit - b'0'))
                    }
                })
                .ok_or("not representable")?;
        }
        for _ in 0..self.decimal.power.max(0) {
            number = number.checked_mul(10).ok_or("not representable")?;
        }
        Ok(number)
    }
}
pub struct NumberWork {
    remaining: usize,
    // A refused debit leaves the remainder for smaller debits, so record the refusal itself.
    exhausted: bool,
}
impl NumberWork {
    pub(crate) fn from_remaining(remaining: usize) -> Self {
        Self {
            remaining,
            exhausted: false,
        }
    }
    pub(crate) fn remaining(&self) -> usize {
        self.remaining
    }
    pub(crate) fn exhausted(&self) -> bool {
        self.exhausted
    }
    pub(crate) fn debit(&mut self, units: usize) -> Result<(), &'static str> {
        let Some(remaining) = self.remaining.checked_sub(units) else {
            self.exhausted = true;
            return Err("numeric work limit");
        };
        self.remaining = remaining;
        Ok(())
    }
    pub fn new(units: u32) -> Result<Self, &'static str> {
        if units == 0 {
            Err("finite positive work required")
        } else {
            Ok(Self::from_remaining(units as usize))
        }
    }
}

#[derive(Clone, Copy)]
pub struct JsonView<'a> {
    pub(crate) source: &'a crate::Source,
    pub(crate) node: usize,
}
pub(crate) enum PointerError {
    NotAvailable,
    Limit,
}
#[derive(Clone)]
pub(crate) struct PointerBudget {
    remaining: usize,
}
impl PointerBudget {
    pub(crate) fn counter(&mut self) -> &mut usize {
        &mut self.remaining
    }
    pub(crate) fn new(remaining: usize) -> Self {
        Self { remaining }
    }
    pub(crate) fn debit(&mut self, cost: usize) -> Result<(), PointerError> {
        match self.remaining.checked_sub(cost) {
            Some(remaining) => {
                self.remaining = remaining;
                Ok(())
            }
            None => {
                self.remaining = 0;
                Err(PointerError::Limit)
            }
        }
    }
}
pub(crate) struct PointerSize {
    pub(crate) bytes: usize,
    depth: usize,
}
impl PointerSize {
    pub(crate) fn render_work(&self) -> usize {
        // Ancestry collection, rendering visits, key scans and output writes.
        2 * self.depth + 1 + 2 * self.bytes
    }
}
fn pointer_token_size(
    node: &Node,
    remaining: usize,
    work: Option<&mut PointerBudget>,
) -> Result<usize, PointerError> {
    let token = if let Some(key) = &node.key {
        if key.len() + 1 > remaining {
            return Err(PointerError::Limit);
        }
        if let Some(work) = work {
            work.debit(key.len())?;
        }
        key.len() + key.bytes().filter(|b| matches!(b, b'~' | b'/')).count()
    } else if let Some(index) = node.index {
        if index == 0 {
            1
        } else {
            index.ilog10() as usize + 1
        }
    } else {
        return Err(PointerError::NotAvailable);
    };
    if token + 1 > remaining {
        Err(PointerError::Limit)
    } else {
        Ok(token + 1)
    }
}
/// Check a JSON pointer selector's shape and escapes before any selection work.
pub(crate) fn validate_pointer(pointer: &str) -> Result<(), &'static str> {
    if pointer.len() > 8192 || !pointer.is_empty() && !pointer.starts_with('/') {
        return Err("invalid or oversized JSON pointer selector");
    }
    let bytes = pointer.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            index += 1;
            if !matches!(bytes.get(index), Some(b'0' | b'1')) {
                return Err("invalid JSON pointer escape");
            }
        }
        index += 1;
    }
    Ok(())
}
impl<'a> JsonView<'a> {
    pub(crate) fn n(self) -> &'a Node {
        &self.source.parsed.nodes[self.node]
    }
    pub fn kind(self) -> JsonKind {
        match self.n().value {
            Value::Null => JsonKind::Null,
            Value::Bool(_) => JsonKind::Boolean,
            Value::Number(_) => JsonKind::Number,
            Value::String(_) => JsonKind::String,
            Value::Array => JsonKind::Array,
            Value::Map => JsonKind::Map,
        }
    }
    pub fn raw_json(self) -> &'a str {
        std::str::from_utf8(&self.source.raw_bytes()[self.n().span.clone()]).expect("parsed UTF-8")
    }
    pub fn span(self) -> Range<usize> {
        self.n().span.clone()
    }
    pub fn source_id(self) -> crate::SourceId {
        self.source.id
    }
    pub fn boolean(self) -> Field<bool> {
        match self.n().value {
            Value::Bool(v) => Field::Present(v),
            _ => Field::Unavailable("Boolean shape"),
        }
    }
    pub fn text(self) -> Field<&'a str> {
        match &self.n().value {
            Value::String(Some(v)) => Field::Present(v),
            Value::String(None) => Field::Unavailable("Unicode scalar decoding unavailable"),
            _ => Field::Unavailable("text shape"),
        }
    }
    pub fn number(self) -> Field<ExactNumber<'a>> {
        match &self.n().value {
            Value::Number(Some(decimal)) => Field::Present(ExactNumber {
                raw: self.raw_json(),
                decimal,
            }),
            Value::Number(None) => Field::Unavailable("exact number capability"),
            _ => Field::Unavailable("number shape"),
        }
    }
    pub fn get(self, name: &str) -> Field<Self> {
        if self.kind() != JsonKind::Map {
            return Field::Unavailable("map shape");
        }
        let mut found = None;
        let mut unknown = false;
        for child in self.children() {
            match child.n().key.as_deref() {
                Some(key) if key == name => found = Some(child),
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
    pub fn at(self, index: usize) -> Field<Self> {
        if self.kind() != JsonKind::Array {
            return Field::Unavailable("array shape");
        }
        self.children()
            .find(|n| n.n().index == Some(index))
            .map_or(Field::Absent, Field::Present)
    }
    pub fn children(self) -> Children<'a> {
        Children {
            source: self.source,
            next: self.n().first,
        }
    }
    pub fn member_name(self) -> Field<&'a str> {
        self.n().key.as_deref().map_or(
            if self.n().index.is_some() || self.n().parent.is_none() {
                Field::Absent
            } else {
                Field::Unavailable("member name decoding")
            },
            Field::Present,
        )
    }
    pub fn capability_complete(self) -> bool {
        self.n().capable
    }
    pub(crate) fn pointer_size(
        self,
        limit: usize,
        work: &mut PointerBudget,
    ) -> Result<PointerSize, PointerError> {
        self.measure_pointer(limit, Some(work))
    }
    fn measure_pointer(
        self,
        limit: usize,
        mut work: Option<&mut PointerBudget>,
    ) -> Result<PointerSize, PointerError> {
        let mut size = 0usize;
        let mut depth = 0usize;
        let mut node = self.node;
        loop {
            if let Some(work) = work.as_deref_mut() {
                work.debit(1)?;
            }
            let current = &self.source.parsed.nodes[node];
            let Some(parent) = current.parent else {
                break;
            };
            size += pointer_token_size(current, limit - size, work.as_deref_mut())?;
            depth += 1;
            node = parent;
        }
        Ok(PointerSize { bytes: size, depth })
    }
    /// Render a source pointer only after its escaped size fits the finite caller budget.
    pub fn pointer(self, byte_limit: u32) -> Field<String> {
        if byte_limit == 0 || byte_limit > 8192 {
            return Field::Unavailable("unsupported pointer output budget");
        }
        match self.measure_pointer(byte_limit as usize, None) {
            Ok(size) => Field::Present(self.render_pointer(size)),
            Err(PointerError::NotAvailable) => {
                Field::Unavailable("source member identity unavailable")
            }
            Err(PointerError::Limit) => Field::Unavailable("pointer output limit"),
        }
    }
    pub(crate) fn render_pointer(self, size: PointerSize) -> String {
        let mut path = Vec::with_capacity(size.depth);
        let mut node = self.node;
        while let Some(parent) = self.source.parsed.nodes[node].parent {
            path.push(node);
            node = parent;
        }
        let mut pointer = String::with_capacity(size.bytes);
        for node in path.into_iter().rev() {
            let current = &self.source.parsed.nodes[node];
            pointer.push('/');
            if let Some(key) = &current.key {
                push_pointer_token(&mut pointer, key);
            } else {
                use std::fmt::Write;
                write!(pointer, "{}", current.index.expect("array index")).expect("String write");
            }
        }
        pointer
    }
}
/// Append `token` to a JSON pointer, escaping `~` as `~0` and `/` as `~1`.
pub(crate) fn push_pointer_token(pointer: &mut String, token: &str) {
    for ch in token.chars() {
        match ch {
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            ch => pointer.push(ch),
        }
    }
}
pub struct Children<'a> {
    source: &'a crate::Source,
    next: Option<usize>,
}
impl<'a> Iterator for Children<'a> {
    type Item = JsonView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next?;
        self.next = self.source.parsed.nodes[node].next;
        Some(JsonView {
            source: self.source,
            node,
        })
    }
}
impl Parsed {
    pub(crate) fn unavailable() -> Self {
        Self {
            nodes: Vec::new(),
            root: None,
            issues: Vec::new(),
            rejected: false,
            issue_count: 0,
            errors: 0,
            allocation_issue: None,
            omitted_details: OmissionCauses::default(),
        }
    }
    fn issue(
        &mut self,
        budget: &mut Budget,
        code: &'static str,
        error: bool,
        span: Option<Range<usize>>,
        reason: &'static str,
    ) {
        self.issue_count += 1;
        if error {
            self.errors += 1;
        }
        if budget.conditions > 0 && budget.detail >= 256 && budget.allocation >= 256 {
            budget.conditions -= 1;
            budget.detail -= 256;
            budget.allocation -= 256;
            self.issues.push(Issue {
                code,
                error,
                span,
                reason,
            });
        } else if self.allocation_issue.is_none() {
            // An inline constant-size failure slot survives exhaustion of derived storage.
            self.allocation_issue = Some(Issue {
                code,
                error,
                span,
                reason,
            });
        } else {
            // Record exhausted gates only when even the inline detail cannot be retained.
            self.omitted_details.count |= budget.conditions == 0;
            self.omitted_details.bytes |= budget.detail < 256;
            self.omitted_details.allocation |= budget.allocation < 256;
        }
    }
}
pub(crate) fn parse(bytes: &[u8], complete: bool, limits: &Limits, budget: &mut Budget) -> Parsed {
    parse_mode(bytes, complete, limits, budget, true)
}
pub(crate) fn parse_value(
    bytes: &[u8],
    complete: bool,
    limits: &Limits,
    budget: &mut Budget,
) -> Parsed {
    parse_mode(bytes, complete, limits, budget, false)
}
#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn parse_mode(
    bytes: &[u8],
    complete: bool,
    limits: &Limits,
    budget: &mut Budget,
    model: bool,
) -> Parsed {
    let mut parsed = Parsed::unavailable();
    if charge(&mut budget.work, bytes.len(), "decoding work limit").is_err() {
        parsed.issue(
            budget,
            "operation.limit",
            false,
            None,
            "decoding work limit",
        );
        return parsed;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        parsed.issue(
            budget,
            "input.encoding",
            false,
            None,
            "UTF-8 decoding unavailable",
        );
        return parsed;
    };
    let mut parser = Parser {
        text,
        pos: 0,
        limits,
        budget,
        parsed: &mut parsed,
    };
    let result = parser.value(None, None, None, 0).and_then(|root| {
        parser.whitespace();
        if parser.pos != text.len() {
            return Err("syntax");
        }
        Ok(root)
    });
    match result {
        Ok(root) => {
            if model && !matches!(parser.parsed.nodes[root].value, Value::Map) {
                parser.parsed.issue(
                    parser.budget,
                    "document.envelope",
                    true,
                    Some(0..text.len()),
                    "SDF document requires one JSON map",
                );
                parser.parsed.rejected = true;
            }
            parser.parsed.root = Some(root);
        }
        Err("syntax") | Err("EOF") if complete || matches!(result, Err("syntax")) => {
            parser.parsed.issue(
                parser.budget,
                "json.syntax",
                true,
                Some(parser.pos..parser.pos),
                "JSON grammar violation",
            );
            parser.parsed.rejected = true;
        }
        Err("EOF") => {
            parser.parsed.issue(
                parser.budget,
                "operation.limit",
                false,
                None,
                "acquisition ended within a JSON token or container",
            );
        }
        Err(reason) => {
            parser.parsed.issue(
                parser.budget,
                "operation.limit",
                false,
                Some(parser.pos..parser.pos),
                reason,
            );
        }
    }
    parsed
}
struct Parser<'a, 'b> {
    text: &'a str,
    pos: usize,
    limits: &'a Limits,
    budget: &'b mut Budget,
    parsed: &'b mut Parsed,
}
fn charge(remaining: &mut usize, units: usize, reason: &'static str) -> Result<(), &'static str> {
    *remaining = remaining.checked_sub(units).ok_or(reason)?;
    Ok(())
}
impl Parser<'_, '_> {
    fn whitespace(&mut self) {
        while matches!(
            self.text.as_bytes().get(self.pos),
            Some(b' ' | b'\n' | b'\r' | b'\t')
        ) {
            self.pos += 1;
        }
    }
    fn byte(&mut self, byte: u8) -> Result<(), &'static str> {
        let found = self.text.as_bytes().get(self.pos).ok_or("EOF")?;
        if *found != byte {
            return Err("syntax");
        }
        self.pos += 1;
        Ok(())
    }
    fn value(
        &mut self,
        parent: Option<usize>,
        key: Option<String>,
        index: Option<usize>,
        depth: usize,
    ) -> Result<usize, &'static str> {
        self.whitespace();
        if depth > self.limits.depth
            && matches!(self.text.as_bytes().get(self.pos), Some(b'{' | b'['))
        {
            return Err("JSON depth limit");
        }
        if self.parsed.nodes.len() >= self.limits.document_nodes {
            return Err("document node limit");
        }
        charge(&mut self.budget.nodes, 1, "catalogue parsed-node limit")?;
        charge(&mut self.budget.work, 1, "decoding work limit")?;
        if self.parsed.nodes.len() == self.parsed.nodes.capacity() {
            let old = self.parsed.nodes.capacity();
            let new = (old.max(4) * 2).min(self.limits.document_nodes);
            charge(
                &mut self.budget.allocation,
                (new - old) * std::mem::size_of::<Node>(),
                "parsed allocation limit",
            )?;
            self.parsed.nodes.reserve_exact(new - old);
        }
        let id = self.parsed.nodes.len();
        let start = self.pos;
        self.parsed.nodes.push(Node {
            value: Value::Null,
            span: start..start,
            parent,
            key,
            index,
            first: None,
            next: None,
            capable: true,
        });
        match self.text.as_bytes().get(self.pos).copied().ok_or("EOF")? {
            b'{' | b'[' => {
                let map = self.text.as_bytes()[self.pos] == b'{';
                self.pos += 1;
                self.parsed.nodes[id].value = if map { Value::Map } else { Value::Array };
                let close = if map { b'}' } else { b']' };
                self.whitespace();
                let mut previous: Option<usize> = None;
                let mut child_index = 0;
                let mut keys = std::collections::HashMap::<u64, Vec<usize>>::new();
                if self.text.as_bytes().get(self.pos) != Some(&close) {
                    loop {
                        self.whitespace();
                        let key = if map {
                            let name = self.string(true)?;
                            self.whitespace();
                            self.byte(b':')?;
                            name
                        } else {
                            None
                        };
                        let child = self.value(
                            Some(id),
                            key,
                            if map { None } else { Some(child_index) },
                            depth + 1,
                        )?;
                        if let Some(previous) = previous {
                            self.parsed.nodes[previous].next = Some(child);
                        } else {
                            self.parsed.nodes[id].first = Some(child);
                        }
                        if !self.parsed.nodes[child].capable
                            || (map && self.parsed.nodes[child].key.is_none())
                        {
                            self.parsed.nodes[id].capable = false;
                        }
                        if map && let Some(name) = self.parsed.nodes[child].key.as_deref() {
                            use std::hash::{Hash, Hasher};
                            charge(
                                &mut self.budget.work,
                                name.len() + 1,
                                "decoded-key work limit",
                            )?;
                            let mut hash = std::collections::hash_map::DefaultHasher::new();
                            name.hash(&mut hash);
                            let hash = hash.finish();
                            let mut duplicate = false;
                            if let Some(previous) = keys.get(&hash) {
                                for old in previous {
                                    let old_name = self.parsed.nodes[*old]
                                        .key
                                        .as_deref()
                                        .expect("decoded key");
                                    charge(
                                        &mut self.budget.work,
                                        name.len() + old_name.len() + 1,
                                        "decoded-key comparison limit",
                                    )?;
                                    if name == old_name {
                                        duplicate = true;
                                        break;
                                    }
                                }
                            }
                            if duplicate {
                                self.parsed.issue(
                                    self.budget,
                                    "input.duplicate_name",
                                    true,
                                    Some(self.parsed.nodes[child].span.clone()),
                                    "duplicate decoded object member refused under A002",
                                );
                                self.parsed.rejected = true;
                            }
                            charge(
                                &mut self.budget.allocation,
                                256,
                                "decoded-key index allocation limit",
                            )?;
                            keys.entry(hash).or_default().push(child);
                        }
                        previous = Some(child);
                        child_index += 1;
                        self.whitespace();
                        if self.text.as_bytes().get(self.pos) == Some(&close) {
                            break;
                        }
                        self.byte(b',')?;
                    }
                }
                self.byte(close)?;
            }
            b'"' => {
                let text = self.string(false)?;
                self.parsed.nodes[id].capable = text.is_some();
                self.parsed.nodes[id].value = Value::String(text);
            }
            b't' => {
                self.literal("true")?;
                self.parsed.nodes[id].value = Value::Bool(true);
            }
            b'f' => {
                self.literal("false")?;
                self.parsed.nodes[id].value = Value::Bool(false);
            }
            b'n' => {
                self.literal("null")?;
            }
            b'-' | b'0'..=b'9' => {
                let start = self.pos;
                self.number_shape()?;
                let raw = &self.text[start..self.pos];
                let number = self.decimal(raw);
                if let Err(reason) = number {
                    self.parsed.issue(
                        self.budget,
                        "operation.limit",
                        false,
                        Some(start..self.pos),
                        reason,
                    );
                }
                let number = number.ok();
                self.parsed.nodes[id].capable = number.is_some();
                self.parsed.nodes[id].value = Value::Number(number);
            }
            _ => return Err("syntax"),
        }
        self.parsed.nodes[id].span.end = self.pos;
        Ok(id)
    }
    fn literal(&mut self, literal: &str) -> Result<(), &'static str> {
        let tail = &self.text[self.pos..];
        if !tail.starts_with(literal) {
            return Err(if literal.starts_with(tail) {
                "EOF"
            } else {
                "syntax"
            });
        }
        self.pos += literal.len();
        Ok(())
    }
    fn number_shape(&mut self) -> Result<(), &'static str> {
        let start = self.pos;
        let bytes = self.text.as_bytes();
        if bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        match bytes.get(self.pos) {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.pos += 1;
                while matches!(bytes.get(self.pos), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            None => return Err("EOF"),
            _ => return Err("syntax"),
        }
        if bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            let start = self.pos;
            while matches!(bytes.get(self.pos), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if start == self.pos {
                return Err(if self.pos == bytes.len() {
                    "EOF"
                } else {
                    "syntax"
                });
            }
        }
        if matches!(bytes.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let start = self.pos;
            while matches!(bytes.get(self.pos), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if start == self.pos {
                return Err(if self.pos == bytes.len() {
                    "EOF"
                } else {
                    "syntax"
                });
            }
        }
        charge(
            &mut self.budget.work,
            self.pos - start,
            "decoding work limit",
        )?;
        Ok(())
    }
    fn decimal(&mut self, raw: &str) -> Result<Decimal, &'static str> {
        if raw.len() > self.limits.number_token {
            return Err("number token limit");
        }
        charge(&mut self.budget.numeric, raw.len(), "numeric work limit")?;
        let negative = raw.starts_with('-');
        let unsigned = raw.trim_start_matches('-');
        let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
            Some((m, e)) => (
                m,
                e.parse::<i64>()
                    .map_err(|_| "lexical exponent capability")?,
            ),
            None => (unsigned, 0),
        };
        if exponent
            .checked_abs()
            .ok_or("lexical exponent capability")?
            > self.limits.exponent
        {
            return Err("lexical exponent capability");
        }
        charge(
            &mut self.budget.allocation,
            mantissa.len(),
            "parsed number allocation limit",
        )?;
        let mut digits = String::with_capacity(mantissa.len());
        for digit in mantissa.bytes() {
            if digit != b'.' {
                digits.push(char::from(digit));
            }
        }
        let digits = digits.trim_start_matches('0');
        let coefficient = digits.trim_end_matches('0');
        if coefficient.len() > self.limits.coefficient {
            return Err("coefficient digit limit");
        }
        charge(
            &mut self.budget.allocation,
            coefficient.len().max(1),
            "parsed coefficient allocation limit",
        )?;
        if digits.is_empty() {
            return Ok(Decimal {
                negative: false,
                coefficient: "0".to_owned(),
                power: 0,
            });
        }
        let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
        Ok(Decimal {
            negative,
            coefficient: coefficient.to_owned(),
            power: exponent - fraction as i64 + (digits.len() - coefficient.len()) as i64,
        })
    }
    fn string(&mut self, key: bool) -> Result<Option<String>, &'static str> {
        self.byte(b'"')?;
        let start = self.pos;
        let mut scan = self.pos;
        let bytes = self.text.as_bytes();
        loop {
            match bytes.get(scan).copied().ok_or("EOF")? {
                b'"' => break,
                b'\\' => {
                    scan += 1;
                    match bytes.get(scan).copied().ok_or("EOF")? {
                        b'u' => {
                            for offset in 1..=4 {
                                let byte = bytes.get(scan + offset).ok_or("EOF")?;
                                if !byte.is_ascii_hexdigit() {
                                    return Err("syntax");
                                }
                            }
                            scan += 4;
                        }
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                        _ => return Err("syntax"),
                    }
                }
                0..=31 => return Err("syntax"),
                _ => {}
            }
            scan += 1;
        }
        charge(
            &mut self.budget.work,
            scan - start + 1,
            "string decoding work limit",
        )?;
        if key {
            charge(
                &mut self.budget.path_tokens,
                scan - start,
                "decoded path-token storage limit",
            )?;
        }
        charge(
            &mut self.budget.allocation,
            scan - start,
            "decoded string allocation limit",
        )?;
        let mut output = String::with_capacity(scan - start);
        let mut scalar = true;
        while self.pos < scan {
            let ch = self.text[self.pos..].chars().next().ok_or("syntax")?;
            self.pos += ch.len_utf8();
            if ch != '\\' {
                output.push(ch);
                continue;
            }
            let escape = bytes[self.pos];
            self.pos += 1;
            match escape {
                b'u' => {
                    let high = u16::from_str_radix(&self.text[self.pos..self.pos + 4], 16)
                        .map_err(|_| "syntax")?;
                    self.pos += 4;
                    let value = if (0xd800..=0xdbff).contains(&high)
                        && self.text[self.pos..scan].starts_with("\\u")
                    {
                        let low = u16::from_str_radix(&self.text[self.pos + 2..self.pos + 6], 16)
                            .map_err(|_| "syntax")?;
                        if (0xdc00..=0xdfff).contains(&low) {
                            self.pos += 6;
                            Some(
                                0x10000 + (u32::from(high) - 0xd800) * 1024 + u32::from(low)
                                    - 0xdc00,
                            )
                        } else {
                            None
                        }
                    } else if (0xd800..=0xdfff).contains(&high) {
                        None
                    } else {
                        Some(u32::from(high))
                    };
                    if let Some(ch) = value.and_then(char::from_u32) {
                        output.push(ch);
                    } else {
                        scalar = false;
                    }
                }
                b'"' => output.push('"'),
                b'\\' => output.push('\\'),
                b'/' => output.push('/'),
                b'b' => output.push('\u{8}'),
                b'f' => output.push('\u{c}'),
                b'n' => output.push('\n'),
                b'r' => output.push('\r'),
                b't' => output.push('\t'),
                _ => return Err("syntax"),
            }
        }
        self.pos = scan + 1;
        if key {
            self.budget.path_tokens += scan - start - output.len();
        }
        if scalar {
            Ok(Some(output))
        } else {
            self.parsed.issue(
                self.budget,
                "input.unicode_unavailable",
                false,
                Some(start - 1..self.pos),
                "non-scalar escape decoding unavailable; raw JSON retained",
            );
            Ok(None)
        }
    }
}
