//! Acquisition and output adapter for the library's selected Markdown reference.
use crate::{cli_style::ColorMode, usage};
use esdieff::ReferenceRequest;
use std::ffi::{OsStr, OsString};
type Error = (u8, String);

pub(crate) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, Error> {
    let mut document = None;
    let mut pointer = None;
    let mut inputs = Vec::new();
    let mut help = false;
    let mut uses = false;
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
            Some("--uses") if !uses => uses = true,
            Some(flag @ ("--document" | "--pointer")) => {
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
                    _ => return Err(usage("duplicate reference option")),
                }
            }
            Some(value) if value.starts_with('-') && value != "-" => {
                return Err(usage(
                    "unknown reference option; use -- before a path beginning with -",
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
    let document = document.ok_or_else(|| usage("reference requires --document MODEL"))?;
    let pointer = pointer.ok_or_else(|| usage("reference requires --pointer POINTER"))?;
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
    let request = ReferenceRequest::new(id, pointer);
    let request = if uses { request.with_uses() } else { request };
    let result = catalogue.reference(request).map_err(usage)?;
    crate::emit_report(result.to_markdown().trim_end_matches('\n'))?;
    Ok(result.disposition().exit_code())
}
