# 0055: Implementer declaration presence

Extend [ADR 0054: Implementer declaration requiredness](0054-implementer-declaration-requiredness.md)
with a bounded presence policy for Property, Action, Event, nested Object and
nested Thing declarations inside corresponding groupings. Optional additions
require no implementation work. Removing a declaration releases modeled
obligations and permits an implementation to retain the physical capability.
This policy assesses conformance to expressed obligations, without a closed
inventory rule or device feature inventory.

Use each existing maximal one-sided definition row for its whole effective
declaration subtree. An addition is breaking when that subtree intersects the
later mandatory closure of any corresponding Thing or Object in the same
document. Otherwise it is compatible. Requirements owned only by newly added
groupings do not independently impose obligations on earlier implementations.
Thus a new mandatory child inside an existing optional grouping is breaking,
while a whole new optional grouping with its own mandatory child is compatible.
Incoming requirements from corresponding groupings, including pointers to
nested targets, can make the latter breaking through established closure.

Both additions and removals need complete later requiredness resolution under
its existing conservative scope. Partial membership proves neither a break nor
optionality. Preserve existing resolution, composition and limit causes. A
removed declaration left in the later requirement list is undetermined even
when unchanged syntax needs no separate requiredness row. Removing it and its
requirement together is compatible. Earlier unresolved relationships keep
their own results without suppressing established later presence classifications.
Obsolete payload constraints need no validation to classify a removal.

Reuse the resolved relationships of
[ADR 0026: Required declaration relationships and source evidence](0026-required-declaration-relationships-and-source-evidence.md)
and effective coordinates of
[ADR 0036: Copied reference relocation](0036-copied-reference-relocation.md).
Corresponding-declaration requiredness stays restricted to declarations present
in both revisions. Independent changes to those obligations remain visible,
including requirements introduced by a new affordance. Presence has no data
direction or JSON value witness. Keep chronological kinds, effective pointers,
nearest present-side origins and null absent sides. Renames and moves remain
removal/addition pairs.

Contract `esdieff.revision-comparison/7` supersedes `/6`, retaining diff envelope
`1.7` and acknowledgement format `1`. Review intended differences and regenerate
acknowledgements whose old undetermined classification no longer matches.
Client defaults and classifications remain unchanged. One-sided top-level
groupings, unmatched documents, standalone definitions, Context, grouping
counts, Property capabilities and whole Action/Event input/output presence
retain their existing boundaries. Protocol bindings, inventories, automatic
binding of unmodeled capabilities and actual device feature selection remain
outside this policy.

## Considered options

Treating every added definition as mandatory would confuse declarations with
reusable definitions and make optional additions breaking. Looking only at the
immediate owner's closure would miss incoming requirements and nested targets.
Treating every removal as breaking would impose an unrequested closed inventory
policy. Reversing client presence classifications would miss conditional
obligations. Instead, assess resolved obligations in corresponding groupings.
