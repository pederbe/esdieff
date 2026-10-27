# Content-format declaration contract

`Assessment::ExpandedContentFormat`, selected by CLI `--assessment expanded-content-format-v1`, requests the fixed inventory `esdieff.expanded.content-format/1`. It adds `content-format-syntax/1` and `content-format-registry/1` to the unchanged eight initial areas, following [ADR 0029: Content-format declarations and recorded registry guidance](adr/0029-content-format-declarations-and-recorded-registry-guidance.md). The initial, modified, unit and sample inventories keep their existing membership. FullBaseline additionally assesses the bounded declaration meanings below within M024 of its immutable model inventory, following [ADR 0033: Content-format model meaning](adr/0033-content-format-model-meaning.md).

The population is final effective Full data descriptions at distinct consuming coordinates, including properties, reusable data, Action input/output, Event output, map properties and established alternatives. Reference overrides and deletions determine the surviving value. A selected check excludes unused donor declarations; a catalogue check independently includes reusable definitions. Each inherited consumer remains a distinct observation with an effective primary location and its nearest source contribution as related evidence. Raw bytes, authored case and coding order remain unchanged. Ordinary scalar getters and initial string-shape checks retain their meanings.

Direct items and opaque donor content do not acquire Full semantics from a familiar spelling. Deleted values and reliable absence are NotApplicable. Unavailable effective content, decoded text, member or role identity, donor meaning and critical-feature context prevent dependent conclusions. A known nonstring value receives the existing `data.string` error once; it is not invented lexical text. Independently established errors survive uncertainty elsewhere.

## Complete spelling and separate range

[RFC 9880 Appendix A note 2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-A) references [RFC 9193 Section 6](https://www.rfc-editor.org/rfc/rfc9193.html#section-6). The complete string must follow that grammar. A decimal number is `0` or a nonzero digit followed by zero or more ASCII decimal digits, without a grammar digit ceiling. Signs, leading zeroes, whitespace, fractions and exponents fail spelling. A fully decimal spelling outside 0 through 65535 has a separate Section 3 range error under recorded interpretation `I-CF-RANGE-1`. Whole classification precedes range: `123456/application` is a media name, not a decimal range error. Arbitrarily long decimal strings are compared by exact spelling and length, without machine-integer overflow.

Media type and subtype each contain 1 through 127 ASCII bytes. Their first byte is alphanumeric; remaining bytes may also be `!#$&-^_.+`. A single slash separates them. Wildcard media components, non-ASCII bytes and `%`, apostrophe, backtick, pipe or tilde in a media component fail this narrowed grammar. Registry comparison ignores ASCII case while retaining the original text.

Parameters follow optional ASCII SP, semicolon, optional SP, a nonempty `tchar` name, equals, and a nonempty `tchar` value or quoted value. `tchar` adds `!#$%&'*+-.^_` followed by backtick, pipe and tilde to ASCII alphanumerics. There is no whitespace around equals. A quoted value may be empty; ordinary quoted bytes are SP, `!`, and ASCII ranges `#` through `[` and `]` through `~`. Backslash pairs accept ASCII SP through `~`. Tabs, controls, DEL and non-ASCII quoted bytes fail, including escaped forms. `@`, semicolon and equals inside quotes are data. Only SP before a following parameter semicolon is admitted; leading or trailing whole-string whitespace is not.

Each content coding is `@` followed by a nonempty `tchar` token. There may be zero or more ordered tokens, including duplicate tokens. Comma lists, empty tokens, quoted tokens, whitespace and coding parameters fail spelling. An absent coding adds no implicit lookup. The explicit token `identity` passes spelling but has reserved meaning outside this inventory.

ExpandedContentFormat excludes parameter meaning, including duplicate-parameter meaning, and discloses this with a Notice. This permits completion of its advertised declaration inventory when syntax and dated queries finish. FullBaseline has the additional bounded meaning rules below. Neither operation establishes payload decoding, sample validity or a protocol binding.

## Bounded Full model meaning

Established nonempty Choice and Enum collections request declaration meaning
from their final inherited alternatives. Superseded shared declarations and
their original descendants do not request an additional model interpretation.
The existing spelling and registry-guidance passes retain their population and
actual-use metadata; an unsupported declaration inherited by any final branch
still leaves its meaning unfinished.

An available numeric Content-Format identifier designates its complete recorded
CoAP tuple: content type including registered parameters, and its optional
coding. All 119 captured positive tuples are retained exactly and independently
checked against the canonical source records. No separate media or coding
registry query is inferred for a numeric declaration. Withheld CoAP evidence
cannot be recovered through another table.

Explicit media names use these supported rules:

| Media type | Established parameter meaning |
| --- | --- |
| `application/json` | No parameter is required. A single `charset` parameter has no effect on compliant recipients, regardless of its value, as [RFC 8259 Section 11](https://www.rfc-editor.org/rfc/rfc8259.html#section-11) specifies. No JSON encoding is inferred from that parameter. |
| `application/cbor` | Parameterless declarations have established meaning; [RFC 8949 Section 9.1](https://www.rfc-editor.org/rfc/rfc8949.html#section-9.1) defines no required or optional parameter. |
| `text/plain` | Absent charset denotes the US-ASCII default; a single `charset=utf-8` or `charset=us-ascii` is established under [RFC 2046 Section 4.1.2](https://www.rfc-editor.org/rfc/rfc2046.html#section-4.1.2). |

Media and parameter names compare without ASCII case. The supported charset
values also compare without ASCII case; token and quoted spellings, including
decoded quoted pairs, are equivalent under [RFC 9110 Sections 5.6.6 and 8.3.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.6).
Other media rules, parameters, charset meanings and duplicate parameters remain
Unassessed, with `content_format.model_meaning_unassessed` as an Incomplete
condition. A registered name such as `multipart/mixed` does not establish that
required parameter knowledge is available. Unsupported meaning creates no
invented definite RFC error.

Positive coding tokens denote their designated transformations in authored
application order, including repeated occurrences, following [RFC 9193 Section 3](https://www.rfc-editor.org/rfc/rfc9193.html#section-3).
No `@` means no transformation and no coding query. Unknown codings and explicit
reserved `identity` remain Undetermined; no HTTP negotiation meaning or header
prohibition is silently imported. Raw case, parameter text, coding order and
source origins remain intact.

Actual admitted consultation of these Full rules is recorded as
`content-format-model-meaning/1`, independently of successful establishment or
optional diagnostics. A numeric tuple consultation needs its available recorded
evidence. A normative media-rule consultation may occur while a separate
registration query is withheld. Neither fact hides unfinished registration or
meaning.

## Evidence and statuses

`RecordedRegistryPolicy::UseRecorded` is the default evidence policy for relevant checks. `WithholdRecorded` deliberately withholds recorded evidence according to an embedding's trust decision. The CLI uses `--registry-knowledge recorded|withheld`; both explicit values require the content, unit or full-baseline assessment. Initial, modified, inspect and sample requests refuse that option. Reports emit one `policy.recorded_registry=use_recorded|withhold_recorded` value, without a competing request field.

| Established observation | Syntax area | Registry area | Condition |
| --- | --- | --- | --- |
| Invalid complete spelling | Assessed | NotApplicable, if no other query | `content_format.syntax`, Error, RFC 9193 Section 6 |
| Decimal spelling outside the identifier range | Assessed | NotApplicable, if no other query | `content_format.number_range`, Error, interpretation `I-CF-RANGE-1` |
| Dated positive registration | Assessed | Assessed | `content_format.registry_known`, Notice with dated flags |
| Completed positive-table miss | Assessed | Undetermined | `content_format.registration_unknown`, Incomplete; current registration unknown |
| Needed evidence withheld | Assessed | Undetermined | `content_format.registry_knowledge_withheld`, Incomplete |
| Explicit reserved identity coding | Assessed | Undetermined | `content_format.interpretation_unavailable`, Incomplete |
| Parameters with otherwise finished queries | Assessed | Assessed | `content_format.parameter_semantics_unassessed`, Notice for excluded meaning |
| Unknown relevant text, role or effective context | Undetermined | Undetermined | Existing scoped capability or meaning condition |

Statuses aggregate the entire requested effective population. An unfinished required pass remains Undetermined; a condition's omission cannot make it Assessed. An unknown query cannot be relabeled an unavailable table. Current registration, available evidence and completed lookup are separate facts. The existing requested coverage counts remain authoritative even when every optional area row and located condition is omitted. Other unfinished Initial work can keep an expanded outcome incomplete. Full joins spelling, range, registration and bounded declaration meaning into M024 and `full.pattern-format/1` once, preserving their producing IDs. One unsupported or unknown sibling keeps that obligation unfinished.

`Report::registry_knowledge(id)` borrows actual-use facts for `coap-content-formats`, `media-types` and `http-content-codings`. Before the first admitted key comparison, unavailable operation work fabricates no table consultation or entry. After a comparison, that table remains available even if lookup later stops. An unneeded table is Absent. Needed withholding produces registry, state unavailable, snapshot null and reason caller_withheld, without digest, dates or entry counts. Failure to admit the semantic metadata owner returns Unavailable for requested registry IDs. Existing wire states remain available and unavailable.

## Recorded primary editions

All three raw XML editions were retrieved at `2026-10-05T16:55:20Z`. Exact raw bytes and canonical extracted JSON are retained in `tests/fixtures/content-format`. Production uses static Rust key/flag projections, with no XML parser, network access or implicit refresh. Metadata scope is `relevant_dated_positive_records`; extraction policy is `iana-content-format-positives/2`.

| Registry and source | Update date | Raw SHA256 | Canonical extracted SHA256 | Positives |
| --- | --- | --- | --- | --- |
| [CoAP Content-Formats](https://www.iana.org/assignments/core-parameters/core-parameters.xml) | 2026-08-11 | `48a9f69ecbebcd8a091c3ec4680d36281da74da8734a133ea6a95d6203f137ea` | `4dfa7aca5dc7d7f98e6d7ec354db0325373d55c12f2add1af096db92bec72a4a` | 119 |
| [Media types](https://www.iana.org/assignments/media-types/media-types.xml) | 2026-09-24 | `0cbb8819dfe01c1badc71f8ba836291ba43718a5e797287cd1ac00d49f168ffb` | `dd7486a8665423f632da7f6d1a5f0c3321e39e94c43c0b983cd88131664f7e08` | 2361 |
| [HTTP content codings](https://www.iana.org/assignments/http-parameters/http-parameters.xml) | 2025-10-02 | `e4581c5fd5bd902d9b8de2a8e7cc855e5616e7a9b9d7fc00b070e94092ba2c67` | `f853d2b95b1e723a3848787bdc48b68946e3fda93b6797338accc6dcc86ee307` | 12 |

Canonical bytes are compact JSON arrays with fixed field order and one terminal LF. Numeric rows are sorted by exact u16 identifier and contain id, content_type, coding, flags, temporary_annotation and temporary_expires. Thirty-six Reserved/Unassigned records or ranges are excluded from 155 raw CoAP rows; all three temporary positives retain their dated annotations. Empty coding is null, without inventing identity. Media rows contain key, canonical, template and flags; coding rows contain key, canonical and flags. Each flags object orders obsolete, deprecated and temporary. Name keys are sorted ASCII lowercase solely for comparison. Media retains 24 obsolete and eight deprecated flags; HTTP retains two deprecated flags and excludes the reserved identity row from its 13 raw rows.

For media identity, a template file's explicit name attribute takes precedence; otherwise its text is the identifier. The file text is also retained separately as the template target. Thus deprecated `image/x-emf` and `image/x-wmf` remain separate identifiers targeting `image/emf` and `image/wmf`. Display text is used only for dated status flags, without a guessed identifier-stripping rule. Global case-insensitive duplicate checks merge only fully identical records; conflicting canonical fields or flags stop extraction rather than choosing a winner. Every generated product key and flag is tested against all captured canonical rows, including both aliases.

## Finite ownership and work

After Catalogue acquisition, check admission requires the existing source/scope reserve C plus E before new scanning. Econtent is19116, Eunits21448 and combined Full31448. C depends on actual caller metadata: one supplied key `authored` has eight bytes, C20528 and exact content admission39644;39643 refuses. Seven-byte `fixture` has C20522 and39638;39637 refuses. Applicable existing role and selector reserves remain independent.

Two complete extension DTO generations and fixed owners receive4E; independent encoding and old/new growth receive another4E. With the recorded extension record, the Full extension bound D62792 and owners208 give 2D+208=125792=4E. Its six registry records use array capacity8. The complete report may have eleven interpretations: four registry interpretations, civil representation, Byte and Unix conventions, both UUID interpretations, copied reference relocation and undeclared extension vocabulary. This grows the interpretation Vec from8 to16; an old8 backing may coexist with new16 and the registry8 backing. Qualification charges all32 Value slots plus eleven40-byte text caps against the existing combined array share1536. On the compiled32-byte Value backend this is1464. It also exercises both complete actual DTO generations, retained-result fallback and wholly omitted optional details at the original admission cap. The actual maximum extension graph bound is36634 bytes; two generations, growth and owners use73732 of125792 available bytes. The private content owner, its maximum alignment padding and borrowed view still fit64 bytes. These are source-specific ownership bounds, without a portable ABI, allocator-overhead or resident-memory claim. No existing allowance grows automatically.

Every visited source, effective node, member/key, ancestry, alternative and examined text byte is precharged on the original operation allowance, including excluded Sample rows. Whole borrowed classification pays a finite32-times-byte coefficient plus one; lookup separately precharges each compared row and both byte lengths, conservatively paying the complete static key maximum before reading a name row. No per-declaration refill, allocated lowercase copy or mandatory per-name graph is used. Four checked u32 counts remain bounded by the100000000 maximum work allowance. Optional finding capacity and retained unit replacement strings keep their already consumed diagnostic storage; beginning the content module cannot renew that credit. Presentation omission preserves errors, requested aggregate counts and actual-use evidence.

One shared borrowed walker first completes the eligible grammar/range population and then replays the already derived effective coordinates for registry queries. Candidate/classified observations, spelling/range findings and parameter-exclusion Notices occur once in the first pass; query/unresolved observations occur only for actual second-pass attempts. An interrupted first pass never starts registry replay. Operationally finished traversal is distinct from monotonic unknown meaning: a completed traversal with an unknown donor or critical premise may still query a different reliable coordinate. Every second-pass stop preserves the first pass's syntax status, including any previous Undetermined state.

A finished first pass with no reliable query-needed bits omits replay, preserving earlier unknowns and fabricating no consulted or withheld facts. Otherwise replay pays traversal and full borrowed classification again from the same remainder, without re-composition, new arenas or a replenished allowance. This permits later independent grammar/range findings to survive lookup exhaustion. The ordinary cost is additional finite work and later first consultation: a tight request can stop before a table formerly reached. Work thresholds are source-specific capability observations, not public constants or a runtime-doubling claim. Under Full, the unit module still runs before content on the same allowance; this scheduling separates content grammar from content lookup.
