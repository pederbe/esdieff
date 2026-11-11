# Troubleshooting

Find the message or symptom below for its cause and fix. Human reports show
supplied filenames and available byte locations beside source details and
findings. In a terminal, report lines also start with a coloured status symbol.

## Command not found

Your shell cannot find `esdieff`. The folder it was installed into is not on
your `PATH`. Add that folder to your `PATH` and open a new terminal. Cargo
installations normally use `~/.cargo/bin`. On Windows, the folder is the one
that contains `esdieff.exe` from the extracted ZIP.

## The check is undetermined because a target is unavailable

```text
Esdieff: undetermined
...
incomplete reference.unavailable: supplied global target unavailable
```

The model refers to a definition in a file you did not supply. Esdieff resolves
references only among the supplied files and never fetches a namespace URI.
The command exits with status 3. Supply the file that holds the definition,
and name the model you want checked with `--root`.

```sh
esdieff check --root device.sdf.json device.sdf.json types.sdf.json
```

[Resolve missing and ambiguous name references](guides/resolving-name-references.md)
shows how to find which definition is missing.

## A model file could not be read

```text
incomplete input.acquisition: Could not read source content: No such file or directory (os error 2). Check the path and read permissions
```

Esdieff could not read a file you named. The report's source line identifies the
file, and the command exits with status 3. The operating system's explanation
depends on the cause and your platform. Correct a missing path or restore read
access, then run the command again.

Repeated notices about unfinished assessment areas appear once per notice code,
with a count. The JSON report retains each area separately. These notices explain
which checks could not finish after the file failure.

## JSON grammar violation

```text
error json.syntax: JSON grammar violation
```

The file is not valid JSON, so the check fails with status 1. The finding shows
the available byte offset in that file. Fix the syntax, for example a missing
closing brace, and check the file again.

## No input or no model files selected

```text
esdieff: at least one explicit input or directory is required
```

Name at least one model file, or a directory with `--directory`.

```text
esdieff: cannot prepare directory input "models": no regular .sdf.json files selected
```

`--directory` selects files directly in the directory whose names end in
`.sdf.json`. Rename the model files to end that way, or add `--recursive` when
they are in subdirectories. A file with another name can still be supplied by
its path.

## The generated values have no `not_multiple 5.25` line

You are following [Install and try a model](getting-started/README.md) and the
report shows 1 description and 9 values instead of 4 and 75. You saved the smaller
`thermostat.sdf.json` from the repository, which has no half-degree step. Save
[rich.sdf.json](https://github.com/pederbe/esdieff/blob/main/examples/thermostat-check/rich.sdf.json)
as `thermostat.sdf.json` instead.

## A CI step passes although the check failed

The check's output is piped into another program, such as `jq`, and the shell
reports that program's status. In Bash, run `set -o pipefail` before the pipeline, or
run the check as its own step. [Use exit statuses](guides/checks-and-reports.md#use-exit-statuses)
lists what each status means.

## Still stuck

[Open an issue](https://github.com/pederbe/esdieff/issues) with the command you
ran and what it printed.
