# Acquire once and replay offline

Run from the repository root. This example selects one historical public OneDM
file using the independently recorded [source identities](SOURCES.md).

```sh
mkdir examples/pinned-acquisition/acquired
cargo run -- acquire --github one-data-model/playground --commit 41be1e0e564ad39f4d5a1c41fcccef435e388ee9 --path sdfObject/sdfobject-switch_binary.sdf.json --sha256 6fe5a1538da0b5f48898e367f0f03b6b70fa38b6ebaa67117e408ab87fb0b61a --document-output examples/pinned-acquisition/acquired/switch.sdf.json --format json
```

The JSON Acquisition receipt goes to stdout. Success exits 0 with schema
`esdieff.acquisition`, version `1.0`, status `acquired`, both `complete` and
`document_committed` true, no failures, the selected commit and path, a byte
count of `787` and the expected SHA-256 as `actual_sha256`. Its `source.uri` is
`https://raw.githubusercontent.com/one-data-model/playground/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-switch_binary.sdf.json`.
It is separate from a Model checking Report. The [acquisition contract](../../docs/acquisition-contract.md)
describes failure phases and finite transfer limits.

Compare against retained original evidence after the transfer

```sh
cmp examples/pinned-acquisition/original.sdf.json examples/pinned-acquisition/acquired/switch.sdf.json
sha256sum examples/pinned-acquisition/acquired/switch.sdf.json
git hash-object examples/pinned-acquisition/acquired/switch.sdf.json
wc -c examples/pinned-acquisition/acquired/switch.sdf.json
```

The outputs reproduce SHA-256
`6fe5a1538da0b5f48898e367f0f03b6b70fa38b6ebaa67117e408ab87fb0b61a`, Git
blob `6261359649925ca90cc20380106f560f6158f948` and `787` bytes. These checks
observe live transport to this one pinned file. An unavailable source leaves
that live observation unavailable, even when the retained original can be
replayed offline.

## Ordinary local operations

The following commands use only local inputs. The command accepts the acquired
file as an ordinary Catalogue contributor

```sh
cargo run -- check examples/pinned-acquisition/acquired/switch.sdf.json --format json
cargo run -- inspect --view source --document examples/pinned-acquisition/acquired/switch.sdf.json --pointer /sdfObject/switch.binary examples/pinned-acquisition/acquired/switch.sdf.json --format json
cargo run -- edit --source switch examples/pinned-acquisition/acquired/switch.sdf.json --plan examples/pinned-acquisition/edit-plan.json --document-output examples/pinned-acquisition/acquired/revised.sdf.json --format json
cargo run -- check --plan examples/pinned-acquisition/check-plan.json --format json
```

The edit replaces the Property's description in a separate revised file and
preserves the original snapshot. The literal version-1 Check plan names that
original file and supplies Boolean `true` as the `On` sample. Its paths resolve
beside the plan. Repeating the plan uses the same bytes with no receipt loading,
network retrieval or new interpreter. Ordinary CLI reports record the local
file acquisition path. Model findings and requested-work completeness remain
independent of successful acquisition.

The SDF namespace `https://onedm.org/ecosystem/ocf` is separate from the receipt's
retrieval URI. Acquisition selects no Root document and retrieves no reference
dependencies. Explicit repeated or overlapping contributors retain ordinary
ambiguity, even when their bytes match.

## Explicit library metadata and refresh

The runnable [library example](../acquired_source.rs) reads this local snapshot,
checks its independently pinned digest and explicitly supplies the URI, commit
and digest through existing `SourceInput` metadata

```sh
cargo run --example acquired_source -- examples/pinned-acquisition/acquired/switch.sdf.json
```

`SourceInput::owned(...).acquisition_uri(uri).version_token(commit).digest(digest)`
records caller-supplied metadata. It creates an ordinary supplied Source and
does not open the URI, verify a digest itself or interpret an Acquisition
receipt. The example performs its digest check before constructing the Source.

A refresh is a new explicit acquisition. Review the new complete commit and
raw-file digest separately, then use those values with a different
`--document-output`, such as `acquired/switch-refresh.sdf.json`. Preserve the old
`switch.sdf.json` and its literal plan for identical offline replay. The command
refuses an existing destination and supplies no replacement option.
