# Check a local model collection

Direct `check` can select model files from explicitly supplied directories.
Run the [mixed collection](../examples/directory-check/README.md) from the
repository root.

```sh
cargo run -- check --directory examples/directory-check --format json
cargo run -- check --directory examples/directory-check --recursive --exclude draft --format json
cargo run -- check --directory examples/directory-check --recursive --exclude draft --root examples/directory-check/device.sdf.json --format json
```

The shallow command selects `device.sdf.json` and `types.sdf.json`. Recursion
with the exclusion adds `nested/status.sdf.json`. The selected-root command
checks the device and its needed shared type, while the nested document remains
a supplied candidate. Each command emits one ordinary model report. Sample JSON,
Check plans, unrelated JSON and the suffix lookalike are not selected. Recursing
without the exclusion includes the deliberately invalid draft and exits 1.

Repeat `--directory DIR` to select several collections. Positional files and
one `-` stdin document can accompany discovery. Explicit files need no naming
suffix. Their order is preserved, followed by each directory's selection in
option order. Within each directory, accepted relative paths are sorted by exact
native encoded path bytes. Ordering is repeatable for unchanged arguments and
tree on the same platform. It gives no reference-resolution precedence.

Selection matches regular files whose final filename ends exactly in the
case-sensitive suffix `.sdf.json`. Without `--recursive`, only immediate entries
are inspected. Matching hidden files and recursive hidden directories follow
the same rules. There are no implicit exclusions or content sniffing.

Repeat `--exclude RELATIVE_PATH` to exclude a literal file or an entire subtree.
The same exclusions apply independently relative to each supplied directory,
and a subtree is pruned before type inspection or descent. An exclusion must be
a nonempty relative component path without a root, `.` or `..` component.
Matching is case-sensitive on native components. There is no globbing, variable
expansion, regular-expression or ignore-file interpretation. A nonexistent
exclusion is harmless. Explicit file inputs are unaffected.

Directories, explicit files and `--root` paths resolve from the invocation
directory. A discovered Root document must be named by its joined acquisition
path, such as `models/device.sdf.json` for `--directory models`. Roots are never
chosen automatically. Missing or multiply matched roots are invocation errors.
Catalogue scope checks every supplied document, including unused definitions.
Selected-model scope checks explicit roots and the dependencies needed for their
checks, following [ADR 0004, Supplied catalogues and checking scope](adr/0004-supplied-catalogues-and-checking-scope.md).

An explicitly supplied directory may resolve through a link. Its displayed
acquisition paths retain that spelling. Descendant file and directory links,
including platform link equivalents, and known special files are skipped.
Native paths select and open files even when a display label is omitted or uses
Unicode replacement.

Each supplied directory must select at least one matching file after exclusions.
Discovery refuses native-file overlap with another discovered or explicit file,
including aliases and hard links. Exclude the automatic duplicate or remove the
overlapping input to select one attempt explicitly. Explicitly repeated file
inputs remain distinct attempts. Physically different files with equal bytes
remain distinct sources and can retain ordinary namespace ambiguity.

Preparation has fixed ceilings, following
[ADR 0009, Bounded processing and reporting](adr/0009-bounded-processing-and-reporting.md).

| Preparation allowance | Ceiling |
| --- | --- |
| Directory requests | 256 |
| Literal exclusions | 256 |
| Observed entries across all requests, including ignored and repeated work | 10,000 |
| Recursive directory depth below each explicit root | 64 |
| Assembled source attempts, including positional files and stdin | 256 |
| Retained discovery path and option metadata, including repeated representations | 1 MiB |

An included subtree beyond a ceiling fails preparation. Excluded subtrees are
not enumerated. The ordinary Catalogue acquisition, parse, semantic-work and
report budgets still apply separately. There are no discovery tuning flags or
unlimited mode.

All option validation, enumeration, overlap checks, root selection and assembled
metadata preflight finish before model content or stdin is read. Missing,
wrong-kind, unreadable or empty directories, inspection failures, overlaps,
invalid roots and exhausted allowances exit 2 with bounded stderr and empty
stdout, including under `--format json`. They produce no partial Catalogue
report and assert no RFC violation.

After successful preparation, a selected file that cannot be read or decoded
remains an ordinary source result in the report. Failed and incomplete checks,
warning policy, coverage, diagnostic omissions, color and output-failure exit 4
retain their existing [report contract](report-contract.md). Directory discovery
is a bounded filesystem observation rather than an atomic snapshot. It changes
no original bytes and promises no cross-file content consistency.

`--recursive` and `--exclude` require a directory input. Discovery options apply
only to direct `check` and `diff`. Saved [Check plan version 1](check-plans.md),
`inspect`, `sample`, `author` and `edit` do not accept them. Discovery adds no Rust
filesystem API, report schema, assessment inventory or standards interpretation.

## Compare two collections

`esdieff diff --old DIR --new DIR` prepares each Model revision's directory with
these rules. `--recursive` and `--exclude` apply to both sides alike. Each side
is prepared separately, with its own ceilings and overlap check, so a directory
can be compared with itself. Both sides finish preparation before either one's
content is read.

A document's source key is its path relative to its directory, with components
joined by `/`, such as `nested/status.sdf.json`. Documents with the same key
correspond, so a moved or renamed document reads as a removal and an addition.
A relative path that is not UTF-8 or exceeds the 256-byte source key cannot pair
documents and is an invocation error. The
[revision comparison contract](revision-comparison-contract.md) describes the
comparison and its report.
