# 0045: One pattern engine per trace or generation request

## Context

[ADR 0014: Bounded partial ECMAScript pattern capability](0014-bounded-pattern-capability.md)
gives each operation allowance its own pattern worker. The worker starts a
QuickJS runtime and context on first use and joins when its allowance is
dropped.
[ADR 0041: Generated values settled by Sample-value checking](0041-generated-values-settled-by-sample-value-checking.md)
and
[ADR 0042: Recorded traces with independent entries](0042-recorded-traces-with-independent-entries.md)
give every [Generated value and Trace entry](../../CONTEXT.md#model-work) the
allowance of one Sample-value checking request. Each checked value against a
`pattern` therefore started and stopped a worker and engine of its own. That
took about 75 µs, against about 9 µs for the match itself. A Recorded trace of
100,000 entries on the door-lock example's credential property took 9.3
seconds, against 0.13 seconds for the same number of entries on a numeric
property.

## Decision

A Recorded trace check and a Generation request each keep one pattern worker
for the whole request. They lend it, with its runtime and context, to the
allowance of each entry or value in turn. Only the worker moves. Each allowance
still pays its own worker stack and engine allocation and has its own polls and
cooperative duration, as ADR 0014 sets out, so a heavy entry or value still
leaves only itself undetermined.

After a match that reaches a limit, the worker discards its engine and the next
match starts a fresh one. A worker that has stopped is never lent again. The
request's worker joins when the request finishes. Other operations keep one
worker per allowance.

The context's own interrupt counter carries over from one entry to the next.
QuickJS counts JavaScript calls and polls the interrupt handler after 10,000 of
them, and a match makes only a few calls. An entry can therefore occasionally
spend one extra poll of its allowance, as a check with several matches already
can under ADR 0014. Reports are otherwise unchanged. The 100,000-entry trace
now takes 1.4 seconds.

## Considered Options

A fresh context for each allowance on a shared runtime would keep every
entry's polling identical to a separate request. Creating a full context costs
about 42 µs, most of the startup cost. A context with only the RegExp and eval
intrinsics still costs about 20 µs and would change the engine configuration
whose capability ADR 0014 established.

Keeping workers in a thread-local cache between requests would also speed up
other operations. It would leave a thread and an engine alive after the
caller's request returns, against ADR 0014 and
[ADR 0010: Domain-oriented library and caller simplicity](0010-domain-oriented-library-and-caller-simplicity.md).

One allowance for a whole trace would let a heavy entry leave the rest
undetermined, against ADR 0042.
