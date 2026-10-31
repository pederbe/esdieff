# 0050: Selected Markdown model reference

## Context

A reader needs the effective interface of one Thing or Object, including nested
interactions and metadata, without tracing reuse across supplied SDF documents.
Inspection already supplies typed effective views and nearest content origins.
Generation already settles candidates through Sample-value checking.

## Decision

A Model reference selects one effective grouping and documents its surviving
contents in fixed Markdown. `ReferenceRequest::new(document, pointer)` and
`Catalogue::reference` expose the operation to library callers. The CLI acquires
explicit inputs, selects the document and writes the same Markdown to stdout.

Composition and declaration-requiredness inspection share one request allowance.
Traversal uses the effective typed views, including effective Choice alternatives
and Direct items role boundaries. Context remains non-interactive metadata under
the recorded draft. Unsupported meaning and absent Context nullability are
visible documentary states, independently of documentation completion.

Each Property, Action input/output, Event output and Context description may
receive one illustrative value. The fixed Generation contract selects the first
retained admitted value no larger than 4 KiB. Construction and settlement keep
the allowances, candidate order and 64-value ceiling of
[ADR 0041: Generated values settled by Sample-value checking](0041-generated-values-settled-by-sample-value-checking.md).
The request searches at most 1,024 descriptions and shares one pattern worker,
following [ADR 0045: One pattern engine per trace or generation request](0045-one-pattern-engine-per-trace-or-generation-request.md).
An unavailable example does not prevent sibling documentation or alone make a
reference partial. Declared defaults remain documentary data.

Generated ASCII anchors identify definitions and source entries by traversal
order. Model text is inert. Origins identify source labels, pointers and retained
line/column positions, separately from effective pointers. The document travels
without machine-specific links or a generation timestamp.

Mandatory identity, source attempts, status and omission summaries reserve space
before composition. Optional rows and sections are retained whole under the
report byte allowance. Work, depth, pointer and allocation limits produce explicit
partial results. A definite requested processing failure takes precedence over
partial documentation. Neither completion nor an admitted example establishes a
Full model assessment. Existing operations and serialized report versions retain
their contracts.

## Considered options

A template system or another SDF interpreter would duplicate supported machinery
and require additional contracts. HTML and a new JSON reference schema would add
formats without helping the initial Markdown workflow. Showing arbitrary
candidates as examples would misrepresent rejected or undetermined values.
