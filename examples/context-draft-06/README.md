# Context draft examples

Five unchanged JSON examples from the IETF's
[Context extension draft, revision 06](https://www.ietf.org/archive/id/draft-ietf-asdf-sdf-nonaffordance-06.html#section-5),
dated 27 October 2026, are saved here for offline checking and reuse.

| Figure | File | Context covered |
| --- | --- | --- |
| 2 | [device-with-context.sdf.json](device-with-context.sdf.json) | Serial number, installation location and calibration offset beside a temperature Property |
| 3 | [asset-container.sdf.json](asset-container.sdf.json) | Physical dimensions, load capacity and location |
| 4 | [environmental-sensor.sdf.json](environmental-sensor.sdf.json) | Floor, mounting position and indoor/outdoor placement |
| 5 | [device-metadata.sdf.json](device-metadata.sdf.json) | Identity and certification records under an sdfThing definition |
| 6 | [device-with-links.sdf.json](device-with-links.sdf.json) | Datasheet, certification and digital twin links |

Figure 2 is a complete document. The other four are the draft's definition
fragments. These are illustrative models, with no claim of a deployed product.
[Sources and licensing](SOURCES.md) record the extraction, hashes and notices.

## Check the originals

From the repository root:

```sh
cargo run -- check --plan examples/context-draft-06/check-plan.json --format json
cargo test --test context_draft_examples
```

The plan checks all five originals and compares Figure 2 with the derived
revision below. It exits 3 because general `sdfType: "link"` meaning remains
unsupported. The first four model checks complete with no errors. The four
fragments receive missing-`info` warnings, and the original `°C` spelling in
Figure 2 receives unit-registry notices. Those source details are preserved.
The regression test asserts these outcomes and exercises admitted and rejected
Context values, including nested required fields, enums and certification arrays.

## Compare a derived revision

[device-with-context-revised.sdf.json](device-with-context-revised.sdf.json) is
an Esdieff test fixture derived from Figure 2. It adds `maximum: 5` to
`calibrationOffset` and changes `info.version` to `2026-10-28`. It is not another
IETF example or evidence of an upstream model revision.

```sh
cargo run -- diff --old examples/context-draft-06/device-with-context.sdf.json --new examples/context-draft-06/device-with-context-revised.sdf.json
```

This exits 0 with one changed `sdfContext` definition and no interaction
directions. Compatibility covers client interactions only. A metadata value of
6 is admitted by the original calibration description and rejected by the
derived description, so a metadata consumer can still need changes. The
regression test checks both value admissions separately from revision comparison.

The originals leave `nullable` absent. A null Context value therefore remains
undetermined under the [recorded draft policy](../../docs/context-contract.md#semantic-boundaries).
The examples do not define runtime metadata messages or device endpoints.
