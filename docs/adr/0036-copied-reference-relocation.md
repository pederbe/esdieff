# 0036: Copied reference relocation

## Context

[RFC 9880 Section 4.4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.4)
copies a referenced definition and applies the composition patch through JSON
Merge Patch. It does not say what name references inside the copy mean
afterwards. Sections 4.3 and 4.4.1 also leave open whether a name reference
selects an element of the document as written or of its resolved model.
Reusable Objects commonly require their own declarations through absolute
pointers such as `#/sdfObject/alarm/sdfProperty/status`, so reusing one through
`sdfRef` copies a pointer to the source's coordinates.

## Decision

Esdieff adopts this RFC interpretation as `copied-reference-relocation/1`,
following [ADR 0002: Recorded standards baseline](0002-recorded-standards-baseline.md).

A copied reference keeps the document and namespace mappings of the definition
that authored it. When its target, read in that context, is the copied
definition or lies beneath it by decoded JSON Pointer tokens, it is an internal
copied reference. It relocates to the corresponding position in the importing
definition. This applies to same-document and CURIE forms alike and at every
copy boundary, including a target's root chain and nested copies. Other copied
references keep their original target. Relocation starts from the target the
reference has in its authoring context, so a copied reference whose target is
missing there stays missing in every copy, even when the importer declares
something at the corresponding position.

A reference written in a composition patch belongs to the importing author and
uses that author's context, even when its text equals the inherited one. Only
the `sdfRef` member or `sdfRequired` array actually written in the patch counts.
Patching sibling qualities leaves a copied reference copied.

Every name reference selects the element at its pointer in the target
document's resolved model. An importer can therefore reference and require
declarations that exist only through a copy. A relocated target that a patch
deleted is missing, as for any other reference, under
[ADR 0006: Scoped reference failures](0006-scoped-reference-failures.md) and
[ADR 0020: Owned effective views and nearest origins](0020-owned-effective-views-and-nearest-origins.md).
Nothing falls back to the source definition. Cycles created by relocation or by
composing a pointer's path are scoped failures.

Composing a pointer's path is an exception to the explicit frames of
[ADR 0020: Owned effective views and nearest origins](0020-owned-effective-views-and-nearest-origins.md).
A lookup composes a definition on its path, and that composition can need
another lookup through another composed definition, so nested path compositions
recurse. Each nested composition is reused for the rest of the operation, and
the nesting stops at a fixed bound of 32 levels, or at the reference depth when
that is lower. Exceeding the bound leaves the affected result undetermined with
`operation.limit`, following
[ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md).
The bound is a supported stack limit rather than an allowance, and raising the
reference depth does not raise it. Recursion keeps lookup on the existing
composition steps, while explicit frames would need a resumable lookup inside
every expansion. Ordinary models nest only a few levels, and 256 levels of
recursion can exhaust a 2 MiB thread stack.

Copied `sdfRequired` pointers relocate by coordinate. A relocated pointer
selects the declaration at that coordinate in the resolved model, with its own
Declaration contribution under
[ADR 0026: Required declaration relationships and source evidence](0026-required-declaration-relationships-and-source-evidence.md).
A deleted declaration leaves the relationship missing, and a reinserted one is
selected as a new contribution. Referenceable names and `true` entries keep
their carrier-relative meaning. Boolean and short-name `sdfRef` keep no
processing meaning.

`baseline.interpretations` lists the identifier whenever a result relied on
this interpretation. That is when requested work relocated a copied reference,
read a copied CURIE reference with its authoring document's namespace mappings,
or found that a name reference target in the resolved model differs from the
document as written, including a target that composition removed. Reports that
did none of these omit it.

## Considered Options

Binding a copied reference to its source target keeps the source's constraints
and ignores the importer's refinement of the very data its copy uses. Reading
inherited pointers literally in the importing document selects unrelated
lookalike definitions, or nothing. Requiring authors to restate every inherited
reference breaks reuse of ordinary models that require their own declarations
by pointer. Keeping copied references undetermined left that common reuse
incomplete, and an opt-in switch would keep that default while doubling the
tested behaviour. String prefix matching would treat a sibling such as
`BaseExtra` as inside `Base`. Limiting resolved-model lookup to `sdfRequired` or
to same-document pointers would let two spellings of one global name disagree.
Classifying a reference by the map that holds it would let an unrelated sibling
patch silently rebind it. Relocating by pointer structure alone, even when the
source lacks the target, would make one copied reference broken in its source
yet valid in each importer that happens to declare the missing element.
Recording the identifier only for relocation and composed-only targets left
passing results that read a copied CURIE in its authoring context, and missing
targets that composition removed, without naming the reading they rely on.
