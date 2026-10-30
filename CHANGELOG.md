# Changelog

## [Unreleased]

### Added

- Check and inspect Context metadata, such as serial numbers and installation
  locations, under Objects and Things. Support follows nonaffordance draft 06
  alongside RFC 9880. See the [Context guide](docs/context-contract.md) for the
  supported draft semantics and limitations. ([#28](https://github.com/pederbe/esdieff/pull/28))
- Reuse and refine Context definitions through local and supplied-document
  references, including inherited metadata, additions and deletions.
  ([#29](https://github.com/pederbe/esdieff/pull/29))
- Author Things from Objects containing Context metadata, refine their Context
  descriptions and edit supported qualities on authored Context definitions.
  ([#30](https://github.com/pederbe/esdieff/pull/30))
- Check supplied Context values with `sample --kind context` or a Check plan,
  using their effective inherited and refined constraints.
  ([#31](https://github.com/pederbe/esdieff/pull/31))
- Generate deterministic test values for selected Context metadata and replay
  Context vectors against the same or a changed model.
  ([#32](https://github.com/pederbe/esdieff/pull/32))
- Show added, removed and changed Context definitions in revision comparisons.
  Compatibility assesses client interactions only. A compatible result does not
  establish compatibility for consumers of Context metadata.
  ([#33](https://github.com/pederbe/esdieff/pull/33))

### Changed

- Diff JSON reports use version `1.6` instead of `1.5`, and the comparison
  contract is `esdieff.revision-comparison/3` instead of `/2`. Report consumers
  that require the earlier identities must be updated, including for comparisons
  without Context. Reports now state the client-interaction compatibility scope.
  ([#33](https://github.com/pederbe/esdieff/pull/33))
- Reuse one pattern engine per trace or generation request. The measured
  pattern-heavy workloads check recorded traces about 6 times faster and generate
  test values about twice as fast. Rarely, a value with no pattern polls to spare
  is now undetermined in a trace or generation request where it would settle on
  its own. ([#20](https://github.com/pederbe/esdieff/pull/20))

### Fixed

- Explain why a recorded trace value is undetermined, including incomplete
  checking alongside model errors.
  ([#27](https://github.com/pederbe/esdieff/pull/27))

### Installation and documentation

- Install on Linux with one command that configures the signed package source
  and installs the package. ([#17](https://github.com/pederbe/esdieff/pull/17))
- Lead Windows installation with the available release ZIP and automate WinGet
  catalogue submissions. External catalogue admission remains separate.
  ([#15](https://github.com/pederbe/esdieff/pull/15),
  [#16](https://github.com/pederbe/esdieff/pull/16))
- Add a workflow guide with an interactive process map, plus all five Context
  draft examples and an example revision comparison.
  ([#26](https://github.com/pederbe/esdieff/pull/26),
  [#34](https://github.com/pederbe/esdieff/pull/34))

## [0.1.0] - 2026-10-27

The first public release of Esdieff provides a Rust library and command-line
tool for working with connected-device models in Semantic Definition Format.

- Inspect and check supplied models, shared definitions and references.
- Compare model revisions and review changes that affect applications.
- Generate test values and check samples and recorded values against models.
- Author and edit composed models while retaining source information.
- Save human, JSON, SARIF and Markdown reports for supported operations.
- Acquire an explicitly selected GitHub document with a pinned revision and digest.
