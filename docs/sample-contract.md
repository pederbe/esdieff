# Selected-sample operation contract

This extends [the shared workflow and report contract](report-contract.md) under [ADR 0016: Selected samples with explicit admission and coverage](adr/0016-selected-sample-contract.md). It specifies observable behavior for the library, command, and versioned report. API spelling remains replaceable until implementation. Standards meanings follow the recorded baseline, [RFC 9880 Sections 4.7, 5.2, 5.3, and 5.4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.7), and normative Appendix C. This contract does not introduce a generic JSON Schema validator or a protocol payload binding.

The numeric workflow and typed projection boundary follow [ADR 0025: Selected numeric samples](adr/0025-selected-numeric-samples.md). An ordinary Rust caller registers both roles together:

```rust
use esdieff::{Admission, Catalogue, SampleKind, SampleRequest, SourceInput};

let catalogue = Catalogue::take_inputs(vec![
    SourceInput::supplied("model", br#"{"sdfProperty":{"level":{"type":"number","maximum":100}}}"#),
    SourceInput::supplied("sample", b"101").sample(),
])?;
let report = catalogue.check_sample(
    SampleRequest::new(catalogue.source_id("model").unwrap(), SampleKind::Property,
        "/sdfProperty/level").sample(catalogue.source_id("sample").unwrap()),
)?;
assert_eq!(report.sample_result().unwrap().admission(), Admission::Rejected);
# Ok::<(), &'static str>(())
```

For typed input, `JsonBuilder` constructs bounded values and `SourceInput::typed` registers their owned projection as a Sample source. `Catalogue::take_inputs` consumes inputs; `Catalogue::from_inputs` preserves the borrowed-input interface with bounded copies. `Source::sample_value` and `SampleResult::sample_value` expose the dedicated value view, whose span is optional. `Source::has_original_bytes` distinguishes original bytes from a projection, and `Source::json` returns unavailable for a typed projection because it cannot supply original-source spans.

Declaring selector identity is retained under a bounded metadata allowance, independently of displayed pointer limits and an older handle owner's derived storage. Its ceiling is 8192 bytes; an Action/Event suffix can extend the internal description pointer to 8206 bytes. Mandatory sample reporting reserves 163930 additional wire bytes and a separate ownership/DTO share within the existing report allocation model. Exact requested string, ancestry and single-result capacities are precharged. Exhausted semantic storage, work, nodes or reference applications produce an unfinished domain result rather than losing the mandatory selector. The accounting and fixed-field inventory are recorded in ADR0025.

## Conventional abstract samples

`sdfType: "byte-string"` selects canonical unpadded base64url decoded JSON text.
Empty text is valid; padding, non-URL alphabet, whitespace and nonzero unused
tail bits are rejected without repair. `sdfType: "unix-time"` selects exact
numeric POSIX coordinates. Nonnegative fractions, large coordinates and `-0`
are supported within the existing exact-number capabilities. Negative nonzero
numbers remain usable inputs with unfinished abstract denotation, rather than
representation errors. Independent integer, null and other constraints retain
their own conclusions. These conventions follow ADR 0007 and do not establish
formal CDDL or protocol-payload admission.

Both const operands must establish their denotations in a common compatible
context before equality or difference is concluded. Const, enum, compound
values, uniqueness and consistency use that shared comparator. Applicable byte
length or pattern qualities remain unfinished where their abstract interpretation
has not been established; no decoded-byte length is invented from a text bound.
Compatible POSIX numeric constraints use the original exact arithmetic allowance.

`SampleResult::meaning()` and mandatory `result.meaning` distinguish final
abstract-values, const and admission support from their actual limitations.
`SampleResult::consulted_interpretations()` records actual Byte/POSIX attempts,
including discarded branches and failed work. A complete whole witness replaces
tentative sample proof while preserving consultation and independent model
findings. The [shared report contract](report-contract.md#selected-abstract-value-meaning)
specifies reference identity, iterator length and optional-detail omission.
For a decisive compound const difference, final proof retains the decisive rule
and its established enclosing containers, excluding completed sibling meanings.
Equal and undetermined pairs preserve complete or partial aggregate support;
independent abstract-value support remains separate.

## Selection and inputs

`Catalogue::check_sample(request)` selects exactly one description. A request carries one supplied model-document identity, an effective definition pointer, and one of five kinds: `property`, `data`, `action_input`, `action_output`, or `event_output`. The pointer names the Property, sdfData definition, Action, or Event, not an arbitrary nested constraint. For Action and Event selectors the kind selects its `sdfInputData` or `sdfOutputData` subdescription. For example, an `action_input` selector points to `/sdfObject/Sensor/sdfAction/reset`; its selected description is that effective Action's `sdfInputData`. No Event-input or grouping selector exists. A typed effective description handle can construct the equivalent request without pointer reconstruction. The handle is bound to its owning catalogue/result and content identity; cross-owner or stale handles are request errors.

The selected document is the single model root. Other supplied model sources remain lookup candidates, with no position-based precedence or implicit fetching. Context includes the dependencies needed to establish that selected effective description and its constraint meanings. Catalogue source identities remain visible, including acquisition failures. An unrelated failed candidate does not alone obstruct sample admission. An unavailable candidate that could affect a needed contributor or ambiguity does obstruct the affected conclusion. Selecting a known absent definition or absent Action/Event description, the wrong class, an invalid pointer, an unknown root identity, or a nonmodel source as root is a request error. A selection that cannot be established because needed acquisition, decoding, composition, or interpretation is unavailable is a domain result with admission `undetermined`, rather than guessed absence or an error for an unknown selector.

Sample input is one whole supplied conventional JSON value, with a distinct request-local `SourceId` and source role `sample`. A conventional-JSON input owns immutable bytes and acquisition metadata; it must contain one JSON text without trailing second values. Exact original bytes remain available, including after decoding or A002 refusal. A typed Esdieff JSON value has null, Boolean, decoded scalar text, exact-number, array, and map constructors/accessors. Number construction accepts a checked JSON numeric lexeme or an exact checked machine conversion; it never rounds through a float. Map construction rejects duplicate decoded names under A002. Typed data is retained as owned values or immutable sharing and has a conventional JSON projection preserving accepted number lexemes. The projection is not an original source text: its sample locations have pointers, with `span:null` unless original supplied bytes and their token spans exist. Conventional byte strings and unix-time retain the representations in [ADR 0007: Generated SDF and sample JSON](adr/0007-generated-sdf-and-sample-json.md); typed JSON is not an alternative binary/time encoding.

Missing sample input is a request error. A supplied JSON null has a value and is checked under [ADR 0005: Nullability and presence](adr/0005-nullability-and-presence.md), including const and choice constraints. No root-value omission or whole-interaction scenario is modeled. Required map entries are evaluated inside the supplied value; absent entries are not populated from `default`. Property readable/writable/observable and declaration `sdfRequired` remain model characteristics; choosing a Property sample does not simulate a read/write/notification or impose those capabilities as value constraints.

All model/sample/generated attempts share the 256-attempt and 1 MiB metadata preflight in [ADR 0013: Shared workflow and report guarantees](adr/0013-shared-workflow-and-report-guarantees.md). The sample consumes one attempt and the shared acquisition/parsed-node/number/decoding budgets: 4 MiB per source, 32 MiB aggregate, depth 64, 100,000 nodes per source, 500,000 nodes per request, the exact-number token/coefficient/exponent/work caps, and shared checked semantic-work, derived-storage, location, diagnostic, and report limits. Sample acquisition is not an extra unlimited channel. Typed builders precharge nodes, text/key bytes, numeric tokens, and owned storage before copying or traversal. The same sample/effective evaluation shares the 10,000,000 work units and applicable equality/pair/pattern limits; repeated alternatives and nested values are charged. Default selection is at most 8 KiB of decoded JSON Pointer bytes; it is validated without clipping. Physical stack limits are supported bounds, never removed by raising a numeric configuration. A mandatory sample-report reserve includes both selectors, source roles, all nine area counters, admission, outcome, and omissions before acquisition. Presentation omissions preserve admission and aggregate counts; exhausted semantic work leaves affected admission or coverage unfinished.

## Admission and conditions

A typed constructor shares one remaining decoding-work allowance across aggregate metadata visits, duplicate-name comparisons, decoded key/text scans, projection copying and decoding. Each traversal is charged before it runs; a new phase or key does not replenish the allowance.

The three conclusions are:

| Admission | Required established basis |
| --- | --- |
| `admitted` | All constraints applicable to this sample in the selected effective description hold under an established common interpretation. For a choice, one complete effective alternative witness satisfies all its inherited and overriding qualities, with surrounding constraints that still apply. |
| `rejected` | At least one violated applicable constraint supplies a proof unaffected by outstanding uncertainty. For a choice, every potentially admitting effective alternative is established not to admit, or an independent proof applies to all alternatives. |
| `undetermined` | Neither admission nor such rejection is established, including unavailable needed context, unsupported applicable meaning, unusable sample decoding, or required work limits. |

The evaluator records the constraint/interpretation basis and sample/model locations. It does not create a synthetic RFC error solely from missing capability. Invalid JSON or A002 refusal is an input finding with admission undetermined; an invalid needed model parameter is a model finding with affected admission undetermined. Definite independent conditions survive scoped limitations.

Choice alternatives can overlap. A known matching branch is sufficient despite unrelated unknown alternatives when they cannot alter that complete witness. An unknown potentially matching branch blocks rejection when all known branches reject. Per-area successes from different branches cannot be combined into a fabricated witness. Effective inheritance and overrides are established before matching; a shared maximum overridden in one alternative cannot supply a proof against every alternative. Unavailable common/surrounding meaning that could invalidate the witness prevents admission. Known unknown branches irrelevant after a witness do not create unfinished requested units merely because they were not traversed. Their existence and witness basis can be explained without selecting an equality meaning.

Scalar and compound `const` compare ordinary values under [ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md). Thus `10`, `10.0` and `1e1` compare equal, escaped text compares after JSON decoding, and composed and combining text remain distinct. Different ordinary JSON kinds compare different without coercion. Arrays compare in order; maps compare decoded key sets and corresponding values without member-order significance. A definite ordinary length, key-set or sibling difference survives unavailable child meaning when the enclosing structure is established. Const contents are data: keys such as `sdfRef` or `type` inside a constant do not become vocabulary or reference instructions.

Before each const comparison, the evaluator establishes common meaning for that sample and effective constant across relevant local and enclosing final alternatives. Nested const and uniqueness retain those contexts through established ordinary array and map structure. An absent child description permits ordinary comparison after that structure is established; an unavailable child description leaves the affected comparison unfinished. A successful comparison or branch admission cannot establish an interpretation. Unknown or unavailable meaning blocks the affected equality proof. A conventional text-format branch can be irrelevant when an operand is an ordinary nonnull nontext value and the branch's text representation domain is independently established; unfamiliar formats, unavailable type meanings and null do not supply that exclusion. A complete branch with no equality obligation can still admit independently of another branch's unavailable equality. Each explicit const override creates its own obligation; it does not restore a replaced shared constant as a constraint on every branch. Enum retains each authored occurrence as a separate effective text-const alternative.

An established absent `type` adds no authored type constraint. For an ordinary nonnull number, applicable bounds and `multipleOf` still run; for ordinary nonnull text, scalar lengths and applicable unfinished pattern work still apply. A Full ordinary array or map with no authored `items`, `properties` or `required` can complete its structural family using the supplied kind, independently of const or uniqueness. Authored child qualities still require established applicable structure. Untyped unavailable count parameters remain unfinished, including beside assessed uniqueness. The evaluator uses the supplied kind without adding an effective `type` quality. Unknown explicit type, unsupported structural interpretation and unavailable inherited meanings retain their affected limitations. A matching const never bypasses another applicable constraint. Null retains `nullable` independently, and a nonnull const can exclude null even when nullability otherwise permits it.

Sample admission is independent of the shared disposition. A rejected sample emits at least one sample-domain error for its established violated constraint. Undetermined requested units contribute incompleteness, not a rejection error. Model-domain findings required to establish the selected context retain their original codes and bases. Strict warning policy can produce `failed` while admission is admitted. A definite rejection alongside an unfinished applicable constraint produces `failed`, `complete:false`, and admission rejected. Without blocking errors or strict warnings, unfinished requested work produces `undetermined`. Detail truncation changes none of these.

No whole-model assessment runs implicitly. `result.context` identifies selected-view availability and the needed model findings, independently of sample conclusions; it does not certify model validity. An explicitly performed catalogue or selected-model check remains a separate operation/result, reusable internally where source content, configuration, and baseline match. Its unrelated error does not become a sample rejection or a required sample error. Acquisition attempts remain visible in `sources`, even where their unrelated findings are outside the sample request.

Numeric sample types and bounds use the information-level meanings in [RFC 9880 Appendix C.1](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.1), including mathematical integrality for tokens such as `10.0`. They do not import Appendix A's CDDL `number`/`uint` model-parameter grammar as a blanket sample-number width restriction. [ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md) separately limits claims about formal model-parameter representability: exact access to a count parameter such as `18446744073709551616` does not certify that grammar. The sample result can establish an ordinary mathematical constraint conclusion without certifying the whole model's grammar. Any actual needed uncertainty about the effective parameter or abstract data meaning leaves that affected sample work undetermined. Neither a failed machine conversion nor unassessed formal model-grammar work alone establishes that a supplied sample is rejected.

Ordinary string and Boolean admission uses the same selection, effective context, input locations and report. Boolean values require no coercion. Text `minLength` and `maxLength` are inclusive Unicode scalar counts under [RFC 9880 Appendix C.2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.2): `"é"` and `"𝄞"` each have length one, while `"e\u0301"` has length two. No Unicode normalization changes the supplied value. Count parameters retain the exact nonnegative mathematical meanings in [ADR 0022: Scalar constraints and characteristic states](adr/0022-scalar-constraints-and-characteristic-states.md), including counts larger than machine integers. Scalar traversal precharges decoded UTF-8 bytes against the work remaining after selected composition and checking, and both comparisons share the numeric allowance remaining after input parsing. Exhaustion leaves the affected area undetermined.

Length and pattern parameters are needed for an ordinary nonnull string. A different base type or a supplied null does not perform text matching; null still retains every other applicable obligation, including const and choice work. Documentary `default` never supplies a value. Supported Unicode ECMAScript matching follows the [pattern contract](pattern-contract.md). An unavailable matching capability or exhausted allowance keeps that work undetermined, while an independent established length violation can still reject the string. Needed format, abstract or alternative uncertainty prevents affected text proofs rather than being ignored. Unavailable Unicode scalar decoding remains a sample-input limitation even when no length bound is present.

## Ordinary map samples

An established ordinary `type: "object"` follows [RFC 9880 Appendix C.5](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.5). `properties` constrains described entries that are present; other entries remain unconstrained. `required` names mandatory entries, including names with no property description. Its whole nonempty list must have decoded names before any absence proof. Duplicate required names request presence once. Whole sample member identities must be available, including keys after a matching member. Unknown enclosing map meaning prevents structural admission or missing-entry rejection.

A present described entry uses its final composed description, real sample value and applicable scalar, null, map, array and alternative rules. An absent optional entry requests no child reference, invalid scalar parameter, array or format work. Neither `default` nor declaration `sdfRequired` supplies an entry. Unavailable equality context, unsupported format meanings and declared Extension features retain scoped limitations. Undeclared extension vocabulary does not, as described under [Undeclared extension vocabulary](#undeclared-extension-vocabulary). An independent missing entry or known sibling violation still supplies a rejection while another needed child remains unfinished.

Every map alternative needs a complete whole-map witness. A child's known choice admission returns to its containing map, and different branches cannot supply different required entries for one fabricated witness. Raw branch roots settle before their final whole-quality overrides and present child references. Parameter finding storage, semantic work, derived storage and one numeric remainder remain shared across all pairs without refunds or resets.

For an Action input selected at `/sdfAction/configure`, an object requiring `mode` rejects `{}` at the sample map pointer `""`, with its real original span and the model's `/sdfAction/configure/sdfInputData/required/0` relation. A present null satisfies presence and is checked separately against the child's nullable meaning. Required absence at a nested map uses that existing parent location; no missing-child location is invented. A wrong present scalar uses its own entry pointer and token span, or `span:null` for a typed projection. A declaration or payload key named `sdfRequiredInputData` remains an ordinary name; assessed legacy vocabulary retains the A001 restructuring guidance.

## Ordinary array samples

An established ordinary `type: "array"` follows [RFC 9880 Appendix C.4](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.4). `minItems` and `maxItems` are inclusive exact nonnegative mathematical counts, including authored counts larger than machine integers. Every actual element requests its established `items` description. Supported direct items include ordinary numeric, string, Boolean and object descriptions, and choices whose Full alternatives may contain nested arrays. Full object properties can also contain arrays. Wider direct-item array types, nested `items`, defaults, nullable meanings and reference imports retain the limits in [ADR 0024: Effective alternatives and nullable characteristics](adr/0024-effective-alternatives-and-nullable-characteristics.md). Unsupported needed item meaning prevents admission; an independent known count or child violation can still reject.

An empty array requests no element description. It can satisfy established outer array constraints even when the deferred item subtree has opaque meaning. This does not establish that subtree's meaning or capability. A supplied element, including an empty inner array, requests its own actual item context and cannot use the outer array's empty-case conclusion. For `uniqueItems: true`, one established equal pair disproves uniqueness, even beside other unknown pairs; proving uniqueness requires every necessary pair to differ. Empty and singleton arrays compare no pairs. False or absent uniqueness imposes no comparison obligation. Item checks and other constraints remain independent.

Const and uniqueness share the request-wide equality-pair ceiling, original structural/string/key work, numeric remainder and derived allocation allowance. The default pair ceiling is 100,000; `Limits::equality_pairs` can reduce it to a nonzero value. Every actual scalar or compound const and uniqueness pair spends one pair before comparison across nested values and alternatives. Work or representation exhaustion leaves the affected conclusion undetermined without refunding prior work. A duplicate condition uses the second element as primary, retains its real effective/source relation and appends the first element as related sample evidence. A const mismatch uses the real decisive leaf or structural root. Typed inputs have null spans; rendered pointer omissions preserve these conclusions.

Each element's choice returns to the containing array conjunction. Different elements may satisfy different item alternatives, while a whole-array choice requires one complete whole-array witness. Shared item roots and final alternatives retain one identity and their real origins; present optional interests accumulate across elements. Absent optional entries never erase an earlier present interest. Common model conditions and newly needed parameter families are checked once per final description, including malformed conditions, without resetting work or finding storage. Inclusive count failures use `sample.count` with `arguments.constraint` naming `minItems` or `maxItems`, located at the actual array with its authored bound as related model evidence.

## Combined compound descriptions

Compound descriptions combine these same rules recursively. Map entries and array
elements can request named choices whose Full alternatives describe maps, arrays,
ordinary scalars or supported abstract values. Shared qualities apply until an
alternative explicitly replaces the whole quality. A complete alternative must
establish the entire selected value; successful checks from separate alternatives
cannot be assembled into one witness. Const and uniqueness still require a common
established equality meaning under [ADR 0015: Bounded value equality in a resolved
data context](adr/0015-value-equality.md), independently of which branch admits a
value. Inherited object `properties` under an explicit array alternative can keep
that common equality meaning unavailable. Nullable, required presence and
documentary default remain separate.

The supported combination includes exact numeric fields, required map entries,
contextual compound const and uniqueness, Unicode ECMAScript patterns, all six
base string formats (`date-time`, `date`, `time`, `uri`, `uri-reference`, `uuid`),
and the byte-string and POSIX conventions. These meanings use their existing
representation contracts at every needed nested location. An inherited field can
keep its donor minimum while a consumer overrides its maximum; findings identify
the actual sample leaf and the effective constraint, with the applicable original
contribution related. A missing required entry is located at its existing parent.

One known admitting compound alternative supplies a witness. A potentially
admitting unresolved alternative prevents rejection by the other branches, while
an independently violated enclosing or sibling constraint can still reject with
unfinished coverage visible. Restricted Direct donor meaning stays unavailable
when copied into a Full alternative until an explicit establishing override.
Decoded-byte lengths, incompatible or unknown equality interpretations,
declared Extension features and matching capabilities retain their existing
scoped limits. All values, alternatives, equality pairs and matches share the
original finite request allowances. Combining implemented capabilities does not
establish whole-model conformance or general satisfiability.

## Undeclared extension vocabulary

Undeclared extension vocabulary does not stop sample checking, following
[ADR 0037: Undeclared extension vocabulary](adr/0037-undeclared-extension-vocabulary.md).
That is a qualified or unqualified Quality Name outside base SDF, such as
`acme:note`, `sdfProtocolMap` or a misspelled `minimun`, in a document that
declares no Extension feature. A selected Property, sdfData, Action input or
output, or Event output description carrying it admits or rejects a sample on
its base description, as if that quality were absent. The same holds for nested
object properties, Direct items and choice alternatives. Const and `uniqueItems`
comparisons use that base meaning too, so `7.0` matches `const: 7` beside
`acme:note`. Recorded extension vocabulary at a position its registration names,
such as `oma:id` on a Property under
[ADR 0038: Recorded extension vocabulary](adr/0038-recorded-extension-vocabulary.md),
is set aside the same way. Sample checking does not check its value.

The extension's own meaning stays unsupported, and a model check still reports
the vocabulary's notice or unfamiliar-name warning. A base quality outside its
role, such as a wider Direct item quality, is not extension vocabulary and keeps
its scoped limitation. So do unknown values of base qualities, such as
`format: "email"`, documents that declare an Extension feature and descriptions
composed from them.

`baseline.interpretations` lists `undeclared-extensions-ignorable/1` whenever a
sample context set aside undeclared extension vocabulary and the sample
conclusion went on from the base description. A Sample report omits it when the
vocabulary lies outside every description the sample reached, for example on
the enclosing Object or in an absent optional entry, and when a base quality
outside its role or an unknown base value already stops the affected
conclusion. Recorded extension vocabulary that a sample context set aside names
it too, because a Sample report never consults recorded registry evidence.
`SampleResult::consulted_interpretations()` still records only Byte and POSIX
attempts.

## Immutable requested coverage

FullBaseline separately establishes known byte-string and Unix-time model
interpretations as M026 and M027. It supplies no synthetic value, decodes no
bytes and assigns no time coordinate. The canonical unpadded base64url and exact
POSIX numeric conventions above continue to govern actual constants and selected
values. Model interpretation does not establish decoded-byte length, negative
POSIX denotation, leap-instant representation or general satisfiability.

The default and first supported profile is `esdieff.sample/1`, requesting all applicable base-SDF admission obligations in the following nine areas. Membership is independent of implementation. Each area contributes exactly one status to the requested aggregate; applicable recursive constraints, values, alternatives, and limitations are summarized within that area and may have bounded per-location detail. A whole-model RFC inventory is not this inventory.

| Area identity | Fixed membership and guards |
| --- | --- |
| `sample-context/1` | Sample acquisition/decoding/policy, description selection, needed reference composition and critical-feature understanding. A failed needed prerequisite leaves this area unfinished and any dependent areas undetermined, not inapplicable. |
| `sample-type-null/1` | Established effective base type, mathematical integrality, null admission, and their interaction at all needed data locations/alternatives. Null does not bypass other constraints; absent map entries are separate map obligations. |
| `sample-numeric/1` | Inclusive/exclusive numeric bounds and positive multipleOf, exactly and recursively where applicable. Unknown abstract numerical meaning is not ignored. |
| `sample-text-boolean/1` | Boolean value admission; Unicode scalar text lengths; ECMA-262 Unicode pattern syntax/matching where applicable. Abstract format denotation belongs in abstract-values; JSON escape decoding belongs in context. |
| `sample-const/1` | Scalar/compound const matching using context equality from [ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md); no fallback to representation equality. Documentary default never supplies sample data. |
| `sample-alternatives/1` | Effective sdfChoice inheritance/overrides, enum expansion, and existential admission/rejection with complete witnesses. Recursive choices retain this meaning; ordering supplies no interpretation or witness by itself. |
| `sample-map/1` | Established object structure, entry descriptions, allowed entries and required presence, recursively. Data required is distinct from declaration sdfRequired. Applicable unsupported item/object vocabulary remains unfinished rather than imported from a generic schema. |
| `sample-array/1` | Array structure, inclusive counts, established item descriptions and uniqueness, recursively; grouping-array counts are outside a selected data description. Item vocabulary and equality limits remain explicit. |
| `sample-abstract-values/1` | Applicable format/sdfType denotations and conventional representations, content-format meaning where relevant to admission, supported extensions and external knowledge needed to determine sample value meaning. Known names alone are not sufficient capability. |

An area's `assessed` status means its requested obligations have been established satisfied or violated; findings are compatible with assessed. `not_applicable` requires established irrelevance for the selected sample context, not an absent implementation. `unassessed` means needed applicable work has not run; `undetermined` means work was attempted but a needed conclusion remains unavailable. All nine statuses sum to nine. Requested completeness requires each area assessed or not applicable, even if an independent rejection already exists. If the context is unavailable, dependent membership cannot be declared not applicable. A profile may gain implemented checks under the same identity; new membership, standards meaning, or guards require a new profile identity. New profile identities are explicit caller choices, not silent aliases for whatever is currently supported.

Ordinary scalar descriptions, including scalar const, supported Unicode patterns, enum and nested choices with complete effective witnesses, can complete this profile when their other areas are established inapplicable. Needed unavailable equality, pattern, compound or abstract meaning keeps requested work unfinished. Pattern outcomes and their finite matching controls follow the [pattern contract](pattern-contract.md). Later implementations fill the existing obligations rather than silently replacing their meaning. Broader model-baseline coverage retains the shared partial/undetermined state and inventory availability; a sample pass supplies no model conformance claim.

## Versioned report extension

Sample finding and rejection-witness bases identify the producing work: numeric type and arithmetic conclusions use RFC 9880 Appendix C.1, ordinary string type and scalar length conclusions use Appendix C.2, Boolean type conclusions use Appendix C.3, const uses Appendix C, alternatives use Section 4.7.2, and rejection of null by `nullable:false` uses Section 4.7 Table 4. Relevant established coverage carries the same type basis; combined type/null coverage cites Section 4.7 and Appendix C. Admission witnesses retain Appendix C and identify the complete alternative when applicable. Tentative branch failures become sample errors only when every potentially admitting alternative has been excluded; independently needed model conditions retain their own counts and bases. Context establishment uses `sample-establishment`, and applicable unimplemented work uses `sample-implementation`, including unfinished pattern work alongside an independent length rejection. Needed sample decoding limitations retain their original cause, domain and available location; optional omission does not remove them from diagnostic counts. Unrelated Model parser leaves do not become requested sample findings.

A const finding and its rejection witness cite Appendix C even when contextual equality uses Byte or POSIX meaning inside a scalar or compound value. The const coverage row summarizes established equality work and can instead carry the specialized Byte or POSIX interpretation when it is the sole abstract interpretation in the final const proof. Complete `meaning.const` and `meaning.admission` references retain that interpretation and any necessary container support alongside Appendix C. Consulted interpretations record actual use, including work that does not survive as final proof. These fields serve distinct purposes; a single finding or coverage basis does not replace the complete meaning references.

The sample report is `schema:"esdieff.report", version:"1.1", operation:"sample"`. Check and inspect contracts, fields, units, values, null meanings, and exit rules remain unchanged. A reader supporting only check/inspect rejects the new operation before interpreting its payload. Existing shared objects apply unless specified below; these tables define the sample operation's additional required fields and closed tags. IDs are compact nonzero u32; Count/Bytes retain shared checked-u32/nonnegative-byte meanings. All strings and details consume shared metadata/report budgets. Missing required fields are invalid reports.

| Object | Required fields and types | Optional fields / null meaning |
| --- | --- | --- |
| Top level | Shared required fields, with operation `"sample"`, request SampleRequest and result SampleResult | None. |
| SampleRequest | assessment:`"esdieff.sample/1"`; inventory:`"esdieff.sample/1"`; scope:Scope; selector:SampleSelector; sample:Id | No nullable assessment/inventory. Scope selected roots is exactly selector.document; sample references a source with role sample and is not a model root/dependency. |
| SampleSelector | kind:`"property"|"data"|"action_input"|"action_output"|"event_output"`; document:Id; pointer:string (<=8192 UTF-8 bytes) | No source view or arbitrary constraint-pointer mode. |
| RequestedCoverage for sample | inventory:`"esdieff.sample/1"`; count_unit:`"sample_area"`; total:9; assessed:Count; not_applicable:Count; unassessed:Count; undetermined:Count | None. Counters sum to9; no substitution of inspection or model-assessment units. |
| CoverageRow for sample | id:one of the nine area identities; count_unit:`"sample_area"`; status:shared coverage status; basis:Basis | Shared source_ids/reason_code. Any separately emitted requirement detail uses count_unit requirement; the two units are never summed. |
| Condition for sample | Shared Condition fields plus domain:`"model"|"sample"|"operation"` | Shared optional fields. A required-presence condition can include `arguments.entry` with the decoded name, or null with a byte-limit omission. domain identifies what was diagnosed, not severity or baseline classification. A related model location does not turn a sample constraint finding into a model error. |
| SampleResult | kind:`"sample"`; admission:`"admitted"|"rejected"|"undetermined"`; sample:Id; selector:SampleSelector; description:EffectiveLocation|null; sample_location:SampleLocation; context:SampleContext; witnesses:SampleWitness[]; explanations:Explanation[]; omissions:Omission[] | description null means the needed effective description location could not be established, never an invented target. Duplicated IDs/selectors must agree with request. Explanation retains the shared nearest-contributor/default/deletion meanings. |
| SampleContext | state:`"available"|"partial"|"unavailable"`; model_assessment:null; findings:ContextFindings | model_assessment null means no model assessment was requested. State describes the needed selected context; partial means usable independent projections coexist with needed unavailable meanings. |
| ContextFindings | errors:Count; warnings:Count; notices:Count | Counts retained-or-omitted relevant model-domain conditions, not sample-domain errors. Unrelated catalogue findings from a different operation are not included. Incomplete requested units are counted only in the shared outcome, without duplicate area counts. |
| SampleWitness | kind:`"admission"|"rejection"`; effective:EffectiveLocation; sample:SampleLocation; basis:Basis | alternative:EffectiveLocation, present only when a named effective alternative supplies the proof. constraint:string (<=256 bytes). No witness asserts equality interpretation or complete provenance history. |

SampleSource metadata uses the shared Source object. A typed input has acquisition supplied, conventional retained bytes if a bounded projection is retained, and no fabricated byte spans. Sample spans for byte input refer to its original immutable bytes. Pointer rendering omissions keep the source ID and available spans. The sample pointer root is `""`; malformed/partially decoded nested input can have pointer null and span when available. Effective description locations use the selector root and the selected description path, including input/output suffixes; they claim no source span. Related source locations explain inherited and overriding constraints when available.

Witness arrays and diagnostic/coverage rows are optional-detail collections bounded by the shared limits; omitted witnesses do not change retained admission or counts. Omissions explicitly identify their report paths and reasons. Human output states selected kind, sample identity, admission, shared disposition/completeness, scope/coverage, and relevant sample/model locations. It derives these from the same result as JSON. No human wording or array order becomes the stable interface.

Adding optional sample fields/codes within 1.x follows the shared rules. Changing required field types, these null/tag/unit meanings, or admission semantics requires a major report version or a newly explicit operation/profile contract as appropriate. Rust builders/accessors keep private fields and extensible result enums; wire DTOs do not serialize internal enum layouts.

Civil `date-time`, `date` and `time` use the [civil-time contract](civil-time-contract.md). Their denotation fills `sample-abstract-values/1` without changing the nine-area inventory. Ordinary string lengths remain independent. Two established Gregorian dates compare equal for the same calendar date and different for different dates, independently of JSON escape spelling. Compatible established date-times compare by UTC instant, excluding offset annotations and preserving exact fractions and distinct inserted seconds. Date-free time retains equality for identical denoting representations and unavailable comparison for differing representations. Invalid or unavailable denotation supplies no temporal equality or difference. Unknown or incompatible meanings across alternatives remain explicit.

## Command acquisition

The proposed command is `esdieff sample --document MODEL --kind KIND --pointer POINTER --sample SAMPLE [--strict-warnings] [--format human|json] MODEL_INPUT...`. MODEL must match one explicit model input; KIND is one of the five selector tags. SAMPLE is an explicit file path or `-`; model inputs also allow `-` for one stdin document. Only one total stdin use is permitted across model and sample acquisition, so a sample `-` excludes a model `-`. Duplicate stdin, missing sample/model/kind/pointer, unknown model roots, wrong known selector roles, invalid options, and malformed pointer are invocation errors with exit2. No default sample is invented and no default document is selected by input position.

The command registers all acquisition attempts before reading and applies shared limits. The sample gets its own identity even if its path equals a model path; roles remain separate, with bounded role-qualified opaque caller keys. Non-UTF-8 paths retain exact filesystem identity with safe bounded labels. File/stdin acquisition or decoding failures are structured domain results. They never make a definitely missing sample silently null. Exactly one chosen report and trailing newline goes to stdout; stderr contains invocation/host-output failures. Shared exits apply: 0 passed, 1 failed including strict warnings or established sample rejection, 2 invocation/configuration error, 3 undetermined without blocking errors, 4 report/host-output failure. Failed plus incomplete uses1 and retains `complete:false`.

## Concrete observations

Temperature describes number, nullable false, minimum -50 and maximum150. Sensor.temperature inherits Temperature. Device.ambient overrides its Property maximum to100; its changed Event output retains maximum150. For the owned byte sample `101`:

- Property selector `/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature` gives rejection at sample source root pointer `""`, span `[0,3]`; its effective `/maximum` is100, with source explanation at the Device override. Its initial numeric work can be complete if all other applicable sample obligations are assessed/not applicable.
- Event-output selector `/sdfThing/Device/sdfObject/ambient/sdfEvent/changed` selects its `/sdfOutputData`; the same sample admits under maximum150. Source Temperature and Sensor bytes and source maximum150 remain unchanged. The Event result is independent of the rejected Property result.
- A model warning relevant to the usable Event description with strict warnings yields admission admitted, disposition failed, complete true, warnings1/errors0. This is not a sample constraint rejection.
- An independent violated established Property maximum plus an unsupported equality-changing const yields admission rejected, disposition failed, complete false, errors1 and one unfinished const area, only when that unsupported meaning cannot change the numeric rejection proof. If it could alter the applicable numeric meaning, this fixture would be undetermined instead.
- A previously performed catalogue check can fail on an unrelated unused source while the selected sample still admits. The attempted source remains visible, but that separate check's error is not counted as a sample condition. Unavailable needed composition gives context unavailable/partial and sample undetermined; dependent areas are not reported inapplicable.
- A complete ordinary choice alternative admitting `101` establishes admission despite another unresolved potentially matching alternative. If the known alternative rejects and the other may match, admission is undetermined. An unresolved surrounding meaning that could invalidate the known witness also gives undetermined.

These observations guide implementation at the public domain boundary. Exact byte spans are supplied by parsing original inputs; expected effective outcomes in a contract exercise are supplied acceptance fixtures and are not evidence that a resolver or sample validator has executed.
