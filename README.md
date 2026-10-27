# Esdieff

An SDF toolkit for connected-device models (RFC 9880).

Esdieff is a Rust library and command-line tool for checking, composing, editing, and inspecting device descriptions in conformance with [IETF RFC 9880](https://www.rfc-editor.org/info/rfc9880/).

For example, an SDF thermostat model could describe its readable temperature, writable target temperature, reset action, and overheating event, including units and allowed values. These descriptions can reuse definitions shared with other device models.

Esdieff checks models and reports invalid definitions and broken references, with clear locations and explanations. It combines supplied model files to show what the resulting device interface contains, and summarizes properties, actions, events, types, units and constraints. It also checks sample values, authors and edits models, and compares two revisions of a catalogue to find the changes that would break the apps using its devices.

A device manufacturer could change a shared temperature definition and use Esdieff to catch mistakes across its device catalogue before downstream tools consume those models. Other Rust applications could embed the same checking and composition library. Actual network interaction requires separate protocol bindings.

For explicit document acquisition, source inspection and Rust usage, see the [supplied-source workflow](docs/source-workflow.md). Structured reports follow the [public report contract](docs/report-contract.md).

## Install and try it

Esdieff supports macOS 15+, Linux and Windows. Version 0.1.0 is being prepared
for its first public release. The installation commands below are prepared for
launch.

On macOS:

```sh
brew install pederbe/tap/esdieff
```

On Windows, once the WinGet listing is accepted:

```powershell
winget install --id PederBergan.Esdieff --exact
```

On Linux, use the [apt or dnf installation instructions](docs/getting-started/README.md#linux).
Normal package-manager upgrades keep Esdieff current.
[Direct downloads](https://github.com/pederbe/esdieff/releases) remain available.
Rust users can install the prebuilt command with `cargo binstall esdieff`, or
build it from source with `cargo install --locked esdieff`.

Run `esdieff --version`, then follow the
[thermostat example](docs/getting-started/README.md#try-the-thermostat-example)
to inspect a device model and generate test values.

Acquire a deliberately selected public GitHub file with `esdieff acquire`, a complete commit and an independently reviewed SHA-256. The [acquisition contract](docs/acquisition-contract.md) explains preserved snapshots, separate receipts and offline replay.

Use `--assessment full-baseline` to check every model requirement Esdieff records from RFC 9880. Two kinds of extra advice go beyond those requirements and have their own assessments. `expanded-numeric-v1` finds number ranges that no integer or step value can satisfy, and `expanded-consistency-v1` finds a `const` or `default` value that its own description excludes. The [model meaning contract](docs/model-meaning-contract.md) distinguishes formal grammar admission, established constraints and unavailable interpretations without requiring a sample value. The [model coverage boundaries](docs/model-coverage-boundaries.md) explain the remaining gaps and what would resolve them.

Selected Property, sdfData, Action input/output and Event output samples use the [sample workflow](docs/sample-contract.md). Ordinary maps check required presence and described present entries; ordinary arrays check inclusive exact counts and established element descriptions. Const and array uniqueness compare exact numbers, decoded text and supported compound values in their shared effective context. Both recurse through supported numeric, string, Boolean, object and alternative meanings. Reports distinguish rejection from unfinished applicable constraints and preserve original sample spans or the absence of spans for typed projections.

Selected `byte-string` samples use canonical unpadded base64url. Selected `unix-time` samples use exact numeric POSIX coordinates, including fractions and large nonnegative values. Negative nonzero coordinates retain usable input but have unfinished abstract denotation. Mandatory Sample meaning identifies final support and limitations separately from interpretations consulted while trying alternatives.

Save named catalogue checks, selected-model checks and revision comparisons with `esdieff check --plan checks.json`. The [plan example](docs/check-plans.md) shows reusable sources, per-case assessment and warning policy, and one human or JSON run report.

Check a local collection with `esdieff check --directory models`. [Directory discovery](docs/directory-discovery.md) selects regular `.sdf.json` files, with explicit recursion, literal exclusions and selected roots.

Compare two revisions of a model with `esdieff diff --old v1 --new v2` before publishing a change. [Revision comparison](docs/revision-comparison-contract.md) classifies each difference as breaking, compatible or undetermined for the apps that use the devices, follows a changed shared definition to every device model that reuses it, and fails the run on a breaking change so CI can stop it. For a deliberate major revision, add `--acknowledgements-output reviewed.json` to write the eligible differences to a file. Review its entries, then pass the intended breaking changes and reviewed undetermined ones with `--acknowledged reviewed.json`. Only the differences left unacknowledged then decide the result, and the report still shows each acknowledged difference with its classification. When a changed document breaks apps but its `info` `version` stays the same, goes down or disappears, the report also warns that the version should compare higher, as RFC 9880 recommends, without changing the result.

Show results where reviewers already look. `esdieff check --format sarif` and `esdieff diff --format sarif` write SARIF logs that code-scanning tools such as GitHub's display as annotations on the model file, with acknowledged changes marked as suppressed. `esdieff diff --format markdown` writes the comparison as a changelog grouped by breaking, undetermined and compatible changes, ready for release notes. The [report contract](docs/report-contract.md#sarif-projection) describes both formats.

Generate test values with `esdieff generate --document device.sdf.json device.sdf.json`. Esdieff builds valid, boundary and invalid values for every Property, Action and Event in the document, such as a minimum, a value just below it or a map missing a required entry. Each value is labelled with the constraint it targets and checked against the model, so firmware and app tests get expectations they can trust without hand-written payloads. Add `--format jsonl` to save them as test vectors. The [generation contract](docs/generation-contract.md) lists every target and limit.

Check recorded device and app values in bulk with `esdieff trace --document device.sdf.json --trace values.jsonl device.sdf.json`. Each line of the JSON Lines file names a Property, Action or Event, a value and optionally whether the model should admit it. Esdieff composes the model once, checks every line on its own and points each failure at its line, so a log of a hundred thousand values checks in well under a second. Generated test vectors replay the same way, which shows exactly which values a new model revision admits differently. The [trace contract](docs/trace-contract.md) describes the line format and limits.

Create a new composed `sdfThing` with `esdieff author --plan recipe.json --source reusable capabilities.sdf.json --document-output product.sdf.json --check initial-v1`. The explicit recipe names the Thing, reusable Object declarations and namespace bindings. Generated references preserve reusable sources; the optional check uses the established selected-model assessment. Document and report destinations are separate, and existing output files require their corresponding replacement option. The [authoring contract](docs/authoring-contract.md) describes library builders, JSON plans, candidates, Authoring fidelity and finite processing limits.

Correct existing annotations and data qualities with `esdieff edit --source model device.sdf.json --plan edits.json --document-output revised.sdf.json --check initial-v1`. The [editing contract](docs/editing-contract.md) shows plans and library requests. Original inputs remain intact, exact numbers retain their spelling, and the report stays separate from the revised document.

## Terminal presentation

Use `--color auto|always|never` or `--color=MODE` before or after any command, including `acquire`. `--no-color` means `--color never`, and `--help` lists the controls. The default is `auto`. Repeated controls use the last valid choice reached during ordinary option scanning. Option values and paths following `--` remain literal input, and an invalid or missing mode is an invocation error.

Automatic styling requires a terminal for the destination stream. stdout and stderr are checked separately, so redirecting one leaves the other's terminal styling available. `TERM=dumb`, nonempty `NO_COLOR`, or unavailable terminal detection suppress automatic styling; an empty `NO_COLOR` does not. `always` overrides those conditions for eligible human output, and `never` suppresses all generated styling, including bold. Environment force conventions do not enable redirected output in `auto` or override `never`.

The palette uses bold default-foreground headings, cyan command and option syntax and notices, green passed outcomes, yellow warnings and unfinished work, and red failures, while retaining the terminal's background. Wording carries the meaning independently of color. Failure diagnostics color only their `esdieff:` prefix. JSON reports, generated SDF documents and embedded source or data content remain plain, and library renderers do not inspect terminal or environment settings.

Human `check`, source/effective `inspect`, and `sample` reports style their headings, outcomes and diagnostic codes. Assessment status `assessed` records completed work, which can contain findings. It remains plain, as does `not_applicable`. Failed outcomes and unfinished work keep separate indications, and strict-warning failures keep yellow warning codes. Broader RFC coverage is a scope disclosure rather than a finding, so its state stays plain. Unfinished requested work is already marked on `Complete` and each assessment area. Counts, names, locations, explanations and embedded inspection JSON remain plain. If ANSI overhead exceeds a report's byte limit, the same retained report is returned in plain text. Library `to_human()` stays plain, and `to_human_with_style(HumanStyle::Ansi)` explicitly opts in without reading the environment.

Human `author` and `edit` reports style generation, fidelity, requested outcomes, optional checking and diagnostic severities independently. Automatic color follows the report's destination: a report file stays plain even when the generated document goes to terminal stdout. `--color=always` can style a human report file. Generated SDF documents and JSON reports remain plain in every mode.

Human `acquire` receipts style their headings, the `acquired` or `failed` outcome, an incomplete result and each failure phase. A document committed before a later failure keeps a cyan notice on `Document committed true`. Repository, commit, path, URI, digests, byte counts, destinations and failure explanations remain plain. Receipt color follows the receipt's destination in the same way as `author` reports, and styling that would exceed the 64 KiB receipt ceiling falls back to the same plain receipt. Downloaded document bytes and JSON receipts never change with the color mode.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this work, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

---

<sub>Esdieff is built by [Peder Bergan](https://pederbe.dev).</sub>
