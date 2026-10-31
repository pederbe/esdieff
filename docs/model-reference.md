# Generate a model reference

Use `esdieff reference` to read one Thing or Object as a Markdown document. The
reference includes its effective nested groupings, Properties, Actions, Events,
Context metadata and contained reusable data descriptions. Reused definitions
appear where they are imported, with source links showing their nearest origins
and importing consumers.

Select the document and its JSON Pointer explicitly. Supply every dependency as
an input. Inputs have no first/last precedence. Esdieff does not fetch namespace
URLs or discover directories for this command.

From the repository root, document the thermostat:

```sh
esdieff reference --document examples/thermostat-check/rich.sdf.json --pointer /sdfObject/Thermostat examples/thermostat-check/rich.sdf.json > thermostat.md
```

Document the composed installation with its reusable source:

```sh
esdieff reference --document examples/context-reuse/device.sdf.json --pointer /sdfThing/Installation examples/context-reuse/base.sdf.json examples/context-reuse/device.sdf.json > installation.md
```

Use `-` once for standard input. Use `--` before input paths beginning with a
hyphen. Shell redirection saves the reference. Output is UTF-8 Markdown with no
terminal colors. Internal links remain usable when you move the document.

## Explore shared definitions

For a complete example connecting source explanations to client compatibility,
follow the [shared-definition walkthrough](guides/shared-definition-changes.md).

Add `--uses` to explore a named authored definition across every supplied model
document. This also permits selecting a reusable `sdfData`, Property, Action,
Event or Context definition. For example, compare two devices using a common
temperature description:

```sh
esdieff reference --uses --document examples/shared-definitions/shared.sdf.json --pointer /sdfData/Temperature examples/shared-definitions/shared.sdf.json examples/shared-definitions/devices.sdf.json > temperature-uses.md
```

The appended **Shared definition uses** section lists effective document names
and pointers. Normal inherits a maximum of 100. Cool overrides it with 40. Each
scalar occurrence shows its exact effective value and origin. An override links
both its current source and its previous source, so you can follow the differing
limit back to the shared definition. **Importing consumer** links point to authored
references and remain separate from effective occurrence coordinates.

This view uses existing nearest-source explanations. It lists occurrences whose
current or previous source lies within the selected authored definition. It is
not a complete transitive reference graph. Earlier contributors replaced through
multiple overrides might no longer be retained. Deleted content is excluded.
References inside opaque content gain no interpretation. A definition available
only through composition has no authored subtree at its effective coordinate,
so use navigation reports that limitation instead of substituting its donor.

Only explicitly supplied model documents are explored, in input order. Sample
inputs are excluded. Namespace URLs are never fetched. Unavailable companion
models and exhausted processing or presentation allowances make the reference
partial or failed while preserving available occurrences. A message that no
established uses were found applies only to the stated scope and available
branches. Ordinary references without `--uses` keep their grouping selection and
dependency scope.

## What the reference establishes

The reference shows effective constraints, exact numeric values, required map
entries, declaration-requiredness relationships and Property capabilities.
Required presence and nullability remain separate. Specified defaults have no
authored origin. A declared `default` is documentary data and is never inserted
into a value.

A displayed illustrative value was admitted by Sample-value checking for its
particular description. It is at most 4 KiB. The search uses the fixed
[Generation contract](generation-contract.md), retaining at most 64 candidates
per description and searching at most 1,024 descriptions per reference. Nested
map entries, items, alternatives and reusable data definitions remain documented
without separate generated examples. An unavailable example or a search finding
no small admitted value does not establish an impossible description.

Context is non-interactive metadata under the recorded
`draft-ietf-asdf-sdf-nonaffordance-06`. It has no read, write or observe
capabilities. Absent Context nullability remains explicitly unresolved.
Unsupported qualities are retained as declared content whose meaning is not
established. Displaying a format, unit or pattern does not establish that every
use of its meaning was checked.

## Outcomes and limits

Completed documentation exits with 0. Unavailable nested definitions or collections
or presentation omissions can produce a partial reference, exiting with 3.
Definite requested acquisition or composition failures exit with 1, including
when the document also has omissions. These domain results still write usable
Markdown, source attempts and processing conditions. Unsupported meaning or an
unavailable illustrative example alone does not make documentation partial.

Invalid invocation, including a definitely absent or wrong-class selection, exits
with 2 and emits no reference. Host or output failures
exit with 4. The report byte allowance, 8 MiB by default, keeps whole optional
rows and sections and reports omissions. An allowance too small for the
mandatory identity and source inventory is rejected before composition.

A completed reference is documentation, not a Full model assessment, compatibility
verdict, satisfiability proof or device test. Run `esdieff check` separately.

## Library use

```rust
use esdieff::{Catalogue, ReferenceRequest, SourceInput};

let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
    "model",
    include_bytes!("../examples/thermostat-check/rich.sdf.json"),
)])?;
let document = catalogue.source_id("model").unwrap();
let result = catalogue.reference(ReferenceRequest::new(
    document,
    "/sdfObject/Thermostat",
))?;
let markdown = result.to_markdown();
let disposition = result.disposition();
# Ok::<(), &'static str>(())
```

The result borrows the immutable catalogue and owns its derived documentation.
For shared-definition navigation, add `.with_uses()` to the `ReferenceRequest`.
`complete()`, `disposition()`, `conditions()` and `omissions()` expose the outcome
without parsing Markdown. An identical request with identical ordered inputs and
settings produces identical content when the same work completes. Pattern
deadlines retain their existing timing-sensitive outcomes.
