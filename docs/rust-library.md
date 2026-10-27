# Use the Rust library

Everything the `esdieff` command does is available as a Rust library, so you can embed model checking, inspection, generation and revision comparison in your own tools. The library uses no unsafe code and performs no network access.

## Add the crate

There is no release on crates.io yet, so depend on the repository directly:

```toml
[dependencies]
esdieff = { git = "https://github.com/pederbe/esdieff" }
```

The crate builds on stable Rust with the 2024 edition. Pin a commit with `rev = "..."` once you depend on it in CI.

## The entry points

A `Catalogue` holds the documents you supply. Build one from `SourceInput` values, each with a key that names it in reports: `SourceInput::file` reads a path, `SourceInput::supplied` takes bytes you already hold, and `.sample()` marks a sample value rather than a model. Every operation is a method on the catalogue that takes a request and returns a result you can query or render:

| Operation | Request | Result |
| --- | --- | --- |
| Check a model | `CheckRequest` | `Report` |
| Inspect one definition | `InspectRequest` | `Report` |
| Read the composed interface | `EffectiveRequest` | `EffectiveModel` |
| Check one sample | `SampleRequest` | `Report` |
| Generate values | `GenerationRequest` | `GenerationResult` |
| Check a recorded trace | `TraceRequest` | `TraceResult` |
| Compose a thing | `AuthorRequest` | `AuthorResult` |
| Edit qualities | `EditRequest` | `EditResult` |
| Compare revisions | `ComparisonRequest` | `RevisionComparison` |

Results borrow the catalogue, so keep it alive while you read them. Each has `to_json()` and `to_human()` for the same reports the command prints, and a disposition whose `exit_code()` matches the command's exit status.

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

Every operation runs within `Limits`, which cap input size, decoding depth, work and report size, so an untrusted document cannot make a tool run away. The defaults suit ordinary models. `Catalogue::take_inputs_with_limits` sets others, and an operation that reaches a limit reports the work it could not finish instead of failing silently. [ADR 0009](adr/0009-bounded-processing-and-reporting.md) explains the approach.

## Where the contracts are

Each operation's reference page states what its result promises, with a Rust example at the top:

- [Supplied source documents](source-workflow.md) covers catalogues, source and effective views, locations and exact numbers.
- [Selected samples](sample-contract.md) covers `SampleRequest`, admission and typed sample values built with `JsonBuilder`.
- [Generated values](generation-contract.md) covers `GenerationRequest`, targets and JSON Lines output.
- [Recorded traces](trace-contract.md) covers `TraceRequest` and per-entry outcomes.
- [Composed sdfThing authoring](authoring-contract.md) and [Authored-quality editing](editing-contract.md) cover `AuthorRequest`, `EditRequest`, plans and fidelity.
- [Revision comparison](revision-comparison-contract.md) covers `ComparisonRequest`, differences, witnesses and acknowledgements.
- [Workflow and report contract](report-contract.md) covers the shared report envelope that `to_json()` renders.

The repository's [examples directory](https://github.com/pederbe/esdieff/blob/main/examples) holds complete programs that read effective values and requirement coverage.
