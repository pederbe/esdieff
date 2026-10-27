# Agreed deviations and acceptance restrictions

This document records explicitly agreed departures from [RFC 9880](https://www.rfc-editor.org/rfc/rfc9880.html) and narrower Esdieff acceptance policies. A policy rejection is distinguished from a definite violation of an RFC requirement.

## Deviations from RFC requirements

No departures from RFC requirements have been agreed.

The selected T/t civil representation convention and conservative date-free exceptional-label interpretation are recorded in [ADR 0018: Civil date and time sample conventions](adr/0018-civil-date-time-sample-conventions.md). A selected-representation error uses Interpretation basis; unavailable date-free or leap knowledge uses Operational incompleteness. Neither creates an additional acceptance restriction or an RFC-invalid diagnosis for an unestablished abstract value.

## Acceptance restrictions

**A001: Reject `sdfRequiredInputData` as an SDF quality.** Esdieff reports an explicit error with guidance toward the current Action-input structure. This historical mechanism was removed, although the final RFC still registers its name and permits extension content. The restriction applies at assessed SDF vocabulary positions; it does not reject Given Names or arbitrary data keys with the same spelling. The reason and checking scope are recorded in [ADR 0008: Reject the historical Action input quality](adr/0008-reject-historical-action-input-quality.md).

**A002: Refuse duplicate decoded JSON names.** Model documents and conventional JSON samples cannot construct a usable map when an object repeats a decoded member name, including in nested or opaque content. Names compare exactly after escape decoding, without case folding or Unicode normalization; repeated identical values also fail. Esdieff retains the supplied source and identifies the repeated member, without choosing first or last precedence. This is an input acceptance policy, not a claim that duplicate names violate JSON syntax. The reason, finite limits, and independent-document behavior are recorded in [ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md).
