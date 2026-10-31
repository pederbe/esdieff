//! Acquisition and output adapter for selected JSON Schema export.
use crate::{cli_style::ColorMode, usage};
use esdieff::SchemaRequest;
use std::ffi::{OsStr, OsString};
type Error = (u8, String);

pub(crate) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, Error> {
    let mut document = None;
    let mut pointer = None;
    let mut kind = None;
    let mut format = None;
    let mut inputs = Vec::new();
    let mut help = false;
    let mut literal = false;
    let mut metadata = 0usize;
    let mut index = 1;
    while let Some(value) = args.get(index) {
        index += 1;
        if !literal && value == "--" {
            literal = true;
            continue;
        }
        if !literal && color.consume(value, args, &mut index)? {
            continue;
        }
        match if literal { None } else { value.to_str() } {
            Some("--help" | "-h") => help = true,
            Some(flag @ ("--document" | "--pointer" | "--kind" | "--format")) => {
                let value = args
                    .get(index)
                    .ok_or_else(|| usage("option requires a value"))?;
                index += 1;
                metadata = metadata.saturating_add(value.as_encoded_bytes().len());
                match flag {
                    "--document" if document.is_none() => document = Some(value.as_os_str()),
                    "--pointer" if pointer.is_none() => {
                        pointer = Some(
                            value
                                .to_str()
                                .ok_or_else(|| usage("pointer must be UTF-8"))?,
                        )
                    }
                    "--kind" if kind.is_none() => {
                        kind = Some(crate::sample_kind(
                            value.to_str().ok_or_else(|| usage("kind must be UTF-8"))?,
                        )?)
                    }
                    "--format" if format.is_none() => {
                        let value = value
                            .to_str()
                            .ok_or_else(|| usage("format must be UTF-8"))?;
                        if !matches!(value, "json" | "schema") {
                            return Err(usage("schema format must be json or schema"));
                        }
                        format = Some(value);
                    }
                    _ => return Err(usage("duplicate schema option")),
                }
            }
            Some(value) if value.starts_with('-') && value != "-" => {
                return Err(usage(
                    "unknown schema option; use -- before a path beginning with -",
                ));
            }
            _ => {
                if inputs.len() >= 256 {
                    return Err(usage("too many source inputs"));
                }
                metadata = metadata.saturating_add(value.as_encoded_bytes().len());
                inputs.push(value.as_os_str());
            }
        }
    }
    if help {
        return crate::emit_help(*color);
    }
    let document = document.ok_or_else(|| usage("schema requires --document MODEL"))?;
    let pointer = pointer.ok_or_else(|| usage("schema requires --pointer POINTER"))?;
    let kind = kind.ok_or_else(|| usage("schema requires --kind KIND"))?;
    if inputs.is_empty() {
        return Err(usage("at least one explicit model input is required"));
    }
    if inputs
        .iter()
        .filter(|input| **input == OsStr::new("-"))
        .count()
        > 1
    {
        return Err(usage("standard input may be supplied only once"));
    }
    if metadata > 1024 * 1024 {
        return Err(usage(
            "input and selector identities exceed metadata budget",
        ));
    }
    let selected = crate::selected_index(&inputs, document)?;
    let catalogue = crate::acquire(&inputs, None)?;
    let id = catalogue
        .sources()
        .nth(selected)
        .expect("registered document")
        .id();
    let result = catalogue
        .export_schema(SchemaRequest::new(id, kind, pointer))
        .map_err(usage)?;
    let rendered = if format == Some("schema") {
        result.schema().ok_or_else(|| {
            (
                3,
                result.reason().unwrap_or("schema unavailable").to_owned(),
            )
        })?
    } else {
        result.to_json()
    };
    crate::emit_report(rendered)?;
    Ok(result.disposition().exit_code())
}
