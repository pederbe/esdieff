# 0042: Recorded traces with independent entries

## Context

Teams collect interaction values from devices, apps and test runs, and want to
know which ones a model rejects. `esdieff sample` checks one value per run,
composes the model each time and spends one of the 256 source attempts per
value. The door-lock demonstration's Simulation trace has authored sequence
rules, which recorded data lacks.

## Decision

A Recorded trace is a JSON Lines file of Trace entries. Each entry names a
Sample selector within one selected document, carries a value and may carry an
Expected admission and a label. Esdieff composes the model once, resolves each
distinct selector once and checks every entry independently, with the
allowance one Sample request would receive, so one heavy entry cannot starve
the rest.

Line order is presentation only. A Recorded trace adds no sequence meaning, no
timing and no protocol binding, and its checks add nothing to any Model
requirement inventory.

An entry passes when its value is admitted with nothing else expected, or when
its definite admission equals its Expected admission. It fails when its value
is rejected with nothing expected, when its definite admission differs from the
expectation, or when the entry itself is malformed or selects nothing. An
undetermined admission stays undetermined whatever was expected, and so does a
line the JSON decoder cannot read, such as one that is not UTF-8, following the
input policies of
[ADR 0012: Exact numbers and JSON input policies](0012-exact-numbers-and-json-input-policies.md).
One bad line
never stops the run. The trace's disposition is failed for any failed entry,
otherwise undetermined for any undetermined entry, otherwise passed, so exit
codes follow `check`.

The trace file is a source with role `sample`, and conditions locate their
entry by byte spans in that file. Generated values written as Trace entries,
with their settled admission as the expectation, replay through the same check,
which turns them into regression vectors for later Model revisions.

## Considered Options

One JSON array of samples would need the whole file decoded before the first
check and would hide which line failed. Accepting each line as a separate
Sample source would spend the source-attempt ceiling and recompose the model
per value. Treating an undetermined entry as passing when a rejection was
expected would claim a conclusion the check did not establish. A sequence-aware
trace would need authored behavior rules, which belong to demonstrations such
as the door-lock simulation.
