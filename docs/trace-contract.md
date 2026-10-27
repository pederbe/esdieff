# Recorded traces

Esdieff checks a Recorded trace, a JSON Lines file of Trace entries, against one model document, so device QA can find every logged value that a model rejects in one run. Each entry names a Sample selector, carries a value and may carry an Expected admission and a label. The document is composed once, each distinct selector is resolved once, and every entry is checked on its own, so one bad line never stops the run. The rules follow [ADR 0042: Recorded traces with independent entries](adr/0042-recorded-traces-with-independent-entries.md), and the terms Recorded trace, Trace entry, Expected admission, Sample selector and Sample-value checking come from [the domain language](../CONTEXT.md).

```rust
use esdieff::{Admission, Catalogue, TraceOutcome, SourceInput, TraceRequest};

let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied(
    "model",
    br#"{"sdfProperty":{"level":{"type":"integer","maximum":10}}}"#,
)])?;
let trace = concat!(
    r#"{"kind":"property","pointer":"/sdfProperty/level","value":7}"#, "\n",
    r#"{"kind":"property","pointer":"/sdfProperty/level","value":11,"label":"spike"}"#, "\n",
);
let request = TraceRequest::new(catalogue.source_id("model").unwrap(), trace.as_bytes());
let result = catalogue.check_trace(request)?;
let spike = result.entries().next().unwrap();
assert_eq!((spike.line(), spike.label()), (2, Some("spike")));
assert_eq!(spike.admission(), Some(Admission::Rejected));
assert_eq!(result.count(TraceOutcome::Passed), 1);
# Ok::<(), &'static str>(())
```

The trace contract identity is `esdieff.trace/1`. It binds the Trace entry format and the rules that settle each entry's outcome, so a release that changes either gets a new identity. A Recorded trace adds no sequence meaning, timing or protocol binding. Line order is presentation only, and the checks add nothing to any Model requirement inventory.

## Command line

`esdieff trace --document MODEL --trace FILE [--format human|json] MODEL_INPUT...` checks the Recorded trace FILE against the model document MODEL, which must match one explicit MODEL_INPUT. Inputs are file paths, and `-` reads one document from standard input. FILE is a file path, or `-` for standard input, and standard input may be used only once across the model inputs and the trace. From the repository root, check a trace of thermostat readings:

```sh
cargo run -- trace --document examples/thermostat-check/rich.sdf.json --trace readings.jsonl examples/thermostat-check/rich.sdf.json
```

`--format human` gives the human report and `json` the [JSON report](#report-15). The shared color controls style the human report. The run exits 1 when any entry failed, otherwise 3 when any entry is undetermined, and otherwise 0, as `check` does. A missing `--document` or `--trace`, an unknown format or option, a document that is not a supplied input, a second `-`, a trace file that is missing, unreadable or larger than 64 MiB, a line over 1 MiB and more than 100,000 entries are invocation errors that exit 2 with empty stdout. The trace is read before any model input. A report that cannot be written exits 4. The [CLI contract](report-contract.md#cli-contract) states the shared rules.

## Library operation

`Catalogue::check_trace(request)` checks one Recorded trace. `TraceRequest::new(document, trace)` names the model document and the trace bytes. The trace becomes a source with role `sample` and the key `trace`, numbered after the Catalogue's sources. `key(key)` gives it another caller key, `label(label)` a display label, such as its path, and `omit_label()` records that its label was left out, as for a path too long to show. An unknown or non-model document, a trace over `TraceRequest::FILE_BYTES`, a line over 1 MiB, more than 100,000 entries, a key over 256 bytes or equal to a Catalogue key, a label over 512 bytes, a Catalogue that already holds the most source attempts and a report allowance too small for the mandatory report envelope are request errors. A malformed entry is never a request error.

`TraceResult` owns every listed entry and borrows the Catalogue for its report. `count(outcome)` counts the entries that settled with that `TraceOutcome`, and `admissions(admission)` counts the checked entries that Sample-value checking settled with that admission. `entries()` yields the failed and undetermined entries in line order while they fit the report byte limit, and `entries_omitted()` counts those that did not. Passing entries are counted, not listed. `disposition()` is failed for any failed entry, otherwise undetermined for any undetermined entry, otherwise passed. `document()` echoes the request and `trace_source()` gives the trace source's identity in the report. `to_json()`, `to_human()` and `to_human_with_style(style)` render the outputs below.

Each `TraceEntry` has its `line()`, counting from 1, its `label()`, its `selector()` when it names a valid kind and a pointer string of at most 8192 bytes, whether or not that selects anything, its `expected()` admission, its `admission()` when its value was checked, its `outcome()` and its `conditions()`, at most eight, with `conditions_omitted()` counting the rest. Each `TraceCondition` has its `code()`, `severity()`, `message()`, its `pointer()` within the entry's JSON map, such as `/value/mode`, and its `span()`, the byte range in the trace file.

## Trace entries

The trace is split into lines at each LF, and a CR before an LF is ignored. The last line may omit its LF, and an empty last line is no entry, so a file holding one empty line has no entries. Every other line, including an empty one, is one Trace entry, and lines are numbered from 1 in file order.

Each entry is one JSON map with these members:

| Member | Required | Value |
| --- | --- | --- |
| `kind` | yes | `property`, `data`, `action_input`, `action_output` or `event_output`, as for `esdieff sample` |
| `pointer` | yes | the effective definition pointer of the Sample selector, as for `esdieff sample` |
| `value` | yes | any JSON value, checked as a Sample value for the selected description |
| `expected` | no | the Expected admission, `admitted` or `rejected` |
| `label` | no | a string of at most 256 bytes that names the entry in the report |

Each line is decoded on its own under the ordinary JSON input policies of [ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md). Numbers keep their exact values, so `9007199254740993` stays distinct from `9007199254740992`, and a repeated member name anywhere in the line is refused, including inside the value. Each line has the decoding allowances one Sample source would have in the same Catalogue, so its depth, node count and number sizes have the shared limits.

## Entry outcomes

Each entry settles on its own, in this order:

1. A line that is not valid JSON, or repeats a member name, fails with its `json.syntax` or `input.duplicate_name` condition. A line that is not UTF-8 or exceeds a decoding limit cannot be read and is undetermined.
2. A line that is not a JSON map fails with `trace.entry`. An unknown member fails with `trace.unknown_member`, a missing required member with `trace.missing_member` and a member with an invalid value with `trace.invalid_member`. Every such member is reported.
3. A selector that selects nothing fails with `trace.unselectable`. That covers a malformed or oversized pointer, a definition that does not exist, and a definition of another kind, such as an Event pointer with kind `property`. A description whose meaning cannot be established, such as one behind an unavailable reference, leaves the admission undetermined with `trace.unavailable`.
4. Otherwise Sample-value checking settles the value against the selected description. The entry passes when the value is admitted and nothing else was expected, or when its definite admission equals `expected`. It fails when the value is rejected and nothing was expected, or when its definite admission differs from `expected`, which adds a `trace.expected` condition. An undetermined admission leaves the entry undetermined whatever was expected.

The conditions of a failed or undetermined entry explain its outcome: the decoding issue, the member conditions, `trace.expected`, and the conditions behind a rejection or an undetermined admission, such as `sample.bound`, in that order.

## Composition and allowances

The selected document is composed once, on the request's own allowance, as `esdieff generate` composes it. Each distinct selector, a kind and a pointer, is resolved once against that composition, and later entries reuse its resolution. Each entry is checked with an allowance of its own, with the work, allocation, numeric and equality limits that one `esdieff sample` request receives against the same Catalogue, as [ADR 0042: Recorded traces with independent entries](adr/0042-recorded-traces-with-independent-entries.md) states. A check that runs out of its allowance leaves only that entry undetermined, with an `operation.limit` condition. Entries take turns with one pattern engine rather than starting their own, as [ADR 0045: One pattern engine per trace or generation request](adr/0045-one-pattern-engine-per-trace-or-generation-request.md) states. Each entry still has its own pattern polls and duration. About once every 1,429 pattern matches, the entry being checked spends one more poll than it would alone, so an entry with no poll to spare can be undetermined in a trace while it settles on its own. An entry never settles in a trace when it would not on its own.

## Bounds

A Recorded trace holds at most 64 MiB, 100,000 entries and 1 MiB per line, apart from its line ending, under [ADR 0009: Bounded processing and reporting](adr/0009-bounded-processing-and-reporting.md). Going beyond any of them is a request error before any entry is checked, so a run never reports a partial file.

The report fits the report byte limit, `Limits::report_bytes`. After the Catalogue's mandatory report reserve and the trace envelope's own, each failed or undetermined entry is charged the bytes of its larger rendering, as a JSON report record or human report lines, in line order. The first entry that does not fit, and every failed or undetermined entry after it, is counted in `entries_omitted` with a `report_limit` record and is not listed. Counts, the outcome and the disposition always cover every entry.

## Report 1.5

The JSON report is `schema:"esdieff.report", version:"1.5", operation:"trace"`, extending the [shared report contract](report-contract.md#recorded-trace-report-15) with these fields:

| Object | Required fields and types | Meaning |
| --- | --- | --- |
| Trace request | assessment:null; inventory:null; trace:Trace; scope:Scope | Trace names its own contract. Scope is `selected` with the document as its one root, the other model sources its composition used as dependencies, both as assessed, and the needed sources it could not use as unavailable. |
| Trace | contract:`"esdieff.trace/1"`; document:Id; source:Id | source is the trace source. |
| Trace source | Source fields | The trace is the last source, with role `sample`, acquisition `supplied`, decode `partial` when some line could not be decoded completely and otherwise `usable`, and retained_bytes the trace's length. |
| Trace outcome | Outcome fields | `errors` counts failed entries and `incomplete_checks` undetermined entries. `warnings` and `notices` are 0. complete is true when no entry is undetermined. The disposition is failed for any failed entry, otherwise undetermined for any undetermined entry, otherwise passed. |
| Trace requested coverage | inventory:null; count_unit:`"trace_entry"`; total:Count; assessed:Count; not_applicable:0; unassessed:0; undetermined:Count | One unit per entry. Passed and failed entries are assessed, and undetermined entries undetermined. |
| TraceResult | kind:`"trace"`; summary:TraceSummary; entries:TraceEntry[]; entries_omitted:Count; omissions:Omission[] | entries lists failed and undetermined entries in line order. omissions has a `report_limit` record for entries that did not fit, which also appears in `diagnostics.omissions` and makes the report truncated, and a `count_limit` record for conditions beyond each entry's eight. |
| TraceSummary | passed:Count; failed:Count; undetermined:Count; admissions:{admitted:Count, rejected:Count, undetermined:Count} | Counts every entry, listed or not. admissions counts the entries whose value was checked or whose selected description was unavailable. |
| TraceEntry | line:Count; label:string|null; selector:SampleSelector|null; expected:`"admitted"|"rejected"`|null; admission:`"admitted"|"rejected"|"undetermined"`|null; outcome:`"failed"|"undetermined"`; conditions:Condition[]; conditions_omitted:Count | selector is present when the entry names a valid kind and a pointer string of at most 8192 bytes, whether or not that selects anything, and null otherwise. admission is null when the value was not checked. |
| Condition for trace | Shared Condition fields plus domain:`"sample"|"operation"` | primary is a SampleLocation of the trace source whose pointer is within the entry's JSON map, such as `/value`, and whose span is the byte range in the trace file, so it selects the right bytes even with CR LF line endings. A decoding issue has a null pointer. A condition from Sample-value checking relates the constraint's effective location and its nearest source location, and names the constraint in `arguments.constraint`. Member conditions name a missing or invalid member in `arguments.member`, and `trace.unavailable` gives its reason in `arguments.reason`. Conditions the trace contract establishes have basis kind `operational` with id `esdieff.trace/1`. |

`baseline.interpretations` names the value interpretations that the checks relied on, as model reports name them. Every source stays in `sources`. `diagnostics.items` is empty, since every condition belongs to an entry. The human report shows the disposition, the entry and admission counts, then each listed entry with its line, selector and outcome, its label, its admission and Expected admission, and one line per condition with its severity, code, pointer and byte range. It gives the JSON report instead if it would not fit the report byte limit.

## Replaying Generated values

`esdieff generate --format jsonl` writes one Trace entry per admitted or rejected Generated value, with its settled admission as `expected` and its Generation target as `label`, as the [generation contract](generation-contract.md#json-lines) describes. So its output is a Recorded trace. Checked against the same model, every entry passes. Checked against a later revision, exactly the entries whose admission changed fail, so last release's values become regression vectors for the next.

```sh
cargo run -- generate --document examples/thermostat-check/rich.sdf.json --format jsonl examples/thermostat-check/rich.sdf.json > vectors.jsonl
cargo run -- trace --document examples/thermostat-check/rich.sdf.json --trace vectors.jsonl examples/thermostat-check/rich.sdf.json
```

Generation leaves undetermined values out, so a replay never expects an admission that checking did not establish. It also leaves out any value whose Trace entry would be longer than the 1 MiB line limit, so every line it writes replays. The selectors keep their pointers, so a revision that renames or removes a definition fails its entries with `trace.unselectable`.
