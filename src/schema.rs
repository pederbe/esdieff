//! Selected effective JSON-value constraints exported to JSON Schema Draft 2020-12.
//! See docs/json-schema.md for fidelity and unsupported meanings.
use crate::allowance::OperationBudget;
use crate::json_emit::{JsonWriter, NodePolicy};
use crate::{
    Catalogue, Disposition, EffectiveJson, EffectiveRequest, Field, JsonKind, SampleKind, SourceId,
};

pub const SCHEMA_CONTRACT: &str = "esdieff.schema/1";
pub const SCHEMA_DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// Uses the same definition pointer and kind as a selected Sample request.
pub struct SchemaRequest<'a> {
    document: SourceId,
    kind: SampleKind,
    pointer: &'a str,
}
impl<'a> SchemaRequest<'a> {
    pub fn new(document: SourceId, kind: SampleKind, pointer: &'a str) -> Self {
        Self {
            document,
            kind,
            pointer,
        }
    }
}

/// One complete schema or an explicit refusal. No partial schema is exposed.
pub struct SchemaResult {
    report: String,
    schema: Option<std::ops::Range<usize>>,
    reason: Option<&'static str>,
}
impl SchemaResult {
    /// The standalone schema, with exact original number tokens.
    pub fn schema(&self) -> Option<&str> {
        self.schema.clone().map(|range| &self.report[range])
    }
    pub fn reason(&self) -> Option<&'static str> {
        self.reason
    }
    /// A versioned fidelity report containing the schema or null and the first refusal.
    pub fn to_json(&self) -> &str {
        &self.report
    }
    pub fn disposition(&self) -> Disposition {
        if self.schema.is_some() {
            Disposition::Passed
        } else {
            Disposition::Undetermined
        }
    }
}

impl Catalogue {
    /// Export one selected effective data description. This does not assess the whole
    /// model, bind it to a wire protocol, or generate Rust types.
    pub fn export_schema(&self, request: SchemaRequest<'_>) -> Result<SchemaResult, &'static str> {
        if request.kind == SampleKind::Data {
            return Err(
                "schema export selects Property, Action input/output, Event output or Context",
            );
        }
        let source = self
            .source(request.document)
            .ok_or("unknown document selector")?;
        if !source.role().is_model() {
            return Err("sample source cannot be a model root");
        }
        if request.pointer.len() > self.limits.pointer_bytes.min(8192) {
            return Err("invalid or oversized JSON pointer selector");
        }
        crate::json::validate_pointer(request.pointer)?;
        let mut budget = OperationBudget::new(&self.limits);
        // Build the mandatory identity before composition and reserve a refusal envelope.
        let mut prefix = JsonWriter::new(self.limits.report_bytes, &mut budget, NodePolicy::Author);
        prefix.emit("{\"contract\":")?;
        prefix.quoted(SCHEMA_CONTRACT)?;
        prefix.emit(",\"document\":")?;
        prefix.quoted(source.key())?;
        prefix.emit(",\"kind\":")?;
        prefix.quoted(request.kind.as_str())?;
        prefix.emit(",\"pointer\":")?;
        prefix.quoted(request.pointer)?;
        prefix.emit(",\"dialect\":")?;
        prefix.quoted(SCHEMA_DIALECT)?;
        prefix.emit(",\"sources\":[")?;
        for (index, source) in self.sources().enumerate() {
            if index > 0 {
                prefix.emit(",")?;
            }
            prefix.emit("{\"key\":")?;
            prefix.quoted(source.key())?;
            prefix.emit(",\"role\":")?;
            prefix.quoted(source.role().as_str())?;
            prefix.emit(",\"acquisition\":")?;
            prefix.quoted(source.acquisition().as_str())?;
            prefix.emit("}")?;
        }
        prefix.emit("]")?;
        let mut report = prefix.into_bytes();
        let reserve = report
            .len()
            .checked_add(2048)
            .ok_or("schema envelope size overflow")?;
        let room = self
            .limits
            .report_bytes
            .checked_sub(reserve)
            .ok_or("schema report envelope limit")?;
        let pointer = format!("{}{}", request.pointer, request.kind.suffix());
        let model = self.compose_gated(
            EffectiveRequest::new(request.document).pointer(&pointer),
            &mut budget,
            false,
        )?;
        if let Field::Present(json) = model.json()
            && crate::sample::description_root(json)?.0 != request.kind
        {
            return Err("selected definition has the wrong schema kind");
        }
        let mut output = JsonWriter::new(room, &mut budget, NodePolicy::Edit { depth: 64 });
        let attempt = (|| match model.json() {
            Field::Present(json) => {
                output.emit("{\"$schema\":")?;
                output.quoted(SCHEMA_DIALECT)?;
                output.emit(",\"allOf\":[")?;
                description(json, &mut output, 0).and_then(|()| output.emit("]}"))
            }
            _ => Err("selected effective description is unavailable"),
        })();
        let schema = output.into_bytes();
        // Envelope construction has its own pre-reserved finite allowance, so exhausted
        // composition/export can still return a truthful refusal.
        let mut envelope_budget = OperationBudget::new(&self.limits);
        let mut tail = JsonWriter::new(2048, &mut envelope_budget, NodePolicy::Author);
        report.extend_from_slice(if model.relocation_interpretation {
            b",\"copied_reference_relocation\":true"
        } else {
            b",\"copied_reference_relocation\":false"
        });
        let (range, reason) = match attempt {
            Ok(()) => {
                report.extend_from_slice(
                    b",\"fidelity\":\"json-values\",\"reason\":null,\"schema\":",
                );
                let start = report.len();
                report.extend_from_slice(&schema);
                (Some(start..report.len()), None)
            }
            Err(reason) => {
                tail.emit(",\"fidelity\":\"refused\",\"reason\":")?;
                tail.quoted(reason)?;
                tail.emit(",\"schema\":null")?;
                report.extend_from_slice(&tail.into_bytes());
                (None, Some(reason))
            }
        };
        report.push(b'}');
        Ok(SchemaResult {
            report: String::from_utf8(report).expect("JSON UTF-8"),
            schema: range,
            reason,
        })
    }
}

fn member<'a>(
    json: EffectiveJson<'a>,
    key: &str,
    out: &mut JsonWriter<'_>,
) -> Result<Option<EffectiveJson<'a>>, &'static str> {
    // semantic_get preserves unavailable donor and Context-default meaning.
    for child in json.children() {
        if !crate::allowance::charge(
            out.work(),
            key.len() + child.n().key.map_or(0, str::len) + 1,
        ) {
            return Err("schema lookup work limit");
        }
    }
    match json.semantic_get(key) {
        Field::Present(value) => Ok(Some(value)),
        Field::Absent => Ok(None),
        Field::Unavailable(_) => Err("effective quality meaning is unavailable"),
    }
}

fn available(json: EffectiveJson<'_>) -> Result<(), &'static str> {
    if crate::budgeted::failure(json).is_some() {
        Err("effective content is unavailable")
    } else {
        Ok(())
    }
}

fn description(
    json: EffectiveJson<'_>,
    out: &mut JsonWriter<'_>,
    depth: usize,
) -> Result<(), &'static str> {
    out.start(depth)?;
    available(json)?;
    if json.kind() != JsonKind::Map {
        return Err("data description must be a map");
    }
    if json.n().role == Some(crate::role::Role::DirectItem) {
        return Err("Direct items nullability and wider meanings are not exportable");
    }
    match json.alternatives() {
        Field::Present(branches) => {
            if branches.len() == 0 {
                return Err("empty alternatives are not exportable");
            }
            out.emit("{\"anyOf\":[")?;
            for (index, (_, branch)) in branches.enumerate() {
                if index > 0 {
                    out.emit(",")?;
                }
                description(branch.require()?.json(), out, depth + 1)?;
            }
            return out.emit("]}");
        }
        Field::Unavailable(_) => return Err("effective alternatives are unavailable"),
        Field::Absent => {}
    }
    let kind = member(json, "type", out)?
        .map(|value| value.text().require())
        .transpose()?;
    if kind.is_some_and(|kind| {
        !matches!(
            kind,
            "number" | "integer" | "string" | "boolean" | "array" | "object"
        )
    }) {
        return Err("unknown or extended data type is not exportable");
    }
    let nullable = member(json, "nullable", out)?
        .map(|value| value.boolean().require())
        .transpose()?
        .unwrap_or(true);
    out.emit("{")?;
    if let Some(kind) = kind {
        out.emit("\"type\":")?;
        if nullable {
            out.emit("[")?;
        }
        out.quoted(kind)?;
        if nullable {
            out.emit(",\"null\"]")?;
        }
    } else {
        out.emit("\"not\":")?;
        out.emit(if nullable {
            "false"
        } else {
            "{\"type\":\"null\"}"
        })?;
    }
    for value in json.children() {
        available(value)?;
        out.start(depth + 1)?;
        let key = value.member_name().require()?;
        if !json
            .n()
            .role
            .is_some_and(|role| role.supports(key, kind == Some("object")))
        {
            return Err("quality is outside its established data-description role");
        }
        match key {
            "type" | "nullable" => {}
            // Interaction and declaration qualities do not constrain this selected value.
            "sdfRequired" | "readable" | "writable" | "observable" | "sdfData" => {}
            "label" | "description" | "$comment" | "unit" => {
                value.text().require()?;
                field(
                    out,
                    match key {
                        "label" => "title",
                        "unit" => "x-sdf-unit",
                        other => other,
                    },
                )?;
                literal(value, out, depth + 1)?;
            }
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf" => {
                let number = value.number().require()?;
                if key == "multipleOf" && number.sign() != std::cmp::Ordering::Greater {
                    return Err("multipleOf must be positive");
                }
                field(out, key)?;
                literal(value, out, depth + 1)?;
            }
            "minLength" | "maxLength" | "minItems" | "maxItems" => {
                let number = value.number().require()?;
                if !number.is_integer() || number.sign() == std::cmp::Ordering::Less {
                    return Err("count must be a nonnegative integer");
                }
                field(out, key)?;
                literal(value, out, depth + 1)?;
            }
            "const" | "default" => {
                field(out, key)?;
                literal(value, out, depth + 1)?;
            }
            "uniqueItems" => {
                value.boolean().require()?;
                field(out, key)?;
                literal(value, out, depth + 1)?;
            }
            "required" => {
                if kind != Some("object") || value.kind() != JsonKind::Array {
                    return Err("required needs established object structure and a string array");
                }
                if value.children().next().is_none() {
                    return Err("required must be nonempty");
                }
                field(out, key)?;
                out.emit("[")?;
                // JSON Schema requires unique names. SDF duplicate required names are
                // semantically redundant, so retain the first occurrence only.
                let mut any = false;
                for name in value.children() {
                    let text = name.text().require()?;
                    let mut duplicate = false;
                    for previous in value.children() {
                        if previous.node == name.node {
                            break;
                        }
                        if !crate::allowance::charge(
                            out.work(),
                            text.len() + previous.text().require()?.len() + 1,
                        ) {
                            return Err("required-name work limit");
                        }
                        duplicate |= previous.text().require()? == text;
                    }
                    if !duplicate {
                        if any {
                            out.emit(",")?;
                        }
                        out.quoted(text)?;
                        any = true;
                    }
                }
                out.emit("]")?;
            }
            "properties" => {
                if kind != Some("object") || value.kind() != JsonKind::Map {
                    return Err("properties need established object structure");
                }
                field(out, key)?;
                out.emit("{")?;
                for (index, property) in value.children().enumerate() {
                    if index > 0 {
                        out.emit(",")?;
                    }
                    out.quoted(property.member_name().require()?)?;
                    out.emit(":")?;
                    description(property, out, depth + 1)?;
                }
                out.emit("}")?;
            }
            "items" => {
                return Err("Direct items nullability and wider meanings are not exportable");
            }
            "format" => return Err("format denotation and equality are not exportable"),
            "sdfType" => return Err("sdfType abstract meaning is not exportable"),
            "contentFormat" => return Err("content-format meaning is not exportable"),
            "pattern" => return Err("pattern engine equivalence is not established for export"),
            _ => return Err("unsupported quality or extension vocabulary in selected description"),
        }
    }
    out.emit("}")
}

fn field(out: &mut JsonWriter<'_>, key: &str) -> Result<(), &'static str> {
    out.emit(",")?;
    out.quoted(key)?;
    out.emit(":")
}

/// Copy JSON values without serde's machine-number conversion or invented semantics.
fn literal(
    json: EffectiveJson<'_>,
    out: &mut JsonWriter<'_>,
    depth: usize,
) -> Result<(), &'static str> {
    out.start(depth)?;
    available(json)?;
    match json.kind() {
        JsonKind::Map | JsonKind::Array => {
            let map = json.kind() == JsonKind::Map;
            out.emit(if map { "{" } else { "[" })?;
            for (index, child) in json.children().enumerate() {
                if index > 0 {
                    out.emit(",")?;
                }
                if map {
                    out.quoted(child.member_name().require()?)?;
                    out.emit(":")?;
                }
                literal(child, out, depth + 1)?;
            }
            out.emit(if map { "}" } else { "]" })
        }
        JsonKind::String => out.quoted(json.text().require()?),
        JsonKind::Number => out.emit(json.number().require()?.lexeme()),
        JsonKind::Boolean => out.emit(if json.boolean().require()? {
            "true"
        } else {
            "false"
        }),
        JsonKind::Null => out.emit("null"),
    }
}
