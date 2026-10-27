# URI and UUID sample contract

This contract follows [ADR 0019: URI, URI-reference and UUID sample conventions](adr/0019-uri-and-uuid-sample-conventions.md). It extends the [selected-sample operation contract](sample-contract.md) without changing the nine-area `esdieff.sample/1` inventory, report version 1.1, selection, ownership or disposition rules. The adopted scope distinguishes representation, applicable URI scheme meaning, opaque UUID values and context-dependent equality.

FullBaseline establishes known `uri`, `uri-reference` and `uuid` model
interpretations without supplying a value or invoking a representation check.
This M049 observation asserts no scheme admission, target identity, constant
denotation or comparator capability. Constants still need their actual
contextual denotation; an unsupported URI scheme can leave M043 unfinished while
the named model interpretation is established. Model selector recognition alone
does not record consultation of the sample facts listed below.

## Meanings and conventional inputs

| Format | Abstract meaning | Conventional JSON |
| --- | --- | --- |
| `uri` | A full resource identifier under RFC 3986. | A JSON string whose decoded text satisfies RFC 3986 `URI`, including a scheme and an optional fragment. No relative form or empty string is a URI. |
| `uri-reference` | A resource-reference expression under RFC 3986. | A JSON string whose decoded text satisfies `URI-reference`: a URI or relative reference, including empty, query-only and fragment-only forms. It need not be resolved to a target for expression admission. |
| `uuid` | An opaque 128-bit UUID value under RFC 9562. | A JSON string with exactly 32 ASCII hex digits and four hyphens at the 8-4-4-4-12 group boundaries, without a `urn:uuid:` prefix, braces or surrounding whitespace. |

Primary definitions are [RFC 9880 Appendix C.2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.2), [RFC 3986 Sections 3 and 4](https://www.rfc-editor.org/rfc/rfc3986.html#section-3) and [RFC 9562 Section 4](https://www.rfc-editor.org/rfc/rfc9562.html#section-4). Conventional text is assessed after JSON decoding; alternate JSON escape spellings do not themselves change its content. This adds no alternative binary protocol representation.

URI generic syntax and scheme-specific semantics are separate. A generic representation conclusion names its syntax basis; abstract-format admission also needs an established compatible meaning, including applicable scheme requirements. HTTP(S) requires authority form and a nonempty host under [RFC 9110 Sections 4.2.1 and 4.2.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-4.2.1). For example, `https:device` and `https://` match generic grammar but fail those scheme requirements. An established scheme failure supplies rejection on its own cited basis, even when generic syntax succeeds. A finite HTTP/HTTPS identity-syntax capability supports ordinary cases without requiring an exhaustive scheme registry or communication behavior. Unknown schemes do not trigger an invented rejection allowlist. An unavailable needed scheme interpretation stays undetermined, and its abstract-values area remains unfinished. Relative references can be admitted as expressions without target resolution; needed base or inherited scheme meaning is separate. No network request, name lookup, existence check or retrieval is performed.

URI text retains supplied characters and percent encodings. Reject malformed percent escapes, spaces, backslashes, literal non-ASCII characters and invalid generic components; do not trim, repair, percent-decode before parsing or silently convert IRIs. Percent-encoded non-ASCII octets can satisfy generic grammar without establishing a particular scheme's character encoding. IP literals and authority/path delimiters require their actual RFC 3986 grammar. Arbitrary bracket content is not an IP literal. Numeric port syntax does not by itself certify a protocol's usable port range. Registered-name grammar is broader than DNS labels. A first relative path segment cannot contain a colon, while `./this:that` can be a relative reference.

UUID text decodes to bytes in displayed/text order. Upper, lower and mixed-case hex name the same bytes. Esdieff interprets the base format as an opaque 128-bit value without a version/variant allowlist. This does not assert that every finer layout is defined or interoperable, certify uniqueness or randomness, validate timestamps, introspect application fields, or prove a generation algorithm. Nil and Max are ordinary representable sentinel UUID values, independent of JSON null. Requested layout-specific meaning must assess applicable variant requirements from [RFC 9562 Section 4.1](https://www.rfc-editor.org/rfc/rfc9562.html#section-4.1) and corresponding version/layout requirements. Unavailable needed layout meaning stays undetermined, even when the opaque representation is available. Alternate UUID formats are outside this conventional channel; describe that input failure with its convention basis instead of calling an abstract UUID universally invalid.

## Equality and additional constraints

Use [ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md) for `const` and `uniqueItems`. In a common established UUID context, both bare representations must first denote UUID values; compare their 16 bytes. Case differences can establish equality, and a differing byte establishes difference. Invalid strings, unavailable interpretation, incompatible choice meanings and exhausted work stay undetermined, including identical text. A comparison result does not establish admission under every other constraint. Supplied output text is preserved; byte comparison does not mandate rewriting it.

No URI normalization or equivalence comparator is adopted. Differing case, percent spelling, paths or relative forms remain undetermined for value equality, even where a future purpose-specific comparator could prove equality. Identical valid representations establish equality only when their common compatible deterministic interpretation is known. If equality asks whether relative references identify the same target, an explicit applicable base is needed; it is never inferred from an SDF namespace, source path or acquisition URI. Expression admission requires no target resolution. A separately requested JSON representation comparison can report text differences without claiming different abstract identifiers.

Unicode scalar length and pattern constraints still apply to the conventional decoded text when their established meaning is needed. Constraints inherited through references or choices retain their effective meaning and overrides. A complete matching alternative can admit; unrelated unfinished alternatives cannot invalidate its established witness. Potentially matching unknown alternatives prevent unjustified rejection. These rules do not add URI-specific composition, a general scheme registry or a new interpretation of restricted item vocabulary.

## Findings, locations and coverage

In an established common compatible context, null equals null and differs from
an established nonnull URI, URI-reference expression or UUID. An invalid or
uninterpreted nonnull operand leaves comparison undetermined. Null/null does not
consume nonnull representation facts; mixed pairs retain the facts actually
consulted for the nonnull value. Equality is independent of null admission.
Full UUID comparator capability supports known absent, true and false nullability
without operands, while restricted or unavailable meaning remains unfinished.
These cases do not supply general nonnull URI equivalence or target identity.

A definitely invalid conventional representation rejects only under an established applicable format/context. Record a sample-domain constraint finding and the conventional/RFC generic grammar basis actually used. An unknown critical feature, unknown equality-changing extension, unavailable needed scheme/base interpretation or exhausted semantic work produces an affected undetermined conclusion instead. Invalid JSON or unusable decoding remains a context/input finding with sample admission undetermined. Unsupported decoded scalar content is not relabeled malformed URI text. Independent definite findings remain available only when unavailable meaning cannot change their proof.

Format denotation and needed external knowledge belong to `sample-abstract-values/1`; text lengths and patterns belong to `sample-text-boolean/1`; const and array uniqueness retain their own areas. Inapplicability requires an established reason. Full sample admission still requires every applicable area. A format success alone never claims sample completeness or model conformance. A definite representation rejection can coexist with unfinished independent pattern work: admission rejected, disposition failed and `complete:false` only when that unavailable work cannot change the representation rejection.

For data selection `/sdfData/Uri`, a format finding refers to effective location `/sdfData/Uri/format` in the selected document and to sample source pointer `""`. For supplied JSON `"../device"`, the sample token span is `[0,11]` in its immutable source bytes. Related source locations identify the declared or inherited format where available. Typed sample input has pointer `""` and `span:null` unless original supplied bytes exist. Const cases additionally identify effective `/const`; array uniqueness identifies the two item sample pointers. Nested map entries or array items use their actual sample pointers and selected effective description suffixes. No source span is invented for effective content.

Finite acquisition, decoded text, structural/pair, number, semantic work, derived storage, location and report limits remain those of [ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md), [ADR 0013: Shared workflow and report guarantees](adr/0013-shared-workflow-and-report-guarantees.md), [ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md) and [ADR 0016: Selected samples with explicit admission and coverage](adr/0016-selected-sample-contract.md). Precharge representation scanning, decoding, comparison and repeated choice work before traversal or allocation. Avoid percent-decoded copies, target resolution and implicit fetching. Budget exhaustion leaves affected work undetermined. Pointer/report presentation omissions preserve established conclusions and aggregate counts separately. A parser's own tighter capability must be disclosed, not treated as an RFC rejection. Algorithm improvements may change implementation, never these semantic boundaries.

## Examples

Each ordinary case uses an available `sdfData` description with `type: "string"`, `nullable: false`, the stated format and no extra constraints. HTTP(S) cases use the supported authority and host requirements under RFC 9110. Other needed scheme meaning remains unavailable.

| Format | Conventional sample JSON | Admission and basis |
| --- | --- | --- |
| `uri` | `"https://example.test/device#reading"` | Admitted with generic syntax and applicable supported HTTPS meaning; the fragment is allowed. |
| `uri` | `"mailto:operator@example.test"` or `"urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6"` | Generic representation conforms; admission undetermined if the needed scheme interpretation is unsupported. No network/delivery claim or UUID-channel coercion. |
| `uri` | `"https://"` or `"https:device"` | Generic representation conforms; rejected on established HTTP(S) nonempty-host or authority-form basis respectively. |
| `uri` | `"../device"` or `""` | Rejected: no scheme/full URI. |
| `uri` | `"https://example.test/%GG"`, `"https://example.test/a b"`, `"https://example.test/é"` | Rejected: invalid generic representation. |
| `uri-reference` | `"../device?mode=1#reading"`, `"//example.test/device"`, `"/device"` | Admitted as reference expressions, with no inferred base. |
| `uri-reference` | `"?mode=1"`, `"#reading"`, `""` | Admitted as reference expressions. |
| `uri-reference` | `"1thing:part"` | Rejected: neither a legal scheme prefix nor a permitted first relative segment. |
| `uri-reference` | `"./1thing:part"` | Admitted; colon occurs after the first relative segment. |
| `uri-reference` | `"device%GG"` or `"device\\reading"` | Rejected: invalid generic representation. |
| `uuid` | `"f81d4fae-7dec-11d0-a765-00a0c91e6bf6"` or mixed-case equivalent | Admitted; same 16-byte value. |
| `uuid` | `"00000000-0000-0000-0000-000000000000"` or `"ffffffff-ffff-ffff-ffff-ffffffffffff"` | Admitted Nil/Max values. |
| `uuid` | `"01890f3e-6c4a-7d4b-8e23-4d87b637659b"` or `"f81d4fae-7dec-f1d0-f765-00a0c91e6bf6"` | Admitted opaque values without layout proof. |
| `uuid` | `"urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6"`, braced or unhyphenated equivalent | Rejected under the explicit bare-text convention; these do not assert universal abstract invalidity. |
| `uuid` | `"g81d4fae-7dec-11d0-a765-00a0c91e6bf6"` or truncated text | Rejected: invalid conventional representation. |
| Any of the three | Recognizable ordinary text plus unknown critical meaning or exhausted required scan work | Undetermined for affected admission; capability/context basis, no invented format violation. |

With an established matching UUID `const` whose hex case differs, the equality obligation succeeds; a one-byte difference supplies a const mismatch. `uniqueItems:true` in established UUID item meaning rejects mixed-case duplicate values. Identical malformed UUID operands give equality undetermined rather than a false equal proof; definite sample format rejection can still be independent. `uri` const operands differing by host case, or reference operands `"device"` and `"./device"`, give equality undetermined under this contract. Identical valid relative-reference expressions can satisfy a constant under the same expression meaning. Target identity would additionally need an applicable base; Esdieff performs no target resolution. An independent matching choice retains an admission witness; unresolved surrounding meaning can prevent it.

Human, typed and JSON results agree on these conclusions, bases and locations under the selected-sample contract. Format findings use `sample.uri_representation`, `sample.http_identity`, `sample.uuid_representation` or `sample.resource_meaning_unavailable` for their distinct reasons. Required work exhaustion uses `operation.limit`.

## Consulted format facts

`Report::format_knowledge(id)` returns a borrowed `FormatKnowledgeView` with static
`id`, `rfc`, `sections` and `scope` text. Sample records the facts actually used by
its request, including visited alternatives and constant comparisons. A later
unfinished branch or omitted optional detail preserves earlier consultations.
The six identities are:

| Identity | Consulted scope |
| --- | --- |
| `rfc3986-uri-syntax/1` | Generic URI representation |
| `rfc3986-reference-expression/1` | Relative-reference expression |
| `rfc9110-http-identity/1` | HTTP authority and nonempty host |
| `rfc9110-https-identity/1` | HTTPS authority and nonempty host |
| `rfc9562-bare-text/1` | Selected UUID text representation |
| `uuid-opaque-value/1` | Decoded opaque UUID value |

An unknown identity is absent. Non-Sample operations omit `format_knowledge` and
return absent through this getter. Sample's baseline field is an array of the
consulted four-field records; an admitted consultation owner with no used facts
produces `[]`. Initial inability to establish that owner produces `null` and
known getter identities are unavailable. Late work exhaustion retains the array
of facts already consulted. These are request observations, not a capability
inventory or evidence of external resource acquisition.

UUID findings identify `uuid-bare-json/1` or `uuid-opaque-value/1` as the selected
interpretation. Their actual use also appears in `baseline.interpretations`.
URI generic grammar and HTTP(S) identity findings retain separate RFC bases.
Human output presents the same consulted facts and interpretations as JSON.
