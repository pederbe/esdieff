# Use the Rust library

The model operations behind the `esdieff` command are available as a Rust
library, so you can embed checking, inspection, generation and revision
comparison in your own tools. The library forbids unsafe Rust in its own code
and performs no network access. The separate `acquire` command's network
retrieval and command-line file-output adapters are outside this library API.

## Add the crate

Add the released crate from [crates.io](https://crates.io/crates/esdieff):

```toml
[dependencies]
esdieff = "0.2"
```

The crate uses the 2024 edition. Use the Rust version required by your selected
release. For APIs added after 0.2.0, including model references and JSON Schema
export, depend on a current repository revision instead:

```toml
[dependencies]
esdieff = { git = "https://github.com/pederbe/esdieff" }
```

Pin a verified commit with `rev = "..."` for a reproducible Git dependency.

## The entry points

A `Catalogue` retains the documents you supply and their acquisition attempts.
Build one from `SourceInput` values, each with a key that names it in reports.
`SourceInput::file` reads a path, `SourceInput::supplied` takes bytes you already
hold, and `.sample()` marks a sample value rather than a model document.
Operations take explicit requests:

| Operation | Call | Result |
| --- | --- | --- |
| Check a model | `catalogue.check(CheckRequest)` | `Report` |
| Inspect selected content | `catalogue.inspect(InspectRequest)` | `Report` |
| Read the composed interface | `catalogue.effective(EffectiveRequest)` | `EffectiveModel` |
| Check one sample | `catalogue.check_sample(SampleRequest)` | `Report` |
| Generate values | `catalogue.generate(GenerationRequest)` | `GenerationResult` |
| Check a recorded trace | `catalogue.check_trace(TraceRequest)` | `TraceResult` |
| Compose an sdfThing | `AuthorRequest::author()` | `AuthorResult` |
| Edit qualities | `EditRequest::edit()` | `EditResult` |
| Compare revisions | `ComparisonRequest::compare()` | `RevisionComparison` |
| Document a definition, after 0.2.0 | `catalogue.reference(ReferenceRequest)` | `ReferenceResult` |
| Export JSON Schema, after 0.2.0 | `catalogue.export_schema(SchemaRequest)` | `SchemaResult` |

Checking, inspection, effective views, generation and trace results borrow their
catalogue. Revision comparison borrows both catalogues. Authoring and editing
requests own their input catalogue, and their results own the resulting one.
Schema export owns its serialized output.

`Report`, generation, trace, authoring, editing and comparison results provide
JSON and human report renderers. An `EffectiveModel` instead exposes typed views
and effective JSON. A `ReferenceResult` renders Markdown, and a `SchemaResult`
exposes its JSON fidelity report and any established schema. Query each
operation's disposition separately from its content. Its `exit_code()` maps the
domain result to the command's exit status, while invocation and host errors are
handled by the command-line adapter.

## A complete example

This program checks the thermostat model from [Install and try a model](getting-started/README.md) with the full assessment, reads the effective range of its target temperature and prints the property as written in the file. Create a new binary crate, add the dependency above, put `thermostat.sdf.json` in the crate directory and use this `src/main.rs`:

```rust
use esdieff::{Assessment, Catalogue, CheckRequest, Class, EffectiveRequest, InspectRequest, Lookup, SourceInput};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Supply the model files. Every input is a candidate, and its key names it in reports.
    let catalogue = Catalogue::from_inputs(&[SourceInput::file(
        "thermostat",
        Path::new("thermostat.sdf.json"),
    )])?;
    let thermostat = catalogue.source_id("thermostat").expect("supplied source");

    // Check the thermostat against every RFC 9880 model requirement Esdieff records.
    let request = CheckRequest::new()
        .root(thermostat)
        .assessment(Assessment::FullBaseline);
    let report = catalogue.check(request)?;
    println!("check: {:?}", report.outcome().disposition());
    for condition in report.diagnostics().conditions() {
        println!("{}: {}", condition.code(), condition.message());
    }

    // Read the effective target temperature range, with inherited qualities applied.
    let effective = catalogue.effective(EffectiveRequest::new(thermostat))?;
    let Lookup::Found(object) = effective.definition(Class::Object, "Thermostat") else {
        panic!("Thermostat missing")
    };
    let (_, target) = object
        .children(Class::Property)
        .require()?
        .find(|(name, _)| *name == "targetTemperature")
        .expect("targetTemperature");
    let description = target.require()?.data_description().require()?;
    println!(
        "targetTemperature: {} to {}",
        description.minimum().require()?.lexeme(),
        description.maximum().require()?.lexeme()
    );

    // Show the authored source of that property as written in the file.
    let pointer = "/sdfObject/Thermostat/sdfProperty/targetTemperature";
    let inspection = catalogue.inspect(InspectRequest::new(thermostat, pointer))?;
    if let Some(json) = inspection.inspection_json() {
        println!("source: {}", json.raw_json());
    }
    std::process::exit(report.outcome().disposition().exit_code().into());
}
```

<pre class="terminal"><code><span class="p">$</span> cargo run --quiet
check: Passed
targetTemperature: 5 to 30
source: {
          "description": "Requested room temperature in half-degree increments",
          "type": "number", "nullable": false, "minimum": 5, "maximum": 30,
          "multipleOf": 0.5, "unit": "Cel", "readable": true, "writable": true
        }
</code></pre>

A few things to notice. Operations return `Err(&'static str)` only for an invalid request, such as an unknown source key. A model problem is never an error: it is a finding in the report, and the disposition says whether the check passed. Fields that may be absent, unfinished or outside the supported meaning are `Field` values, and `require()` turns anything but a present value into an error, which suits a tool that expects a complete model. Numbers are exact: `lexeme()` returns the text from the document, and `ExactNumber` compares values without converting them to floating point. The source view prints the member byte for byte, whitespace included.

## Keep the memory and work bounded

Operations apply fixed internal safeguards for input size, decoding depth,
reference expansion, processing work and retained report size. Ordinary callers
use the defaults through `Catalogue::from_inputs`, `Catalogue::take_inputs` and
`JsonBuilder::new`. Exhaustion leaves affected work explicitly unfinished.
A request that cannot retain its mandatory report envelope is a request error.
[ADR 0056](adr/0056-fixed-internal-safeguards-and-rust-patterns.md) explains the
fixed policy and the cooperative pattern timeout.

## Migrate resource configuration

The public `Limits` type and processing-budget setters have been removed.
Replace `Catalogue::with_limits(inputs, limits)` with
`Catalogue::from_inputs(inputs)`, and `Catalogue::take_inputs_with_limits(inputs, limits)`
with `Catalogue::take_inputs(inputs)`. Replace `JsonBuilder::with_limits(limits)`
with `JsonBuilder::new()`, which needs no configuration validation.
Use `DifferenceAcknowledgements::from_json(bytes)` without a limits argument.
`NumberWork` is also internal. Call `ExactNumber::compare(other)` to obtain an
`Ordering` directly from admitted exact values. Location accessors use
`pointer()` with the fixed internal 8 KiB ceiling, without a byte-budget argument.
Semantic request options, including selectors, assessments and strict warnings,
are unchanged. No removed budget is silently ignored.

## Where the contracts are

Each operation's reference page states what its result promises, with a Rust example at the top:

- [Supplied source documents](source-workflow.md) covers catalogues, source and effective views, locations and exact numbers.
- [Selected samples](sample-contract.md) covers `SampleRequest`, admission and typed sample values built with `JsonBuilder`.
- [Generated values](generation-contract.md) covers `GenerationRequest`, targets and JSON Lines output.
- [Recorded traces](trace-contract.md) covers `TraceRequest` and per-entry outcomes.
- [Composed sdfThing authoring](authoring-contract.md) and [Authored-quality editing](editing-contract.md) cover `AuthorRequest`, `EditRequest`, plans and fidelity.
- [Revision comparison](revision-comparison-contract.md) covers `ComparisonRequest`, differences, witnesses and acknowledgements.
- [Model references](model-reference.md) and [JSON Schema export](json-schema.md) cover the current-checkout `ReferenceRequest` and `SchemaRequest` APIs.
- [Workflow and report contract](report-contract.md) covers the shared report envelope that `to_json()` renders.

The repository's [examples directory](https://github.com/pederbe/esdieff/blob/main/examples) holds complete programs that read effective values and requirement coverage.
