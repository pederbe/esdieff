# 0054: Implementer declaration requiredness

Extend [ADR 0053: Implementer revision comparison](0053-implementer-revision-comparison.md)
to classify declaration obligations for corresponding groupings. An existing
implementation satisfying the earlier model may omit an optional declaration.
Making that declaration mandatory can therefore require work, even when some
implementations already provide it. Releasing an obligation still permits its
representation and is compatible for implementers.

Compare each Thing and Object's resolved mandatory closure after composition,
restricted to declarations present at corresponding document, class and Given
Name paths in both revisions. Any new obligation is breaking for implementers,
including when other obligations are released. Released obligations alone are
compatible. Equal sets produce no requiredness difference. Client comparison
remains the default and retains its existing rule that a released obligation
is breaking. These are Esdieff compatibility policies derived from the selected
perspective. RFC 9880 Section 4.5 supplies the declaration relationships, not
these classifications.

Reuse the relationships of
[ADR 0026: Required declaration relationships and source evidence](0026-required-declaration-relationships-and-source-evidence.md)
and effective coordinates of
[ADR 0036: Copied reference relocation](0036-copied-reference-relocation.md).
The closure includes enclosing groupings needed to represent a nested target
and requirements of mandatory groupings. A child requirement in an optional
grouping applies whenever that grouping is represented without making the
outer grouping mandatory. Pointer, short-name and carrier forms are equivalent
when they establish the same closure.

Added or removed declarations retain independent endpoint differences, which
remain undetermined for implementers. Missing declarations do not establish
optionality. Partial or unavailable resolution keeps the existing conservative
scope and cause. It cannot prove a subset relation. Independent composition
and data differences retain their own results. No data direction or JSON value
witness is added to requiredness, which reports one changed `sdfRequired`
quality per affected corresponding grouping.

Comparison contract `esdieff.revision-comparison/6` supersedes `/5`. Diff
envelope `1.7` and acknowledgement format `1` remain unchanged. An old
undetermined acknowledgement does not match a newly classified difference.
Authors must review intended differences and may need to regenerate
acknowledgements. Property capabilities, grouping counts, whole Action/Event
input/output presence and Context metadata compatibility retain their existing
implementer boundaries.

## Considered options

Reversing the final client classification would mark a simultaneous addition
and release compatible for implementers. Instead, assess whether each side's
obligations entered or left the closure. Comparing authored lists would report
changes between equivalent spellings and lose composed requirements. Treating
an absent endpoint as optional would introduce an endpoint-presence policy
outside this decision's scope.
