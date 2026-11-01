# Effective model pattern syntax and matching

Esdieff uses pinned `corvus-regex-ecma262` 0.2.7 with Unicode `u` semantics and
unanchored matching, following [RFC 9880 Appendix C.2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.2)
and [ECMA-262 2025](https://tc39.es/ecma262/2025/multipage/text-processing.html#sec-patterns).
The suggested portable subset does not restrict otherwise supported syntax.
The current implementation decision is
[ADR 0056: Fixed internal safeguards and Rust patterns](adr/0056-fixed-internal-safeguards-and-rust-patterns.md).

## Effective syntax

Request `Assessment::ExpandedPatternSyntax` or use
`esdieff check --assessment expanded-pattern-syntax-v1 model.sdf.json`.
The immutable profile `esdieff.expanded.pattern-syntax/1` adds `pattern-syntax/1`
to the initial eight assessment areas. `FullBaseline` binds the same syntax
checking to M047 in `full.pattern-format/1` of its fixed 13-area inventory.
Other assessments, source/effective inspection and Sample retain their purposes.

Composition precedes syntax checking. Surviving inherited and overridden
patterns are assessed at the consumer's effective path, with the nearest
contributing authored token related. A merge-patch null deletion removes the
quality. Ordinary source null retains its nonstring shape error. Catalogue
checks root every model source, while selected checks assess only selected roots
and needed imports. Unused, deleted and overwritten dependency patterns are not
separately assessed in selected scope.

Full descriptions at Property, sdfData, Action input/output, Event output,
nested properties and final Choice/Enum alternatives are eligible. An explicit
`type:"string"` is unnecessary for syntax checking. A present nonstring retains
its `data.string` shape error without a string syntax operation. Restricted
Direct items retain opaque pattern vocabulary. Their eligible Full descendants
are checked. A Direct donor's unavailable meaning stays unavailable in a Full
receiving branch until a surviving explicit Full override establishes it.

The qualified guards retain active Unicode escape spelling, capture-name
terminator and decimal endpoint checks. Duplicate decoded capture names and
unverified name decoding remain unsupported. Reversed decimal endpoints establish
invalidity. Endpoints at or above 1,000,000 exceed the fixed capability. Successful
compilation alone does not expand the qualified name subset.

| Established result | Condition | Requested area |
| --- | --- | --- |
| Applicable syntax evaluated | No condition for admitted syntax | `assessed` |
| Qualified invalid syntax | `pattern.syntax`, RFC/ECMA error | `assessed` when other needed work completes |
| Unsupported name decoding or engine capability | `pattern.unsupported`, operational incomplete | `undetermined` |
| Needed effective text, role or interpretation unavailable | `pattern.unavailable`, operational incomplete | `undetermined` |
| Internal work, structure or engine capacity reached | `operation.limit`, operational incomplete | `undetermined` |
| Complete traversal finds no applicable pattern | No syntax condition | `not_applicable` |

Syntax conditions identify `pattern-syntax/1` and keep fixed messages rather
than exposing private parser text. Unknown engine rejection is incomplete, not
an invented syntax violation. Report-detail omission preserves established
errors and semantic status. The [report contract](report-contract.md) separates
findings, requested coverage and broader coverage.

## Model matching meaning

FullBaseline establishes M048 when the surviving effective pattern compiles
under the adopted matching interpretation. No subject, match or satisfying
witness is required. For example, `(?!)` can have established matching meaning
while admitting no text value. M047 independently owns syntax findings.
Corvus's capacity errors leave dependent matching meaning unfinished. The former
QuickJS limits on capture-name bytes, capture slots and named alternative scopes
are removed. Checked longer names and larger capture/alternative sets can now
have established meaning.

Byte-string interpretation does not establish whether an authored text pattern
constrains encoded text or decoded bytes. That affected M048 meaning stays
undetermined under [ADR 0007](adr/0007-generated-sdf-and-sample-json.md), without
hiding independently known byte, type and syntax observations.

## Selected samples and other consumers

`Catalogue::check_sample` and `sample` apply matching to needed ordinary nonnull
string values. Unicode properties, astral characters, lookaround and numbered
or named backreferences use the selected engine. Anchors apply only when the
pattern supplies them.

A definite mismatch produces the sample-domain error `sample.pattern`, citing
RFC 9880 Appendix C.2. Its primary location is the actual sample value, with its
original byte span or a null span for a typed input. Related locations identify
the effective consumer pattern and nearest authored source token. Independent
length and const failures remain visible.

Malformed needed syntax remains a model error and does not prove sample
mismatch. A complete good Choice alternative can admit a sample while an
independently established malformed-pattern finding still fails the operation.
Nontext samples and null do not request text matching. Direct-item and inherited
unavailable meanings retain their existing boundaries.

`pattern.unsupported` and matching `operation.limit` use the operational basis
`pattern-matching-capability` and identify `sample-text-boolean/1`. They leave
affected matching incomplete without hiding independent rejections. The fixed
nine-area `esdieff.sample/1` inventory is unchanged. Choice witnesses,
declared-value consistency, Generation and Recorded trace checking use this same
implementation. Documentary defaults are not inserted into sample data.

## Fixed internal safeguards

Supplied patterns are limited to 4,096 UTF-8 bytes, subject strings to 1 MiB,
group nesting to 64 and total alternatives to 512. Internal scanning, name
storage/comparison and finding retention use the operation's fixed counters.
Corvus applies additional compilation-size and backtracking-state capacities.
Each expression receives an explicit nonzero 100 ms cooperative backtracking
timeout. No process-global timeout default is changed.

These are internal safeguards, without public processing-budget setters.
There is no JavaScript heap/stack setting, toolkit worker-stack reservation,
interrupt-poll allowance or request-wide accumulated matching duration. Every
match has its own timeout. An exhausted match leaves later independent matches
and Trace entries usable. The engine may retain bounded working storage on the
calling thread according to its own implementation policy.

The timeout does not impose a hard deadline on every search path, parsing,
compilation or whole operation. Some linear searches run to completion, and
large batches can take proportionally longer. There is no exact per-request
memory, total resident-memory or instruction-count guarantee. Successful model
compilation does not guarantee completion for every future subject.

## Rust API migration

Use `Catalogue::from_inputs`, `Catalogue::take_inputs`, `JsonBuilder::new` or
`JsonBuilder::default`. `Limits`, its setters, `Catalogue::with_limits`,
`Catalogue::take_inputs_with_limits` and `JsonBuilder::with_limits` are no longer
public APIs. Decode acknowledgements with
`DifferenceAcknowledgements::from_json(bytes)` without a limits argument.
`NumberWork` is internal, exact numeric comparison is `compare(other)` returning
`Ordering`, and location accessors use `pointer()` without a byte-budget argument.
Semantic request options remain supported. No removed setting is silently
accepted or ignored.
