# Inspect and check a model

Check a model that reuses a shared definition, read the findings and inspect the effective definitions behind them. You need Esdieff [installed](../getting-started/README.md), the rich model saved as `thermostat.sdf.json` on that page, and the repository examples linked below.

## Supply the files a model needs

An SDF model can reference definitions in other SDF documents. Esdieff resolves those name references among the documents you supply, without fetching namespace URIs. Together, the supplied SDF documents form the **catalogue**.

Save the following two documents from the [directory-check example](https://github.com/pederbe/esdieff/blob/main/examples/directory-check) as `device.sdf.json` and `types.sdf.json`, respectively. The `Sensor` sdfObject declares a Property with the Given Name `reading`. Its `sdfRef` quality reuses the `Reading` sdfData definition through a CURIE reference whose namespace prefix is `types`:

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
<b><span class="y">⚠ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="y">undetermined</span></b>
<span class="m">Complete:</span> <b><span class="y">false</span></b>
<span class="m">Errors:</span> 0 · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> <b><span class="y">6</span></b>
<span class="m">Scope:</span> catalogue
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "device.sdf.json" (source 1)

<b><span class="y">⚠ incomplete</span></b> reference.unavailable: supplied global target unavailable
<span class="m">Input:</span> "device.sdf.json" (source 1)
effective 1 "/sdfObject/Sensor/sdfProperty/reading"
  related source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
  related source 1 "/sdfObject/Sensor/sdfProperty/reading" bytes 187..226

<b><span class="y">⚠ incomplete</span></b> coverage.unavailable: Requested assessment area has unfinished applicable work (5 assessment areas)

<b><span class="y">⚠ incomplete</span></b> coverage.advisory_unavailable: Requested assessment area has unfinished applicable work

<span class="m">Diagnostic details omitted:</span> 0
...
</code></pre>

The missing target leaves reference resolution unfinished, producing an `undetermined` verdict and exit status 3. This is an Esdieff processing limitation under the supplied catalogue. Supply both files, and select `device.sdf.json` as the **root document** so the requested model check covers that document and its needed dependencies:

<pre class="terminal"><code><span class="p">$</span> esdieff check --root device.sdf.json device.sdf.json types.sdf.json
<b><span class="g">✔ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="g">passed</span></b>
<span class="m">Complete:</span> true
<span class="m">Errors:</span> 0 · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> 0
<span class="m">Scope:</span> selected
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "device.sdf.json" (source 1)
<span class="m">Diagnostic details omitted:</span> 0

<b><span class="h">Details:</span></b>

<span class="m">Requested assessment: esdieff.initial/1</span>
<span class="m">Baseline: rfc9880/base-1 (RFC 9880)</span>
<span class="m">Acceptance restrictions: A001, A002; strict warnings: false</span>
<span class="m">Requested sources: [1]</span>
<span class="m">Requested inventory: esdieff.initial/1</span>
<span class="m">Requested coverage (assessment_area):</span> total 8 · assessed 7 · not applicable 1 · unassessed 0 · undetermined 0
<span class="m">Source 1 ["input1"] "device.sdf.json": acquired / usable (247 retained bytes, content complete: true)</span>
<span class="m">Source 2 ["input2"] "types.sdf.json": acquired / usable (211 retained bytes, content complete: true)</span>
<span class="m">Dependency sources: [2]</span>
</code></pre>

`Scope: selected` with `Dependency sources: [2]` records that `types.sdf.json` was used as a dependency of the root document. Without `--root`, the scope is `catalogue`: every supplied model document, including unused definitions, is assessed under the requested assessment profile. Catalogue scope suits a collection you maintain. Selected-model scope starts from the chosen roots and assesses the dependency content needed for their checks.

## Choose how much to check

`--assessment` selects an **assessment profile**. The default `initial-v1` checks the fundamentals in eight areas, including names, namespaces, composition and parameters. `full-baseline` checks all 13 areas and accounts for all 54 model requirements Esdieff records from RFC 9880, using bundled registry snapshots. Two additional profiles provide focused advice beyond those requirements. `expanded-numeric-v1` finds ranges with no allowed integer or step value. `expanded-consistency-v1` finds a `const` or `default` value that its own description excludes.

```sh
esdieff check --assessment full-baseline --root device.sdf.json device.sdf.json types.sdf.json
```

Every model-check report names its inventory and counts assessed, not applicable, unassessed and undetermined areas. Read those counts with the verdict to see what the requested assessment established. `--strict-warnings` makes warnings fail the check too, which is useful when CI should reject warning advice. The [model meaning contract](../model-meaning-contract.md) and the [assessment inventory](../model-assessment-inventory.md) describe each area.

## Read findings and locations

A finding has a condition code, a message and any available locations. To follow this example, create a `draft` directory and save [unfinished.sdf.json](https://github.com/pederbe/esdieff/blob/main/examples/directory-check/draft/unfinished.sdf.json) there. Its `type` quality has a numeric value where text is required:

<pre class="terminal"><code><span class="p">$</span> esdieff check draft/unfinished.sdf.json
<b><span class="r">✗ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="r">failed</span></b>
<span class="m">Complete:</span> true
<span class="m">Errors:</span> <b><span class="r">1</span></b> · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> 0
<span class="m">Scope:</span> catalogue
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "draft/unfinished.sdf.json" (source 1)

<b><span class="r">✗ error</span></b> data.string: A textual data quality must contain a string
<span class="m">Input:</span> "draft/unfinished.sdf.json" (source 1)
source 1 "/sdfData/Unfinished/type" bytes 84..85

<span class="m">Diagnostic details omitted:</span> 0
...
</code></pre>

`data.string` is the stable condition code, and the message explains it. This **source location** identifies the authored `type` quality by its source ID and JSON Pointer. The Input line names the file beside that identity. Errors fail the check. Warnings fail it when `--strict-warnings` is set. Notices are informational and do not themselves fail the check. Incomplete checks mark unfinished requested work. With no error or strict-warning failure, that incompleteness makes the verdict undetermined. `Complete: true` records completed requested work, including established errors.

An **effective location** identifies a position in a root document's composed model space. The earlier `reference.unavailable` finding uses `/sdfObject/Sensor/sdfProperty/reading` for the Property whose composition could not finish. Its related source locations identify the authored `sdfRef` and referencing definition. The [report contract](../report-contract.md#numbers-and-locations) describes both location kinds.

## Inspect what the interface contains

`esdieff inspect` selects retained source JSON or composed effective JSON by document and JSON Pointer. In the effective view, `sdfRef` composition copies referenced content and applies the referencing definition's JSON Merge Patch, following [RFC 9880 Section 4.4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.4). Source explanations identify content origins and any importing consumer. Established implicit defaults appear as separate explanations. They are never inserted into the source or composed JSON, as described in [the effective-view contract](../adr/0020-owned-effective-views-and-nearest-origins.md).

Inspection completion concerns the selected view. Run `esdieff check` to request model assessment. Inspect the thermostat's target temperature:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view effective --document thermostat.sdf.json --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature thermostat.sdf.json
<b><span class="g">✔ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="g">passed</span></b>
<span class="m">Complete:</span> true
<span class="m">Errors:</span> 0 · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> 0
<span class="m">Scope:</span> selected
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "thermostat.sdf.json" (source 1)

<b><span class="h">Inspection</span></b> effective at "/sdfObject/Thermostat/sdfProperty/targetTemperature":
{"description":"Requested room temperature in half-degree increments","type":"number","nullable":false,"minimum":5,"maximum":30,"multipleOf":0.5,"unit":"Cel","readable":true,"writable":true}
...
<span class="m">Alternatives</span> absent at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature"
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature" bytes 449..696
default true at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/observable"
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/description"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/description" bytes 476..530
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/type"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/type" bytes 550..558
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/nullable"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/nullable" bytes 572..577
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/minimum"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/minimum" bytes 590..591
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum" bytes 604..606
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf" bytes 632..635
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/unit"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/unit" bytes 645..650
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/readable"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/readable" bytes 664..668
source at effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/writable"; source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/writable" bytes 682..686
</code></pre>

The technical details between the inspection and its source explanations are trimmed. The JSON shows the authored qualities of this Property. `observable` is absent, so a separate `default true` explanation records the specified default from [RFC 9880 Section 5.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-5.2). There is no inserted `observable` member or authored source for that default.

The `Sensor` grouping's `reading` Property obtains its data qualities from the other document:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view effective --document device.sdf.json --pointer /sdfObject/Sensor/sdfProperty/reading device.sdf.json types.sdf.json
<b><span class="g">✔ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="g">passed</span></b>
<span class="m">Complete:</span> true
<span class="m">Errors:</span> 0 · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> 0
<span class="m">Scope:</span> selected
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "device.sdf.json" (source 1)

<b><span class="h">Inspection</span></b> effective at "/sdfObject/Sensor/sdfProperty/reading":
{"type":"number","minimum":0}
...
<span class="m">Alternatives</span> absent at effective 1 "/sdfObject/Sensor/sdfProperty/reading"
inherited at effective 1 "/sdfObject/Sensor/sdfProperty/reading"; source 2 "/sdfData/Reading" bytes 172..206; source 1 "/sdfObject/Sensor/sdfProperty/reading" bytes 187..226; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/readable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/writable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/observable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
default true at effective 1 "/sdfObject/Sensor/sdfProperty/reading/nullable"; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
inherited at effective 1 "/sdfObject/Sensor/sdfProperty/reading/type"; source 2 "/sdfData/Reading/type" bytes 182..190; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
inherited at effective 1 "/sdfObject/Sensor/sdfProperty/reading/minimum"; source 2 "/sdfData/Reading/minimum" bytes 203..204; imported at source 1 "/sdfObject/Sensor/sdfProperty/reading/sdfRef" bytes 199..224
</code></pre>

The `inherited` explanations identify source contributions to the copied content. `imported at` identifies the source `sdfRef` that establishes the composition boundary, called the **importing consumer** in Esdieff's glossary. The separate `default true` explanations have no authored quality source. Together these locations let you trace an effective bound to the shared definition that supplied it. A bound is a data quality, distinct from the Property that carries it.

For a reusable sdfData definition, save [count.sdf.json](https://github.com/pederbe/esdieff/blob/main/examples/catalogue-check/count.sdf.json) in your working directory, then inspect it:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view effective --document count.sdf.json --pointer /sdfData/Count count.sdf.json
<b><span class="g">✔ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="g">passed</span></b>
<span class="m">Complete:</span> true
<span class="m">Errors:</span> 0 · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> 0
<span class="m">Scope:</span> selected
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "count.sdf.json" (source 1)

<b><span class="h">Inspection</span></b> effective at "/sdfData/Count":
{"type":"integer","minimum":0,"maximum":100}
...
<span class="m">Alternatives</span> absent at effective 1 "/sdfData/Count"
source at effective 1 "/sdfData/Count"; source 1 "/sdfData/Count" bytes 64..115
default true at effective 1 "/sdfData/Count/nullable"
source at effective 1 "/sdfData/Count/type"; source 1 "/sdfData/Count/type" bytes 74..83
source at effective 1 "/sdfData/Count/minimum"; source 1 "/sdfData/Count/minimum" bytes 96..97
source at effective 1 "/sdfData/Count/maximum"; source 1 "/sdfData/Count/maximum" bytes 110..113
</code></pre>

Add `--required-declarations` to inspect the required declaration relationships selected by `sdfRequired`. These concern affordance and grouping declarations whose corresponding data are mandatory, following [RFC 9880 Section 4.5](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.5). Relationship completion and target capability are reported separately. Map-data `required` selects mandatory map entries. Nullability controls whether a value can be null. Add `--format json` for structured inspection results.

## Follow a finding to its source

A finding's source location is a pointer you can inspect directly. Take the draft's `data.string` finding at `/sdfData/Unfinished/type` and look at its owner:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view source --document draft/unfinished.sdf.json --pointer /sdfData/Unfinished draft/unfinished.sdf.json
<b><span class="g">✔ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="g">passed</span></b>
<span class="m">Complete:</span> true
<span class="m">Errors:</span> 0 · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> 0
<span class="m">Scope:</span> selected
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "draft/unfinished.sdf.json" (source 1)

<b><span class="h">Inspection</span></b> source at "/sdfData/Unfinished":
{ "type": 7 }
...
</code></pre>

For original file inputs, the source view displays retained JSON with its original spelling and formatting. Source inspection can remain available when model checking fails or reference resolution cannot finish. Acquisition and decoding limits can still leave selected content unavailable.

For an effective finding, select the document that owns the effective location and inspect its pointer. Follow the content-origin and importing-consumer locations to the authored documents. If the effective subtree is unavailable, inspect the finding's related source locations directly, starting with the authored `sdfRef` when one is identified.

## Where to go next

- [Resolve missing and ambiguous name references](resolving-name-references.md) diagnoses unavailable targets and competing contributions to a global name.
- [Repeat checks and share reports](checks-and-reports.md) saves these commands as a plan, checks whole directories and emits JSON and SARIF.
- [Compare two revisions](revision-comparison.md) follows a changed shared definition to every device that uses it.
- The [supplied source documents](../source-workflow.md) reference explains catalogues, roots and views in full.
