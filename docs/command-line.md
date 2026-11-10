# Command line

Every `esdieff` command reads the model files you name, does one job and
reports the result on standard output. Model files are always supplied
explicitly. Esdieff does not fetch the namespace URIs a model mentions.

```console
$ esdieff check thermostat.sdf.json
$ esdieff generate --document thermostat.sdf.json thermostat.sdf.json
```

Run `esdieff --help` for the same synopses in your terminal, and
`esdieff --version` for the installed version.

## Commands

| Command | What it does | Details |
| --- | --- | --- |
| [`check`](#check) | Check models, or run a saved check plan | [Inspect and check a model](guides/inspect-and-check.md) |
| [`inspect`](#inspect) | Show one definition as written or as composed | [Inspect and check a model](guides/inspect-and-check.md#inspect-what-the-interface-contains) |
| [`sample`](#sample) | Check one JSON value against one description | [Selected samples](sample-contract.md) |
| [`generate`](#generate) | Generate test values from model rules | [Generated values](generation-contract.md) |
| [`trace`](#trace) | Check a log of recorded values | [Recorded traces](trace-contract.md) |
| [`diff`](#diff) | Compare two revisions | [Revision comparison](revision-comparison-contract.md) |
| [`reference`](#reference) | Write a Markdown reference for a Thing or Object | [Generate a model reference](model-reference.md) |
| [`schema`](#schema) | Export JSON Schema for one description | [Export JSON Schema](json-schema.md) |
| [`author`](#author) | Compose a new `sdfThing` from reusable definitions | [Composed sdfThing authoring](authoring-contract.md) |
| [`edit`](#edit) | Replace or remove qualities in an existing document | [Authored-quality editing](editing-contract.md) |

## check

```text
esdieff check [--directory DIR] [--recursive] [--exclude RELATIVE_PATH] [--root INPUT] [--assessment initial-v1|expanded-modified-v1|expanded-units-v1|expanded-content-format-v1|expanded-pattern-syntax-v1|expanded-numeric-v1|expanded-consistency-v1|full-baseline] [--registry-knowledge recorded|withheld] [--strict-warnings] [--format human|json|sarif] [INPUT...]
esdieff check --plan PLAN [--format human|json]
```

Checks the supplied model documents. At least one `INPUT` or `--directory` is
required. `INPUT` is a file path, or `-` for one document on standard input.

| Option | Meaning |
| --- | --- |
| `--root INPUT` | Check this document and the dependencies it needs. Without it, every supplied model document is checked. |
| `--assessment PROFILE` | Choose how much to check. The default is `initial-v1`. [Choose how much to check](guides/inspect-and-check.md#choose-how-much-to-check) describes the profiles. |
| `--strict-warnings` | Fail the check on warnings as well as errors. |
| `--directory DIR` | Select the regular files directly in `DIR` whose names end in `.sdf.json`. Repeat it for several directories. |
| `--recursive` | Descend into the subdirectories of every supplied directory, including hidden entries. |
| `--exclude RELATIVE_PATH` | Leave out a literal file or subtree, relative to each directory. Repeat it for several paths. |
| `--registry-knowledge recorded\|withheld` | Use or withhold the recorded registry snapshots. An explicit value needs an expanded unit or content-format assessment, or `full-baseline`. |
| `--format human\|json\|sarif` | Choose the report format. |
| `--plan PLAN` | Run the cases of a [check plan](check-plans.md) instead of a direct check. |

[Check a local model collection](directory-discovery.md) states the directory
selection rules and limits.

## inspect

```text
esdieff inspect --view source|effective --document INPUT --pointer POINTER [--required-declarations] [--format human|json] INPUT...
```

Shows the definition at a JSON Pointer in one document. The `source` view is
the JSON as written. The `effective` view is the definition after `sdfRef`
composition, with the origin of each quality. `--required-declarations` adds
the declaration relationships selected by `sdfRequired`.

## sample

```text
esdieff sample --document MODEL --kind property|data|action_input|action_output|event_output|context --pointer POINTER --sample SAMPLE [--strict-warnings] [--format human|json] MODEL_INPUT...
```

Checks the JSON value in `SAMPLE` against the description that `--kind` and
`--pointer` select in `MODEL`. The value is admitted, rejected or undetermined.
[Check one sample](guides/testing-values.md#check-one-sample) shows a run.

## generate

```text
esdieff generate --document MODEL [--kind property|data|action_input|action_output|event_output|context --pointer POINTER] [--format human|json|jsonl] MODEL_INPUT...
```

Generates test values for one selected description, or for every affordance
description of `MODEL` when `--kind` and `--pointer` are left out. The two
options come together or not at all. `--format jsonl` writes the values as
JSON Lines that `trace` can check.

## trace

```text
esdieff trace --document MODEL --trace FILE [--format human|json] MODEL_INPUT...
```

Checks each entry of the JSON Lines file `FILE` against `MODEL`. `FILE` is a
path, or `-` for standard input.

## diff

```text
esdieff diff --old PATH --new PATH [--recursive] [--exclude RELATIVE_PATH] [--perspective client|implementer] [--acknowledged FILE] [--acknowledgements-output FILE] [--replace-acknowledgements] [--format human|json|sarif|markdown]
```

Compares an earlier and a later revision. Both paths are files or both are
directories, and directories pair their documents by relative path.

| Option | Meaning |
| --- | --- |
| `--perspective client\|implementer` | Classify differences for clients built against the earlier model, which is the default, or for implementations of it. |
| `--acknowledged FILE` | Read a file of acknowledged differences, so only unacknowledged differences decide the result. |
| `--acknowledgements-output FILE` | Write the report's eligible acknowledgement entries to a file for review. |
| `--replace-acknowledgements` | Allow `--acknowledgements-output` to replace an existing file. |
| `--recursive`, `--exclude RELATIVE_PATH` | Select documents in both directories as for `check`. |
| `--format human\|json\|sarif\|markdown` | Choose the report format. `markdown` writes a changelog. |

[Compare two revisions](guides/revision-comparison.md) walks through a
comparison and its acknowledgements.

## reference

```text
esdieff reference [--uses] --document MODEL --pointer POINTER INPUT...
```

Selects one effective Thing or Object and writes a Markdown reference to
standard output. It does not check the model. `--uses` adds a section
listing where a named definition is used across the supplied models. It also
lets you select a reusable data, Property, Action, Event or Context definition.

## schema

```text
esdieff schema --document MODEL --kind KIND --pointer POINTER [--format json|schema] INPUT...
```

Exports the effective constraints of one selected description for use with
JSON Schema tools. The default output is a JSON report. `--format schema`
writes only the standalone schema.

## author

```text
esdieff author --plan PLAN --document-output DEST [--report-output DEST] [--check initial-v1|full-baseline] [--strict-warnings] [--format human|json] [--replace-document] [--replace-report] --source KEY PATH...
```

Composes a new `sdfThing` from the definitions in the supplied sources, as the
plan describes, and writes it to `--document-output`. Repeat `--source KEY PATH`
for each model file. `--check` also checks the generated document.
`--report-output` writes the report to a file instead of standard output.
An existing destination is replaced only with `--replace-document` or
`--replace-report`.

## edit

```text
esdieff edit --plan PLAN --document-output DEST [--report-output DEST] [--check initial-v1|full-baseline] [--strict-warnings] [--format human|json] [--replace-document] [--replace-report] --source KEY PATH...
```

Replaces or removes the qualities a plan names in one existing document and
writes the result to `--document-output`. The options match `author`.
[Compose and edit models](guides/authoring-and-editing.md) shows both commands.

## Global options

| Option | Meaning |
| --- | --- |
| `--color auto\|always\|never`, `--color=MODE` | Choose terminal styling. The default is `auto`. |
| `--no-color` | The same as `--color never`. |
| `-h`, `--help` | Show the help. |

`esdieff --version`, on its own, prints the installed version. Put `--` before
a path that begins with `-`.

Use `-` for standard input once across the model, sample and trace inputs of a
command. `diff` never reads standard input.

## Exit statuses

Every command exits with 0 when the requested work passed, 1 when a finding
failed it, 2 for an invalid invocation, 3 when some work could not be settled,
and 4 when a host or output operation failed.
[Use exit statuses](guides/checks-and-reports.md#use-exit-statuses) has the
full table and what each status means in CI.
