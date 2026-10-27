# 0040: Acknowledged revision differences

## Context

A [Revision comparison](../../CONTEXT.md#revision-comparison) fails a CI job on
every Breaking change, following
[ADR 0039: Client-side revision comparison](0039-client-side-revision-comparison.md).
A deliberate major revision breaks Clients on purpose, and so does a change the
maintainer has reviewed but Esdieff leaves undetermined, such as a changed
pattern. Without a way to record that intent, the maintainer must disable the
comparison for the release that needs it most. The words "accepted" and
"baseline" already name Acceptance restrictions and the Standards baseline.

## Decision

A maintainer can supply Difference acknowledgements. Each one names one
intended difference by its kind, subject, quality, classification and, for each
revision, the document's source key and the difference's effective pointer. A
difference that matches one is an Acknowledged difference. It keeps its
classification, and only unacknowledged differences decide the disposition and
exit code, so an acknowledged breaking comparison exits 0 while its report
still says it breaks Clients.

The identity leaves out request-local source IDs, which shift when documents are
added, and content origins, which move when content is refactored into a shared
definition. It also leaves out witnesses and quality values, so an
acknowledgement covers later changes to the same quality at the same place in
the same classification. Because classification is part of the identity, a rules
change that turns an acknowledged undetermined difference into a breaking one
brings it back for review.

Breaking differences and undetermined differences of kind removed, added or
changed can be acknowledged. Unavailable content, differences left undetermined
because content they depend on is unavailable or by the operation limit, and
differences whose details the report could not retain cannot be, since nobody
examined that content. An acknowledgement that matches nothing is reported and
leaves the disposition unchanged. The diff report gains a new version for the
changed disposition fields, while the comparison contract identity stays the
same because no classification changes.
Acknowledgements follow
[ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md).

## Considered Options

A new accepted classification would hide the real classification and change a
fixed value set. Acknowledging breaking differences only would still fail CI on
a reviewed undetermined change, such as an added unit. Failing on unused
acknowledgements would force cleanup, but reverting an intended change or
upgrading the comparison rules would then turn CI red. Including witnesses or
quality values in the identity would pin exact values, so entries would break
whenever the witness rules improve.
