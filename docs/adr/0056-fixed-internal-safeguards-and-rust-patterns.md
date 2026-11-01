# 0056: Fixed internal safeguards and Rust patterns

Esdieff uses fixed internal safeguards for input size and depth, reference
expansion, processing work and report growth. There is no established caller
need for configurable processing budgets. Remove the public `Limits` type,
quota setters and quota-selecting constructors instead of carrying
implementation choices into the caller's interface. Semantic request options
remain public. This supersedes the configurability requirement in
[ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md).
Its distinction between unfinished work, invalid models and omitted report
details remains in force.

Use pinned `corvus-regex-ecma262` 0.2.7 for Unicode `u` syntax and unanchored
matching, including Model checking, Sample-value checking, Generation and
Recorded trace checking. Retain the qualified guards for names, escape spelling,
decimal endpoints and the fixed supplied-size/structure ceilings. An explicit
100 ms cooperative backtracking timeout is attached to each compiled expression.
Corvus additionally enforces its compilation-size and backtracking-state
capacities. Unknown/unsupported capabilities and capacity or timeout exhaustion
leave affected work unfinished. Neither establishes invalid syntax or mismatch.

Compile and match directly through the supported Rust API, without a toolkit
pattern worker, JavaScript runtime, custom allocator or helper executable.
Independent calls do not share spent polling or duration allowances. The engine
may reuse thread-local working storage according to its own capacity policy.
A completed or exhausted call does not poison later calls. This supersedes
[ADR 0014: Bounded partial ECMAScript pattern capability](0014-bounded-pattern-capability.md)
and the worker/allocation decisions of
[ADR 0028: Bounded effective model pattern syntax](0028-bounded-model-pattern-syntax.md)
and [ADR 0045: One pattern engine per trace or generation request](0045-one-pattern-engine-per-trace-or-generation-request.md).
The effective scopes, source locations and incomplete-result semantics of
ADR 0028 remain unchanged.

The timeout is an internal policy, not a universal deadline. Some linear search
paths finish without consulting it, parsing and compilation are not externally
cancellable, and a batch can take proportionally longer. There is no exact
per-request memory, resident-memory or instruction-count guarantee. Existing
fixed internal counters remain useful protections, without claiming to account
for engine-private allocations.

## Considered options

Preserving QuickJS heap, JavaScript stack, worker-stack and interrupt settings
would constrain the replacement to implementation details without a demonstrated
caller benefit. A patched engine or supervised helper would add ongoing
maintenance, packaging and platform complexity. Corvus supplies the safeguards
needed for the current workflows through its released API. Rust regex lacks the
required lookaround/backreference semantics, while jlens-regress would require
more custom control and syntax integration. Strict host isolation and a new
public resource-budget interface are outside this decision.
