# Model meaning assessment

FullBaseline establishes the interpretation of effective data descriptions under
the immutable `rfc9880.model/1` inventory. This work uses the same composition,
roles and finite operation allowance as model checking. It neither requests a
sample nor searches for a value satisfying the description. The distinction
between assessed meaning, findings and unfinished work follows
[ADR 0003: Check findings and completeness](adr/0003-check-findings-and-completeness.md).

## Ordinary scalar descriptions

The scalar observations are made in the owning effective description, including
the final inherited qualities of each effective alternative:

| Obligation | Established model meaning |
| --- | --- |
| M042 | `number`, mathematical `integer`, Unicode-scalar `string`, and `boolean` selectors have their base information-level interpretations. Mathematical integer meaning includes a value written `10.0`. |
| M043 | An available ordinary scalar `const` fixes one exact value under the adopted equality contract. Numeric tokens retain mathematical value, strings retain decoded scalar identity, and Boolean and null constants retain their respective meanings. |
| M045 | Available exact numeric parameters establish inclusive and exclusive bounds and the positive integer-multiple meaning of `multipleOf`. Nonpositive steps and malformed parameters retain their existing definite findings. |
| M046 | Available exact nonnegative integral text lengths bound Unicode scalar counts inclusively. Supplementary characters count once; combining characters retain their individual scalar identities. Malformed parameters retain their existing definite findings. |

Absence of `type` imposes no authored type or origin. It does not prevent an
otherwise established numeric parameter or ordinary scalar constant from having
meaning. Independent formal numeric and literal admission belongs to M035-M037,
following [ADR 0032: Formal model admission](adr/0032-formal-model-admission.md)
and the [formal admission contract](formal-admission-contract.md).
For example, exact mathematical meaning for a count of `18446744073709551616`
remains established when Full separately finds its exclusion from the imported
CDDL `uint` domain.
Extension-only final constants retain unavailable extension and const meaning,
even when their framework literal admission is assessed. Ordinary type or const contradictions and
inverted bounds can have established meaning and existing advisory findings;
meaning assessment does not prove general satisfiability.

Reference composition and final alternatives determine the context. Established
nonempty Choice and Enum collections supply the final descriptions whose meaning
is assessed. Their shared owner and its original quality descendants do not impose
additional semantic constraints. An explicit branch quality replaces the shared
quality as a whole, including `properties` maps; surviving inherited qualities
remain needed in every final branch. A pattern or const replaced in every branch
therefore requests no additional matching or denotation capability. Source
grammar and existing registry guidance remain independently checked, and an
owner's documentary default remains stated. Unknown, malformed and empty
collections retain their existing handling. Restricted Direct items retain their admitted scalar vocabulary;
wider retained qualities and their inherited donor meaning do not become Full
meaning solely because a final alternative is Full. Explicit supported
overrides are assessed in that final context, following
[ADR 0024: Effective alternatives and nullable characteristics](adr/0024-effective-alternatives-and-nullable-characteristics.md).

Declared Extension features, unknown values of base qualities, unavailable
interpretations, origins, member identities, scalar values or numeric
magnitudes leave the affected observations undetermined. Independently
established descriptions retain their own observations. Required semantic work
is charged before scanning the description; exhausting it leaves requested work
unfinished. Diagnostic or report omission does not change established meaning.

Undeclared extension vocabulary leaves the base meaning of its description
intact, following
[ADR 0037: Undeclared extension vocabulary](adr/0037-undeclared-extension-vocabulary.md).
That is a qualified or unqualified Quality Name outside base SDF, such as
`acme:note`, `sdfProtocolMap` or a misspelled `minimun`, in a document that
declares no Extension feature. Full scalar and compound meaning and contextual
equality are established as if the quality were absent, and so are the
declared-value consistency and numeric advice of the expanded assessments. Its own meaning
stays unsupported, so M028 remains undetermined wherever it occurs, and its
existing notice or unfamiliar-name warning is retained. A base quality outside
its role, such as a wider Direct item quality, is not extension vocabulary and
keeps its unfinished outcome. So do unknown values of base qualities, documents
that declare an Extension feature and definitions composed from them. Reports
name the reading as described in the [report contract](report-contract.md).

Initial and explicitly expanded assessments retain their existing scopes.
FullBaseline requirement statuses remain available through typed report
accessors even when optional detail is omitted. Human and JSON output preserve
the same completion, disposition and inventory meanings described by the
[report contract](report-contract.md).

## Compound descriptions

An established Full array or map selector supplies its ordinary structural
meaning for M042. M050 follows every authored final item description; M053 follows
every authored final property description, including optional properties.
Empty or passing sample values provide no credit for these model obligations.
Nested Full properties and final alternatives can describe arrays; wider Direct
items retain their own vocabulary and unavailable inherited meanings.

M051 establishes inclusive exact mathematical item counts, separately from formal
CDDL admission in M037. M054 establishes decoded mandatory entry names under an
explicit map description. Required entries need no property description,
described entries remain optional unless required, and other entries remain
unconstrained. Malformed parameters retain their existing findings; understood
invalidity can still be assessed work.

M052 establishes adopted contextual equality capability for ordinary items and
the supported canonical byte-string, opaque UUID, Gregorian-date and dated date-time conventional domains,
recursively through maps, arrays and every final Choice interpretation. False
uniqueness requests no comparison capability. It does not select whichever
alternative admits a value or supplies a preferred equality result. Declared
Extension features, unknown base values, base qualities outside their role and
unsupported abstract interpretations remain unfinished, while undeclared
extension vocabulary does not affect comparator capability. Every
candidate at the same projected item or property must have compatible equality
meaning. A missing description supplies ordinary unconstrained meaning, so it
cannot silently select another alternative's abstract interpretation. Distinct
properties can retain distinct compatible meanings. Known time or URI
names do not establish complete comparison capability for every representation;
generic Unix-time equality retains the negative-coordinate gap. This is model
capability assessment, not a theorem that
every possible pair can be decided within any configured allowance.

UUID, Gregorian-date and dated date-time comparator capability require an established Full interpretation with
known effective nullability in every relevant alternative. Known absence uses
the Full default of true; explicit true and false are also supported. Null equals
null and differs from an established nonnull value in that compatible format. Two
nonnull dates must each establish valid Gregorian denotation before comparison;
the same calendar date is equal and different calendar dates differ. Dated
date-times compare their established UTC instants with exact fractions and
distinct inserted-second slots under ADR 0018. Model capability does not assert
that every possible dated operand has available adjustment knowledge. Unavailable or restricted
Direct nullability stays undetermined. Model-only capability assessment does not
parse a sample or record consultation of representation facts.

Actual formatted null constants can establish M043 in compatible civil, URI,
URI-reference and UUID contexts. Null/null needs no nonnull representation scan;
a mixed pair needs the nonnull operand's established denotation. Invalid or
uninterpreted operands retain unfinished equality. These null cases do not fill
the nonnull time or URI comparator gaps in M052. Equality remains
separate from admission, including when nullability excludes the constant.

For compound M043, contextual equality examines the actual effective const and
its needed subtree against itself. This establishes that stated value's
denotation and equality capability without inventing a sample, checking its
admission or proving satisfiability. Invalid abstract representations remain
undetermined. The shared const is superseded by final alternative constants.
The rules follow [ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md)
and [ADR 0016: Selected samples with explicit admission and coverage](adr/0016-selected-sample-contract.md).

Every description, alternative and actual const uses the original request's
semantic work, allocation, numeric and equality allowances. Fixed owner costs
are reserved before the Full interpretation phase; traversal capacities and
growth are charged before allocation. No child or alternative refills an
allowance. Limits keep the affected work unfinished, while optional report
omission preserves established observations.

## Patterns and known abstract interpretations

M048 establishes available modeled Unicode, unanchored matching by compiling
the surviving effective pattern with the adopted matching implementation. It
supplies no subject and invokes no match. M047 owns syntax independently; a
matching-only capacity can leave M048 unfinished with syntax assessed. A
pattern admitting no text, such as `(?!)`, still has known matching meaning.
Original worker, allocation, engine, interrupt and duration allowances remain
shared. Compilation does not guarantee runtime capacity for arbitrary future
subjects, following [ADR 0014: Bounded partial ECMAScript pattern capability](adr/0014-bounded-pattern-capability.md).

M026 and M027 establish the known byte-string and Unix-time model
interpretations, and M049 establishes the six known base format interpretations.
These observations require no constant, sample, denotation or temporal fact.
Absent `type` does not prevent recognition of the named model meaning. A
conflicting explicit type or combined abstract selectors can leave affected
interpretations unavailable without erasing independently established type or
numeric meaning. Actual special constants use their borrowed contextual
denotation and self-comparison for M043. An unsupported URI scheme, unavailable
temporal slot or negative POSIX denotation can therefore leave M043 unfinished
while the named model interpretation is assessed.

Known declarations no longer block unrelated base constraints. Byte text
qualities retain their specific limit: an authored length or pattern does not
establish an encoded-text or decoded-byte interpretation, so affected M046 or
M048 stays undetermined. Content-format declaration meaning belongs to M024;
unknown payload equality stays scoped to M043 and M052. Neither declaration
recognition nor comparator-capability assessment records actual operand or
external-knowledge consultation. These limits follow
[ADR 0007: Generated SDF and sample JSON](adr/0007-generated-sdf-and-sample-json.md),
[ADR 0018: Civil date and time sample conventions](adr/0018-civil-date-time-sample-conventions.md)
and [ADR 0019: URI, URI-reference and UUID sample conventions](adr/0019-uri-and-uuid-sample-conventions.md).

## Recorded extension vocabulary

FullBaseline checks `oma:id`, the first Recorded extension vocabulary, under
[ADR 0038: Recorded extension vocabulary](adr/0038-recorded-extension-vocabulary.md).
Its registered CDDL sockets place it on sdfObject and sdfAction definitions and
on every full data description: sdfData, sdfProperty, Action input and output
data, Event output data, object property entries and choice alternatives. At
those positions it does not stop the base meaning of its definition, and Full
checks its value against the CDDL `uint` domain of
[ADR 0032: Formal model admission](adr/0032-formal-model-admission.md). A
whole number from 0 through 2^64−1, such as `3303` or `3303.0`, is admitted.
A string, a negative or fractional number, a number above that range or any
other JSON value is an `extension.recorded_value_excluded` error. Its basis has
kind `registration`, the citation `OMNA SDF Quality Names registry` and the id
`oma:id`, and it is owned by M028 in `full.critical-extensions/1`. Either way
M028 is assessed. On an sdfEvent or sdfThing definition or a Direct item,
`oma:id` stays undeclared extension vocabulary, and so does every other `oma:`
name. The check covers value admission only, not
whether an ID names a real LwM2M Object or Resource.

Like unit checking under
[ADR 0027: Unit names and recorded registry guidance](adr/0027-unit-names-and-recorded-registry-guidance.md),
the check visits each final effective coordinate, including inherited values at
every consuming definition, the importer's overriding value and final
alternatives. An excluded inherited value is reported once for each definition
that copies it, and an importer's whole-number override clears it. A null
deletion removes the value from the effective definition. An unavailable value, an unfinished composition or exhausted work leaves M028
undetermined. The full-baseline optional-vocabulary notice leaves out `oma:id`
at its registered positions. Initial and expanded assessments neither check it
nor list its record, and their notice still counts it.

Where a FullBaseline check consulted the recorded evidence, a conclusion that
set `oma:id` aside relies on no further reading. Where a report did not consult
that evidence, such a conclusion relies on `undeclared-extensions-ignorable/1`
from
[ADR 0037: Undeclared extension vocabulary](adr/0037-undeclared-extension-vocabulary.md),
and the report names it as the [report contract](report-contract.md) describes.
That covers the numeric advice and declared-value consistency of the expanded
assessments and a FullBaseline check that withholds recorded evidence.

The check consults Recorded registry evidence `omna.sdf-quality-names` and lists
it in `baseline.registry_knowledge` once a value was compared with the recorded
domain. A caller that withholds recorded evidence gets that record as
unavailable with reason `caller_withheld`, no value is reported as excluded and
M028 stays undetermined. The
evidence combines two sources, both retrieved on 16 October 2026. The
[IANA SDF registry XML](https://www.iana.org/assignments/sdf/sdf.xml) was
retrieved at `2026-10-16T08:38:44Z`. Its SDF Quality Name Prefixes registry
holds one record, `oma`, dated 2026-05-12 and referring to the OMNA registry.
The XML's SHA256 is `0fdb79625d83a034b53ef15a007bb07a0e695bb58a60996866641acb5c06333e`,
and its raw bytes are a test fixture. The
[OMNA SDF Quality Names registry page](https://www.openmobilealliance.org/specifications/registries/sdfqualitynames/)
was retrieved at `2026-10-16T08:38:45Z`, with server modification date
`2026-10-15` and raw SHA256 `98098ac43cd36acca64e73e95b78bc3caca10f9c9d174f431c935d413423a62f`.
The site's terms leave the raw page out of the repository. Its introduction,
registry table and CDDL are kept as extracted text of 810 bytes with SHA256
`42c8be5e61ce56b45529160ec4e51d9cdaa4780cd5099948209f8c61c5b80b2f`. Extraction
policy `esdieff.recorded-extension-vocabulary/1` writes one row per registered
Quality Name, `name<TAB>sockets<TAB>type<LF>` with sockets in CDDL order and no
header. The single `oma:id` row occupies 79 bytes with SHA256
`58b2e81f1a27e35f2156f79c3361dc4fcd57d353831d3e62d466dec021392656`, which is the
record's digest. Its snapshot is
`omna.sdf-quality-names/2026-10-16/58b2e81f1a27e35f`, its scope
`oma-quality-names` and its `entries_count` 1. The `raw_digest` and
`source_updated` describe the OMNA page. A test compares the recorded positions,
value domain and canonical bytes against both captured sources. Production
reads neither.

The sixth record grows the full-baseline registry reserve. One more record map
and its text add 4608 bytes to the Full extension bound, so D is 62792 and
2D+208=125792=4E with E=31448. Six synthetic all-cap records occupy 3175 bytes,
and the projected fixed wire of 7271 bytes fits the new 7680-byte ceiling. The
six actual records occupy 2647 bytes, the OMNA record 453 of them. Two
generations, growth and owners of the actual maximum graph use 73732 of 125792
available bytes.
