# 0029: Content-format declarations and recorded registry guidance

## Context

Authors need to distinguish a malformed complete declaration from valid spelling
whose registration or meaning is unknown. RFC 9880 Appendix A note 2 references
[RFC 9193 Section 6](https://www.rfc-editor.org/rfc/rfc9193.html#section-6), whose
decimal grammar has no digit ceiling; [Section 3](https://www.rfc-editor.org/rfc/rfc9193.html#section-3)
separately gives the Content-Format identifier range. A small convenient registry
subset would turn known dated positives into misleading unfamiliar results.
Parameter and payload meaning require work beyond declaration checking.

## Decision

An explicit fixed ten-area content-format assessment adds complete grammar and
dated registry guidance to the immutable initial inventory, following
ADR 0003: Check findings and completeness and ADR 0013: Shared workflow and
report guarantees. FullBaseline joins both declaration dimensions into M024 and
its pattern/format area while preserving producing condition identities.
Final effective Full coordinates define
the observation population, following ADR 0020: Owned effective views and nearest
origins and ADR 0024: Effective alternatives and nullable characteristics.
Overrides, deletions, critical-feature gates and Direct donor uncertainty retain
their established meanings.

Whole borrowed grammar classification precedes decimal range checking under
recorded interpretation I-CF-RANGE-1. Dated guidance uses all relevant positives
from pinned CoAP, media-type and HTTP-coding editions, with canonical identities
and temporary, obsolete and deprecated flags preserved. A positive-table miss
leaves current registration unknown; actual table use does not imply completed
lookup or current status. This follows ADR 0002: Recorded standards baseline.

UseRecorded and WithholdRecorded share the existing evidence-policy and borrowed
metadata seam established by ADR 0027: Unit names and recorded registry guidance.
Withholding is an embedding's evidence trust choice. It reports needed unavailable
evidence without pretending the compiled table is physically absent. Parameters
have explicit excluded-meaning Notices in ExpandedContentFormat. FullBaseline
additionally uses [ADR 0033: Content-format model meaning](0033-content-format-model-meaning.md)
for bounded parameter, registered tuple and ordered coding meaning; reserved identity remains interpretation
unknown. No AST, payload decoder, protocol binding or refresh machinery is added.

The [content-format contract](../content-format-contract.md) records the exact
grammar, extracted identities, dates, digests, status distinctions and conditions.
Mandatory aggregates and actual-use evidence survive complete optional omission.
Shared source-bound ownership, two complete DTO generations, old-result fallback
and independent output growth fit the unchanged admission reserves. Every new
semantic visit uses the original finite operation allowance, following
ADR 0009: Bounded processing and reporting.

A complete grammar/range traversal precedes registry replay over the same borrowed
derived coordinates. Operational completion remains separate from unknown
semantic premises; replay never clears earlier uncertainty or duplicates findings
and observations. A finished population with no reliable query needs omits replay.
This preserves later independent declaration errors when lookup exhausts work,
while deliberately spending additional traversal/classification credit and moving
first consultation later under a tight capability. The unit module's ordering and
the original allocation/work allowances remain unchanged.

## Considered options

A short popular-format allowlist was rejected because unfamiliar misses would
omit available dated positive knowledge. Runtime XML or registry refresh was
rejected because it would change reproducibility, evidence trust and capability.
A public parser AST or payload validation was rejected because callers need
declaration conclusions and preserved effective locations, not decoder machinery.
Giving declaration checking its own ad hoc Full membership was rejected because
it cannot establish a complete model inventory. The separate immutable model
inventory now supplies that relation. Parameter
semantics within the expanded inventory was rejected because syntax and registry evidence
cannot establish parameter-specific rules or payload compatibility.

Interleaving classification and lookup was rejected because a lookup stop can
leave later declarations unvisited behind a false completed syntax area. Marking
both areas unfinished on every lookup stop is a truthful smaller alternative,
but loses useful independent syntax completion and later spelling/range findings.
Retaining classifications for later lookup was rejected because it introduces a
growing population owner where charged borrowed replay suffices.

## Consequences

Callers opt into added checking without altering the initial meaning of scalar
getters or the old inventory. Reliable syntax errors remain blocking errors,
while missing registration evidence and unsupported meaning are incomplete.
Recorded positives and dated flags are Notices, with no current-status claim.
Tight finite capabilities can leave requested work unfinished; detail omission
alone cannot change semantic status. Table, backend, header or reporting graph
changes require renewed source-bound qualification.
