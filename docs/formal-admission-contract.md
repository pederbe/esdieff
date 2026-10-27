# Formal model admission

FullBaseline checks M035-M037 of `rfc9880.model/1` against the imported domains in
[RFC 9880 Appendix A](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-A),
following [ADR 0032: Formal model admission](adr/0032-formal-model-admission.md).
Formal admission is grammar membership, separate from
[model meaning](model-meaning-contract.md), actual const denotation, sample
admission and general satisfiability. Initial and expanded assessments, source
inspection, unchanged generated-token preservation and selected-sample workflows
keep their existing exact-decimal contracts.

## Number and count domains

`number` admits every JSON number, as an exact decimal without a precision or
range limit. `uint` is the integer interval 0 through 2^64-1.
Integrality is a mathematical property: `10`, `10.0`, `1e1` and `100e-1` have
the same membership. Negative zero is zero. No decimal is rounded into a machine
value for either check.

| Supplied mathematical value | `uint` | `number` |
| --- | --- | --- |
| `0`, `-0`, `10.0`, `1e1` | admitted | admitted |
| `18446744073709551615` | admitted | admitted |
| `18446744073709551616`, `-1` | excluded | admitted |
| `0.5`, `0.1`, `5e-2` | excluded | admitted |
| `1e400`, `1e-400` | excluded | admitted |

These results apply to successfully decoded exact numbers with sufficient work
allowance. An operand beyond the 512-digit decoding capability leaves membership
undetermined. It does not establish exclusion. Exact mathematical
processing and retained source tokens remain governed by
[ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md).

M036 checks the numeric parameters admitted in Full descriptions and Direct
`minimum` and `maximum` entries. M037 checks Full lengths and array counts, Direct
lengths and grouping repetition counts. A JSON number always satisfies M036. A
known nonnegative integral count above 2^64-1 in a colon parameter produces a
definite formal grammar error. Its owning requirement is assessed,
and independent exact bound, type or count meaning remains assessed. Completed
work can therefore have a failed disposition.

Object and Thing repetition entries use `=>`, allowing fallback to framework
extension content. Full treats a known negative, fractional, nonnumeric or too
large repetition value as admitted grammar: M037 is assessed, while M032 base
repetition meaning and M028 extension meaning are undetermined. These cases
produce no Full grouping-count grammar error. Initial and expanded grouping
checks retain their narrower behavior.

## Declared literals

Full `const` and `default` use `allowed-types`, including an `any` feature
alternative. M035 is assessed when either base or framework-extension membership
is established. The base alternatives are JSON numbers, decoded text, Boolean,
null, homogeneous arrays of numbers, text or Boolean, and text-key maps with
unrestricted values. An empty array is base-admitted.

| Literal | Framework admission | Semantic consequence |
| --- | --- | --- |
| `0.1`, `[0.1,2]`, `["a","b"]`, `[true,false]` | base alternative | ordinary meaning can be established independently |
| `{"x":[[0.1]],"n":1e400}` | base map with `any` values | no recursive numeric restriction on map values |
| `[null]`, `[1,"x"]`, `[[1]]`, `[{}]` | extension alternative | M028 and needed final const meaning M043 remain undetermined |

An extension-only default remains documentary and is not inserted. An
extension-only const overridden by every final alternative does not constrain
those alternatives. An extension-only const in an optional property still
prevents claiming that property's complete model meaning. Framework admission
does not establish a registered extension interpretation or successful contextual
equality. Unknown decoding, feature or effective context leaves needed membership
undetermined.

## Scope, limits and evidence

Formal checks examine effective definitions in their actual roles. Source patch
nulls that delete an inherited parameter impose no final constraint. Selected
root requests exclude unrelated supplied definitions; catalogue requests retain
their wider scope. A finding on an inherited operand names the effective consumer
and relates its original source span, preserving the authored number token.

All operands and phases share the original semantic, numeric and allocation
allowances. Count comparisons avoid expanding authored exponents. No operand,
child or replay replenishes a counter. Capability or work failure leaves only
affected work unfinished. Human and JSON output share the same mandatory error
and completion totals; omitted diagnostic detail does not erase established
findings or make assessed membership unfinished.
