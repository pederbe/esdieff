# Context metadata

Context definitions describe non-interactive metadata, such as serial numbers, installation locations and calibration offsets. They live under `sdfContext` inside an Object or Thing. A temperature reading that a client reads or observes remains a Property.

Esdieff supports inline checking and inspection from [draft-ietf-asdf-sdf-nonaffordance-06](https://www.ietf.org/archive/id/draft-ietf-asdf-sdf-nonaffordance-06.txt), dated 27 October 2026. This is draft extension support alongside RFC 9880. Reports record the selected revision and its digest, following [ADR 0046: Inline Context draft support](adr/0046-inline-context-draft-support.md).

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

## Semantic boundaries

The draft imports RFC data qualities but also says absent qualities have no assumed defaults. Effective Context descriptions, including nested descriptions and alternatives, therefore leave absent `nullable` meaning unavailable. Source views retain the authored absence. Explicit `nullable` values work normally, and independent checks continue. For example, checking a declared `const: null` may be undetermined when `nullable` is absent, while an independent numerical constraint can still be assessed. Base-SDF descriptions keep their established nullable default.

The draft leaves Context use in `sdfRequired` undefined. Context does not become a mandatory affordance or satisfy an affordance requirement just by sharing its name. Explicit Context requirements receive `context.required_undefined` with an incomplete draft basis. The `required` quality inside map data remains supported.

Context does not acquire readable, writable or observable Property characteristics. No device endpoints are implied. A link-bearing description stays inspectable, but general `sdfType: "link"` meaning is unavailable. Knowing Context does not establish an unrelated critical feature listed in `info.features`.

This support covers inline checking and inspection. Reference reuse of a Context definition or its nested data descriptions reports an unavailable capability, so it cannot accidentally establish a withheld default in another role. Context composition, authoring, editing, supplied-value selection, generated vectors, traces and revision differences have separate support work. Runtime metadata messages and transport bindings are outside this workflow.
