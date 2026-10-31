# Inspect and check a model

This guide checks a model that reuses a shared definition, reads the findings and looks at what the composed interface contains. It assumes Esdieff is [installed](../getting-started/README.md) and uses the `thermostat.sdf.json` from that page, plus two small files shown below.

## Supply the files a model needs

An SDF model can reference definitions in other documents. Esdieff resolves such references only among the files you supply on the command line, so a check sees exactly what you give it and never fetches anything. Together, the supplied files are the **catalogue**.

Save these two files from the repository's [directory-check example](https://github.com/pederbe/esdieff/blob/main/examples/directory-check) next to each other. The sensor's `reading` property reuses a `Reading` type from the second file through its namespace `types`:

```json
{
  "info": { "title": "Directory sensor" },
  "namespace": { "types": "https://example.com/directory/types" },
  "sdfObject": {
    "Sensor": {
      "sdfProperty": {
        "reading": { "sdfRef": "types:#/sdfData/Reading" }
      }
    }
  }
}
```

```json
{
  "info": { "title": "Shared reading type" },
  "namespace": { "types": "https://example.com/directory/types" },
  "defaultNamespace": "types",
  "sdfData": { "Reading": { "type": "number", "minimum": 0 } }
}
```

Checking the device on its own cannot finish, because the referenced definition is not in the catalogue:

<pre class="terminal"><code><span class="p">$</span> esdieff check device.sdf.json
<b>Esdieff:</b> <span class="y">undetermined</span>
<b>Complete:</b> <span class="y">false</span>
<b>Errors:</b> 0; <b>warnings:</b> 0; <b>notices:</b> 0; <b>incomplete checks:</b> 6
<b>Scope:</b> catalogue
...
<span class="y">reference.unavailable</span>: supplied global target unavailable
effective 1 "/sdfObject/Sensor/sdfProperty/reading"
  related source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
  related source 1 "/sdfObject/Sensor/sdfProperty/reading"
<span class="y">coverage.unavailable</span>: Requested assessment area has unfinished applicable work
...
</code></pre>

The verdict is `undetermined`, not `failed`. Esdieff does not treat a missing dependency as a mistake in the model, and it never reports a definition as valid when it could not establish what it means. Supply both files, and name the device as the **root** so the check covers the device and whatever it needs:

<pre class="terminal"><code><span class="p">$</span> esdieff check --root device.sdf.json device.sdf.json types.sdf.json
<b>Esdieff:</b> <span class="g">passed</span>
<b>Complete:</b> true
<b>Errors:</b> 0; <b>warnings:</b> 0; <b>notices:</b> 0; <b>incomplete checks:</b> 0
<b>Scope:</b> selected
<b>Broader RFC coverage:</b> partial
<b>Requested assessment:</b> esdieff.initial/1
<b>Baseline:</b> rfc9880/base-1 (RFC 9880)
<b>Acceptance restrictions:</b> A001, A002; <b>strict warnings:</b> false
<b>Requested sources:</b> [1]
<b>Requested inventory:</b> esdieff.initial/1
<b>Requested coverage (assessment_area):</b> total 8; assessed 7; not applicable 1; unassessed 0; undetermined 0
<b>Source</b> 1 ["input1"]: acquired / usable (247 retained bytes, content complete: true)
<b>Source</b> 2 ["input2"]: acquired / usable (211 retained bytes, content complete: true)
<b>Dependency sources:</b> [2]
<b>Diagnostic details omitted:</b> 0
</code></pre>

`Scope: selected` with `Dependency sources: [2]` records that the types file was used as a dependency of the root. Without `--root`, the scope is `catalogue` and every supplied file is checked in full, including definitions nobody uses. Both scopes are useful: catalogue scope for a collection you maintain, selected scope for one product and its dependencies.

## Choose how much to check

`--assessment` selects an assessment level. The default `initial-v1` covers the document envelope, names and roles, namespaces, composition, parameters and the other fundamentals in eight assessment areas. `full-baseline` requests all 13 areas and accounts for all 54 model requirements Esdieff records from RFC 9880, including unit names against a recorded registry snapshot and numeric contradictions. The `expanded-*` levels add one concern to the initial set, such as `expanded-pattern-syntax-v1` for regular expressions. Two of them offer advice that `full-baseline` does not include. `expanded-numeric-v1` finds number ranges that no integer or step value can satisfy, and `expanded-consistency-v1` finds a `const` or `default` value that its own description excludes.

```sh
esdieff check --assessment full-baseline --root device.sdf.json device.sdf.json types.sdf.json
```

Every report says which inventory it assessed and how many areas were assessed, not applicable or undetermined, so a passed check always states what it covered. `--strict-warnings` turns warnings into a failed verdict, which is the setting to use in CI once a collection is clean. The [model meaning contract](../model-meaning-contract.md) and the [assessment inventory](../model-assessment-inventory.md) describe each area.

## Read findings and locations

A finding has a condition code, a message and a location. The example collection includes a draft with a broken type:

<pre class="terminal"><code><span class="p">$</span> esdieff check draft/unfinished.sdf.json
<b>Esdieff:</b> <span class="r">failed</span>
<b>Complete:</b> true
<b>Errors:</b> 1; <b>warnings:</b> 0; <b>notices:</b> 0; <b>incomplete checks:</b> 0
...
<span class="r">data.string</span>: A textual data quality must contain a string
source 1 "/sdfData/Unfinished/type"
<b>Diagnostic details omitted:</b> 0
</code></pre>

`data.string` is the stable code of the rule, and the message explains it. The location is a **source** location: source 1 is the first supplied file, and the JSON Pointer `/sdfData/Unfinished/type` is the member in that file. Findings come in four kinds. Errors fail the check, warnings and notices are advice that leave it passed unless `--strict-warnings` is set, and incomplete checks mark work Esdieff could not finish, which makes the verdict undetermined.

Some findings are about the composed interface rather than one file. The `reference.unavailable` finding earlier had an **effective** location, `/sdfObject/Sensor/sdfProperty/reading`, which is the property as the device presents it after references and overrides are applied. Its related source locations point at the `sdfRef` that caused it. The [report contract](../report-contract.md#numbers-and-locations) describes both location kinds.

## Inspect what the interface contains

`esdieff inspect` shows one definition in either view. The `--view source` form prints the member as written in the file, found by its JSON Pointer. The `--view effective` form prints the definition the device presents, with inherited qualities applied and RFC defaults filled in, and then lists where each quality came from. Inspect the thermostat's target temperature:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view effective --document thermostat.sdf.json --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature thermostat.sdf.json
<b>Esdieff:</b> <span class="g">passed</span>
...
<b>Inspection</b> effective at "/sdfObject/Thermostat/sdfProperty/targetTemperature":
{"description":"Requested room temperature in half-degree increments","type":"number","nullable":false,"minimum":5,"maximum":30,"multipleOf":0.5,"unit":"Cel","readable":true,"writable":true}
<b>Alternatives</b> absent at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature"
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature"
default true at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/observable"
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/description"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/description"
...
</code></pre>

The report before the inspection is trimmed. Every quality of this property is authored in the file itself, and `observable` is absent in the source, so the view records that its RFC default of `true` applies.

The sensor's `reading` is more interesting, because its content comes from the other file:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view effective --document device.sdf.json --pointer /sdfObject/Sensor/sdfProperty/reading device.sdf.json types.sdf.json
...
<b>Inspection</b> effective at "/sdfObject/Sensor/sdfProperty/reading":
{"type":"number","minimum":0}
<b>Alternatives</b> absent at effective 1 "/sdfObject/Sensor/sdfProperty/reading"
inherited at effective 1 "/sdfObject/Sensor/sdfProperty/reading"; source 2 "/sdfData/Reading"; source 1 "/sdfObject/Sensor/sdfProperty/reading"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/readable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/writable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/observable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/nullable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
inherited at effective 1 "/sdfObject/Sensor/sdfProperty/reading/type"; source 2 "/sdfData/Reading/type"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
inherited at effective 1 "/sdfObject/Sensor/sdfProperty/reading/minimum"; source 2 "/sdfData/Reading/minimum"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef"
</code></pre>

Each `inherited` line names the file and pointer the quality came from, and `imported at` names the `sdfRef` that brought it in. This is how you see whether a supplier's property really has the bound you expect and which shared definition sets it.

The [catalogue-check example](https://github.com/pederbe/esdieff/blob/main/examples/catalogue-check) works the same way for a reusable `sdfData` definition:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view effective --document count.sdf.json --pointer /sdfData/Count count.sdf.json
...
<b>Inspection</b> effective at "/sdfData/Count":
{"type":"integer","minimum":0,"maximum":100}
<b>Alternatives</b> absent at effective 1 "/sdfData/Count"
source at effective 1 "/sdfData/Count"; source 1 "/sdfData/Count"
default true at effective 1 "/sdfData/Count/nullable"
source at effective 1 "/sdfData/Count/type"; source 1 "/sdfData/Count/type"
source at effective 1 "/sdfData/Count/minimum"; source 1 "/sdfData/Count/minimum"
source at effective 1 "/sdfData/Count/maximum"; source 1 "/sdfData/Count/maximum"
</code></pre>

Add `--required-declarations` to list which contained definitions an owner declares as required through `sdfRequired`, and `--format json` to get the same views as JSON for scripts.

## Follow a finding to its source

A finding's source location is a pointer you can inspect directly. Take the draft's `data.string` finding at `/sdfData/Unfinished/type` and look at its owner:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view source --document draft/unfinished.sdf.json --pointer /sdfData/Unfinished draft/unfinished.sdf.json
...
<b>Inspection</b> source at "/sdfData/Unfinished":
{ "type": 7 }
</code></pre>

The source view prints the member byte for byte as it appears in the file, so what you see is what you need to fix. For a finding with an effective location, inspect the effective pointer first to see the composed definition, then use its `inherited` and `imported at` lines to find the file to change. Source inspection also works when a check failed or could not finish, so you can always look at what was supplied.

## Where to go next

- [Resolve missing and ambiguous name references](resolving-name-references.md) diagnoses unavailable targets and competing contributions to a global name.
- [Repeat checks and share reports](checks-and-reports.md) saves these commands as a plan, checks whole directories and emits JSON and SARIF.
- [Compare two revisions](revision-comparison.md) follows a changed shared definition to every device that uses it.
- The [supplied source documents](../source-workflow.md) reference explains catalogues, roots and views in full.
