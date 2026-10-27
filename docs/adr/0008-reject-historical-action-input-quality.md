# 0008: Reject the historical Action input quality

Esdieff rejects `sdfRequiredInputData` at assessed SDF vocabulary positions with an explicit, helpful error. The pre-standard Action-input mechanism was [removed from the specification](https://github.com/ietf-wg-asdf/SDF/commit/03cb745147667701b88eaaab872900359ce32ac4), and its [legacy grammar production was subsequently removed](https://github.com/ietf-wg-asdf/SDF/commit/0becec2e3db9a32f78a61813a6dcd24b57e21dbd). Supporting its historical semantics would introduce compatibility behavior outside the current Action model.

The diagnostic identifies the unsupported historical quality and points to `sdfInputData`. Where input has multiple named fields, it explains the current `type: "object"`, `properties`, and `required` structure described in [RFC 9880, Section 5.3](https://www.rfc-editor.org/rfc/rfc9880.html#section-5.3). Migration requires reviewing the input description rather than renaming a keyword.

The final RFC still lists the name in its registry, and its framework syntax has liberal extension points. Consequently, this is an explicit [acceptance restriction](../../CONTEXT.md#conformance), recorded in [the agreed restrictions list](../deviations.md), rather than a claim that every occurrence is a mandatory RFC violation. Esdieff's selective restriction does not imply use of the entire closed validation syntax from Appendix A.

The restriction applies to assessed vocabulary positions, including document and information-block qualities, and to effective definitions after JSON Merge Patch. Catalogue checking still assesses supplied definitions independently. Given Names, sample-data entries, and arbitrary keys within opaque extension content are not vocabulary positions. Raw inspection and independent checks remain available when this error occurs.

## Considered Options

Preserving this historical quality as ordinary optional content would conceal a removed mechanism behind extension handling. Automatic conversion would require assuming the old model's intended input structure. Explicit rejection provides an actionable path to the current Action model.
