# Coffee machine

Explore resource percentages, preparation choices, brew data and maintenance
outputs through three model variants and 20 named Sample cases.

| Variant | Files | Intended behaviour |
| --- | --- | --- |
| Minimal supported | `minimal.sdf.json`, `minimal-plan.json` | Required water and beans percentages, optional milk and an espresso/latte enumeration |
| Rich supported | `rich.sdf.json`, `rich-plan.json` | Nested recipe Choice, bounded servings, structured brew output and maintenance Event output |
| Limited | `limited.sdf.json`, `limited-plan.json` | Optional calibration needs an unsupplied definition, preserving independent servings rejection |

From the repository root, run a plan or the focused regression test.

```sh
cargo run -- check --plan examples/coffee-machine-check/minimal-plan.json --format json
cargo run -- check --plan examples/coffee-machine-check/rich-plan.json
cargo run -- check --plan examples/coffee-machine-check/limited-plan.json --format json
cargo test --test device_catalogue coffee_machine_catalogue_matches_independent_expectations
```

Both supported variants complete Initial Catalogue and Full selected-model
checks. All three plans exit 1 because they contain intentional Sample
rejections. The limited model checks are undetermined and incomplete.
Brew input with absent optional calibration is admitted completely. Present
calibration requires unavailable child meaning. Excessive servings independently
reject a sample whose present calibration remains incomplete.

Resource readings exercise inclusive percentage endpoints 0 and 100, an
excessive reading, missing required beans and supplied null beans. A complete
espresso recipe matches the espresso literal and size 25 through 60. A complete
latte recipe matches latte and size 150 through 300. Espresso with size 300 has
no whole Choice witness. A valid latte child cannot admit outer servings 6,
and documentary default 1 does not supply missing required servings.

Brew output requires a Boolean result and nonempty message. Maintenance output
conjoins an enumerated reason with nested resource readings. A valid reason
cannot erase an invalid nested water percentage. Decisive failures retain
sample, effective and source pointer relations in the private expectations.

These are selected data values inspired by a coffee-machine interface. They do
not simulate brewing, resource consumption, maintenance schedules or interaction
sequences. Copy this complete directory to move its plans. Runs use local
relative paths and preserve input bytes without contacting external services.

See [sources and adaptations](SOURCES.md) for the pinned interface ideas and
licence evidence, and the [device Catalogue](../../docs/device-catalogue.md)
for independent expectations and constraint coverage.
