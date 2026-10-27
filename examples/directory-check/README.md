# Mixed model collection

Run from the repository root.

```sh
cargo run -- check --directory examples/directory-check --format json
cargo run -- check --directory examples/directory-check --recursive --exclude draft --format json
cargo run -- check --directory examples/directory-check --recursive --exclude draft --root examples/directory-check/device.sdf.json --format json
```

The first command checks `device.sdf.json` and `types.sdf.json` in that order.
The second adds `nested/status.sdf.json` between them. Both complete Initial
Catalogue checks. The third completes an Initial Selected-model check of the
device, importing Reading from `types.sdf.json` and leaving unused Status as a
candidate. Inspect the report's source inventory and requested scope.

`reading.json`, `plan.json`, `unrelated.json` and `device.sdf.json.bak` are not
model candidates. Recursing without `--exclude draft` includes the invalid
type in `draft/unfinished.sdf.json` and exits 1. Excluding a directory prunes its
entire subtree. Root paths use the invocation directory, while exclusions use
each supplied directory independently.

The plan uses literal model sources and remains independent of discovery.
All content is authored here under the repository's MIT or Apache-2.0 licence.
See [directory discovery](../../docs/directory-discovery.md) for link, overlap,
failure and finite-work rules.
