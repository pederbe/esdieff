# Lock demonstration

A deterministic offline replay of the rich door-lock model. It runs a short
authored command sequence against a two-state demonstration lock and checks
every state reading, Action input, Action result and Event through ordinary
Sample-value checking.

From the repository root, run the demonstration with the default seed or a
chosen one, or run its focused test target.

```sh
cargo run --example lock-demo
cargo run --example lock-demo -- 2
cargo test --test lock_demo
```

The run identifies itself as lock-demo/1 with its seed. The optional seed is an
unsigned decimal integer from 1 through 4294967295 written in ASCII digits,
and it defaults to 1. Leading zeros are accepted, and the output always names
the seed without them. A sign, spaces, any other characters, an out-of-range
value or a second argument is refused before simulation.
It first requires the unchanged [rich model](../door-lock-check/rich.sdf.json)
to pass its Initial Catalogue check and Full Selected-model check under strict
warnings. The lock then starts locked, with one state reading at the virtual
epoch 2024-01-01T12:00:00Z. Each command step advances the simulated clock by
one whole second, including refused and malformed requests. No host clock,
randomness, network access or file output affects a run.

| Step | Action input | Intended behaviour |
| --- | --- | --- |
| 1 | demo, unlock | Accepted and unlocked, with an Event |
| 2 | demo, unlock | Accepted while already unlocked, so no Event |
| 3 | nope, lock | Admitted but refused, still unlocked |
| 4 | abc, lock | Rejected for its length and never invoked |
| 5 | null credential, lock | Rejected for nullability and never invoked |
| 6 | demo without an operation | Rejected for the missing required entry and never invoked |
| 7 | demo, lock | Accepted and locked, with an Event |
| 8 | demo, lock | Accepted while already locked, so no Event |
| 9 | demo, unlock | Accepted and unlocked, with an Event |

Eight commands generated from the seed follow as steps 10 through 17, so every
run has exactly 17 command steps. The generator is xorshift32 on an unsigned
32-bit state that starts at the seed. Each generated command advances it once
with `x ^= x << 13`, then `x ^= x >> 17`, then `x ^= x << 5`, keeping the low
32 bits. Bit 0 of the result selects lock for 0 and unlock for 1, and bit 1
selects demo for 0 and nope for 1. The literal prefix consumes no state and no
other values are drawn. This is a reproducible command source rather than a
statistical or security generator. With the default seed 1, steps 10 through
17 are demo unlock three times, nope unlock, demo unlock, demo lock, then nope
lock twice. Seed 2 starts with a lock using nope.

## lock-demo/1 behavior

These rules belong to this demonstration alone. They are not read from the SDF
model, its descriptions or any vendor behavior, and they add nothing to any
Model requirement inventory. Native admission decides whether a request is invoked,
but it never sets an expectation or grades a transition. An independent oracle
judges the observed trace against the authored commands, tracking its own
expected state from the authored initial state.

| Rule | What it asserts |
| --- | --- |
| step accounting | Each authored command has exactly one step, in command order, whose Action input means that command, and no step lacks a command |
| step timing | The initial reading is at the epoch, step n is n whole seconds later, and each Event occurs at its step's instant |
| interaction order | A step holds one Action input first, then any Action output, then any Event, then one reading last |
| rejected input | A malformed input is never invoked, has no output or Event and leaves the reading unchanged |
| refused request | A well-formed nope request is invoked and answers accepted false with the unchanged state, without an Event |
| accepted request | A demo request answers accepted true with the requested state, emits one stateChanged Event only when the state changes and ends with that state |
| state continuity | Readings start from the authored initial state and change only through a stateChanged Event in the same step, so nothing changes between steps |

The authored admission of each command comes from its literal form. The three
malformed prefix inputs are expected rejected, and well-formed demo and nope
requests are expected admitted. The model's jammed state remains valid data but
is never produced.

## Output and exit status

The output shows the Simulation trace with each value's native admission. It
ends with two separate summaries. Native sample expectations compare each
value's admission, disposition, completion, requested inventory and decisive
diagnostic with independently written expectations, and expected rejections
count as both rejected and matched. Authored behavior assertions report the
literal comparison with the step table and each named rule on its own line,
with every mismatch naming its step, rule and the expected and observed values.
The run exits 0 only when the model checks and both summaries match. Any
mismatch exits 1, an invalid seed or extra argument exits 2 before simulating
and an output failure exits 4.

## Bounds and fidelity

Each run makes at most 60 Sample-value checking requests after its two Model
checks, limits each sample value to 512 UTF-8 bytes and keeps at most 64 KiB of
trace. The output's finite work line shows what a run used. Exhausted
allowances stop the run rather than being raised.

The demonstration does not model firmware, authentication or a device
protocol. The tokens demo and nope are synthetic, and a passing run establishes
only these expectations for this model. The model's origins and the retained
licence notice are described in the door-lock
[sources and adaptations](../door-lock-check/SOURCES.md) and
[licence](../door-lock-check/LICENSE-BSD-3-Clause).
