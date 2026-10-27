//! `esdieff generate` builds Generated values for one selected description, or for every
//! affordance description of a selected document. Follows ADR 0041: Generated values settled
//! by Sample-value checking, and ADR 0013: Shared workflow and report guarantees.
use crate::{Format, cli_style::ColorMode, usage};
use esdieff::{GenerationRequest, SampleKind};
use std::ffi::{OsStr, OsString};

type Error = (u8, String);

struct Options<'a> {
    document: &'a OsStr,
    selector: Option<(SampleKind, &'a str)>,
    format: Format,
    inputs: Vec<&'a OsStr>,
}

pub(crate) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, Error> {
    let Some(options) = parse(args, color)? else {
        return crate::emit_help(*color);
    };
    let document = crate::selected_index(&options.inputs, options.document)?;
    let catalogue = crate::acquire(&options.inputs, None)?;
    let id = catalogue
        .sources()
        .nth(document)
        .expect("registered document")
        .id();
    let mut request = GenerationRequest::new(id);
    if let Some((kind, pointer)) = options.selector {
        request = request.select(kind, pointer);
    }
    let result = catalogue.generate(request).map_err(usage)?;
    // Every format fits the report byte limit, since generation charges each value the bytes
    // its largest rendering needs.
    let rendered = match options.format {
        Format::Json => result.to_json(),
        Format::Jsonl => result.to_json_lines(),
        Format::Human => result.to_human_with_style(crate::human_style(*color)),
        Format::Sarif | Format::Markdown => {
            unreachable!("the format parser refuses it for generate")
        }
    };
    // JSON Lines without any entry is empty, with no line ending to add.
    if !rendered.is_empty() || options.format != Format::Jsonl {
        crate::emit_report(&rendered)?;
    }
    Ok(result.disposition().exit_code())
}

fn parse<'a>(args: &'a [OsString], color: &mut ColorMode) -> Result<Option<Options<'a>>, Error> {
    let mut document = None;
    let mut kind = None;
    let mut pointer = None;
    let mut format = Format::Human;
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
            Some(flag @ ("--document" | "--kind" | "--pointer" | "--format")) => {
                let value = args
                    .get(index)
                    .ok_or_else(|| usage("option requires a value"))?;
                index += 1;
                metadata = metadata.saturating_add(value.as_encoded_bytes().len());
                match flag {
                    "--document" if document.is_none() => document = Some(value.as_os_str()),
                    "--kind" if kind.is_none() => {
                        kind = Some(crate::sample_kind(
                            value.to_str().ok_or_else(|| usage("unknown sample kind"))?,
                        )?);
                    }
                    "--pointer" if pointer.is_none() => {
                        pointer = Some(
                            value
                                .to_str()
                                .ok_or_else(|| usage("pointer must be UTF-8"))?,
                        );
                    }
                    "--format" => {
                        format =
                            crate::format(value, &[Format::Human, Format::Json, Format::Jsonl])?
                    }
                    _ => return Err(usage("duplicate generate option")),
                }
            }
            Some(value) if value.starts_with('-') && value != "-" => {
                return Err(usage(
                    "unknown generate option; use -- before a path beginning with -",
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
        return Ok(None);
    }
    let Some(document) = document else {
        return Err(usage("generate requires --document MODEL"));
    };
    let selector = match (kind, pointer) {
        (Some(kind), Some(pointer)) => Some((kind, pointer)),
        (None, None) => None,
        _ => return Err(usage("--kind and --pointer must be given together")),
    };
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
    Ok(Some(Options {
        document,
        selector,
        format,
        inputs,
    }))
}
