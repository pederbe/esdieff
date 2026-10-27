# 0037: Undeclared extension vocabulary

## Context

[RFC 9880 Section 2.3.3](https://www.rfc-editor.org/rfc/rfc9880.html#section-2.3.3)
makes Quality Names an extension point, and
[Section 3.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.1) gives
the `features` quality for listing critical extension features that cannot be
safely ignored. Esdieff previously let any quality outside base SDF stop the
base meaning of the definition carrying it. One `oma:id` on a Property left its
type, constraints and Sample admission undetermined. Every model converted from
OMA LwM2M carries such a quality, and working-group extensions such as protocol
mapping add unqualified qualities like `sdfProtocolMap`.

## Decision

Esdieff adopts this RFC interpretation as `undeclared-extensions-ignorable/1`,
following [ADR 0002: Recorded standards baseline](0002-recorded-standards-baseline.md).

Undeclared extension vocabulary is a Quality Name outside base SDF, used in a
document whose `features` declares no Extension feature. It leaves the base
meaning of its definition intact. Base conclusions, including types,
constraints, requiredness, composition and Sample admission, proceed as if it
were absent. Its own meaning stays unsupported unless it is Recorded extension
vocabulary under
[ADR 0038: Recorded extension vocabulary](0038-recorded-extension-vocabulary.md).
Its notices stay as they are. A qualified name keeps
`extension.optional_unsupported`, and an unqualified name keeps its
`quality.unfamiliar` warning, so a misspelled base quality is still reported.
Composition copies and merges these qualities like any other, and inspection
shows them.

The rule covers qualified and unqualified names alike. Working-group extensions
use unqualified names, and the RFC reserves the cannot-ignore treatment for
declared features.

A base Quality Name in a position whose role does not support it is not
extension vocabulary. Wider Direct item descriptions and other role questions
keep their existing outcomes. A document that declares any Extension feature is
still deferred as a whole under
[ADR 0003: Check findings and completeness](0003-check-findings-and-completeness.md),
and so is any result whose composition depends on such a document.

`baseline.interpretations` lists the identifier whenever a reported conclusion
was established for a description carrying undeclared extension vocabulary.
Reports that relied on no such description omit it.

## Considered Options

Keeping every unknown quality blocking was safe, but it left every OMA-derived
model and every protocol-mapping model without base meaning, although the RFC
already offers `features` for extensions that do change it. Ignoring only
qualified names would guard against misspelled base qualities, but it would keep
working-group extensions blocked. A misspelling already receives a warning, and a
strict warning policy turns that warning into a failure.
