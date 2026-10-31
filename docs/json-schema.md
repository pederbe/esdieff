# Export JSON Schema

Export the effective constraints of one Property value, Action input or output,
Event output or Context definition for use with JSON Schema tools.

```sh
esdieff schema --document examples/thermostat-check/rich.sdf.json \
  --kind action_input --pointer /sdfObject/Thermostat/sdfAction/configure \
  examples/thermostat-check/rich.sdf.json
```

The default output is an `esdieff.schema/1` JSON report. It identifies the
selected document, kind and definition pointer, supplied source attempts,
dialect, fidelity and first refusal reason. A successful report contains a
`schema` object. A refusal contains `schema: null` and exits with status 3.
Invalid requests exit with status 2. Failed host output uses status 4, as with
other commands.

Use `--format schema` to write only the standalone schema to stdout. On refusal
this mode writes the reason to stderr, exits with status 3 and writes nothing
to stdout. Shell redirection may still create an empty file, so check the exit
status before using the file.

Select `property`, `action_input`, `action_output`, `event_output` or `context`.
Action and Event pointers name the affordance, as in Sample-value checking.
Context pointers name the Context definition itself. Supply dependencies as
additional explicit inputs. Export never downloads dependencies. One input may
be `-` for stdin. Use `--` before paths starting with a hyphen.

## Fidelity

The dialect is [JSON Schema Draft 2020-12](https://json-schema.org/draft/2020-12/schema).
Successful fidelity is `json-values`. This means the supported effective
constraints describe ordinary JSON values using the mappings below. It does
not establish whole-model validity, a protocol payload binding, satisfiability
or generated-code correctness. Model acquisition and reference composition
reuse the normal catalogue. The report records whether composition relied on
the copied-reference relocation interpretation.

| Effective SDF quality | JSON Schema meaning |
| --- | --- |
| `type`, `nullable` | Type union with null when nullable, or explicit null exclusion for an untyped description |
| Numeric bounds, `multipleOf` | Same constraints, retaining exact number tokens |
| `minLength`, `maxLength` | Unicode character counts |
| `minItems`, `maxItems`, `uniqueItems` | Array bounds and ordinary JSON value equality |
| `properties`, `required` | Described entries and mandatory presence, with extra entries allowed |
| `const` | Ordinary scalar or compound JSON equality |
| `sdfChoice`, `enum` | `anyOf` over effective alternatives after shared-quality inheritance and whole overrides |
| `label`, `description`, `$comment` | `title`, `description`, `$comment` annotations |
| `default` | Documentary default, without inserting a value |
| `unit` | `x-sdf-unit` annotation, without conversion or unit validation |

Nullable defaults to true for ordinary Full data descriptions. It widens type
admission only. Const and enum constraints still apply to null. An optional map
entry is separate from an entry whose value can be null. Repeated SDF required
names become one JSON Schema required name. Choice alternatives can overlap,
so `oneOf` would be incorrect.

Property read/write/observe characteristics, declaration requiredness and
enclosed reusable data definitions do not constrain this selected value. They
are excluded from the schema. This export does not model interactions.

The schema preserves exact numeric spelling, including values beyond machine
integer or floating-point ranges. Your downstream tool must use exact arithmetic
if those distinctions matter. Typed deserialization alone does not enforce
bounds, steps, presence or other model constraints.

## Refused descriptions

Export withholds the entire schema if any needed selected description contains
unsupported meaning, even in an optional map entry or an alternative. It does
not offer a lossy fallback. The first export supports no `format`, `sdfType`,
`contentFormat`, `pattern`, Direct `items` descriptions, unknown data types or
extension qualities. Context needs explicit `nullable` because its draft does
not establish the ordinary default. Unestablished object structure, unavailable
references, malformed supported constraints and exhausted processing limits
also prevent export.

These boundaries preserve existing semantics. For example, format denotation
can change equality, patterns depend on engine behavior, and Direct items have
restricted vocabulary and unavailable nullability. Copying those keywords would
make a stronger fidelity claim than the existing model supports.

Composition and export share the configured work, node, reference and derived
allocation allowances. Exported schema traversal has a depth ceiling of 64 and
uses the report byte ceiling, reserving space for a refusal first. If mandatory
identity cannot fit, the library returns a request error rather than a report.
Existing check and inspection report versions do not change.

## Library

```rust
use esdieff::{Catalogue, SampleKind, SchemaRequest, SourceInput};

let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
    "sensor", br#"{"sdfProperty":{"reading":{"type":"number","minimum":0}}}"#,
)])?;
let document = catalogue.source_id("sensor").unwrap();
let result = catalogue.export_schema(SchemaRequest::new(
    document, SampleKind::Property, "/sdfProperty/reading",
))?;
if let Some(schema) = result.schema() {
    println!("{schema}");
} else {
    println!("{}", result.to_json());
}
# Ok::<(), &'static str>(())
```

The comparison fixtures in `tests/schema_export.rs` use independently expected
samples, Esdieff Sample-value checking and a separate Draft 2020-12 validator
with exact arithmetic. They include the maintained thermostat model, deliberately
authored constraints and refusal cases. These fixtures establish the bounded
bridge, not equivalence for unsupported SDF meaning. See
[ADR 0052](adr/0052-selected-json-schema-export.md).
