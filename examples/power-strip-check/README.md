# Metered power strip

Explore repeated outlets, Boolean state, exact meter readings and supplied
reusable definitions through four model variants and 22 named Sample cases.

| Variant | Files | Intended behaviour |
| --- | --- | --- |
| Minimal supported | `minimal.sdf.json`, `minimal-plan.json` | Two to four outlets with Boolean state and inclusive power bounds |
| Rich supported | `rich.sdf.json`, `definitions.sdf.json`, `rich-plan.json` | Imported outlets, shared Power and Energy data, an explicit power override and exact fractional energy readings |
| Missing dependency | `missing.sdf.json`, `missing-plan.json` | Failed acquisition of a named local meter file, with independently usable outlet state |
| Copied requiredness | `copied.sdf.json`, `definitions.sdf.json`, `copied-plan.json` | An imported Object's own required declaration, relocated to the importing outlet |

From the repository root, run a plan or the focused regression test.

```sh
cargo run -- check --plan examples/power-strip-check/minimal-plan.json --format json
cargo run -- check --plan examples/power-strip-check/rich-plan.json
cargo run -- check --plan examples/power-strip-check/missing-plan.json --format json
cargo run -- check --plan examples/power-strip-check/copied-plan.json --format json
cargo test --test device_catalogue power_strip_catalogue_matches_independent_expectations
```

The minimal and rich variants complete Initial Catalogue and Full selected-model
checking. Their plans exit 1 because they also contain intentional Sample
rejections. The copied plan completes every case and exits 0. The missing plan
exits 3, preserving incomplete model work and complete independent Sample
admissions. Every case remains visible.
The missing plan deliberately names `absent-meter.sdf.json`, which must remain
absent. Its failed acquisition appears alongside all successfully supplied
sources. No URI is retrieved to fill the gap.

The rich model imports `Outlet`, `Power` and `Energy` from the explicitly
supplied `definitions.sdf.json`. `minItems` and `maxItems` describe repeated
outlet components. Each Sample is a whole selected Property or `sdfData` value,
not an array of device instances or a protocol envelope. A direct `sdfData`
selection admits 2400 under the shared Power maximum 3600. The consuming
`totalPower` Property narrows that maximum to 1800, admitting 1800 and rejecting
1800.001 and the same shared 2400 reading. Its rejection points to the effective
Property bound and the authored override in `rich.sdf.json`.

Energy is expressed in watt-hours with an exact step of 0.125. The ordinary
42.125 reading and large 9007199254740993.125 reading are admitted. The
formally admissible model maximum 9007199254740994 is inclusive. Adding 0.125
to it is rejected even though an ordinary machine float would lose that
difference. The small 0.0625 reading independently fails the step constraint.
These large values demonstrate numeric handling rather than expected household
consumption.

Unit declarations describe the values. Full model checking reports the recorded
Secondary Units guidance for `Wh`, following
[ADR 0027, Unit names and recorded registry guidance](../../docs/adr/0027-unit-names-and-recorded-registry-guidance.md).
That notice is separate from numeric bounds, step membership and admission.
The example performs no conversion between `Wh`, `J` or any other unit.

The separately authored `RequiredOutlet` has an original `sdfRequired` pointer
to its own Boolean Property. When the power strip imports it as `outlet`, the
copied pointer relocates to the outlet's own Boolean Property, following
[ADR 0036, Copied reference relocation](../../docs/adr/0036-copied-reference-relocation.md).
The copied model completes, and its reports name `copied-reference-relocation/1`
among the interpretations they relied on. Checking never rewrites the pointer
in either document.

Copy this complete directory to move its plans. All runs use local relative
paths, preserve input bytes and need no hardware or external service. See
[sources and adaptations](SOURCES.md) for pinned sources and original notices,
and the [device Catalogue](../../docs/device-catalogue.md) for independent
expectations and constraint coverage.
