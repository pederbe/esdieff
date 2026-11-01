# 0035: Explicit pinned document acquisition

Superseded by [ADR 0057: Supplied models without network acquisition](0057-supplied-models-without-network-acquisition.md).
The following records the removed operation.

[Pinned document acquisition](../../GLOSSARY.md#model-work) is an explicit CLI operation for one public GitHub file at a complete commit identity and a separately reviewed expected SHA-256. It preserves unchanged bytes in a new native file and produces a bounded Acquisition receipt. Keeping retrieval separate makes the resulting Acquired snapshot an ordinary offline input and preserves [ADR 0004: Supplied catalogues and checking scope](0004-supplied-catalogues-and-checking-scope.md) and [ADR 0006: Scoped reference failures](0006-scoped-reference-failures.md).

The [historical acquisition contract](https://github.com/pederbe/esdieff/blob/63e8920a58ca5f4625f066575b57fb3c7641ee40/docs/acquisition-contract.md) fixes origin, verified TLS, identity content encoding, no redirects or retries, byte ceilings and transfer deadlines under [ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md). Native destination preflight and no-replacement commits protect prior evidence. The document and optional named receipt are independent outputs. A committed document can remain after receipt delivery or staging cleanup fails, and the outcome must disclose that state. Neither transport success nor a matching digest establishes Model checking, licensing or real-device acceptance.

## Considered Options

Implicit namespace retrieval would make model meaning and completeness depend on network availability and an acquisition convention SDF does not require. Following branches, learning the expected digest from the same transfer, replacing snapshots or coalescing identical contributors would weaken explicit source selection and offline replay. Receipts therefore remain separate evidence that callers may deliberately translate into existing Source metadata, rather than new remote inputs or automatically loaded plans.
