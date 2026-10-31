# Esdieff documentation

<p class="docs-lead">Esdieff is a command-line toolkit and Rust library for connected-device models written in <strong>Semantic Definition Format</strong>, the SDF of IETF RFC 9880. It checks models, shows what a composed interface contains, compares revisions and turns model rules into test values. Every operation runs offline on the files you supply.</p>

## Start here

- [How Esdieff works](guides/how-esdieff-works.md) Follow the process map from supplied files to model checks, revision comparisons and test values.
- [Install and try a model](getting-started/README.md) Install Esdieff, run the supplied thermostat model and read your first generated values.
- [Inspect and check a model](guides/inspect-and-check.md) Supply model files and shared definitions, run the checks and follow each finding back to its source.
- [Compare two revisions](guides/revision-comparison.md) See which changes between two revisions break the apps that use a device, and acknowledge the ones you intend.
- [Test values against a model](guides/testing-values.md) Check single samples, generate boundary and invalid values and check a log of recorded values in one run.

## Guides

The guides walk through one task each with real commands and their output.

- [Compose and edit models](guides/authoring-and-editing.md) builds a new `sdfThing` from reusable definitions and makes repeatable edits to an existing document.
- [Follow a shared definition through a change](guides/shared-definition-changes.md) connects effective uses and local overrides to a revision comparison.
- [Repeat checks and share reports](guides/checks-and-reports.md) saves checks as plans, checks whole directories and brings results into automation with JSON, SARIF and Markdown.
- [Use the Rust library](rust-library.md) embeds the same checking, inspection and comparison in your own program.

## Reference

The reference pages are the contracts behind each command. They state what an operation promises, what its report contains and where its limits lie.

- [Workflow and report contract](report-contract.md) covers the shared report envelope, exit statuses and the command-line rules.
- [Supplied source documents](source-workflow.md) explains how supplied files become a catalogue and how source and effective views differ.
- [Repeatable catalogue checks](check-plans.md), [Check a local model collection](directory-discovery.md) and [Pinned document acquisition](acquisition-contract.md) describe the ways to bring documents in.
- [Model meaning assessment](model-meaning-contract.md) and its subpages define what each assessment level checks.
- [Selected samples](sample-contract.md), [Generated values](generation-contract.md) and [Recorded traces](trace-contract.md) cover value checking.
- [Composed sdfThing authoring](authoring-contract.md), [Authored-quality editing](editing-contract.md) and [Revision comparison](revision-comparison-contract.md) cover changing and comparing models.

## Design

The [domain language](domain-language.md) defines the terms the reports and contracts use, such as catalogue, effective definition and breaking change. The [architecture decisions](adr/README.md) record why Esdieff behaves as it does, from exact numbers to client-side revision comparison.
