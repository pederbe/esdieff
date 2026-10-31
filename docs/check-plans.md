# Repeatable catalogue checks

A [Check plan](../GLOSSARY.md#model-work) saves supplied files and named model
checks, sample checks and revision comparisons. Run the included model-only
example from the repository root.

```sh
cargo run -- check --plan examples/catalogue-check/plan.json
cargo run -- check --plan examples/catalogue-check/plan.json --format json
```

The [example plan](../examples/catalogue-check/plan.json) checks the whole
catalogue and then the selected Count model with Full baseline assessment.
Each case keeps its own requested scope, profile, warning policy and outcome.
Initial is the default assessment and still discloses broader partial RFC
coverage. A passed case establishes completion of its requested assessment.
Failed checking may also be complete. Missing input and unsupported meaning,
such as a Boolean `sdfRef`, remain explicit incomplete work.

The [mixed example](../examples/catalogue-check/mixed-plan.json) adds two samples
of Count to those same model checks.

```sh
cargo run -- check --plan examples/catalogue-check/mixed-plan.json
cargo run -- check --plan examples/catalogue-check/mixed-plan.json --format json
```

Count admits 25 and rejects 101. The example exits 1 because rejection is an
actual failed check. Naming a sample does not turn rejection into an expected
success or establish whole-model validity.

## Use your own model and samples

The [thermostat plan](../examples/thermostat-check/plan.json) is a runnable
template for a device property. Its [model](../examples/thermostat-check/thermostat.sdf.json)
describes a readable and writable target temperature from 5 through 30 degrees
Celsius. The supplied value `21.5` is admitted and `35` is rejected.

```sh
cargo run -- check --plan examples/thermostat-check/plan.json
cargo run -- check --plan examples/thermostat-check/plan.json --format json
```

Both model checks pass. All four cases are complete, with three passed cases and
one failed case. The run exits 1 because `35` exceeds the property's maximum.
The sample checks assess values against the property description. They do not
send commands to a thermostat or simulate property reads or writes.

The same directory also supplies minimal, rich and deliberately limited
thermostat variants with an offline Rust regression runner. See the
[device Catalogue](device-catalogue.md) for direct plans, named cases and the
device-and-constraint coverage matrix.

Copy the `examples/thermostat-check` directory to start your own checks, then
adapt the files together.

1. Replace `thermostat.sdf.json` with your model and update its source `path`
   in `plan.json` if you rename the file. Relative paths resolve beside the plan,
   so the directory can move as a unit.
2. Replace the sample files with one complete JSON value each. Use the value
   described by the selected property, such as `21.5`, rather than a device
   protocol's message envelope.
3. Set each sample case's `pointer` to the property in your model. The template
   selects `/sdfObject/Thermostat/sdfProperty/targetTemperature` with
   `kind: "property"`. Other selections are described under [Plan input](#plan-input).
4. Give cases descriptive names. If you rename source keys, update the model
   case's `roots` and each sample case's `document` and `sample` references.
   Add or remove sample sources with their corresponding cases.

Run the copied plan with `cargo run -- check --plan path/to/plan.json` from the
repository root. Check each case's outcome and completion separately. A sample
you intend to reject still produces a failed case and exit 1.

## Compare revisions in the same run

A `diff` case runs a [Revision comparison](revision-comparison-contract.md)
beside the plan's checks, so one command and one run report cover a release.
The [rich door lock plan](../examples/door-lock-check/rich-plan.json) checks the
rich model and compares it with the minimal one.

```sh
cargo run -- check --plan examples/door-lock-check/rich-plan.json
```

```json
{
  "name": "Minimal to rich revision",
  "operation": "diff",
  "earlier": {"document": "minimal.sdf.json"},
  "later": {"document": "rich.sdf.json"},
  "acknowledged": "acknowledged-differences.json"
}
```

`earlier` and `later` each map a source key to a literal file. Documents with
the same key correspond, and a key on only one side is a removed or added
document. Use each document's path within its revision's directory as its key,
so that an acknowledgement file written for `esdieff diff --old DIR --new DIR`
works unchanged. Two single files compared directly share the key `document`,
which is why the door lock plan uses it.

The comparison reads its documents only when the case runs, into two
Catalogues of its own, exactly as direct `esdieff diff` does. They never join
the plan's sources, so check and sample cases never see an earlier revision
and its definitions cannot make references ambiguous. The optional
`acknowledged` file holds the same
[Difference acknowledgements](revision-comparison-contract.md#acknowledged-differences)
that `esdieff diff --acknowledged` reads. The door lock file acknowledges the
added jammed state and the broader credential pattern, so the case passes while
its classification stays breaking.

## Plan input

Plans use schema `esdieff.check-plan` and string version `1`. Both `sources`
and `cases` are nonempty arrays. Every source has exactly `key`, `path` and
`role`. Keys are unique after JSON decoding. Role is `model` or `sample`, and
at least one source must be a model. Sample-role files are retained acquisition
attempts but contribute no definitions to model checks.

Each model case requires a unique `name`, `operation: "check"` and `scope`.
Use `{"kind":"catalogue"}` for all model sources, or
`{"kind":"selected","roots":["count"]}` for one or more explicit model keys.
Other supplied models remain dependency candidates. `assessment` accepts every
profile supported by direct `check`, defaulting to `initial-v1`.
`strict_warnings` is a Boolean, defaulting to false. Recorded registry knowledge
uses the native profile defaults.

Each sample case requires a unique `name`, `operation: "sample"`, `document`,
`kind`, `pointer` and `sample`. `document` names a model key and `sample` names a
sample key. `kind` is `property`, `data`, `action_input`, `action_output`,
`event_output` or `context`. Context selects a definition under an Object or
Thing. `pointer` is the effective-definition JSON Pointer, at most
8192 UTF-8 bytes, with ordinary `~0` and `~1` escapes. Action and Event pointers
select the declaration, and `kind` selects its input or output data description.
`strict_warnings` defaults to false. Sample cases use the existing Sample
inventory. There is no model assessment selector or expected-result field.
JSON null remains a supplied sample, and no default or sample value is inserted.

Each diff case requires a unique `name`, `operation: "diff"`, `earlier` and
`later`, and accepts an optional `acknowledged` path and a `perspective` of
`"client"` or `"implementer"`. Client is the default. Implementer comparison
assesses whether an existing implementation still satisfies the later model,
within the [comparison contract's supported rules](revision-comparison-contract.md#comparison-perspective).
Each side is an object of
1 through 256 members whose names are source keys of 1 through 256
UTF-8 bytes and whose values are nonempty paths. Duplicate decoded keys are
refused. `scope`, `assessment` and `strict_warnings` do not apply. The
comparison uses the native defaults, including the recorded registry evidence.
Diff documents are acquisition attempts of their case, separate from plan
`sources`, so naming a plan source's file there acquires it again. A plan still
needs a model source, so a comparison on its own belongs to direct
`esdieff diff`.

Paths are literal local files. Relative source, diff document and
acknowledgement paths use the parent of the CLI-supplied plan path made
absolute. A symlinked plan uses the symlink's parent,
not the target's directory. There is no globbing, variable expansion, discovery,
remote retrieval, plan inclusion or stdin substitution. Repeated references to
one key share one acquisition attempt. Different keys remain different attempts
even for the same path or bytes. Reading a plan changes no input file.

Plan mode permits `--format human|json` and the ordinary color controls.
Direct source arguments, roots, assessment and policy options are incompatible.
Directory discovery options are also incompatible. Use a separate direct `check`
with [directory discovery](directory-discovery.md) when source selection should
follow the current collection rather than the plan's saved literal paths.
Malformed or unreadable plans, duplicate decoded JSON members, unknown fields
or versions, invalid roles, aliases, profiles or pointer syntax, empty or
oversized diff sides, and acknowledgement files that cannot be read or decoded
are configuration errors with exit 2 on stderr and no run report.

A content-dependent selection mistake, such as a definitely absent description
or an established wrong class, becomes a named case request error. Later cases
continue. Acquisition/decode failures and unavailable needed meaning remain
native domain results, with the ordinary dependency rules and finite allowances.
A diff document that cannot be read is such a result too. Its source row records
the failed acquisition, and the comparison leaves the content it would have
held undetermined.

## Run output

JSON is plain under every color setting. Its separate schema is
`esdieff.check-run`, version `1.1`, with `plan`, `sources`, `cases` and `summary`.
Version `1.1` adds diff cases to `1.0`, and every run report is emitted at `1.1`.
`plan` records its acquisition path and accepted schema/version. Source rows use
the existing [report contract](report-contract.md) meanings and list the plan
sources only. Cases retain plan
order, with `name`, `operation`, `request`, `state: "report"`, native `outcome`,
`baseline` and `policy`. Sample cases also retain native `admission` as
`admitted`, `rejected` or `undetermined`. Diff cases also retain a mandatory
`classification`, the native `result.outcome` of `breaking`, `compatible` or
`undetermined`, which acknowledgements leave unchanged while the disposition in
`outcome` follows them. That `outcome` also counts the comparison's Version
warnings, which never change the case's disposition, and the native diff report
keeps its own version `1.7`. `request` is the native requested scope,
selector and assessment/inventory record, or for a diff case the comparison
identity and both revisions' scopes. A whole native `report` is optional. A diff
case's report is the native diff report with both revisions' sources. When it cannot
fit, `report_omission.reason` is `run-report-budget`. Native diagnostic truncation
inside a retained report is a separate presentation limitation. Neither form of
omission changes the case's outcome or completion.

A case with `state: "request_error"` has exactly `name`, `operation`, `request`,
`state` and `request_error`. Its request records the supplied native selector
and sample IDs and `strict_warnings`. `request_error` has stable operational
`code: "sample.request"`, an explanatory `message` and
`location: {"kind":"plan","pointer":"/cases/N"}` using the zero-based case
index. It has no native outcome, baseline, policy, admission, coverage or report.
Every source attempt remains visible even if every case is a request error.

`summary` counts `total`, `passed`, `failed`, `undetermined`, `request_errors`,
`complete_cases`, `incomplete_cases` and `omitted_reports`. Sample admission
counts `admitted`, `rejected` and `admission_undetermined` are independent of
disposition, and are zero in model-only runs. Counts describe cases, not combined
RFC obligations. A diff case counts by its native disposition after
acknowledgements, so an acknowledged breaking comparison is a passed case.
Request errors count as incomplete. Complete and incomplete
counts sum to total cases, and failed cases can be complete.
The aggregate `complete` flag requires every case to have a complete native result.
`verdict` follows invalid before failed before undetermined before passed, with
exits 2, 1, 3 and 0. Invalid describes request configuration, not SDF validity.
Host reporting or output failure exits 4 and makes no success claim for partial
or unavailable output. Human output lists the same named results and coverage,
with accessible findings and locations when detail fits. A diff case adds a
`Classification:` line and the native human diff report.

## Finite allowances

Plans are at most 1 MiB, with the ordinary native JSON depth, node, decoding and
allocation ceilings. There are at most 64 cases and 256 declared sources across
both roles. Keys and names contain 1 through 256 UTF-8 bytes. Resolved source
paths, plan identity, names and repeated references undergo a separate 1 MiB
control-metadata admission before copying. Diff keys, diff document paths and
acknowledgement paths count there as source keys and paths do. Ordinary
Catalogue input and metadata admission also applies.

The entire mandatory envelope is reserved before acquiring sources or running
cases. Its conservative reserve uses six bytes per control-metadata byte for
escaping, 4096 bytes per source and per diff document, 16 KiB per case for
native request, outcome, baseline/knowledge, policy and classification, and
16 KiB for the fixed run envelope and summary.
An envelope that cannot fit fails configuration admission. Optional detail uses
only the unreserved remainder of the 8 MiB complete-output ceiling.

Each acknowledgement file is read and fully decoded during admission, after the
envelope fits and before any source is acquired. A file is at most 1 MiB, and a
plan's files together are at most 4 MiB. A diff case runs on its own native
operation allowance, so its comparison spends nothing of the plan Catalogue's
work. Each diff document's control metadata covers what its revision's Catalogue
admits for it, so an admitted comparison always fits its native report
envelope.

The plan's native decoder retains its bounded original input and parsed view.
Control storage adds at most the admitted metadata, bounded case/source/document
slots and the acknowledgements decoded from at most 4 MiB of files. The
aggregate reserves one 8 MiB output buffer. Only one native case is
active at a time, with its ordinary processing and presentation allowances. A
diff case's two Catalogues are released when the case ends.
One native serialization is transient, with at most 8 MiB of content and a
separate 16 MiB capacity allowance for String growth. Human rendering starts
after that JSON serialization is released. The source envelope is appended
directly without another whole-source formatting buffer. Borrowed raw fields and whole
native JSON avoid a second full aggregation JSON graph or floating-point
conversion. No native work allowance is replenished within a case.

Selectors and sample aliases are included in control-metadata admission before
copying. The fixed case reserve also covers request-error messages up to 1024
bytes, tagged records and locations. One case record's DTO and encoding use a
separate 128 KiB staging allowance, covering the fixed fields and worst-case
escaping of the ordinary 8192-byte selector. Source-view inspection supplies
the native source metadata before cases start. It uses the same transient
serialization allowance and releases its report before any case executes.
