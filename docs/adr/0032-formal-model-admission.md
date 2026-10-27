# 0032: Formal model admission

FullBaseline checks the formal numeric and literal domains imported by
[RFC 9880 Appendix A](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-A).
These grammar obligations are separate from the exact information-level meaning
of numbers in [Appendix C.1](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.1),
following [ADR 0012: Exact numbers and JSON input policies](0012-exact-numbers-and-json-input-policies.md).
Initial, expanded assessments, source inspection and selected-sample admission
retain their existing scope and exact-decimal behavior. The
[formal admission contract](../formal-admission-contract.md) records the boundaries.

Imported `number` admits every JSON number, read as an exact decimal without a
precision or range limit. [RFC 8610 Appendix E](https://www.rfc-editor.org/rfc/rfc8610.html#appendix-E)
maps CDDL onto JSON and describes JSON numbers as decimal, without limits to
their precision or range. It also calls a floating-point precision restriction
unlikely to be useful for JSON. RFC 9880 itself uses a decimal that binary64
cannot express: [Figure 5 in Section 6.2.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-6.2.1)
overrides a length with `"minimum": 5e-2`. This is Esdieff's RFC
interpretation. Later guidance from the RFC authors would be evidence for
revisiting it, not a precondition for applying it.

Imported `uint` admits integral values from 0 through 2^64-1, following the
[prelude](https://www.rfc-editor.org/rfc/rfc8610.html#appendix-D). Fractional or
exponential notation does not change an integral value's membership. No
supplied decimal is rounded to establish admission. For example, `0.1`, `5e-2`
and `1e400` are numbers, `10.0` is a `uint`, and 2^64 is a `number` but not a
`uint`.

Full and Direct numeric parameters, lengths and Full array counts use colon
entries in Appendix A. An established value outside their imported domain is a
formal grammar error: a colon entry cuts off fallback to map extension entries,
as specified in [RFC 8610 Section 3.5.4](https://www.rfc-editor.org/rfc/rfc8610.html#section-3.5.4).
Every JSON number satisfies a numeric parameter, so only lengths and counts can
be excluded.
Grouping repetition counts instead use `=>`. Their non-`uint` values, including
negative, fractional and nonnumeric values, can fall through to the framework's
extension entries. Full records known grammar admission and unavailable base
repetition and extension meaning. It does not retain the narrower grouping-count
error for those fallback values. No independent normative prohibition has been
adopted for them.

The `allowed-types` union for Full `const` and `default` includes the feature
alternative `any`. Known content outside the base alternatives is therefore
framework-admitted, with extension meaning and applicable final const meaning
left undetermined. Homogeneous number, text or Boolean arrays are base
alternatives; nested, mixed or null-element arrays use the extension alternative.
Text-key maps have unrestricted values, so their nested numbers and arrays do not
inherit the scalar number or array restrictions. The feature annotation describes
an extension alternative without itself requiring a named critical feature,
following [RFC 9165 Section 4](https://www.rfc-editor.org/rfc/rfc9165.html#section-4).

`number` membership needs no arithmetic once a number is decoded. One private
exact predicate compares a decoded count with virtual decimal digits for `uint`,
which avoids a large-integer dependency, rounding and exponent-sized expansion.
The role-aware literal classifier stays beside the existing effective traversal,
without a second JSON model or public interface.

Use the original request's remaining semantic and numeric counters, and charge
scans and comparisons before performing them. Decoding limits, unknown critical
context, unavailable donor meaning and exhausted work leave the affected
obligation undetermined; they never become domain exclusions. Source composition
patches do not impose deleted operands on the effective model. Selected and
catalogue scopes retain their existing boundaries, and inherited findings retain
both their effective locations and original source operands. Optional detail
omission preserves aggregate errors and completed work, following [ADR 0009:
Bounded processing and reporting](0009-bounded-processing-and-reporting.md).

## Considered Options

Restricting `number` to integers from -2^64 through 2^64-1 and values exactly
expressible in binary64 follows RFC 8610's float types literally and was the
earlier decision. It made RFC 9880's own Figure 5 a grammar error, and it
rejected ordinary decimal steps in real models, such as `multipleOf: 0.1` with
unit `s` and `multipleOf: 0.005` for a 5 ms delay step. Sample checking already
treated those values exactly. Rounding through binary64 would change the
supplied value and accept rounded large integers. Rejecting every extended
literal or grouping fallback would replace the normative framework grammar with
stricter validation syntax. Leaving known extension admission unfinished would
confuse grammar membership with unavailable semantics. Extending these checks to
every workflow would change the narrower Initial, expanded and sample contracts.
A general CDDL interpreter or arbitrary-precision dependency would add
unnecessary surface for these fixed domains and shallow literal alternatives.
