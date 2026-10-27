# Profiling

Esdieff can be profiled with [hotpath-rs](https://hotpath.rs/) 0.28.6 to see
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
would. Each round then checks a Recorded trace of 1,000 thermostat readings
against the rich thermostat model, as
`trace --document examples/thermostat-check/rich.sdf.json` would. Every
hundredth reading is too hot, so 10 entries fail. One round covers 138
sources, 23 effective views, 40 model checks, 116 sample checks, one
comparison and 1,000 trace entries. Report rendering is not part of the
workload.

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

Times include nested calls. Model checking, Sample-value checking, Revision
comparison and Recorded trace checking each compose the documents they need,
so composition time appears both under `effective::compose_mode` and under the
operation that asked for it. A Recorded trace check composes its document
once, however many entries it holds, so the trace adds one
`effective::compose_mode` call per round. Allocation figures count each
operation's own allocations, so composition allocations appear only under
`effective::compose_mode`. Other programs built with the feature record
nothing, because only the workload starts the profiler.

## Live view and JSON reports

The hotpath CLI shows the same measurements live. Install the matching version
with `cargo install hotpath --version 0.28.6 --features tui`. Start
`hotpath console` in one terminal, then run the workload with the `hotpath`
feature and enough rounds to watch, such as `-- 20000`, in another.

For a machine-readable report, set `HOTPATH_OUTPUT_FORMAT=json` and
`HOTPATH_OUTPUT_PATH` to a file before running the workload. The
[configuration reference](https://hotpath.rs/configuration) lists the other
settings.
