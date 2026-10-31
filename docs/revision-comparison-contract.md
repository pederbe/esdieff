# Revision comparison

Esdieff compares two Model revisions from the Client's side and classifies each difference between their corresponding groupings as breaking, compatible or undetermined. The rules follow [ADR 0039: Client-side revision comparison](adr/0039-client-side-revision-comparison.md), and the terms Model revision, Corresponding definition, Client, Client-supplied data, Thing-supplied data, Client direction, Breaking change, Meaning quality, Breaking witness, Representative value, Monotone rule, Compatibility proof, Difference acknowledgement, Acknowledged difference and Version warning come from [the domain language](../GLOSSARY.md#revision-comparison). A difference Esdieff cannot classify stays undetermined, so it is never presented as safe.

```rust
use esdieff::{Catalogue, ComparisonRequest, SourceInput};
use std::path::Path;

let earlier = Catalogue::from_inputs(&[SourceInput::file("device.sdf.json", Path::new("v1/device.sdf.json"))])?;
let later = Catalogue::from_inputs(&[SourceInput::file("device.sdf.json", Path::new("v2/device.sdf.json"))])?;
let comparison = ComparisonRequest::new(&earlier, &later).compare()?;
println!("{}", comparison.to_human());
std::process::exit(comparison.disposition().exit_code().into());
```

The comparison contract identity is `esdieff.revision-comparison/4`. It binds which differences are classified and how. A release that classifies more differences gets a new identity, so a report keeps its meaning.

This page describes the current checkout. Contract `/4`, including the
pattern-aware representative candidates below, was added after release 0.2.0.
The serialized comparison envelope remains version `1.6` independently of this
classification-contract identity.

## Context metadata

The comparison assesses client interactions only. It does not assess compatibility for consumers of instance metadata, following [ADR 0049: Context revision comparison](adr/0049-context-revision-comparison.md).

Named Context definitions correspond within Objects and Things after inheritance and refinement. Their additions, removals and changed effective content appear as separate `sdfContext` differences, with no client data directions. Available Context differences are compatible for client interactions, including changed constraints, defaults and documentary content. This says nothing about whether a metadata consumer can accept the new values. Structural comparison uses exact numbers and unordered maps. It does not compare the abstract meaning of metadata values, including unsupported meanings such as `sdfType: "link"`, or settle disputed Context defaults.

A Context and Property with the same Given Name remain separate definitions. Moving a Property into Context still removes the old affordance and is breaking. A shared `sdfData` description is compared normally through each Property that reuses it, even when a Context uses it too. A Context origin never makes an actual Property change safe. Direct references from a non-Context role into Context remain unavailable under [ADR 0047: Context reuse with existing composition](adr/0047-context-reuse-with-existing-composition.md).

Unavailable effective content within a Context remains undetermined even beside a known structural difference. Unknown critical features and processing limits retain their scoped results. Independent affordance differences remain visible. Context gains no endpoint-presence or affordance-requiredness rule.

## Command line

`esdieff diff --old PATH --new PATH` compares an earlier and a later Model revision. Each PATH is a directory, prepared with the [directory discovery](directory-discovery.md) rules, or a single file, and both take the same form. `--recursive` and `--exclude RELATIVE_PATH` select both directories alike. Each document's source key is its path relative to its directory, so documents correspond by location. Two single files correspond directly, whatever their names. From the repository root, compare the minimal and rich door lock models:

```sh
cargo run -- diff --old examples/door-lock-check/minimal.sdf.json --new examples/door-lock-check/rich.sdf.json
```

`--format human|json` selects one of the reports below, and the shared color controls style the human report. `--format sarif` emits the report's [SARIF projection](report-contract.md#sarif-projection) for code-scanning annotations, and `--format markdown` its [Markdown changelog](report-contract.md#markdown-changelog) for release notes. `--acknowledged FILE` supplies the [acknowledged differences](#acknowledged-differences) of an intended revision. Exit codes follow `check` and the disposition: a comparison without unacknowledged breaking or undetermined differences exits 0, one with an unacknowledged breaking difference 1 and otherwise one with an unacknowledged undetermined difference 3. Without `--acknowledged`, a compatible comparison exits 0, a breaking one 1 and an undetermined one 3. [Version warnings](#version-warnings) never change the exit code. Invocation errors, such as a file beside a directory, `-` on either side, a missing `--old` or `--new`, an empty directory, a repeated `--acknowledged`, `--acknowledged -`, or an acknowledgement file that cannot be read or decoded, exit 2 with empty stdout before any document content is read. A report that cannot be written exits 4. The [CLI contract](report-contract.md#cli-contract) states every rule.

## Library operation

`ComparisonRequest::new(earlier, later)` takes the earlier and the later Catalogue, and `compare()` performs the comparison in one operation. Both Catalogues must use the same `Limits`, which set the one operation allowance described below. Different limits, or a report allowance too small for both revisions' mandatory report envelopes, are request errors. `recorded_registry_policy(policy)` sets the recorded evidence policy for unit names and Content-Format identifiers and codings, described under [Units](#units) and [Content formats](#content-formats). As for `check`, the default uses the recorded evidence. `acknowledgements(&acknowledgements)` supplies the Difference acknowledgements described under [Acknowledged differences](#acknowledged-differences).

`RevisionComparison` borrows both Catalogues and owns their effective models, which its typed differences and reports borrow, in the way model reports do. `catalogue(revision)` returns either Catalogue. `outcome()` returns the comparison's `Classification`. It is breaking when any difference is breaking, otherwise undetermined when any difference is undetermined, otherwise compatible. `Classification::disposition()` maps it to the `check` dispositions: compatible is passed and breaking is failed. `disposition()` is the comparison's own disposition, which only unacknowledged differences decide. Without acknowledgements it equals `outcome().disposition()`. `complete()` is true when no unacknowledged difference is undetermined, as the report's `outcome.complete` states. `count(classification)` counts every difference, including any whose details could not be retained, and `acknowledged_count(classification)` counts the acknowledged ones. `unused_acknowledgements()` yields the zero-based positions of the supplied entries that acknowledge no difference. `differences()` yields the retained differences with unacknowledged breaking ones first, then unacknowledged undetermined, acknowledged breaking, acknowledged undetermined and compatible. `compared_documents()`, `removed_documents()` and `added_documents()` describe document correspondence. `version_warnings()` yields the [Version warnings](#version-warnings) in the earlier revision's source order. `registry_knowledge(id)` returns the recorded registry evidence that unit and content-format comparison consulted or found withheld. `to_json()`, `to_human()` and `to_human_with_style(style)` render the reports below, `to_sarif(directory)` renders the SARIF projection and `to_markdown()` the Markdown changelog.

Each `Difference` has:

- `kind()`: `Removed` when only the earlier revision has the content, `Added` when only the later one has it, `Changed` for a quality whose effective value differs, including one present in only one revision, and `Unavailable` when an effective view could not establish the content, so whether it changed is unknown.
- `subject()`: a whole `Document`, a grouping or affordance as `Definition(class)`, a map `properties` `Entry`, or a `Quality(name)`.
- `classification()` and `reason()`. The reason is present exactly when the difference is undetermined, with a stable `code()` and an explanatory `message()`. A unit change the recorded evidence cannot settle has the reason `Unit(UnitReason)`, and a `contentFormat` change whose meanings cannot be compared has the reason `ContentFormat(ContentFormatReason)`.
- `directions()` and `affects(direction)`: the Client directions of the data the difference affects.
- `witness()`: the `Witness` of a data difference that is breaking because of one, described under [Breaking witnesses and compatibility proofs](#breaking-witnesses-and-compatibility-proofs).
- `side(revision)`: the difference in the earlier or later revision, absent when that revision lacks the corresponding definition, `properties` entry or document. A side has its `presence()` (present, absent or unavailable), its `root_document()` numbered within its revision, its effective `pointer(limit)`, and its `origin()`. The origin is the `Explanation` of the nearest content origin, or of the composition patch deletion that removed an absent quality.
- `acknowledged()`: whether a supplied entry acknowledges the difference, and `acknowledgement()`: the `DifferenceAcknowledgement` that would, present when the difference can be acknowledged and its effective pointers can be rendered.

Each `VersionWarning` has its `code()`, the earlier and later source IDs of its
document pair as `documents()`, its `earlier_version()`, its `later_version()`,
absent when the later document has none, and the pair's breaking difference
count as `breaking()`. `location(Revision::Later)` returns the later document's
source view at `/info/version`, or at `/info` or the document root when that
member is absent. `location(Revision::Earlier)` returns the earlier document's
`/info/version`. The serialized warning presents these as its primary and
related locations, respectively.

## Acknowledged differences

A maintainer who publishes an intended breaking revision, or has reviewed an undetermined change, supplies Difference acknowledgements, following [ADR 0040: Acknowledged revision differences](adr/0040-acknowledged-revision-differences.md). An Acknowledged difference keeps its classification, and the comparison's `result.outcome` stays as it is. Only unacknowledged differences decide the disposition, so an acknowledged breaking comparison passes while its report still says it breaks Clients.

An acknowledgement file is one JSON object:

```json
{
  "schema": "esdieff.acknowledged-differences",
  "version": "1",
  "differences": [
    {"kind": "removed", "subject": "sdfProperty", "classification": "breaking",
     "earlier": {"document": "lock.sdf.json", "pointer": "/sdfObject/Lock/sdfProperty/credential"},
     "later": null, "note": "Release 2 drops the credential"},
    {"kind": "changed", "subject": "quality", "quality": "minimum", "classification": "breaking",
     "earlier": {"document": "meter.sdf.json", "pointer": "/sdfObject/Meter/sdfProperty/power/minimum"},
     "later": {"document": "meter.sdf.json", "pointer": "/sdfObject/Meter/sdfProperty/power/minimum"}}
  ]
}
```

Each entry has a `kind` of `removed`, `added` or `changed`, a `subject` as in the [report](#report-16), a `quality` exactly when the subject is `quality`, a `classification` of `breaking` or `undetermined`, and `earlier` and `later` members. Each side is null or names the source key of the document holding the difference in that revision and its effective pointer, and at least one side is not null. An optional `note` is a comment for readers. An empty `differences` array is allowed. Unknown and duplicate members, two entries with the same identity, an entry of kind `unavailable` and an entry whose classification is `compatible` are refused.

`DifferenceAcknowledgements::from_json(bytes, limits)` decodes a file within the JSON decoding limits of `limits`, normally those of the revisions, and refuses limits a Catalogue would refuse, and `DifferenceAcknowledgements::new(entries)` builds the same value from typed entries. `DifferenceAcknowledgement::new(kind, subject, classification)` starts an entry, `earlier(document, pointer)` and `later(document, pointer)` locate it, and `with_note(note)` adds a note. Each validates as the file does, and `DifferenceAcknowledgements::new` refuses an entry without a location and duplicate identities. `entries()` lists them in file order, and each entry returns its `kind()`, `subject()`, `classification()`, `document(revision)`, `pointer(revision)` and `note()`.

An entry acknowledges a difference when the kind, subject, quality and classification are equal and, for each revision, both sides are null or the side's root document source key and rendered effective pointer equal the entry's `document` and `pointer` as exact decoded text. The note takes no part, and neither do request-local source IDs, content origins, directions, reasons or witnesses. An acknowledgement therefore still matches after an unrelated document is added to both revisions, and a shared definition seen through two groupings gives two differences that need two entries. Matching runs after the comparison over the retained differences, so it changes no classification. Breaking differences can be acknowledged, and so can undetermined differences of kind `removed`, `added` or `changed` whose content was examined, such as a reviewed added unit. A difference whose content nobody examined is never acknowledged: an `Unavailable` difference, a difference whose reason is `UndeterminedReason::Unavailable`, and a difference whose details the operation allowance could not retain. That reason covers the codes of [unavailable content](#unavailable-content), such as `reference.unavailable`, `data.number_unavailable`, `comparison.directions_unavailable` and `required.unavailable`, and `operation.limit`. A difference whose pointer cannot be rendered within the materialized-pointer allowance also stays unacknowledged. Each pointer an entry needs is rendered once, so the report's `acknowledgement` member and `acknowledgement()` give exactly the entry that matching compares. Because the classification is part of the identity, comparison rules that later classify an acknowledged undetermined difference as breaking bring it back for review: it is unacknowledged and its entry is unused. An entry that matches nothing is listed as unused and leaves the disposition unchanged.

The disposition is failed when any unacknowledged difference is breaking, undetermined when any unacknowledged difference is undetermined, and passed otherwise. `outcome.errors` counts unacknowledged breaking differences. `outcome.complete`, `incomplete_checks` and the requested coverage count unacknowledged undetermined differences.

Write an acknowledgement file for review with `--acknowledgements-output FILE`:

```sh
esdieff diff --old v1 --new v2 --acknowledgements-output reviewed.json
```

The file contains every eligible difference retained in the JSON report, including already acknowledged differences. Compatible differences and unavailable content have no entry. An empty comparison writes an empty `differences` array. Review the file before supplying it with `--acknowledged`. Exporting leaves the comparison report on standard output and preserves its exit status, so a breaking comparison still exits 1.

The destination must be a named file with an existing parent directory. An existing file requires `--replace-acknowledgements`, which permits replacement only of a regular non-symlink file. The destination cannot alias a selected document from either revision or the supplied acknowledgement file, including through hard links or parent-directory aliases. Destination validation precedes reading revision content. Delivery uses the same staged-write and input identity protections as `edit`. Destination or export-limit errors exit 2 with empty stdout. A write failure exits 4. If writing the report fails after export, the acknowledgement file has already been committed.

The file must fit the acknowledgement reader's limits below. Export refuses an oversized or otherwise unreadable file before changing the destination. It covers the differences retained by the JSON report, so consult its omissions when comparison details were truncated. All report formats export the same entries.

Each difference that can be acknowledged also carries the exact entry that would acknowledge it in the report, so you can project a file yourself:

```sh
esdieff diff --old v1 --new v2 --format json | jq '{schema: "esdieff.acknowledged-differences", version: "1", differences: [.result.differences[].acknowledgement // empty]}'
```

This covers the differences the report kept within its allowance. An acknowledgement file is at most 1 MiB, which `DifferenceAcknowledgements::FILE_BYTES` states, decoded with the ordinary JSON input policies, and holds at most 4096 entries. Each `document` holds 1 to 256 bytes, each `pointer` at most 8192 bytes with RFC 6901 syntax, each `quality` at most 256 bytes and each `note` at most 1024 bytes. The mandatory report envelope reserves five bytes per supplied entry for the unused positions, and a report allowance too small for that reserve is a request error.

## Correspondence

Documents correspond by source key, so the caller decides how documents pair. A document's path within its revision's directory is a natural key, and then a moved or renamed document has no correspondent. A model document only one revision has is a removed or added document, and each of its top-level groupings is a removed or added difference.

Top-level groupings correspond by their position in corresponding documents: the same class and Given Name. Nested groupings, affordances and map `properties` entries correspond by Given Name within their owner. A renamed definition therefore reads as a removal and an addition. Enum values correspond by value, so reordering an enum changes nothing. Choice alternatives correspond by Given Name and compare as the effective alternatives they derive, as described under [Choices](#choices). Corresponding `properties` entries are compared at any depth of Map data with the same rules as the data description that holds them, and each difference inside an entry is located at that entry. An array's `items` is compared in the same way, as the nested description of its values, described under [Array data](#array-data).

Every grouping of each corresponding document is compared through the effective view, including reusable Objects in shared documents. Top-level affordances and `sdfData` definitions are not compared as interfaces. Their changes appear through the effective groupings that use them, at any depth in their data, with the shared definition as the content origin.

## Whether a quality changed

Structural comparison decides whether an effective quality changed, using the
exact numbers of [ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md).
It does not derive an abstract-value context or establish denotation under
[ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md).
Numbers compare their exact values, so `10` and `1e1` are the same bound. Text
compares decoded Unicode scalar values, arrays are ordered and maps unordered.
`enum` values and map-data `required` names compare as sets. An absent readable,
writable or observable characteristic, or an absent nullable characteristic of
a full data description, equals its explicit specified default `true`. An absent
`uniqueItems` in a full data description equals an explicit `false`. The nullable
and unique-items defaults do not apply to a Direct items description itself.
Full descriptions nested inside its object properties or alternatives retain
their own roles. These defaults are comparison semantics, not inserted JSON.
A representation that ordinary equality cannot settle under another meaning,
such as two spellings of one instant, counts as a change and is classified like
any other. Two `contentFormat` declarations in data with one established
meaning, such as `50` and `application/json`, are the same, as described under
[Content formats](#content-formats).

Unchanged qualities produce no difference. `sdfRequired` instead compares by the requiredness it resolves to, described under [Requiredness](#requiredness).

## Classification

| Difference | Classification |
| --- | --- |
| Removed grouping or affordance | Breaking |
| Added grouping or affordance | Compatible |
| Property `readable`, `writable` or `observable` characteristic that becomes false, including a Specified default `true` replaced by an explicit `false` | Breaking |
| Property characteristic that becomes true, including an explicit `false` that becomes absent | Compatible |
| Input data added to an Action that had none, since existing invocations send none | Breaking |
| Input data removed from an Action, or output data removed from an Action or Event | Breaking |
| Output data added to an Action or Event that had none, which no Client of the earlier revision reads | Compatible |
| A declaration of both revisions that stops being mandatory in a corresponding grouping | Breaking |
| A declaration that becomes mandatory in a corresponding grouping | Compatible |
| Such a change, or a changed `sdfRequired`, while either revision's requiredness resolution is partial or unavailable | Undetermined, `required.unavailable` |
| Narrowed instance count range of a Grouping array in both revisions | Compatible |
| Any other changed `minItems` or `maxItems` of a Grouping array in both revisions | Breaking |
| Changed `minItems` or `maxItems` of a grouping that becomes or stops being an array, or whose counts do not form a range of exact nonnegative integers | Undetermined, `comparison.not_classified` |
| Changed `description`, `label` or `$comment`, or documentary `default` in data, other than a `label` or `default` inside an `items` description that carries a quality outside the Direct items vocabulary | Compatible |
| Changed `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `minLength`, `maxLength`, `multipleOf`, `enum`, `const`, `pattern`, checked `format`, `type`, `minItems`, `maxItems`, `uniqueItems` or Nullable characteristic in data, or an `items` description added to or removed from data, with a Breaking witness | Breaking |
| Such a change with a Compatibility proof | Compatible |
| Such a change without either | Undetermined, with a [witness reason](#breaking-witnesses-and-compatibility-proofs) |
| An alternative removed from a choice in data or added to one, or `sdfChoice` added to or removed from data as a whole, with a Breaking witness | Breaking |
| Such a change with a Compatibility proof, such as an alternative added to Client-supplied data or removed from Thing-supplied data | Compatible |
| Such a change without either | Undetermined, with a [witness reason](#breaking-witnesses-and-compatibility-proofs) |
| Changed `sdfChoice` in data whose alternatives either revision does not establish, such as beside an `enum` | Undetermined, `comparison.not_classified` |
| Changed map-data `required` that newly requires an entry in Client-supplied data, or drops a requirement from Thing-supplied data, with a Breaking witness | Breaking |
| Changed `required` with a Compatibility proof, such as a requirement added to Thing-supplied data or dropped from Client-supplied data | Compatible |
| Changed `required` without either | Undetermined, with a [witness reason](#breaking-witnesses-and-compatibility-proofs) |
| A `properties` entry that only the later revision describes and the earlier revision does not require | Compatible |
| A `properties` entry removed from map data, or one that only the later revision describes and the earlier revision requires, with a Breaking witness | Breaking |
| Such an entry with a Compatibility proof, such as one removed from Client-supplied data | Compatible |
| Such an entry without either | Undetermined, with a [witness reason](#breaking-witnesses-and-compatibility-proofs) |
| Any changed data quality or `properties` entry of a Property whose revisions offer no common Client direction | Compatible |
| Changed quality inside an `items` description, other than its `description` or `$comment`, when either revision's `items` carries a quality outside the Direct items vocabulary | Undetermined, `comparison.not_classified` |
| Changed `unit`, `contentFormat` or `sdfType` beside a choice in data, when every alternative replaces it | Compatible |
| Changed `unit` in data between two different current names in the recorded SenML evidence | Breaking |
| Any other changed `unit` in data | Undetermined, with its [unit reason](#units) |
| Changed `contentFormat` in data between two different established meanings | Breaking |
| Any other changed `contentFormat` in data | Undetermined, with its [content-format reason](#content-formats) |
| Changed `sdfType` in data between `byte-string` and `unix-time` | Breaking |
| `sdfType` added to or removed from data, with a Breaking witness | Breaking |
| `sdfType` added to or removed from data without a witness | Undetermined, with a [witness reason](#breaking-witnesses-and-compatibility-proofs) |
| Any other changed `sdfType` in data | Undetermined, `comparison.not_classified` |
| Changed `format` in data that either revision has outside the six [checked formats](#patterns-and-formats) or beside no `type: "string"` | Undetermined, `comparison.not_classified` |
| Any other changed quality | Undetermined, `comparison.not_classified` |
| Content an effective view could not establish | Undetermined, with the view's reason |

## Client directions

Each difference in a data description names the Client directions it affects. A writable Property's data and Action input are Client-supplied data. A readable or observable Property's data, Action output and Event output are Thing-supplied data. A Property that is readable and writable carries data in both directions. Effective Property characteristics, including Specified default characteristics, decide which directions each revision offers, and data is compared only in the directions both revisions offer, since gaining or losing a direction is a characteristic change. When a needed characteristic is unavailable, the Property's non-documentary data differences are undetermined with the reason `comparison.directions_unavailable`, apart from `properties` entries that no Client of the earlier revision uses, described under [Map entries](#map-entries).

A Property whose revisions offer no common direction carries data to or from no Client in both of them, so each of its data differences is compatible and names no direction. The characteristic changes decide, as when a Property stops being readable and becomes writable.

A changed characteristic names the direction it allows. `writable` allows Client-supplied data, and `readable` and `observable` allow Thing-supplied data, so a characteristic that becomes false names the direction it removes. Input or output data added to or removed from an affordance names its own direction. Other differences outside data descriptions, such as removed affordances and grouping qualities, affect no data direction.

## Breaking witnesses and compatibility proofs

A change to the values a data description admits is breaking only with a Breaking witness and compatible only with a Compatibility proof. Each changed quality that constrains admitted values has a monotone rule, which states whether the later revision admits more values than the earlier one under that quality, fewer, the same, or none of these. A Compatibility proof covers every changed quality of one data description: a Property's data, an Action's input or output, an Event's output, or one map `properties` entry, array `items` description or choice alternative of them. It holds when each changed quality's rule loosens in the safe direction for every Client direction the data carries, and nothing else in the description changed apart from documentary qualities. Client-supplied data may only widen and Thing-supplied data only narrow, so a readable and writable Property's value set cannot change compatibly. An added or removed `properties` entry has a rule of its own, and one that no Client of the earlier revision uses leaves the proof as it is, as described under [Map entries](#map-entries). A changed quality without a rule, another quality classified breaking or undetermined on its own, such as a breaking unit change or a changed `contentFormat` meaning, or unavailable content in the description leaves no proof. A changed `sdfType` changes how values read, so it never loosens in a safe direction.

Without a proof, Esdieff settles the candidate witnesses that the changed quality itself suggests, and for some qualities the [representative values](#representative-values) of the revision that must admit a candidate. At most 8 candidates are tried for one change, in the order its rule gives them, which bounds what settling a change costs. Sample-value checking checks each candidate against the effective description in both revisions, as it would a value supplied for that Property, input or output. A candidate that the earlier revision admits and the later one rejects is a witness for Client-supplied data, and one that the later revision admits and the earlier one rejects is a witness for Thing-supplied data. A witness counts only in a direction the data carries. Any other admission, including a candidate that another constraint such as a `const` excludes in both revisions, establishes nothing. A difference without a proof or a witness is undetermined with one of these reasons:

| Code | Applies when |
| --- | --- |
| `comparison.unproven` | No applicable candidate is a witness. |
| `comparison.witness_unsettled` | Sample-value checking left a candidate's admission undetermined, and no other candidate is a witness. |
| `operation.limit` | Generating representative values or a witness check ran out of the operation allowance. A check that runs out stays undetermined even after an established rejection. |

A candidate for a change inside a map `properties` entry is placed at that entry and checked against the whole Property data, input or output. Each enclosing map also holds a representative value of every other entry that either revision requires there, from the description of the revision that must admit the candidate, so the enclosing constraints still apply. So a candidate `23` for the `end` entry of a `window` entry is checked as `{"window":{"end":23}}`, or as `{"window":{"end":23},"level":0}` when the enclosing map requires an integer `level` with a minimum of `0`. When a filled entry has no representative value, or its description rejects that value in both revisions, the difference stays undetermined with `comparison.unproven`. A candidate for a change inside an array's `items` is placed as the items of that array, which holds it as often as the `minItems` of either revision requires, and at least once. So a candidate `100` inside the `items` of `levels` is checked as `[100]`, or as `[100,100]` when `levels` requires two items. Repeated items are not unique, so an enclosing array that requires two or more unique items rejects the candidate in both revisions, and an enclosing `minItems` beyond the value capabilities gives no candidate. Either leaves the difference unproven.

The other revision must read the enclosing values as the admitting revision does, or it could reject the candidate for another change alone. So a candidate is not placed when the revisions describe a filled entry differently apart from its documentary qualities, such as its `description` or a `$comment` deep inside it, or when the other revision's enclosing array could reject it for its length alone, with a `maxItems` below that length or with `uniqueItems` true for two or more items. Its difference then stays unproven unless another candidate witnesses it. So when two entries that a map requires both change what they admit, neither change gets a candidate placed beside the other entry. A candidate for a change inside a choice alternative stands where the description holding the choice does, since each alternative describes that description's values, so a candidate `5` for a `fan` entry of the `manual` alternative of a `mode` entry is checked as `{"mode":{"fan":5}}`.

### Representative values

A representative value is a simple value that an effective data description may admit. Esdieff generates them from the revision that must admit a candidate: the earlier revision for Client-supplied data and the later one for Thing-supplied data. A description's representative values are, in order:

1. a copy of its `const`, in which each number keeps its authored spelling
2. its enum values
3. a copy of its documentary `default`
4. one value for each base type it admits, every base type when it has no `type`, in the order number, string, boolean, map and array, or for a choice the first representative value of each of its effective alternatives, in their order
5. `null`, when it is Nullable

A number lies at the description's lower bound, or else at its upper bound: the bound itself when it is inclusive, and one unit inside it when it is exclusive, where the unit is 1 for an integer bound and one of the bound's last digit otherwise. Without a bound it is `0`. A string is a conventional value of a `format` that Sample-value checking checks beside `type: "string"`: `"1970-01-01T00:00:00Z"` for `date-time`, `"1970-01-01"` for `date`, `"00:00:00Z"` for `time`, `"https://example.com"` for `uri`, `""` for `uri-reference` and `"00000000-0000-0000-0000-000000000000"` for `uuid`. Otherwise it is `minLength` copies of `a`, which is `""` without a `minLength`. Boolean data gives `true`. A map holds every entry the description requires, each filled with the first representative value of its own description, and `null` for a required entry without a description. An array holds `minItems` copies of the first representative value of its `items`, or of `null` without an `items` description, and is `[]` without a `minItems`. A Direct items description has no Nullable characteristic, so `null` is never a representative value of `items`.

Without a conventional checked format value, strings first use [bounded pattern-aware synthesis](generation-contract.md#pattern-aware-representatives), combining the supported ASCII pattern subset with length limits. This also fills required map entries around a candidate witness. Unsupported patterns or impossible lengths retain the copies-of-`a` fallback above. Contract `esdieff.revision-comparison/4` supersedes `/3` because these candidates can establish additional Breaking witnesses. The classification rules and report version remain unchanged.

A value that would nest deeper than a generated value may, or a count beyond the value capabilities, is never generated. A representative value that its own description rejects is no witness, so the change it was generated for may stay unproven. Generation is charged to the one operation allowance, and generation that runs out of it leaves its difference undetermined with `operation.limit`.

A `Witness` has its `value()` as JSON text, its `direction()`, and its `admission(revision)` in each revision. The JSON report gives the same as the difference's `witness`, and the human report shows it on its own line beneath the difference.

### Numeric bounds

Bounds compare as exact numbers. `minimum` and `exclusiveMinimum` together set a description's lower bound, the tighter of the two, and `maximum` and `exclusiveMaximum` its upper bound. A change to either quality of a pair is judged by the effective bound the pair leaves. So an exclusive bound replacing an inclusive one at the same value narrows by that value alone, and a redundant bound can change without changing the admitted values. A raised lower bound, a lowered upper bound or an added bound narrows, and the reverse widens.

The candidates lie among the values the wider of the two revisions' bounds admits and the tighter one rejects. They are the tighter bound when it is exclusive and the wider bound when it is inclusive, each in its authored spelling, so a bound written `1e2` is the witness `1e2`. When neither bound value lies there, the candidate is one unit of the finer bound's last digit across an open edge, or half a unit when one unit reaches the other edge. Without a wider bound the unit is 1 where the coefficient digit limit allows, so integer data gets an integer. An exclusive minimum of `0` replaced by a minimum of `10` has the candidate `1`, a removed maximum of `2.5` the candidate `2.6`, and a removed maximum of `100` the candidate `101`.

### String lengths

`minLength` and `maxLength` bound how many Unicode scalar values a string holds, inclusively, and compare as exact nonnegative integer counts. An absent `minLength` reads as 0 and an absent `maxLength` as unbounded. So a raised `minLength`, a lowered `maxLength` or an added `maxLength` narrows, and the reverse widens, while an added `minLength` of `0` leaves the admitted values as they are. The first candidate is a string of `a` whose length lies just across the tighter bound: one shorter than a tighter `minLength` and one longer than a tighter `maxLength`. The representative values of the revision that must admit a candidate follow it. So a `minLength` of Client-supplied data raised from `2` to `4` is breaking with the witness `"aaa"`, a `maxLength` of Thing-supplied data raised from `6` to `8` with the witness `"aaaaaaa"`, and a `minLength` of `3` removed from Thing-supplied data with the witness `"aa"`. A length beyond the value capabilities is never generated, so such a change stays undetermined unless a representative value witnesses it. A count that is not an exact nonnegative integer neither narrows nor widens.

### Steps

`multipleOf` admits the exact integer multiples of a positive step, and the comparison decides multiples exactly. Every multiple of the earlier step is a multiple of the later one exactly when the earlier step is a multiple of the later step. So a later step that divides the earlier one widens, such as `0.5` replaced by `0.25`, and a later step that the earlier one divides narrows, such as `0.25` replaced by `0.5`. Any other change, such as `0.3` replaced by `0.5`, does neither. An added step narrows and a removed one widens. The earlier step is a candidate for Client-supplied data unless it is a multiple of the later step, and the later step is a candidate for Thing-supplied data unless it is a multiple of the earlier step, each in its authored spelling. Half a step is never a multiple of it, so half an added or removed step is the candidate, followed by `1` for an added or removed step above 1, which integer data may admit. The representative values of the revision that must admit a candidate come last. So `0.25` replaced by `0.5` breaks Client-supplied data with the witness `0.25`, `0.5` replaced by `0.25` breaks Thing-supplied data with the witness `0.25`, `0.3` replaced by `0.5` breaks them with `0.3` and `0.5`, a step of `0.5` added to Client-supplied data breaks it with `0.25`, a step of `3` added to integer Client-supplied data breaks it with `1`, and the same step removed from integer Thing-supplied data breaks it with `1`. A step that is not a positive exact number neither narrows nor widens.

### Patterns and formats

A `pattern` restricts string data to the strings it matches, so an added pattern narrows and a removed one widens. Esdieff does not decide whether one pattern matches every string another matches, so a changed pattern neither narrows nor widens. The candidates are the representative values of the earlier revision for Client-supplied data, when the later revision has a pattern, and those of the later revision for Thing-supplied data, when the earlier revision has one. So a pattern `^[0-9]+$` added to write-only string data is breaking with the witness `""`, and a removed one is compatible there. A changed pattern is breaking only when a representative value matches one pattern and not the other, and otherwise stays undetermined with `comparison.unproven`.

A `format` follows the same rule for the six formats that Sample-value checking checks beside `type: "string"`: `date-time`, `date`, `time`, `uri`, `uri-reference` and `uuid`. So `date-time` added to write-only string data is breaking with the witness `""`, and `date` replaced by `date-time` there is breaking with `"1970-01-01"`, the conventional value of the earlier format. Every URI is a URI-reference, and Sample-value checking reads it as the same URI under either format, so `uri` replaced by `uri-reference` widens and the reverse narrows. So `uri` replaced by `uri-reference` is compatible for Client-supplied data and breaking for Thing-supplied data with the witness `""`, which is a URI-reference but no URI. A change where either revision's `format` is another value, or sits beside no `type: "string"`, is undetermined with `comparison.not_classified`.

### Enum values

Enum values correspond by value as decoded strings, so reordering them changes nothing. A value only the earlier revision lists narrows the data, and a value only the later revision lists widens it, so an enum that loses some values and gains others does neither. Each such value is a candidate in the direction it breaks: a removed value for Client-supplied data and an added value for Thing-supplied data. So a value removed from a write-only Property's enum, or added to a read-only Property's enum, is breaking with that value as the witness, such as `"off"`, while the reverse changes are compatible. An enum added to a description narrows it and a removed enum widens it. The candidates of an added enum are the representative values of the earlier revision, for Client-supplied data, and those of a removed enum the representative values of the later revision, for Thing-supplied data. So an enum added to write-only string data is breaking with the witness `""`, which the enum excludes.

Each enum value becomes an alternative whose own `const` replaces any `const` beside the enum, and an enum beside an `sdfChoice` gives no alternatives, following [ADR 0024: Effective alternatives and nullable characteristics](adr/0024-effective-alternatives-and-nullable-characteristics.md). So an enum beside a `const` or an `sdfChoice`, or one that is not a nonempty array of strings, neither narrows nor widens.

### Choices

A choice admits the values that any of its effective alternatives admits, following [ADR 0024: Effective alternatives and nullable characteristics](adr/0024-effective-alternatives-and-nullable-characteristics.md). Alternatives correspond by Given Name, and each is compared as the effective alternative it derives, with the shared qualities it inherits and the branch qualities that replace them. Comparing the authored branches instead would read a branch that overrides a shared `maximum` of `10` with `20` as an added bound, although that alternative widened.

An alternative that only the earlier revision's choice has is a `Removed` difference of the `sdfChoice` quality, located at that alternative, and one that only the later revision's choice has is an `Added` one. A removed alternative narrows the data and an added one widens it, so an alternative removed from Client-supplied data or added to Thing-supplied data is breaking with a value of that alternative as the witness, and the reverse changes are compatible. The candidates are the representative values of that alternative, from the earlier revision for a removed one and the later revision for an added one. A candidate that another alternative still admits is no witness, so the difference stays undetermined with `comparison.unproven`.

Each alternative inherits every shared quality that its branch does not replace. A changed shared quality is reported once, at the description that holds the choice, and it is judged on each corresponding alternative that inherits it in both revisions, beside that alternative's own qualities. So a shared quality that every alternative replaces changes nothing and is compatible, and a raised shared `exclusiveMinimum` that a shared `minimum` makes redundant still narrows an alternative that replaces that `minimum`. A quality that an alternative's branch states in either revision is compared at that alternative, located there, so a branch that comes to override a shared `maximum` is a changed `maximum` of that alternative. The meaning qualities `unit`, `contentFormat` and `sdfType` follow the same distribution. A shared one tells a Client how to read only the values of the alternatives that inherit it in both revisions, so a change between two meanings that every alternative replaces is compatible, and one that an alternative inherits keeps its own classification. Beside a choice whose alternatives either revision does not establish, a change that would be breaking is not classified.

A choice added to or removed from data as a whole may admit values the other revision rejects, and reject values it admits, so it neither narrows nor widens. Its candidates are the representative values of the earlier revision for Client-supplied data and of the later revision for Thing-supplied data. An `enum` beside an `sdfChoice` derives no alternatives, and a choice with an unavailable branch does not establish all of them. A changed choice whose alternatives either revision does not establish is therefore not classified, and a shared quality beside it neither narrows nor widens.

### Type

The widening and narrowing rules here concern the `type` quality alone. Other
constraints still participate in candidate admission and compatibility proofs.

Every integer is a number, and a description without a `type` admits values of every type. So integer data that becomes number data widens, number data that becomes integer data narrows, an added `type` narrows and a removed one widens. A fraction separates integer data from number data. The candidates are `0.5`, then a fraction just inside each bound of the revision that admits fractions: an inclusive bound that is a fraction itself, and otherwise half a unit inside the bound, of 1 for an integer bound and of its last digit for another. So integer Thing-supplied data that becomes number data has the witness `0.5`, or `10.5` with a minimum of `10`, and integer Client-supplied data that becomes number data is compatible. Any other change between base types, such as string data that becomes boolean data, neither narrows nor widens. The candidates of every change of `type` other than one between integer and number are representative values: those of the earlier revision for Client-supplied data, when the type is added or changed, and those of the later revision for Thing-supplied data, when it is removed or changed. So string Client-supplied data that becomes boolean data has the witness `""`, the same change to Thing-supplied data the witness `true`, and a `type` added to Client-supplied data without one the witness `0`. A `type` other than the six base types neither narrows nor widens and suggests no candidate, so it stays undetermined.

### Nullable

The effective Nullable characteristic decides each revision, with its Specified default `true` where `nullable` is absent. Data that stops being Nullable narrows and data that becomes Nullable widens, and `null` is the candidate. So Client-supplied data that stops being Nullable and Thing-supplied data that becomes Nullable are breaking with the witness `null`, unless another constraint excludes `null` in both revisions, and the reverse changes are compatible. A `nullable` that is not a boolean neither narrows nor widens.

### Required map entries

Map data's `required` names compare as a set of decoded strings. A later revision that requires every entry the earlier one requires, and more, narrows, and one that requires only some of them widens. A requirement added to Thing-supplied data or dropped from Client-supplied data therefore loosens in the safe direction. A representative map holds exactly the entries its revision requires. So the representative values of the earlier revision lack every newly required entry, and are the candidates for Client-supplied data, and those of the later revision lack every dropped requirement, and are the candidates for Thing-supplied data. When the earlier revision requires `flow` and the later one `flow` and `label`, the Client-supplied witness is a map holding only `flow`, such as `{"flow":5}` for an integer `flow` with a minimum of `5`. A requirement dropped from Thing-supplied data has a witness map that lacks it, such as `{}` when nothing else is required.

### Map entries

Clients use only what the earlier revision describes or requires, following [ADR 0039: Client-side revision comparison](adr/0039-client-side-revision-comparison.md). A `properties` entry that only the later revision describes, and that the earlier revision does not require, is part of no Client's interaction, so it is compatible in every Client direction, whatever else changes in its map and even when the Property's directions are unavailable. So an optional `note` added to write-only or read-only map data is compatible. Beside a choice, the earlier revision also requires an entry that a corresponding alternative requires when that alternative inherits the entries in both revisions, as a [changed shared quality](#choices) is judged. So when an earlier `scheduled` alternative requires an undescribed `start`, a `start` of string data described beside the choice of write-only data is breaking with the witness `{"start":0}`. An earlier choice whose alternatives either revision does not establish, or that the later revision lacks, may require the entry too, so the entry is settled as a required one. When the later revision also comes to require such an entry, its changed `required` decides, as described under [Required map entries](#required-map-entries). So when map data that requires `zone` comes to require `zone` and a newly described `note`, the change is compatible for Thing-supplied data, and breaking for Client-supplied data with a witness map lacking the note, such as `{"zone":1}` for an integer `zone` with a minimum of `1`.

Maps are open, so an entry without a description admits every value. An entry whose description is removed therefore widens its map. An entry that the earlier revision requires without describing it narrows its map when the later revision describes it. A removed entry is compatible for Client-supplied data and breaking for Thing-supplied data with a Breaking witness. A newly described entry that the earlier revision requires is compatible for Thing-supplied data and breaking for Client-supplied data with a Breaking witness. The candidates are maps holding the entry with one value of each JSON kind, `0`, `""`, `true`, `{}`, `[]` and `null` in that order, beside a representative value of every other entry that either revision requires there, as for a [nested candidate](#breaking-witnesses-and-compatibility-proofs). So when read-only map data requires such a `zone`, a removed `flow` entry of number data is breaking with the witness `{"flow":"","zone":1}`. When write-only map data requires such a `zone` and a `note` in both revisions, and only the later revision describes the note as string data, the note is breaking with the witness `{"note":0,"zone":1}`. A removed entry whose earlier description admits every candidate, such as one with only a `description`, stays undetermined with `comparison.unproven`.

A candidate is therefore placed only at an entry that the earlier revision describes or requires. A candidate for a change inside an entry stands at an entry that both revisions describe, and the candidate of an added or removed entry stands at that entry. The other entries around a candidate are filled as for any nested candidate, so they hold every entry that either revision requires, including one that only the later revision requires.

### Array data

An array's `items` is the nested description of its values. Its changes are
compared like those of a map `properties` entry: they are settled together,
apart from the array's own qualities, and each is located inside `items`. An
`items` description is a [Direct items description](../GLOSSARY.md#data-descriptions),
whose grammar vocabulary is `sdfRef`, `type`, `sdfChoice`, `enum`, `minimum`,
`maximum`, `format`, `minLength`, `maxLength`, `description` and `$comment`, with
`required` and `properties` when its `type` is `object`, following
[ADR 0024: Effective alternatives and nullable characteristics](adr/0024-effective-alternatives-and-nullable-characteristics.md).
Grammar admission does not establish every processing meaning, including
Direct-item imports. A description carrying wider qualities such as `const`,
`pattern`, `exclusiveMinimum`, `nullable` or `label` has no complete supported
interpretation. When either revision's `items` carries such a quality, the
comparison leaves every change inside it other than `description` or `$comment`
undetermined with `comparison.not_classified`, including a narrowed bound
beside it. Independent known sample violations can still reject a candidate.
A Direct items description has no nullable characteristic, so an absent
`nullable` there differs from an explicit one, and `null` is never a
representative value of `items` itself. Its object properties and choice
alternatives contain full descriptions with their own roles.

An array without an `items` description admits items of every type, so an `items` description added to array data narrows it and a removed one widens it. The candidates are arrays of `0`, `""`, `true`, `{}` and `[]`, in that order, each repeated as often as the `minItems` of the revision that must admit them requires, and at least once. So an `items` description of integers added to Client-supplied data, or removed from Thing-supplied data, is breaking with the witness `[""]`. An `items` that is a description in one revision and something else in the other is not classified.

`minItems` and `maxItems` are inclusive counts that compare as exact numbers. An absent minimum reads as 0 and an absent maximum as unbounded, so an added `minItems` of `0` changes nothing. A raised minimum, a lowered maximum and an added maximum narrow, and the reverse changes widen. The candidate is an array of the count just across the tighter bound: one item short of the higher minimum, or one item over the lower maximum. Each item is a copy of the first representative value of the `items` of the revision that must admit the candidate, or `null` without an `items` description. So when write-only data's items are integers with a minimum of `0`, a `minItems` raised from 1 to 3 is breaking with the witness `[0,0]`, and a `maxItems` lowered from 4 to 2 with the witness `[0,0,0]`. A count that is not a nonnegative integer neither narrows nor widens, and a candidate count beyond the value capabilities is never generated, so its change stays unproven.

`uniqueItems` reads as `false` where it is absent, so an explicit `false` is no change. Data that comes to require unique items narrows, and data that stops requiring them widens. The candidate repeats one representative item, twice or as often as the `minItems` of the revision that must admit it requires. So `uniqueItems` that becomes `true` on Client-supplied data is breaking with a witness such as `[0,0]`, and the reverse change on Thing-supplied data with the same witness. Repeated items are never unique, so a count candidate of two or more items for an array that requires unique items stays unproven.

### Constants

A `const` restricts a description to one value, unless it sits beside an enum, whose values each replace it. So an added `const` narrows and a removed one widens. A changed `const` neither narrows nor widens. The candidates for Client-supplied data are the representative values of the earlier revision, when the later revision has a `const`, and those for Thing-supplied data the representative values of the later revision, when the earlier revision has one. A changed `const` therefore has the earlier constant as its first Client-supplied candidate and the later constant as its first Thing-supplied candidate, each copied with its numbers in their authored spelling. So a constant added to write-only string data is breaking with the witness `""`, a constant `"open"` changed to `"shut"` there with the witness `"open"`, and a removed constant is compatible there.

### sdfType

RFC 9880 Section 4.7.1 names the semantic types `byte-string` and `unix-time`.
Esdieff checks sample JSON using its recorded base64url and POSIX-coordinate
interpretations, as described in the [sample contract](sample-contract.md).
These interpretations do not select a device wire protocol. The types tell a
client how to read a value, so a change between them is breaking in every client
direction the data carries, without a witness, following the meaning qualities
of [ADR 0039: Client-side revision comparison](adr/0039-client-side-revision-comparison.md).
A type added or removed changes how values read as well, so the change neither
narrows nor widens, and it is breaking only with a breaking witness. The
candidate lies outside Esdieff's selected JSON representation: `"!"`, outside
the base64url alphabet, for `byte-string`, and `""`, which is not a number, for
`unix-time`. Thus `byte-string` added to client-supplied string data, or removed
from thing-supplied string data, is breaking with the witness `"!"`. The reverse
changes, and a candidate that another constraint such as a `pattern` already
excludes, stay undetermined with `comparison.unproven`. Any other `sdfType` value
is not classified.

## Requiredness

Each grouping's mandatory declarations come from its Required declaration relationships, following [ADR 0026: Required declaration relationships and source evidence](adr/0026-required-declaration-relationships-and-source-evidence.md). An `sdfRequired` entry of a grouping makes the declaration it selects mandatory in that grouping. An affordance's `sdfRequired` and a `true` carrier belong to the grouping that encloses the definition carrying them, so a carrier makes its own affordance or grouping mandatory there. A declaration required from an enclosing grouping is represented only inside the groupings between them, so those groupings are mandatory there too, and the declaration is mandatory in each of them. Requiredness then closes through mandatory groupings: whatever is mandatory in a mandatory grouping is also mandatory in the grouping that requires it.

Corresponding groupings compare these closures at every depth, and declarations correspond by their position in the document. A declaration that both revisions have and that stops being mandatory in a grouping is breaking, because a Thing may now omit what a Client uses. A declaration that becomes mandatory only obliges Things to provide more, so it is compatible. A declaration that only one revision has is a removed or added definition of its own and does not count here. Each grouping whose mandatory declarations change has one changed `sdfRequired` quality, located at that grouping's `sdfRequired`, present or absent, and it affects no data direction. It is breaking when any declaration stops being mandatory there, even when others become mandatory.

Since requiredness is compared after resolution, `"temp"`, `"#/sdfObject/Oven/sdfProperty/temp"` and a `true` carrier in `temp` spell one requirement, and replacing one with another is no change. A requirement that a Thing moves to a required Object inside it, which then requires the declaration itself, is no change either. An Object that keeps requiring `temp` while its enclosing Thing stops requiring it is breaking at the Thing, because the Object may now be absent.

A revision's resolution is partial or unavailable when a name selects no declaration, a reference is ambiguous, unavailable or malformed, a target has no effective declaration, or the traversal could not see every definition. Then a grouping whose known mandatory declarations differ, or whose own `sdfRequired` or that of a corresponding direct affordance changed, is undetermined with the code `required.unavailable`, and an unchanged requirement stays no change. `sdfRequired` in data-only descriptions selects no declaration of a grouping, so it is compared as a data quality and stays not classified.

## Grouping instance counts

`minItems` and `maxItems` make an `sdfObject` or `sdfThing` a Grouping array and bound its instance count inclusively. Things decide how many instances they have, so the range behaves like Thing-supplied data, following [ADR 0039: Client-side revision comparison](adr/0039-client-side-revision-comparison.md). When both revisions are arrays, a raised minimum, a lowered maximum or an added maximum narrows the range and is compatible, and a lowered or removed minimum or a raised or removed maximum widens it and is breaking. An absent minimum reads as 0 and an absent maximum as unbounded, so an added `minItems` of `0` leaves the range as it is and is compatible. Counts compare as exact numbers, each change is judged at its own end of the range, and none has a witness or a data direction. A grouping that becomes or stops being an array changes how its instances are addressed, so its count changes are undetermined with `comparison.not_classified`, and so are counts that are not exact nonnegative integers or a minimum above the maximum.

## Units

A changed `unit` in a data description follows the unit rule of [ADR 0039: Client-side revision comparison](adr/0039-client-side-revision-comparison.md) and the recorded evidence of [ADR 0027: Unit names and recorded registry guidance](adr/0027-unit-names-and-recorded-registry-guidance.md), described in the [unit contract](unit-contract.md). A change between two different current names breaks every Client direction the data carries, because Clients would misread its values. A current name has a nonlegacy entry in the recorded SenML evidence, from either its SenML Units or its Secondary Units section. So `Cel` to `K`, `ms` to `s` and `Wh` to `kWh` are all breaking.

Any other unit change is undetermined, with the first reason that applies:

| `UnitReason` | Code | Applies when |
| --- | --- | --- |
| `Added` | `comparison.unit_added` | Only the later revision states a unit. |
| `Removed` | `comparison.unit_removed` | Only the earlier revision states a unit. |
| `UriName` | `comparison.unit_uri_name` | Either name contains a colon, which makes it a URI name under RFC 9880 Section 4.7. |
| `EvidenceWithheld` | `registry.caller_withheld` | The request withholds the recorded evidence the change needs. |
| `UnrecordedSpelling` | `comparison.unit_unrecorded` | Either name has no entry in the recorded evidence, such as `cel` or `kwh`. Lookup compares exact decoded text, without trimming or case folding. |
| `LegacyName` | `comparison.unit_legacy` | Either name is a legacy entry. Legacy names can give one unit several names, such as `beat/min` and `1/min`. |

A unit that is not text leaves the change undetermined with `comparison.not_classified`. A `unit` outside a data description is compared like any other unclassified quality. When a needed Property characteristic is unavailable, the change is undetermined with `comparison.directions_unavailable`, as for other data qualities.

Only names that need the recorded evidence are looked up, so added and removed units and URI names stay undetermined with their own reasons under either policy. With `RecordedRegistryPolicy::WithholdRecorded`, every other unit change is undetermined with `registry.caller_withheld`, and no recorded table is consulted. Each consulted row is charged to the one operation allowance. A lookup that exhausts it leaves its unit change undetermined with `operation.limit`.

## Content formats

A changed `contentFormat` in a data description follows the meaning qualities of [ADR 0039: Client-side revision comparison](adr/0039-client-side-revision-comparison.md) and the bounded model meaning of [ADR 0033: Content-format model meaning](adr/0033-content-format-model-meaning.md), described in the [content-format contract](content-format-contract.md#bounded-full-model-meaning). A declaration's established meaning is its media type and subtype compared without ASCII case, its parameter meaning, and its content codings in order:

- A media name has established parameter meaning only under the supported rules: `application/json` without parameters or with one charset, which has no effect, `application/cbor` without parameters, and `text/plain` in its US-ASCII default or with one `utf-8` or `us-ascii` charset. Parameter names and these charset values compare without ASCII case, and quoted and token spellings are equivalent.
- Each coding needs a positive entry in the recorded HTTP content codings, compared without ASCII case. The reserved `identity`, an unrecorded token, and the deprecated `x-gzip` and `x-compress`, which RFC 9110 Section 8.4.1 makes equivalent to other codings, have no established meaning.
- A numeric identifier means its complete recorded CoAP tuple, without a separate coding lookup. Its content type is read through the same media rules where they apply, so `50` means `application/json` and `0` means `text/plain; charset=utf-8`. A tuple outside those rules, such as `21` for `image/gif`, keeps its registered parameters, and distinct tuples are distinct registrations.

Two declarations with one meaning, such as `50` and `application/json`, `Application/JSON` and `application/json`, or `text/plain` and `text/plain; charset=us-ascii`, are no change and produce no difference. Two different established meanings make Clients misread values in every Client direction the data carries, so `application/json` to `text/plain`, `application/json` to `application/json@gzip`, `@gzip@deflate` to `@deflate@gzip`, and `21` to `22` are breaking. Sample-value checking never establishes a value beside a `contentFormat`, so no witness settles these changes.

Any other `contentFormat` change is undetermined, with the first reason that applies:

| `ContentFormatReason` | Code | Applies when |
| --- | --- | --- |
| `Added` | `comparison.content_format_added` | Only the later revision declares a content format. |
| `Removed` | `comparison.content_format_removed` | Only the earlier revision declares a content format. |
| `Unestablished` | `comparison.content_format_unestablished` | A declaration has no established meaning: its spelling fails the RFC 9193 grammar, its identifier is out of range or unrecorded, its media name or parameters lie outside the supported rules, such as `image/gif`, or a coding has no established meaning. One media name with parameters both inside and outside those rules, which no recorded tuple has, is unestablished too. |
| `EvidenceWithheld` | `registry.caller_withheld` | The request withholds the recorded CoAP Content-Formats or HTTP content codings that a declaration needs. |

Both spellings and their media rules are read before any recorded evidence, so a declaration without meaning on its own spelling is unestablished under either policy, and the supported media rules need no recorded evidence. Then the earlier declaration's identifier or codings are looked up before the later one's. With `RecordedRegistryPolicy::WithholdRecorded`, a change that needs a numeric identifier or a coding is undetermined with `registry.caller_withheld`, and the withheld registry is named in the report. A `contentFormat` that is not text leaves the change undetermined with `comparison.not_classified`, and a `contentFormat` outside a data description is compared like any other unclassified quality. When a needed Property characteristic is unavailable, a change that is not a respelling is undetermined with `comparison.directions_unavailable`.

Classification, the media rules, every compared row and the comparison itself are charged to the one operation allowance, and a comparison that exhausts it leaves its change undetermined with `operation.limit`. A comparison that consults the media rules or a recorded tuple names `content-format-model-meaning/1` in `baseline.interpretations`.

## Unavailable content

A revision with findings, unresolved references or other unavailable content is still compared, following [ADR 0006: Scoped reference failures](adr/0006-scoped-reference-failures.md). Only the differences that depend on content an effective view could not establish are undetermined, each with a reason from that content. Content the view could not establish is an `Unavailable` difference whose reason carries the view's code, and a change whose Client directions or requiredness depend on such content is undetermined with `comparison.directions_unavailable` or `required.unavailable`. Independent differences keep their classifications, so a breaking removal elsewhere still decides the outcome.

- An unresolved reference, an effective reference cycle or an unavailable dependency leaves the affected definition, `properties` entry or quality undetermined, with a code such as `reference.unavailable` or `reference.effective_cycle`. Content hidden by an unavailable owner never reads as removed or added.
- A document that could not be acquired or decoded keeps its acquisition result in its source row under its revision, and its revision's scope lists it in `unavailable_required`. Each top-level grouping of the corresponding document in the other revision is an `Unavailable` definition with the acquisition reason, such as `json.syntax`, `document.envelope` or `input.acquisition`. Its side in the failed revision is that document, with presence `unavailable`, so no grouping reads as removed or added. When neither document has a grouping to name, the document itself is one `Unavailable` difference.
- A document that declares an Extension feature is deferred as a whole, following [ADR 0003: Check findings and completeness](adr/0003-check-findings-and-completeness.md). It is compared like an undecodable document, with the code `structure.feature_context_unavailable`, and a definition importing content from it is undetermined with the same code.
- Undeclared extension vocabulary leaves the base meaning of its definition intact, following [ADR 0037: Undeclared extension vocabulary](adr/0037-undeclared-extension-vocabulary.md). Base qualities beside it keep their classifications, and a change to the vocabulary itself is undetermined as not classified. `baseline.interpretations` names `undeclared-extensions-ignorable/1` when a breaking or compatible classification concerns a quality of a definition or data description carrying extension vocabulary, or rests on a witness whose checks set aside such vocabulary deeper in the description. Recorded extension vocabulary such as `oma:id` counts too, because the comparison does not consult its record, following [ADR 0038: Recorded extension vocabulary](adr/0038-recorded-extension-vocabulary.md).

## One operation allowance

One operation allowance covers both revisions' acquisition, composition and comparison, following [ADR 0009: Bounded processing and reporting](adr/0009-bounded-processing-and-reporting.md) and [ADR 0013: Shared workflow and report guarantees](adr/0013-shared-workflow-and-report-guarantees.md). No revision or phase refills it. Both revisions' parsed nodes, parsed allocation, decoding work, numeric work and retained bytes together must fit the shared `Limits`. When the later revision's acquisition exceeds what the earlier one left, every later document is unavailable with `operation.limit`. Composition and comparison then share one work, allocation, node and reference allowance, which also covers collecting each composed document's Required declaration relationships and closing its requiredness. Corresponding documents are composed and compared one pair at a time in the earlier revision's source order, followed by removed and then added documents. Monotone rules and witness checks charge the same allowance, and a witness check that runs out of it leaves its difference undetermined with `operation.limit`. When the allowance runs out, the rest of that document and every later one are undetermined with `operation.limit`. A document whose composition ran out is unavailable on that side, and a document whose comparison ran out gains one document difference with both sides present. Differences already found keep their classifications, and a difference whose details the allowance could not retain still counts. In the description where the allowance ran out, a changed quality without a monotone rule, in data that a common Client direction reaches, stays undetermined with `comparison.not_classified`. Every other change there that was still waiting to be settled is undetermined with `operation.limit`. Each revision keeps its own source-attempt ceiling, because source IDs are numbered within each revision.

## Version warnings

A Client that watches a model's `info` `version` misses a breaking revision that keeps or lowers it. RFC 9880 Section 3.1 recommends a version that is lexicographically increasing over a model's life, so Esdieff warns about such a revision, following [ADR 0044: Versions of breaking revisions](adr/0044-versions-of-breaking-revisions.md). A compared document pair gets one Version warning with the code `comparison.version_not_increased` when all of these hold:

1. The document's bytes changed between the revisions.
2. The comparison of the pair found at least one breaking difference, acknowledged or not. Every difference found while comparing a pair is located in its documents.
3. The earlier revision's `info` `version` is text, and the later revision's is absent or is text that does not compare higher by Unicode code point order.

Comparison is plain code point order, without date parsing, so `2020-07-06` after `20200706` and `2019-11-14` after `2019-11-14` both warn, and `2019-11-15` after `2019-11-14` does not. A pair whose document is unchanged gets no warning, even when a document it imports changed. A breaking difference belongs to the pair in whose effective groupings it is located, so a changed document that holds only shared definitions, such as `sdfData` that other documents import, gets no warning of its own. Neither does a pair whose earlier version is absent or not text, or whose later version is present but not text, nor a removed or added document. A breaking difference whose details the operation allowance could not retain still counts.

The warning never changes a classification, the outcome, the disposition or the exit code, and `strict_warnings` stays false. Reading the two versions and comparing the document bytes is linear in content both Catalogues already admitted, so it charges nothing to the operation allowance.

## Report 1.6

Operation `diff` uses the `esdieff.report` envelope with version `1.6`, which adds the explicit client-interaction compatibility scope to version `1.5`. Version `1.5` added [Version warnings](#version-warnings) to version `1.4`. Version `1.4` added the acknowledgement fields and the disposition they decide to version `1.3`. Every diff report is emitted at `1.6`, with or without acknowledgements or warnings. The shared source, outcome and diagnostic meanings of the [report contract](report-contract.md) remain intact. Both Catalogues number their sources from 1, so each source names its revision, and every ID inside an `earlier` or `later` member selects a source of that revision.

| Object | Required fields and types | Meaning |
| --- | --- | --- |
| Diff request | assessment:null; inventory:null; comparison:`"esdieff.revision-comparison/4"`; scope:{earlier:Scope, later:Scope} | Each revision has catalogue scope. `assessed` lists its model documents. `unavailable_required` lists documents whose retained source JSON is unavailable, rather than every document with incomplete effective composition. |
| Diff source | Source fields; revision:`"earlier"|"later"` | Every acquisition attempt of both revisions. |
| Diff policy and baseline | policy.recorded_registry:`"use_recorded"|"withhold_recorded"`; policy.acknowledgements:Count; policy.compatibility_scope:`"client_interactions"`; baseline.registry_knowledge:RegistryKnowledge[]; baseline.extensions:Extension[] | The request's recorded evidence policy and the number of supplied acknowledgement entries, 0 without any. Registry knowledge lists each recorded unit section and content-format registry that unit and content-format comparison consulted or found withheld, as in `check` reports, and is empty when no change needed it. |
| Diff outcome | complete:boolean; errors:Count; warnings:Count; notices:0; incomplete_checks:Count; strict_warnings:false; disposition | `errors` counts unacknowledged breaking differences. `warnings` counts Version warnings, including omitted ones, and changes neither completeness nor the disposition. `complete` is false and `incomplete_checks` is 1 when any unacknowledged difference is undetermined. The disposition is failed when any unacknowledged difference is breaking, otherwise undetermined when any unacknowledged difference is undetermined, otherwise passed. Without acknowledgements it is failed for a breaking comparison, undetermined for an undetermined one and passed for a compatible one. |
| Diff diagnostics | retained:Count; omitted:Count; truncated:boolean; reasons:string[]; omissions:Omission[]; items:VersionCondition[] | Each item is one retained Version warning. `omitted` counts the Version warnings whose details did not fit, with the reason `byte_limit`. |
| VersionCondition | code:`"comparison.version_not_increased"`; severity:`"warning"`; basis:{kind:`"advice"`, id:`"comparison.version_not_increased"`, citation:`"RFC 9880 Section 3.1"`}; message:string; primary:RevisionSourceLocation; related:[RevisionSourceLocation]; arguments:{earlier_version:string, later_version?:string, breaking_differences:Count} | The primary location is the later document's `/info/version`, or its `/info` or root when that member is absent, and the related location is the earlier `/info/version`. `later_version` is absent exactly when the later document has no version. |
| RevisionSourceLocation | SourceLocation fields; revision:`"earlier"|"later"` | A source location whose source ID selects a source of the named revision. |
| Diff requested coverage | inventory:null; count_unit:`"comparison"`; total:1; assessed:Count; not_applicable:0; unassessed:0; undetermined:Count | The one comparison is assessed when no unacknowledged difference is undetermined. Broader coverage stays partial, because a comparison assesses no RFC requirements. |
| DiffResult | kind:`"diff"`; outcome:`"breaking"|"compatible"|"undetermined"`; summary:Summary; documents:Documents; unused_acknowledgements:Count[]; differences:Difference[]; differences_omitted:Count; omissions:Omission[] | The outcome classifies every difference, acknowledged or not. `unused_acknowledgements` lists the zero-based positions of the supplied entries that acknowledge no difference. Summary counts, the outcome and the unused positions survive any omission of difference details. |
| Summary | breaking:Count; undetermined:Count; compatible:Count; acknowledged:{breaking:Count, undetermined:Count}; documents:{compared:Count, removed:Count, added:Count} | Every difference, including omitted ones, the acknowledged differences among them, and the compared and unmatched documents. |
| Documents | compared:{earlier:Id, later:Id}[]; removed:Id[]; added:Id[] | Removed IDs are earlier sources and added IDs later sources. |
| Difference | kind:`"removed"|"added"|"changed"|"unavailable"`; subject:`"sdfContext"|"document"|"sdfThing"|"sdfObject"|"sdfProperty"|"sdfAction"|"sdfEvent"|"properties_entry"|"quality"`; directions:(`"client_supplied"|"thing_supplied"`)[]; classification:`"breaking"|"compatible"|"undetermined"`; acknowledged:boolean; earlier:Side|null; later:Side|null | quality:string when subject is `quality`; reason:{code:string, message:string} exactly when undetermined; witness:Witness exactly when breaking through a Breaking witness; acknowledgement:Acknowledgement when the difference can be acknowledged and its effective pointers render within the materialized-pointer allowance. Null means that revision lacks the corresponding definition, `properties` entry or document. |
| Acknowledgement | kind; subject; classification; earlier:{document:string, pointer:string}|null; later:{document:string, pointer:string}|null | quality:string when subject is `quality`. The exact acknowledgement file entry that would acknowledge the difference, without a note. |
| Witness | direction:`"client_supplied"|"thing_supplied"`; raw_json:string; admission:{earlier:`"admitted"|"rejected"`, later:`"admitted"|"rejected"`} | The value as JSON text, the direction it breaks and its admission in each revision. The earlier revision admits a Client-supplied witness and the later one a Thing-supplied witness. |
| Side | presence:`"present"|"absent"|"unavailable"`; location:EffectiveLocation; origin:SourceLocation|null | The effective location in that revision. An absent quality's location names where it would be. The origin is the nearest content origin, or the deletion that removed an absent quality, and null when there is none. |

Version warnings, then differences, are optional detail within the report allowance left after the mandatory envelope. Warnings come first, in the earlier revision's source order, and one that does not fit ends the list. Differences are listed unacknowledged breaking first, then unacknowledged undetermined, acknowledged breaking, acknowledged undetermined and compatible, so omission drops acknowledged details before unacknowledged ones. When they do not all fit, the leading ones are kept, `differences_omitted` counts the rest, and a `result.differences` omission record appears in `result.omissions` and `diagnostics.omissions`. A location whose pointer cannot be presented has a null pointer with its own omission record, and `diagnostics.omissions` counts such locations once per field and reason. `diagnostics.truncated` is true whenever anything was omitted. Omission never removes a source, a count, the warning count, the unused positions, the outcome or the disposition, and a report allowance too small for both revisions' mandatory envelopes and the unused-position reserve fails the request before any comparison work. `baseline.interpretations` names `content-format-model-meaning/1` when content-format comparison consulted it, the value interpretations that the checks of a Breaking witness relied on, `copied-reference-relocation/1` when a composed revision relied on it, and `undeclared-extensions-ignorable/1` under the rule for extension vocabulary above. The value interpretations are those a model report names for Sample-value checking: `ADR0018-selected-representation` for a civil-time representation, `byte-string-json/1` and `unix-time-posix-coordinate/1` for the `sdfType` conventions, and `uuid-bare-json/1` and `uuid-opaque-value/1` for UUIDs. Candidates that establish no witness name none.

The human report states the client-interaction scope and that Context metadata compatibility is not assessed, the outcome and contract identity and, when acknowledgements were supplied, a `Disposition:` line with the acknowledged breaking and undetermined counts. It counts compared, removed and added documents, lists removed and added documents, counts each classification, and names consulted or withheld registry evidence and the content-format meaning interpretation as `check` does. A `Warnings:` list follows with each retained Version warning's code, later document, both versions as JSON strings and breaking count, and the number of omitted warning details. It then lists unacknowledged breaking differences, then unacknowledged undetermined, then acknowledged differences, whose rows name their classification, then compatible, each with its witness and admissions, both effective locations and, where it differs from the location, the nearest content origin. An `Unused acknowledgements:` list follows, naming each unused entry's position in the file, kind, subject, classification and locations. It uses the shared color controls and palette, and it reports omitted difference details. It renders the differences the JSON report kept, within the same report allowance. When they are too long as text, the JSON report is given in its place, as for model reports.
