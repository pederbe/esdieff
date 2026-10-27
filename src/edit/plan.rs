//! Textual intent shares decoding and operation quotas under ADR 0034, Authored-quality editing
//! with source substitution.
use super::*;
use crate::json::{Parsed, Value};

impl EditRequest {
    /// Read a bounded explicit plan and retain its original request coordinates.
    pub fn from_plan(mut catalogue: Catalogue, bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > (4 * 1024 * 1024).min(catalogue.limits.document_bytes) {
            return Err("edit plan exceeds byte ceiling");
        }
        if std::str::from_utf8(bytes).is_err() {
            return Err("edit plan requires UTF-8 JSON");
        }
        if catalogue.attempt_count() >= catalogue.limits.documents {
            return Err("generated attempt exceeds source-attempt ceiling");
        }
        validate_metadata(&catalogue, 0)?;
        let (parsed, decoding_work) = catalogue.lend_decoding(|catalogue, decoding| {
            if !charge(&mut decoding.allocation, bytes.len()) {
                return Err("edit plan retained-byte allocation limit before request admission");
            }
            let parsed = crate::json::parse_value(bytes, true, &catalogue.limits, decoding);
            if parsed.rejected || parsed.errors > 0 {
                return Err("edit plan syntax or duplicate decoded member rejected");
            }
            Ok((parsed, decoding.work))
        })?;
        let root = parsed
            .root
            .ok_or("edit plan decoding limit before complete request admission")?;
        let mut operation = OperationBudget::new(&catalogue.limits);
        operation.work = operation.work.min(decoding_work);
        let mut cursor = Cursor {
            parsed: &parsed,
            budget: &mut operation,
        };
        cursor.fields(
            root,
            &["key", "source", "generated_key", "generated_label", "edits"],
        )?;
        let key = cursor.text(root, "key")?;
        let source_key = cursor.text(root, "source")?;
        let generated_key = cursor.text(root, "generated_key")?;
        if key.len() > 256 || source_key.len() > 256 || generated_key.len() > 256 {
            return Err("edit plan request/source key exceeds 256 bytes");
        }
        let mut selected = None;
        for source in catalogue.sources() {
            if !charge(
                &mut cursor.budget.work,
                source.key.len() + source_key.len() + 1,
            ) {
                return Err("edit plan source selection work limit before request admission");
            }
            if source.key == source_key {
                selected = Some(source.id);
                break;
            }
        }
        let selected = selected.ok_or("edit plan source key is not supplied")?;
        let label = cursor
            .member(root, "generated_label")?
            .map(|node| cursor.string(node))
            .transpose()?;
        if label.is_some_and(|label| label.len() > 512) {
            return Err("one bounded generated label required");
        }
        let edits = cursor.required(root, "edits")?;
        if !matches!(parsed.nodes[edits].value, Value::Array) {
            return Err("edit plan edits must be an array");
        }
        let mut metadata = key.len() + generated_key.len() + label.map_or(0, str::len);
        let mut intents: Vec<Intent<'_>> = Vec::new();
        let mut next = parsed.nodes[edits].first;
        while let Some(node) = next {
            cursor.fields(node, &["owner", "quality", "operation", "value"])?;
            let owner = cursor.text(node, "owner")?;
            let quality = cursor.text(node, "quality")?;
            let action = cursor.text(node, "operation")?;
            let value = cursor.member(node, "value")?;
            let action = match (action, value.is_some()) {
                ("replace", true) => EditOperation::Replace,
                ("remove", false) => EditOperation::Remove,
                ("replace" | "remove", _) => {
                    return Err("replace requires value and remove forbids value");
                }
                _ => return Err("edit plan operation must be replace or remove"),
            };
            crate::json::validate_pointer(owner)?;
            if !charge(&mut cursor.budget.work, owner.len() * 4 + quality.len() + 1) {
                return Err("edit plan selector validation work limit before request admission");
            }
            if owner.len() > catalogue.limits.pointer_bytes
                || owner.len() > 8192
                || quality.len() > 256
            {
                return Err("edit selector exceeds byte ceiling");
            }
            metadata = metadata
                .checked_add(owner.len() + quality.len() + 32)
                .ok_or("edit metadata overflow")?;
            for previous in &intents {
                if !charge(
                    &mut cursor.budget.work,
                    owner.len() + previous.owner.len() + quality.len() + previous.quality.len() + 1,
                ) {
                    return Err("edit plan duplicate-target work limit before request admission");
                }
                if previous.owner == owner && previous.quality == quality {
                    return Err("duplicate decoded edit target");
                }
            }
            if intents.len() >= 256 {
                return Err("one through 256 edits required");
            }
            if !charge(
                &mut cursor.budget.allocation,
                std::mem::size_of::<Intent<'_>>(),
            ) {
                return Err("edit plan target allocation limit before request admission");
            }
            intents.reserve_exact(1);
            intents.push(Intent {
                owner,
                quality,
                action,
                value,
            });
            next = parsed.nodes[node].next;
        }
        if intents.is_empty() {
            return Err("one through 256 edits required");
        }
        validate_metadata(&catalogue, metadata)?;
        if !charge(
            &mut cursor.budget.allocation,
            metadata + intents.len() * std::mem::size_of::<Edit>(),
        ) || !charge(&mut cursor.budget.nodes, intents.len())
            || !charge(&mut cursor.budget.work, metadata + intents.len())
        {
            return Err("edit plan request allocation limit before admission");
        }
        let mut request = Self::new(catalogue, key, selected, generated_key)?;
        if let Some(label) = label {
            request = request.generated_label(label)?;
        }
        for intent in &intents {
            request.admit_edit(
                Edit {
                    owner: intent.owner.to_owned(),
                    quality: intent.quality.to_owned(),
                    operation: intent.action,
                    value: None,
                },
                true,
            )?;
        }
        // Retain only already charged node indices before moving the parsed evidence.
        let mut values = [None; 256];
        for (index, intent) in intents.iter().enumerate() {
            values[index] = intent.value;
        }
        let plan = crate::Source {
            role: SourceRole::Model,
            original: true,
            id: SourceId(0),
            key: String::new(),
            bytes: bytes.to_owned(),
            label: None,
            uri: None,
            version: None,
            digest: None,
            raw_path: None,
            acquisition: Acquisition::Supplied,
            label_omitted: false,
            complete: true,
            parsed,
        };
        request.catalogue.lend_decoding(|catalogue, decoding| {
            for (index, edit) in request.edits.iter_mut().enumerate() {
                if let Some(node) = values[index] {
                    let value = JsonView {
                        source: &plan,
                        node,
                    };
                    decoding.work = decoding.work.min(operation.work);
                    match JsonValue::copy_authored(value, &catalogue.limits, decoding) {
                        Ok(value) => edit.value = Some(value),
                        Err(reason) => {
                            request.plan_incomplete = Some(reason);
                            break;
                        }
                    }
                    operation.work = operation.work.min(decoding.work);
                }
            }
            decoding.work = decoding.work.min(operation.work);
        });
        request.plan = Some(plan);
        request.request_budget = Some(operation);
        Ok(request)
    }
}

struct Intent<'a> {
    owner: &'a str,
    quality: &'a str,
    action: EditOperation,
    value: Option<usize>,
}

struct Cursor<'a, 'b> {
    parsed: &'a Parsed,
    budget: &'b mut OperationBudget,
}
impl<'a> Cursor<'a, '_> {
    fn fields(&mut self, node: usize, allowed: &[&str]) -> Result<(), &'static str> {
        if !matches!(self.parsed.nodes[node].value, Value::Map) {
            return Err("edit plan and each edit require map fields");
        }
        let mut next = self.parsed.nodes[node].first;
        while let Some(child) = next {
            let key = self.parsed.nodes[child]
                .key
                .as_deref()
                .ok_or("edit plan member decoding unavailable")?;
            if !charge(&mut self.budget.work, key.len() * allowed.len() + 1) {
                return Err("edit plan field validation work limit");
            }
            if !allowed.contains(&key) {
                return Err("unknown edit plan field");
            }
            next = self.parsed.nodes[child].next;
        }
        Ok(())
    }
    fn member(&mut self, node: usize, name: &str) -> Result<Option<usize>, &'static str> {
        let mut next = self.parsed.nodes[node].first;
        while let Some(child) = next {
            let key = self.parsed.nodes[child]
                .key
                .as_deref()
                .ok_or("edit plan member decoding unavailable")?;
            if !charge(&mut self.budget.work, key.len() + name.len() + 1) {
                return Err("edit plan lookup work limit");
            }
            if key == name {
                return Ok(Some(child));
            }
            next = self.parsed.nodes[child].next;
        }
        Ok(None)
    }
    fn required(&mut self, node: usize, name: &str) -> Result<usize, &'static str> {
        self.member(node, name)?
            .ok_or("required edit plan field absent")
    }
    fn text(&mut self, node: usize, name: &str) -> Result<&'a str, &'static str> {
        let field = self.required(node, name)?;
        self.string(field)
    }
    fn string(&self, node: usize) -> Result<&'a str, &'static str> {
        match &self.parsed.nodes[node].value {
            Value::String(Some(text)) => Ok(text),
            Value::String(None) => Err("edit plan string decoding unavailable"),
            _ => Err("edit plan field must be a string"),
        }
    }
}
