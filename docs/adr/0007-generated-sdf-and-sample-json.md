# 0007: Generated SDF and sample JSON

Generated SDF prioritizes readable JSON and faithful known semantics, names, and numeric values. Permitted opaque extension content is retained, but retaining its structure cannot establish preservation of unknown semantics when definitions or reference contexts move. Exact reproduction of source whitespace and key ordering is a separate presentation-preservation capability.

Sample inputs use conventional JSON representations, including unpadded base64url for byte strings and numeric POSIX time for `unix-time`, as described in [RFC 9880, Section 4.7.1, Table 5](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.7.1). These representations are input conventions for abstract information-level data, rather than protocol payload bindings.

FullBaseline establishes the two known model interpretations without supplying
or searching for a value. M026 and M027 therefore do not depend on decoding a
byte sequence or assigning a particular POSIX coordinate. Actual constant
denotation and contextual equality still use the same conventional operands and
limits below. Model interpretation does not settle decoded-byte length or the
unavailable negative-coordinate and leap-instant meanings.

For `byte-string`, the decoded JSON text must use the URL-safe alphabet without padding or ignored whitespace. Empty text denotes an empty byte sequence. A final two-character group has zero low four bits; a three-character group has zero low two bits. These canonical images follow the base64url convention in [RFC 8949 Section 3.4.5.2](https://www.rfc-editor.org/rfc/rfc8949.html#section-3.4.5.2) and [RFC 4648 Sections 3.5 and 5](https://www.rfc-editor.org/rfc/rfc4648.html#section-5). JSON escapes are decoded before this check. Invalid text is rejected without repair, alternate-alphabet conversion or padding removal. Byte-sequence meaning does not establish a Unicode length or a decoded-byte-length constraint from an authored text quality.

For `unix-time`, available exact JSON numbers are usable representation inputs, including fractions, large coefficients and exponents, negative values and mathematical zero written `-0`. A nonnegative number establishes its exact POSIX coordinate; no machine-width, binary floating-point or host calendar conversion is needed. Negative nonzero inputs retain exact representation and consultation, but their abstract denotation remains undetermined under this convention. This limitation does not itself reject the sample. An independent integer, null or other applicable constraint can still reject it. Information-level numeric coordinates do not certify the separate formal CDDL or any protocol binding.

Equality requires both operands' denotations in a common compatible context, following [ADR 0015: Bounded value equality in a resolved data context](0015-value-equality.md). Canonical byte text has one image per sequence, so comparing two established images establishes equality or difference without allocating decoded bytes. Established POSIX coordinates use exact numeric equality and the original remaining numeric allowance. Representation failure, unknown denotation and exhausted work retain distinct causes.

## Considered Options

Requiring exact source presentation for every generated document would couple initial authoring to existing-document editing needs. Supporting multiple sample encodings from the outset would add representation choices before establishing the conventional SDF workflow.

Accepting noncanonical tails or repairing input would add alternate images beyond the selected channel. Rejecting all negative numeric input would confuse representation with unresolved denotation; assigning it a signed civil-time meaning would settle an additional convention. Limiting coordinates to host dates, fixed-width integers or floating point would reject or merge exact inputs that remain usable under the existing numeric capabilities.
