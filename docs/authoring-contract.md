# Composed sdfThing authoring contract

This contract specializes [ADR 0013: Shared workflow and report guarantees](adr/0013-shared-workflow-and-report-guarantees.md) for Model authoring under [ADR 0017: Reference-preserving sdfThing authoring](adr/0017-reference-preserving-composed-thing-authoring.md). Domain terms come from [the domain language](../GLOSSARY.md). SDF composition follows [RFC 9880 Sections 4.4 and 6](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.4); namespace identity follows [Sections 3.2 and 4.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.2). Patch deletion follows [RFC 7396 Section 2](https://www.rfc-editor.org/rfc/rfc7396.html#section-2). Resource, output and API limits below are toolkit capabilities.

## Request and library ownership

A private-field authoring builder accepts an owned Catalogue, one generated-source key and optional bounded label, a new sdfThing Given Name, an explicit namespace map with an optional chosen default prefix, and a nonempty list of sdfObject declarations. Each declaration has an exact new Given Name, a supplied source ID plus decoded pointer selecting an `sdfObject`, an explicit output namespace prefix for that sdfObject's established global namespace, an owned JSON map Composition patch, and an explicit declaration-requiredness flag. The sdfThing can also have caller-supplied common qualities and information-block metadata. New generation creates one document; existing-document editing, nested sdfThing recipes and flattened export are later capabilities.

The sdfObject selector never selects by display label, filename or input order. Unknown IDs, malformed/overlong pointers, a present non-sdfObject selector, repeated declaration names, duplicate namespace prefixes with different bindings, duplicate generated keys and conflicting builder settings are request errors. A selector whose source/definition is unavailable produces scoped domain incompleteness; an established missing pointer is a request error. Prefixes and Given Names compare exactly after JSON decoding, contain no colon and receive no case folding or Unicode normalization. The toolkit bounds each newly supplied Given Name or prefix to 1,024 UTF-8 bytes, requires a nonempty prefix for its authored CURIE capability, permits at most 256 sdfObject declarations and 256 namespace bindings, and bounds each selector to the shared 8 KiB semantic-pointer cap before copying. These are new-request capabilities rather than RFC restrictions on source names; larger source names remain accessible.

The `required` flag authors an entry in the new sdfThing's `sdfRequired`. Under [RFC 9880 Section 4.5](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.5), an abbreviated Referenceable name excludes `#` and `:`; Given Names can contain `#`. When the declaration's Given Name is a valid abbreviated name, emit that exact decoded name without JSON Pointer escaping. Otherwise, preserve the Given Name and emit a correctly encoded same-document SDF pointer reference to the newly authored declaration. Form decoded RFC 6901 tokens first, escaping token `~` and `/` as `~0` and `~1`, then UTF-8/percent-encode the URI-fragment representation. An internal `#` becomes `%23` in that fragment, not a second fragment delimiter. Decoded source/effective pointers retain the literal `#`. Inherited internal `sdfRequired` content is unchanged unless explicitly refined; this generation rule does not rewrite copied absolute pointers, which relocate during composition under [ADR 0036: Copied reference relocation](adr/0036-copied-reference-relocation.md).

The explicit output prefix must already map to the selected sdfObject's established global namespace URI. Authoring constructs the CURIE from that prefix and the selected definition's namespace-rooted pointer; it does not copy a source prefix spelling. The target must be unambiguous in the supplied/generated catalogue. A requested generated namespace/global name that collides with an existing contributor is refused with `author.identity_collision`; no coalescing or precedence applies. Multiple bindings to the same URI are allowed; one prefix cannot denote two URIs. A missing source default namespace leaves reuse by this operation unsupported with `author.local_only_object`, rather than assigning a new source namespace or diagnosing an RFC violation. Namespace URI spelling and declared source identities remain unchanged; no network retrieval occurs.

The patch is SDF-authored JSON, not a generic mutable graph. It excludes an authored top-level `sdfRef`, since the builder owns that reference. sdfThing common-quality input excludes structural declaration groups and `sdfRef`; the builder owns declaration names and `sdfRequired`. Bad JSON and duplicate decoded names use the shared input/acceptance policies. Patch null retains its Merge Patch deletion meaning; it is not coerced into an explicit model value. Nested introduced references remain authored in the generated document's explicit namespace context, while copied source references retain any interpretation limitation. Ordinary supported composition does not wait for interpretations that are irrelevant to its selected definitions.

The operation owns accepted request metadata, patch content and raw source bytes. Views borrow its owned result. Callers can retrieve a generated document's bounded JSON bytes, the generated Catalogue, typed source/effective views and explanations using the same check/inspect operations as supplied models. Every generated source has a new compact SourceId and unique caller key; it does not reuse a supplied identity. Supplied raw bytes, names, number tokens and metadata remain unchanged. Authored declaration identities belong to the new sdfThing, while available origins point to selected reusable sources and request patches. This does not promise complete provenance history.

## Fidelity, checking and failures

Generation state is `complete`, `partial` or `unavailable`. `complete` establishes a complete readable JSON document whose requested assembly is finished; `partial` means some requested assembly is unfinished even when a structurally complete candidate JSON exists. A candidate's `content_complete` only describes its bytes. A fidelity state `established` or `undetermined` is independent: `established` requires all requested known-semantic/name/number and permitted opaque-structure preservation established, with no unresolved context meaning. A proven loss is an error and cannot yield faithful success. Preserving unknown extension JSON alone cannot establish preservation of its meaning after containment or reference context changes. Independent known observations and retained opaque content remain inspectable with affected locations and reasons.

Generated known JSON values preserve meaning; unchanged accepted numbers preserve their raw lexemes, including lexical exponent-boundary tokens. New patch numbers preserve their accepted request tokens. Mathematical number fidelity does not establish formal normative CDDL number admission, which remains a separately assessed requirement under [ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md). Names preserve decoded scalar values, without promising original escape spelling. Output is readable UTF-8 JSON with indentation and a trailing newline. Source whitespace, member order, original escape spelling and byte-for-byte regenerated documents are outside the guarantee. The operation introduces no computed numeric normalization that would cross the accepted output limits.

Checking is explicitly `not_requested` or `requested` with `initial-v1` or `full-baseline` and the shared strict-warning policy. A requested check applies to selected generated-document roots and needed dependencies; every supplied input remains visible. It uses the same immutable inventory and baseline as ordinary checking. Its completion/findings remain separate from generation/fidelity. Checking can fail while generated content is available, or be incomplete while fidelity is established. Requested broader omissions cannot become a pass. No sample checker or every specialized validator is a prerequisite for established authoring. No general proof that a patch is a Restrictive refinement is promised.

Overall author disposition is failed for established errors or selected strict warnings, otherwise undetermined if generation, fidelity or requested checking is unfinished, otherwise passed. `complete` means that this one requested authoring operation, including fidelity and requested checking, finished; independent errors can coexist with complete work. Top-level author `incomplete_checks` counts this one unfinished authoring unit at most once. The separately represented checking outcome counts its own assessment units, never added to author coverage counters. Detail omission alone changes neither completion nor fidelity.

Sources unavailable, global contributor ambiguity, effective cycles, Boolean or short-name `sdfRef` without processing meaning, unknown critical features and semantic work/output limits are conditions with their existing basis or the author's operational reason. `author.local_only_object` is an operational unsupported capability; `author.identity_collision` is an authoring assembly conflict, not an invented RFC rule; `author.fidelity_unknown` names affected unknown context; `author.generation_limit` is operational unfinished work. Unknown request selectors and invalid output configurations are request errors before generation. Sources and useful candidate views survive failures.

## Limits and locations

All model, sample and generated attempts share the 256-attempt ceiling. Reserving one generated identity reduces available supplied attempts to 255. All caller metadata, including request keys, new names, prefixes, URI strings and repeated source selectors, consumes the shared 1 MiB pre-copy budget and worst-escaping mandatory-report reserve. Source key/label/URI/version/digest bounds remain unchanged. New semantic identities are never clipped. The accepted plan is owned only after preflight.

The CLI author plan is one bounded JSON request text, at most 4 MiB, with the shared decoding, duplicate-name, depth and numeric policies. Its parsed nodes and storage share the request's 500,000-node/64 MiB parsed allocation budgets. It has no SDF source role or namespace identity. Builder requests have the same new-name, patch-depth/node/byte and metadata charges. Raw plan bytes are additional bounded request storage, not an unbounded metadata escape.

Raw generated bytes share the 4 MiB/document and 32 MiB catalogue ceilings with supplied content. Before appending emitted bytes, the writer checks emitted UTF-8 bytes, including escaping, indentation and newline, against remaining output/input and derived-allocation budgets. Generated representation shares 250,000 effective nodes/64 MiB derived allocation, ancestry and 10,000,000 work units with composition and requested checking. Already charged immutable content can be shared, but traversal still charges work. No phase resets the 2,000,000 numeric units or report/location limits. Request configurations cannot disable finite caps. Exhaustion yields explicit operational incompleteness; a partially built buffer is never a completed generated SDF document or ordinary CLI output.

Source and effective locations retain the shared meanings. Author-request locations add a distinct `kind:"author_request"`, request key, decoded pointer and optional original-plan byte span. A typed builder has a pointer into its logical request shape and a null span because no textual offsets exist. A CLI span belongs to the retained plan text, never a generated source. A generated-source span belongs to its emitted candidate bytes. For a refined maximum, the effective location names the generated root, with related locations in the original sdfObject and `/objects/0/refinement/sdfProperty/temperature/maximum` of the author request. A deletion can identify an absent effective quality. Pointer/detail truncation keeps compact identity and explicit omission reasons; it does not invent a source span.

## Author report JSON 1.1

The author operation extends the shared supported wire contract in minor version 1.1 with a new tag. Check/inspect readers continue rejecting this unknown operation before reading its payload; existing 1.0 tags, units, fields and null meanings do not change. The following additions apply only to `operation:"author"`. Count, Bytes, SourceId, Outcome, Scope, Coverage, Basis and diagnostic omission types retain the shared bounds and meanings. There is no mandatory embedded generated SDF payload in the report.

| JSON object | Required fields | Absence and null rules |
| --- | --- | --- |
| Top level | schema:`"esdieff.report"`; version:`"1.1"`; operation:`"author"`; request:Author Request; sources:Source[]; baseline:Baseline; policy:Policy; outcome:Outcome; coverage:Coverage; diagnostics:Diagnostics; result:AuthorResult | All fields are required. Source/Baseline/Policy/Outcome/diagnostic definitions retain shared 1.0 meanings. Only the author-specific request, coverage unit, result and request-location additions below apply. |
| Author Request | assessment:null; inventory:null; scope:Scope; authoring:AuthorPlanSummary | Null assessment/inventory means the author operation has no model-assessment identity. Optional checking has its separate explicit identity below. scope is selected with the reserved generated root, dependencies/assessed/unavailable_required report actual established work. |
| AuthorPlanSummary | key:string (<=256); thing_name:string (<=1024); generated_source:Id; declaration_count:Count; checking:`"not_requested"\|"requested"` | Optional label uses the shared presentation-label omission rule. Complete recipe/content is accessed through owned request views rather than repeated in every report. |
| Author RequestedCoverage | inventory:null; count_unit:`"authoring"`; total:1; assessed:Count; not_applicable:0; unassessed:Count; undetermined:Count | The three counters sum to 1. Assessed means the author work finished, including any requested checking; unassessed/undetermined explicitly identify unfinished work. BroaderCoverage still discloses RFC coverage independently. |
| AuthorResult | kind:`"author"`; generation:`"complete"\|"partial"\|"unavailable"`; fidelity:Fidelity; documents:GeneratedDocument[]; checking:AuthorChecking; omissions:Omission[] | No fields are omitted. documents includes the reserved generated identity even if content is unavailable. One document is requested in this contract. |
| Fidelity | state:`"established"\|"undetermined"`; known_semantics:`"established"\|"undetermined"`; opaque_content:`"none"\|"retained"\|"unavailable"`; limitation_count:Count | Bounded location-bearing conditions carry detail. An empty retained condition list does not override limitation_count or aggregate state. |
| GeneratedDocument | source_id:Id; state:`"available"\|"partial"\|"unavailable"`; content_complete:boolean; retained_bytes:Bytes; faithful:boolean | Source role is generated, acquisition generated. Unavailable has false content_complete, zero retained_bytes and false faithful. faithful requires generation complete and fidelity established; it does not assert model-check success. Report carries identities/statuses only. |
| AuthorChecking | state:`"not_requested"` OR state:`"requested"`; assessment:string; inventory:string\|null; scope:Scope; outcome:Outcome; coverage:Coverage | The five additional fields are required only for requested; they are absent for not_requested, never null success defaults. Requested assessment is initial/full, inventory null explicitly means unavailable requested inventory. Its counts use the shared assessment_area/requirement conventions. Diagnostics stay in the top-level collection with named assessment units and are not double-counted. |
| AuthorRequestLocation | kind:`"author_request"`; key:string (<=256); pointer:string\|null (<=8192); span:[Bytes,Bytes]\|null | Optional omission follows shared rules. Null pointer/span means unavailable; empty pointer is the plan root. The key identifies owned request metadata, not a SourceId or SDF namespace. |

Rust result enums use stable accessors/private fields with future-compatible variants. Existing check/inspect readers are not required to understand author-request locations. A known operation's meanings cannot silently change within report 1.x; breaking fields/tags/units require a major version. The author DTO is independent of parser, graph and Rust serialization layout.

## CLI and destinations

The command is `esdieff author --plan PLAN --document-output DEST [--report-output DEST] [--check initial-v1|full-baseline] [--strict-warnings] [--format human|json] [--replace-document] [--replace-report] --source KEY PATH...`. Repeat the two-argument `--source KEY PATH` option for each explicit supplied model path; keys use the shared bounded unique caller-key contract. Paths supply candidates, not declaration roles. PLAN is an explicit path or `-`, distinct from SDF inputs. At most one input/plan uses stdin. `--document-output` is required; report defaults to stdout. Each DEST is an explicit file or `-`. Document and report cannot both use stdout or name the same filesystem target. No directory discovery, parent-directory creation, source mutation or implicit writes occur.

The JSON plan contains `key`, `generated_key`, optional `generated_label`, `thing_name`, `namespace` map, optional `default_namespace`, optional `info` and `thing_qualities` maps, and `objects` array. Each object contains `name`, `source` (the exact supplied KEY, solely for source selection), `pointer`, `prefix`, `refinement` map and `required` boolean. CLI acquisition associates each key with its explicit raw filesystem path, then translates selectors to the same library request. This supports non-UTF-8 paths without placing them in JSON or deriving identity from labels. Plan source selectors must exactly select one supplied key; file aliases under distinct keys remain distinct contributors and never silently coalesce. Duplicate plan members are refused. New names/namespaces are deliberate request data, not inferred filenames.

Before generation, the adapter validates destinations and source aliases. The plan path is a protected input alongside every supplied model path. An existing output requires its corresponding replacement flag and a regular, non-symlink file; the flag does not authorize an input alias. A replacement option for stdout is a configuration error. Existing destination identities are compared to input identities and each other, including hard links and resolved parent-directory aliases; source overlap is refused. Absent paths use resolved parent identity plus exact basename to detect aliases. File commits recheck identity/collision policy and use create-without-replacement or explicit replacement of the validated destination. Unexpected changes cause a host failure rather than overwriting a newly appeared path. No portability claim of preventing every adversarial filesystem race is made; stronger platform primitives are separate implementation choices.

Only generation complete, fidelity established and a passed requested check permit ordinary document delivery. When checking is not requested, established generation/fidelity permits delivery. Failed or undetermined authoring emits its report and exits 1 or 3 with no generated document write. Library candidates remain available for explicit inspection; this first CLI has no flag that presents an uncertain candidate as ordinary output. Domain failures do not erase source attempts. Invocation/configuration errors exit 2; host acquisition/output failure follows the shared distinctions, including reportable input acquisition conditions versus exit 4 when an output cannot be emitted. Strict warnings can prevent document delivery without changing semantic fidelity.

For files, output is staged under a bounded temporary file in the existing destination directory and committed only after successful generation and policy checks; abandoned staging is cleaned up. Each file commit is atomic where the platform supports it, without a two-file transaction or stdout rollback guarantee. A permitted generated document is delivered before emitting the passed CLI report. The report's domain outcome does not itself assert host delivery; successful process status establishes delivery of both requested outputs. If the report later fails, exit 4 and stderr identify any document already delivered. A stdout document may be partially emitted on host failure and cannot be withdrawn; exit 4/stderr make the failure explicit. Reports go to their separate destination with a trailing newline and equivalent human/JSON conclusions. No report prose is mixed into document stdout.

## Ordinary acceptance fixture

The reusable source contributes `Sensor / indoor` in `https://example.com/esdieff/capabilities`, with inline temperature Property data: type number, nullable false, minimum -50, maximum 150, multipleOf `0.10`, a `maxLength` of `18446744073709551616` that FullBaseline reports outside the CDDL `uint` domain, unit Cel, writable false; an inline Action input/output and Event output contain no references. It also declares `sdfRequired:["temperature"]` and a source description. An additional inline reusable `Switch` sdfObject provides a Boolean `enabled` Property in the same document. The reusable source remains byte-identical.

The request chooses sdfThing `Room / Monitor`, declarations `ambient / sensor` and `switch`, explicit `reuse` binding to the source namespace and `product` default namespace `https://example.com/esdieff/products`. The temperature refinement deletes the sdfObject description with null and overrides maximum with the accepted token `1e2`; the source maximum remains 150 and Event-output maximum remains 150. `ambient / sensor` is required, `switch` optional. Generated `sdfRef` uses `reuse:#/sdfObject/Sensor%20~1%20indoor`, the URI-fragment spelling of the pointer whose decoded token is `Sensor / indoor`. The effective maximum is mathematically 100 with raw patch lexeme `1e2`; minimum -50, step `0.10`, nullable false and writable false are inherited. The generated effective path is `/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/sdfProperty/temperature/maximum`.

A second supplied acceptance variant names the required declaration `ambient#sensor` and selects the reusable sdfObject `Sensor# / indoor`. The decoded source selector is `/sdfObject/Sensor# ~1 indoor`; its generated CURIE is `reuse:#/sdfObject/Sensor%23%20~1%20indoor`. The new sdfThing's `sdfRequired` entry is `#/sdfThing/Room%20~1%20Monitor/sdfObject/ambient%23sensor`, not the invalid abbreviation `ambient#sensor`. The decoded effective declaration pointer is `/sdfThing/Room ~1 Monitor/sdfObject/ambient#sensor`. Inherited `sdfRequired:["temperature"]` remains exact. The slash/space control `ambient / sensor` remains a valid exact abbreviated name, without `~1` or percent encoding in that abbreviated entry. These are supplied, unexecuted semantic acceptance expectations, not evidence of a current emitter defect. Copied absolute pointers relocate under [ADR 0036: Copied reference relocation](adr/0036-copied-reference-relocation.md) without being rewritten.

Acceptance checks the generated catalogue with explicit generated roots and inspects the effective Property and required declarations. Comparison uses these independently worked semantic observations and exact values, not whitespace/key order. A permitted optional opaque quality remains structurally retained but leaves affected fidelity undetermined when its new containment meaning is unknown. A copied Boolean or short-name `sdfRef`, which has no processing meaning, likewise leaves only affected composition/fidelity incomplete. A generated global-name collision, a duplicate declaration name, a 256-supplied-plus-generated attempt request, a too-small generated-byte budget and conflicting CLI destinations have explicit failure expectations. These are contract acceptance obligations; a supplied expected fixture is not evidence that product composition has executed.

## Using the authoring operation

The library accepts explicit source selection and retains the complete result for inspection:

```rust
use esdieff::{Assessment, AuthorObject, AuthorRequest, AuthorResult, Catalogue, SourceId};

fn product(catalogue: Catalogue, reusable: SourceId) -> Result<AuthorResult, &'static str> {
    AuthorRequest::new(catalogue, "room-recipe", "room-product", "Room")?
        .namespace("reuse", "https://example.com/capabilities")?
        .namespace("product", "https://example.com/products")?
        .default_namespace("product")?
        .object(AuthorObject::new("sensor", reusable, "/sdfObject/Sensor", "reuse", true)?)?
        .checking(Assessment::Initial)?
        .author()
}
```

`AuthorResult::outcome()` describes the entire requested operation. `generation()` and `fidelity()` retain the independent assembly and Authoring fidelity conclusions. `generated_document()` returns available candidate bytes even when requested checking fails or remains unfinished. `catalogue()` supplies immutable source and effective inspection through the ordinary operations. Only `document_deliverable()` permits the CLI's ordinary generated-document output.

For the CLI, save an explicit recipe such as:

```json
{
  "key": "room-recipe",
  "generated_key": "room-product",
  "thing_name": "Room",
  "namespace": {
    "reuse": "https://example.com/capabilities",
    "product": "https://example.com/products"
  },
  "default_namespace": "product",
  "objects": [
    {
      "name": "sensor",
      "source": "reusable",
      "pointer": "/sdfObject/Sensor",
      "prefix": "reuse",
      "refinement": {},
      "required": true
    }
  ]
}
```

Run `esdieff author --plan recipe.json --source reusable capabilities.sdf.json --document-output room.sdf.json --check initial-v1 --format json`. The report goes to stdout. Use `--report-output report.json` for a separate report file, or use `--document-output - --report-output report.json` to send only the generated document to stdout. `--replace-document` and `--replace-report` separately authorize replacing an existing regular output, while protected input aliases remain refused.

Destination identity uses retained filesystem handles, including parent handles for absent paths, with checks before individual commits. Identity or hard-link support unavailable on a host produces an explicit failure. Conservatively reported filesystem identity collisions can refuse output, including wider filesystem identifier limits on Windows. This operation does not claim universal race prevention or a transaction across document and report delivery.

## Refinements and explanations

Refinements are owned JSON maps. `JsonBuilder` retains exact accepted number lexemes; a null in an actual Composition patch deletes an inherited quality:

```rust
use esdieff::{Assessment, AuthorObject, AuthorRequest, AuthorResult, Catalogue, JsonBuilder, SourceId};

fn refined_product(catalogue: Catalogue, reusable: SourceId) -> Result<AuthorResult, &'static str> {
    let json = JsonBuilder::new();
    let patch = json.map(vec![
        ("description", json.null()?),
        ("sdfProperty", json.map(vec![
            ("temperature", json.map(vec![
                ("description", json.text("Product temperature")?),
                ("maximum", json.number("1e2")?),
            ])?),
        ])?),
    ])?;
    AuthorRequest::new(catalogue, "room-recipe", "room-product", "Room / Monitor")?
        .namespace("reuse", "https://example.com/capabilities")?
        .object(AuthorObject::new(
            "ambient / sensor", reusable, "/sdfObject/Sensor ~1 indoor", "reuse", true,
        )?.refinement(patch)?)?
        .checking(Assessment::Initial)?
        .author()
}
```

The reusable source keeps its original description, maximum and number tokens. The generated effective Property inherits its minimum, step and capability flags while overriding the maximum with the accepted `1e2` token. The Object description is absent, with its deletion explanation retained. Unrefined Action and Event descriptions keep their inherited values.

Use the ordinary `EffectiveModel::explanation` interface, then map a generated-source contributor back to its recipe:

```rust
use esdieff::{AuthorResult, EffectiveRequest};

fn inspect_refinement(result: &AuthorResult) -> Result<(), &'static str> {
    let effective = result.catalogue().effective(EffectiveRequest::new(result.generated_source()))?;
    let maximum = effective.explanation(
        "/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/sdfProperty/temperature/maximum",
    ).require()?;
    let generated_origin = maximum.sources().next().ok_or("maximum origin unavailable")?;
    let recipe = result.request_origin(generated_origin).require()?;
    assert_eq!(recipe.pointer().require()?, "/objects/0/refinement/sdfProperty/temperature/maximum");
    Ok(())
}
```

`request_origin` accepts a view belonging to this result's Catalogue. A reusable-source contributor yields `Absent`; a foreign Catalogue view yields `Unavailable`, even if its compact Source ID and path coincide. Textual plans expose exact original-plan byte spans. Typed builders expose the logical recipe pointer with no fabricated textual span. Effective locations and generated-source byte spans keep their existing meanings. Author report 1.1 adds related recipe locations to located generated-source findings; ordinary check and inspection report tags retain their established contracts.

`AuthorResult::checking()` exposes the requested assessment, outcome, source roots and coverage through typed accessors. Its requested assessment-area and requirement counts remain separate from the single authoring unit in `AuthorResult::requested_coverage()`. `full-baseline` can remain unfinished while the candidate and independent inspection are available. Strict warnings can prevent document delivery while Authoring fidelity stays established. Limitations from Boolean or short-name `sdfRef` and from opaque contexts identify affected effective and source coordinates without hiding independent refinements or established errors.

For a delivered CLI result, inspect it with its explicit reusable inputs, for example `esdieff inspect --view effective --document room.sdf.json --pointer '/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/sdfProperty/temperature' capabilities.sdf.json room.sdf.json`. The ordinary inspection report exposes inherited, overridden and deleted observations and their nearest source origins. Use `--format json` for the shared structured inspection contract.
