# 0059: Implementer Property capabilities

For corresponding effective Properties, enabling `readable`, `writable` or
`observable` is breaking for implementers and disabling it is compatible.
This extends [ADR 0053: Implementer revision comparison](0053-implementer-revision-comparison.md)
and the client capability decision in
[ADR 0039: Client-side revision comparison](0039-client-side-revision-comparison.md).
Client comparison stays the default with its existing classifications.
These are Esdieff compatibility rules, not RFC compatibility rules.

An earlier conforming implementation may lack a disabled interaction. Enabling
it creates an obligation regardless of its payload. Disabling it releases the
modeled obligation, consistent with the open obligation policy in
[ADR 0055: Implementer declaration presence](0055-implementer-declaration-presence.md).
An implementation can retain physical capabilities outside that obligation.
This does not establish protocol permission to expose extra operations.
Leaving disablement undetermined would leave routine reductions unresolved.
Treating both directions as breaking would impose an obligation to remove
capabilities. Neither policy is selected.

RFC 9880 Section 5.2, Table 7 supplies default `true` for all three Boolean
characteristics. Composition precedes default interpretation. Absence and
explicit `true` compare equal without changing authored JSON. Malformed values
and unavailable composition do not establish absence. Reads and notifications
are distinct interactions, even when the other already offers Thing-supplied
data. Switching between them keeps both rows and is breaking through enablement.

The rule is conditional on representing the Property, including optional
Properties and optional ancestors. It does not change declaration presence or
requiredness. Whole additions/removals keep their maximal declaration rows.
Changed payload constraints use only common data directions and their existing
proofs and witnesses. No common direction leaves payload changes compatible.
Unchanged payloads create no new rows. Capability rows need no value witness.

The existing characteristic classifier applies with implementer revision order.
Public sides, origins and version warnings retain chronological order. Invalid
unrelated characteristics can prevent dependent data proofs without suppressing
an established capability fact. Unchanged unresolved requiredness creates no
dependency. Identity, composition and allowance failures keep their scoped causes.

Contract `esdieff.revision-comparison/9` supersedes `/8`. Diff envelope `1.7` and
acknowledgement format `1` stay unchanged. Old undetermined acknowledgements do
not match newly classified rows. Review and regenerate them using the intended
perspective. No new report field, reason code, dependency, capability inventory
or whole-model solver is needed.
