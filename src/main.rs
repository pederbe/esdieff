//! Explicit inputs and shared reports give command-line callers truthful operation outcomes.
//! Follows ADR0010: Domain-oriented library and caller simplicity, and
//! ADR0013: Shared workflow and report guarantees.

use esdieff::{
    Assessment, Catalogue, CheckRequest, DifferenceAcknowledgements, InspectRequest, Limits,
    RecordedRegistryPolicy, SampleKind, SampleRequest, SourceInput, View,
};
use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    io::{Read, Write},
    path::Path,
    process::ExitCode,
};
mod acquire_cli;
mod author_cli;
mod check_plan;
mod cli_style;
mod diff_cli;
mod directory_discovery;
mod document_cli;
mod edit_cli;
mod generate_cli;
mod reference_cli;
mod schema_cli;
mod trace_cli;
use cli_style::{ColorMode, Role};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut color = ColorMode::default();
    match run(&args, &mut color) {
        Ok(code) => ExitCode::from(code),
        Err((code, message)) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "{} {message}",
                color.stderr().span(Role::Failure, "esdieff:")
            );
            ExitCode::from(code)
        }
    }
}
struct Options<'a> {
    plan: Option<&'a OsStr>,
    sample: bool,
    sample_path: Option<&'a OsStr>,
    kind: Option<SampleKind>,
    inspect: bool,
    inputs: Vec<&'a OsStr>,
    directories: Vec<&'a OsStr>,
    recursive: bool,
    exclusions: Vec<&'a OsStr>,
    roots: Vec<&'a OsStr>,
    document: Option<&'a OsStr>,
    pointer: Option<&'a str>,
    view: View,
    assessment: Assessment,
    format: Format,
    strict: bool,
    required: bool,
    registry_policy: Option<RecordedRegistryPolicy>,
}
fn usage(reason: &str) -> (u8, String) {
    (2, reason.to_owned())
}
fn sample_kind(value: &str) -> Result<SampleKind, (u8, String)> {
    SampleKind::from_name(value).ok_or_else(|| usage("unknown sample kind"))
}
fn parse<'a>(
    args: &'a [OsString],
    color: &mut ColorMode,
) -> Result<Option<Options<'a>>, (u8, String)> {
    let command = args.first().and_then(|v| v.to_str()).ok_or_else(|| {
        usage(
            "expected check, inspect, sample, generate, reference, trace, author, edit, diff or acquire and explicit inputs",
        )
    })?;
    if !matches!(command, "check" | "inspect" | "sample") {
        return Err(usage(
            "unknown command; use check, inspect, sample, generate, reference, trace, author, edit, diff or acquire",
        ));
    }
    let mut options = Options {
        plan: None,
        sample: command == "sample",
        sample_path: None,
        kind: None,
        inspect: command == "inspect",
        inputs: Vec::new(),
        directories: Vec::new(),
        recursive: false,
        exclusions: Vec::new(),
        roots: Vec::new(),
        document: None,
        pointer: None,
        view: View::Source,
        assessment: Assessment::Initial,
        format: Format::Human,
        strict: false,
        required: false,
        registry_policy: None,
    };
    let mut index = 1;
    let mut literal = false;
    let mut help = false;
    let mut direct_option = false;
    let mut format_value = None;
    while index < args.len() {
        let value = &args[index];
        index += 1;
        if !literal && value == "--" {
            literal = true;
            continue;
        }
        if !literal && color.consume(value, args, &mut index)? {
            continue;
        }
        if !literal
            && !matches!(
                value.to_str(),
                Some("--plan" | "--format" | "--help" | "-h")
            )
        {
            direct_option = true;
        }
        match if literal { None } else { value.to_str() } {
            Some("--help" | "-h") => help = true,
            Some("--strict-warnings") => options.strict = true,
            Some("--recursive") if command == "check" => options.recursive = true,
            Some("--required-declarations") if options.inspect => options.required = true,
            Some(
                flag @ ("--root"
                | "--document"
                | "--directory"
                | "--exclude"
                | "--pointer"
                | "--view"
                | "--assessment"
                | "--format"
                | "--kind"
                | "--sample"
                | "--plan"
                | "--registry-knowledge"),
            ) => {
                let value = args
                    .get(index)
                    .ok_or_else(|| usage("option requires a value"))?;
                index += 1;
                match flag {
                    "--plan" if command == "check" && options.plan.is_none() => {
                        options.plan = Some(value);
                    }
                    "--root" if !options.inspect && !options.sample => {
                        if options.roots.len() >= 256 {
                            return Err(usage("too many selected roots"));
                        }
                        options.roots.push(value);
                    }
                    "--directory" if command == "check" => {
                        if options.directories.len() >= 256 {
                            return Err(usage("too many directory requests"));
                        }
                        options.directories.push(value);
                    }
                    "--exclude" if command == "check" => {
                        if options.exclusions.len() >= 256 {
                            return Err(usage("too many literal exclusions"));
                        }
                        options.exclusions.push(value);
                    }
                    "--document" if options.inspect || options.sample => {
                        if options.document.is_some() {
                            return Err(usage("one inspection document required"));
                        }
                        options.document = Some(value);
                    }
                    "--pointer" if options.inspect || options.sample => {
                        if options.pointer.is_some() {
                            return Err(usage("one inspection pointer required"));
                        }
                        options.pointer = Some(
                            value
                                .to_str()
                                .ok_or_else(|| usage("pointer must be UTF-8"))?,
                        );
                    }
                    "--view" if options.inspect => {
                        options.view = match value.to_str() {
                            Some("source") => View::Source,
                            Some("effective") => View::Effective,
                            _ => return Err(usage("view must be source or effective")),
                        }
                    }
                    "--kind" if options.sample && options.kind.is_none() => {
                        options.kind = Some(sample_kind(
                            value.to_str().ok_or_else(|| usage("unknown sample kind"))?,
                        )?);
                    }
                    "--sample" if options.sample && options.sample_path.is_none() => {
                        options.sample_path = Some(value)
                    }
                    "--assessment" if !options.inspect && !options.sample => {
                        options.assessment = assessment(value.to_str())?;
                    }
                    "--registry-knowledge" if options.registry_policy.is_none() => {
                        options.registry_policy = Some(match value.to_str() {
                            Some("recorded") => RecordedRegistryPolicy::UseRecorded,
                            Some("withheld") => RecordedRegistryPolicy::WithholdRecorded,
                            _ => {
                                return Err(usage(
                                    "registry knowledge must be recorded or withheld",
                                ));
                            }
                        });
                    }
                    "--format" => format_value = Some(value.as_os_str()),
                    _ => return Err(usage("option does not apply to this operation")),
                }
            }
            Some(value) if value.starts_with('-') && value != "-" => {
                return Err(usage(
                    "unknown option; use -- before a path beginning with -",
                ));
            }
            _ => {
                if options.inputs.len() >= 256 {
                    return Err(usage("too many source inputs"));
                }
                options.inputs.push(value);
            }
        }
    }
    if help {
        return Ok(None);
    }
    if let Some(value) = format_value {
        options.format = if command == "check" && options.plan.is_none() {
            format(value, &[Format::Human, Format::Json, Format::Sarif])?
        } else {
            format(value, &[Format::Human, Format::Json])?
        };
    }
    if options.plan.is_some() {
        if direct_option || !options.inputs.is_empty() {
            return Err(usage(
                "plan mode is incompatible with direct inputs and policy options",
            ));
        }
        return Ok(Some(options));
    }
    if options.inputs.is_empty() && options.directories.is_empty() {
        return Err(usage(
            "at least one explicit input or directory is required",
        ));
    }
    if (options.recursive || !options.exclusions.is_empty()) && options.directories.is_empty() {
        return Err(usage(
            "recursive selection and exclusions require a directory input",
        ));
    }
    if options.registry_policy.is_some()
        && (options.inspect
            || options.sample
            || !matches!(
                options.assessment,
                Assessment::ExpandedUnits
                    | Assessment::ExpandedContentFormat
                    | Assessment::FullBaseline
            ))
    {
        return Err(usage(
            "registry knowledge policy requires a registry assessment",
        ));
    }
    if options
        .inputs
        .iter()
        .copied()
        .chain(options.sample_path)
        .filter(|v| *v == OsStr::new("-"))
        .count()
        > 1
    {
        return Err(usage("standard input may be supplied only once"));
    }
    if options.inspect && (options.document.is_none() || options.pointer.is_none()) {
        return Err(usage("inspection requires --document and --pointer"));
    }
    if options.sample
        && (options.document.is_none()
            || options.pointer.is_none()
            || options.kind.is_none()
            || options.sample_path.is_none())
    {
        return Err(usage(
            "sample requires --document, --kind, --pointer and --sample",
        ));
    }
    let metadata = options
        .inputs
        .iter()
        .chain(options.roots.iter())
        .chain(options.directories.iter())
        .chain(options.exclusions.iter())
        .copied()
        .chain(options.document)
        .chain(options.sample_path)
        .map(|s| s.as_encoded_bytes().len())
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| usage("input and selector metadata overflow"))?;
    if metadata > 1024 * 1024 {
        return Err(usage(
            "input and selector identities exceed metadata budget",
        ));
    }
    Ok(Some(options))
}
/// A requested report format. Each operation states which formats it accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Format {
    Human,
    Json,
    Sarif,
    Markdown,
    /// JSON Lines Trace entries, which only `generate` writes.
    Jsonl,
}
impl Format {
    const ALL: [Self; 5] = [
        Self::Human,
        Self::Json,
        Self::Sarif,
        Self::Markdown,
        Self::Jsonl,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Sarif => "sarif",
            Self::Markdown => "markdown",
            Self::Jsonl => "jsonl",
        }
    }
}
/// The `--format` value among the formats an operation `accepts`, or a usage error naming
/// them.
pub(crate) fn format(value: &OsStr, accepts: &[Format]) -> Result<Format, (u8, String)> {
    if let Some(format) = Format::ALL
        .into_iter()
        .find(|format| value.to_str() == Some(format.name()) && accepts.contains(format))
    {
        return Ok(format);
    }
    let names: Vec<_> = accepts.iter().map(|format| format.name()).collect();
    let (last, rest) = names.split_last().expect("an accepted format");
    Err(usage(&format!(
        "format must be {} or {last} for this command",
        rest.join(", ")
    )))
}
/// The directory relative paths resolve from, so SARIF artifact URIs can be relative to it.
pub(crate) fn invocation_directory() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_default()
}
fn assessment(value: Option<&str>) -> Result<Assessment, (u8, String)> {
    match value {
        Some("initial-v1") => Ok(Assessment::Initial),
        Some("expanded-modified-v1") => Ok(Assessment::ExpandedModified),
        Some("expanded-units-v1") => Ok(Assessment::ExpandedUnits),
        Some("expanded-content-format-v1") => Ok(Assessment::ExpandedContentFormat),
        Some("expanded-pattern-syntax-v1") => Ok(Assessment::ExpandedPatternSyntax),
        Some("expanded-numeric-v1") => Ok(Assessment::ExpandedNumeric),
        Some("expanded-consistency-v1") => Ok(Assessment::ExpandedConsistency),
        Some("full-baseline") => Ok(Assessment::FullBaseline),
        _ => Err(usage("unknown assessment")),
    }
}
fn selected_index(inputs: &[&OsStr], name: &OsStr) -> Result<usize, (u8, String)> {
    let mut matches = inputs.iter().enumerate().filter(|(_, v)| **v == name);
    let index = matches
        .next()
        .map(|(index, _)| index)
        .ok_or_else(|| usage("selected root or document is not a supplied input"))?;
    if matches.next().is_some() {
        return Err(usage(
            "selected path identifies more than one supplied attempt",
        ));
    }
    Ok(index)
}
fn emit_help(color: ColorMode) -> Result<u8, (u8, String)> {
    std::io::stdout()
        .lock()
        .write_all(cli_style::help(color.stdout()).as_bytes())
        .map_err(|error| (4, format!("cannot emit requested help: {error}")))?;
    Ok(0)
}
fn display_label(path: &OsStr) -> Option<Cow<'_, str>> {
    if path.as_encoded_bytes().len() > 512 {
        return None;
    }
    let label = path.to_string_lossy();
    (label.len() <= 512).then_some(label)
}
/// Read and decode an acknowledgement file under the default limits the revisions use, with
/// the number of bytes read. Direct comparison and Check plan diff cases share it.
fn read_acknowledgements(path: &Path) -> Result<(DifferenceAcknowledgements, usize), String> {
    let mut bytes = Vec::new();
    // One byte beyond the decoder's cap shows that the file is larger.
    let cap = DifferenceAcknowledgements::FILE_BYTES as u64 + 1;
    std::fs::File::open(path)
        .and_then(|file| file.take(cap).read_to_end(&mut bytes))
        .map_err(|error| error.to_string())?;
    let acknowledgements = DifferenceAcknowledgements::from_json(&bytes, &Limits::default())?;
    Ok((acknowledgements, bytes.len()))
}
fn run(args: &[OsString], color: &mut ColorMode) -> Result<u8, (u8, String)> {
    if args.len() == 1 && args[0] == "--version" {
        writeln!(
            std::io::stdout().lock(),
            "esdieff {}",
            env!("CARGO_PKG_VERSION")
        )
        .map_err(|error| (4, format!("cannot emit requested version: {error}")))?;
        return Ok(0);
    }
    let mut index = 0;
    let mut help = false;
    while let Some(value) = args.get(index) {
        index += 1;
        if color.consume(value, args, &mut index)? {
            continue;
        }
        if value == "--help" || value == "-h" {
            help = true;
            continue;
        }
        if help {
            break;
        }
        index -= 1;
        break;
    }
    if help {
        return emit_help(*color);
    }
    let args = &args[index..];
    if args.first().is_some_and(|arg| arg == "acquire") {
        return acquire_cli::run(args, color);
    }
    if args.first().is_some_and(|arg| arg == "author") {
        return author_cli::run(args, color);
    }
    if args.first().is_some_and(|arg| arg == "edit") {
        return edit_cli::run(args, color);
    }
    if args.first().is_some_and(|arg| arg == "diff") {
        return diff_cli::run(args, color);
    }
    if args.first().is_some_and(|arg| arg == "reference") {
        return reference_cli::run(args, color);
    }
    if args.first().is_some_and(|arg| arg == "schema") {
        return schema_cli::run(args, color);
    }
    if args.first().is_some_and(|arg| arg == "generate") {
        return generate_cli::run(args, color);
    }
    if args.first().is_some_and(|arg| arg == "trace") {
        return trace_cli::run(args, color);
    }
    let Some(options) = parse(args, color)? else {
        return emit_help(*color);
    };
    if let Some(path) = options.plan {
        return check_plan::run(Path::new(path), options.format == Format::Json, *color);
    }
    let discovered = directory_discovery::prepare(
        &options.inputs,
        &options.directories,
        &options.roots,
        options.recursive,
        &options.exclusions,
    )?;
    let paths: Vec<_> = options
        .inputs
        .iter()
        .copied()
        .chain(discovered.iter().map(|path| path.as_os_str()))
        .collect();
    let roots = options
        .roots
        .iter()
        .map(|name| selected_index(&paths, name))
        .collect::<Result<Vec<_>, _>>()?;
    let document = options
        .document
        .map(|name| selected_index(&paths, name))
        .transpose()?;
    let sample_label = options.sample_path.and_then(display_label);
    let sample = options.sample_path.map(|path| {
        let input = if path == OsStr::new("-") {
            SourceInput::stdin("sample1")
        } else {
            SourceInput::file("sample1", Path::new(path))
        }
        .sample();
        if let Some(label) = &sample_label {
            input.label(label)
        } else {
            input.omit_label()
        }
    });
    let catalogue = acquire(&paths, sample)?;
    let ids: Vec<_> = catalogue.sources().map(|s| s.id()).collect();
    let report = if options.sample {
        catalogue
            .check_sample(
                SampleRequest::new(
                    ids[document.expect("validated document")],
                    options.kind.expect("validated kind"),
                    options.pointer.expect("validated pointer"),
                )
                .sample(*ids.last().expect("registered sample"))
                .strict_warnings(options.strict),
            )
            .map_err(usage)?
    } else if options.inspect {
        catalogue
            .inspect(
                InspectRequest::new(
                    ids[document.expect("validated document")],
                    options.pointer.expect("validated pointer"),
                )
                .view(options.view)
                .required_declarations(options.required)
                .strict_warnings(options.strict),
            )
            .map_err(usage)?
    } else {
        let mut request = CheckRequest::new()
            .assessment(options.assessment)
            .strict_warnings(options.strict);
        if let Some(policy) = options.registry_policy {
            request = request.recorded_registry_policy(policy);
        }
        for index in roots {
            request = request.root(ids[index]);
        }
        catalogue.check(request).map_err(usage)?
    };
    let code = report.outcome().disposition().exit_code();
    let rendered = match options.format {
        Format::Json => report.to_json(),
        Format::Sarif => report.to_sarif(&invocation_directory()),
        Format::Human => report.to_human_with_style(human_style(*color)),
        Format::Markdown | Format::Jsonl => {
            unreachable!("the format parser refuses it for check, inspect and sample")
        }
    };
    emit_report(&rendered)?;
    Ok(code)
}
/// Acquire `paths` as model sources keyed `input1`, `input2` and onwards, with `-` read from
/// standard input and each path as its label, followed by the `extra` inputs.
fn acquire<'a>(
    paths: &[&OsStr],
    extra: impl IntoIterator<Item = SourceInput<'a>>,
) -> Result<Catalogue, (u8, String)> {
    let keys: Vec<_> = (0..paths.len())
        .map(|index| format!("input{}", index + 1))
        .collect();
    let labels: Vec<_> = paths.iter().map(|path| display_label(path)).collect();
    let mut inputs: Vec<_> = paths
        .iter()
        .zip(&keys)
        .zip(&labels)
        .map(|((path, key), label)| {
            let input = if *path == OsStr::new("-") {
                SourceInput::stdin(key)
            } else {
                SourceInput::file(key, Path::new(path))
            };
            match label {
                Some(label) => input.label(label),
                None => input.omit_label(),
            }
        })
        .collect();
    for input in extra {
        inputs.push(input);
    }
    Catalogue::take_inputs(inputs).map_err(usage)
}
fn human_style(color: ColorMode) -> esdieff::HumanStyle {
    if color.stdout().enabled() {
        esdieff::HumanStyle::Ansi
    } else {
        esdieff::HumanStyle::Plain
    }
}
/// Writes the one requested report to stdout with a trailing newline.
fn emit_report(rendered: &str) -> Result<(), (u8, String)> {
    let mut output = std::io::stdout().lock();
    output
        .write_all(rendered.as_bytes())
        .and_then(|_| {
            if rendered.ends_with('\n') {
                Ok(())
            } else {
                output.write_all(b"\n")
            }
        })
        .map_err(|error| (4, format!("cannot emit requested report: {error}")))
}
