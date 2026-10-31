# 0053: Implementer revision comparison

A device implementer needs to know whether an implementation satisfying the
earlier model still satisfies the later model. This differs from whether a
later implementation can replace an earlier one. Esdieff adds an explicit
implementer perspective while retaining the client perspective of
[ADR 0039: Client-side revision comparison](0039-client-side-revision-comparison.md)
as the default.

For corresponding data descriptions, widened accepted inputs can require work
from an existing implementation. Narrowed permitted outputs can invalidate its
existing output. The existing bounded proof and witness machinery assesses
these dual obligations with its comparison order reversed internally. Public
revision labels, difference kinds, source origins, witness admissions,
acknowledgements and version warnings retain chronological order. Unit and
content-format added/removed reasons retain that order too.

The client-only exclusion of optional map entries absent from its earlier
description does not apply to implementers. An open map's newly unconstrained
entry can widen accepted inputs, and a newly constrained entry can narrow
permitted outputs. Both need ordinary whole-description witness checking.

This initial implementer perspective classifies data changes, established
meaning changes and documentary changes. Endpoint presence, declaration
requiredness, grouping counts, Property capabilities and whole Action/Event
input or output additions and removals remain undetermined. Client rules for
these changes cannot simply be reversed. In particular, an added optional
affordance need not impose a new implementation obligation. Further structural
rules need their own acceptance examples. Unknown meaning and exhausted limits
keep their scoped undetermined results.

Context changes retain the interaction-only scope of
[ADR 0049: Context revision comparison](0049-context-revision-comparison.md).
Neither perspective assesses metadata-consumer compatibility.

The library, direct CLI and saved Check plans select the same perspective.
Every report records it, including an empty SARIF report. Comparison contract
`esdieff.revision-comparison/5` supersedes `/4`. Diff envelope `1.7` adds the
required `policy.perspective` field to `1.6`. Acknowledgement file format `1`
is unchanged and does not bind entries to a perspective. Authors should export
and review acknowledgements using the perspective they intend to assess.
