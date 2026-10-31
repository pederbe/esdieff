# Offline device Catalogue

The [thermostat](../examples/thermostat-check/README.md),
[RGB lamp](../examples/rgb-lamp-check/README.md),
[door lock](../examples/door-lock-check/README.md),
[metered power strip](../examples/power-strip-check/README.md),
[irrigation controller](../examples/irrigation-check/README.md) and
[coffee machine](../examples/coffee-machine-check/README.md) examples provide authored
models and conventional JSON Sample values for local development. Run the
regression checks from the repository root.

```sh
cargo test --test device_catalogue
```

The private Rust runner invokes the existing Check plan CLI. Every case has
an independent constraint or contract rationale and typed expectations for its
native state, disposition, completion and Sample admission or comparison
classification. Decisive diagnostics
also require the expected source, effective and sample pointer relations.
Every expected source acquisition remains visible, including deliberately
failed acquisitions. Missing or unexpected source identities fail the runner.
Case identities are checked once per plan. Missing, unused or duplicate
expectations, unexpected or duplicate observed cases, host failures and
malformed or unavailable reports fail the test with the device, variant and
case identified where available.

An intentionally rejected Sample retains its native failed disposition and
CLI exit 1. It passes the regression test when those results were expected.
Completion is checked separately, including the limited thermostat's known
rejection alongside unavailable calibration meaning. Small controlled records
exercise identity accounting, and altered admission, completion and diagnostic
expectations demonstrate named failures.

These are supplied descriptions and values. They establish neither physical
device behaviour nor network protocol interoperability. Each value is checked
against its selected data description. No stateful device engine or
interaction-sequence assertion is involved. The native meanings follow
[ADR 0003, Check findings and completeness](adr/0003-check-findings-and-completeness.md),
[ADR 0004, Supplied catalogues and checking scope](adr/0004-supplied-catalogues-and-checking-scope.md)
and [ADR 0016, Selected samples with explicit admission and coverage](adr/0016-selected-sample-contract.md).

## Direct Check plans

```sh
cargo run -- check --plan examples/thermostat-check/minimal-plan.json --format json
cargo run -- check --plan examples/thermostat-check/rich-plan.json
cargo run -- check --plan examples/thermostat-check/limited-plan.json --format json
cargo run -- check --plan examples/rgb-lamp-check/rich-plan.json
cargo run -- check --plan examples/door-lock-check/rich-plan.json --format json
cargo run -- check --plan examples/power-strip-check/rich-plan.json --format json
cargo run -- check --plan examples/irrigation-check/rich-plan.json --format json
cargo run -- check --plan examples/coffee-machine-check/rich-plan.json
```

The minimal and rich variants of each family complete Initial Catalogue and
Full selected-model checks. Their plans exit 1 because they contain intentional
sample rejections. The rich lock plan also compares the minimal lock with the
rich one. Both breaking changes are acknowledged, so that case passes with a
breaking classification. The limited thermostat combines an invalid bound and missing
dependency. Its model checks fail and remain incomplete. Its two undetermined
samples and independently rejected sample also remain incomplete.

The limited lamp has only unavailable calibration meaning. Its model and sample
cases are undetermined and incomplete, and its plan exits 3. The limited lock
combines an invalid numeric bound with an unsupplied credential definition.
Its plan exits 1, with failed incomplete model checks, an undetermined credential
sample and an admitted complete state sample. Model findings remain distinct
from selected-sample admission.

The power strip supplies three supported variants and one limited variant. Its
rich model imports repeated outlets and shared meter definitions from an
explicitly supplied second document. Its separate copied-requiredness plan
imports an Object that requires its own declaration through an absolute
pointer, and the copied requirement relocates to the importing outlet under
[ADR 0036, Copied reference relocation](adr/0036-copied-reference-relocation.md),
so that plan exits 0. Its missing plan records the failed acquisition of a
needed local dependency and exits 3 while preserving complete independent
Sample admissions.

The limited irrigation and coffee models need unsupplied optional calibration
meaning. Their model checks are undetermined and incomplete. Samples with
absent optional calibration are admitted completely. Present calibration keeps
applicable child work incomplete, while a missing required zone array or
excessive servings still gives an independent rejection. Both plans exit 1.

All ordinary runs are offline and use existing finite allowances. The plans
retrieve nothing and change no input bytes. Copy the complete
directory for any family elsewhere, then pass the copied plan path
to `esdieff check --plan`. All sources resolve relative to that plan. The Rust
suite repeats every device plan from a temporary copy and checks that the
original and copied bytes remain unchanged.

## Device and constraint coverage

There are 19 model variants and 116 distinct Sample cases across 20 plans.
The thermostat provides three variants and 20 cases, the lamp three variants and
16 cases, the lock three variants and 17 cases, and the power strip four
variants and 22 cases. Irrigation adds three variants and 21 cases, and coffee
adds three variants and 20 cases. The preserved thermostat
entry plan shares the minimal model. The matrix lists actual cases rather than
potential capabilities. Sample areas use the existing
`esdieff.sample/1` inventory from the [Sample contract](sample-contract.md).

| Family and variant | Named cases | Constraint or contract | Requested work |
| --- | --- | --- | --- |
| Thermostat, minimal entry | Comfortable target, Too hot target | Fractional number within bounds, above maximum | Property, sample-type-null/1 and sample-numeric/1 |
| Thermostat, minimal | Target lower boundary, Target upper boundary, Target below minimum | Inclusive exact bounds 5 and 30 | Property, sample-numeric/1 |
| Thermostat, minimal | Target wrong type, Target unavailable | String instead of number, explicit nonnullable target | Property, sample-type-null/1 |
| Thermostat, rich | Fractional current reading, Current reading unavailable | Unconstrained fractional step, nullable sensor reading | Property, sample-type-null/1 and sample-numeric/1 |
| Thermostat, rich | Target off half-degree step | Exact multiple of 0.5 | Property, sample-numeric/1 |
| Thermostat, rich | Configure heating | Required target and enumerated mode, absent optional label, exact target step | Action input, sample-map/1, sample-alternatives/1 and sample-numeric/1 |
| Thermostat, rich | Configure missing mode, Configure null mode | Absent required entry differs from present disallowed null | Action input, sample-map/1 and sample-type-null/1 |
| Thermostat, rich | Configure unknown mode, Configure wrong optional label | Rejected enum value, present optional entry with wrong type | Action input, sample-alternatives/1 and sample-type-null/1 |
| Thermostat, rich | Overheating alarm, Unknown alarm reason | Structured Event output and enumerated reason | Event output, sample-map/1 and sample-alternatives/1 |
| Thermostat, limited | Current reading needs dependency, Configure needs calibration | Needed referenced meaning is not supplied | Property and Action input, sample-context/1 |
| Thermostat, limited | Configure too hot despite missing calibration | Independent bound rejection with unfinished sibling work | Action input, sample-numeric/1, sample-map/1 and sample-context/1 |
| Thermostat, minimal and rich | Catalogue and selected-model cases named by each plan | Complete supported model descriptions | Initial Catalogue and Full selected-model |
| Thermostat, limited | Limited Catalogue, Limited model | Textual numeric maximum is invalid, calibration dependency unavailable | Initial Catalogue and Full selected-model, failed and incomplete |
| RGB lamp, minimal | Lamp on, Lamp off, Text is not on state | Boolean values and wrong-type rejection | Property, sample-type-null/1 and sample-text-boolean/1 |
| RGB lamp, minimal | Brightness lower boundary, Brightness upper boundary, Brightness above maximum | Inclusive integer bounds 0 and 100 | Property, sample-numeric/1 |
| RGB lamp, rich | RGB channel boundaries, RGB green above maximum | Required integer channels, inclusive bounds 0 and 255, invalid compound value | Property, sample-map/1 and sample-numeric/1 |
| RGB lamp, rich | Steady mode, Unknown mode | Admitted and rejected enum values | Property, sample-alternatives/1 |
| RGB lamp, rich | Dim red scene, Bright blue scene, Mixed scene has no Choice witness | One whole Choice witness, exact branch boundaries and rejection of mixed branch constraints | Action input, sample-map/1, sample-alternatives/1, sample-const/1 and sample-numeric/1 |
| RGB lamp, rich | Scene applied output, Scene refused output | Required Boolean result and const true | Action output, sample-map/1 and sample-const/1 |
| RGB lamp, limited | Brightness needs calibration | Unsupplied selected brightness description | Property, sample-context/1 |
| Door lock, minimal | Locked state, Unknown lock state | Two-state text enumeration | Property, sample-alternatives/1 |
| Door lock, minimal | PIN lower length boundary, PIN upper length boundary, PIN too short, PIN has a letter | Inclusive scalar lengths 4 and 8, anchored digit pattern | Property, sample-text-boolean/1 |
| Door lock, rich | Authorize Unicode credential, Authorize short Unicode credential, Authorize spaced credential | Unicode scalar length rather than UTF-8 bytes, anchored no-space pattern | Action input, sample-text-boolean/1 and sample-map/1 |
| Door lock, rich | Authorize missing operation, Authorize null credential | Absent required entry differs from present disallowed null | Action input, sample-map/1 and sample-type-null/1 |
| Door lock, rich | Authorization result, Authorization result wrong type | Structured result with required Boolean and enumerated state | Action output, sample-map/1, sample-type-null/1 and sample-alternatives/1 |
| Door lock, rich | Timestamped locked event, Impossible event date | Established date-time and definite calendar rejection | Event output, sample-abstract-values/1 |
| Door lock, limited | State survives unrelated model faults, Credential needs dependency | Complete selected state despite unrelated model errors, incomplete selected credential | Property, sample-alternatives/1 and sample-context/1 |
| RGB lamp and door lock, minimal and rich | Catalogue and selected-model cases named by each plan | Complete supported descriptions | Initial Catalogue and Full selected-model |
| RGB lamp, limited | Limited Catalogue, Limited model | Unsupplied calibration meaning | Initial Catalogue and Full selected-model, undetermined and incomplete |
| Door lock, limited | Limited Catalogue, Limited model | Invalid textual maximum and unsupplied credential meaning | Initial Catalogue and Full selected-model, failed and incomplete |
| Power strip, minimal | Outlet on, Outlet off, Outlet text state | Boolean outlet state and wrong-type rejection | Property, sample-type-null/1 and sample-text-boolean/1 |
| Power strip, minimal | Outlet power lower boundary, Outlet power upper boundary, Outlet power above maximum | Inclusive watt bounds 0 and 2400 | Property, sample-numeric/1 |
| Power strip, rich | Outlet fractional power | Imported repeated outlet with exact 12.345 watt reading | Property, sample-context/1 and sample-numeric/1 |
| Power strip, rich | Shared power above product bound, Total power rejects shared reading | The same 2400 reading is admitted by shared Power and rejected by its consuming override | sdfData and Property, sample-context/1 and sample-numeric/1 |
| Power strip, rich | Total power within override, Total power override boundary, Total power just above override | Explicit maximum 1800 with source/effective override relation | Property, sample-numeric/1 |
| Power strip, rich | Ordinary fractional energy, Large fractional energy, Large exact energy boundary, Energy above exact boundary | Exact values beyond ordinary machine-float precision, inclusive model maximum 9007199254740994 | Property, sample-numeric/1 |
| Power strip, rich | Energy off eighth step, Shared energy lower boundary | Rejected half-step 0.0625 and admitted zero under shared step 0.125 | Property and sdfData, sample-numeric/1 |
| Power strip, minimal and rich | Catalogue and selected-model cases named by each plan | Complete supported models, repeated outlet components, supplied reusable definitions, W and Wh declarations | Initial Catalogue and Full selected-model, including full.special-type-unit/1 registry guidance separately from numeric admission |
| Power strip, missing dependency | Missing dependency Catalogue, Missing dependency model, Total power needs dependency | Failed acquisition of an explicitly named needed local file and unavailable imported meaning | Initial Catalogue, Full selected-model and Property, incomplete model work and sample-context/1 |
| Power strip, missing dependency | Outlet state survives missing dependency | Independent complete Boolean admission despite the absent donor | Property, sample-type-null/1 and sample-text-boolean/1 |
| Power strip, copied requiredness | Copied requiredness Catalogue, Copied requiredness model | Surviving copied required pointer relocated to the importing outlet's own declaration | Initial Catalogue and Full selected-model, complete and passed |
| Power strip, copied requiredness | Copied outlet state, Independent total power | Inherited Boolean and locally authored numeric Properties | Property, sample-type-null/1, sample-text-boolean/1 and sample-numeric/1 |
| Irrigation, minimal | One zone lower count boundary, Four zones upper count boundary, No zones, Five zones | Inclusive identifier-array counts 1 and 4 and definite count failures | Property, sample-array/1 |
| Irrigation, minimal | Duplicate zone identifier, Invalid zone identifier | Whole-element text uniqueness with both sample locations, anchored zone-1 through zone-4 pattern | Property, sample-array/1 and sample-text-boolean/1 |
| Irrigation, rich | Configure without optional feedback, Configure moisture lower boundary, Configure moisture upper boundary, Configure excessive nested moisture | Recursive conjunction of arrays, zone records and optional nested moisture bounds 0 and 100 | Action input, sample-array/1, sample-map/1 and sample-numeric/1 |
| Irrigation, rich | Configure missing required zones, Configure null zones | Missing required array differs from supplied disallowed null | Action input, sample-map/1 and sample-type-null/1 |
| Irrigation, rich | Configure missing duration despite default, Configure invalid optional feedback | No insertion of a documentary default, absent optional child differs from present wrong-type child | Action input, sample-map/1 and sample-type-null/1 |
| Irrigation, rich | Configure empty zone array, Configure excessive zone array | Nested zone-record counts without projecting uniqueness onto id fields | Action input, sample-array/1 |
| Irrigation, rich | Configuration applied output, Configuration output above maximum | Structured Boolean result and bounded configured-zone count | Action output, sample-map/1, sample-text-boolean/1 and sample-numeric/1 |
| Irrigation, limited | Absent optional calibration, Present calibration needs definition, Missing zones despite incomplete calibration | Complete admission with absent optional child, undetermined needed child and independent required-entry rejection with incomplete work | Action input, sample-map/1 and sample-context/1 |
| Coffee machine, minimal | Resources lower boundary, Resources upper boundary, Water above maximum | Required water and beans plus optional milk percentages with inclusive 0 and 100 bounds | Property, sample-map/1 and sample-numeric/1 |
| Coffee machine, minimal | Missing required beans, Present null beans | Missing required entry differs from supplied disallowed null | Property, sample-map/1 and sample-type-null/1 |
| Coffee machine, minimal | Espresso preparation, Unknown preparation | Admitted and rejected preparation enum values | Property, sample-alternatives/1 |
| Coffee machine, rich | Brew espresso lower boundaries, Brew latte upper boundaries, Mixed recipe has no Choice witness | One whole nested recipe alternative with const drink and matching size bounds, no witness assembled across alternatives | Action input, sample-map/1, sample-alternatives/1, sample-const/1 and sample-numeric/1 |
| Coffee machine, rich | Recipe witness cannot admit excessive servings, Missing servings despite default | Complete child witness preserves outer conjunction, documentary default supplies no required value | Action input, sample-map/1 and sample-numeric/1 |
| Coffee machine, rich | Structured brew result, Brew result wrong type | Required Boolean result and nonempty text message | Action output, sample-map/1, sample-type-null/1 and sample-text-boolean/1 |
| Coffee machine, rich | Maintenance clean event, Unknown maintenance reason, Maintenance nested resource above maximum | Enumerated maintenance reason conjoined with nested bounded resource readings | Event output, sample-map/1, sample-alternatives/1 and sample-numeric/1 |
| Coffee machine, limited | Brew without optional calibration, Brew calibration needs definition, Excess servings despite incomplete calibration | Complete admission with absent optional child, undetermined needed child and independent bound rejection with incomplete work | Action input, sample-map/1, sample-numeric/1 and sample-context/1 |
| Irrigation and coffee machine, minimal and rich | Catalogue and selected-model cases named by each plan | Complete supported descriptions | Initial Catalogue and Full selected-model |
| Irrigation and coffee machine, limited | Limited Catalogue, Limited model | Optional model description needs an unsupplied calibration definition | Initial Catalogue and Full selected-model, undetermined and incomplete |

These families exercise the five base selectors through Property, sdfData, Action
input, Action output and Event output. Irrigation exercises Sample-array counts,
text-element uniqueness and authored zone identifier patterns. These patterns
are ordinary text constraints, rather than URI or UUID format checks. Repeated
outlet declarations are model components rather than Sample-array checks.
Nullability, presence, enum, Choice and temporal cases here do not claim
exhaustive combinations.

## Sources and adaptations

All thermostat models and samples are authored for this repository and use its
MIT or Apache-2.0 licence. No external device document, vendor data, simulator
implementation or historical SDF dialect is imported.

The minimal Catalogue reuses the existing
[`thermostat.sdf.json`](../examples/thermostat-check/thermostat.sdf.json)
without changing it. The entry `plan.json`, `comfortable.json` and `too-hot.json`
also remain unchanged. The rich variant adds a nullable current reading,
half-degree target steps, a configuration Action and an alarm Event. The limited
variant is newly authored to demonstrate a faulty bound and unsupplied
calibration definitions. Its namespace uses the reserved `example.test` domain
as an identifier and is never retrieved. The
[Count examples](../examples/catalogue-check/plan.json) remain a separate entry
workflow.

The lamp and lock adapt interface ideas from the same pinned One Data Model
playground revision used by the external-model pilot. Their
[lamp source notes](../examples/rgb-lamp-check/SOURCES.md) and
[lock source notes](../examples/door-lock-check/SOURCES.md) identify the original
documents, metadata, copyright notices and unchanged BSD licence text alongside
the newly authored constraints, models, plans and samples. Historical source
documents and their absolute required declarations are not imported into the
standalone examples.

The [power-strip source notes](../examples/power-strip-check/SOURCES.md) record
the RFC composition ideas and the pilot's pinned Binary Switch source with its
original notice and licence. The meter definitions, exact readings, product
override and distinguishing limitation variants are newly authored. No original
external corpus needs to be downloaded or rerun. Its failed-dependency source
expectations are checked explicitly, including a regression check that rejects
hidden failed or successful acquisition attempts or a fabricated acquisition success.

The [irrigation source notes](../examples/irrigation-check/SOURCES.md) identify
newly authored examples and the recorded RFC array, map and Choice rules.
The [coffee-machine source notes](../examples/coffee-machine-check/SOURCES.md)
pin Eclipse Thingweb's interface ideas and original licence notice. Coffee
resource readings and brew results are re-expressed as SDF with authored
presence constraints, recipes and maintenance data. No simulator code, protocol
forms, scheduling behaviour or upstream defaults are imported.
