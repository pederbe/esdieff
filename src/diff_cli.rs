//! `esdieff diff` compares two Model revisions, each a directory or a single file. Follows
//! ADR 0039: Client-side revision comparison, ADR 0040: Acknowledged revision differences, and
//! ADR 0004: Supplied catalogues and checking scope.
use crate::{Format, cli_style::ColorMode, directory_discovery, usage};
use esdieff::{Catalogue, ComparisonRequest, DifferenceAcknowledgements, Perspective, SourceInput};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Component, Path, PathBuf},
};

type Error = (u8, String);

/// The report formats `diff` writes.
const ACCEPTED: &[Format] = &[Format::Human, Format::Json, Format::Sarif, Format::Markdown];

/// The key both documents share in the two-file form, so they correspond directly.
const FILE_KEY: &str = "document";

struct Options<'a> {
    perspective: Perspective,
    /// The earlier revision, given as `--old`.
    earlier: &'a OsStr,
    /// The later revision, given as `--new`.
    later: &'a OsStr,
    recursive: bool,
    exclusions: Vec<&'a OsStr>,
    /// The Difference acknowledgements file, given as `--acknowledged`.
    acknowledged: Option<&'a OsStr>,
    acknowledgements_output: Option<&'a OsStr>,
    replace_acknowledgements: bool,
    format: Format,
}

pub(crate) fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, Error> {
    let Some(options) = parse(args, color)? else {
        return crate::emit_help(*color);
    };
    // The acknowledgements are decoded, and both revisions finish preparation, before either
    // revision's content is read.
    let acknowledgements = options.acknowledged.map(acknowledgements).transpose()?;
    let [earlier, later] = revisions(&options)?;
    let output = options
        .acknowledgements_output
        .map(|path| {
            crate::document_cli::FileOutput::prepare(
                path,
                options.replace_acknowledgements,
                earlier
                    .iter()
                    .chain(&later)
                    .map(|(_, path)| path.as_path())
                    .chain(options.acknowledged.map(Path::new)),
            )
        })
        .transpose()?;
    let earlier = catalogue(&earlier)?;
    let later = catalogue(&later)?;
    let mut request = ComparisonRequest::new(&earlier, &later).perspective(options.perspective);
    if let Some(acknowledgements) = &acknowledgements {
        request = request.acknowledgements(acknowledgements);
    }
    let comparison = request.compare().map_err(usage)?;
    if let Some(output) = output {
        // Project the report's ready-made entries, exactly as the documented jq command does.
        let report: serde_json::Value =
            serde_json::from_str(&comparison.to_json()).expect("comparison report is JSON");
        let entries: Vec<_> = report["result"]["differences"]
            .as_array()
            .expect("comparison differences")
            .iter()
            .filter_map(|difference| difference.get("acknowledgement"))
            .collect();
        let mut bytes = serde_json::to_vec(&serde_json::json!({
            "schema": "esdieff.acknowledged-differences", "version": "1", "differences": entries,
        }))
        .expect("acknowledgement JSON");
        bytes.push(b'\n');
        // Refuse an export the reader cannot accept, before committing a destination.
        DifferenceAcknowledgements::from_json(&bytes)
            .map_err(|reason| usage(&format!("cannot export acknowledgements: {reason}")))?;
        output.emit(&bytes)?;
    }
    let rendered = match options.format {
        Format::Json => comparison.to_json(),
        Format::Sarif => comparison.to_sarif(&crate::invocation_directory()),
        Format::Markdown => comparison.to_markdown(),
        Format::Human => comparison.to_human_with_style(crate::human_style(*color)),
        Format::Jsonl => unreachable!("the format parser refuses it for diff"),
    };
    crate::emit_report(&rendered)?;
    Ok(comparison.disposition().exit_code())
}

fn parse<'a>(args: &'a [OsString], color: &mut ColorMode) -> Result<Option<Options<'a>>, Error> {
    let mut earlier = None;
    let mut perspective = None;
    let mut later = None;
    let mut recursive = false;
    let mut exclusions = Vec::new();
    let mut acknowledged = None;
    let mut acknowledgements_output = None;
    let mut replace_acknowledgements = false;
    let mut format = Format::Human;
    let mut help = false;
    let mut metadata = 0usize;
    let mut index = 1;
    while let Some(flag) = args.get(index) {
        index += 1;
        if color.consume(flag, args, &mut index)? {
            continue;
        }
        match flag.to_str() {
            Some("--help" | "-h") => help = true,
            Some("--recursive") => recursive = true,
            Some("--replace-acknowledgements") if !replace_acknowledgements => {
                replace_acknowledgements = true;
            }
            Some(
                flag @ ("--old"
                | "--new"
                | "--exclude"
                | "--acknowledged"
                | "--format"
                | "--perspective"
                | "--acknowledgements-output"),
            ) => {
                let value = args
                    .get(index)
                    .ok_or_else(|| usage("option requires a value"))?;
                index += 1;
                metadata = metadata
                    .checked_add(value.as_encoded_bytes().len())
                    .ok_or_else(|| usage("input metadata overflow"))?;
                match flag {
                    "--perspective" if perspective.is_none() => {
                        perspective = Some(parse_perspective(
                            value
                                .to_str()
                                .ok_or_else(|| usage("invalid comparison perspective"))?,
                        )?);
                    }
                    "--old" if earlier.is_none() => earlier = Some(value.as_os_str()),
                    "--new" if later.is_none() => later = Some(value.as_os_str()),
                    "--acknowledged" if acknowledged.is_none() => {
                        acknowledged = Some(value.as_os_str())
                    }
                    "--acknowledgements-output" if acknowledgements_output.is_none() => {
                        acknowledgements_output = Some(value.as_os_str())
                    }
                    "--exclude" => {
                        if exclusions.len() >= 256 {
                            return Err(usage("too many literal exclusions"));
                        }
                        exclusions.push(value.as_os_str());
                    }
                    "--format" => format = crate::format(value, ACCEPTED)?,
                    _ => return Err(usage("duplicate diff option")),
                }
            }
            _ => return Err(usage("unknown diff option")),
        }
        if metadata > 1024 * 1024 {
            return Err(usage("diff CLI metadata exceeds whole-request budget"));
        }
    }
    if help {
        return Ok(None);
    }
    let (Some(earlier), Some(later)) = (earlier, later) else {
        return Err(usage("diff requires --old PATH and --new PATH"));
    };
    if earlier == "-" || later == "-" {
        return Err(usage(
            "diff does not accept standard input; supply two files or two directories",
        ));
    }
    if acknowledged.is_some_and(|path| path == "-") {
        return Err(usage(
            "diff does not read acknowledgements from standard input; supply a file",
        ));
    }
    if replace_acknowledgements && acknowledgements_output.is_none() {
        return Err(usage(
            "--replace-acknowledgements requires --acknowledgements-output",
        ));
    }
    Ok(Some(Options {
        perspective: perspective.unwrap_or_default(),
        earlier,
        later,
        recursive,
        exclusions,
        acknowledged,
        acknowledgements_output,
        replace_acknowledgements,
        format,
    }))
}

pub(crate) fn parse_perspective(value: &str) -> Result<Perspective, Error> {
    match value {
        "client" => Ok(Perspective::Client),
        "implementer" => Ok(Perspective::Implementer),
        _ => Err(usage("perspective must be client or implementer")),
    }
}

/// Read and decode the acknowledgement file. Every failure is an invocation error.
fn acknowledgements(path: &OsStr) -> Result<DifferenceAcknowledgements, Error> {
    let label = crate::display_label(path).unwrap_or("[path label omitted]".into());
    crate::read_acknowledgements(Path::new(path))
        .map(|(acknowledgements, _)| acknowledgements)
        .map_err(|reason| {
            (
                2,
                format!("cannot read acknowledgements {label:?}: {reason}"),
            )
        })
}

/// Each revision's documents as source keys and joined paths.
fn revisions(options: &Options<'_>) -> Result<[Vec<(String, PathBuf)>; 2], Error> {
    let directory = |path: &OsStr| -> Result<bool, Error> {
        fs::metadata(path)
            .map(|metadata| metadata.is_dir())
            .map_err(|error| failure(path, error))
    };
    match (directory(options.earlier)?, directory(options.later)?) {
        (true, true) => Ok([
            discover(options.earlier, options)?,
            discover(options.later, options)?,
        ]),
        (false, false) => {
            if options.recursive || !options.exclusions.is_empty() {
                return Err(usage(
                    "recursive selection and exclusions require a directory input",
                ));
            }
            Ok([options.earlier, options.later]
                .map(|path| vec![(FILE_KEY.to_owned(), path.into())]))
        }
        _ => Err(usage(
            "--old and --new must both be directories or both be files",
        )),
    }
}

/// One revision's directory, keyed by each document's path relative to it.
fn discover(directory: &OsStr, options: &Options<'_>) -> Result<Vec<(String, PathBuf)>, Error> {
    directory_discovery::discover(
        &[],
        &[directory],
        &[],
        options.recursive,
        &options.exclusions,
    )?
    .into_iter()
    .map(|(relative, joined)| Ok((key(&relative, &joined)?, joined)))
    .collect()
}

/// Joins the relative path's components with `/`, so keys read alike on every platform.
fn key(relative: &Path, joined: &Path) -> Result<String, Error> {
    let components: Option<Vec<_>> = relative
        .components()
        .map(|component| match component {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    match components.map(|components| components.join("/")) {
        Some(key) if key.len() <= 256 => Ok(key),
        Some(_) => Err(failure(
            joined.as_os_str(),
            "relative path exceeds the 256-byte source key",
        )),
        None => Err(failure(
            joined.as_os_str(),
            "relative path is not UTF-8, so it cannot pair documents",
        )),
    }
}

fn catalogue(documents: &[(String, PathBuf)]) -> Result<Catalogue, Error> {
    let labels: Vec<_> = documents
        .iter()
        .map(|(_, path)| crate::display_label(path.as_os_str()))
        .collect();
    let inputs = documents
        .iter()
        .zip(&labels)
        .map(|((key, path), label)| {
            let input = SourceInput::file(key, path);
            match label {
                Some(label) => input.label(label),
                None => input.omit_label(),
            }
        })
        .collect();
    Catalogue::take_inputs(inputs).map_err(usage)
}

fn failure(path: &OsStr, reason: impl std::fmt::Display) -> Error {
    let label = crate::display_label(path).unwrap_or("[path label omitted]".into());
    (2, format!("cannot prepare revision {label:?}: {reason}"))
}
