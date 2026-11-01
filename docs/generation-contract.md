# Generated values

Esdieff generates sample values for a selected effective data description, so firmware and app teams can test their code against a model without writing payloads by hand. Each Generated value aims at one Generation target, such as a minimum, a value just below it or a wrong type, and Sample-value checking settles it against the description it was generated for. Its admission, not its target, says whether the description admits it. The rules follow [ADR 0041: Generated values settled by Sample-value checking](adr/0041-generated-values-settled-by-sample-value-checking.md), and the terms Generated value, Generation target, Representative value, Sample-value checking, Effective data description and Nullable come from [the domain language](../GLOSSARY.md).

```rust
use esdieff::{Admission, Catalogue, GenerationRequest, SampleKind, SourceInput};

let catalogue = Catalogue::take_inputs(vec![SourceInput::supplied(
    "model",
    br#"{"sdfProperty":{"level":{"type":"integer","minimum":10}}}"#,
)])?;
let request = GenerationRequest::new(catalogue.source_id("model").unwrap())
    .select(SampleKind::Property, "/sdfProperty/level");
let result = catalogue.generate(request)?;
let level = result.descriptions().next().unwrap();
let below = level.values().find(|value| value.target() == "below_minimum").unwrap();
assert_eq!((below.to_json(), below.admission()), ("9", Admission::Rejected));
# Ok::<(), &'static str>(())
```

The generation contract identity is `esdieff.generation/2`. It binds the Generation targets, their order and the rules that build their values, so a release that changes any of them gets a new identity and a report keeps its meaning. Candidate construction is deterministic, with no randomness or seed. Identical inputs and settings give the same values in the same order when the same work completes. Pattern matching retains its timing-sensitive limits, so admissions and completed output can vary when a limit is reached.

## Command line

`esdieff generate --document MODEL [--kind KIND --pointer POINTER] [--format human|json|jsonl] MODEL_INPUT...` generates values for descriptions of the model document MODEL, which must match one explicit MODEL_INPUT. Inputs are file paths, and `-` reads one document from standard input. `--kind` and `--pointer` select one description as `esdieff sample` does, with KIND one of `property`, `data`, `action_input`, `action_output`, `event_output` and `context`, and they come together or not at all. Without them every affordance description of the document is selected, as described under [Selection](#selection). From the repository root, generate values for the rich thermostat model:

```sh
cargo run -- generate --document examples/thermostat-check/rich.sdf.json examples/thermostat-check/rich.sdf.json
```

`--format human` gives the human report, `json` the [JSON report](#report-15) and `jsonl` the [JSON Lines Trace entries](#json-lines). The shared color controls style the human report. Generation reports no model findings, so it exits 0 when generation completed for every selected description and 3 otherwise, including when any value stays undetermined. A missing `--document`, `--kind` without `--pointer` or the reverse, an unknown kind, format or option, a selector that selects nothing, a document that is not a supplied input and a second `-` are invocation errors that exit 2 with empty stdout. A report that cannot be written exits 4. The [CLI contract](report-contract.md#cli-contract) states the shared rules.

## Library operation

`Catalogue::generate(request)` performs one generation. `GenerationRequest::new(document)` selects every affordance description of a model document, and `select(kind, pointer)` narrows it to the one description that Sample selector selects. An unknown or non-model document, a malformed or oversized pointer, a selector whose definition is absent or has another kind, and a report allowance too small for the mandatory report envelope are request errors.

`GenerationResult` owns every value and borrows the Catalogue for its report. `descriptions()` yields each selected `GeneratedDescription` in selection order, and `descriptions_omitted()` counts selected descriptions beyond the [ceiling](#bounds) or left out because their details did not fit the report byte limit. `count(admission)` counts the kept values Sample-value checking settled with that admission. `selection_complete()` is false when a grouping or affordance of the document could not be read, so some descriptions may be missing. `complete()` is true when the selection is complete, no description was omitted and every description is complete. `disposition()` is passed then and undetermined otherwise, never failed. `document()` and `selector()` echo the request. `to_json()`, `to_json_lines()`, `to_human()` and `to_human_with_style(style)` render the outputs below.

Each `GeneratedDescription` has its Sample selector `kind()` and `pointer()`, its `state()`, its kept `values()` in target order and `values_omitted()`, the distinct values left out beyond the ceiling, because they did not fit the report byte limit or because their Trace entry would be longer than a trace line. Its state is:

- `Complete` when every applicable target was generated and every kept value is admitted or rejected.
- `Partial` when generation ran out of the description's allowance, values were omitted beyond the ceiling, for the report byte limit or for the trace line limit, or Sample-value checking left a value undetermined, including one whose check ran out of its own allowance.
- `Unavailable` when the description's effective meaning could not be established, so no value was generated.

Each `GeneratedValue` has its `target()`, its `to_json()` text, in which each number keeps the spelling it was generated with, its `admission()` and its `codes()`, the distinct condition codes behind a rejection, at most 8, such as `sample.bound`.

## Corpus evidence

Use corpus evidence to see what the retained candidates establish before using
them as test cases. `GeneratedValue::evidence()` combines the value's construction
aim with its settled admission. `GeneratedDescription::count_evidence(evidence)`
counts the kept values in that category. The five categories are disjoint:

| Evidence | What it establishes |
| --- | --- |
| `admitted_example` | An admitted example candidate, or an outside candidate that the whole description nevertheless admits. |
| `admitted_boundary` | A candidate for `minimum`, `maximum`, `min_length`, `max_length`, `min_items` or `max_items` that the whole description admits, including nested targets. |
| `rejected_outside` | A candidate for one of the third group's outside targets that the whole description rejects. It may fail several constraints, so it does not establish an isolated cause of rejection. |
| `rejected_candidate` | A rejected example or boundary candidate. It establishes no admitted example or admitted boundary. |
| `undetermined` | A candidate whose admission could not be settled. |

For an integer with `minimum:10` and `maximum:20`, `10` and `20` establish
admitted boundary candidates, while `9` and `21` establish rejected outside
candidates. For `{"type":"string","nullable":false,"pattern":"^(foo|bar)$","minLength":3,"maxLength":3}`,
the length-boundary candidate `"aaa"` is rejected with `sample.pattern`.
It contributes to `rejected_candidate`, never `admitted_boundary`. This pattern
is outside the synthesis subset, so the report can show complete generation
with no admitted example even though `"foo"` and `"bar"` would be admitted.
The human report calls out the absence of retained admitted examples without
claiming that the description admits no value.

For a map with a required `mode` entry whose description is `{"const":"on"}`,
the `missing_entry` candidate `{}` establishes a rejected outside case with
`sample.required`. A candidate rejected for another constraint alone is still
only a whole-description rejection. Corpus evidence does not attribute causes
or prove that the omitted entry was the sole reason for rejection.

Counts describe retained values under their winning targets after
[deduplication](#settlement-and-duplicates). They exclude omitted values and
discarded target labels. Two Choice alternatives with `const:1` give one
retained candidate, and admission checks the whole Choice. Even distinct
admitted `alternative` candidates do not prove admission by their originating
branches. There is no per-alternative or per-constraint denominator, inventory,
coverage percentage or new search for examples. Constraints without generation
targets, such as `pattern`, have no dedicated evidence category.

This is separate from the report's requested description coverage and operation
completeness. Complete generation can have no admitted boundary candidates.
Partial generation can retain useful admitted examples. An unavailable
description has zero evidence counts because it has no values, not because its
constraints were exercised. Check description state and omissions alongside
these counts.

The human report lists the category counts per description and annotates each
value with its evidence. JSON report 1.5 adds optional `evidence` fields: a
category string on each GeneratedValue and an object of five category counts on
each GeneratedDescription. Older reports without them do not establish zero
evidence. These additions leave the generation contract, admission, completeness,
exit codes and JSON Lines Trace schema unchanged. The extra report detail is
charged to the existing byte limit and can leave less room for retained values.

## Choice admission evidence

Choice admission evidence explains which named `sdfChoice` alternatives supplied
an established admission witness for retained Generated values. It is separate
from Corpus evidence, which still counts values in its five categories. It does
not measure RFC assessment coverage, generation completeness or a percentage of
alternatives exercised.

The checker stops at the first admitting Effective alternative. For
`{"nullable":false,"sdfChoice":{"low":{"const":1},"high":{"const":9}}}`,
retained values `1` and `9` witness `low` and `high`. For overlapping alternatives
that both construct `0`, only the first admitting alternative is witnessed.
Reversing alternative order can reverse this observation without changing
admission. `no_witness_established` includes skipped alternatives and establishes
neither rejection nor impossibility. Documentary names are never constant values.
Enum occurrences retain their identities and get no invented names or rows.

Rows follow model order, including nested Choices in described properties,
array items and final alternative contexts. Const/default value maps and
documentary content are not descriptions. Each row identifies its owning Choice
by semantic steps relative to the selected effective description. A `property`
step names a described map entry, `items` denotes its item context, `choice`
names an ancestor alternative, and `enum` records an ancestor occurrence index.
These are semantic coordinates, not pointer-selectable authored members. The
containing description supplies the effective document and description identity.
Inherited nested Choices remain distinct under their ancestor alternatives.

A `witnessed` row references the zero-based `value_index` in the final retained
values and an ordinary `sample_pointer` within that value. A root occurrence has
an empty pointer. Array occurrences use actual indices. The first retained value
in final deduplicated order wins, then its first occurrence in checking order.
Moving a duplicate to its winning target preserves its already established proof.
Rejected or undetermined whole values contribute nothing. Failed outer branches
discard their nested observations, even when a local nested Choice admitted.
Shared qualities, references and overrides follow existing Effective alternative
meaning. Collection neither generates more candidates nor checks skipped branches.

Each selected description has one separate evidence allowance initialized from
its fixed internal work and effective-allocation ceilings. Enumeration, proof
bookkeeping and retained observations share it without refill. Existing depth
and effective-model bounds also apply. Exhaustion stops optional retention and
leaves evidence `partial`, without spending checking allowances or changing
admission, retained candidates, generation completeness, disposition or exit codes.
`complete` means enumeration and observation retention finished within scope,
even when rows have no witnesses. `unavailable` means the selected description
could not be meaningfully inspected. Unreadable structure that might hide nested
Choices leaves evidence partial. Known omitted rows are counted separately.
Unknown membership supplies no total or coverage denominator.

`GeneratedDescription::choice_evidence()` borrows the typed collection, which
retains its state independently of presentation. JSON report 1.5 optionally adds
`choice_evidence` with `state`, ordered `alternatives` and
`alternatives_omitted`. Each row has `owner`, `alternative`, `status` and an
optional `witness` with `value_index` and `sample_pointer`. Human output lists
the same statuses and explains skipped overlapping alternatives. These optional
sections use capacity remaining after all ordinary generation results fit.
A shortened rendering reports partial evidence and known omitted rows. If no
container fits, the field or section is absent. Absence, including in older
reports, means evidence is unavailable in that rendering, not zero witnesses.
All output remains within the existing report byte ceiling. JSON Lines Trace
schema and replay are unchanged.

## Selection

With a selector, generation covers the one description that Sample-value checking would select for that `kind` and `pointer`, including an sdfData definition or Context under an Object or Thing. Without one, it covers every Property, Action `sdfInputData`, Action `sdfOutputData` and Event `sdfOutputData` reachable from the document's top-level groupings and affordances, including nested groupings. They are taken in document order, and an Action's input comes before its output. sdfData and Context definitions are generated only when selected explicitly, so adding metadata does not add vectors to an existing affordance corpus. Event input and groupings have no description to generate. A description whose effective meaning is unavailable is listed as unavailable, and a grouping, Action or Event that cannot be read leaves the selection partial.

`SampleKind::Context` and CLI kind `context` use the final inline, inherited or referenced description and the [Context defaults policy](context-contract.md#semantic-boundaries). A generated null with absent `nullable` stays undetermined and leaves generation partial. Explicit nullability is honored. Supported nonnull values and independent rejections can still settle. General link meaning remains unavailable, and no runtime instance-information message is synthesized. Follow the [offline Context round trip](context-contract.md#generate-and-replay-context-vectors) for a concrete example.

## Generation targets

Each description's targets come in four groups, in this order. A target that does not apply produces no value, and so does one whose value would exceed the value capabilities, such as a string longer than a document may be or a value nested deeper than the depth limit. `pattern` has no separate target. The string `type` representative uses the bounded synthesis below, including when it fills a required sibling of a nested target.

### Pattern-aware representatives

Without a conventional checked format value, a string representative first tries to satisfy its effective `pattern`, `minLength` and `maxLength` together. For example, `{"type":"string","pattern":"^ID-[0-9]+$","minLength":5,"maxLength":7}` gives `"ID-00"`. This same representative fills required entries of maps and supplies candidate revision-comparison witnesses. Sample-value checking still settles every candidate against all applicable constraints.

The supported synthesis subset is ASCII literals and concatenation, optional leading `^` and trailing `$`, positive character classes with ASCII ranges, `\d`, `\w`, `\s`, escaped regular-expression punctuation, and `\n`, `\r`, `\t`, `\f`, `\v`. Atoms can use `?`, `*`, `+`, `{n}`, `{n,m}` or `{n,}`, including lazy forms. Inside a class, escapes can contribute characters but a range needs single-character endpoints. Groups, alternation, negated classes, wildcard `.`, lookaround, backreferences, Unicode literals and Unicode escapes are outside this subset. This synthesis boundary does not restrict the patterns Sample-value checking can match.

Each atom chooses its lowest ASCII character, using `0` for `\d` and `\w`, and space for `\s`. Generation starts with each atom's minimum repetition and increases repetitions from left to right until `minLength` is met. If repetitions cannot reach that length, an unanchored pattern permits `a` padding after its matched text, or before it when `$` is present. A fully anchored pattern permits no padding. A candidate longer than `maxLength` or the document byte ceiling is not built. Without `maxLength`, the document byte ceiling bounds open repetitions. Supplied counts must fit the generator's count capabilities.

Unsupported syntax, unreadable `maxLength`, impossible lengths or overflowing repetition counts keep the ordinary `minLength` copies of `a` fallback. An unreadable or unsupported `minLength` prevents even that fallback. No satisfying candidate is promised in those cases, and failure to synthesize is not a proof that the description admits no string. Conventional format values retain precedence, and const, enum and default candidates retain their existing order. Boundary and outside length targets still use copies of `a`, so another constraint can reject them. Exhausted synthesis work or allocation leaves the description partial rather than silently replenishing its allowance. The parser storage, parse work and constructed text are charged to that description's existing allowance before they are used.

This is contract `esdieff.generation/2`, superseding `/1` for the changed string representative rules. The target names, order, report version and trace format stay the same. [ADR 0051: Bounded pattern-aware representatives](adr/0051-bounded-pattern-aware-representatives.md) records the decision.

### Target order

The first group is the [Representative values](revision-comparison-contract.md#representative-values) of ADR 0039, with their number, string, map and array rules:

| Target | Applies when | Value | Example |
| --- | --- | --- | --- |
| `const` | the description has a `const` | a copy of it, numbers in their authored spelling | `{"const":1e1}` gives `1e1` |
| `enum` | the description has an enum of strings | each enum value, in order | `{"enum":["off","heat"]}` gives `"off"` and `"heat"` |
| `default` | the description has a documentary `default` | a copy of it | `{"type":"number","default":21}` gives `21` |
| `type` | always, apart from a choice | one value of each base type the description admits | `{"type":"string"}` gives `""`, `{"type":"string","format":"date"}` gives the conventional `"1970-01-01"`, `{"type":"object","required":["mode"],"properties":{"mode":{"enum":["off"]},"note":{"type":"string"}}}` gives `{"mode":"off"}`, and a description without `type` gives `0`, `""`, `true`, `{}` and `[]` |
| `alternative` | the description has a choice | the first Representative value of each effective alternative | `{"sdfChoice":{"low":{"const":1},"high":{"const":9}}}` gives `1` and `9` |
| `null` | every Full data description | `null`, which settlement shows to be admitted only when the description is Nullable | `{"type":"number","nullable":false}` gives a rejected `null` |

The second group holds boundary values, which lie on an edge of the admitted range:

| Target | Applies when | Value | Example |
| --- | --- | --- | --- |
| `minimum` | `minimum` or `exclusiveMinimum` | the tighter lower bound when it is inclusive, in its authored spelling, or one unit inside it when it is exclusive | `{"minimum":10}` gives `10`, and `{"exclusiveMinimum":0}` gives `1` |
| `maximum` | `maximum` or `exclusiveMaximum` | the same at the upper bound | `{"exclusiveMaximum":2.5}` gives `2.4` |
| `min_length` | `minLength` | `minLength` copies of `a` | `{"minLength":2}` gives `"aa"` |
| `max_length` | `maxLength` | `maxLength` copies of `a` | `{"maxLength":4}` gives `"aaaa"` |
| `min_items` | `minItems` | an array of `minItems` [items](#array-items) | `{"type":"array","minItems":2,"items":{"type":"integer","minimum":0}}` gives `[0,0]`, or `[0,1]` beside `"uniqueItems":true` |
| `max_items` | `maxItems` | an array of `maxItems` items | `{"type":"array","maxItems":4,"uniqueItems":true,"items":{"type":"integer","minimum":0}}` gives `[0,1,2,3]` |
| `all_entries` | `properties` with at least one entry | a map holding every described entry in order, then every other required entry, each filled with the first Representative value of its description, or `null` for a required entry without one | `{"type":"object","properties":{"mode":{"enum":["off"]},"note":{"type":"string"}}}` gives `{"mode":"off","note":""}` |

The third group holds values just outside the admitted range:

| Target | Applies when | Value | Example |
| --- | --- | --- | --- |
| `below_minimum` | `minimum` or `exclusiveMinimum` | the tighter lower bound when it is exclusive, or one unit below it when it is inclusive | `{"minimum":10}` gives `9`, and `{"exclusiveMinimum":0}` gives `0` |
| `above_maximum` | `maximum` or `exclusiveMaximum` | the same at the upper bound | `{"exclusiveMaximum":2.5}` gives `2.5`, and `{"maximum":2.5}` gives `2.6` |
| `not_multiple` | a positive `multipleOf` | half a step above an inclusive minimum that is a multiple of the step, otherwise half a step above 0 | `{"minimum":5,"multipleOf":0.5}` gives `5.25`, and `{"multipleOf":3}` gives `1.5` |
| `too_short` | a `minLength` above 0 | one `a` fewer than `minLength` | `{"minLength":2}` gives `"a"` |
| `too_long` | `maxLength` | one `a` more than `maxLength` | `{"maxLength":4}` gives `"aaaaa"` |
| `too_few_items` | a `minItems` above 0 | an array of one item fewer than `minItems` | `{"type":"array","minItems":2,"uniqueItems":true,"items":{"type":"integer","minimum":0}}` gives `[0]` |
| `too_many_items` | `maxItems` | an array of one item more than `maxItems` | `{"type":"array","maxItems":4,"uniqueItems":true,"items":{"type":"integer","minimum":0}}` gives `[0,1,2,3,4]` |
| `duplicate_items` | `uniqueItems` is `true` | an array repeating the first item, twice or as often as `minItems` requires | `{"type":"array","uniqueItems":true,"items":{"type":"integer","minimum":0}}` gives `[0,0]` |
| `missing_entry` | `required` | one value for each required entry, in order: a map holding every other required entry, filled as for `all_entries`, and lacking that one | `{"type":"object","required":["mode","level"],...}` gives `{"level":5}` and `{"mode":"off"}` when `mode` and `level` are an enum starting with `"off"` and an integer with a minimum of `5` |
| `wrong_type` | a `type` that is one of the base types | one value of each base type it does not admit, from `0`, `""`, `true`, `{}` and `[]`, where integer data admits `0` | `{"type":"integer"}` gives `""`, `true`, `{}` and `[]` |

### Array items

The items of an array value are copies of the first Representative value of its `items` description, or `null` without one. When the array requires unique items and holds two or more, its items are distinct instead: the Representative values of `items` in order, then further values that continue from the first of them. Numbers continue one unit at a time, strings vary their last character through `a` to `z` and then longer suffixes, and a boolean continues with the other boolean. Without an `items` description the items are the numbers from `0`. When no such sequence has enough distinct items, such as for an `items` description with only a `const`, the target gives no value.

### Nested targets

The fourth group holds the targets of each `properties` entry, in order, and then of `items`. Each is generated by these same rules for that nested description, including its own nested targets, and its value is placed where the nested description stands:

- inside the map its entry belongs to, beside the first Representative value of every other entry that map requires, so the enclosing constraints still apply
- among the items of its array, repeated as often as the array's `minItems` requires and at least once, or beside other distinct items when the array requires unique items

A nested target is named after the path to it, as `<entry>.<target>` or `items.<target>`, so a minimum inside the `level` entry of `settings` data is `level.minimum`, and one inside the items of an array inside the `readings` entry is `readings.items.minimum`. With `{"type":"object","required":["mode","level"],"properties":{"mode":{"enum":["off","heat"]},"level":{"type":"integer","minimum":5}}}`, `level.minimum` is `{"level":5,"mode":"off"}` and `level.below_minimum` is `{"level":4,"mode":"off"}`. A nested value is often the same value as an earlier one, such as the map of Representative values, and is then kept once under the [duplicate rules](#settlement-and-duplicates), which favor the nested boundary target. With `{"type":"array","minItems":2,"uniqueItems":true,"items":{"type":"integer","minimum":3,"maximum":9}}`, `items.maximum` is `[9,3]`. A Direct items description has no Nullable characteristic, so `items` never has a `null` target. Nested descriptions are followed only while their values can nest within the depth limit, and a value whose enclosing value cannot be built, such as one whose sibling entry has no Representative value, is not generated.

A unit follows the rules of the [numeric bounds](revision-comparison-contract.md#numeric-bounds) witnesses: it is 1 where the coefficient digit limit allows, and otherwise one of the bound's last significant digit. So a maximum of `20.50` gives an `above_maximum` of `20.6`, and a maximum of `1e600` gives `2e600`, since adding 1 would need more digits than the limit allows. A count that is not an exact nonnegative integer within the value capabilities, and a step that is not a positive exact number, give no target.

## Settlement and duplicates

Sample-value checking settles every kept value against the selected description, as it would a value supplied for that Property, sdfData definition, input, output or Context. A target name never overrides admission. A `below_minimum` value that another constraint also rejects is just rejected, and a `type` value that a `pattern` rejects is rejected too. A value the checker cannot settle, such as one beside an unsupported pattern or one needing authored `properties` or `items` without established applicable structure, is undetermined and leaves its description partial. An absent `type` alone does not prevent admission.

Each value is settled as soon as it is kept, so the admissions already settled stand when later generation runs out of the description's allowance. Two targets can produce the same value, and it is then kept once. Values are the same when their JSON texts are, apart from the order of the entries in any map, so `{"level":5,"mode":"off"}` and `{"mode":"off","level":5}` are one value, while numbers compare by their exact spelling, so `10` and `1e1` are two. A duplicate is kept under the target with the highest precedence, so that a label names the constraint a value probes:

1. Boundary and outside targets, every target of the second and third groups apart from `all_entries`, rank above the Representative-value targets `const`, `enum`, `default`, `type`, `alternative` and `null`, and above `all_entries`.
2. Within each of these two classes, a nested target, `<entry>.<target>` or `items.<target>`, ranks above a top-level one.
3. Between targets of equal rank, the first in contract order keeps the value.

The value stands at the place of the target that keeps it and shows that target's text. So a typed description with a minimum of `10` lists `10` as its `minimum` rather than as its `type`, a `const` of `3` beside a minimum of `3` lists `3` as the `minimum`, a `const` of `0` beside a `type` of `integer` lists `0` as the `const`, and an array of at least two unique integer items with a minimum of `0` lists `[0,1]` as `items.minimum` rather than as `min_items`. In a map that requires a `mode` enum starting with `"off"` and a `level` with a minimum of `5`, the `type` map `{"mode":"off","level":5}`, the first `mode.enum` value and the `level.minimum` value are one value, listed once as `level.minimum` with the text `{"level":5,"mode":"off"}`.

## Bounds

One description keeps at most 64 distinct values, and one request selects at most 1024 descriptions. Distinct values beyond the first ceiling are counted in `values_omitted` and leave their description partial. Descriptions beyond the second are counted in `descriptions_omitted`, have no values and leave the operation undetermined. These two ceilings keep a request finite under [ADR 0009: Bounded processing and reporting](adr/0009-bounded-processing-and-reporting.md) and [ADR 0013: Shared workflow and report guarantees](adr/0013-shared-workflow-and-report-guarantees.md).

Each value or Trace entry retains independent checking safeguards. Pattern matching
uses Corvus with an internal cooperative timeout for each match, following
[ADR 0056](adr/0056-fixed-internal-safeguards-and-rust-patterns.md). An interrupted
match does not poison later independent work. No JavaScript worker, polling
allowance or accumulated matching-duration budget is shared across values.

Checking a value against an enum or a choice costs work roughly linear in its number of alternatives, so even an enum of thousands of values settles within one check's allowance. Values whose checks are costlier, such as long unique arrays, can still run out of that allowance and stay undetermined.

The values a request keeps always fit the report byte limit, the fixed 8 MiB report ceiling. After the Catalogue's mandatory report reserve and the generate envelope's own, every description is charged the bytes its details need before its values are generated, and every kept value is charged the bytes its largest rendering needs, as a JSON report record, a JSON Lines Trace entry or a human report line, before it is kept. The charge assumes the longest target name the description can give, so when a target of higher precedence later takes the value under the [duplicate rules](#settlement-and-duplicates), its name always fits. A description whose details do not fit is left out, counted in `descriptions_omitted` with a `report_limit` record, and leaves the operation undetermined. A distinct value that does not fit is left out, counted in `values_omitted` with a `byte_limit` record, and leaves its description partial. So every output format fits the limit, whatever the model asks for, such as many strings with a `maxLength` of millions. A value whose Trace entry would be longer than 1 MiB, the longest line a [Recorded trace](trace-contract.md#trace-entries) accepts, is left out the same way, also charged as if under the longest target name, so every JSON Lines entry replays through `esdieff trace`.

## Report 1.5

The JSON report is `schema:"esdieff.report", version:"1.5", operation:"generate"`, extending the [shared report contract](report-contract.md#generated-values-report-15) with these fields:

| Object | Required fields and types | Meaning |
| --- | --- | --- |
| Generate request | assessment:null; inventory:null; generation:Generation; scope:Scope | Generation names its own contract. Scope is `selected` with the document as its one root, the other model sources its composition used as dependencies, both as assessed, and the needed sources it could not use as unavailable. |
| Generation | contract:`"esdieff.generation/2"`; document:Id | selector:SampleSelector, present only for an explicit selector. |
| Generate outcome | Outcome fields | `errors`, `warnings` and `notices` are 0. `incomplete_checks` counts descriptions that are not complete, descriptions left out beyond the ceiling or for the report byte limit, and 1 more when the selection is partial. The disposition is passed or undetermined. |
| Generate requested coverage | inventory:null; count_unit:`"description"`; total:Count|null; assessed:Count; not_applicable:0; unassessed:Count; undetermined:Count | Complete descriptions are assessed, partial and unavailable ones undetermined, and those left out beyond the ceiling or for the report byte limit unassessed. total is null when the selection is partial, since some descriptions may be missing. |
| GenerateResult | kind:`"generate"`; summary:GenerateSummary; selection:`"complete"|"partial"`; descriptions:GeneratedDescription[]; descriptions_omitted:Count; omissions:Omission[] | descriptions_omitted counts descriptions beyond the ceiling and those whose details did not fit the report. omissions has a `count_limit` record for the first, a `report_limit` record for the second, and for values a `count_limit` record for those beyond the ceiling and a `byte_limit` record for those that did not fit. The `report_limit` and `byte_limit` records also appear in `diagnostics.omissions`, which makes the report truncated. |
| GenerateSummary | admitted:Count; rejected:Count; undetermined:Count | Counts every kept value by admission. Omitted values are counted separately. |
| GeneratedDescription | selector:SampleSelector; description:EffectiveLocation; state:`"complete"|"partial"|"unavailable"`; values:GeneratedValue[]; values_omitted:Count | The effective location points at the selected description, including an `sdfInputData` or `sdfOutputData` suffix. Optional evidence holds the five [corpus evidence](#corpus-evidence) category counts, each a Count. |
| GeneratedValue | target:string; value:any JSON value; admission:`"admitted"|"rejected"|"undetermined"`; codes:string[] | value is the generated JSON, with each number in its generated spelling. codes is empty unless the value is rejected. Optional evidence is one [corpus evidence](#corpus-evidence) category string. |

`baseline.interpretations` names the value interpretations that the checks relied on, as model reports name them. An explicit Context request also records the supported draft and its limitations in `baseline.extensions`. Every source stays in `sources`. Description details are written in selection order within the report byte limit, and any that do not fit are counted, never silently dropped. The human report shows the disposition, the value counts, then each description with its state and one line per value with its admission, its target padded to at most 40 characters, its value and its rejection codes. It is written one description at a time and gives the JSON report instead as soon as it would not fit the report byte limit.

## JSON Lines

`--format jsonl` and `to_json_lines()` write one Trace entry per admitted or rejected value, each a JSON map on its own line ending in LF: `{"kind":KIND,"pointer":POINTER,"value":VALUE,"expected":ADMISSION,"label":TARGET}`, with the description's Sample selector, the generated value, its settled admission as `expected` and its target as `label`. A target longer than 256 bytes, which only a long nested entry name gives, is left out as a label rather than shortened. Undetermined values are left out, and the exit code still reports them, as it does values left out for the report byte limit or the trace line limit. A run without settled values writes nothing at all.

The output is a Recorded trace, so `esdieff trace` replays it as regression vectors, as the [trace contract](trace-contract.md#replaying-generated-values) describes. An unchanged model retains each expected admission when replay completes the needed checks. Against a later revision, a changed definite admission fails the entry, such as a former `maximum` that a narrowed maximum now rejects. A removed selector also fails, while unavailable needed meaning or processing capacity leaves an entry undetermined. Keep the vectors under version control alongside the generation outcome. Byte-identical replayable output requires the same inputs, settings and settled admissions, including completion within pattern limits.
