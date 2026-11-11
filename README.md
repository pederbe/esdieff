# Esdieff

**Put device models to work.**

Esdieff helps you explore, check and test connected-device models written in
[Semantic Definition Format (SDF)](https://www.rfc-editor.org/info/rfc9880/).
Use it with models you create, receive from a supplier or find in a public
collection. It provides a command-line tool and a Rust library.

Here it turns the rules for a thermostat's target temperature, 5 to 30 °C in
half-degree steps, into values to test with.

```console
$ esdieff generate --document thermostat.sdf.json --kind property \
    --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature thermostat.sdf.json
✔ Generated values esdieff.generation/3: passed
Model: "thermostat.sdf.json" (source 1)
Selection: property at "/sdfObject/Thermostat/sdfProperty/targetTemperature"
Values: 2 admitted · 8 rejected · 0 undetermined
Descriptions: 1
Corpus evidence counts kept values, not constraints or individual Choice alternatives. Complete generation does not imply complete constraint coverage.

✔ property /sdfObject/Thermostat/sdfProperty/targetTemperature: complete
  Corpus evidence: 0 other admitted examples, 2 admitted boundary candidates, 7 rejected outside candidates, 1 other rejected candidates, 0 undetermined
  rejected     null          null (sample.null) [rejected_candidate]
  admitted     minimum       5 [admitted_boundary]
  admitted     maximum       30 [admitted_boundary]
  rejected     below_minimum 4 (sample.bound) [rejected_outside]
  rejected     above_maximum 31 (sample.bound) [rejected_outside]
  rejected     not_multiple  5.25 (sample.multiple) [rejected_outside]
  rejected     wrong_type    "" (sample.type) [rejected_outside]
  rejected     wrong_type    true (sample.type) [rejected_outside]
  rejected     wrong_type    {} (sample.type) [rejected_outside]
  rejected     wrong_type    [] (sample.type) [rejected_outside]
```

## Highlights

- **Understand an interface.** Explore a device's readings, settings, commands
  and events, including definitions shared across files.
- **Catch model mistakes.** Find invalid definitions and broken references,
  with findings that point back to their source.
- **Review changes before integration.** Compare model revisions and see which
  changes would break the apps using a device.
- **Turn models into tests.** Generate valid, boundary and invalid values, and
  check recorded values against the same model.
- **Automate your checks.** Run saved checks in CI, produce JSON and SARIF
  reports, or embed the library in your own tools.

Esdieff works on model files and JSON values. It does not connect to devices
or test network protocols.

## Installation

| Platform | Installation |
| --- | --- |
| macOS 15+ | `brew install pederbe/tap/esdieff` |
| Linux | `curl -fsSL https://raw.githubusercontent.com/pederbe/esdieff/main/scripts/install.sh \| sh` |
| Windows | [Download the Windows ZIP](https://github.com/pederbe/esdieff/releases/latest/download/esdieff-x86_64-pc-windows-msvc.zip) |

With [Rust](https://rustup.rs) installed, you can also build the command using
`cargo install --locked esdieff`. On Windows, extract the ZIP and add the
folder containing `esdieff.exe` to your `PATH`. Run `esdieff --version` to
check the installation. It prints the installed version.

## Everyday use

Save the included [thermostat model](examples/thermostat-check/rich.sdf.json) as
`thermostat.sdf.json` to try the commands that name it. The model describes
temperature readings, a target setting, a configuration command and an alarm
event. The other commands use file and directory names of your own.

Check a model. The first line of the report is the verdict.

```sh
esdieff check thermostat.sdf.json
```

Check a model that uses definitions from other files. Supply every file and
name the model to check with `--root`.

```sh
esdieff check --root device.sdf.json device.sdf.json types.sdf.json
```

Generate test values for the whole model. Generated cases include allowed
temperatures, values outside the range and values that break its half-degree
step. The [worked example](https://esdieff.com/docs/getting-started/#try-the-thermostat-example)
walks through the results.

```sh
esdieff generate --document thermostat.sdf.json thermostat.sdf.json
```

Check one value against one definition. Here `too-hot.json` holds the number
35, which the model rejects.

```sh
echo 35 > too-hot.json
esdieff sample --document thermostat.sdf.json --kind property \
    --pointer /sdfObject/Thermostat/sdfProperty/targetTemperature \
    --sample too-hot.json thermostat.sdf.json
```

Compare two revisions and see which changes break the apps using the device.

```sh
esdieff diff --old v1/thermostat.sdf.json --new v2/thermostat.sdf.json
```

Check every model in a directory from a script or CI job.

```sh
esdieff check --directory models --recursive --strict-warnings
```

Commands exit with status 0 when the work passed, 1 when a finding failed it,
2 for an invalid invocation, 3 when some work could not be settled, such as a
missing model file or dependency, and 4 when a host or output operation failed, such as writing
the report. A CI step therefore fails on anything but a pass.

| Option | What it does |
| --- | --- |
| `--root INPUT` | Check this model and the supplied definitions it needs |
| `--assessment full-baseline` | Check every model requirement Esdieff records from RFC 9880 |
| `--strict-warnings` | Fail the check on warnings too |
| `--directory DIR` | Check every file ending in `.sdf.json` in a directory, and with `--recursive` in its subdirectories |
| `--format json` | Write a JSON report for scripts |
| `--format sarif` | Write a SARIF log for code-scanning tools, with `check` and `diff` |

Run `esdieff --help` for every command and option.

## Learn more

- [Install and try a model](https://esdieff.com/docs/getting-started/) covers
  updates, downloads and other ways to install, then walks through the
  thermostat results.
- [User guides](https://esdieff.com/docs/guides/inspect-and-check) for
  inspecting models, testing values, comparing revisions and automating checks.
- [Compose and edit models](https://esdieff.com/docs/guides/authoring-and-editing)
  assembles new models from existing capabilities and applies repeatable edits.
- [Command line reference](https://esdieff.com/docs/command-line) lists every
  command.
- [Rust library](https://esdieff.com/docs/rust-library) for using Esdieff in
  another application.
- [How Esdieff works](https://esdieff.com/docs/guides/how-esdieff-works) shows
  the flow from supplied files to model checks, comparisons and test values.
- [SDF coverage](https://esdieff.com/docs/model-coverage-boundaries) for detailed
  support and limits.
- [Troubleshooting](https://esdieff.com/docs/troubleshooting) when a command
  does not do what you expect.
- [Changelog](CHANGELOG.md) for changes between versions.

## Contributing

Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md)
and the [security policy](SECURITY.md).

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.

---

Created by [Peder Bergan](https://pederbe.dev).
