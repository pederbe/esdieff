# Unit names and recorded registry guidance

Unit work follows ADR 0027: Unit names and recorded registry guidance. Authors opt into `Assessment::ExpandedUnits` or `esdieff check --assessment expanded-units-v1`. The immutable profile `esdieff.expanded.units/1` consists of the eight initial areas plus `unit-spelling/1` and `unit-registry-guidance/1`. Initial, expanded modified, inspection and sample requests retain their inventories. Full joins both unit dimensions into M023 and `full.special-type-unit/1` once, preserving producing condition IDs. Completing unit work does not complete other unfinished Initial or Full areas.

```rust
use esdieff::{Assessment, Catalogue, CheckRequest, SourceInput, RecordedRegistryPolicy};
let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("fixture", br#"{
  "info": {}, "sdfProperty": {"Temperature": {"type":"number", "unit":"Cel"}}
}"#)])?;
let report = catalogue.check(CheckRequest::new().assessment(Assessment::ExpandedUnits))?;
let units = report.unit_guidance().require()?;
assert_eq!(units.known_primary(), 1);
assert!(units.complete());
// An embedding can exclude this recorded edition under its evidence policy.
let withheld = catalogue.check(CheckRequest::new()
    .assessment(Assessment::ExpandedUnits)
    .recorded_registry_policy(RecordedRegistryPolicy::WithholdRecorded))?;
assert_eq!(withheld.unit_guidance().require()?.unavailable(), 1);
# Ok::<(), &'static str>(())
```

The corresponding CLI option is `--registry-knowledge recorded|withheld`. Both explicit values require a registry assessment; they are refused for initial, modified, inspection and sample requests. The additive report field is only `policy.recorded_registry`, with `use_recorded` or `withhold_recorded`. No arbitrary snapshot input, automatic network lookup, refresh or symbol rewrite is performed.

Decoded JSON text remains exact. Ordinary `kg` and `Cel` have recorded SenML positives, `ms` and `/100` have secondary positives, and `cel` remains a scoped unknown. `%` retains its legacy ratio meaning, distinct from `/100` percentage. A URI containing a colon is checked against generic full URI syntax, without validating resource content or scheme-specific meaning. Recognizable `urn:ietf:params:unit:kg` is prohibited; `urn:ietf:params:unit:m%2Fs` represents `m/s` and is also prohibited. `urn:ietf:params:unit:example%3Aratio` establishes the colon exception only, without certifying membership or assignment. `%253A` decodes once to `%3A` and supplies no represented colon. A slash written directly in the unit index is a Section 7.3 spelling error. Case-conflicting markers and percent-encoded markers retain separate uncertainty diagnostics.

`unit_guidance()` is Absent when not requested. Present complete or partial retains every established count independently of optional diagnostics. A reliable no-unit result is complete with six zeros and no registry observation; partial zeros do not prove absence. Nontext unit shape has an independent error and no invented registry query. Fixed semantic owner admission failure is outer Unavailable and wire `state: unavailable`, `counts: null`, `reason: owner_limit`.

Within recognized unit URNs, an empty or non-pchar-first r/q component is `unit.urn_components_syntax`, an RFC 8141 Section 2 error. An ordinary first top-level question mark is `unit.urn_delimiter_advice`, notice/advice with the same citation and its code as basis id, preserving the SHOULD processor recommendation. Literal index slash retains `unit.index_syntax` and its Section 7.3 basis. Valid component questions and fragments remain data; malformed URI escapes fail the generic URI check first. These component classifications can finish with URI count1, all query counts0 and no registry record. Completed classification does not establish URN conformance, represented-symbol meaning, definition or assignment. Other successful queries keep their own actual-use metadata.

The six counters describe final effective Full unit-quality coordinates within the requested roots. An independently checked provider root and each consuming copied coordinate count separately. Selected unused providers do not contribute observations. Overrides use the surviving unit, deletions contribute none, opaque Direct donors and critical/unresolved contributors supply no invented meaning. Alternative refinements are distinct effective coordinates; raw source preflight and replay frequency add no observations.

| Count | Meaning |
| --- | --- |
| known_primary | Completed exact positive in the SenML section |
| known_secondary | Completed exact positive in the Secondary Units section, after a primary miss |
| unknown | Completed union miss in these dated tables; current registration unknown |
| unavailable | Established needed lookup unfinished because of withholding, work or index meaning |
| legacy | Subset of positive counts with the retained legacy flag |
| uri_names | Established generic full-URI declarations, including prohibited unit URNs; independent of lookup outcomes |

Only the first four are disjoint query outcomes. Neither their sum nor any counter asserts valid unit URN assignment. `source_edition()` returns borrowed raw edition facts when at least one section was consulted, even if later work stopped. It is Absent before any consultation or needed withholding, and Unavailable for needed withheld evidence. `registry_knowledge("iana.senml-units")` and `registry_knowledge("iana.secondary-units")` expose borrowed section-specific observations. Available means actual consultation, not match or completion. Needed withheld records contain only registry, state unavailable, snapshot null and reason caller_withheld. Before the first key comparison an operational stop produces no record; after it, the actual consulted section remains available. An unvisited secondary table stays Absent.

The raw [IANA SenML XML](https://www.iana.org/assignments/senml/senml.xml) was retrieved at `2026-10-05T16:49:51Z`, with XML update date `2026-02-02`. Its SHA256 is `3eff03640c5a5d2abd09e330488aeb9fae0b2766ea69b1114295bbec5c44e754`. Raw bytes and section canonical bytes are retained as test fixtures. Canonical section extraction preserves XML row order and writes `symbol<TAB>true|false<LF>` with no header. Primary 67 rows occupy 630 bytes, SHA256 `6246352c590e933b72c25ef9f363171e40fea347bb7e23d08b04f9436894ed34`; secondary 39 rows occupy 398 bytes, SHA256 `de5c5c009ffbc61aa66e71ee55065f95108709575906c6ec7cd64a2bbc7e173b`. `entries_count` describes the extracted section, while `raw_digest` describes the common full XML edition. Generated Rust tables and public exact-name controls are checked against every captured row, flag and canonical byte. Production does not parse XML.

Before effective or registry scanning, `Catalogue::check` requires the existing source/scope reserve C plus E to fit `report_bytes`, after Catalogue acquisition and decoding. E is 21448 for expanded units and 31448 for full-baseline registry work; the separately prepared content contribution is 19116. These are ownership-derived reserves, not minimum wire lengths or measured resident-memory claims. A single supplied source key `fixture` contributes seven metadata bytes, giving C=20522 and exact unit admission 41970; 41969 refuses before checking work. At that cap every optional coverage row and diagnostic can be omitted without losing errors, counts or consulted evidence.

The fixed unit owner is bounded by 128 bytes and shared control by 16. Two complete extension DTO generations plus owners receive 4E; independent output and old/new buffer growth receive another 4E. Units D=42824 and owners144 imply 2D+144=85792=4E. Full registry composition has six possible records at array capacity8, D=62792 and owners208, giving 125792=4E. Shared parents, policy, profiles, arrays and control are paid once. Existing mandatory source/scope owners remain in C; optional detail and scratch use only the positive R-(C+E) remainder, preserving the existing total 8R accounting. No per-name mandatory graph is retained.

Qualification guards word<=8, String<=24, Value<=128, Map<=256 and alignment<=8. Up to ten entries fit a default BTree leaf or the qualified IndexMap growth from7 to14 dense buckets and8 to16 index buckets. Old/new controls include both bucket counts and group tails, 248 bytes on the qualified 8-byte-word backend; the guarded per-map bound4024 fits4096. Static record text has fixed safe ASCII caps and no caller strings. Synthetic simultaneous field caps produce a conservative 530-byte record bound including array punctuation; actual finite literals are qualified independently. The six-record synthetic array is3175 bytes and the projected full fixed wire7271 fits7680. This is a requested-capacity and source-specific accounting argument, not a portable allocator-overhead guarantee. Dependency or graph changes require renewed qualification.

Every new visited source row, node, member, ancestry/alternative step and examined key byte is precharged on the original operation allowance. Unit text receives a finite32-times-byte allowance for URI, markers, suffixes, pchar, UTF-8 and optional exact replacement passes; each registry comparison then precharges its row and both examined key lengths. No replay or per-value refill changes counters. With at most100000000 work units, each checked u32 observation count is bounded below u32 maximum. Optional replacement allocation is charged to diagnostic storage after the spelling premise and its copy work are admitted; detail omission cannot retract a prohibition or lookup result.
