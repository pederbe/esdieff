# Room thermostat

Run this directory's Check plans to explore three authored SDF model variants
and 20 Sample cases without hardware.

| Variant | Model and plan | Intended behaviour |
| --- | --- | --- |
| Minimal supported | `thermostat.sdf.json`, `plan.json` and `minimal-plan.json` | Readable and writable target from 5 through 30 degrees Celsius, with ordinary, boundary, wrong-type and nonnullable samples |
| Rich supported | `rich.sdf.json` and `rich-plan.json` | Nullable current reading, half-degree target steps, required configuration entries, optional label and structured alarm output |
| Faulty and limited | `limited.sdf.json` and `limited-plan.json` | Invalid textual numeric maximum, missing calibration definitions, undetermined samples and independent rejection beside incomplete work |

From the repository root, run any plan directly, or run all regression checks.

```sh
cargo run -- check --plan examples/thermostat-check/plan.json
cargo run -- check --plan examples/thermostat-check/minimal-plan.json --format json
cargo run -- check --plan examples/thermostat-check/rich-plan.json
cargo run -- check --plan examples/thermostat-check/limited-plan.json --format json
cargo test --test device_catalogue
```

The plans intentionally exit 1. A known rejected value is a native failed case,
even when the regression test expects it. Admission, disposition and completion
remain separate. Both supported variants complete their Initial Catalogue and
Full selected-model checks.

Each Sample file contains the whole value selected by its Property, Action
input or Event output description. The files do not encode a protocol message
or assert how a physical thermostat behaves. Copy this entire directory to
move the examples, since source paths resolve beside the supplied plan.

All content is authored here under the repository's MIT or Apache-2.0 licence.
The original entry files remain unchanged. See the
[device Catalogue](../../docs/device-catalogue.md) for independent expectations,
actual constraint coverage and source adaptations, and the
[Check plan contract](../../docs/check-plans.md) for plan fields and native results.
