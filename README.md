# Esdieff

**Put device models to work.**

Esdieff helps you explore, check and test connected-device models written in
[Semantic Definition Format (SDF)](https://www.rfc-editor.org/info/rfc9880/).
Use it with models you create, receive from a supplier or find in a public
collection. It provides a command-line tool and a Rust library.

## Why Esdieff

- **Understand an interface.** Explore a device's readings, settings, commands
  and events, including definitions shared across files.
- **Catch model mistakes.** Find invalid definitions and broken references,
  with findings that point back to their source.
- **Review changes before integration.** Compare model revisions and see which
  changes would break the apps using a device.
- **Turn models into tests.** Generate valid, boundary and invalid values, and
  check recorded values against the same model.
- **Reuse shared definitions.** Assemble new models from existing capabilities
  and apply repeatable edits.
- **Automate your checks.** Run saved checks in CI, produce JSON and SARIF
  reports, or embed the library in your own tools.

## Install

Install a prebuilt release with your package manager. WinGet becomes available
after its listing is approved.

| Platform | Installation |
| --- | --- |
| macOS 15+ | `brew install pederbe/tap/esdieff` |
| Linux | [Install with apt or dnf](docs/getting-started/README.md#linux) |
| Windows | `winget install --id PederBergan.Esdieff --exact` |

With [Rust](https://rustup.rs) installed, you can also build the command using
`cargo install --locked esdieff`. The
[installation guide](docs/getting-started/README.md#install-esdieff) covers
updates, downloads and other options.

## Quick start

Save the included [thermostat model](examples/thermostat-check/rich.sdf.json) as
`thermostat.sdf.json`, then check it and generate test values.

```sh
esdieff check thermostat.sdf.json
esdieff generate --document thermostat.sdf.json thermostat.sdf.json
```

The model describes temperature readings, a target setting, a configuration
command and an alarm event. Generated cases include allowed temperatures, values
outside the range and values that break its half-degree step.

The [worked example](docs/getting-started/README.md#try-the-thermostat-example)
walks through the results.

## Learn more

- [User guides](docs/guides/inspect-and-check.md) for inspecting models, testing
  values, comparing revisions and automating checks.
- [Rust library](docs/source-workflow.md) for using Esdieff in another application.
- [SDF coverage](docs/model-coverage-boundaries.md) for detailed support and limits.
- [Changelog](CHANGELOG.md) for changes between versions.

## Contributing

Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md)
and the [security policy](SECURITY.md).

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.

---

Created by [Peder Bergan](https://pederbe.dev).
