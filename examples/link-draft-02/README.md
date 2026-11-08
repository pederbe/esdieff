# Link draft examples

Five unchanged JSON examples from the IETF's
[Link sdfType draft, revision 02](https://www.ietf.org/archive/id/draft-ietf-asdf-sdftype-link-02.html),
dated 23 June 2026, are saved here for offline checking. A link value is a JSON
object whose lowercase members are named after Web Link target attributes, such
as `href` for the target and `rt` for a resource type.

| Draft location | File | Links described |
| --- | --- | --- |
| Section 2 | [section-2-link.sdf.json](section-2-link.sdf.json) | A temperature link with fixed `rt` and `if` attributes |
| Appendix A.1 | [my-object.sdf.json](my-object.sdf.json) | A Property reusing an organization's `idlink` |
| Appendix A.1 | [org-types.sdf.json](org-types.sdf.json) | The organization's exported `idlink` |
| Appendix A.2 | [test-object.sdf.json](test-object.sdf.json) | LwM2M object and CoRE link Properties, one refining a limit |
| Appendix A.2 | [oma-types.sdf.json](oma-types.sdf.json) | The exported `corelink` and `objlink` definitions |

These are illustrative models, with no claim of a deployed product.
[Sources and licensing](SOURCES.md) record the extraction, hashes and notices.
Esdieff checks the values a link description admits. It does not follow links
or resolve their targets. The reading of each unclear point in the draft is
recorded in [ADR 0060: Link sdfType draft support](../../docs/adr/0060-link-sdftype-draft-support.md).

## Check the examples

From the repository root:

```sh
esdieff check --plan examples/link-draft-02/check-plan.json
cargo test --test link_draft_examples
```

The plan exits 0. It checks the Section 2 example and both Appendix A.2 files
with `--assessment full-baseline`, and the A.1 export with the default
assessment. It also checks two values against the composed A.2 Properties:

- [object-link.json](object-link.json) is admitted. Object instance 0 is within
  the limit of 42 that `ObjLink_Value` adds to the exported `objlink`.
- [core-link.json](core-link.json) is admitted. It has an `href`, a typed `pmin`
  and an `_other` array of strings.

The Section 2 fragment and the A.1 export receive missing-`info` warnings.

## Rejected values

From the repository root, check two values that the draft's rules exclude:

```sh
esdieff sample --document examples/link-draft-02/test-object.sdf.json --kind property --pointer /sdfObject/LwM2M_v1.1_Test_Object/sdfProperty/ObjLink_Value --sample examples/link-draft-02/object-link-out-of-range.json examples/link-draft-02/test-object.sdf.json examples/link-draft-02/oma-types.sdf.json
esdieff sample --document examples/link-draft-02/test-object.sdf.json --kind property --pointer /sdfObject/LwM2M_v1.1_Test_Object/sdfProperty/CoreLnk_Value --sample examples/link-draft-02/core-link-uppercase.json examples/link-draft-02/test-object.sdf.json examples/link-draft-02/oma-types.sdf.json
```

[object-link-out-of-range.json](object-link-out-of-range.json) uses object
instance 43, so `sample.bound` rejects it against the refined maximum.
[core-link-uppercase.json](core-link-uppercase.json) spells a member `Pmin`.
The draft requires lowercase attribute names, so `sample.link_member_name`
rejects it. A member with no declared `type`, such as `{"rel":3}`, must be a
string and is rejected with `sample.link_member`. Both commands exit 1.

## Draft details that affect the results

Appendix A.1 has two inconsistencies, so its pair does not compose as
published. The consumer spells its namespace map `namespaces` instead of
`namespace`, which leaves `defaultNamespace` and the `orgtypes` prefix
unmapped. Checking `my-object.sdf.json` exits 1 with those two errors. The
export also has no `defaultNamespace`, so its `idlink` has no global name for
the consumer to reference.

The A.1 export spells its range qualities `minvalue` and `maxvalue`. These are
not SDF qualities, so each receives a `quality.unfamiliar` warning and no range
meaning. With `--assessment full-baseline` the export exits 3, because Full
keeps the meaning of unfamiliar qualities unavailable under
[ADR 0037: Undeclared extension vocabulary](../../docs/adr/0037-undeclared-extension-vocabulary.md).
Its link meaning is complete in either assessment.

Appendix A.2's `objlink` declares no `href`. Esdieff requires `href` only when
a description's `required` lists it, so object links without a target URI are
admitted.

## Generate and replay link values

```sh
esdieff generate --document examples/link-draft-02/test-object.sdf.json --kind property --pointer /sdfObject/LwM2M_v1.1_Test_Object/sdfProperty/ObjLink_Value --format jsonl examples/link-draft-02/test-object.sdf.json examples/link-draft-02/oma-types.sdf.json > link-vectors.jsonl
esdieff trace --document examples/link-draft-02/test-object.sdf.json --trace link-vectors.jsonl examples/link-draft-02/test-object.sdf.json examples/link-draft-02/oma-types.sdf.json
```

Generation exits 0 with 25 settled values. Their targets include object
instance 42, which is admitted, and 43, which is rejected. Replay passes every
entry against the same model.
