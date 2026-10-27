# 0043: SARIF and Markdown report projections

## Context

CI systems display code-scanning results from SARIF 2.1.0 logs as inline
annotations, and release notes are usually Markdown. Esdieff's JSON and human
reports carry findings with source byte spans and effective pointers, and
revision comparison differences with their classifications and
acknowledgements, but CI users must read them from job logs.

## Decision

`check` and `diff` can emit SARIF, and `diff` can emit a Markdown changelog.
Both are projections of the result the operation already produces. They change
no conclusion, disposition or exit code, and they render only the findings and
differences the operation retained, within the same byte limit, with omitted
items counted in the output. Each projection fits its own output to that limit,
so it can list a detail the JSON report dropped for space, or the reverse.

In SARIF, a check result is a retained error, warning or notice condition, and
a diff result is a retained difference: breaking as `error`, undetermined as
`warning` and compatible as `note`. A Version warning from
[ADR 0044: Versions of breaking revisions](0044-versions-of-breaking-revisions.md)
is a diff result at `warning` too. Incomplete work is a tool execution
notification rather than a result, since it is not a finding about the model.
Regions come from the retained source spans, with lines and columns counted in
Unicode code points and the byte range alongside. An effective pointer is a
logical location because it has no source position. An acknowledged difference
is a result with an external suppression, so reviewers still see the break
while CI treats it as intended.

The Markdown changelog groups differences by classification, puts acknowledged
differences in their own section and renders model text inertly.

## Considered Options

A separate SARIF-only analysis would duplicate the checking rules and drift
from the JSON report. Reporting incomplete work as warning results would let a
dismissed annotation hide unfinished checking. Hiding acknowledged differences
from SARIF would remove intended breaking changes from review entirely.
