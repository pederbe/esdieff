# Compare two revisions

This guide compares two revisions of a model before a change is published, reads
which differences break the apps that use the device, and records the ones you
intend. It uses a [current-checkout installation](../getting-started/README.md#direct-downloads-and-cargo)
for the `esdieff.revision-comparison/4` output shown here.

## Build a before and after pair

Save the earlier revision as `v1/thermostat.sdf.json`. It is the thermostat's target temperature on its own, with a version in its `info` block:

```json
{
  "info": { "title": "Room thermostat", "version": "1.0.0" },
  "sdfObject": {
    "Thermostat": {
      "sdfProperty": {
        "targetTemperature": {
          "description": "Requested room temperature in half-degree increments",
          "type": "number", "nullable": false, "minimum": 5, "maximum": 30,
          "multipleOf": 0.5, "unit": "Cel", "readable": true, "writable": true
        }
      }
    }
  }
}
```

Save the later revision as `v2/thermostat.sdf.json`. It raises the maximum to 35, drops the half-degree step, shortens the description and leaves the version unchanged:

```json
{
  "info": { "title": "Room thermostat", "version": "1.0.0" },
  "sdfObject": {
    "Thermostat": {
      "sdfProperty": {
        "targetTemperature": {
          "description": "Requested room temperature",
          "type": "number", "nullable": false, "minimum": 5, "maximum": 35,
          "unit": "Cel", "readable": true, "writable": true
        }
      }
    }
  }
}
```

## Compare them

<pre class="terminal"><code><span class="p">$</span> esdieff diff --old v1/thermostat.sdf.json --new v2/thermostat.sdf.json
<b>Revision comparison</b> esdieff.revision-comparison/4: <span class="r">breaking</span>
Client interactions only. Context metadata compatibility is not assessed.
<b>Documents:</b> 1 compared, 0 removed, 0 added
<b>Differences:</b> 1 <span class="r">breaking</span>, 1 <span class="y">undetermined</span>, 1 <span class="g">compatible</span>
<b>Warnings:</b>
  <span class="y">comparison.version_not_increased</span> in document: later version "1.0.0" does not compare higher than earlier version "1.0.0", after 1 breaking difference
<span class="r">Breaking differences:</span>
  changed quality maximum (client-supplied and thing-supplied data)
    witness 35 for thing-supplied data: earlier rejected, later admitted
    earlier document /sdfObject/Thermostat/sdfProperty/targetTemperature/maximum
    later document /sdfObject/Thermostat/sdfProperty/targetTemperature/maximum
<span class="y">Undetermined differences:</span>
  changed quality multipleOf (client-supplied and thing-supplied data): no breaking witness or compatibility proof
    earlier document /sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf
    later document /sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf (absent)
<span class="g">Compatible differences:</span>
  changed quality description (client-supplied and thing-supplied data)
    earlier document /sdfObject/Thermostat/sdfProperty/targetTemperature/description
    later document /sdfObject/Thermostat/sdfProperty/targetTemperature/description
</code></pre>

Esdieff takes the side of the **client**, the app or controller that talks to the device. Each difference is classified by what it means for a client built against the earlier revision:

- **Breaking** means such a client can be surprised. Raising the maximum looks harmless for values the client writes, but the property is also readable, and a device may now report 35 where the client expects at most 30. The **witness** line shows the concrete value that proves it: `35` was rejected before and is admitted now, in the direction the device supplies.
- **Compatible** means the difference preserves client interactions, as with a changed description.
- **Undetermined** means Esdieff found neither a breaking witness nor a proof of compatibility, so the change needs a human look. Removing the step is such a case. An undetermined difference is never presented as safe.

The warning says the later document's `version` does not compare higher despite
a breaking difference. The comparison has not established that anything was
shipped. RFC 9880 recommends version strings that increase lexicographically
over a model's lifetime. Esdieff emits this advisory specifically when it finds
a breaking difference. Warnings do not change the comparison disposition. Without acknowledgements,
the command exits 1 for a breaking comparison, 3 for an undetermined one and 0
for a compatible one, so a CI job can stop a breaking change from merging.

## Context metadata

Context definitions describe non-interactive metadata. Their additions, removals and changed content appear as `sdfContext` differences in the same comparison, including inherited and refined Context. These differences have no read or write direction and are compatible for client interactions. A metadata consumer may still need changes, so the report explicitly says that Context metadata compatibility is not assessed.

Changing shared data used by a Property still receives the Property's usual compatibility assessment, even when a Context uses that data too. Moving a value from Property to Context removes the old client interaction and remains breaking. See [Context metadata](../revision-comparison-contract.md#context-metadata) for the scope and reference boundaries.

## Changes that reach other documents through a shared definition

The [shared-definition walkthrough](shared-definition-changes.md) starts with
`reference --uses`, explains a local override, and follows a changed shared
quality into a comparison of two revisions.

A change in a shared definition can appear at each effective consumer where it
survives local overrides. For this separate example, create `shared-v1/` and
`shared-v2/`. Save the following documents as `shared-v1/types.sdf.json` and
`shared-v1/thermostat.sdf.json`, respectively. Copy both to `shared-v2/`, then
lower the shared minimum there from -40 to -50. Keep the original `v1/` and
`v2/` pair for the acknowledgement walkthrough below:

```json
{
  "info": { "title": "Shared types" },
  "namespace": { "types": "https://example.com/types" },
  "defaultNamespace": "types",
  "sdfData": { "Celsius": { "type": "number", "minimum": -40, "maximum": 80, "unit": "Cel" } }
}
```

```json
{
  "info": { "title": "Room thermostat" },
  "namespace": { "types": "https://example.com/types" },
  "sdfObject": {
    "Thermostat": {
      "sdfProperty": {
        "currentTemperature": { "sdfRef": "types:#/sdfData/Celsius", "writable": false },
        "targetTemperature": { "sdfRef": "types:#/sdfData/Celsius", "minimum": 5, "maximum": 30 }
      }
    }
  }
}
```

<pre class="terminal"><code><span class="p">$</span> esdieff diff --old shared-v1 --new shared-v2
<b>Revision comparison</b> esdieff.revision-comparison/4: <span class="r">breaking</span>
Client interactions only. Context metadata compatibility is not assessed.
<b>Documents:</b> 2 compared, 0 removed, 0 added
<b>Differences:</b> 1 <span class="r">breaking</span>, 0 <span class="y">undetermined</span>, 0 <span class="g">compatible</span>
<span class="r">Breaking differences:</span>
  changed quality minimum (thing-supplied data)
    witness -50 for thing-supplied data: earlier rejected, later admitted
    earlier thermostat.sdf.json /sdfObject/Thermostat/sdfProperty/currentTemperature/minimum
      from types.sdf.json /sdfData/Celsius/minimum
    later thermostat.sdf.json /sdfObject/Thermostat/sdfProperty/currentTemperature/minimum
      from types.sdf.json /sdfData/Celsius/minimum
</code></pre>

Documents in the two directories pair up by relative path, and files that end in `.sdf.json` are selected as for `esdieff check --directory`. The difference is reported on the thermostat's `currentTemperature`, because that is what a client sees, and the `from` lines name the shared definition that caused it. `targetTemperature` overrides the minimum, so it is unaffected. Add `--recursive` and `--exclude` to select nested files, as [Check a local model collection](../directory-discovery.md) describes.

## Acknowledge the changes you intend

When a breaking change is deliberate, record it so the comparison passes while the report still shows the break. First export the eligible differences for review:

```sh
esdieff diff --old v1/thermostat.sdf.json --new v2/thermostat.sdf.json --acknowledgements-output reviewed.json
```

The comparison report and exit status stay as before, and `reviewed.json` lists every breaking and undetermined difference with its exact identity:

<pre class="terminal"><code><span class="p">$</span> jq . reviewed.json
{
  "differences": [
    {
      "classification": "breaking",
      "earlier": {
        "document": "document",
        "pointer": "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum"
      },
      "kind": "changed",
      "later": {
        "document": "document",
        "pointer": "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum"
      },
      "quality": "maximum",
      "subject": "quality"
    },
    {
      "classification": "undetermined",
      ...
    }
  ],
  "schema": "esdieff.acknowledged-differences",
  "version": "1"
}
</code></pre>

Review the file, delete any entry you do not intend, add a `note` to each if you like, and keep it with the model. Then supply it:

<pre class="terminal"><code><span class="p">$</span> esdieff diff --old v1/thermostat.sdf.json --new v2/thermostat.sdf.json --acknowledged reviewed.json
<b>Revision comparison</b> esdieff.revision-comparison/4: <span class="r">breaking</span>
Client interactions only. Context metadata compatibility is not assessed.
<b>Disposition:</b> <span class="g">passed</span>, with 1 breaking and 1 undetermined differences acknowledged
<b>Documents:</b> 1 compared, 0 removed, 0 added
<b>Differences:</b> 1 <span class="r">breaking</span>, 1 <span class="y">undetermined</span>, 1 <span class="g">compatible</span>
<b>Warnings:</b>
  <span class="y">comparison.version_not_increased</span> in document: later version "1.0.0" does not compare higher than earlier version "1.0.0", after 1 breaking difference
<b>Acknowledged differences:</b>
  changed quality maximum (client-supplied and thing-supplied data), <span class="r">breaking</span>
    witness 35 for thing-supplied data: earlier rejected, later admitted
    earlier document /sdfObject/Thermostat/sdfProperty/targetTemperature/maximum
    later document /sdfObject/Thermostat/sdfProperty/targetTemperature/maximum
  changed quality multipleOf (client-supplied and thing-supplied data), <span class="y">undetermined</span>: no breaking witness or compatibility proof
    earlier document /sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf
    later document /sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf (absent)
<span class="g">Compatible differences:</span>
  changed quality description (client-supplied and thing-supplied data)
    earlier document /sdfObject/Thermostat/sdfProperty/targetTemperature/description
    later document /sdfObject/Thermostat/sdfProperty/targetTemperature/description
</code></pre>

The classification is still `breaking`, the disposition is `passed` and the exit
status is 0. Only unacknowledged differences decide the result. An entry matches
the difference's kind, subject, quality, classification and document/pointer
coordinates. It does not bind the model bytes, changed values or witness. Review
acknowledgements again whenever the compared revisions change, since a different
limit at the same coordinates can still match. A reclassified difference, such
as an undetermined change later proved breaking, no longer matches its old
entry. The version warning stays after acknowledgement, so update the document's
version before publishing the intended breaking revision.

## Share the comparison

`--format markdown` writes the comparison as a changelog for release notes, grouped by breaking, undetermined and compatible changes:

<pre class="terminal"><code><span class="p">$</span> esdieff diff --old v1/thermostat.sdf.json --new v2/thermostat.sdf.json --format markdown
# Model changes

Disposition: failed, with 1 breaking, 1 undetermined and 1 compatible differences.

Client interactions only. Context metadata compatibility is not assessed.

## Warnings

- Version `1.0.0` in `document` at `/info/version` does not compare higher than earlier version `1.0.0`, after 1 breaking difference (`comparison.version_not_increased`)

## Breaking differences

- Changed quality `maximum` in `document` at `/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum` (witness `35` for thing-supplied data, earlier rejected, later admitted)

## Undetermined differences

- Changed quality `multipleOf` in `document` at `/sdfObject/Thermostat/sdfProperty/targetTemperature/multipleOf`: no breaking witness or compatibility proof (`comparison.unproven`)

## Compatible differences

- Changed quality `description` in `document` at `/sdfObject/Thermostat/sdfProperty/targetTemperature/description`
</code></pre>

`--format sarif` writes a SARIF log that code-scanning tools such as GitHub's show as annotations on the model file. Acknowledged differences are marked as suppressed, so reviewers still see them while scanning treats them as intended:

<pre class="terminal"><code><span class="p">$</span> esdieff diff --old v1/thermostat.sdf.json --new v2/thermostat.sdf.json --acknowledged reviewed.json --format sarif | jq -c '.runs[0].results[] | {ruleId, level, suppressed: (.suppressions | length &gt; 0)}'
{"ruleId":"comparison.version_not_increased","level":"warning","suppressed":false}
{"ruleId":"comparison.breaking","level":"error","suppressed":true}
{"ruleId":"comparison.unproven","level":"warning","suppressed":true}
{"ruleId":"comparison.compatible","level":"note","suppressed":false}
</code></pre>

`--format json` gives the full report for scripts. A comparison can also run as a case in a check plan beside the model checks, as [Repeat checks and share reports](checks-and-reports.md) shows. The [revision comparison contract](../revision-comparison-contract.md) explains how each kind of quality is classified and what a witness proves.
