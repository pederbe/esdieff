//! Task-focused help shares presentation while each command owns its examples and options.
use crate::cli_style::{Role, Style};
use std::fmt::Write;

struct CommandHelp {
    name: &'static str,
    summary: &'static str,
    usage: &'static [&'static str],
    examples: &'static [(&'static str, &'static str)],
    options: &'static [(&'static str, &'static str)],
    notes: &'static str,
}

const MODEL_INPUTS: &str = "--document selects one of the supplied model files. The trailing paths supply\nthat model and its dependencies. Use exactly the same path spelling in both places.";
const KINDS: &str = "property, data, action_input, action_output, event_output or context.";
const DIRECTORY_NOTES: &str = "Select regular .sdf.json files directly in each directory. --recursive descends\nthrough all supplied directories, including hidden entries. Exclusions are literal,\ncase-sensitive paths relative to each directory. No globbing, . or .. components.\nDescendant links and special files are skipped. Explicit directory links are allowed.\nEmpty selections, overlapping files and setup failures stop discovery before checking.";

const DOCUMENT_OPTIONS: &[(&str, &str)] = &[
    (
        "--plan PLAN",
        "Read the plan from a file, or - for standard input.",
    ),
    (
        "--source KEY PATH",
        "Supply one source under a unique key. Plan selectors use these keys.\nRepeat the complete --source KEY PATH option for every file.",
    ),
    (
        "--document-output DEST",
        "Write the resulting document to a new file, or - for standard output.",
    ),
    (
        "--check initial-v1|full-baseline",
        "Also check the resulting model (default: no model check).",
    ),
    (
        "--report-output DEST",
        "Write the report to a file (default: standard output).",
    ),
    (
        "--format human|json",
        "Choose the report format (default: human).",
    ),
    ("--strict-warnings", "Fail on warnings as well as errors."),
    (
        "--replace-document",
        "Allow replacement of the document output file.",
    ),
    (
        "--replace-report",
        "Allow replacement of the report output file.",
    ),
];

const COMMANDS: &[CommandHelp] = &[
    CommandHelp {
        name: "check",
        summary: "Check model files or run a saved check plan.",
        usage: &[
            "check [OPTIONS] [INPUT...]",
            "check --plan PLAN [--format human|json]",
        ],
        examples: &[
            ("Check a model file", "check model.sdf.json"),
            (
                "Check a model with shared definitions",
                "check --root device.sdf.json device.sdf.json types.sdf.json",
            ),
        ],
        options: &[
            (
                "--assessment PROFILE",
                "Choose checks (default: initial-v1). Profiles are initial-v1,\nexpanded-numeric-v1, expanded-consistency-v1 and full-baseline.",
            ),
            (
                "--root INPUT",
                "Check this model and its needed dependencies. Repeat for several roots.\nWithout --root, check every supplied model.",
            ),
            (
                "--format human|json|sarif",
                "Choose the report format (default: human). Plans accept human or json.",
            ),
            ("--strict-warnings", "Fail on warnings as well as errors."),
            (
                "--plan PLAN",
                "Run the cases in a saved plan instead of direct inputs and check options.",
            ),
            (
                "--directory DIR",
                "Select model files from a directory. Repeat for several directories.",
            ),
            (
                "--recursive",
                "Include subdirectories of every --directory.",
            ),
            (
                "--exclude RELATIVE_PATH",
                "Exclude a file or subtree from every directory. Repeat for several paths.",
            ),
        ],
        notes: "For a direct check, supply at least one INPUT or --directory.\nUse - for one model on standard input. The files after --root supply the root\nmodel and its dependencies. Roots select exact joined paths relative to the\ninvocation directory.",
    },
    CommandHelp {
        name: "inspect",
        summary: "Show model JSON as written or after shared definitions are composed.",
        usage: &["inspect --document INPUT --pointer POINTER [OPTIONS] INPUT..."],
        examples: &[
            (
                "Inspect an Object named Sensor",
                "inspect --document model.sdf.json --pointer /sdfObject/Sensor model.sdf.json",
            ),
            (
                "Show the same Object after composition",
                "inspect --view effective --document model.sdf.json --pointer /sdfObject/Sensor model.sdf.json",
            ),
        ],
        options: &[
            ("--document INPUT", "Select the supplied model to inspect."),
            (
                "--pointer POINTER",
                "Select a JSON Pointer, such as /sdfObject/Sensor. Use an empty string\nfor the document root.",
            ),
            (
                "--view source|effective",
                "Choose as-written or composed JSON (optional, default: source).",
            ),
            (
                "--format human|json",
                "Choose the report format (default: human).",
            ),
            (
                "--required-declarations",
                "Include the declaration relationships selected by sdfRequired.",
            ),
            ("--strict-warnings", "Fail on warnings as well as errors."),
        ],
        notes: "Inspection shows the selected view. Use check to assess the model.\nUse - for one model on standard input.",
    },
    CommandHelp {
        name: "sample",
        summary: "Check one JSON value against one model description.",
        usage: &[
            "sample --document MODEL --kind KIND --pointer POINTER --sample SAMPLE [OPTIONS] INPUT...",
        ],
        examples: &[(
            "Check a value for a Property named temperature",
            "sample --document model.sdf.json --kind property --pointer /sdfProperty/temperature --sample value.json model.sdf.json",
        )],
        options: &[
            (
                "--document MODEL",
                "Select the supplied model that describes the value.",
            ),
            ("--kind KIND", KINDS),
            (
                "--pointer POINTER",
                "Select the description, such as /sdfProperty/temperature.",
            ),
            (
                "--sample SAMPLE",
                "Read one JSON value from this file, or - for standard input.",
            ),
            (
                "--format human|json",
                "Choose the report format (default: human).",
            ),
            ("--strict-warnings", "Fail on warnings as well as errors."),
        ],
        notes: "The result says whether the value is admitted, rejected or undetermined.\nUse - only once across model and sample inputs.",
    },
    CommandHelp {
        name: "generate",
        summary: "Generate test values from model constraints.",
        usage: &["generate --document MODEL [--kind KIND --pointer POINTER] [OPTIONS] INPUT..."],
        examples: &[
            (
                "Generate values for a model",
                "generate --document model.sdf.json model.sdf.json",
            ),
            (
                "Generate values for one data definition",
                "generate --document model.sdf.json --kind data --pointer /sdfData/Count model.sdf.json",
            ),
        ],
        options: &[
            (
                "--document MODEL",
                "Select the supplied model to generate values from.",
            ),
            (
                "--format human|json|jsonl",
                "Choose a report or JSON Lines test vectors (default: human).\nThe jsonl output can be checked with trace.",
            ),
            ("--kind KIND", KINDS),
            (
                "--pointer POINTER",
                "Select one description, such as /sdfData/Count.\nGive --kind and --pointer together, or omit both to cover the model's\nProperties, Action inputs and outputs, and Event outputs.",
            ),
        ],
        notes: "Select Context metadata explicitly with --kind context and its pointer.\nUse - for one model on standard input.",
    },
    CommandHelp {
        name: "trace",
        summary: "Check recorded JSON Lines values against a model.",
        usage: &["trace --document MODEL --trace FILE [--format human|json] INPUT..."],
        examples: &[(
            "Check a file of recorded values",
            "trace --document model.sdf.json --trace readings.jsonl model.sdf.json",
        )],
        options: &[
            (
                "--document MODEL",
                "Select the supplied model describing the recorded values.",
            ),
            (
                "--trace FILE",
                "Read JSON Lines entries, each with kind, pointer and value fields.\nGenerated jsonl test vectors are also accepted.",
            ),
            (
                "--format human|json",
                "Choose the report format (default: human).",
            ),
        ],
        notes: "Each entry is checked independently. Use - only once across model and trace\ninputs to read standard input.",
    },
    CommandHelp {
        name: "diff",
        summary: "Compare two model revisions and identify compatibility changes.",
        usage: &["diff --old PATH --new PATH [OPTIONS]"],
        examples: &[
            (
                "Compare two model files",
                "diff --old old.sdf.json --new new.sdf.json",
            ),
            (
                "Write a Markdown changelog to standard output",
                "diff --old old.sdf.json --new new.sdf.json --format markdown",
            ),
        ],
        options: &[
            (
                "--old PATH",
                "Read the earlier revision. Supply two files or two directories.",
            ),
            (
                "--new PATH",
                "Read the later revision. Directories pair files by relative path.",
            ),
            (
                "--format human|json|sarif|markdown",
                "Choose the report format (default: human).",
            ),
            (
                "--perspective client|implementer",
                "Classify changes for clients or implementations (default: client).",
            ),
            (
                "--acknowledged FILE",
                "Read previously reviewed differences from this file.",
            ),
            (
                "--acknowledgements-output FILE",
                "Write eligible acknowledgement entries to a file for review.",
            ),
            (
                "--replace-acknowledgements",
                "Allow replacement of the acknowledgement output file.",
            ),
            (
                "--recursive",
                "Include subdirectories of both revision directories.",
            ),
            (
                "--exclude RELATIVE_PATH",
                "Exclude a literal file or subtree from both directories (repeatable).",
            ),
        ],
        notes: "--recursive and --exclude require directories. Standard input is not accepted.",
    },
    CommandHelp {
        name: "reference",
        summary: "Write a Markdown reference for a model definition.",
        usage: &["reference [--uses] --document MODEL --pointer POINTER INPUT..."],
        examples: &[(
            "Describe an Object named Sensor",
            "reference --document model.sdf.json --pointer /sdfObject/Sensor model.sdf.json",
        )],
        options: &[
            (
                "--document MODEL",
                "Select the supplied model containing the definition.",
            ),
            (
                "--pointer POINTER",
                "Select an sdfThing or sdfObject, such as /sdfObject/Sensor.",
            ),
            (
                "--uses",
                "Include uses across the supplied models. Also allow named data,\nProperty, Action, Event and Context definitions.",
            ),
        ],
        notes: "The reference describes the effective definition and writes Markdown to\nstandard output. It does not check the model. Use - for one model on standard input.",
    },
    CommandHelp {
        name: "schema",
        summary: "Export JSON Schema for one model description.",
        usage: &[
            "schema --document MODEL --kind KIND --pointer POINTER [--format json|schema] INPUT...",
        ],
        examples: &[(
            "Export a standalone schema for a temperature Property",
            "schema --document model.sdf.json --kind property --pointer /sdfProperty/temperature --format schema model.sdf.json",
        )],
        options: &[
            (
                "--format json|schema",
                "Write a JSON report (default: json) or only the standalone schema.",
            ),
            (
                "--document MODEL",
                "Select the supplied model containing the description.",
            ),
            ("--kind KIND", KINDS),
            (
                "--pointer POINTER",
                "Select the description, such as /sdfProperty/temperature.",
            ),
        ],
        notes: "Use - for one model on standard input.",
    },
    CommandHelp {
        name: "author",
        summary: "Compose a new sdfThing from reusable definitions and a saved plan.",
        usage: &[
            "author --plan PLAN --source KEY PATH [--source KEY PATH ...] --document-output DEST [OPTIONS]",
        ],
        examples: &[
            (
                "Compose a document using a plan and its base source",
                "author --plan recipe.json --source base base.sdf.json --document-output authored.sdf.json",
            ),
            (
                "Supply another source with its own key",
                "author --plan recipe.json --source base base.sdf.json --source types types.sdf.json --document-output combined.sdf.json",
            ),
        ],
        options: DOCUMENT_OPTIONS,
        notes: "Use source keys that match your plan. The examples use a key named base.\nUse - once for a plan or source on standard input. Document and report need\nseparate destinations. Output files cannot replace inputs, directories or symbolic links.",
    },
    CommandHelp {
        name: "edit",
        summary: "Replace or remove qualities in a model using a saved edit plan.",
        usage: &[
            "edit --plan PLAN --source KEY PATH [--source KEY PATH ...] --document-output DEST [OPTIONS]",
        ],
        examples: &[(
            "Apply an edit plan and keep the original source",
            "edit --plan edits.json --source base base.sdf.json --document-output revised.sdf.json",
        )],
        options: DOCUMENT_OPTIONS,
        notes: "Use source keys that match your plan. The example uses a key named base.\nUse - once for a plan or source on standard input. Document and report need\nseparate destinations. Output files cannot replace inputs, directories or symbolic links.",
    },
];

pub(crate) fn render(style: Style, command: Option<&str>) -> String {
    let mut output = String::new();
    if let Some(help) = command.and_then(|name| COMMANDS.iter().find(|help| help.name == name)) {
        writeln!(output, "{}\n", help.summary).expect("help text");
        examples(&mut output, style, help.examples);
        heading(&mut output, style, "Usage:");
        for usage in help.usage {
            command_line(&mut output, style, usage);
        }
        heading(&mut output, style, "Options:");
        for (syntax, description) in help.options {
            writeln!(
                output,
                "  {}\n      {}",
                style.syntax(syntax),
                description.replace('\n', "\n      ")
            )
            .expect("help option");
        }
        if matches!(
            help.name,
            "inspect" | "sample" | "generate" | "trace" | "reference" | "schema"
        ) {
            writeln!(output, "\n{MODEL_INPUTS}").expect("input guidance");
        }
        writeln!(output, "\n{}", help.notes).expect("command guidance");
        if matches!(help.name, "check" | "diff") {
            heading(&mut output, style, "Directory selection:");
            writeln!(output, "{DIRECTORY_NOTES}").expect("directory guidance");
            if help.name == "check" {
                output.push_str("Limits: 256 directories, 256 exclusions, 64 levels below each root, 10,000 entries,\n256 sources and 1 MiB of path metadata.\n");
            } else {
                output.push_str("Limits per revision: 256 sources, 64 levels below the root, 10,000 entries\nand 1 MiB of path metadata. At most 256 exclusions.\n");
            }
        }
        global_options(&mut output, style, false);
        writeln!(
            output,
            "\nGuide: https://esdieff.com/docs/command-line#{}",
            help.name
        )
        .expect("guide link");
    } else {
        output.push_str("Esdieff checks connected-device models written in Semantic Definition Format (SDF).\n\n");
        examples(
            &mut output,
            style,
            &[
                ("Check a model file", "check model.sdf.json"),
                (
                    "Generate test values from a model",
                    "generate --document model.sdf.json model.sdf.json",
                ),
            ],
        );
        output.push_str("\nReplace model.sdf.json with your model file.\n");
        heading(&mut output, style, "Usage:");
        command_line(&mut output, style, "COMMAND [OPTIONS]");
        command_line(&mut output, style, "--version");
        heading(&mut output, style, "Commands:");
        for help in COMMANDS {
            writeln!(
                output,
                "  {}{}{}",
                style.span(Role::Command, help.name),
                " ".repeat(11 - help.name.len()),
                help.summary
            )
            .expect("command list");
        }
        global_options(&mut output, style, true);
        output.push_str("\nRun esdieff COMMAND --help for options and examples.\nGuide: https://esdieff.com/docs/command-line\n");
    }
    output
}

fn heading(output: &mut String, style: Style, text: &str) {
    writeln!(output, "\n{}", style.span(Role::Heading, text)).expect("help heading");
}

fn examples(output: &mut String, style: Style, examples: &[(&str, &str)]) {
    writeln!(output, "{}", style.span(Role::Heading, "Examples:")).expect("examples heading");
    for (purpose, command) in examples {
        writeln!(output, "  {purpose}:").expect("example purpose");
        command_line(output, style, command);
    }
}

fn command_line(output: &mut String, style: Style, command: &str) {
    let (name, rest) = command.split_once(' ').unwrap_or((command, ""));
    let role = if name == "COMMAND" {
        Role::Placeholder
    } else {
        Role::Command
    };
    write!(
        output,
        "    {} {}",
        style.span(Role::Command, "esdieff"),
        style.span(role, name)
    )
    .expect("command name");
    if !rest.is_empty() {
        write!(output, " {}", style.syntax(rest)).expect("command syntax");
    }
    output.push('\n');
}

fn global_options(output: &mut String, style: Style, version: bool) {
    heading(output, style, "Global options:");
    for (syntax, description) in [
        ("-h, --help", "Show this help."),
        (
            "--color auto|always|never, --color=MODE",
            "Choose terminal styling (default: auto).",
        ),
    ] {
        writeln!(output, "  {}\n      {description}", style.syntax(syntax)).expect("global option");
    }
    if version {
        writeln!(
            output,
            "  {}\n      Print the installed version. Use this option on its own.",
            style.syntax("--version")
        )
        .expect("version option");
    }
}
