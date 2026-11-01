# Profiling

Esdieff can be profiled with [hotpath-rs](https://hotpath.rs/) to see
where time and allocations go. Profiling is off by default. Ordinary
builds, tests and the published library neither depend on hotpath nor compile
any instrumentation. The `hotpath` feature turns on timing, and
`hotpath-alloc` adds allocation tracking.

## Run the workload

Run the profiling workload from the repository root.

```sh
cargo run --release --example profiling_workload --features hotpath
cargo run --release --example profiling_workload --features hotpath-alloc
cargo run --release --example profiling_workload
```

The first command prints the workload's totals and timing, followed by a
hotpath report of calls, average, 95th percentile and total time for each
instrumented operation. The second adds the bytes each operation allocates
itself, excluding nested instrumented calls. The third builds without
instrumentation. Use it for before-and-after timing comparisons, since it
carries no profiling cost.

The workload runs 200 rounds by default. Pass another count after `--`, such as
`-- 1000`. Every round does the same work, and the totals line repeats exactly
for unchanged inputs and code. A changed totals line after an optimization
means the change altered results, not just speed.

## What the workload covers

Each round runs all 20 Check plans of the
[offline device Catalogue](device-catalogue.md) through the library, as
`check --plan` would. For each plan it acquires and parses the plan's sources
from their files, composes an effective view of each model document, and runs
every Model checking, Sample-value checking and Revision comparison case. The
rich door lock plan's one comparison acquires the minimal and rich models
apart from the plan's sources, as
`diff --old examples/door-lock-check/minimal.sdf.json --new examples/door-lock-check/rich.sdf.json`
would.

Each round then checks a Recorded trace of 1,000 thermostat readings against
the rich thermostat model, as
`trace --document examples/thermostat-check/rich.sdf.json` would. Every
hundredth reading is too hot, so 10 entries fail. A second Recorded trace of
1,000 door lock credentials goes through the credential's length and pattern
checks, as `trace --document examples/door-lock-check/rich.sdf.json` would.
Every hundredth credential holds a space, which the pattern rejects, so 10
more entries fail. Finally the round generates values for every affordance description of
the rich door lock model, as
`generate --document examples/door-lock-check/rich.sdf.json` would.

The previously documented totals were 138 sources, 23 effective views, 40 model checks,
116 sample checks, one comparison, 2,000 trace entries and 83 generated values per
round. These are historical observations. Use the totals printed by your run for
the current workload and code. Report rendering is not part of the workload.

## Instrumented operations

| Report name | Operation |
| --- | --- |
| `source::build` | Acquisition and parsing of a Catalogue's sources |
| `json::parse_mode` | Decoding one source's JSON, within acquisition |
| `effective::compose_mode` | Composition of one document into its effective view, for any operation |
| `report::check_with_budget` | Model checking |
| `report::check_sample` | Sample-value checking |
| `comparison::compare` | Revision comparison |
| `trace::check_trace` | Recorded trace checking |
| `generation::generate` | Generation of test values |
| `pattern::compile` | Qualified guards and Corvus compilation for syntax or matching |
| `pattern::matches` | One Corvus match, including compilation |

Times include nested calls. Model checking, Sample-value checking, Revision
comparison and Recorded trace checking each compose the documents they need,
so composition time appears both under `effective::compose_mode` and under the
operation that asked for it. A Recorded trace check composes its document
once, however many entries it holds, so each trace adds one
`effective::compose_mode` call per round. Pattern operations run directly through Corvus. There is no toolkit pattern
worker or JavaScript engine-start span. Each match compiles its expression with
the fixed internal timeout. Corvus may reuse thread-local scratch according to
its own capacity policy. The `pattern::compile` span also includes compile-only
Model checking. Other programs built with the feature record
nothing, because only the workload starts the profiler.

## Live view and JSON reports

The hotpath CLI shows the same measurements live. Install the matching version
with `cargo install hotpath --version 0.30.3 --features tui`. Start
`hotpath console` in one terminal, then run the workload with the `hotpath`
feature and enough rounds to watch, such as `-- 20000`, in another.

For a machine-readable report, set `HOTPATH_OUTPUT_FORMAT=json` and
`HOTPATH_OUTPUT_PATH` to a file before running the workload. The
[configuration reference](https://hotpath.rs/configuration) lists the other
settings.

## Pattern migration workload

Run `cargo run --profile dist --example pattern_qualification_consumers` from
the repository root to check all six rich device families, Context reuse and
independently expected 1,000/100,000-entry credential and lookbehind traces.
Optional positional arguments name pinned public model files, including the
OneDM Door and Operational state models at playground revision
`41be1e0e564ad39f4d5a1c41fcccef435e388ee9`. Each model is checked, its settled
generated values are checked individually, and their trace replay must pass.
Generation/replay consistency is separate from the independently authored
pattern trace expectations.

Build without features for ordinary timing and whole-process CPU/RSS captures.
Add `--features hotpath-alloc` for diagnostic allocation traffic, and set
`HOTPATH_OUTPUT_FORMAT=json` with `HOTPATH_OUTPUT_PATH` to retain the report.
Allocation traffic, peak memory and internal safeguard counters have different
meanings. The fixed 100 ms pattern timeout is cooperative, so the workload does
not establish a hard deadline or exact memory ceiling.

The [migration receipt](profiling/68/receipt.txt) retains the ordinary paired
counts, process observations and separate allocation capture. Its measurements
support only the named workload and recorded host conditions.
