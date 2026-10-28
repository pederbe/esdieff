# 0044: Versions of breaking revisions

## Context

A Client that watches a model's `info` `version` to notice incompatible changes
misses a breaking revision that keeps or lowers its version.
[RFC 9880 Section 3.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.1)
gives `version` no mandatory format but RECOMMENDS one that is lexicographically
increasing over a model's life, so a newer model's version compares higher than
every earlier one. A [Revision comparison](../../GLOSSARY.md#revision-comparison)
already finds the Breaking changes of each document pair, following
[ADR 0039: Client-side revision comparison](0039-client-side-revision-comparison.md).
In the OneDM playground and exploratory histories, 14 of the 25 first-parent
revisions with a breaking difference did not raise their version, and every
version there is a date written as `2019-11-14` or `20200706`.

## Decision

A Revision comparison warns once for each compared document pair when the
document's bytes changed, at least one Breaking change was found in that pair,
acknowledged or not, and the earlier revision's `info` `version` is text while
the later revision's is absent or is text that does not compare higher by
Unicode code point order. The warning is a
[Version warning](../../GLOSSARY.md#revision-comparison) with code
`comparison.version_not_increased`, located at the later version, or at the
nearest member that would hold an absent one, with the earlier version as
related evidence. It carries both versions and the pair's breaking count.

Because the RFC only recommends the order, the warning never changes a
classification, the disposition or the exit status, and `strict_warnings` stays
false for `diff`. An acknowledged Breaking change still counts, since an
intended break is exactly when a higher version is expected, following
[ADR 0040: Acknowledged revision differences](0040-acknowledged-revision-differences.md).
An unchanged document earns no warning, even when a document it imports
changed. A breaking difference belongs to the document pair in whose effective
groupings it is located, so a changed document that holds only shared
definitions, such as `sdfData` that other documents import, earns no warning of
its own. Neither does a pair whose earlier version is absent or not text, nor
an added or removed document. The diff report gains a new version for its
warnings, while the comparison contract identity stays the same because no
classification changes.

## Considered Options

Warning on every change without a higher version would flag about 63% of the
measured revisions, including many with no classified difference at all, so only
the Client-relevant breaking case warns. Parsing dates would accept `2020-07-06`
after `20200706`, but it would adopt a format the RFC deliberately leaves open,
so plain code point comparison applies and the warning shows both strings to
make the reason visible. Failing the comparison on the warning would turn a
recommendation into a requirement. Assigning each breaking difference to the
document holding its content origin would warn for a changed shared-definition
document, but a change that only points a reference at other content would then
belong to the unchanged target and warn nowhere. Leaving acknowledged
differences out would silence the warning for the releases that most need a
higher version.
