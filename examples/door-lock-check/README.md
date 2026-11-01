# Door lock

Explore lock states, example credentials, authorization data and timestamped
events through three model variants, 17 named Sample cases and one revision
comparison.

| Variant | Files | Intended behaviour |
| --- | --- | --- |
| Minimal supported | `minimal.sdf.json`, `minimal-plan.json` | Locked/unlocked enumeration and a four-to-eight-character ASCII-digit PIN |
| Rich supported | `rich.sdf.json`, `rich-plan.json`, `acknowledged-differences.json` | Unicode credentials, required authorization entries, structured results and dated state-change events, compared with the minimal model |
| Faulty and limited | `limited.sdf.json`, `limited-plan.json` | Invalid attempts maximum and unsupplied credential meaning, with independently usable state data |

From the repository root, run a plan or the focused regression test.

```sh
cargo run -- check --plan examples/door-lock-check/minimal-plan.json --format json
cargo run -- check --plan examples/door-lock-check/rich-plan.json
cargo run -- check --plan examples/door-lock-check/limited-plan.json --format json
cargo test --test device_catalogue door_lock_catalogue_matches_independent_expectations
```

Every plan exits 1 because it includes an intentional native failure. Both
supported variants complete Initial Catalogue and Full selected-model checks.
The limited variant's model checks fail and remain incomplete, while the
selected locked state is admitted completely and the credential is undetermined.
An unrelated model finding does not become a rejection of the selected state.

The rich plan also compares the minimal model with the rich one. The added
jammed state and credentials that are no longer only digits break Clients of the
minimal lock. The acknowledgement file records both as intended, so that case
passes while its classification stays breaking. Direct comparison reads the
same file.

```sh
cargo run -- diff --old examples/door-lock-check/minimal.sdf.json --new examples/door-lock-check/rich.sdf.json --acknowledged examples/door-lock-check/acknowledged-differences.json
```

The minimal PIN has inclusive length boundaries of four and eight and matches
an anchored ASCII-digit pattern. The richer credential allows any non-space
characters under the same length bounds. The four scalars in `é𝄞门a` satisfy
the lower bound. The three scalars in `é𝄞门` occupy nine UTF-8 bytes but fail
the minimum length. A present null credential differs from an absent required
operation. Event timestamps use an established ordinary `date-time` value,
alongside a rejected February 30 calendar date.

These are authored demonstration tokens and selected data values. They do not
implement authentication or describe a device-protocol envelope. Each file
contains the whole selected Property, Action input/output or Event output value.
Copy this complete directory to move its plans. All runs use local relative
paths, preserve input bytes and need no hardware or external service.

See [sources and adaptations](SOURCES.md) for the pinned One Data Model ideas
and original notices, and the [device Catalogue](../../docs/device-catalogue.md)
for independent expectations and constraint coverage. The
[lock example](../lock-demo/README.md) shows the Rust library calls for checking
the rich model and eight fixed sample values.
