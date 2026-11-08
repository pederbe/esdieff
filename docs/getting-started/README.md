# Install and try a model

This guide installs Esdieff and runs it on a supplied thermostat model. No
device is needed. Installation and downloading the example use the network.
The subsequent model operations run locally without sending your documents.

## Install Esdieff

Esdieff supports macOS 15+, Linux and Windows. Prebuilt releases are available
through Homebrew, apt and dnf, and as direct downloads.

### macOS

Install the prebuilt command from the Esdieff Homebrew tap:

```sh
brew install pederbe/tap/esdieff
```

Update with `brew update && brew upgrade esdieff`. Remove it with
`brew uninstall esdieff`.

### Windows

Download the [Windows ZIP](https://github.com/pederbe/esdieff/releases/latest/download/esdieff-x86_64-pc-windows-msvc.zip),
extract it, and add the folder containing `esdieff.exe` to your `PATH`. Open a
new terminal and check the installation:

```powershell
esdieff --version
```

To update, replace the extracted files with a newer release. To uninstall,
remove the folder and its entry from your `PATH`.

A WinGet listing is being prepared. WinGet installation will become available
after the community listing is approved.

### Linux

Install on Debian, Ubuntu or Fedora:

```sh
curl -fsSL https://raw.githubusercontent.com/pederbe/esdieff/main/scripts/install.sh | sh
```

Updates arrive through your package manager.

### Direct downloads and Cargo

[Download a release archive](https://github.com/pederbe/esdieff/releases), extract
it and put the command on your `PATH`. Download a newer archive to update it.
Remove the extracted command to uninstall it.

Rust users can install a prebuilt release with:

```sh
cargo binstall esdieff
```

To build from source, install a current [Rust toolchain](https://rustup.rs) and run:

```sh
cargo install --locked esdieff
```

This builds the `esdieff` command and puts it in Cargo's binary directory, normally `~/.cargo/bin`. Make sure that directory is on your `PATH`.

From a checkout of the repository, install the same way with a path instead:

```sh
cargo install --locked --path .
```

The walkthrough output below describes the current repository checkout.
Release 0.3.0 predates the `esdieff.generation/3` report, and 0.2.0 also
predates the reference and schema commands. To reproduce the current output, use the checkout
installation above. Released versions may print earlier report identities and
different counts.

Repeat the Cargo installation command to update. Use `cargo uninstall esdieff`
to remove it.

## Check that it works

<pre class="terminal"><code><span class="p">$</span> esdieff --help
<b><span class="h">Usage:</span></b>
<span class="c">esdieff</span> <span class="c">check</span> <span class="c">--plan</span> PLAN [<span class="c">--format</span> human|json]
<span class="c">esdieff</span> <span class="c">check</span> [<span class="c">--directory</span> DIR] [<span class="c">--recursive</span>] [<span class="c">--exclude</span> RELATIVE_PATH] [<span class="c">--root</span> INPUT] [<span class="c">--assessment</span> initial-v1|expanded-modified-v1|expanded-units-v1|expanded-content-format-v1|expanded-pattern-syntax-v1|expanded-numeric-v1|expanded-consistency-v1|full-baseline] [<span class="c">--registry-knowledge</span> recorded|withheld] [<span class="c">--strict-warnings</span>] [<span class="c">--format</span> human|json|sarif] [INPUT...]
</code></pre>

The help continues with one line per command. If the command is not found,
check that its installation folder is on your `PATH` and open a new terminal.
Cargo installations normally use `~/.cargo/bin`.

## Try the thermostat example

The example is a room thermostat with two properties, one action and one event. Its writable `targetTemperature` accepts numbers from 5 to 30 °C in half-degree steps:

```json
"targetTemperature": {
  "description": "Requested room temperature in half-degree increments",
  "type": "number", "nullable": false, "minimum": 5, "maximum": 30,
  "multipleOf": 0.5, "unit": "Cel", "readable": true, "writable": true
}
```

Download [rich.sdf.json](https://github.com/pederbe/esdieff/blob/main/examples/thermostat-check/rich.sdf.json) from the repository, or copy it from a checkout, and save it as `thermostat.sdf.json` in an empty directory. Take the rich model, not the smaller `thermostat.sdf.json` next to it in the repository: that one has no half-degree step, so it accepts 5.25. Then generate test values from its rules. The command names the document to generate for, followed by every model file to supply, which here is the same file:

<pre class="terminal"><code><span class="p">$</span> esdieff generate --document thermostat.sdf.json thermostat.sdf.json
<b><span class="h">Generated values</span></b> esdieff.generation/3: <b><span class="g">✔ passed</span></b>
<b>Values:</b> 15 <b><span class="g">admitted</span></b> · 60 <b><span class="r">rejected</span></b> · 0 <b><span class="y">undetermined</span></b>
<b>Descriptions:</b> 4
Corpus evidence counts kept values, not constraints or individual Choice alternatives. Complete generation does not imply complete constraint coverage.

<b><span class="h">property /sdfObject/Thermostat/sdfProperty/currentTemperature</span></b>: <b><span class="g">✔ complete</span></b>
  ...

<b><span class="h">property /sdfObject/Thermostat/sdfProperty/targetTemperature</span></b>: <b><span class="g">✔ complete</span></b>
  Corpus evidence: 0 other admitted examples, 2 admitted boundary candidates, 7 rejected outside candidates, 1 other rejected candidates, 0 undetermined
  <b><span class="r">rejected    </span></b> null          null (sample.null) [rejected_candidate]
  <b><span class="g">admitted    </span></b> minimum       5 [admitted_boundary]
  <b><span class="g">admitted    </span></b> maximum       30 [admitted_boundary]
  <b><span class="r">rejected    </span></b> below_minimum 4 (sample.bound) [rejected_outside]
  <b><span class="r">rejected    </span></b> above_maximum 31 (sample.bound) [rejected_outside]
  <b><span class="r">rejected    </span></b> not_multiple  5.25 (sample.multiple) [rejected_outside]
  <b><span class="r">rejected    </span></b> wrong_type    "" (sample.type) [rejected_outside]
  <b><span class="r">rejected    </span></b> wrong_type    true (sample.type) [rejected_outside]
  <b><span class="r">rejected    </span></b> wrong_type    {} (sample.type) [rejected_outside]
  <b><span class="r">rejected    </span></b> wrong_type    [] (sample.type) [rejected_outside]

<b><span class="h">action_input /sdfObject/Thermostat/sdfAction/configure</span></b>: <b><span class="g">✔ complete</span></b>
  ...
</code></pre>

The output is trimmed to the `targetTemperature` block. The full report lists 75 values across the two properties, the `configure` action input and the `alarm` event output.

Each line is one generated value. The middle column names the rule the value aims at, and the first column says what the model does with it:

- `5` and `30` aim at the `minimum` and `maximum` and are **admitted** by this selected description. They are boundary values to include in a device or application test.
- `31` aims just above the maximum and is **rejected**, with the condition code `sample.bound` naming the rule it breaks.
- `5.25` is a quarter-degree value, so it is **rejected** by the half-degree step, `sample.multiple`.

Esdieff checks every generated value against the selected model description the
same way it checks a sample you supply. The labels record that assessment,
independently of the target the generator aimed at. A value whose admission
Esdieff cannot establish is listed as undetermined.

Next, check the model itself. `--assessment full-baseline` requests every model obligation Esdieff records from RFC 9880:

<pre class="terminal"><code><span class="p">$</span> esdieff check --assessment full-baseline thermostat.sdf.json
<b><span class="h">Esdieff:</span></b> <b><span class="g">✔ passed</span></b>
<b>Complete:</b> true
<b>Errors:</b> 0 · <b>warnings:</b> 0 · <b>notices:</b> 0 · <b>incomplete checks:</b> 0
<b>Scope:</b> catalogue
<b>Broader RFC coverage:</b> complete
<span class="m">Recorded registry policy: use_recorded</span>
<b>Unit guidance:</b> complete
<b>Assessment area</b> unit-spelling/1: assessed
<b>Assessment area</b> unit-registry-guidance/1: assessed
<b>Unit counts:</b> known_primary 4 · known_secondary 0 · unknown 0 · unavailable 0 · legacy 0 · uri_names 0
<b>Assessment area</b> content-format-syntax/1: not_applicable
<b>Assessment area</b> content-format-registry/1: not_applicable
<span class="m">Registry iana.senml-units: available; snapshot iana.senml-units/2026-10-05/6246352c590e933b; scope senml-units; entries_count 67</span>
<span class="m">Registry extraction_policy esdieff.unit-section-positives/1; digest 6246352c590e933b72c25ef9f363171e40fea347bb7e23d08b04f9436894ed34; raw_digest 3eff03640c5a5d2abd09e330488aeb9fae0b2766ea69b1114295bbec5c44e754; source_updated 2026-02-02; retrieved_at 2026-10-05T16:49:51Z</span>
<span class="m">Requested assessment: rfc9880/base-1</span>
<span class="m">Baseline: rfc9880/base-1 (RFC 9880)</span>
<span class="m">Acceptance restrictions: A001, A002; strict warnings: false</span>
<span class="m">Requested sources: [1]</span>
<span class="m">Requested inventory: esdieff.full-model/1</span>
<b>Requested coverage (assessment_area):</b> total 13 · assessed 10 · not applicable 3 · unassessed 0 · undetermined 0
<span class="m">Requirement inventory: rfc9880.model/1</span>
<b>Requirement coverage:</b> total 54 · assessed 28 · not applicable 26 · unassessed 0 · undetermined 0
<span class="m">Source 1 ["input1"]: acquired / usable (1932 retained bytes, content complete: true)</span>
<b>Diagnostic details omitted:</b> 0
</code></pre>

The first line is the verdict. `Complete: true` means the requested work
finished, and the coverage lines account for every one of the 54 recorded
requirements as assessed or not applicable to this model. The unit lines show
that the four `Cel` unit names were found in the recorded SenML units registry
snapshot, so the check needed no network access. The command exits with status
0 for a passed check, 1 for an error or a warning under strict-warning policy,
and 3 for unfinished work when no failing finding takes precedence. Notices
and ordinary warnings do not by themselves fail the check.

## Next steps

- [Inspect and check a model](../guides/inspect-and-check.md) supplies shared definitions, reads findings and follows them to their source.
- [Test values against a model](../guides/testing-values.md) checks single samples, saves generated values as test vectors and checks recorded logs.
- [Compare two revisions](../guides/revision-comparison.md) finds the changes that break the apps using a device.
- [Repeat checks and share reports](../guides/checks-and-reports.md) saves these checks as a plan and runs them in CI.
