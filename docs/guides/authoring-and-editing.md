# Compose and edit models

Build a new device description from reusable definitions, then make repeatable edits to an existing document. Both commands read a small JSON plan, write a new file and leave every input unchanged. You need Esdieff [installed](../getting-started/README.md).

## Compose an sdfThing from reusable objects

An `sdfObject` groups affordances and reusable definitions. An `sdfThing` can
contain `sdfObject` and `sdfThing` groupings. `esdieff author` creates a new
`sdfThing` document whose `sdfObject` declarations reference supplied reusable
`sdfObject` definitions through `sdfRef`. The generated source keeps those references.
Effective inspection then composes the referenced content and local refinements.

Save a reusable capabilities document as `capabilities.sdf.json`:

```json
{
  "info": { "title": "Reusable climate capabilities" },
  "namespace": { "cap": "https://example.com/capabilities" },
  "defaultNamespace": "cap",
  "sdfObject": {
    "Thermostat": {
      "description": "Reusable thermostat capability",
      "sdfProperty": {
        "targetTemperature": {
          "type": "number", "nullable": false, "minimum": 5, "maximum": 30,
          "multipleOf": 0.5, "unit": "Cel", "readable": true, "writable": true
        }
      }
    },
    "Fan": {
      "sdfProperty": {
        "speed": { "type": "integer", "minimum": 0, "maximum": 3 }
      }
    }
  }
}
```

Then write the recipe as `recipe.json`. It names the new `sdfThing`, binds the
namespace prefixes the generated document will use, and declares each `sdfObject`.
Each entry names the declaration, the supplied source and pointer selecting its
reusable definition, the output namespace prefix, a refinement patch and whether
the declaration is required. The thermostat refinement lowers the maximum to
25 for this product:

```json
{
  "key": "climate-recipe",
  "generated_key": "climate-unit",
  "thing_name": "ClimateUnit",
  "info": { "title": "Climate unit" },
  "namespace": {
    "cap": "https://example.com/capabilities",
    "product": "https://example.com/products"
  },
  "default_namespace": "product",
  "objects": [
    {
      "name": "thermostat",
      "source": "capabilities",
      "pointer": "/sdfObject/Thermostat",
      "prefix": "cap",
      "refinement": { "sdfProperty": { "targetTemperature": { "maximum": 25 } } },
      "required": true
    },
    {
      "name": "fan",
      "source": "capabilities",
      "pointer": "/sdfObject/Fan",
      "prefix": "cap",
      "refinement": {},
      "required": false
    }
  ]
}
```

`source` matches the key you give the file with `--source KEY PATH`. The `key` and `generated_key` name the request and the generated document in reports. Run the recipe, and ask for an initial check of the result in the same step:

<pre class="terminal"><code><span class="p">$</span> esdieff author --plan recipe.json --source capabilities capabilities.sdf.json --document-output climate-unit.sdf.json --check initial-v1
Document written: "climate-unit.sdf.json"
Plan: "recipe.json"
Report destination: standard output (-)
<b><span class="g">✔ </span></b><b><span class="h">Author</span></b> ClimateUnit: <b><span class="g">passed</span></b>
<span class="m">Generation:</span> <b><span class="g">complete</span></b>
<span class="m">Fidelity:</span> <b><span class="g">established</span></b>
Errors: 0 · warnings: 0 · notices: 0 · unfinished authoring units: 0
<span class="m">Input:</span> "capabilities.sdf.json" (source 1)
<span class="m">Input:</span> "climate-unit" (source 2)
<b><span class="g">✔ </span></b><span class="m">Requested checking</span> esdieff.initial/1: <b><span class="g">passed</span></b>
<span class="m">Diagnostic details omitted:</span> 0

<b><span class="h">Details:</span></b>
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Baseline: rfc9880/base-1</span>
<span class="m">Selected generated root: 2</span>
<span class="m">Dependency sources: 1</span>
<span class="m">Sources with attempted work: 2, 1</span>
<span class="m">Unavailable required sources: none</span>
<span class="m">Source 1 "capabilities": Model, Acquired</span>
<span class="m">Source 2 "climate-unit": Generated, Generated</span>
<span class="m">Requested check inventory: esdieff.initial/1</span>
<span class="m">Requested check coverage</span> (assessment_area): total 8 · assessed 8 · not applicable 0 · unassessed 0 · undetermined 0
</code></pre>

`Document written` confirms the output destination. `Generation: complete` says the document was assembled, and `Fidelity: established` says Esdieff verified that every generated reference still points at its source and every refinement value kept its exact spelling. The document is written only when both hold and the requested check passes. A failed or unfinished check instead says `Document not written` and explains
why. When replacing an existing output, withholding preserves that file. If you
omit `--check`, the report says checking was not requested. Here is the generated
`climate-unit.sdf.json`:

```json
{
  "namespace": {
    "cap": "https://example.com/capabilities",
    "product": "https://example.com/products"
  },
  "defaultNamespace": "product",
  "info": {
    "title": "Climate unit"
  },
  "sdfThing": {
    "ClimateUnit": {
      "sdfObject": {
        "thermostat": {
          "sdfRef": "cap:#/sdfObject/Thermostat",
          "sdfProperty": {
            "targetTemperature": {
              "maximum": 25
            }
          }
        },
        "fan": {
          "sdfRef": "cap:#/sdfObject/Fan"
        }
      },
      "sdfRequired": ["thermostat"]
    }
  }
}
```

The thermostat keeps its link to the capabilities document and overrides only the maximum. Inspect the effective property to confirm what the product presents, supplying both files:

<pre class="terminal"><code><span class="p">$</span> esdieff inspect --view effective --document climate-unit.sdf.json --pointer /sdfThing/ClimateUnit/sdfObject/thermostat/sdfProperty/targetTemperature climate-unit.sdf.json capabilities.sdf.json
<b><span class="g">✔ </span></b><b><span class="h">Esdieff:</span></b> <b><span class="g">passed</span></b>
<span class="m">Complete:</span> true
<span class="m">Errors:</span> 0 · <span class="m">warnings:</span> 0 · <span class="m">notices:</span> 0 · <span class="m">incomplete checks:</span> 0
<span class="m">Scope:</span> selected
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Input:</span> "climate-unit.sdf.json" (source 1)

<b><span class="h">Inspection</span></b> effective at "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfProperty/targetTemperature":
{"type":"number","nullable":false,"minimum":5,"maximum":25,"multipleOf":0.5,"unit":"Cel","readable":true,"writable":true}
...
override at effective 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfProperty/targetTemperature"; source 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfProperty/targetTemperature" bytes 387..430; source 2 "/sdfObject/Thermostat/sdfProperty/targetTemperature" bytes 293..459; imported at source 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfRef" bytes 297..325
override at effective 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfProperty/targetTemperature/maximum"; source 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfProperty/targetTemperature/maximum" bytes 414..416; source 2 "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum" bytes 367..369; imported at source 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfRef" bytes 297..325
inherited at effective 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfProperty/targetTemperature/multipleOf"; source 2 "/sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf" bytes 395..398; imported at source 1 "/sdfThing/ClimateUnit/sdfObject/thermostat/sdfRef" bytes 297..325
</code></pre>

The `maximum` is an `override` from the generated document, and the step is `inherited` from the capabilities file. Running the same command again refuses to overwrite the output until you add `--replace-document`:

<pre class="terminal"><code><span class="p">$</span> esdieff author --plan recipe.json --source capabilities capabilities.sdf.json --document-output climate-unit.sdf.json --check initial-v1
<b><span class="r">esdieff:</span></b> output "climate-unit.sdf.json" already exists. Choose another destination or use --replace-document to replace it
</code></pre>

Use `--report-output report.json --format json` to save the report beside the document, and `--check full-baseline` for the complete assessment. A refinement member set to `null` deletes the inherited member. The [authoring contract](../authoring-contract.md) lists every plan field, the refinement rules and the limits.

## Edit qualities in an existing document

`esdieff edit` changes named qualities of definitions you already have, such as a description, a bound or a unit, and writes a new document. It is a way to make the same correction repeatably, review it as a plan and keep the original untouched.

Starting from the thermostat model as `thermostat.sdf.json`, save this plan as `edits.json`. Each edit names the definition that owns the quality, the quality, and either a replacement value or a removal:

```json
{
  "key": "winter-edits",
  "source": "thermostat",
  "generated_key": "thermostat-winter",
  "edits": [
    {
      "owner": "/sdfObject/Thermostat/sdfProperty/targetTemperature",
      "quality": "maximum",
      "operation": "replace",
      "value": 25
    },
    {
      "owner": "/sdfObject/Thermostat/sdfProperty/targetTemperature",
      "quality": "description",
      "operation": "replace",
      "value": "Requested room temperature in half-degree increments, capped for winter"
    },
    {
      "owner": "/sdfObject/Thermostat/sdfProperty/currentTemperature",
      "quality": "description",
      "operation": "remove"
    }
  ]
}
```

Run it the same way as a recipe:

<pre class="terminal"><code><span class="p">$</span> esdieff edit --plan edits.json --source thermostat thermostat.sdf.json --document-output thermostat-winter.sdf.json --check initial-v1
Document written: "thermostat-winter.sdf.json"
Plan: "edits.json"
Report destination: standard output (-)
<b><span class="g">✔ </span></b><b><span class="h">Edit</span></b> winter-edits: <b><span class="g">passed</span></b>
<span class="m">Generation:</span> <b><span class="g">complete</span></b>
<span class="m">Fidelity:</span> <b><span class="g">established</span></b> (esdieff.authored-quality-edit/1)
Errors: 0 · warnings: 0 · notices: 0 · unfinished editing units: 0
<span class="m">Input:</span> "thermostat-winter" (source 2)
<b><span class="g">✔ </span></b><span class="m">Checking:</span> esdieff.initial/1: <b><span class="g">passed</span></b>
<span class="m">Diagnostic details omitted:</span> 0

<b><span class="h">Details:</span></b>
<span class="m">Broader RFC coverage:</span> partial
<span class="m">Substitution: source 1 with generated source 2</span>
<span class="m">Root sources: [2]</span>
<span class="m">Dependency sources: []</span>
<span class="m">Assessed sources: [2]</span>
<span class="m">Unavailable required sources: []</span>
<span class="m">Requested editing coverage:</span> total 1 · assessed 1 · undetermined 0
<span class="m">Requested model coverage:</span> total 8 · assessed 8 · not applicable 0 · unassessed 0 · undetermined 0
</code></pre>

Fidelity here means that every requested change was made and that everything else, including the spelling of every number, is unchanged. Esdieff reformats the output as indented JSON, so expect whitespace differences but no value differences. Each edit must target a member that exists: `replace` and `remove` both refuse an absent quality, which keeps a plan from silently doing nothing after the model changes under it.

Compare the result with the original to see exactly what changed and what it means for apps:

<pre class="terminal"><code><span class="p">$</span> esdieff diff --old thermostat.sdf.json --new thermostat-winter.sdf.json --format markdown
# Model changes

Disposition: failed, with 1 breaking, 0 undetermined and 2 compatible differences.

Earlier input: `thermostat.sdf.json`

Later input: `thermostat-winter.sdf.json`

Interface scope: effective Things and Objects, including inherited content. Standalone top-level affordances and data are excluded.

Perspective: client. Client interactions only. Context metadata compatibility is not assessed.

## Breaking differences

- Changed quality `maximum` in `document` at `/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum` (witness `30` for client-supplied data, earlier admitted, later rejected)

## Compatible differences

- Changed quality `description` in `document` at `/sdfObject/Thermostat/sdfProperty/targetTemperature/description`
- Changed quality `description` in `document` at `/sdfObject/Thermostat/sdfProperty/currentTemperature/description`
</code></pre>

Lowering the maximum is breaking, because an app that used to write 30 would
now be refused under the later description. [Compare two revisions](revision-comparison.md)
explains Esdieff's classification. The [editing contract](../editing-contract.md)
lists the qualities each kind of definition accepts and the limits. The editing
operation refuses an owner or enclosing definition/data-description context with
an authored `sdfRef`, including locally written overrides there. It does not
rewrite a composition patch or modify the referenced document through that owner.

## Where to go next

The [Context authoring example](../../examples/context-authoring/README.md) assembles a sensor with non-interactive calibration metadata, refines its limits and edits an authored site-name definition. Context uses the existing recipe and editing plans, with the same source-preservation guarantees and `sdfRef` ownership restrictions. See the [Context draft policy](../context-contract.md) for its defaults and requiredness boundaries.

- [Inspect and check a model](inspect-and-check.md) checks the generated documents with their dependencies.
- [Use the Rust library](../rust-library.md) builds the same requests from Rust with `AuthorRequest` and `EditRequest`.
