# Changelog

## [Unreleased]

## [0.2.0] - 2026-10-30

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

### Source changes

- Activate weekly releases and repair live package checks (#13) ([191cdc1](https://github.com/pederbe/esdieff/commit/191cdc1fdd7ffc9055eb09305cbe77ab85304cbf))
- Update daily dependencies (#14) ([66f4ac7](https://github.com/pederbe/esdieff/commit/66f4ac7a944d66dc1439cb90246c95ed475cd03e))
- Lead Windows installation with the available release ZIP (#15) ([9d664a4](https://github.com/pederbe/esdieff/commit/9d664a499446e70ba9daf2914542347be754915f))
- Complete automatic WinGet catalog submission (#16) ([a88850f](https://github.com/pederbe/esdieff/commit/a88850fe8bc17d5b79a71d553741749cf686e85d))
- Make Linux installation one command (#17) ([fa1101b](https://github.com/pederbe/esdieff/commit/fa1101b5443c1d14a0b3ffc0183d109d9cedd5c3))
- Defer homepage JavaScript and avoid a forced style read (#18) ([82accac](https://github.com/pederbe/esdieff/commit/82accac60ffa7904067a7153f57abbf20b408210))
- Reuse one pattern engine per trace or generation request (#20) ([d2e386f](https://github.com/pederbe/esdieff/commit/d2e386f892cdb0993d9d41ac5cc02639f5fe2aa8))
- Update Rust crate hotpath to v0.30.0 (#21) ([3acefef](https://github.com/pederbe/esdieff/commit/3acefef3e20a2d1f6adee14dffbb07d317c9b08e))
- Add social sharing card and complete website metadata (#22) ([773a663](https://github.com/pederbe/esdieff/commit/773a66352a3fd3b1de3cec232e004a74eb05a209))
- Migrate domain glossary and clarify SDF terminology (#23) ([446d7a5](https://github.com/pederbe/esdieff/commit/446d7a5854f4680d5ad21c6a6ffb1e4ccd9ca82d))
- Deploy merged website changes automatically after CI (#24) ([5fee459](https://github.com/pederbe/esdieff/commit/5fee45940c91b54922c21299065b7f8e305ff71d))
- Document automatic website deployment logs (#25) ([d1ece2d](https://github.com/pederbe/esdieff/commit/d1ece2d06dd7b2bf8be763721560f6c40baeeeea))
- Document Esdieff workflows with an interactive process map (#26) ([31132ba](https://github.com/pederbe/esdieff/commit/31132ba9c0eebccd6d471350a340a239d6b0754a))
- Explain undetermined sample admissions in recorded traces (#27) ([b54cdd0](https://github.com/pederbe/esdieff/commit/b54cdd029f2c36b8c72521641753766db65dbad1))
- Check and inspect inline Context definitions (#28) ([9f5cd82](https://github.com/pederbe/esdieff/commit/9f5cd82d4cefb1d3bb7565bfefb3e19eed9d5575))
- Support Context reuse and refinement (#6) (#29) ([4105ab8](https://github.com/pederbe/esdieff/commit/4105ab8bd48bbfbfd58852dc7bbfd107fc53f832))
- Support Context authoring and authored-quality editing (#7) (#30) ([8730bc7](https://github.com/pederbe/esdieff/commit/8730bc789b9a454c0efff26c76d788f3b825daa2))
- Check selected Context values (#8) (#31) ([9116d3f](https://github.com/pederbe/esdieff/commit/9116d3f92b64918b108c028ca55077cd2621f57d))
- Generate and replay Context test vectors (#32) ([8b9e9fe](https://github.com/pederbe/esdieff/commit/8b9e9fef705829fcae1af9d913ece86db9264ed5))
- Compare revisions containing Context (#33) ([2e60847](https://github.com/pederbe/esdieff/commit/2e6084792ec6a3409630bae40af379a5fb55d284))
- Preserve all five Context draft examples with a revision comparison (#34) ([be91e9a](https://github.com/pederbe/esdieff/commit/be91e9a1f0f9fdbe30092b98d933b94999ffae21))
- Deepen Context structural assessment (#35) ([ec2e379](https://github.com/pederbe/esdieff/commit/ec2e379722e9620342178f45b1f499d1bd024346))
- Complete v0.2 changelog and require release-note coverage (#36) ([d5bc61f](https://github.com/pederbe/esdieff/commit/d5bc61f15b1822b3849e1217741a2cb3ff498c31))

[Full comparison](https://github.com/pederbe/esdieff/compare/v0.1.0...v0.2.0)

## [0.1.0] - 2026-10-27

The first public release of Esdieff provides a Rust library and command-line
tool for working with connected-device models in Semantic Definition Format.

- Inspect and check supplied models, shared definitions and references.
- Compare model revisions and review changes that affect applications.
- Generate test values and check samples and recorded values against models.
- Author and edit composed models while retaining source information.
- Save human, JSON, SARIF and Markdown reports for supported operations.
- Acquire an explicitly selected GitHub document with a pinned revision and digest.
