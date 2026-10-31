# Repeat checks and share reports

This guide saves checks so they run the same way every time, checks a whole directory of models, and brings the results into CI and code review. It assumes Esdieff is [installed](../getting-started/README.md).

## Save checks as a plan

A **check plan** is a JSON file that names the model and sample files to supply and the cases to run on them. One command then runs every case and reports each outcome by name. The repository's [thermostat-check example](https://github.com/pederbe/esdieff/blob/main/examples/thermostat-check) includes this `plan.json`:

```json
{
  "schema": "esdieff.check-plan",
  "version": "1",
  "sources": [
    { "key": "thermostat", "path": "thermostat.sdf.json", "role": "model" },
    { "key": "comfortable", "path": "comfortable.json", "role": "sample" },
    { "key": "too-hot", "path": "too-hot.json", "role": "sample" }
  ],
  "cases": [
    { "name": "Catalogue", "operation": "check", "scope": { "kind": "catalogue" } },
    {
      "name": "Thermostat model",
      "operation": "check",
      "scope": { "kind": "selected", "roots": ["thermostat"] },
      "assessment": "full-baseline",
      "strict_warnings": true
    },
    {
      "name": "Comfortable target",
      "operation": "sample",
      "document": "thermostat",
      "kind": "property",
      "pointer": "/sdfObject/Thermostat/sdfProperty/targetTemperature",
      "sample": "comfortable"
    },
    {
      "name": "Too hot target",
      "operation": "sample",
      "document": "thermostat",
      "kind": "property",
      "pointer": "/sdfObject/Thermostat/sdfProperty/targetTemperature",
      "sample": "too-hot"
    }
  ]
}
```

Paths are relative to the plan, so the directory moves as a unit. Each case keeps its own scope, assessment level and warning policy. Copy the example directory and run it. The human report prints each case in full, so this command keeps the summary lines:

<pre class="terminal"><code><span class="p">$</span> esdieff check --plan plan.json | grep -E '^(Case|Disposition|Admission|Check case summary)'
<b>Case:</b> "Catalogue"
Disposition: passed
<b>Case:</b> "Thermostat model"
Disposition: passed
<b>Case:</b> "Comfortable target"
Disposition: passed
Admission: admitted
<b>Case:</b> "Too hot target"
Disposition: failed
Admission: rejected
Check case summary: {"admission_undetermined":0,"admitted":1,"complete":true,"complete_cases":4,"failed":1,"incomplete_cases":0,"omitted_reports":0,"passed":3,"rejected":1,"request_errors":0,"total":4,"undetermined":0,"verdict":"failed"}
</code></pre>

The run exits 1, because the rejected sample is a failed case. A plan has no expected-result field: a sample you intend to reject still fails, which keeps the verdict honest. Keep such samples in a separate plan, or check them with [recorded traces](testing-values.md#check-a-log-of-recorded-values), where an `expected` member turns an intended rejection into a pass. The [catalogue-check example](https://github.com/pederbe/esdieff/blob/main/examples/catalogue-check) is the smallest model-only plan, and a `diff` case can compare two revisions beside the checks, as [Repeatable catalogue checks](../check-plans.md) shows.

## Check a directory of models

`--directory` selects every regular file ending in `.sdf.json` in a directory, so a check follows the collection as files are added. Run this on the repository's [directory-check example](https://github.com/pederbe/esdieff/blob/main/examples/directory-check), which holds a device, a shared type, a nested status model and an unfinished draft:

<pre class="terminal"><code><span class="p">$</span> esdieff check --directory examples/directory-check --recursive --exclude draft --format json | jq -c '.sources[] | {id, label}'
{"id":1,"label":"examples/directory-check/device.sdf.json"}
{"id":2,"label":"examples/directory-check/nested/status.sdf.json"}
{"id":3,"label":"examples/directory-check/types.sdf.json"}
</code></pre>

`--recursive` descends into subdirectories and `--exclude` prunes a literal file or subtree relative to each directory. Sample files, plans and a `.bak` lookalike are not selected. Without the exclusion, the draft's broken type fails the run:

<pre class="terminal"><code><span class="p">$</span> esdieff check --directory examples/directory-check --recursive
<b>Esdieff:</b> <span class="r">failed</span>
<b>Complete:</b> true
<b>Errors:</b> 1; <b>warnings:</b> 0; <b>notices:</b> 0; <b>incomplete checks:</b> 0
...
<span class="r">data.string</span>: A textual data quality must contain a string
source 2 "/sdfData/Unfinished/type"
<b>Diagnostic details omitted:</b> 0
</code></pre>

Repeat `--directory` for several collections, add explicit files beside them, and name a discovered root by its joined path, such as `--root examples/directory-check/device.sdf.json`. [Check a local model collection](../directory-discovery.md) states the selection rules and limits.

## Use JSON reports in automation

`check`, `inspect` and `sample` can write a versioned JSON report with
`--format json`. Its `outcome` member is the summary a script needs:

<pre class="terminal"><code><span class="p">$</span> esdieff check --format json thermostat.sdf.json | jq '.outcome'
{
  "complete": true,
  "disposition": "passed",
  "errors": 0,
  "incomplete_checks": 0,
  "notices": 0,
  "strict_warnings": false,
  "warnings": 0
}
</code></pre>

`diagnostics.items` holds each finding with its code, message, locations and the RFC section it rests on, and `coverage` records what was assessed. A plan run has its own schema, `esdieff.check-run`, whose `summary` counts the cases:

<pre class="terminal"><code><span class="p">$</span> esdieff check --plan plan.json --format json | jq -c '.summary'
{"admission_undetermined":0,"admitted":1,"complete":true,"complete_cases":4,"failed":1,"incomplete_cases":0,"omitted_reports":0,"passed":3,"rejected":1,"request_errors":0,"total":4,"undetermined":0,"verdict":"failed"}
</code></pre>

The [report contract](../report-contract.md) describes the shared envelope and
how reports stay within their size limits. Other operations have their own
versioned shapes and summary fields, including revision comparison and schema
export. `reference` writes Markdown rather than accepting `--format json`.
Check each operation's contract and identity before reading its fields. Additive
fields can appear in later releases, so read the members you need rather than
comparing whole documents.

## Show findings in code review with SARIF

`esdieff check --format sarif` and `esdieff diff --format sarif` write a SARIF 2.1.0 log, the format code-scanning tools such as GitHub's read to annotate files in a pull request. Each finding becomes a result with its code as the rule, a level and the file, line and column it is about:

<pre class="terminal"><code><span class="p">$</span> esdieff check --directory examples/directory-check --recursive --format sarif | jq '.runs[0].results[] | {ruleId, level, message: .message.text, file: .locations[0].physicalLocation.artifactLocation.uri, line: .locations[0].physicalLocation.region.startLine}'
{
  "ruleId": "data.string",
  "level": "error",
  "message": "A textual data quality must contain a string",
  "file": "examples/directory-check/draft/unfinished.sdf.json",
  "line": 3
}
</code></pre>

File paths are relative to the directory the command ran in, so run it from the repository root and upload the log with your code-scanning tool's SARIF step. Acknowledged revision differences appear as suppressed results, as [Compare two revisions](revision-comparison.md#share-the-comparison) shows.

## Write change notes with Markdown

`esdieff diff --format markdown` writes a revision comparison as a changelog grouped by breaking, undetermined and compatible changes, ready to paste into release notes or a pull request description. [Compare two revisions](revision-comparison.md#share-the-comparison) shows the output.

## Use exit statuses

Every command uses the same exit statuses, so a shell or CI step can act on them without parsing a report:

| Status | Meaning |
| --- | --- |
| 0 | The requested work passed |
| 1 | A finding failed it: an error, a rejected sample, a breaking change or a warning under `--strict-warnings` |
| 2 | The invocation, plan or a case request was invalid |
| 3 | Nothing failed, but some work could not be settled, such as a missing dependency or an undetermined difference |
| 4 | A host or output operation failed, such as writing the report |

An invalid invocation or undecodable plan writes its reason to standard error
without a report. A valid plan whose individual case request is invalid still
writes a check-run report, records that case's `request_error` and continues
with the other cases, then exits 2.

Generation has no failed domain disposition, so a valid `esdieff generate`
request with successful output exits 0 or 3. Invalid invocation and host/output
failures can still exit 2 or 4. A status of 3 deserves attention in CI because
some requested work remains unsettled. Neither successful generation nor
successful documentation establishes a passed model assessment.
