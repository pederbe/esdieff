# 0009: Bounded processing and reporting

The library and CLI use explicit, configurable finite processing budgets for input size and depth, reference expansion, checking, numeric work, patterns, and diagnostics. A processing limit produces an explicit operational limitation and leaves affected requested checks incomplete, rather than establishing an RFC violation. This bounds work on supplied input while preserving the distinction in [ADR 0003: Check findings and completeness](0003-check-findings-and-completeness.md).

Diagnostic truncation is reported separately from unfinished semantic checking. The aggregate outcome is retained even when individual diagnostics cannot all be displayed, so report limits cannot conceal errors behind earlier warnings. Numerical limitations are explicit; processing never silently rounds model values, overflows, or substitutes an incorrect verdict. Exact budgets, numerical representation, and dependencies remain implementation decisions requiring suitable evidence.

## Considered Options

Unbounded processing would allow small reference graphs, extreme exponents, or patterns to consume disproportionate resources. Treating every limit as invalid input would misstate a toolkit limitation as a standards rule. Equating report truncation with incomplete checking would likewise confuse presentation limits with assessment coverage.
