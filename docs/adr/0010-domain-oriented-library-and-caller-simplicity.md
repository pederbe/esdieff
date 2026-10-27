# 0010: Domain-oriented library and caller simplicity

Esdieff exposes typed access to SDF definitions, groupings, affordances, data descriptions, and structured findings, backed by the shared domain model. The library carries the burden of interpreting references, composition, and checking rules so that callers can use SDF concepts without reconstructing their meaning from generic JSON maps. Source and permitted extension JSON remain accessible.

Simplicity, correctness, and efficient normal use guide the library's design. Interfaces should express the guarantees actually available and make incomplete results clear. Internal complexity should produce a concrete benefit for callers rather than requiring them to manage SDF processing details. Performance choices must preserve the standards baseline and semantic fidelity in [ADR 0002: Recorded standards baseline](0002-recorded-standards-baseline.md) and [ADR 0007: Generated SDF and sample JSON](0007-generated-sdf-and-sample-json.md).

Concrete public types, fields, identity, ownership, serialization contracts, and editing interfaces remain subsequent design decisions. This principle does not choose a graph representation, allocation strategy, or numeric dependency.

Public interfaces should accommodate future capabilities without transferring avoidable migration work to callers. This applies to typed Rust access and machine-readable reports alike. Compatibility rules and extension points must serve concrete expected uses while preserving precise meanings; the principle does not require speculative general-purpose mechanisms.

## Considered Options

Making generic JSON the sole interface would push traversal and RFC interpretation onto every library consumer. Exposing mutable internals as the main interface would likewise make callers responsible for preserving invariants. Domain-oriented operations allow Esdieff to carry those responsibilities while retaining access to the underlying content.
