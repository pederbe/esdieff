# Effective model pattern syntax

Model authors can request `Assessment::ExpandedPatternSyntax` or use `esdieff check --assessment expanded-pattern-syntax-v1 model.sdf.json`. This immutable profile is `esdieff.expanded.pattern-syntax/1`: the initial eight assessment areas plus `pattern-syntax/1`. It checks the syntax of effective string-valued `pattern` qualities. The reasons are [ADR 0014: Bounded partial ECMAScript pattern capability](adr/0014-bounded-pattern-capability.md) and [ADR 0028: Bounded effective model pattern syntax](adr/0028-bounded-model-pattern-syntax.md).

Syntax follows [RFC 9880 Appendix C.2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.2) and [ECMA-262 2025 Section 22.2.1](https://tc39.es/ecma262/2025/multipage/text-processing.html#sec-patterns), with Unicode `u` semantics. The suggested portable subset is not a restriction on permitted syntax. A private pinned regress 0.12.0 parser uses Unicode mode with optimization disabled. This partial capability independently guards active Unicode escape spelling, escaped name terminators and decimal quantifier endpoints; an unverified parser rejection does not establish an RFC error.

Full descriptions at Property, sdfData, Action input/output, Event output, nested properties and final choice/enum alternatives are eligible. An explicit `type:"string"` is unnecessary for model syntax checking. A present nonstring retains its ordinary `data.string` shape error and supplies no string syntax operation. Restricted Direct items retain opaque pattern vocabulary; their eligible Full properties and alternatives are checked. A Direct donor's unavailable pattern meaning stays unavailable in a Full receiving branch until a surviving explicit Full override establishes that quality.

Composition precedes this rule. A surviving inherited pattern is assessed at its consumer's effective path with the nearest authored source token related. A surviving override uses its own authored token; a merge-patch null deletion removes the quality. Ordinary source null remains an ordinary nonstring value. Source structural replay does not cover syntax and cannot suppress this rule. Unknown needed critical context, role, member, contributor or text leaves affected work unfinished while available siblings remain usable. Source content is never substituted for unavailable effective meaning.

Catalogue checks independently root every model source, including unused definitions. Selected checks assess surviving content of their selected roots and needed imports. Listing a dependency does not separately assess its unused, overridden or deleted patterns. Source/effective inspection and other explicit assessment profiles retain their purposes. Selected sample checking checks needed pattern syntax as a model parameter before matching; it does not request the expanded model-syntax assessment or change that assessment's inventory.

| Established result | Condition and basis | Area status |
| --- | --- | --- |
| All applicable syntax evaluated | No condition for admitted syntax | `assessed`, including any established errors |
| Invalid qualified syntax | `pattern.syntax`, error, RFC and ECMA citations | `assessed` when other needed work completes |
| Duplicate decoded capture names, unverified escaped names or parser rejection | `pattern.unsupported`, incomplete, `pattern-syntax-capability` | `undetermined` |
| Needed effective meaning or text unavailable | `pattern.unavailable`, incomplete, `pattern-establishment` | `undetermined` |
| Original work/allocation, admitted structure, parser or worker capacity exhausted | `operation.limit`, incomplete, `pattern-syntax-capability` | `undetermined` |
| Complete traversal finds no applicable string patterns | No syntax condition | `not_applicable` |

Every retained pattern condition has `assessment_unit:"pattern-syntax/1"`. Fixed messages keep private parser text out of reports. Optional condition, coverage-row or pointer omission preserves established errors and semantic status. Failure to retain a finding after proving invalidity omits its detail without creating a semantic limit. Several unsupported patterns still leave only this one requested area undetermined.

`FullBaseline` also runs this available rule, owning M047 in `full.pattern-format/1` of its fixed13-area inventory. Producing conditions retain `pattern-syntax/1`; optional coverage_owner identifies the Full area. Syntax completion is separate from the modeled matching capability M048. The [report contract](report-contract.md) separates findings, requested coverage and broader coverage.

Only three syntax capacities are configurable: `Limits::pattern_bytes` at most 4096 UTF-8 bytes, `pattern_nesting` at most 64 group levels, and `pattern_alternatives` at most 512 total alternatives. Defaults are these ceilings; zero or larger values are configuration errors. Decimal quantifier endpoints at or above 1,000,000 exceed capability; reversed decimal bounds independently establish invalidity without machine-integer conversion. Capture-name length, matching capture slots and named-scope limits are matching capacities, not syntax restrictions.

Traversal, guard scans, decoded-name storage, comparisons, parser admission and pattern finding capacity use the original operation's remaining work/allocation counters. Sources, branches and patterns receive no fresh allowance. Before its first worker, syntax reserves one logical 4 MiB stack slot from that original allocation allowance and retains it for sequential reuse. One worker is created lazily for the allowance and joined when it is dropped. Guards, parsing, emission and destruction stay inside it, and no engine object escapes. A lowered allocation limit unable to admit that slot leaves syntax unfinished.

Work accounting counts logical byte visits and admitted dependency invocations. Finite input/structure admission does not measure every parser/emitter instruction, bound its private heap, enforce cancellation or promise total resident memory or a wall-time deadline. The library owns the lifecycle, so embeddings need no executable discovery, caller runtime or regex-engine types.

Report admission adds 171 bytes to the existing applicable mandatory reserve for either invoking profile, before composition or syntax work, and subtracts the same amount from optional presentation space. Mandatory owners, optional owners and encoding/output buffers keep their existing independent partitions. This reserve adds no engine manifest or complete-conformance claim.

## Model matching meaning

FullBaseline establishes M048 only when the surviving effective pattern can be
compiled by the adopted Unicode, unanchored matching implementation. It supplies
no subject, performs no match and does not seek a satisfying witness. A pattern
such as `(?!)` therefore has established matching meaning despite admitting no
text value. M047 independently owns syntax errors.

Compilation reuses the selected sample matching implementation, including its
runtime allocation probe, matching-only capture/name/scope guards and original
operation allowances. Syntax can be assessed while M048 remains undetermined
because matching capacity is unavailable. Zero remaining interrupts or duration,
worker/runtime capacity, unsupported interpretation and invalid syntax also
leave matching meaning unfinished. The compiler uses the same paid sequential
worker stack slot and configured heap reservation as sample matching. No new
allowance is assigned to a pattern or alternative.

Byte-string model interpretation does not establish whether an authored text
pattern constrains encoded text or byte data. That affected M048 meaning stays
undetermined under [ADR 0007: Generated SDF and sample JSON](adr/0007-generated-sdf-and-sample-json.md);
the independently known byte interpretation, base type and pattern syntax retain
their own observations.

This establishes the adopted modeled interpretation and available compilation
capability. It does not promise that every future subject fits the subject,
allocation, interrupt or duration allowance. Parsing and compilation retain the
finite supplied-size guards and lack external cancellation, as documented in
[ADR 0014: Bounded partial ECMAScript pattern capability](adr/0014-bounded-pattern-capability.md).

## Selected sample matching

`Catalogue::check_sample` and the `sample` command apply Unicode `u` matching to needed ordinary nonnull string values. Matching is unanchored unless the authored pattern supplies anchors. Unicode escapes, character classes, lookaround and backreferences use the pinned ECMAScript engine selected by ADR 0014; the suggested portable subset is not imposed as a restriction.

A definite mismatch is `sample.pattern`, a sample-domain error with RFC 9880 Appendix C.2 as its basis. Its primary location is the actual sample value, with an original span for byte input and a null span for a typed input. Related locations identify the effective consumer pattern and nearest authored source token, including imported and overridden patterns. A mismatch can coexist with independent length or const failures. Optional diagnostic, witness and pointer omissions do not change the established rejection or its counts.

A malformed needed pattern remains a `pattern.syntax` model error. Its sample matching conclusion is undetermined; the malformed model alone does not prove a sample mismatch. A complete good choice alternative can admit the sample and finish the text area while the independently established model error still fails the operation. Nontext samples and null do not request text constraints. Opaque Direct items and inherited unavailable meanings remain unfinished.

Unsupported matching produces `pattern.unsupported`; finite matching work, subject, engine compilation, allocation or stack exhaustion produces `operation.limit`. Both use operational `pattern-matching-capability` and identify `sample-text-boolean/1`, preserving independent findings. Neither is a syntax violation or a sample acceptance restriction. The fixed nine-area `esdieff.sample/1` inventory remains unchanged. Supported matching also feeds selected choice witnesses and declared-value consistency; documentary defaults are never inserted as sample data.

The supported controls are `Limits::pattern_subject_bytes` at most 1 MiB, `pattern_heap_bytes` at most 16 MiB of configured engine allocation, `pattern_stack_bytes` at most 256 KiB, `pattern_interrupts` at most 10,000 polls and `pattern_deadline` at most one second of accumulated cooperative engine time. Defaults are 1 MiB, 16 MiB, 256 KiB, 100 polls and 25 milliseconds. Subject, allocation and stack settings must be nonzero; zero polls or duration gives a work limit. Each engine call spends one entry poll, and engine interrupt callbacks spend the same shared remainder. Short matches therefore spend allowance even if the reused runtime does not reach its internal callback interval. The engine allocation setting excludes allocator rounding and overhead and is not a resident-memory guarantee. A minimal-limit runtime probe checks gross enforcement, including feature-unified embeddings. Pattern size, nesting and alternatives retain the syntax ceilings above.

Matching-only capacities are 124 decoded UTF-8 capture-name bytes, 254 explicit captures and 256 alternatives when any named capture occurs. These limits are applied after syntax admission and leave matching incomplete. Syntax and matching share the original operation work/allocation and one paid worker stack slot; the engine reservation, polling allowance and cooperative duration are retained across values, alternatives and declared-value candidates. Parsing and compilation remain finitely admitted but have no externally enforced cancellation. One runtime and context are initialized lazily and reused within the allowance. A Recorded trace or generation request keeps one worker for the whole request and lends it, with its runtime and context, to the allowance of each entry or value in turn, following [ADR 0045: One pattern engine per trace or generation request](adr/0045-one-pattern-engine-per-trace-or-generation-request.md). Each of those allowances still pays its own worker stack slot and engine reservation and has its own polls and duration. A new context polls on its first call. A lent engine has already made that poll, so a borrowing allowance's first match calls the interrupt callback itself once its first helper is compiled. The engine's later callback polls keep their rhythm across allowances, so an allowance can rarely spend one more poll than it would with an engine of its own. A worker that stops stays with its allowance, whose later matches reach a limit, and is never lent. Each call compiles its own expression, clears pending exceptions and collects garbage. Request text is copied only after admission against the original work and allocation counters. The runtime is destroyed on its owning worker when the allowance, or the trace or generation request that lent it, is dropped.
