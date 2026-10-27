# Recorded content-format evidence

The three XML files are exact IANA responses retrieved at
2026-10-05T16:55:20Z. They retain excluded Reserved/Unassigned records, template
aliases and dated annotations. The three positive JSON files are canonical
extracted evidence under policy `iana-content-format-positives/2`, with compact
field ordering and exactly one terminal LF. Dates, primary URLs and all six
SHA256 values are in the [content-format contract](../../../docs/content-format-contract.md).

The product uses a static borrowed key/flag projection. Complete canonical rows
retain numeric media/coding fields and temporary annotation/expiry, and media
canonical identity/template distinction. Rust table qualification compares every
generated key and flag against these canonical rows; public declaration callers
exercise every positive, both aliases, case-insensitive lookup and all dated flags.
Product runtime never reads these fixtures or parses XML.

The literal grammar cases independently distinguish RFC 9193 Section 6 spelling
from the Section 3 number range. The SDF fixtures exercise selected inheritance,
overrides/deletions, nearest origins, critical features and unavailable Direct
donor/text meaning. Their cross-document references use the established
`shared:#/sdfData/Payload` resolver spelling.
