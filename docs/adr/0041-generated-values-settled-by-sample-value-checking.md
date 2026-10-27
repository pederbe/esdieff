# 0041: Generated values settled by Sample-value checking

## Context

Firmware and app teams test their code against a model with example
interaction data, and today they write every value by hand. Esdieff already
builds Representative values for
[ADR 0039: Client-side revision comparison](0039-client-side-revision-comparison.md)
and checks values it builds itself against an effective data description. A
generator that guesses whether a value is valid would hand testers wrong
expectations whenever a constraint interacts with another, such as a `pattern`
beside a `minLength`.

## Decision

Esdieff generates Generated values for a selected effective data description.
Each value aims at one named Generation target, such as a minimum, a value just
below it, a missing required entry or a wrong type. The fixed target list,
order and value rules form the contract `esdieff.generation/1`, so a changed
rule is a new contract identity.

Sample-value checking settles every Generated value, and its admission is the
only statement Esdieff makes about whether the description admits it. A target
called `below_minimum` that another constraint also rejects is still just
rejected, and a value the checker cannot settle is undetermined, never listed
as valid or invalid. Generation is deterministic, with no randomness or seed,
so the same model and request give the same values in the same order.

A target that does not apply produces no value, and so does one whose value
would exceed the value capabilities. Constraints that Esdieff cannot construct
values for, such as an arbitrary `pattern`, get no target rather than a guess.
Each Generated value is settled with the allowance one Sample-value checking
request would receive, and building each description's values is charged to
that description's own allowance, so one heavy description cannot leave the
others undetermined. Fixed ceilings on values per description and descriptions
per request, and a request-wide limit on retained value bytes that keeps every
output within the report byte limit, keep the operation finite under
[ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md).
A value whose check runs out is undetermined, and its description is partial.
Generation reports no model findings, so its disposition is passed or
undetermined, never failed.

## Considered Options

Random or seeded generation, as in json-schema-faker, finds unexpected
interactions but gives different values across tool versions and needs a seed
contract to stay reproducible. Labelling values valid or invalid by their
target alone would be wrong whenever constraints combine. Synthesizing strings
for a `pattern` would need a regular-expression generator whose results the
bounded pattern capability of ADR 0014 might not settle. One allowance shared
by the whole request, as revision comparison uses for its witnesses, would let
a single large enum leave every description in a document undetermined.
