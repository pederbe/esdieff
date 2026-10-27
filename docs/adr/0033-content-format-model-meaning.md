# 0033: Content-format model meaning

## Context

A Full model assessment must establish the content-format declaration's modeled
meaning independently of a sample. Grammar and positive registry membership do
not establish arbitrary media parameters, their absence or ordered coding
meaning. For example, a registered multipart name cannot conceal missing
knowledge about its required boundary parameter. The existing expanded
assessment in [ADR 0029: Content-format declarations and recorded registry guidance](0029-content-format-declarations-and-recorded-registry-guidance.md)
intentionally advertises only grammar and recorded guidance.

## Decision

Keep ExpandedContentFormat unchanged. FullBaseline M024 additionally establishes
complete recorded numeric CoAP tuples and a small explicit media-rule set:
parameterless JSON and CBOR, one no-effect JSON charset, and the plain-text
US-ASCII default or one UTF-8/US-ASCII charset. Other media-specific rules,
parameters and duplicate parameters remain Unassessed. Missing or withheld
registration evidence remains Undetermined. The [content-format contract](../content-format-contract.md)
pins these distinctions and their primary RFC sources.

Positive explicit coding tokens identify transformations applied in their
stated order, including repeated occurrences, under [RFC 9193 Section 3](https://www.rfc-editor.org/rfc/rfc9193.html#section-3).
Absent coding means no transformation. Explicit reserved identity remains
unavailable; do not import HTTP negotiation semantics as an SDF error.
Numeric declarations designate complete registered content-type/coding tuples,
without a second unrelated registry query or payload interpretation.

Use the existing charged borrowed grammar/registry replay, with a private
meaning module and no public syntax tree. Preserve every authored string and
origin. Record actual admitted consultation as
`content-format-model-meaning/1`, even when optional findings are omitted.
Unsupported semantic observations occur before finding-retention decisions.
All scans, tuple accesses and parameter comparisons consume the original shared
work allowance.

The complete Full report can carry ten interpretation IDs, beyond the registry
module's four, including `copied-reference-relocation/1` from
[ADR 0036: Copied reference relocation](0036-copied-reference-relocation.md).
Requalify both complete report generations, interpretation Vec
growth, registry records, retained-result fallback and optional omission against
the original reserves. The compiled backend fits those reserves; qualification
checks the combined graph rather than assuming a free fourth array slot.
This follows [ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md).

## Considered Options

Completing every registered parameterless media name would conceal required
parameters outside the known rules. Inferring arbitrary media semantics from
recorded CoAP tuple spellings would need additional normalization and matching
rules. Acquiring every registry template would broaden dependencies and trust
without delivering a proportional general interpreter. A public parser tree
would force callers to rebuild conclusions. Payload decoding and protocol
bindings are outside model declaration meaning. Increasing reserves before
qualifying the compiled maximum graph would hide an avoidable ownership cost.
