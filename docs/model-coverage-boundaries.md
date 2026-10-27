# Model coverage boundaries

Esdieff requests the whole recorded base-model scope through
`--assessment full-baseline`. The [model assessment inventory](model-assessment-inventory.md)
contains 54 obligations, grouped into 13 assessment areas. The
[clause-family trace](model-clause-trace.tsv) also records sample, processor,
converter, registry and operator duties that do not belong in that model count.
This separation follows [RFC 9880](https://www.rfc-editor.org/rfc/rfc9880.html),
including its normative Appendices A and C.

A complete report establishes the requested work for the supplied model.
It does not establish that every SDF model is supported. For example, the combined
ordinary model in `tests/fixtures/model-meaning/ordinary.sdf.json` exercises
available scalar, compound, pattern, format, special-type and content-format
meanings. An empty document, a Boolean `sdfRef` or an unavailable comparison
interpretation has different applicable obligations.

The default Initial assessment retains its narrower scope. Choosing Full requests
more conclusions; it does not supply missing interpretations or knowledge.
Completion, findings and validity remain separate under
[ADR 0003: Check findings and completeness](adr/0003-check-findings-and-completeness.md).

## Available comparisons

The shared contextual equality contract supports exact ordinary numbers, decoded
text, Boolean and null values, and supported maps, arrays and alternatives. Its
specialized domains include canonical byte strings, opaque UUIDs, Gregorian
dates, dated UTC instants and established nonnegative POSIX coordinates. Nullable
UUID, Gregorian-date and dated date-time comparator capability includes known
absent, true and false Full nullability. Dated instant comparison follows the
existing civil denotation and recorded adjustment knowledge boundaries.

Null compares with null and with an established nonnull value in compatible
`date`, `date-time`, `time`, `uri`, `uri-reference` and `uuid` contexts. A malformed
or uninterpreted operand does not establish a difference. Identical established
nonnull time and URI representations can establish equality, while
their general nonnull comparison capability remains unavailable. Established
date-times compare by UTC instant with exact fractional seconds and distinct
inserted-second slots, after every relevant interpretation is compatible. These rules
follow [ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md),
[ADR 0018: Civil date and time sample conventions](adr/0018-civil-date-time-sample-conventions.md)
and [ADR 0019: URI and UUID sample conventions](adr/0019-uri-and-uuid-sample-conventions.md).

## Retained gaps

Each entry retains its affected obligations, actual unfinished outcome, reason
and condition for reconsideration. These are capability boundaries, not another
runtime status or a change to the immutable inventory. Where the reason is an
RFC ambiguity, the retained outcome is deliberate Esdieff policy. Esdieff does
not wait for author clarification. It adopts its own RFC interpretation,
recorded in an ADR, when a real model or caller needs one, and treats any later
author guidance as evidence. An affected model can also have independent
assessed findings. The exact status depends on the final effective context and
all applicable occurrences, as described in the [model meaning
contract](model-meaning-contract.md).

| Capability | Affected obligations and current outcome | Reason retained | Reconsider when |
| --- | --- | --- | --- |
| Short-name and Boolean `sdfRef` processing | M018/M019 where processing is needed: undetermined | Formal admission of a reference form does not establish its operational target selection. `sdfRequired` abbreviations have their own established declaration rules. | Retained as policy: Esdieff gives these forms no processing meaning. A real model that needs one prompts an adopted interpretation. |
| Empty-document definition-group presence | M008: undetermined for a document without a populated group | Section 3.3 describes one or more groups; Appendix A makes the groups optional. No unsupported empty-document rejection is invented. | Retained as policy until a real model or caller needs an interpretation, covering absent groups and present empty maps. |
| `sdfRequired` in data-only descriptions | M020: undetermined when its meaning is needed | Appendix A admits the quality through common qualities; Section 4.5 explains requiredness through affordance and grouping declarations. | Retained as policy until a real model or caller needs an interpretation of its applicability and carrier meaning, independent of map-entry `required`. See [ADR 0026: Required declaration relationships and source evidence](adr/0026-required-declaration-relationships-and-source-evidence.md). |
| Wider Direct item descriptions and role-changing imports | M041 and dependent M042/M050 or comparison obligations: undetermined | Restricted `jso-items` content and Full property/Choice descendants have different roles. Framework admission alone does not supply wider Direct meaning. Older exploratory models exercise Direct-item reference imports, so this is an observed compatibility limit. | Deliberately deferred. A current model or named caller workflow must justify adopting a role-specific interpretation of wider qualities, defaults and imported descriptions. The older examples alone do not select that work. Full descendants keep their available meanings. |
| Byte-string length and pattern constraints | M046 for length or M048 for matching: undetermined when present | The conventional channel denotes bytes through base64url text. The adopted rules do not decide whether these constraints operate on that text or the abstract bytes. Pattern syntax can still be assessed independently. | Retained as policy until a real model or caller needs an interpretation of the length unit and pattern subject, with distinguishing encoded-text/byte examples. See [ADR 0007: Generated SDF and sample JSON](adr/0007-generated-sdf-and-sample-json.md). |
| General date-free time equality | M052 general comparator capability: undetermined | A clock expression carries no date; offset wraparound and exceptional labels need their own comparison meaning. | A recurring date-free workflow justifies a separate contract without supplying a host date. See ADR 0018. |
| General URI and URI-reference equality | M052 general comparator capability: undetermined | String difference is not a general resource-identity proof. Normalization, scheme meaning and a URI-reference base depend on the selected purpose. | A concrete workflow establishes its equivalence purpose, scheme rules and explicit base requirements. See ADR 0019 and [RFC 3986 Section 6](https://www.rfc-editor.org/rfc/rfc3986.html#section-6). |
| Negative nonzero POSIX denotation and general Unix-time comparator capability | M052 generic Unix-time capability: undetermined; actual M043 remains undetermined where denotation is unavailable | Exact negative numbers are retained, but their pre-epoch abstract denotation is outside the adopted bounded convention. Nonnegative operands retain their available equality. | A real pre-epoch workflow justifies a separately evidenced convention. See [ADR 0007](adr/0007-generated-sdf-and-sample-json.md) and [ADR 0015](adr/0015-value-equality.md). |
| Extended `sdfType` name admission | M025 for a non-base string: unassessed | Extended lexical admission has not been implemented. This is separate from whether a particular extension's meaning is known. | Extension-bearing models need bounded name admission, with malformed spelling, framework fallback and M028 meaning kept distinct. |
| Unknown extension, critical-feature and abstract-selector meaning | M028 where undeclared extension vocabulary occurs: undetermined. M013/M028 and dependent obligations for a declared Extension feature or an unknown base selector value: undetermined when needed | A retained declaration or a registered spelling does not by itself provide an implemented interpretation. Undeclared extension vocabulary leaves base meaning intact under [ADR 0037: Undeclared extension vocabulary](adr/0037-undeclared-extension-vocabulary.md), so only its own meaning stays unsupported. Recorded extension vocabulary is the exception. `oma:id` at its registered positions does not stop base meaning, and Full assesses M028 when its value is in the recorded `uint` domain. A value outside that domain is an `extension.recorded_value_excluded` error at each effective coordinate, with a registration basis, and M028 is still assessed. | A selected extension has a stable specification, a real model workflow and explicit coverage rules. See [ADR 0003](adr/0003-check-findings-and-completeness.md) and [ADR 0038: Recorded extension vocabulary](adr/0038-recorded-extension-vocabulary.md). |
| Additional media-specific content-format parameters and meanings | M024: unassessed for unsupported media rules; undetermined for needed unavailable registration evidence or reserved identity meaning | Full supports recorded numeric tuples and bounded JSON, CBOR and plain-text rules. Other media rules cannot be inferred from spelling or registration. | A concrete media declaration needs its own primary specification and bounded parameter rules. See [ADR 0033: Content-format model meaning](adr/0033-content-format-model-meaning.md). |
| ECMAScript patterns beyond bounded syntax or matching capability | M047/M048: undetermined for needed unsupported capability | The supported engines and finite controls do not cover every valid construct or guarantee resources for every subject. Unsupported capability is not proof of invalid syntax. | Representative model patterns establish a worthwhile missing construct and its bounded implementation. See [ADR 0014: Bounded partial ECMAScript pattern capability](adr/0014-bounded-pattern-capability.md). |
| Missing, withheld or out-of-edition registry and temporal knowledge | M012/M023/M024/M028 and dependent actual-value conclusions as applicable: undetermined | Recorded positive registry entries do not establish current absence, and missing leap knowledge does not establish a future adjustment. A withheld OMNA record leaves every `oma:id` value unchecked. | Explicit evidence or knowledge policy supplies the needed facts. See [ADR 0021: Modified metadata assessment](adr/0021-modified-metadata-assessment.md), [ADR 0027: Unit names and recorded registry guidance](adr/0027-unit-names-and-recorded-registry-guidance.md), [ADR 0029: Content-format declarations and recorded registry guidance](adr/0029-content-format-declarations-and-recorded-registry-guidance.md) and [ADR 0038: Recorded extension vocabulary](adr/0038-recorded-extension-vocabulary.md). |

Missing or ambiguous supplied dependencies, effective expansion cycles and
exhausted operation budgets can also prevent conclusions. They are assessed
through the existing scoped failure and finite-work contracts, rather than
promising automatic acquisition, recursion or unlimited processing. See
[ADR 0004: Supplied catalogues and checking scope](adr/0004-supplied-catalogues-and-checking-scope.md)
and [ADR 0009: Bounded processing and reporting](adr/0009-bounded-processing-and-reporting.md).

For unsupported vocabulary, Full conditions name each retained quality and its
source location. Composed observations also identify the effective position.
These details help locate the cause of unfinished work, including draft
vocabulary and base qualities used outside their supported role. They do not
provide the missing meaning. The [report contract](report-contract.md) describes
their bounded retention and omission reporting.

## Scope beyond model checking

Whole interaction scenarios, protocol bindings, ecosystem conversions, device
behavior, existing-document editing and separately specified extensions are
additional workflows. General satisfiability and unrestricted refinement proofs
are additional analyses. They do not silently increase this model inventory or
become prerequisites for completing an ordinary supported model assessment.

Selected sample checking establishes outcomes for the actual supplied values.
A passing sample does not discharge model obligations, and an available model
interpretation does not guarantee every future sample's denotation or processing
capacity. The [sample contract](sample-contract.md) records that separate scope.
