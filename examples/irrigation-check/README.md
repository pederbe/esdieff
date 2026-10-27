# Irrigation controller

Explore zone identifiers, nested configuration and Action results through three
model variants and 21 named Sample cases.

| Variant | Files | Intended behaviour |
| --- | --- | --- |
| Minimal supported | `minimal.sdf.json`, `minimal-plan.json` | One to four unique zone identifiers with an anchored zone-1 through zone-4 pattern |
| Rich supported | `rich.sdf.json`, `rich-plan.json` | Required identifier and zone arrays, zone durations, optional nested moisture feedback and structured configuration output |
| Limited | `limited.sdf.json`, `limited-plan.json` | Optional calibration needs an unsupplied definition, preserving independent required-entry rejection |

From the repository root, run a plan or the focused regression test.

```sh
cargo run -- check --plan examples/irrigation-check/minimal-plan.json --format json
cargo run -- check --plan examples/irrigation-check/rich-plan.json
cargo run -- check --plan examples/irrigation-check/limited-plan.json --format json
cargo test --test device_catalogue irrigation_catalogue_matches_independent_expectations
```

Both supported variants complete Initial Catalogue and Full selected-model
checks. All three plans exit 1 because they contain intentional Sample
rejections. The limited model checks are undetermined and incomplete.
Absent optional calibration still permits complete Sample admission. Present
calibration needs unavailable meaning, leaving the sample undetermined.
Missing required zones independently reject the configuration even when present
calibration keeps other applicable work incomplete.

The identifier array checks inclusive counts 1 and 4 and whole-element
uniqueness. It does not enforce uniqueness by projecting id fields from zone
records, or compare identifiers between the two arrays. Nested zones conjoin
their required entries, duration bounds and any present moisture feedback.
Absent optional feedback differs from present invalid feedback. Missing zones
differs from supplied null. A missing required duration is rejected even though
its description carries a documentary default of 10.

Array items use explicit Full Choice alternatives for their nonnullable
identifier and zone descriptions. They do not assume wider Direct-item
semantics. The private regression expectations require decisive sample,
effective and source locations, including the pair of duplicate identifiers
and nested `/zones/0/feedback/moisture` failures.

These are authored data examples. Each file supplies a whole selected Property
or Action value. No controller state, scheduling clock or watering sequence is
simulated. Copy this complete directory to move its plans. Runs use local
relative paths and preserve all input bytes.

See [sources and adaptations](SOURCES.md) and the
[device Catalogue](../../docs/device-catalogue.md) for coverage and provenance.
