# RGB lamp

Explore Boolean state, brightness, RGB channels and scene controls through three
model variants and 16 named Sample cases.

| Variant | Files | Intended behaviour |
| --- | --- | --- |
| Minimal supported | `minimal.sdf.json`, `minimal-plan.json` | Boolean on/off and inclusive integer brightness from 0 through 100 |
| Rich supported | `rich.sdf.json`, `rich-plan.json` | Required RGB channels from 0 through 255, enumerated modes, whole scene Choice witnesses and structured Action results |
| Limited | `limited.sdf.json`, `limited-plan.json` | Brightness requires an intentionally unsupplied calibration definition |

From the repository root, run a plan or the focused regression test.

```sh
cargo run -- check --plan examples/rgb-lamp-check/minimal-plan.json --format json
cargo run -- check --plan examples/rgb-lamp-check/rich-plan.json
cargo run -- check --plan examples/rgb-lamp-check/limited-plan.json --format json
cargo test --test device_catalogue rgb_lamp_catalogue_matches_independent_expectations
```

The supported variants complete Initial Catalogue and Full selected-model
checks. Their plans exit 1 because they include deliberately rejected samples.
The limited plan exits 3, with undetermined disposition and incomplete work
for both model checks and its brightness sample.

The scene input needs one whole matching Choice alternative. A red preset with
brightness 20 satisfies dimRed, and a blue preset with brightness 80 satisfies
brightBlue. Red with brightness 80 fails both branches. Taking the preset from
one branch and the brightness from another cannot admit it. The Action output
requires `applied` to equal the Boolean constant true.

Sample files contain the whole selected Property or Action data value. They
describe conventional JSON values, without a device-protocol envelope or a
claim about physical lamp behaviour. Copy this complete directory to run its
plans elsewhere. All inputs resolve beside the plan and checking changes no
input bytes.

See [sources and adaptations](SOURCES.md) for the pinned One Data Model ideas
and original notices, and the [device Catalogue](../../docs/device-catalogue.md)
for independent expectations and constraint coverage.
