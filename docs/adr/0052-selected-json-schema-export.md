# 0052: Selected JSON Schema export

Selected effective Property values, Action input/output, Event output and
Context descriptions can be exported as JSON Schema Draft 2020-12 through
`Catalogue::export_schema` and `esdieff schema`. Effective composition, selectors
and alternatives remain library-owned. The separate `esdieff.schema/1` report
uses `json-values` fidelity for the documented ordinary JSON subset and
`refused` when that subset cannot represent the selected meaning. Refusal
withholds the whole schema, including when an unsupported branch is optional.
This avoids giving downstream validators apparently complete schemas with
missing meaning.

Choices use `anyOf` over effective alternatives, since branches can overlap and
can replace shared qualities. Nullable widens type admission without weakening
const or enum. Numbers retain their exact tokens. Units and defaults are
annotations, with no conversions or inserted values. Interaction characteristics
and declaration requiredness remain outside selected-value export.

Formats, abstract types, content formats, patterns, extension qualities and
Direct items are refused. The existing interpretations do not establish a
general equivalent JSON Schema interpretation, particularly for denotation,
equality and Direct items nullability. Context still requires explicit
nullability. The [export contract](../json-schema.md) records supported mappings,
limits and examples, grounded in [RFC 9880](https://www.rfc-editor.org/rfc/rfc9880.html),
[JSON Schema validation](https://json-schema.org/draft/2020-12/json-schema-validation)
and [JSON Schema core](https://json-schema.org/draft/2020-12/json-schema-core).

Direct Rust type generation would require naming and generated-code compatibility
policy while still needing separate constraint validation. Lossy schema export
would risk silent false admission. A bounded schema bridge lets existing tools
serve those uses without either additional contract. A separate exact-arithmetic
validator is a development dependency only, for comparison fixtures.
