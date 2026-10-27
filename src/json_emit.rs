//! Bounded JSON emission supports ADR 0017, Reference-preserving sdfThing authoring,
//! and ADR 0034, Authored-quality editing with source substitution.
use crate::allowance::{OperationBudget, charge};
use crate::{JsonKind, JsonValueView};

pub(crate) enum NodePolicy {
    Author,
    Edit { depth: usize },
}

pub(crate) struct JsonWriter<'a> {
    bytes: Vec<u8>,
    cap: usize,
    budget: &'a mut OperationBudget,
    nodes: NodePolicy,
}
impl<'a> JsonWriter<'a> {
    pub(crate) fn new(cap: usize, budget: &'a mut OperationBudget, nodes: NodePolicy) -> Self {
        Self {
            bytes: Vec::new(),
            cap,
            budget,
            nodes,
        }
    }
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub(crate) fn work(&mut self) -> &mut usize {
        &mut self.budget.work
    }
    pub(crate) fn emit(&mut self, text: &str) -> Result<(), &'static str> {
        if text.len() > self.cap.saturating_sub(self.bytes.len())
            || !charge(&mut self.budget.allocation, text.len() * 2)
            || !charge(&mut self.budget.work, text.len() + 1)
        {
            return Err("Generated UTF-8 byte, allocation or work ceiling reached");
        }
        self.bytes.reserve_exact(text.len());
        self.bytes.extend_from_slice(text.as_bytes());
        Ok(())
    }
    pub(crate) fn quoted(&mut self, text: &str) -> Result<(), &'static str> {
        self.emit("\"")?;
        for ch in text.chars() {
            match ch {
                '"' => self.emit("\\\"")?,
                '\\' => self.emit("\\\\")?,
                '\n' => self.emit("\\n")?,
                '\r' => self.emit("\\r")?,
                '\t' => self.emit("\\t")?,
                ch if ch < ' ' => {
                    let hex = b"0123456789abcdef";
                    let scalar = ch as usize;
                    let escape = [b'\\', b'u', b'0', b'0', hex[scalar >> 4], hex[scalar & 15]];
                    self.emit(std::str::from_utf8(&escape).expect("ASCII JSON escape"))?;
                }
                ch => {
                    let mut bytes = [0; 4];
                    self.emit(ch.encode_utf8(&mut bytes))?;
                }
            }
        }
        self.emit("\"")
    }
    pub(crate) fn indent(&mut self, depth: usize) -> Result<(), &'static str> {
        for _ in 0..depth {
            self.emit("  ")?;
        }
        Ok(())
    }
    pub(crate) fn start(&mut self, depth: usize) -> Result<(), &'static str> {
        match self.nodes {
            // Authoring's established literal policy charges nodes but no per-node work.
            NodePolicy::Author => {
                if depth > 64 || !charge(&mut self.budget.nodes, 1) {
                    return Err("generated depth/node ceiling reached");
                }
            }
            NodePolicy::Edit { depth: limit } => {
                if depth > limit
                    || !charge(&mut self.budget.nodes, 1)
                    || !charge(&mut self.budget.work, 1)
                {
                    return Err("Generated depth, node or work ceiling reached");
                }
            }
        }
        Ok(())
    }
    pub(crate) fn value(
        &mut self,
        value: JsonValueView<'_>,
        depth: usize,
    ) -> Result<(), &'static str> {
        self.start(depth)?;
        match value.kind() {
            JsonKind::Map | JsonKind::Array => {
                let map = value.kind() == JsonKind::Map;
                self.emit(if map { "{" } else { "[" })?;
                let mut any = false;
                for child in value.children() {
                    if any {
                        self.emit(",")?;
                    }
                    self.emit("\n")?;
                    self.indent(depth + 1)?;
                    if map {
                        self.quoted(child.member_name().require()?)?;
                        self.emit(": ")?;
                    }
                    self.value(child, depth + 1)?;
                    any = true;
                }
                if any {
                    self.emit("\n")?;
                    self.indent(depth)?;
                }
                self.emit(if map { "}" } else { "]" })
            }
            JsonKind::String => self.quoted(value.text().require()?),
            JsonKind::Number => self.emit(value.number().require()?.lexeme()),
            JsonKind::Boolean => self.emit(if value.boolean().require()? {
                "true"
            } else {
                "false"
            }),
            JsonKind::Null => self.emit("null"),
        }
    }
}
