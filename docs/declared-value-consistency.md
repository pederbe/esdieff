# Declared-value consistency

Use `Assessment::ExpandedConsistency` in the library or
`esdieff check --assessment expanded-consistency-v1` in the command. The requested
identity `esdieff.expanded.consistency/1` contains the unchanged Initial eight areas
and `const-consistency/1` and `default-consistency/1`, counted once across model
scope. Other assessments retain their membership, including Full's fixed model
inventory. See [ADR 0031: Bounded declared-value consistency](adr/0031-bounded-declared-value-consistency.md)
and the [shared report contract](report-contract.md).

`const` restricts data to a stated value. `default` is documentary data that may
be used when interaction data is absent; base SDF defines no insertion process.
This assessment checks stated values against their final effective data description.
It does not modify either model or sample input, infer absent data, prove restrictive
refinement or perform general satisfiability analysis.

## Candidates and alternatives

Formatted-null equality follows ADR 0015 across the six base string formats.
Null equals null and differs from an established nonnull formatted value; an
invalid or uninterpreted nonnull operand leaves the comparison unfinished.
Nullability can independently exclude a null constant or default. A null default
can be excluded by a nonnull constant and vice versa without inserting any value.
The same contextual comparison supports actual compound candidates, preserving
one advisory warning per definitely excluded candidate and scoped uncertainty.

An ordinary description contributes its surviving const and default values.
Choice overrides replace shared qualities, so each final alternative contributes
its const candidate; an Enum occurrence is distinct even when two occurrences
have equal text. A shared owner default is checked once against the complete
Choice or Enum. An introduced or overridden branch default is checked separately.
A branch reference introducing a donor default is local to that branch even when
its source origin is inherited. Deleted branch overrides fall back to the surviving
shared quality according to
[ADR 0024: Effective alternatives and nullable characteristics](adr/0024-effective-alternatives-and-nullable-characteristics.md).
Distinct importing consumers are assessed in their own effective contexts.

An alternative must admit the entire candidate. Constraints from different
alternatives cannot supply different pieces of one witness. If one alternative
admits it, the candidate is not excluded. If an unknown alternative could admit it,
exclusion remains undetermined.

## Supported reasoning and limits

The shared selected-value evaluator handles established ordinary scalar root kinds,
null admission, exact numeric integrality and bounds, positive steps, Unicode scalar
text lengths and supported Unicode ECMAScript patterns, map required entries and present properties, array counts, applicable
items and contextual uniqueness. Equality preserves exact numbers, decoded text
and structural values under the resolved description. Defaults are also checked
against a surviving const restriction.

The same rules apply inside compound candidate values and nested named
alternatives, including required presence, exact inherited/overridden numeric
fields, compound const, contextual uniqueness, Unicode patterns, the six base
string formats and the byte-string/POSIX conventions. A warning still belongs to
one excluded declared-value candidate, with its actual decisive leaf and effective
constraint. A sample mismatch remains a sample error in the separate sample
operation. Documentary defaults do not fill missing required sample entries.

Civil date, time and date-time denotation follows [ADR 0018: Civil date and time sample conventions](adr/0018-civil-date-time-sample-conventions.md) through the same evaluator. An invalid civil candidate can supply established exclusion advice; unavailable leap meaning leaves consistency unfinished. Actual convention and C72 use are recorded even when an independent length restriction already excludes the candidate or a branch result is discarded. Under a common compatible format, established dates compare by calendar date and dated date-times by UTC instant with exact fractions and distinct inserted-second slots. Date-free time retains the identical-valid-representation boundary.

Unsupported needed pattern matching, other special formats or types, extension meaning,
Direct item meaning, reference interpretation, unavailable data and finite semantic,
numeric, equality or allocation limits leave the affected area unfinished. An empty
array does not require unused item meaning, and an absent optional map property
does not request its value constraints. Established independent restrictions can
still produce advice while other work remains unfinished. Completing consistency
does not complete Initial or other requested work.

Pattern matching follows the [pattern contract](pattern-contract.md). A malformed
pattern actually consulted for a candidate is an independent model syntax error,
recorded once per effective pattern even across const and default candidates or
discarded alternatives. A complete admitting alternative can finish consistency
while that model error still fails the operation. A dormant pattern without a
candidate, or a constraint on an absent optional candidate entry, is not requested.
The model-condition identity remains separate from optional diagnostic retention.

An established alternative collection may remain incomplete because some branch
meaning is unfinished. Its owner candidate can still be evaluated when that
deferral only summarizes the established branches. Every possible branch must
independently exclude the entire candidate; a potentially admitting branch prevents
advice. Local owner failures, unavailable candidates and unknown applicable
constraint premises retain their existing limits. Aggregate deferral never makes
the enclosing operation complete or supplies meaning to an unknown branch.

Each area uses `assessed`, `not_applicable`, `unassessed` or `undetermined` with shared
status meanings. Assessed includes established exclusions. No applicable candidate
after complete traversal is not applicable. Needed unsupported or unavailable
evaluation makes the area undetermined. A completed or not-applicable row has
Advice basis; an unfinished row has Operational basis without an RFC citation.
The requested denominator stays ten when rows or condition details are omitted.
Unavailable candidate-bearing containers also leave both areas unfinished when
their descendants cannot be traversed. This includes hidden Full descriptions in
Direct item properties or alternatives; it does not assess a Direct item's own
default or give unavailable descriptions evaluation capability. Established
subtree overrides and opaque content retain their existing role boundaries.

## Findings and provenance

`consistency.const_excluded` and `consistency.default_excluded` are advisory warnings,
one per excluded candidate. They never add sample errors or turn advice into an RFC
violation. Existing Initial warnings retain their producing identity and count:
`type: "boolean", const: 1` can produce the existing primitive contradiction and
one const consistency warning, for two warnings in total. Strict warnings change
disposition only. Aggregate counts survive zero diagnostic detail.

The source primary identifies the real decisive candidate leaf, or the real
container when absence or structure is decisive. Related locations include actual
effective description/constraint operands, paired values for uniqueness, a branch
identity when needed, original contributions and the importing consumer. Composed
values never claim a fabricated original Sample source or a donor's whole compound
token. Location or condition omission is explicit and preserves aggregate advice.

One request owns the remaining equality, numeric, semantic and allocation counters
across all candidates and final alternatives. No per-candidate allowance, refill,
refund or source-as-effective fallback is introduced. Existing mandatory report
owners hold the two area aggregates; no added mandatory reserve is required.
