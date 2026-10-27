// Metadata-only controls for the common fixed producer envelope. These do not
// supply production lookup facts or execute content-format syntax or guidance.
// Follows ADR 0027: Unit names and recorded registry guidance.
const CONTENT_PRODUCER_FACTS: [Facts; 3] = [
    Facts {
        registry: "coap-content-formats",
        snapshot: "iana.coap-content-formats/2026-10-05",
        scope: "relevant_dated_positive_records",
        extraction_policy: "iana-content-format-positives/2",
        digest: "4dfa7aca5dc7d7f98e6d7ec354db0325373d55c12f2add1af096db92bec72a4a",
        raw_digest: "48a9f69ecbebcd8a091c3ec4680d36281da74da8734a133ea6a95d6203f137ea",
        source_updated: "2026-08-11",
        retrieved_at: "2026-10-05T16:55:20Z",
        entries_count: 119,
    },
    Facts {
        registry: "media-types",
        snapshot: "iana.media-types/2026-10-05",
        scope: "relevant_dated_positive_records",
        extraction_policy: "iana-content-format-positives/2",
        digest: "dd7486a8665423f632da7f6d1a5f0c3321e39e94c43c0b983cd88131664f7e08",
        raw_digest: "0cbb8819dfe01c1badc71f8ba836291ba43718a5e797287cd1ac00d49f168ffb",
        source_updated: "2026-09-24",
        retrieved_at: "2026-10-05T16:55:20Z",
        entries_count: 2361,
    },
    Facts {
        registry: "http-content-codings",
        snapshot: "iana.http-content-codings/2026-10-05",
        scope: "relevant_dated_positive_records",
        extraction_policy: "iana-content-format-positives/2",
        digest: "f853d2b95b1e723a3848787bdc48b68946e3fda93b6797338accc6dcc86ee307",
        raw_digest: "e4581c5fd5bd902d9b8de2a8e7cc855e5616e7a9b9d7fc00b070e94092ba2c67",
        source_updated: "2025-10-02",
        retrieved_at: "2026-10-05T16:55:20Z",
        entries_count: 12,
    },
];
