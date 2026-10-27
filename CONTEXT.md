# Esdieff domain language

The vocabulary of SDF connected-device models, grounded in [RFC 9880](https://www.rfc-editor.org/rfc/rfc9880.html). Terms describe the modeling domain rather than software structure.

## Documents and models

Terminology source: [RFC 9880, Section 1.2.3](https://www.rfc-editor.org/rfc/rfc9880.html#section-1.2.3).

**SDF document**:
A JSON text representing a single map containing SDF definitions and optional document information.
_Avoid_: Model when referring to one document.

**SDF model**:
Definitions and declarations describing the network interactions of kinds of Things. A model can span multiple SDF documents through references.
_Avoid_: File, document when referring to the model as a whole.

**SDF specification**:
An SDF description when the distinction between its model and its documents is irrelevant.

**Definition**:
A named semantic term with associated qualities. A definition does not necessarily introduce a component into its enclosing definition.

**Declaration**:
A definition that introduces a component into its enclosing definition. It remains available for reuse as a definition.

**Quality**:
A metadata entry describing a definition or declaration.
_Avoid_: Property when referring to metadata.

**Block**:
A set of SDF map entries described together because they serve one function, such as document information or namespace mappings.

**Group**:
An entry whose Class Name Keyword selects the class of its named definitions.

**Definition group**:
A collection of named definitions identified by a class name keyword.

**Class**:
The category of model information identified by a Class Name Keyword.

**Class name keyword**:
One of `sdfThing`, `sdfObject`, `sdfProperty`, `sdfAction`, `sdfEvent`, and `sdfData`.

**Information block**:
Document metadata under `info`, describing the document and its included definitions.

**Namespaces block**:
The namespace mappings and optional default namespace of an SDF document.

**Definitions block**:
The definition groups contained in an SDF document.

## Document and definition metadata

Terminology sources: [RFC 9880, Section 3.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.1) and [Section 4.6](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.6).

**title**:
A short summary of a document and its definitions for display or discovery.

**version**:
A string identifying the evolution of a document's definitions, with no mandatory version-string format.
_Avoid_: SDF format version.

**modified**:
The latest revision time of the definitions, expressed as a full date or a UTC date-time.

**copyright**:
Copyright information supplied as text or a link to text.

**license**:
License terms supplied as text or a link, preferably an unambiguous license URI or SPDX identifier.

**description**:
A long-form textual description of a definition or document.

**label**:
A short textual description of a definition. An application may display its final name token when no label is supplied, without adding a model quality.

**$comment**:
Source commentary whose presence does not change model meaning.

## Things and interactions

Terminology sources: [RFC 9880, Section 1.2.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-1.2.2), [Section 1.2.3](https://www.rfc-editor.org/rfc/rfc9880.html#section-1.2.3), and [Section 2.2.5](https://www.rfc-editor.org/rfc/rfc9880.html#section-2.2.5).

**Thing**:
A physical item with network interaction opportunities.
_Avoid_: sdfThing when referring to the physical item.

**Affordance**:
A network interaction opportunity described by the model: a Property, Action, or Event.

**Property**:
An affordance for reading, writing, or observing state.
_Avoid_: JSON entry, quality.

**Action**:
An affordance for invoking a named operation.

**Event**:
An affordance for obtaining information about a happening.

**Grouping**:
An `sdfObject` or `sdfThing` describing a collection of affordances, directly or through nested groupings.

**sdfObject**:
A reusable grouping that can declare affordances but cannot contain other groupings.
_Avoid_: JSON object.

**sdfThing**:
A grouping that can also contain other groupings.
_Avoid_: Thing as shorthand for this model element.

**Grouping array**:
An `sdfObject` or `sdfThing` describing repeated groupings because `minItems`, `maxItems`, or both are present. Those qualities bound the number of repetitions inclusively.
_Avoid_: Array data, which describes interaction values.

**sdfData definition**:
A reusable description of data constraints and semantics that does not itself declare an affordance.

## Names

Terminology source: [RFC 9880, Section 2.3.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-2.3.1).

**Given Name**:
A case-sensitive name chosen by the model author for a definition or another named entity. Embedded colons are reserved for a future extension and prohibited in base SDF.

**Quality Name**:
A vocabulary name whose associated entry has semantics specified by SDF or an extension.

Terminology sources for namespace identity and references: [RFC 9880, Section 3.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.2) and [Section 4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4).

**SDF namespace**:
A space of global names identified by a namespace URI. Multiple documents can contribute names to the same namespace.
_Avoid_: Document location, download URL.

**Namespace prefix**:
A document-local short name mapped to a namespace URI.
_Avoid_: Quality Name Prefix, which has a separate extension-vocabulary role.

**Default namespace**:
The namespace selected by `defaultNamespace` to receive a document's contributed global names; also called its target namespace. A document without this setting contributes no global names.

**Global name**:
A definition's namespace URI followed by its JSON Pointer fragment identifier.
_Avoid_: File path.

**Same-document reference**:
A name reference consisting of `#` and a JSON Pointer fragment that selects an element in the current document.

**CURIE reference**:
A name reference abbreviated using a namespace prefix from the document's namespace mappings.

**Referenceable name**:
An unprefixed affordance or grouping name used by `sdfRequired` to select directly contained declarations by name. It can select declarations of different classes carrying the same Given Name.

**Quality Name Prefix**:
A registered prefix used to construct qualified Quality Names for extension vocabulary.

**Qualified Quality Name**:
A Quality Name Prefix followed by a colon and an unqualified Quality Name.
_Avoid_: CURIE reference, which selects a model definition using namespace mappings.

## Composition

Terminology sources: [RFC 9880, Section 4.4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.4), [Section 4.4.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.4.1), and [Section 6](https://www.rfc-editor.org/rfc/rfc9880.html#section-6).

**Composition**:
The combination and refinement of definitions through references and containment to describe reusable capabilities and products.
_Avoid_: File concatenation, arbitrary JSON merging.

**sdfRef**:
A quality that reuses a referenced definition, with additions and overrides from the referencing definition applied through JSON Merge Patch.

**Composition patch**:
The referencing definition's map with its `sdfRef` entry removed, applied to a copy of the referenced definition through JSON Merge Patch.

**Copied definition**:
The referenced definition as copied by `sdfRef`, before the composition patch applies.

**Copied reference**:
A name reference that reaches an effective definition from a copied definition rather than from the composition patch.
_Avoid_: Inherited reference.

**Internal copied reference**:
A copied reference whose target is the copied definition or lies within it.

**Reference relocation**:
The replacement of an internal copied reference's target by the corresponding position in the importing definition.
_Avoid_: Rebinding, rebasing.

**Effective definition**:
A definition after the applicable reference composition and overrides have been processed.
_Avoid_: Source patch, valid definition.

**Content origin**:
The nearest source contribution establishing an effective quality, or removing it through a composition patch.

**Importing consumer**:
The source `sdfRef` that establishes the composition boundary containing an effective quality. It is distinct from the quality's content origin.

**Default origin**:
An established implicit semantic value when a quality is absent, without an authored source contribution or inserted JSON member.

**Restrictive refinement**:
The convention that an override narrows the possibilities described by a reused definition. RFC 9880 does not provide a general procedure for checking that convention.

**Resolved model**:
An SDF model in which every `sdfRef` has been processed according to its composition semantics.
_Avoid_: Valid model, since resolution alone does not establish validity.

**sdfRequired**:
A quality identifying declarations whose corresponding data must be represented in the modeled grouping.
_Avoid_: `required`, which constrains entries within map data.

**Required declaration relationship**:
An `sdfRequired` occurrence's relationship to the affordance or grouping declarations it selects, including all directly contained classes sharing a Given Name. An array entry `true` identifies its carrying affordance or grouping.

**Declaration contribution**:
The authored definition contribution from which a surviving declaration originates, independently of later content refinements or reused data descriptions. Reinsertion after deletion introduces a new contribution.

**Relationship completion**:
Establishment of the selected declaration identities and their relevant context. It does not establish the declarations' effective capabilities or the validity of every relationship.

## Data descriptions

Terminology sources: [RFC 9880, Section 4.7](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.7), [Section 5](https://www.rfc-editor.org/rfc/rfc9880.html#section-5), and [Appendix C](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C).

**Data description**:
Qualities describing the meaning and permitted values of interaction data at the information model level.
_Avoid_: Wire format, protocol payload schema.

**Data quality**:
A quality that describes data semantics or constrains data values.

**Full data description**:
A Data description using the full data-quality vocabulary, including Property and sdfData definitions, Action input/output, Event output, properties entries and Choice alternatives. Nested descriptions retain their own vocabulary role, as described in [RFC 9880, Appendix A](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-A) and [ADR 0020: Owned effective views and nearest origins](docs/adr/0020-owned-effective-views-and-nearest-origins.md).

**Direct items description**:
The restricted description directly under `items`. Its established base vocabulary includes scalar bounds and lengths, `format`, `enum` and `sdfChoice`; explicit map data also permits `required` and `properties`. Those properties entries and Choice alternatives are Full data descriptions. Wider admitted content remains inspectable without gaining full data-quality meaning from its spelling.
_Avoid_: Full data description, blanket prohibition of nested array data.

**Data type**:
The kind of data selected by `type`: number, integer, string, boolean, array, or map (`object`).

**Map data**:
Data selected by `type: "object"`, with optional value constraints for named entries in `properties` and mandatory presence in `required`. An undescribed entry has no implicit value constraint, following [ADR 0016: Selected samples with explicit admission and coverage](docs/adr/0016-selected-sample-contract.md).
_Avoid_: sdfObject, Property affordances.

**required**:
A data quality naming mandatory entries in map data.
_Avoid_: `sdfRequired`, which concerns declarations in groupings.

**properties**:
A map from data-entry names to descriptions of their values. A described entry is optional unless named by `required`; this quality does not close the map to other entries.
_Avoid_: Property affordances.

**sdfType**:
A quality naming additional information-level data semantics, such as a byte sequence or a point in civil time.

**Civil date-time**:
An absolute point in civil time identified by `format: "date-time"`, with conventional RFC 3339 text and its supplied offset annotation.

**Civil date**:
A Gregorian calendar date identified by `format: "date"`, without a time of day or timezone.

**Civil time**:
A date-free clock coordinate with an offset annotation, identified by `format: "time"`. It does not identify a unique dated instant.

**Civil denotation**:
The established calendar or clock meaning of a selected conventional civil representation under [ADR 0018: Civil date and time sample conventions](docs/adr/0018-civil-date-time-sample-conventions.md). Recognized format names or identical invalid text do not establish it. An eligible date-free second 60 can have unavailable denotation without lacking a required field.

**sdfChoice**:
A choice between named data alternatives, with shared data qualities except where an alternative overrides them.

**Choice alternative**:
A named set of data qualities within `sdfChoice`. Its Given Name identifies the alternative without automatically constraining a data value to that name.

**Effective alternative**:
A Full data description established after reference composition, shared-quality inheritance and whole-quality overrides. A named Choice alternative retains its Given Name; an Enum alternative retains its occurrence index and original text value as `const`. Its semantic coordinates can differ from authored source locations. Inherited qualities retain their donor's established meaning and uncertainty, following [ADR 0024: Effective alternatives and nullable characteristics](docs/adr/0024-effective-alternatives-and-nullable-characteristics.md).

**enum**:
A nonempty list of string values abbreviating `sdfChoice` alternatives with corresponding `const` values. A definition uses either `enum` or `sdfChoice`.

**const**:
A data quality restricting data to its stated value.

**default**:
Documentary data that may be used when interaction data is absent. Base SDF defines no process that inserts this value.

**Declared-value consistency**:
Advice about whether a stated const or documentary default is excluded by its final effective data description. It reuses selected-value checking without inserting defaults, changing source roles or proving general satisfiability, following [ADR 0031: Bounded declared-value consistency](docs/adr/0031-bounded-declared-value-consistency.md).

**Consistency candidate**:
One surviving effective const or default checked in its owning description. Final alternative constants and distinct importing consumers remain distinct candidates. An owner default requests one complete existential alternative witness; introduced or overridden branch defaults are separate candidates. One definite exclusion produces one advisory warning regardless of rejected leaf count.

**contentFormat**:
A data quality naming the content format of a data value, using a string-form CoAP Content-Format identifier or a media type with parameters and content codings, as defined by [RFC 9193, Section 3](https://www.rfc-editor.org/rfc/rfc9193.html#section-3).
_Avoid_: Protocol payload binding.

**Content-format declaration**:
The complete authored `contentFormat` text, with decimal identifier spelling or a media name, optional parameters and ordered content-coding tokens. Its spelling follows [RFC 9193 Section 6](https://www.rfc-editor.org/rfc/rfc9193.html#section-6); decimal identifier range follows Section 3 separately. Spelling and registration do not establish parameter meaning, payload validity or a protocol binding.

**Content coding**:
A transformation named by a Content-format declaration. Coding tokens apply in their stated order; repeated tokens denote repeated transformations. An absent coding sequence denotes no transformation, following [RFC 9193 Section 3](https://www.rfc-editor.org/rfc/rfc9193.html#section-3).

**Dated positive registration**:
An entry in a specifically recorded relevant registry edition, with its canonical identifier and dated temporary, obsolete or deprecated flags. A missing positive leaves current registration unknown; it establishes no negative registration fact.

**Unit name**:
The identifier of a measurement unit carried by `unit`, normally using a registered SenML unit name, with URI-based identification also available under the RFC's conditions.

**Nullable**:
A data characteristic admitting a null value beyond the effective type, with default `true`, while other applicable constraints still apply, according to [ADR 0005: Nullability and presence](docs/adr/0005-nullability-and-presence.md).
_Avoid_: Optional presence.

**Byte string**:
An information-level sequence of bytes identified by `sdfType: "byte-string"`.

**Unix time**:
A point in civil time identified by `sdfType: "unix-time"`, with a conventional numeric POSIX-time representation.

**Abstract representation**:
A conventional JSON input channel for an information-level value. Usable representation input and established denotation are separate facts, following ADR 0007: Generated SDF and sample JSON.

**Established denotation**:
The abstract value established under a known, applicable interpretation, beyond recognizing its name or retaining its representation.

**Model meaning**:
The interpretation of an effective data description, independently of any supplied data value. Establishing it does not prove that an arbitrary representation denotes a value, that values can be compared, that a particular value is admitted, or that the description admits any value at all.

**Sample meaning**:
The final supporting references and limitations for selected abstract values, const and admission. An alternative's whole witness determines final proof, while independent unfinished areas remain visible.

**Consulted sample interpretation**:
A conventional byte-string or POSIX interpretation actually attempted during the request. Consultation survives discarded alternatives and failed work without asserting final proof or completed denotation.

**sdfInputData**:
The data description of an Action's input.

**sdfOutputData**:
The data description of an Action's or Event's output.

**Readable**:
A Property characteristic indicating that its state can be read, with default `true`.

**Writable**:
A Property characteristic indicating that its state can be written, with default `true`.

**Observable**:
A Property characteristic indicating availability of asynchronous state notifications, with default `true`.

**Property characteristic**:
A Boolean quality describing whether Property state is readable, writable, or observable.

**Declared characteristic**:
A Property characteristic or Nullable characteristic supplied by an authored value, including one inherited from a reused definition. Its contributor origin is a separate question.

**Specified default characteristic**:
A Property characteristic or Nullable characteristic supplied by the standard when its quality is absent in an established role. It describes the interface without adding a quality to the document or inserting interaction data.

## Value constraints

Terminology source: [RFC 9880, Appendix C](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C). Grouping-array uses of `minItems` and `maxItems` are described separately above.

**minimum / maximum**:
Inclusive lower and upper bounds for numeric values.

**exclusiveMinimum / exclusiveMaximum**:
Numeric lower and upper bounds that exclude the bound itself.
_Avoid_: Boolean exclusivity flags.

**multipleOf**:
A positive numeric step requiring the value to be an integer multiple of that step.

**Numeric lattice gap**:
A finite ordinary numeric interval containing no integer, or no integer multiple
of an applicable positive step, after endpoint exclusion and established null
exclusion. An integer description also intersects the step lattice with integers,
following
[ADR 0030: Bounded numeric advice](docs/adr/0030-bounded-numeric-advice.md).

**Numeric const-bound contradiction**:
A sole ordinary numeric const excluded by an applicable numeric bound. Established
absence of type does not supply an authored type or type origin. Unavailable
magnitude is distinct from an independently established nonnull const root kind.

**minLength / maxLength**:
Inclusive bounds on text-string length measured in Unicode scalar values.
_Avoid_: UTF-8 byte count, displayed-character count.

**pattern**:
An ECMA-262 regular expression in Unicode mode constraining text strings. Matching is unanchored unless the expression supplies anchors.
Supported syntax and matching capabilities are distinct; unavailable capability leaves affected conclusions incomplete, following [ADR 0014: Bounded partial ECMAScript pattern capability](docs/adr/0014-bounded-pattern-capability.md).
The explicit expanded pattern-syntax assessment checks surviving effective Full string patterns; Direct donor meaning remains unavailable until an establishing Full override. Full baseline assessment binds this rule to its model requirement and pattern/format area, following [ADR 0028: Bounded effective model pattern syntax](docs/adr/0028-bounded-model-pattern-syntax.md).
_Avoid_: The I-Regexp conventions used for name patterns in the RFC text.

**format**:
Additional abstract text-data meaning, such as a civil date or time, URI, URI reference, or UUID. Its meaning is separate from the conventional input representation.

**URI value**:
A full resource identifier with a scheme, which may include a fragment. Identification does not imply resource retrieval, following [RFC 3986 Sections 1.2.2 and 3](https://www.rfc-editor.org/rfc/rfc3986.html#section-1.2.2).
_Avoid_: Relative reference, fragment-free absolute-URI.

**URI-reference value**:
A full URI or relative resource-reference expression, including empty, query-only and fragment-only forms. A relative expression's target can depend on a base, following [RFC 3986 Sections 4.1 and 4.2](https://www.rfc-editor.org/rfc/rfc3986.html#section-4.1).
_Avoid_: Model name reference, automatically retrieved resource.

**UUID value**:
An opaque 128-bit identifier as defined by [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html#section-4).
_Avoid_: A required version-4 identifier, text spelling, proof of globally unique generation.

**Array data**:
An ordered collection of interaction-data values selected by `type: "array"`.
_Avoid_: Grouping array.

**items**:
The description that every element of array data must satisfy, subject to the item-description vocabulary defined by base SDF.

**minItems / maxItems**:
Inclusive bounds on the element count of array data, or on grouping repetition when used as grouping qualities.

**uniqueItems**:
A Boolean data quality requiring all array elements to differ when set to `true`.

**Value equality**:
Comparison of data values in an applicable effective context, returning equal, different or undetermined. Equality is separate from sample admission, following [ADR 0015: Bounded value equality in a resolved data context](docs/adr/0015-value-equality.md).


## Model work

These terms distinguish the work performed on SDF specifications from the network interactions they describe.

**Pinned document acquisition**:
Explicit retrieval of one selected source document at a complete revision identity, retaining its unchanged bytes only when their digest matches a separately supplied expectation.
_Avoid_: Model checking, namespace resolution or dependency discovery.

**Acquired snapshot**:
The unchanged local bytes retained by Pinned document acquisition. Its content identity does not establish JSON decoding, SDF validity or device behavior.

**Acquisition receipt**:
Evidence of a Pinned document acquisition request and its actual transfer, content verification and output outcome. It is separate from a Model checking report.

**Model checking**:
Assessment of an SDF specification against the applicable SDF requirements.
_Avoid_: Sample-value checking when assessing the specification itself.

**Model authoring**:
Creation or revision of SDF documents that express definitions, declarations, and their relationships.

**Generated SDF document**:
A new SDF document expressing an authoring request's selected definitions, Given Names, namespace choices and Composition patches. Its existence does not establish completion of the requested assembly or Model checking.

**Authoring fidelity**:
Established preservation of the requested model meanings, Given Names, data values and permitted opaque content during Model authoring, following [ADR 0017: Reference-preserving sdfThing authoring](docs/adr/0017-reference-preserving-composed-thing-authoring.md). Retained opaque structure alone does not establish its meaning after a context change.

**Authored-quality edit**:
An explicit replacement or removal of a present quality on a selected authored named definition, preserving the other authored content and its context. Composition patches have a separate null meaning and are outside this operation, following [ADR 0034: Authored-quality editing with source substitution](docs/adr/0034-authored-quality-editing-with-source-substitution.md).

**Editing request**:
The selected original document and explicit authored owner, Quality Name and replacement or removal intent for each requested Authored-quality edit.

**Edited SDF document**:
A generated revision expressing an Editing request. Candidate availability does not establish Editing fidelity or Model checking.

**Editing fidelity**:
Established application of every requested Authored-quality edit and preservation of every other authored JSON value, Given Name, definition position, namespace mapping and reference spelling under `esdieff.authored-quality-edit/1`. It establishes source preservation, not before-and-after model equivalence or unknown extension meaning.

**Source substitution**:
Replacement of exactly one original contributor by its Edited SDF document in the active result Catalogue. The original remains immutable Source evidence and cannot resolve names as a fallback.

**Active result Catalogue**:
The Edited SDF document and all other active supplied contributors after Source substitution. Explicitly supplied aliases remain distinct contributors.

**Model inspection**:
Examination of a model's definitions, declarations, affordances, and data descriptions.

**Catalogue**:
The SDF documents supplied together for model work, whether selected individually or from an explicitly supplied local collection.
_Avoid_: Namespace, since a catalogue can cover multiple namespaces and several documents can contribute to one namespace.

**Root document**:
A document selected as the starting point of an SDF model under examination.

**Catalogue check**:
Model checking whose scope includes every document in the supplied catalogue, including unused definitions.

**Selected-model check**:
Model checking whose scope starts from selected root documents and includes the dependencies needed for the checks.

**Check plan**:
Saved model and sample source identities and ordered named checking and Revision comparison requests for a repeatable catalogue workflow.
_Avoid_: SDF document or Assessment profile.

**Check case**:
One named Model checking, Sample-value checking or Revision comparison request within a Check plan, retaining its own scope, selector or revisions and policy, and either its actual result or a request error. A Revision comparison names its own revisions' documents, apart from the plan's sources. Sample admission and a comparison's classification remain separate from disposition and completeness.
_Avoid_: SDF definition, Assessment area or expected-result assertion.

**Check coverage**:
The model scope and SDF requirements that a check actually assessed.

**Requested assessment**:
The predetermined requirements selected for a model check, independently of which checks a processor can currently complete.

**Assessment profile**:
A named set of requirements defining a requested assessment.
_Avoid_: Implemented checks when referring to the profile's promised scope.

**Assessment area**:
A named group of requirements in a requested assessment, counted once across its model scope.

**Model requirement inventory**:
The immutable, cited model obligations selected by a requested assessment, independently of current implementation. `rfc9880.model/1` contains 54 exclusive obligations from the normative body and Appendices A/C. The Full area inventory `esdieff.full-model/1` groups them into 12 model areas plus the existing advisory area, following [ADR 0011: Machine-readable report contracts](docs/adr/0011-machine-readable-report-contracts.md). Initial retains its narrower eight areas. Sample, device, protocol, registry and operator duties have separate roles.
_Avoid_: Implemented checks or user stories as a complete standards inventory.

**Coverage owner**:
The requested Full area owning an actual recorded requirement. A condition retains its producing assessment-unit identity; optional coverage-owner detail relates it to Full without renaming its producer.

**Assessment status**:
Whether a requested area is assessed, established not applicable, unassessed or undetermined because needed facts or capability are unavailable. Assessed includes established findings, rather than asserting that the model satisfies the area.

The same four states describe fine-grained model obligations. Repeated observations join undetermined before unassessed, assessed, then not applicable, so one completed occurrence cannot hide an unfinished sibling. Areas join their owned requirements once. Completion, error counts and optional presentation are independent.

**Incomplete check**:
A check whose requested assessment could not be completed, for example because a needed definition or critical feature was unavailable.

**Deferred model check**:
A model-checking capability deliberately left for later delivery. Deferral preserves the affected requirements and their actual assessment status; it does not establish that they are satisfied or not applicable.

**Constraint contradiction**:
Applicable effective data constraints that together admit no values.
_Avoid_: Definite RFC violation unless a violated requirement is established independently.

**Sample value**:
Example interaction data whose meaning and constraints are described by an SDF data description.
_Avoid_: Protocol payload without an applicable protocol binding.

**Sample projection**:
Conventional JSON text generated from a typed sample value, retaining its exact numerical representation but carrying no original source-text provenance.
_Avoid_: Original sample bytes when referring to generated text.

**Sample-value checking**:
Assessment of one sample value against a selected effective data description, returning admitted, rejected, or undetermined. Admission is separate from model findings and requested-work completeness, following [ADR 0016: Selected samples with explicit admission and coverage](docs/adr/0016-selected-sample-contract.md).
Ordinary map entries and array elements conjoin with their containing value's constraints. Array counts are inclusive and exact. An empty array requests no item value and establishes no meaning for a deferred item subtree. Repeated elements share descriptions while accumulating actual present interests; completed model conditions are observed once per final description.
_Avoid_: Model checking when assessing example interaction data.

**Generated value**:
A value Esdieff constructs for a selected effective data description, aimed at one Generation target and settled by Sample-value checking. Its admission, not its target, says whether the description admits it, following [ADR 0041: Generated values settled by Sample-value checking](docs/adr/0041-generated-values-settled-by-sample-value-checking.md).
_Avoid_: Sample value, which a caller supplies; Representative value, which revision comparison tries as a Breaking witness.

**Generation target**:
The named edge of a data description that a Generated value probes, such as `minimum`, `below_minimum` or `missing_entry`, fixed by the generation contract. A target that does not apply produces no value.
_Avoid_: Expected result, since only Sample-value checking settles admission.

**Simulation trace**:
The ordered record of one deterministic device demonstration: an initial state reading, then each command's step, simulated time and invocation with its input, optional Action result and Event, and subsequent state reading in observed order. Sample-value checking assesses its individual values, while its sequence meaning comes only from the demonstration's own authored rules and adds nothing to any Model requirement inventory.
_Avoid_: Check case, protocol log, device state machine.

**Authored behavior assertion**:
A named rule that a demonstration states for itself, relating its authored commands and independently tracked expected state to an observed Simulation trace, such as when a stateChanged Event must appear. It is never inferred from SDF descriptions, and it is separate from a Check case, the admission result of Sample-value checking and the Model requirement inventory.
_Avoid_: SDF requirement, model obligation, admission result, device protocol rule.

**Recorded trace**:
A JSON Lines file of Trace entries, observed from a Thing or Client or prepared as test vectors, whose entries are checked independently against one selected document. Line order carries no meaning beyond presentation, following [ADR 0042: Recorded traces with independent entries](docs/adr/0042-recorded-traces-with-independent-entries.md).
_Avoid_: Simulation trace, which has authored sequence rules; protocol log.

**Trace entry**:
One line of a Recorded trace: a Sample selector, a value and optionally an Expected admission and a label. A malformed entry fails alone without stopping the others.

**Expected admission**:
The admission a Trace entry states for its value, admitted or rejected. A definite admission that differs from it fails the entry, and an undetermined admission stays undetermined.
_Avoid_: Generation target, which names the edge a Generated value probes.

## Revision comparison

These terms describe how a later revision of a model affects the clients of the Things it describes.

**Model revision**:
The documents of an SDF model at one point in their evolution, supplied together as one Catalogue.
_Avoid_: version, which is an authored document quality with no mandatory format.

**Corresponding definition**:
A grouping or affordance in a later Model revision with the same Given Name position, in the corresponding document, as one in the earlier revision. Documents correspond by the key the caller gives each one, which `esdieff diff` takes from the document's location within its revision. A moved or renamed definition has no correspondent, so it reads as a removal and an addition.
_Avoid_: Global name, which many device documents lack.

**Revision comparison**:
Classification of each difference between the corresponding groupings of two Model revisions as breaking, compatible or undetermined, from the Client's side. Changes to shared definitions appear through the effective groupings that use them. A difference Esdieff cannot classify remains undetermined, never compatible, following [ADR 0039: Client-side revision comparison](docs/adr/0039-client-side-revision-comparison.md).
_Avoid_: Model checking, which assesses one model against SDF requirements.

**Client**:
The party that reads, writes and observes a Thing's Properties, invokes its Actions and receives its Events.
_Avoid_: Consumer, which names an Importing consumer in composition.

**Client-supplied data**:
Interaction data a Client sends to a Thing: written Property values and Action input.

**Thing-supplied data**:
Interaction data a Thing sends to a Client: read or observed Property values, Action output and Event output. A Property that is both readable and writable carries data in both directions.

**Client direction**:
Whether data is Client-supplied or Thing-supplied. A Property's effective characteristics decide which directions its data carries.

**Breaking change**:
A difference between two Model revisions after which an interaction the earlier revision described can fail for a Client, or yield data the Client was not described to receive. Narrowed Client-supplied data, widened Thing-supplied data, a removed grouping or affordance, a Property characteristic that becomes false, input data added to or removed from an Action, output data removed from an Action or Event, a declaration that stops being mandatory, a widened grouping instance count and a change between two established meanings of a unit, Content-Format or `sdfType` are breaking. A difference that only obliges Things to accept or provide more is not, and neither is describing what no Client of the earlier revision uses, such as a map entry it neither describes nor requires or output data an affordance did not have.
_Avoid_: Any difference; Restrictive refinement, which concerns overrides within one model.

**Meaning quality**:
A data quality that tells a Client how to read a value: `unit`, `contentFormat` or `sdfType`.

**Breaking witness**:
A value that one revision's effective data description admits and the other's rejects, in the direction that makes the difference breaking, as established by Sample-value checking against both revisions. A change to the values a data description admits is breaking only with a witness, compatible only with a proof that no witness exists, and otherwise undetermined.

**Representative value**:
A simple value that an effective data description may admit, generated from its constant, enum values or documentary default, as one value of each type it admits or of each Effective alternative of its choice, or as `null` when it is Nullable. Revision comparison tries it as a candidate Breaking witness from the revision that must admit it, and value generation offers it as a Generated value. Sample-value checking settles each one, so a representative value its own description rejects establishes nothing.
_Avoid_: Sample value, which a caller supplies.

**Monotone rule**:
How a change to one data quality relates the values two Model revisions admit, with every other quality as it is: the later revision admits more, fewer or the same values, or none of these is established.

**Compatibility proof**:
A demonstration that a data description's changes admit no Breaking witness: every changed quality loosens in the safe direction for each Client direction, widening Client-supplied data and narrowing Thing-supplied data, and nothing else in the description changed except documentation or content no Client of the earlier revision uses.
_Avoid_: Subset proof, which suggests general satisfiability solving.

**Difference acknowledgement**:
A maintainer's record that one difference between two Model revisions is intended, naming its kind, subject, quality, classification and its location in each revision, following [ADR 0040: Acknowledged revision differences](docs/adr/0040-acknowledged-revision-differences.md).
_Avoid_: Acceptance, which names an Acceptance restriction, and baseline, which names the Standards baseline.

**Acknowledged difference**:
A breaking or undetermined difference that matches a Difference acknowledgement. It keeps its classification but no longer decides the comparison's disposition.

**Version warning**:
Advice that a changed document with a Breaking change has a later `info` `version` that is absent or does not compare higher, although RFC 9880 recommends a version that compares higher in every later revision, following [ADR 0044: Versions of breaking revisions](docs/adr/0044-versions-of-breaking-revisions.md). It never decides the comparison's disposition.
_Avoid_: Version check, which suggests a required format.

## Base SDF and extensions

Terminology sources: [RFC 9880, Section 1](https://www.rfc-editor.org/rfc/rfc9880.html#section-1), [Section 1.2.3](https://www.rfc-editor.org/rfc/rfc9880.html#section-1.2.3), and [Section 3.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.1).

**Base SDF**:
The SDF specification defined by RFC 9880, including its extension mechanisms.

**SDF extension**:
Additional SDF vocabulary or semantics defined beyond base SDF.

**Extension feature**:
A named critical extension capability listed in a document's `features` quality because correct processing requires understanding it.

**features**:
The information-block quality listing critical extension features. A feature name is a registered name or a URI; a URI name does not imply retrieval.

**Undeclared extension vocabulary**:
A Quality Name outside base SDF, used in a document that declares no Extension feature. It leaves the base meaning of its definition intact, following [ADR 0037: Undeclared extension vocabulary](docs/adr/0037-undeclared-extension-vocabulary.md).
_Avoid_: Unknown critical feature, which defers the whole document.

**Recorded extension vocabulary**:
A qualified Quality Name whose registered definition, positions and value domain Esdieff records as dated registry evidence, such as `oma:id`, following [ADR 0038: Recorded extension vocabulary](docs/adr/0038-recorded-extension-vocabulary.md).
_Avoid_: Registered prefix, which identifies only the defining organization.

**Framework syntax**:
The normative SDF grammar with liberal extension points, defined in RFC 9880 Appendix A.

**Formal admission**:
Membership of a supplied value in the applicable normative grammar alternative. A value admitted through a framework extension alternative can have unavailable extension meaning. Formal admission is separate from exact mathematical meaning and actual sample admission, following [ADR 0032: Formal model admission](docs/adr/0032-formal-model-admission.md).

**Validation syntax**:
The stricter grammar derived by removing the framework syntax's extension points to help detect vocabulary mistakes.
_Avoid_: Complete semantic checking, which also requires prose requirements and composition processing.

**Augmentation mechanism**:
Companion information that supplements an SDF model for an ecosystem or another specific use.

**Protocol binding**:
An augmentation mechanism that maps abstract model concepts to a particular ecosystem's network protocols.

## Conformance

**Standards baseline**:
The RFC, explicitly adopted verified corrections, and recorded external knowledge against which SDF requirements are assessed.

**Temporal knowledge**:
Recorded facts about dated UTC leap adjustments and the interval in which adjustment or nonadjustment is established.

**RFC interpretation**:
An agreed reading used to resolve an ambiguity or inconsistency in the RFC without intentionally departing from the standard.

**Agreed deviation**:
An explicitly accepted departure from an RFC requirement.
_Avoid_: RFC interpretation, incomplete check, or unimplemented requirement.

**Acceptance restriction**:
An explicitly agreed Esdieff policy refusing particular supplied model or sample content without asserting that its presence necessarily violates the RFC.
_Avoid_: Definite RFC violation, agreed deviation when no departed requirement has been identified.

**Recorded registry evidence**:
A deliberately captured set of positive registry facts whose edition identifies its scope and date. A positive match establishes membership in that evidence; a miss leaves current registration unknown, following [ADR 0027: Unit names and recorded registry guidance](docs/adr/0027-unit-names-and-recorded-registry-guidance.md).

**Recorded evidence policy**:
An embedding's request-level choice to use or withhold the bundled dated registry evidence. Withholding makes needed evidence unavailable for that request without asserting that the recorded table is physically absent.

**Registry consultation**:
Actual comparison against a recorded registry table, independent of whether a match or a completed lookup was established.

**Unit source edition**:
The common raw registry source from which the unit sections were extracted. Its use identifies at least one consulted section, while each section retains its own extracted identity.

**Consulted format fact**:
A static specification record actually consumed while a selected Sample request
assesses a URI, URI-reference or UUID. Its immutable identity, RFC, sections and
scope describe the consulted rule. Observations survive discarded alternatives
and late limits; an unavailable initial owner differs from an established empty
set. See ADR 0019: URI, URI-reference and UUID sample conventions.
