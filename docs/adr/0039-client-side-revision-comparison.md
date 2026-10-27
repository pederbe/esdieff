# 0039: Client-side revision comparison

## Context

A manufacturer that changes a shared definition needs to know which device
interfaces now break their clients. RFC 9880 gives `version` no mandatory format
and defines no compatibility rules, and the
[Restrictive refinement](../../CONTEXT.md#composition) convention has no general
checking procedure. Esdieff already establishes effective groupings with their
content origins, exact numbers and Sample-value checking, so a
[Revision comparison](../../CONTEXT.md#revision-comparison) can reuse them. The
rules below are Esdieff's own.

## Decision

`esdieff diff` compares two Model revisions from the Client's side and
classifies each difference between their corresponding groupings as breaking,
compatible or undetermined.

**Direction.** Narrowed Client-supplied data, widened Thing-supplied data, a
removed grouping or affordance, and a Property characteristic that becomes false
are Breaking changes. So is input data added to an Action that had none, since
existing invocations send none, and so are input data removed from an Action and
output data removed from an Action or Event. These changes are to the affordance
as a whole rather than to a data description, so they need no Breaking witness.
A declaration that stops being mandatory in a grouping is breaking, and one that
becomes mandatory is compatible. Requiredness is compared as the closure of
mandatory declarations in each corresponding grouping, through its required
groupings, and an affordance's `sdfRequired` belongs to its enclosing grouping.
A declaration required from an enclosing grouping can only be represented inside
the groupings between them, so each of those is mandatory there too, and the
declaration is mandatory in each of them. Things decide how many instances a
Grouping array has, so its instance count range behaves like Thing-supplied
data: a narrower range is compatible and any other change is breaking. An absent
minimum count reads as 0 and an absent maximum as unbounded, and a grouping that
becomes or stops being an array stays undetermined. A Property that is both
readable and writable carries data both ways, so any change to its value set
breaks one direction or the other. Data that the two revisions carry in no
common Client direction reaches no Client in both, so its changes are compatible
and its characteristic changes decide. Clients use only what the earlier
revision describes or requires. A map entry that the earlier revision neither
describes nor requires, or output data an affordance did not have, is part of no
Client's interaction, so describing it later is compatible in either direction.
A described entry whose description is removed from Client-supplied data stays
admitted, since maps are open, so that removal is compatible too. Removing a
described entry's description from Thing-supplied data is breaking with a
witness map holding a value that the earlier description rejects. Candidate
witnesses for map data are therefore placed only at entries the earlier revision
describes or requires, while the other required entries around them are filled
as for any nested candidate, described under Proof. A difference that only
obliges Things to accept or provide more is compatible. Compatibility for Thing
implementers is not assessed.

**Proof.** A change to the values a data description admits is breaking only
with a Breaking witness: a value that Sample-value checking establishes as
admitted by one revision's effective description and rejected by the other's, in
the breaking direction, following
[ADR 0016: Selected samples with explicit admission and coverage](0016-selected-sample-contract.md).
It is compatible only with a proof that no witness exists, such as every changed
quality loosening in the safe direction. Candidate witnesses come from the
changed quality itself and from representative values of the revision that must
admit them, such as its `const`, its enum values, its documentary `default` or a
simple value of each type it admits. A witness belongs to the change it was made
for, so a candidate inside nested data stands beside representative values of
the enclosing entries that either revision requires, and only where both
revisions describe that enclosing content alike. Everything else, including a
quality the comparison cannot yet classify, is undetermined, following
[ADR 0003: Check findings and completeness](0003-check-findings-and-completeness.md).
A witness check that runs out of the operation allowance stays undetermined,
even when it has already established a rejection that ADR 0016 keeps for a
supplied sample. Exact numbers and Value equality decide whether an effective
quality changed at all, so `10` and `1e1` are the same bound, following
[ADR 0012: Exact numbers and JSON input policies](0012-exact-numbers-and-json-input-policies.md)
and [ADR 0015: Bounded value equality in a resolved data context](0015-value-equality.md).
An absent `uniqueItems` reads as `false`. No general subset solver is involved.

**Correspondence.** Each revision is one supplied Catalogue. Documents
correspond by source key, which `esdieff diff` derives from each document's
location within its revision, groupings by their position in the corresponding
document, and nested groupings and affordances by Given Name. Enum values
correspond by value, so reordering an enum changes nothing. Choice alternatives
correspond by Given Name and are compared as the effective alternatives they
derive, and an array's `items` is compared as the nested description of its
values. Moves and renames read as a removal and an addition. Shared definitions
are compared through the effective groupings that use them, with their content
origins, following
[ADR 0020: Owned effective views and nearest origins](0020-owned-effective-views-and-nearest-origins.md).

**Meaning qualities.** `unit`, `contentFormat` and `sdfType` tell a Client how
to read a value, so a change between two established, different meanings is
breaking in every Client direction the data carries. For units, that is a change
between two different current names in the recorded SenML evidence, from either
its SenML Units or its Secondary Units section. A legacy name, a URI name, an
unrecorded spelling, an added or removed unit, or withheld evidence leaves the
change undetermined, following
[ADR 0027: Unit names and recorded registry guidance](0027-unit-names-and-recorded-registry-guidance.md).
Legacy names can give one unit several names, such as `beat/min` and `1/min`.
For `contentFormat`, two declarations with established meaning that differ are
breaking, and two spellings of one meaning, such as `50` and `application/json`,
are no change, following
[ADR 0033: Content-format model meaning](0033-content-format-model-meaning.md).
For `sdfType`, a change between `byte-string` and `unix-time` is breaking. An
added or removed meaning quality, or one without established meaning, is
undetermined unless a Breaking witness settles its data change. Beside a choice,
a shared meaning quality tells Clients how to read only the values of the
alternatives that inherit it, so a change that every alternative replaces is
compatible.

**Other qualities.** Changes to `description`, `label`, `$comment` and
documentary `default` are compatible, since none affects an interaction. An
`items` description that carries a quality outside the vocabulary of a Direct
items description has no established meaning, following
[ADR 0024: Effective alternatives and nullable characteristics](0024-effective-alternatives-and-nullable-characteristics.md),
so inside it only changes to `description` and `$comment` are compatible and
every other change is undetermined. A revision with findings or unresolved
references is still compared, and the affected parts are undetermined, following
[ADR 0006: Scoped reference failures](0006-scoped-reference-failures.md). The
result is breaking when any difference is breaking, otherwise undetermined when
any is undetermined, and it uses the exit codes of `check`.

## Considered Options

Flagging every tightened quality is simpler, but it raises false alarms when
another constraint, such as a `const`, already excluded the value the change
removes. Checking only narrowed inputs misses a widened reading that clients
never handled. Keeping a rejection that a witness check established before its
allowance ran out would classify a difference whose checking was cut short.
Treating unclassified differences as compatible would hide breakage behind
missing capability. Pairing by Global name survives file moves, but device
documents commonly declare no default namespace and would stay unpaired.
Guessing renames from similarity would invent identity. Counting any change of
unit text as breaking would flag legacy-name replacements and typo fixes.
Treating only SenML Units names as current would leave a change such as `ms` to
`s` undetermined, although Clients misread every value. Classifying the data of
a Property whose revisions share no Client direction by each quality's own rule
would call a unit change there breaking, although no Client exchanges that data
in both revisions. Comparing authored `sdfChoice` branches rather than derived
alternatives would read a branch that overrides a shared bound as the opposite
change. Filling the entries around a nested candidate from the admitting
revision alone would let the other revision reject it for a change elsewhere in
the enclosing map, and pin that witness on the wrong difference. Comparing raw
`sdfRequired` arrays would miss a requirement spelled another way or moved
between nested groupings. Reading open maps literally would make every added
optional entry in Client-supplied data breaking, although no Client built from
the earlier revision sends it. Letting a requirement of a nested declaration
reach only the requiring grouping would report a requirement moved into a
required nested grouping as a change, although Things must provide the same
data.
