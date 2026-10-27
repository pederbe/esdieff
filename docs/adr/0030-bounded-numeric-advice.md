# 0030: Bounded numeric advice

Model authors benefit from local contradictions that ordinary interval comparison
does not find, including fractional bounds with no integer and intervals with no
multiple of a positive step. Provide an explicitly selected assessment, using the
effective Full data descriptions established by [ADR 0020: Owned effective views
and nearest origins](0020-owned-effective-views-and-nearest-origins.md) and
[ADR 0024: Effective alternatives and nullable characteristics](0024-effective-alternatives-and-nullable-characteristics.md).
Numeric constraints follow [RFC 9880 Appendix C.1](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.1).

`esdieff.expanded.numeric/1` contains the unchanged Initial eight areas and
`advisory-numeric-three/1`. Its three Warning/Advice classes are an integer interval
gap, a sole numeric const excluded by each applicable bound, and a positive-step
interval gap. These are local sufficient contradictions, without a general
satisfiability, restrictive-refinement or sample-admission conclusion. Full keeps
its fixed model obligations and existing advisory area; it does not implicitly
request these new classes.

Proofs require final ordinary meaning and the applicable Full role. An established
absent type permits the numeric const comparison only. A malformed or unavailable
type is not absence. Interval proofs require established integer/number type as
appropriate and null exclusion, supplied by nullable false or the independently
known nonnull root kind of the sole ordinary const. Unavailable const magnitude
does not erase an established root kind. Documentary default supplies no guard.
Donor interpretation, unsupported qualities and unestablished structural meanings
cannot be inferred from source JSON kinds or spelling. Derived Choice alternatives
are checked separately; no union proof is added. The [numeric advice contract](../numeric-advice.md)
states all applicability and support boundaries.

Evaluate every surviving const bound independently. Select the strongest known
lower and upper bounds for interval work, with exclusion winning exact ties. A
bound with established restrictive meaning but unavailable magnitude leaves the
affected work undetermined while a contradiction using independent known bounds
can survive. Unknown denotation cannot be presumed restrictive.

Use borrowed normalized decimals and bounded virtual long division, without
floating point, a large-integer dependency, generated public number tokens or
exponent-sized storage. Divide each endpoint by the positive lattice step, round
the lower quotient upwards and upper quotient downwards, then adjust integral
exclusive endpoints by one. No lattice value exists when the lower integer index
exceeds the upper. For integer type with rational decimal step p/q in lowest
terms, use the common integer lattice pZ; finite coefficient factor removal cancels
powers of two and five.

Keep class outcomes independent: an unavailable positive step can leave its
attempt unfinished while a supported integer interval contradiction remains a
warning. The one numeric area's status is undetermined until every applicable
class is completed; absence of a warning supplies no satisfiability conclusion.

Virtual numerator and denominator lengths are at most 1024 digits; adjusted
quotients have at most 1025 digits. Factor storage has at most 513 digits. A
magnitude-first shortcut handles a quotient smaller than one before virtual-width
admission. Successful factor removals are bounded by the finite coefficient,
without a loop counting to the exponent. Capability limits leave work
undetermined and never establish a gap.

All traversal, comparisons, digit scans, carry/borrow, factor trials, buffer
acquisition and finding growth debit the original remaining operation and numeric
counters before work or allocation. Sequential classes and descriptions share
those counters. No refill, refund or allowance increase is introduced. Existing
mandatory report owners contain the added area and aggregates with zero additional
numeric reserve. Optional conditions retain at most four original operand
identities, their final effective coordinates, nearest authored contributions and
consumer origin. Absent type contributes no invented argument or location.
Numeric human fallback destroys the first result before constructing its
replacement; retained model and finding owners remain accounted for.

## Considered Options

A general constraint solver would expand the public claim and the supported
language considerably. Reusing supplied-candidate divisibility cannot establish
interval existence. Floating point would lose exact endpoints and decimal steps.
Expanding powers into strings would make accepted exponents select unbounded
allocation. Per-class budgets or a larger report reserve would conceal failures
of the original shared owner contract. Treating every familiar quality as harmless
would miss its role, interpretation and structural premises.
