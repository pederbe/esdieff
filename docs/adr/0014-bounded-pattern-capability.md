# 0014: Bounded partial ECMAScript pattern capability

The engine, runtime and resource-control decision is superseded by
[ADR 0056: Fixed internal safeguards and Rust patterns](0056-fixed-internal-safeguards-and-rust-patterns.md).
The text below retains the historical decision. Effective scopes, semantic
results and report-detail distinctions continue where not superseded.

Esdieff interprets the `pattern` data quality using [ECMA-262 2025](https://tc39.es/ecma262/2025/multipage/text-processing.html#sec-patterns) Unicode `u` syntax and unanchored matching, as required by [RFC 9880 Appendix C.2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.2). Its suggested portable subset does not restrict otherwise permitted syntax.

An initially partial capability uses pinned regress 0.12.0 with optimizations disabled for syntax, and pinned rquickjs 0.14.0 with bundled QuickJS-ng 0.16.2 for matching. Matching guards, parsing, engine initialization, helper evaluation, compilation, matching and destruction occur on one private Rust worker with a 4 MiB stack per operation allowance. The worker starts lazily, reuses its runtime and context, and joins when the allowance is dropped. Syntax checks share the worker without initializing an engine. Supplied patterns and subjects are values passed to fixed internal functions, and the toolkit owns the runtime and allocator choice.

All duplicate decoded capture names, including otherwise valid disjunction duplicates, are unsupported. Capture-name escapes the guard cannot decode reliably are also unsupported. Active Unicode escapes are independently checked for the hexadecimal spelling required by the standard, including inside character classes, capture names and named backreferences. This excludes signed forms erroneously admitted by the pinned parser's integer conversion while preserving escaped literal backslashes and valid Unicode spellings. An escaped greater-than sign inside a name is independently invalid under the identifier early errors, even though the pinned parser treats it as a delimiter; ordinary and class escapes for that character remain valid. Decimal quantifier endpoints are compared without machine-integer conversion: reversed bounds establish invalid syntax; endpoints at or above 1,000,000 exceed the capability. Unknown parser or engine rejection does not automatically establish invalid syntax.

Maximum supplied sizes are 4 KiB of pattern text and 1 MiB of subject text, measured in UTF-8 bytes. Maximum group nesting is 64 and maximum total alternatives is 512. The engine's configured allocation limit is 16 MiB and its stack limit is 256 KiB, with the remainder of the 4 MiB worker stack reserved for Rust and engine frames. The allocation limit checks requested growth before allocator rounding and bookkeeping, so it is not an exact ceiling on the engine's reported allocation counter. Configuration may only reduce supported size, structural, allocation and stack settings. Zero cannot disable these bounds.

Matching initially supports decoded capture names of at most 124 UTF-8 bytes. This conservative engine capacity is applied after syntax admission, so a valid longer name can remain syntactically valid while matching is incomplete. Escape spellings are charged by their decoded UTF-8 bytes. The engine permits at most 254 explicit capturing groups because its 255 capture slots include the whole match. Its named-group scope counter has a maximum of 255 and can be exhausted by ordinary alternatives when any named capture is present. Those known capacities are engine compile limits, not syntax violations. Other `invalid group name` exceptions retain their distinct unsupported classification.

Regexp work uses a request-wide default allowance of 100 polls, with a maximum configurable allowance of 10,000. Each engine call spends one entry poll, and interrupt callbacks spend the remainder. The next poll interrupts execution. Entry polling keeps short matches bounded even when the reused runtime does not reach its internal callback interval. Its accumulated cooperative engine duration defaults to 25 milliseconds and may be configured up to one second. Zero allowance or duration returns a work limit. Alternatives, array elements and declared-value candidates receive no fresh allowance. These count engine polling and cooperative elapsed time, not exact instructions or a hard wall-time deadline across parsing and compilation.

A minimal-limit runtime probe at engine startup and engine allocation source checks establish gross enforcement of the configured allocation setting for this dependency pin, including feature-unified embeddings. If the linked runtime cannot establish enforcement, matching is unsupported. The toolkit accepts no caller runtime or custom allocator. The promise excludes allocator rounding, bookkeeping overhead, total resident memory, arena reserve overhead and Rust allocations. Parsing and regexp compilation have finite supplied sizes and structures but lack externally enforced cancellation.

Selected text samples, choices and declared-value consistency use the same matching implementation through the library's ordinary operations. Syntax and matching share one paid sequential worker stack slot from the original operation allocation. Matching reserves its configured engine allowance from that same allocation before engine work. The allowance owns the worker, which owns every engine object. Copied request text is admitted against the original allocation allowance before transfer. Each match compiles its own expression, clears pending exceptions and collects garbage before the context is reused. No runtime, compiled expression or source string escapes to callers. Successful matching fills the fixed text obligation of the existing sample inventory without establishing model-wide pattern meaning or general satisfiability.

FullBaseline establishes available model matching meaning through compilation
with that same implementation and its qualified guards. No subject or match is
needed: the obligation concerns the adopted Unicode, unanchored interpretation
and available capability, rather than existence of a satisfying value. This
compile-only operation retains the original work, allocation, worker-stack,
engine-stack, interrupt and cooperative-duration allowances. Matching-only
capacities can therefore leave M048 unfinished independently of M047 syntax.
Successful compilation does not guarantee completion for every future subject
or add external cancellation to compilation.

Resource exhaustion during worker startup, joining, engine initialization, helper evaluation, compilation or matching leaves affected conclusions incomplete under ADR 0009: Bounded processing and reporting. Independently established syntax errors remain RFC findings; unsupported capabilities and operational limits are neither syntax violations nor acceptance restrictions. Updates require renewed capability evidence rather than silently changing the standards baseline or claiming complete conformance from a finite fixture set.

The toolkit owns thread and runtime lifecycle so callers receive pattern conclusions without executable discovery, JavaScript or resource bookkeeping, following ADR 0010: Domain-oriented library and caller simplicity. Reusing one worker and runtime avoids repeated startup for sequential pattern checks. Expressions remain uncached so retained state and cache policy do not grow with model size. Broader support and expression caching require separate evidence.

## Considered Options

Rust regex and fancy-regex directly change Unicode class and backreference semantics. Their finite matching controls do not establish ECMAScript fidelity. OXC exhibited relevant false-invalid and false-valid syntax outcomes, so it is not a syntax authority for this capability.

Regress in a supervised Rust process can avoid a C matcher and enforce external termination across syntax, compilation and matching. It requires worker packaging/discovery, IPC and platform-specific memory controls for library callers. It becomes the preferred alternative if excluding C or strict external process containment becomes a requirement.

The selected QuickJS-ng path adds C compilation and a C/FFI safety boundary while providing real regexp interruption and common tracked allocation limits within an ordinary Rust operation. Those controls are narrower than process isolation, and do not establish C memory safety or a complete ECMAScript conformance guarantee.
