# Check door-lock values

This short Rust example checks the supplied [door-lock model](../door-lock-check/rich.sdf.json)
and eight fixed values through Esdieff's public library API.

Run it from the repository root:

```sh
cargo run --example lock-demo
```

The example checks the model against the Full baseline, then checks valid and
invalid Property values, Action inputs, Action outputs and Event values. It
prints each admission alongside its expected result. Expected rejections count
as successful demonstrations of the model's constraints.

| Value | Expected result |
| --- | --- |
| State `"locked"` | Admitted |
| Unknown state `"open"` | Rejected |
| Authorization input with a valid Unicode credential | Admitted |
| Authorization input without its required operation | Rejected |
| Action output with Boolean `accepted` | Admitted |
| Action output with text in place of Boolean `accepted` | Rejected |
| Event value with a valid timestamp | Admitted |
| Event value dated 30 February | Rejected |

A successful run ends with `8 sample expectations matched` and exits 0. A failed
model check, unfinished sample check, unexpected admission or output failure
exits 1. The example takes no arguments. The old simulator's seed argument is
removed.

The example checks data against SDF descriptions. It does not invoke an Action,
change a lock, generate Events or assess device behavior. Read
[checks.rs](checks.rs) for the complete library calls. The model's
[sources and adaptations](../door-lock-check/SOURCES.md) retain its original notices.
