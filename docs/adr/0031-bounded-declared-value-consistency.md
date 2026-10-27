# 0031: Bounded declared-value consistency

Model authors need advice when a stated `const` or documentary `default` is
excluded by its effective data description. Select `esdieff.expanded.consistency/1`
explicitly, containing the unchanged Initial eight areas and `const-consistency/1`
and `default-consistency/1`. Exclusion is Warning/Advice, separate from normative
parameter errors and existing advisory producers. The [consistency contract](../declared-value-consistency.md)
sets the exact scope, following [RFC 9880 Section 4.7.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.7.2)
and [Appendix C](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C).

Reuse the selected-value evaluator and contextual equality of
[ADR 0015: Bounded value equality in a resolved data context](0015-value-equality.md)
and [ADR 0016: Selected samples with explicit admission and coverage](0016-selected-sample-contract.md).
An internal borrowed value distinguishes a role-checked Sample source from a
current effective Model value. One immutable context carries the effective
description and candidate root through nested evaluation and equality projection.
Projection validates arena, ancestry and that boundary; it starts at the candidate,
without interpreting model document containers as data. Public sample handles
retain their source-role guarantees.

Check each final alternative's local `const` independently. Check the owner's
documentary `default` once against the complete existential Choice or Enum;
separately check an alternative's introduced or overridden default. A private
construction marker records that boundary. Origin kind alone cannot identify it:
a branch `sdfRef` can introduce inherited donor default `1` with branch minimum `2`
when the owner has no default. Actual deletion and fallback follow
[ADR 0024: Effective alternatives and nullable characteristics](0024-effective-alternatives-and-nullable-characteristics.md).
Distinct final alternatives and importing consumers remain distinct candidates.

One definitive exclusion yields one new warning per candidate, even when several
leaf constraints reject it. Existing warnings remain independent. An unknown
possibly admitting alternative prevents exclusion; unrelated unfinished work does
not erase an established restriction's finding. Unsupported needed meaning remains
visible in the two area statuses. Completed work and absence of a warning do not
establish that the description admits any value, restrictive refinement or general
satisfiability. Documentary defaults are assessed without inserting them.

Distinguish aggregate unfinished branch meaning from a local owner failure within
the existing failure state. Only successful alternative membership can set this
aggregate cause, preserving an existing local cause. Consistency evaluation may
use it only on that collection's exact owner with no group establishment failure.
Copied nodes lose the group association and must establish their own membership;
the cause alone grants no eligibility. Keep ordinary-context, child-meaning,
feature, donor and candidate guards. The existing all-branch evaluator can then
retain one independent exclusion while the overall operation stays incomplete.

All candidates, descriptions and alternatives share one request equality counter
and the original remaining numeric, semantic and allocation counters. Charge loops,
scans, copies, retained capacities and retries before work; no phase refill or
refund is introduced. Preserve a real decisive candidate leaf or container as the
source primary, with effective operands, relevant origins and consumer locations.
If evidence cannot be retained, keep the single aggregate warning and mark its
detail omitted. Never substitute a donor's whole compound token for a derived
candidate. This preserves [ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md)
and [ADR 0013: Shared workflow and report guarantees](0013-shared-workflow-and-report-guarantees.md).

## Considered Options

A separate model validator would duplicate selected-value semantics and resource
rules. Turning model source JSON into a Sample handle would break source roles and
composed coordinates. Storing the description and root in every nested value would
enlarge repeated frames. Counting every rejected leaf would make advice totals
depend on evaluation detail. A general constraint solver would promise broader
reasoning than checking these declared values.
