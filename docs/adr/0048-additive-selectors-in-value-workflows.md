# 0048: Additive selectors in value workflows

## Context

Context metadata extends the Sample selector vocabulary used by generation and
Recorded traces. Existing affordance vectors have the same structure and meaning,
but an older implementation refuses the new `context` kind. The trace contract's
original reference to a fixed entry format did not distinguish its member
structure from the set of supported selector kinds.

## Decision

Adding a Sample selector kind is a compatible extension when existing kinds,
entry members and outcome rules retain their meaning. Context therefore uses
`esdieff.generation/1` and `esdieff.trace/1`. Generation retains its target list,
ordering and settlement rules. Context requires explicit selection, so default
generation does not add metadata to an existing affordance corpus.

This clarifies [ADR 0042: Recorded traces with independent entries](0042-recorded-traces-with-independent-entries.md).
A contract identity does not promise that every implementation supports every
selector kind or model meaning. Unknown kinds remain refused. Supported Context
requests identify their draft and limitations in the report. Changes to required
members, the meaning of an existing kind or the rules that map admission and
expectations to outcomes require a new trace identity.

## Considered options

A new identity for every additional selector kind would also change reports for
unchanged affordance traces. It would require consumers to update their contract
guards even when they never select the added kind. Keeping the identity preserves
those consumers while requiring explicit use of the new capability. It does not
make Context vectors replayable by older implementations.
