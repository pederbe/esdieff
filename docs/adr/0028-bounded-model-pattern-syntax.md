# 0028: Bounded effective model pattern syntax

The parser, worker and resource-accounting implementation is superseded by
[ADR 0056: Fixed internal safeguards and Rust patterns](0056-fixed-internal-safeguards-and-rust-patterns.md).
The text below retains the historical decision. Effective scopes, semantic
results and report-detail distinctions continue where not superseded.

Model authors need syntax findings at the description they use, including imported and overridden patterns. Integrate the syntax portion of [ADR 0014: Bounded partial ECMAScript pattern capability](0014-bounded-pattern-capability.md) through the shared effective evaluator, following [ADR 0020: Owned effective views and nearest origins](0020-owned-effective-views-and-nearest-origins.md) and [ADR 0024: Effective alternatives and nullable characteristics](0024-effective-alternatives-and-nullable-characteristics.md). Normative syntax is [RFC 9880 Appendix C.2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.2) and [ECMA-262 2025 Section 22.2.1](https://tc39.es/ecma262/2025/multipage/text-processing.html#sec-patterns).

Provide the immutable `esdieff.expanded.pattern-syntax/1` assessment with the initial eight areas and `pattern-syntax/1`. Full invokes the same syntax rule as M047 in its fixed pattern/format area, while unfinished matching or other applicable meaning remains incomplete. Other profiles, inspection and Sample keep their meanings. The private dependency is exact regress 0.12.0 with its qualified default std/backend-pikevm features, Unicode mode and disabled optimization. Matching and its runtime remain separate work.

Assess established Full string patterns after composition, independently of explicit string type and source shape replay. Direct patterns remain opaque; receiving a Direct donor in a Full branch does not settle its unavailable meaning. Explicit Full overrides can establish it. Deleted/overridden imports are not separately assessed in selected scope. Effective findings point to the consumer path and nearest authored token, and unavailable needed contexts leave the area undetermined without hiding independent errors.

Preserve the independent Unicode spelling/name-terminator and decimal endpoint guards. Duplicate decoded capture names, unverified name decoding and unverified parser rejection remain unsupported. Known malformed syntax is an RFC finding; finite capability and operational exhaustion are incomplete. Matching-only name/capture capacities do not restrict syntax.

The only reducers are 4096 UTF-8 pattern bytes, 64 group levels and 512 alternatives. All traversal, secondary scans, name decoding/comparison/growth and finding growth debit the original operation counters before work or allocation. Pattern findings count before optional retention, and their shared finding vector admits its full new requested capacity from the original allocation owner before growth. Failed retention alone preserves the established conclusion.

Reserve one logical 4 MiB stack slot from the original operation allocation for its lazy joined worker, shared with matching under ADR 0014: Bounded partial ECMAScript pattern capability. All guards, parse/emission and destruction occur inside that worker. Logical dependency invocation/input accounting and finite admission do not provide parser-heap, resident-memory, exact-instruction, cancellation or wall-time guarantees. Callers supply ordinary model work and receive domain results without runtime ownership.

Add a uniform 171 report bytes above each invoking request's existing mandatory allowance, at both request admission and metadata production. Keep four mandatory-owner buffers, four optional-owner residual buffers and four encoding/output buffers, totaling the existing eight times report ceiling. Fixed owners, literal DTOs, collection growth and simultaneous fallback generations must fit those original partitions for the supported backends. No pattern phase can borrow a fresh budget.

## Considered Options

Checking source patterns before composition would assess overwritten/deleted imports and could misstate Direct donor meaning. Reusing source structural replay for syntax would suppress the first actual syntax check. Checking only an explicit string type would miss a present model constraint whose spelling must still be valid.

Leaving known syntax out of Full would conflict with its request for all applicable baseline requirements. Publishing a nine-area Full inventory would confuse partial implementation with complete membership. A separate pattern work/allocation allowance would replenish the original operation; a guessed parser allocation slab would not prove containment. A larger report reserve is unnecessary when the complete producer fits the original owners and this derived additive reserve.

The supervised process and alternative regex dependencies remain the separately considered options in ADR 0014. They are not introduced by this syntax integration. The [pattern contract](../pattern-contract.md) states the current public behavior and limitations.
