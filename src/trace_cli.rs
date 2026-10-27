//! `esdieff trace` checks a Recorded trace against one selected model document. Follows
//! ADR 0042: Recorded traces with independent entries, and ADR 0013: Shared workflow and
//! report guarantees.
use crate::{Format, cli_style::ColorMode, usage};
use esdieff::TraceRequest;
use std::{
    ffi::{OsStr, OsString},
    io::Read,
};

type Error = (u8, String);

struct Options<'a> {
    document: &'a OsStr,
    trace: &'a OsStr,
    format: Format,
    inputs: Vec<&'a OsStr>,
}

pub(crate) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, Error> {
    let Some(options) = parse(args, color)? else {
        return crate::emit_help(*color);
    };
    let document = crate::selected_index(&options.inputs, options.document)?;
    // The trace is read before any model, so a trace that cannot be read or is too large
    // fails the invocation early.
    let trace = read(options.trace)?;
    let catalogue = crate::acquire(&options.inputs, None)?;
    let id = catalogue
        .sources()
        .nth(document)
        .expect("registered document")
        .id();
    let label = crate::display_label(options.trace);
    let request = match &label {
        Some(label) => TraceRequest::new(id, &trace).label(label),
        None => TraceRequest::new(id, &trace).omit_label(),
    };
    let result = catalogue.check_trace(request).map_err(usage)?;
    let rendered = match options.format {
        Format::Json => result.to_json(),
        Format::Human => result.to_human_with_style(crate::human_style(*color)),
        Format::Sarif | Format::Markdown | Format::Jsonl => {
            unreachable!("the format parser refuses it for trace")
        }
    };
    crate::emit_report(&rendered)?;
    Ok(result.disposition().exit_code())
}

/// Read the trace file, or standard input for `-`, up to one byte beyond the largest trace.
fn read(path: &OsStr) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    let cap = TraceRequest::FILE_BYTES as u64 + 1;
    let read = if path == "-" {
        std::io::stdin().lock().take(cap).read_to_end(&mut bytes)
    } else {
        std::fs::File::open(path).and_then(|file| file.take(cap).read_to_end(&mut bytes))
    };
    let label = path.to_string_lossy();
    read.map_err(|error| usage(&format!("cannot read trace {label:?}: {error}")))?;
    if bytes.len() > TraceRequest::FILE_BYTES {
        return Err(usage(&format!("trace {label:?} exceeds 64 MiB")));
    }
    Ok(bytes)
}

fn parse<'a>(args: &'a [OsString], color: &mut ColorMode) -> Result<Option<Options<'a>>, Error> {
    let mut document = None;
    let mut trace = None;
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
            Some(flag @ ("--document" | "--trace" | "--format")) => {
                let value = args
                    .get(index)
                    .ok_or_else(|| usage("option requires a value"))?;
                index += 1;
                metadata = metadata.saturating_add(value.as_encoded_bytes().len());
                match flag {
                    "--document" if document.is_none() => document = Some(value.as_os_str()),
                    "--trace" if trace.is_none() => trace = Some(value.as_os_str()),
                    "--format" => format = crate::format(value, &[Format::Human, Format::Json])?,
                    _ => return Err(usage("duplicate trace option")),
                }
            }
            Some(value) if value.starts_with('-') && value != "-" => {
                return Err(usage(
                    "unknown trace option; use -- before a path beginning with -",
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
        return Err(usage("trace requires --document MODEL"));
    };
    let Some(trace) = trace else {
        return Err(usage("trace requires --trace FILE"));
    };
    if inputs.is_empty() {
        return Err(usage("at least one explicit model input is required"));
    }
    if inputs
        .iter()
        .copied()
        .chain([trace])
        .filter(|input| *input == OsStr::new("-"))
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
        trace,
        format,
        inputs,
    }))
}
