# Pinned document acquisition

Use `esdieff acquire` to save one deliberately selected public GitHub file as unchanged local bytes. Select its complete commit and establish the expected raw-file SHA-256 from separately reviewed source evidence before acquisition. The resulting Acquired snapshot is an ordinary input for offline checking, inspection, editing and literal Check plans. This follows [ADR 0035: Explicit pinned document acquisition](adr/0035-explicit-pinned-document-acquisition.md).

```sh
esdieff acquire --github OWNER/REPOSITORY --commit COMPLETE_40_HEX_COMMIT \
  --path repository/relative/file.sdf.json --sha256 EXPECTED_64_HEX_SHA256 \
  --document-output snapshot.sdf.json --format json
```

Every required option occurs exactly once. `--format human|json` is optional and defaults to human. `--report-output RECEIPT_FILE` selects a separate native file, and omission sends the receipt to stdout. Repeated, missing, unknown and unsupported options and positional model inputs are invocation errors. The document always needs a named native file. Acquisition offers no replacement flag or request plan.

Add `--report-output acquisition.json` to the command above to retain its JSON receipt in a new file. Use a different name for a human receipt. Both formats retain the same acquisition outcome and 64 KiB ceiling.

The repository has an owner and repository name. The commit is a complete 40-hex identity and the expected SHA-256 is 64 hex digits. The repository path excludes empty or dot segments, backslashes, absolute paths, controls, queries and fragments. Accepted UTF-8 path segments are encoded into the fixed `https://raw.githubusercontent.com` origin. SDF namespace URIs never select retrieval locations.

Request validation and native destination preflight finish before retrieval. Source metadata obeys applicable existing finite Source field ceilings, and the whole request is at most 64 KiB. Preflight also reserves worst-case escaping space for every mandatory receipt identity within the complete 64 KiB receipt envelope. Identity that cannot fit is rejected rather than clipped. Named outputs must be new, distinct native filesystem destinations. Existing files, aliases and collisions are refused. Display labels never reopen paths. Native non-UTF-8 destinations retain their actual same-platform identities.

## Transport and content

The operation verifies HTTPS and sends no credentials, cookies or authorization headers. It does not follow redirects or retry. It requests identity content encoding, refuses other encodings and accepts only a complete HTTP 200 body. Authentication, other origins and proxy configuration are outside this operation.

The body ceiling is 4 MiB and the total header ceiling is 64 KiB. The connection deadline is 10 seconds and the whole-transfer deadline is 30 seconds. Streaming work and transport buffers are bounded, including responses without a trustworthy length. Oversize, incomplete and deadline-exhausted responses fail acquisition.

SHA-256 covers the exact complete response bytes. The document is committed only after comparison with the independent expected digest. Acquisition never parses, repairs or regenerates JSON. Empty bytes and malformed JSON are valid acquisition results when explicitly selected by a matching digest. Their later ordinary decode and Model checking outcomes remain separate.

Bytes are staged privately beside the document with bounded writes. Commit cannot replace an existing or concurrently created destination. Task-created staging removal is attempted after failure and after commit. Cleanup failure identifies retained staging, preserves any earlier failure and discloses whether a verified document was committed. Host failure does not promise removal.

Staging identity is checked before cleanup. That check and unlink are separate filesystem operations, so a further change to the staging name can race cleanup. Cleanup is not a transaction over the mutable directory.

Acquisition pins each native destination's parent directory during preflight. Staging, document commit and named receipt creation use that directory's retained identity, so an ancestor alias introduced later cannot redirect a write. Destination rechecks refuse changes observed before delivery. Named receipts use exclusive creation and bounded writes. Failed receipt delivery can leave a partial file, which is disclosed through stderr and exit 4 rather than treated as complete evidence.

## Acquisition receipts

Human and JSON receipts express the same authoritative outcome. A JSON receipt is an object with `schema` equal to `esdieff.acquisition` and `version` equal to `1.0`.

| Field | Meaning |
| --- | --- |
| `source.repository` | Requested `OWNER/REPOSITORY` |
| `source.commit` | Complete requested commit identity |
| `source.path` | Requested repository-relative path |
| `source.uri` | Encoded fixed-origin retrieval location |
| `source.expected_sha256` | Independently supplied expected raw-file digest |
| `result.status` | `acquired` or `failed` |
| `result.complete` | Whether acquisition completed successfully |
| `result.document_committed` | Whether the verified document was committed |
| `result.actual_sha256` | Lowercase digest of a complete transferred body, otherwise null |
| `result.byte_count` | Exact nonnegative complete-body byte count, otherwise null |
| `result.failures` | Ordered objects containing `phase` and `message` |
| `document_output` | Readable `label` and exact `native_bytes` array |
| `staging_output` | Retained document staging identity, or null |

Native bytes are integers from 0 through 255 and represent a same-platform path identity, not a portable path serialization. A successful receipt has status `acquired`, `complete` and `document_committed` true and no failures. A complete digest-mismatching body retains its actual digest and count. An incomplete body records null for both.

Stable failure phases are `transport`, `http`, `limits`, `digest`, `stage`, `commit`, `cleanup` and `receipt`. Their order preserves causes, such as document commit failure followed by cleanup failure. Human message wording is explanatory rather than a stable identifier. Mandatory outcomes and identities take priority over optional explanatory detail within the 64 KiB envelope.

Human receipts follow the shared `--color auto|always|never` and `--no-color` controls, accepted before or after `acquire`. In `auto`, a receipt on terminal stdout can be styled, while redirected stdout and a `--report-output` file stay plain. `always` can style a human receipt file, and `never` removes all styling. Removing the style sequences gives exactly the plain receipt. If styling would push a receipt past the 64 KiB envelope, the plain receipt is delivered instead, so no detail is lost. JSON receipts and downloaded document bytes are identical in every mode.

Every valid, destination-preflighted request attempts one receipt after acquisition success or failure. Invocation and preflight errors write explanatory stderr without a receipt or retrieval request. A failed receipt delivery can leave partial evidence. Stderr identifies the receipt failure and whether a verified document exists. The document and receipt have no cross-output transaction.

Receipt delivery failure uses stderr and exit 4 as the authoritative outcome. Written bytes alone do not establish complete delivery, including when a later identity check finds that the requested path changed. A verified document commit remains recorded even if its requested path subsequently becomes unavailable or resolves elsewhere.

| Exit | Outcome |
| --- | --- |
| 0 | Verified document committed and complete receipt delivered |
| 1 | Transport, HTTP, limits or digest failure with receipt delivered |
| 2 | Invocation or destination preflight error |
| 4 | Staging, commit, cleanup or receipt delivery failure |

Any applicable host-output or cleanup failure takes exit 4. A document already committed before receipt failure remains available. A partial receipt is not complete acquisition evidence.

## Offline use and refresh

Ordinary CLI checks record their actual local file acquisition path. Library callers can explicitly provide the receipt's URI, commit and digest through existing `SourceInput::acquisition_uri`, `version_token` and `digest` metadata. No operation automatically loads a receipt.

Checking selects no implied Root document, retrieves no unsupplied reference and gives repeated or overlapping contributors no deduplication or input-order precedence. These are the existing Catalogue and scoped-reference rules. A refresh deliberately selects newly reviewed revision and digest values and a different destination, leaving prior bytes available for identical replay.

The [pinned OneDM example](../examples/pinned-acquisition/README.md) records independent source identity, retained notices and reproducible offline use. Historical interface examples with an unknown exact pre-RFC dialect are not current RFC-conformance or real-device acceptance oracles.
