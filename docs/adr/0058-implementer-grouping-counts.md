# 0058: Implementer grouping counts

The implementer perspective of
[ADR 0053: Implementer revision comparison](0053-implementer-revision-comparison.md)
now classifies changed counts on corresponding Object and Thing groupings that
are arrays in both revisions. Every modeled instance count permitted by the
earlier revision must remain permitted by the later revision. A raised minimum
or lowered maximum is breaking, while a lowered minimum or raised maximum is
compatible. This extends the grouping-count decision of
[ADR 0039: Client-side revision comparison](0039-client-side-revision-comparison.md).
Client classifications remain unchanged. These are Esdieff compatibility rules,
not rules supplied by RFC 9880.

Both complete count ranges must be established, nonempty ranges of nonnegative
integers. An absent minimum supplies no restriction below zero and an absent
maximum is unbounded. These comparison semantics insert no model defaults.
At least one count quality must remain in each revision. Array/non-array changes
and invalid ranges remain undetermined. Exact numeric comparison and existing
operation allowances apply. Each changed quality retains its own row without a
data direction or value witness. Equal effective bound meanings are compatible,
while equal numeric spellings produce no difference.

Optional groupings and optional ancestors do not suppress count obligations.
An earlier implementation may represent them with a count the later revision
excludes. A positive minimum does not make a declaration mandatory, and zero
instances differ from an absent declaration. Declaration presence and
requiredness keep their separate rules. Retaining extra physical capabilities
outside the model does not retain a count excluded from a modeled grouping.
Hiding or rebinding an instance would change the implementation.

An overlap-only rule would accept a shift from [2,4] to [3,6], even though an
earlier two-instance implementation loses conformance. We reject that rule.
Both ranges contain three and four, but preservation must hold for every earlier
permitted count. Each bound is classified independently, so this shift has a
breaking minimum and a compatible maximum for implementers.

Counts use the existing effective composition, source-key, class and Given Name
correspondence. Copied bounds and local overrides report at consumer coordinates
with nearest content origins, including deletion origins. Unavailable facts and
exhausted allowances keep their existing scoped causes. No device inventory,
protocol binding, rename mapping or whole-model realizability solver is added.

Contract `esdieff.revision-comparison/8` supersedes `/7`. Diff envelope `1.7` and
acknowledgement format `1` remain unchanged. Old undetermined acknowledgements do
not match newly classified count differences. Review intended differences and
regenerate acknowledgements where needed using the selected perspective.
