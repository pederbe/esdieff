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

A Recorded trace check and a generation request each keep one pattern worker
for the whole request. They lend it, with its runtime and context, to the
allowance of each entry or value in turn. This narrows ADR 0014, under which
every allowance owns the worker it uses. Only the worker moves. Each allowance
still pays its own worker stack and engine allocation and has its own polls and
cooperative duration, as ADR 0014 sets out, so a heavy entry or value still
leaves only itself undetermined.

A worker that has stopped is never lent again, so the next entry or value
starts its own. The request's worker joins when the request finishes. Other
operations keep one worker per allowance.

Each match builds its own regular expression, and afterwards the worker clears
any pending exception and collects garbage. A match that reaches a limit
therefore leaves nothing behind, and the engine is lent on as it is.

A new QuickJS context polls the interrupt handler on its first call and then
once every 10,000 calls. The allowance that starts the engine makes that first
poll as before. A lent engine has already made it, so each borrowing
allowance's first match calls the handler itself, once its first helper is
compiled and so never earlier than a new context would. Every allowance
therefore spends the polls a fresh engine would on its first match.

The later polls keep their 10,000-call rhythm across allowances, so a borrowing
allowance reaches its next poll within 10,000 calls rather than exactly 10,000
calls later. A match makes 7 calls, so about once every 1,429 matches the entry
or value being checked spends one poll more than it would alone. An entry with
no poll to spare can then be undetermined in a trace while it settles on its
own, as 2 of 3,000 copies of such an entry did. Polls never let an entry settle
in a trace when it would not on its own. The handler also checks the entry's
duration at those polls, so those checks move too. Duration is wall-clock time
and already varies between runs. Reports are otherwise unchanged. The
100,000-entry trace now takes 1.4 seconds.

## Considered Options

A fresh context for each allowance on a shared runtime would keep every
entry's polling identical to a separate request. Creating a full context costs
about 42 µs, most of the startup cost. A context with only the RegExp and eval
intrinsics still costs about 20 µs and would change the engine configuration
whose capability ADR 0014 established. A fresh runtime and context for each
allowance on one shared thread also keeps exact parity, but only brings the
100,000-entry trace down to 6.9 seconds.

Lending the engine without charging each allowance the first poll was as fast,
but let 1,427 of 3,000 entries settle that are undetermined on their own.

Keeping workers in a thread-local cache between requests would also speed up
other operations. It would leave a thread and an engine alive after the
caller's request returns, against ADR 0014 and
[ADR 0010: Domain-oriented library and caller simplicity](0010-domain-oriented-library-and-caller-simplicity.md).

One allowance for a whole trace would let a heavy entry leave the rest
undetermined, against ADR 0042.
