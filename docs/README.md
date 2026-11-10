# Esdieff documentation

<p class="docs-lead">Esdieff is a command-line toolkit and Rust library for connected-device models written in <strong>Semantic Definition Format</strong>, the SDF of IETF RFC 9880. It checks models, shows what a composed interface contains, compares revisions and turns model rules into test values. Model operations run offline on the documents you supply.</p>

## Where to start

- **New to Esdieff?** [Install and try a model](getting-started/README.md) installs the command, runs the supplied thermostat model and reads your first generated values.
- **Checking a model?** [Inspect and check a model](guides/inspect-and-check.md) supplies model files and shared definitions, runs the checks and follows each finding back to its source.
- **Testing a device or an app?** [Test values against a model](guides/testing-values.md) checks single samples, generates boundary and invalid values and checks a log of recorded values in one run.
- **Reviewing a model change?** [Compare two revisions](guides/revision-comparison.md) shows which changes between two revisions break the apps that use a device, and acknowledges the ones you intend.
- **Running checks in CI?** [Repeat checks and share reports](guides/checks-and-reports.md) saves checks as plans, checks whole directories and brings results into automation with JSON, SARIF and Markdown.
- **Building your own tool?** [Use the Rust library](rust-library.md) embeds the same checking, inspection and comparison in your own program.
- **Looking up a command or option?** See the [command line reference](command-line.md).
- **Something not working?** See [troubleshooting](troubleshooting.md).

Esdieff works on model files and JSON values. It does not connect to devices or test network protocols.

## More guides

Each guide walks through one task with real commands and their output.

- [Resolve missing and ambiguous name references](guides/resolving-name-references.md) follows a finding to its authored reference and corrects the supplied catalogue.
- [Follow a shared definition through a change](guides/shared-definition-changes.md) connects effective uses and local overrides to a revision comparison.
- [Compose and edit models](guides/authoring-and-editing.md) builds a new `sdfThing` from reusable definitions and makes repeatable edits to an existing document.

## Reference

The reference pages are the contracts behind each command. They state what an operation promises, what its report contains and where its limits lie.

- [Command line](command-line.md) lists every command with its options and exit statuses.
- [Workflow and report contract](report-contract.md) covers the shared report envelope, exit statuses and the command-line rules.
- [Supplied source documents](source-workflow.md) explains how supplied files become a catalogue and how source and effective views differ.
- [Repeatable catalogue checks](check-plans.md) and [Check a local model collection](directory-discovery.md) describe how to select local inputs.
- [Model meaning assessment](model-meaning-contract.md) and its subpages define what each assessment level checks.
- [Selected samples](sample-contract.md), [Generated values](generation-contract.md) and [Recorded traces](trace-contract.md) cover value checking.
- [Model references](model-reference.md) document effective definitions and shared uses. [JSON Schema export](json-schema.md) exports supported selected payload constraints.
- [Composed sdfThing authoring](authoring-contract.md), [Authored-quality editing](editing-contract.md) and [Revision comparison](revision-comparison-contract.md) cover changing and comparing models.
- The [glossary](domain-language.md) defines the terms the reports and contracts use, such as catalogue, effective definition and breaking change.

## How it works and why

[How Esdieff works](guides/how-esdieff-works.md) follows the process map from supplied files to model checks, revision comparisons and test values. The [architecture decisions](adr/README.md) in the repository record why Esdieff behaves as it does, from exact numbers to client-side revision comparison. [Contributing and project records](project.md) links the contribution guide and the changelog.
