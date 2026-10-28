# Context metadata

Context definitions describe non-interactive metadata, such as serial numbers, installation locations and calibration offsets. They live under `sdfContext` inside an Object or Thing. A temperature reading that a client reads or observes remains a Property.

Esdieff supports checking, composition, inspection and selected-value checking from [draft-ietf-asdf-sdf-nonaffordance-06](https://www.ietf.org/archive/id/draft-ietf-asdf-sdf-nonaffordance-06.txt), dated 27 October 2026. This is draft extension support alongside RFC 9880. Reports record the selected revision and its digest, following [ADR 0046: Inline Context draft support](adr/0046-inline-context-draft-support.md) and [ADR 0047: Context reuse with existing composition](adr/0047-context-reuse-with-existing-composition.md).

## Check and inspect

The [example device](../examples/context-inspection/device.sdf.json) describes a temperature Property and three Context entries. From the repository root:

```sh
esdieff check --assessment full-baseline examples/context-inspection/device.sdf.json
esdieff inspect --view source --document examples/context-inspection/device.sdf.json --pointer /sdfObject/Device examples/context-inspection/device.sdf.json
esdieff inspect --view effective --document examples/context-inspection/device.sdf.json --pointer /sdfObject/Device/sdfContext/installationLocation examples/context-inspection/device.sdf.json
```

The existing assessment profiles apply their supported data checks to Context, including nested map properties, alternatives, arrays, constraints, units and formats. Use `--assessment expanded-consistency-v1` to check declared constant and documentary default values against their descriptions. Existing processing limits and unavailable meanings still apply.

Empty Context groups are allowed. A group must be a map, each named definition must be a map, and names follow the imported Given Name rules. A Context group at the document top level or inside another definition class receives a draft placement error.

## Rust views

`Class::Context` identifies the class. Obtain an Object or Thing through the existing source or effective definition views, then call `children(Class::Context)`. Each Context definition exposes `data_description()`, names, source or effective locations and the supported data qualities. A same-name Property remains a separate child selection. Context's Property characteristic accessors return absent.

## Reuse and refinement

A Context can reuse an `sdfData` description through a local `sdfRef` or a namespace reference to another supplied document. Reusing an Object or Thing also inherits its Context group. JSON Merge Patch lets the consumer refine constraints, add entries and delete inherited content with `null`. Final checks apply to the resulting descriptions.

The [reuse example](../examples/context-reuse/README.md) inherits a Site and Sensor, reuses Location and AssetTag data descriptions, refines a calibration limit and removes a retired Context entry. Its [Rust example](../examples/context_reuse.rs) checks the selected model and asserts effective constraints and content origins. From the repository root:

```sh
esdieff check --root examples/context-reuse/device.sdf.json --assessment full-baseline examples/context-reuse/base.sdf.json examples/context-reuse/device.sdf.json
esdieff inspect --view effective --document examples/context-reuse/device.sdf.json --pointer /sdfThing/Installation/sdfObject/sensor examples/context-reuse/base.sdf.json examples/context-reuse/device.sdf.json
cargo run --example context_reuse
```

Effective explanations identify inherited, overridden and deleted qualities, their nearest source content and the importing `sdfRef`. Internal copied references follow the importing definition's refinements under [ADR 0036: Copied reference relocation](adr/0036-copied-reference-relocation.md). Original documents remain unchanged.

Missing or ambiguous targets, cycles, exhausted processing budgets and unsupported critical features leave dependent conclusions incomplete. Independent supported checks and subviews remain available. Namespace references use only supplied documents and never fetch a model from the network. A selected-model check excludes an inherited reference deleted by its patch, while a Catalogue check still checks that reference in the supplied donor.

## Check Context values

Use `esdieff sample --kind context` to check one supplied JSON value against a Context definition. The pointer selects its final effective description, including inherited constraints and refinements. A same-name Property uses `--kind property` and its own `sdfProperty` pointer.

The [sample files and Check plan](../examples/context-samples/README.md) use the reuse example above. From the repository root, check a simple asset tag, then a compound location with a required room and an integer floor:

```sh
esdieff sample --document examples/context-reuse/device.sdf.json --kind context --pointer /sdfThing/Installation/sdfObject/sensor/sdfContext/assetTag --sample examples/context-samples/asset-tag.json examples/context-reuse/base.sdf.json examples/context-reuse/device.sdf.json
esdieff sample --document examples/context-reuse/device.sdf.json --kind context --pointer /sdfThing/Installation/sdfObject/sensor/sdfContext/location --sample examples/context-samples/location.json examples/context-reuse/base.sdf.json examples/context-reuse/device.sdf.json
esdieff check --plan examples/context-samples/check-plan.json
```

Both values are admitted. Replacing the tag with `"A-18"` rejects it against the constant. A location with no `room` is rejected, as is a floor below the refined minimum of 2. Reports retain separate sample and model locations, the Context selector and the existing admission and coverage results. Add `--format json` for the structured report. An unavailable reference, applicable unsupported link meaning or exhausted limit leaves affected work incomplete. A null value with absent `nullable` remains undetermined under the draft policy below.

Rust callers use `SampleKind::Context` with `SampleRequest::new`. An effective Context's `data_description()` also supports `SampleRequest::from_description`, without rebuilding a pointer or interpreting constraints. These follow the [selected-sample contract](sample-contract.md).

This checks the value you supply. It does not establish that Context is present on every Thing, expose a readable, writable or observable endpoint, insert documentary defaults or change the model. Instance-information messages remain outside this operation.

## Author and edit

The existing Thing-authoring workflow preserves Context groups in selected reusable Objects. Recipe refinements can override, add or delete Context content through the same composition patches used for other supported definitions. Supported Context content participates in authoring fidelity. Unsupported nested vocabulary and unavailable references still leave affected fidelity undetermined. Source and request origins identify inherited content, refinements and deletions.

The existing editing workflow can replace or remove present supported common and data qualities on authored Context definitions under an Object or Thing. It uses the same quality set as `sdfData`, listed in the [editing contract](editing-contract.md). Owners inside an authored `sdfRef` boundary, nested anonymous data descriptions and unsupported qualities remain request errors. Editing preserves unrelated content and exact number spellings under the existing source-preservation contract.

Follow the [authoring and editing example](../examples/context-authoring/README.md) to assemble a sensor, refine its Context and edit an authored site-name definition. Both commands accept `--check full-baseline` to check the generated model under the recorded Context draft policy. Generation, fidelity and requested checking retain their independent outcomes. A failed or unfinished requested check prevents ordinary document delivery.

## Semantic boundaries

The draft imports RFC data qualities but also says absent qualities have no assumed defaults. Effective Context descriptions, including nested descriptions and alternatives, therefore leave absent `nullable` meaning unavailable. Source views retain the authored absence. Explicit `nullable` values work normally, and independent checks continue. For example, checking a declared `const: null` may be undetermined when `nullable` is absent, while an independent numerical constraint can still be assessed. Base-SDF descriptions keep their established nullable default.

The draft leaves Context use in `sdfRequired` undefined. Context does not become a mandatory affordance or satisfy an affordance requirement just by sharing its name. Explicit Context requirements receive `context.required_undefined` with an incomplete draft basis. The `required` quality inside map data remains supported.

Context does not acquire readable, writable or observable Property characteristics. No device endpoints are implied. A link-bearing description stays inspectable, but general `sdfType: "link"` meaning is unavailable. Knowing Context does not establish an unrelated critical feature listed in `info.features`.

Context-to-Context reuse preserves the same defaults boundary, including nested descriptions. Inherited explicit `nullable` values work normally. Deleting `nullable` restores the unresolved default. Reusing a Context description in a non-Context role remains unavailable, so it cannot silently acquire a withheld default. Factor descriptions intended for sharing across roles or models into `sdfData`, as the draft recommends.

Generated vectors, traces and revision differences retain their separate support boundaries. Runtime metadata messages and transport bindings are outside this workflow.
