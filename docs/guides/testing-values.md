# Test values against a model

This guide checks single values against a model, generates boundary and invalid
values for tests, and checks a log of recorded values in one run. It uses the
current-checkout command and `thermostat.sdf.json` from
[Install and try a model](../getting-started/README.md), whose `targetTemperature`
accepts 5 to 30 °C in half-degree steps.

## Check one sample

A **sample** is one JSON value for one definition. Save `21.5` in `comfortable.json` and check it against the target temperature. `--kind property` and `--pointer` select the definition, and the trailing argument supplies the model file:

<pre class="terminal"><code><span class="p">$</span> esdieff sample --document thermostat.sdf.json --kind property --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature --sample comfortable.json thermostat.sdf.json
<b>Esdieff:</b> <span class="g">passed</span>
<b>Complete:</b> true
<b>Errors:</b> 0; <b>warnings:</b> 0; <b>notices:</b> 0; <b>incomplete checks:</b> 0
<b>Scope:</b> selected
...
<b>Selected</b> property in model 1 at "/sdfObject/Thermostat/sdfProperty/targetTemperature"
<b>Sample</b> 2: <span class="g">admitted</span>; context available
...
</code></pre>

`Sample 2: admitted` is the verdict on the value. Now save `35` as `too-hot.json`:

<pre class="terminal"><code><span class="p">$</span> esdieff sample --document thermostat.sdf.json --kind property --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature --sample too-hot.json thermostat.sdf.json
<b>Esdieff:</b> <span class="r">failed</span>
<b>Complete:</b> true
<b>Errors:</b> 1; <b>warnings:</b> 0; <b>notices:</b> 0; <b>incomplete checks:</b> 0
...
<b>Sample</b> 2: <span class="r">rejected</span>; context available
...
<span class="r">sample.bound</span>: Sample violates an exact numerical bound
source 2 ""
  related effective 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum"
  related source 1 "/sdfObject/Thermostat/sdfProperty/targetTemperature/maximum"
...
</code></pre>

The reports are trimmed to the lines about the sample. A rejected sample is a failed check with exit status 1, and the finding names the rule it broke and where that rule is defined. The other kinds are `data` for an `sdfData` definition, `action_input`, `action_output` and `event_output`, which select the data an action or event carries, and `context` for non-interactive metadata under an Object or Thing. Follow the [Context value example](../context-contract.md#check-context-values) to check a simple asset tag and compound location data. Numbers compare exactly, so `30.0` and `30` are the same value and `0.1` is never rounded.

## Generate representative test values

`esdieff generate` builds values from the model's rules, so your tests get expectations without hand-written payloads. Select one definition with `--kind` and `--pointer`, or leave them out to cover every property, action and event in the document:

<pre class="terminal"><code><span class="p">$</span> esdieff generate --document thermostat.sdf.json --kind property --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature thermostat.sdf.json
<b>Generated values</b> esdieff.generation/2: <span class="g">passed</span>
<b>Values:</b> 2 <span class="g">admitted</span>, 8 <span class="r">rejected</span>, 0 <span class="y">undetermined</span>
<b>Descriptions:</b> 1
Corpus evidence counts kept values, not constraints or individual Choice alternatives. Complete generation does not imply complete constraint coverage.

<b>property /sdfObject/Thermostat/sdfProperty/targetTemperature</b>: <span class="g">complete</span>
  Corpus evidence: 0 other admitted examples, 2 admitted boundary candidates, 7 rejected outside candidates, 1 other rejected candidates, 0 undetermined
  <span class="r">rejected    </span> null          null (sample.null) [rejected_candidate]
  <span class="g">admitted    </span> minimum       5 [admitted_boundary]
  <span class="g">admitted    </span> maximum       30 [admitted_boundary]
  <span class="r">rejected    </span> below_minimum 4 (sample.bound) [rejected_outside]
  <span class="r">rejected    </span> above_maximum 31 (sample.bound) [rejected_outside]
  <span class="r">rejected    </span> not_multiple  5.25 (sample.multiple) [rejected_outside]
  <span class="r">rejected    </span> wrong_type    "" (sample.type) [rejected_outside]
  <span class="r">rejected    </span> wrong_type    true (sample.type) [rejected_outside]
  <span class="r">rejected    </span> wrong_type    {} (sample.type) [rejected_outside]
  <span class="r">rejected    </span> wrong_type    [] (sample.type) [rejected_outside]
</code></pre>

Each value aims at one **target**, named in the middle column: the two bounds, a value just outside each, a value off the step, and one wrong type of each kind. Esdieff then checks every value against the model, and the first column is the result of that check, not the intention. The two agree here, but a model that allows null would list the `null` target as admitted. For a map, generation also builds a value missing each required entry, and a wrong value for each entry, as the full thermostat output in the getting-started guide shows for the `configure` action.

The **corpus evidence** summary counts retained candidates by what they establish.
Here both boundary candidates are admitted. A pattern could reject a length
boundary candidate, leaving it under `rejected_candidate` instead. Complete
generation does not promise an admitted example for every constraint or Choice
alternative. See [corpus evidence](../generation-contract.md#corpus-evidence)
for the categories and their limits.

For non-interactive metadata, select `--kind context` and its `sdfContext` pointer. Context requires explicit selection and uses the same value-checking rules, including unresolved nullability when `nullable` is absent. The [Context round trip](../context-contract.md#generate-and-replay-context-vectors) shows generation and offline replay with inherited and refined constraints.

Generation uses fixed target and candidate ordering within its supported meanings
and processing limits. Pattern deadlines can leave work unfinished. Generated
values are representative tests rather than exhaustive coverage of every rule
or possible value. Save settled values as test vectors with `--format jsonl`,
which writes one JSON object per line and omits undetermined candidates:

<pre class="terminal"><code><span class="p">$</span> esdieff generate --document thermostat.sdf.json --kind property --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature --format jsonl thermostat.sdf.json &gt; vectors.jsonl
<span class="p">$</span> head -3 vectors.jsonl
{"kind":"property","pointer":"/sdfObject/Thermostat/sdfProperty/targetTemperature","value":null,"expected":"rejected","label":"null"}
{"kind":"property","pointer":"/sdfObject/Thermostat/sdfProperty/targetTemperature","value":5,"expected":"admitted","label":"minimum"}
{"kind":"property","pointer":"/sdfObject/Thermostat/sdfProperty/targetTemperature","value":30,"expected":"admitted","label":"maximum"}
</code></pre>

Each line carries the selector, the value and its settled result as `expected`. Feed the values to the device or app under test and assert that it accepts the admitted ones and refuses the rejected ones. `--format json` gives the same values inside the full report. The [generation contract](../generation-contract.md) lists every target.

## Check a log of recorded values

`esdieff trace` checks a **recorded trace**, a JSON Lines file of values your device or app actually produced, in one run. Each line names a definition the way the generated vectors do, carries the value and may say what you expected. Save this as `readings.jsonl`:

```json
{"kind":"property","pointer":"/sdfObject/Thermostat/sdfProperty/targetTemperature","value":21.5,"label":"evening"}
{"kind":"property","pointer":"/sdfObject/Thermostat/sdfProperty/targetTemperature","value":21.3,"label":"app bug"}
{"kind":"property","pointer":"/sdfObject/Thermostat/sdfProperty/currentTemperature","value":null,"label":"sensor offline"}
{"kind":"action_input","pointer":"/sdfObject/Thermostat/sdfAction/configure","value":{"targetTemperature":31,"mode":"heat"}}
{"kind":"event_output","pointer":"/sdfObject/Thermostat/sdfEvent/alarm","value":{"temperature":85,"reason":"overheat"},"expected":"rejected"}
```

<pre class="terminal"><code><span class="p">$</span> esdieff trace --document thermostat.sdf.json --trace readings.jsonl thermostat.sdf.json
<b>Recorded trace</b> esdieff.trace/1: <span class="r">failed</span>
<b>Entries:</b> 3 <span class="g">passed</span>, 2 <span class="r">failed</span>, 0 <span class="y">undetermined</span>
<b>Admissions:</b> 2 admitted, 3 rejected, 0 undetermined

line 2 property "/sdfObject/Thermostat/sdfProperty/targetTemperature": <span class="r">failed</span>
  label "app bug"
  admission rejected
  error sample.multiple at "/value" bytes 206..210: Sample is not an exact multiple of the positive step

line 4 action_input "/sdfObject/Thermostat/sdfAction/configure": <span class="r">failed</span>
  admission rejected
  error sample.bound at "/value/targetTemperature" bytes 459..461: Sample violates an exact numerical bound
</code></pre>

Only the failed lines are listed here, each with its line number, the byte range
in the trace file and the rule it broke. Line 1 passed because 21.5 is admitted.
Line 3 passed because the current temperature is nullable. Line 5 passed too:
the alarm's temperature of 85 is above its maximum of 80, and the line expected
rejection. Lines 2 and 4 failed because their omitted `expected` members default
to admitted, while the actual values were rejected. Undetermined entries are
also listed when present.

Each line is checked on its own, so one malformed line fails only itself. The
model is composed once, and each entry has its own Sample-value checking
allowance. File-size and entry-count limits bound the trace as a whole. The
report keeps aggregate counts even when individual entry details do not fit.
The [trace contract](../trace-contract.md) describes the line format and limits.

## Replay generated vectors against a new revision

Generated vectors are a recorded trace, so `esdieff trace` replays them. With the
same supported meanings and enough processing allowance, their settled
admissions match the recorded expectations. This thermostat replay passes:

<pre class="terminal"><code><span class="p">$</span> esdieff trace --document thermostat.sdf.json --trace vectors.jsonl thermostat.sdf.json
<b>Recorded trace</b> esdieff.trace/1: <span class="g">passed</span>
<b>Entries:</b> 10 <span class="g">passed</span>, 0 <span class="r">failed</span>, 0 <span class="y">undetermined</span>
<b>Admissions:</b> 2 admitted, 8 rejected, 0 undetermined
</code></pre>

Create a revision that raises the maximum to 35 and drops the half-degree step:

```sh
sed 's/"maximum": 30/"maximum": 35/g; s/"multipleOf": 0.5, //g' thermostat.sdf.json > revised.sdf.json
```

Against this revision, the vectors whose settled admission changed fail:

<pre class="terminal"><code><span class="p">$</span> esdieff trace --document revised.sdf.json --trace vectors.jsonl revised.sdf.json
<b>Recorded trace</b> esdieff.trace/1: <span class="r">failed</span>
<b>Entries:</b> 8 <span class="g">passed</span>, 2 <span class="r">failed</span>, 0 <span class="y">undetermined</span>
<b>Admissions:</b> 4 admitted, 6 rejected, 0 undetermined

line 5 property "/sdfObject/Thermostat/sdfProperty/targetTemperature": <span class="r">failed</span>
  label "above_maximum"
  admission admitted, expected rejected
  error trace.expected at "/expected" bytes 648..658: The definite admission differs from the Expected admission

line 6 property "/sdfObject/Thermostat/sdfProperty/targetTemperature": <span class="r">failed</span>
  label "not_multiple"
  admission admitted, expected rejected
  error trace.expected at "/expected" bytes 791..801: The definite admission differs from the Expected admission
</code></pre>

This is a quick way to see what a model change does to values you already rely on. [Compare two revisions](revision-comparison.md) explains the same change from the app's point of view, and [Repeat checks and share reports](checks-and-reports.md) saves sample checks in a plan to run again.
