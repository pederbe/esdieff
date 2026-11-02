# Changelog

## Unreleased

### Development

- Follow rolling stable Rust and install the standard analysis, source, formatting and lint components with the project toolchain.

### Changed

- Classify Property `readable`, `writable` and `observable` changes with
  `--perspective implementer`. Enabling an interaction is breaking and disabling
  it is compatible. Client comparison remains the default with its existing
  classifications. Comparison contract `/9` replaces `/8`, retaining diff
  envelope `1.7` and acknowledgement format `1`. Review intended capability
  differences and regenerate old undetermined acknowledgements where needed.

- Classify grouping-array count changes with `--perspective implementer`.
  Narrowing the permitted counts is breaking, widening is compatible, and
  client comparison remains the default with its existing classifications.
  Comparison contract `/8` replaces `/7`, retaining diff envelope `1.7` and
  acknowledgement format `1`. Review intended differences and regenerate old
  undetermined count acknowledgements where needed.

- Render captured documentation terminal examples with a Rust helper instead of
  Perl. The existing `scripts/ansi-to-html.sh` pipeline and terminal colours stay
  the same, with correctly balanced markup for combined styles and resets.
  Documentation authors need the repository's Rust toolchain to run the helper.

- Replace the door-lock simulator example with a short library example that
  checks the model and eight sample values. Run `cargo run --example lock-demo`
  without a seed argument. The example no longer simulates device behavior.

- Remove `esdieff acquire` and its network transport. Obtain model files with
  your preferred tools, then pass the local files to Esdieff. Acquisition
  receipts are no longer produced. Ordinary local inputs, source metadata and
  model operations keep their existing behavior.

- Replace the JavaScript pattern runtime with the Rust Corvus engine. Pattern
  matching uses fixed internal capacity safeguards and a cooperative backtracking
  timeout. Remove the public `Limits` type and quota-selecting constructors.
  Use `Catalogue::from_inputs`, `Catalogue::take_inputs` and `JsonBuilder::new`,
  and call `DifferenceAcknowledgements::from_json(bytes)` without a limits
  argument. Semantic request options remain unchanged.
  `NumberWork` is now internal. Use `ExactNumber::compare(other)` returning
  `Ordering`, and location `pointer()` accessors without a byte-budget argument.
  Previously unsupported QuickJS capture/name capacities can now complete.
  Report envelopes, condition codes and assessment inventories are unchanged.

- Classify added and removed declarations inside corresponding groupings with
  `--perspective implementer`. Added mandatory obligations are breaking,
  optional additions and established removals are compatible. Incomplete later
  requiredness stays undetermined. Comparison contract `/7` replaces `/6`, with
  diff envelope `1.7` and acknowledgement format `1` unchanged. Review intended
  differences and regenerate old undetermined acknowledgements where needed.

- Classify changes to declaration obligations with `--perspective implementer`.
  Making a corresponding declaration mandatory is breaking, releasing
  obligations alone is compatible, and equivalent resolved requirements produce
  no difference. Declaration presence follows the rules above. Comparison
  contract `/9` includes these rules, with diff envelope `1.7` and acknowledgement
  format `1` unchanged. Review intended differences and regenerate old
  undetermined acknowledgements where classifications have changed.

- Compare revision data changes from an existing device implementation's
  perspective with `diff --perspective implementer`, the library's
  `ComparisonRequest::perspective`, or a saved diff case's `perspective`.
  Client comparison remains the default. Reports state the perspective.
  Other structural implementation obligations remain undetermined and Context
  metadata compatibility remains outside scope. Diff JSON readers must accept
  envelope `1.7` with `policy.perspective` and comparison contract `/9`.
  Whole Action/Event input or output presence changes remain undetermined for
  implementers. Explicit rename mapping remains outside the supported workflow.

### Fixed

- Generate Homebrew recipes that pass current Homebrew style and audit checks.
  Existing installations keep the same release archives and checksums.

- Correct documentation and requirement descriptions for metadata absence,
  implicit defaults, report contracts and current capability boundaries.
  Inspection guidance now distinguishes authored, effective and requirement
  views and uses consistent SDF terminology.
  ([#54](https://github.com/pederbe/esdieff/pull/54),
  [#56](https://github.com/pederbe/esdieff/pull/56))

### Added

- Report Choice admission evidence for generated corpora through borrowed library
  accessors, human output and optional JSON `choice_evidence` detail. Named
  alternatives identify a retained admitted value and its actual sample pointer.
  Skipped alternatives establish no rejection or impossibility. Collection has
  separate limits and cannot displace existing generated values or descriptions.
  Existing generation, admission and Trace contracts remain unchanged.

- Explain generated corpora with admitted examples and boundary candidates,
  rejected outside and other candidates, and undetermined values. The library,
  human report and optional JSON `evidence` fields share these observations.
  Counts describe retained values rather than all constraints or Choice
  alternatives. Report and generation identities and JSON Lines remain unchanged.
- Explore a shared definition with `esdieff reference --uses` or
  `ReferenceRequest::with_uses()`. The Markdown reference lists effective
  consumers across supplied models, inherited and overridden scalar values,
  and links to current, previous and importing sources. Use navigation accepts
  named definitions including `sdfData`. Ordinary selection and report formats
  are unchanged.
- Export selected effective payload constraints as JSON Schema Draft 2020-12
  through `esdieff schema` and `Catalogue::export_schema`. The separate
  `esdieff.schema/1` report states fidelity and withholds schemas containing
  unsupported meaning. Existing commands and report formats are unchanged.
- Generate pattern-aware string representatives for a documented ASCII subset,
  combining length limits and required map entries. Revision comparison can use
  them as breaking witnesses. Generation now reports `esdieff.generation/2` and
  comparison `esdieff.revision-comparison/4`. Consumers checking those identities
  must accept the new values. Report schemas and recorded traces are unchanged.
- Generate a Markdown model reference for a selected Thing or Object with
  effective constraints, illustrative admitted values and source origins. Use
  `esdieff reference --document MODEL --pointer POINTER INPUT...` or the
  library’s `ReferenceRequest`. Existing commands and report formats are unchanged.

### Documentation and maintenance

- Add a guide to resolving missing and ambiguous name references, including
  namespace lookup and supplied-document examples. Connect shared-definition
  exploration to revision comparison in the authoring workflow.
  ([#52](https://github.com/pederbe/esdieff/pull/52),
  [#53](https://github.com/pederbe/esdieff/pull/53))
- Finalize source changelogs before weekly release preparation and record
  published release notes through pull requests. Keep source entries separate
  from published notes, and check the recorded notes against their release.
  These changes maintain release records without changing model operations.
  ([#37](https://github.com/pederbe/esdieff/pull/37),
  [#38](https://github.com/pederbe/esdieff/pull/38),
  [#39](https://github.com/pederbe/esdieff/pull/39),
  [#40](https://github.com/pederbe/esdieff/pull/40))
- Update optional Hotpath profiling from `0.30.0` to `0.30.3`, website tooling
  Wrangler from `4.154.0` to `4.156.0`, Node.js from `26.12.1` to `26.13.0`
  and the Homebrew setup action revision.
  ([#41](https://github.com/pederbe/esdieff/pull/41),
  [#66](https://github.com/pederbe/esdieff/pull/66))
- Move checks that depend on internal resource controls into the internal test
  suite, retaining public library and command-line coverage for the fixed
  resource API. Separate pattern semantics checks from the default deadline.
  ([#40](https://github.com/pederbe/esdieff/pull/40),
  [#73](https://github.com/pederbe/esdieff/pull/73))

## [0.3.0] - 2026-11-02

### Breaking changes

- feat: generate bounded pattern-aware string representatives ([72d1df1](https://github.com/pederbe/esdieff/commit/72d1df1936df96f689ee5cac0072690ccc9a40d5))
  Generate deterministic ASCII pattern-aware representatives combined with length limits and required sibling entries. Keep independent sample admission and documented unsupported-pattern fallbacks.
  
  Keep semantic comparison assertions independent of the default matching deadline and separately cover exhausted matching.
  
  Closes #43.
  
  BREAKING CHANGE: generation uses esdieff.generation/2 and revision comparison uses esdieff.revision-comparison/4. Consumers checking these contract identities must accept the new values. Report schemas and trace formats are unchanged.
- feat: compare revisions from the existing implementer perspective (#60) ([b58a3ee](https://github.com/pederbe/esdieff/commit/b58a3ee1492fe888e357709816c5888e50dc5a2a))
  Add an explicit implementer perspective to the library, direct CLI and saved diff cases. Preserve client comparison as the default, chronological evidence and scoped unknown results. Keep structural implementation obligations undetermined and Context metadata compatibility outside scope.
  
  Closes #47.
  
  BREAKING CHANGE: Diff reports use envelope 1.7 with required policy.perspective and comparison contract esdieff.revision-comparison/5. JSON readers must accept the new envelope. Acknowledgement format 1 remains unchanged.
- feat: classify implementer declaration presence (#64) ([503b306](https://github.com/pederbe/esdieff/commit/503b306caa629193873d14b989eef77bc2016bc7))
  Classify added and removed declarations inside corresponding groupings from the implementer perspective using complete later mandatory closures. Preserve unavailable correspondence, chronological reporting and existing client classifications.
  
  BREAKING CHANGE: Implementer declaration-presence classifications now follow comparison contract esdieff.revision-comparison/7. Previously undetermined differences can become breaking or compatible. Review intended differences and regenerate affected acknowledgements. Diff envelope 1.7 and acknowledgement format 1 remain unchanged.
  
  Closes #63.
- feat!: replace JavaScript patterns with Corvus and fixed safeguards (#73) ([63e8920](https://github.com/pederbe/esdieff/commit/63e8920a58ca5f4625f066575b57fb3c7641ee40))
  Replace the JavaScript pattern runtime with Corvus Unicode matching and fixed internal safeguards. Preserve model, sample, generation and trace conclusions, source locations and report formats. Retain internal failure-path coverage and ordinary public CLI tests.
  
  BREAKING CHANGE: Remove the public Limits and NumberWork types and quota-selecting constructors. Use Catalogue::from_inputs, Catalogue::take_inputs and JsonBuilder::new. ExactNumber::compare(other) returns Ordering, location pointer() accessors take no byte budget, and DifferenceAcknowledgements::from_json(bytes) takes no limits argument. Semantic request options remain supported.
  
  Closes #68.
- feat!: remove network acquisition from Esdieff (#76) ([64e6167](https://github.com/pederbe/esdieff/commit/64e61678cabe67b0d6f6def448b946dbf2b3cc48))
  Remove downloading from the installed command. Model operations consume supplied local files and bytes. Delete acquisition receipts, examples, transport dependencies and dedicated filesystem and license handling. Supersede the acquisition decision and reconcile current documentation and diagrams.
  
  BREAKING CHANGE: esdieff acquire and its acquisition receipt format are removed. Obtain model files with existing tools and pass local paths to Esdieff. Local source metadata and model operations retain their meanings.
  
  Closes #75.
- feat: classify implementer grouping counts (#80) ([ecc63c0](https://github.com/pederbe/esdieff/commit/ecc63c03bca6ae9a1612eeae1c154abab4a7ab5d))
  Preserve every earlier permitted modeled instance count for corresponding Object and Thing arrays. Reuse exact range comparison and effective composition, keeping client classifications unchanged. Record ADR 0058 and advance comparison contract to /8 while retaining diff envelope 1.7 and acknowledgement format 1.
  
  BREAKING CHANGE: Implementer grouping-count differences formerly reported as undetermined can now be breaking or compatible under comparison contract esdieff.revision-comparison/8. Review intended differences and regenerate old undetermined count acknowledgements where needed.
  
  Closes #65.

### Added

- feat: generate a readable Markdown model reference (#48) ([683c113](https://github.com/pederbe/esdieff/commit/683c11394090dd9a50189b5557063fed06cde01f))
  Add the selected library reference operation and CLI, bounded inert Markdown, admitted illustrative values, source navigation, documentation and focused public contract coverage.
  
  Closes #42.
- feat: export selected effective payloads as JSON Schema (#50) ([3c6284c](https://github.com/pederbe/esdieff/commit/3c6284c56b5f05458c0f9e87c46534dd7a0be487))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- feat: classify implementer declaration obligations (#62) ([9c4da0a](https://github.com/pederbe/esdieff/commit/9c4da0a29e3d168b84e7086d3b158cc6bd57f821))
  Classify new mandatory declarations as breaking for implementers and released obligations alone as compatible. Reuse resolved mandatory closures, preserving client defaults, endpoint boundaries and unavailable reasons.
  
  Advance the comparison contract to esdieff.revision-comparison/6. Diff envelope 1.7 and acknowledgement format 1 are unchanged. Intended differences need review and old undetermined acknowledgements may need regeneration.
  
  Closes #61.
- feat: classify implementer Property capability changes (#82) ([02edf56](https://github.com/pederbe/esdieff/commit/02edf56a26ad0649d41627f905b0bed01273c7a0))
  Closes #81
  
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>

### Other changes

- Update changelog for version 0.2.0 release (#37) ([f4c25f8](https://github.com/pederbe/esdieff/commit/f4c25f8159f38a7d04161b34e0b52519793a59da))
- Restore the changelog section for upcoming changes (#38) ([3ed530d](https://github.com/pederbe/esdieff/commit/3ed530de36b10a20e30e08add3776f443b023cdd))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- Keep the released changelog under version 0.2.0 (#39) ([d83e416](https://github.com/pederbe/esdieff/commit/d83e416c0d6f8e4d5cbde9c064ac9b7c8f684bf9))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- Finalize release notes and record publications on main (#40) ([09e18ec](https://github.com/pederbe/esdieff/commit/09e18ec71237ebcba4ba354e375976dc9b279caa))
  * Finalize release changelogs and record published notes on main
  
  * Keep source and published prose records separate
  
  * Filter paginated releases with supported GitHub CLI options
  
  * Report validated changelog checks on the checked commit
  
  * Separate semantic pattern assertions from the default deadline
  
  ---------
  
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- Update daily dependencies (#41) ([e86e7a4](https://github.com/pederbe/esdieff/commit/e86e7a4278e00bf362ff2ffb95cc1b462bbf0c0a))
- Explore shared definitions and inherited constraints (#51) ([0c5cd81](https://github.com/pederbe/esdieff/commit/0c5cd81ea31243a961507b43c2867b06c08af154))
  * feat: explore shared definition uses in model references
  
  Extend the existing Markdown reference with opt-in catalogue use navigation and previous-source links for local overrides.
  
  Refs #45
  
  * fix: exclude the selected alias from its own uses
  
  * fix: select authored refinements by their source coordinate
  
  * test: tolerate stdin closure after schema invocation rejection
  
  ---------
  
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- docs: connect shared-definition exploration with revision comparison (#52) ([5158833](https://github.com/pederbe/esdieff/commit/5158833c7d73d878be48a33318ebd44e1b72d1b8))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- docs: explain missing and ambiguous name reference resolution (#53) ([d7040cd](https://github.com/pederbe/esdieff/commit/d7040cda9bd08936e21597eaf582c9692b646260))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- docs: correct inspection views and domain terminology (#54) ([934fad8](https://github.com/pederbe/esdieff/commit/934fad8612b2cd1fe8d458cf343b5ec97621d54c))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- docs: audit terminology and current behavior across maintained documentation (#56) ([43a89cd](https://github.com/pederbe/esdieff/commit/43a89cd94876734c55bbe1ccb55c49951df4e940))
  Align guides, contracts, examples, site copy and API documentation with RFC 9880, the canonical glossary and current behavior.
  
  Closes #55.
- Explain generated corpus candidates and boundary evidence (#57) ([c49a14d](https://github.com/pederbe/esdieff/commit/c49a14d7564404559e3f787950419c853a59981b))
  Explain retained generation candidates with corpus evidence in the library and reports. Preserve whole-description admission and distinguish it from constraint coverage.
  
  Closes #46.
- Report successful Choice admission evidence for generated corpora (#59) ([d515c6c](https://github.com/pederbe/esdieff/commit/d515c6ccdbd95851a64ec910c81c0877423df9a2))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- Update Rust crate hotpath to v0.30.3 (#66) ([9dce6b6](https://github.com/pederbe/esdieff/commit/9dce6b6667b498f6f94b5d119c0af32a39b78e0b))
- docs: replace the door-lock simulator with sample checks (#78) ([c76cb62](https://github.com/pederbe/esdieff/commit/c76cb621c264d159eb7604b5614073ee666e1b25))
  Replace the simulated device, seeded commands, virtual clock and behavioral oracle with a short example that checks a supplied door-lock model and eight fixed values through the public SDF APIs. Retire demonstration-only glossary terms and clarify historical trace context.
  
  Run cargo run --example lock-demo without a seed argument. Editing, installation routes, package-manager support and application/library behavior are unchanged.
  
  Closes #77.
- Render documentation terminal examples with Rust (#71) (#79) ([277b35a](https://github.com/pederbe/esdieff/commit/277b35ac53bf697aba77cd6a22ac109a75fb135b))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- docs: complete changelog coverage since v0.2.0 (#83) ([706f20e](https://github.com/pederbe/esdieff/commit/706f20e8978a6ca59beb7a34c7da20ab9262f45d))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- Follow rolling stable Rust for development and CI (#84) ([10544ba](https://github.com/pederbe/esdieff/commit/10544ba29e0d46ef47794751c456fd4be0a19bc0))
- chore(deps): update dependency wrangler to v4.156.1 (#85) ([408472f](https://github.com/pederbe/esdieff/commit/408472f7bbf597716bff9b99d385026f5b459266))
- Fix Homebrew formula generation and version detection (#86) ([fc474a8](https://github.com/pederbe/esdieff/commit/fc474a8b0e9dce5b3da4dba68aa9d75bddd060fb))
  * Fix generated Homebrew formula syntax and version handling
  
  * Group Homebrew fix with existing Unreleased fixes
  
  * Keep a default archive for Homebrew cross-platform loading
  
  ---------
  
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- Check Homebrew formula style, audit and cross-platform loading (#87) ([67998d3](https://github.com/pederbe/esdieff/commit/67998d345f9bbc116b8cac5565684ec3d421305b))
  * Audit generated Homebrew formulas before installation checks
  
  * Trust the generated temporary tap before loading its formula
  
  ---------
  
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>
- Keep Homebrew updater formula checks in sync with the tap (#88) ([bfef4bc](https://github.com/pederbe/esdieff/commit/bfef4bcfaad471d6ea869f0bee51b54f2790b323))
  Co-authored-by: Peder Bergan <16326438+pederbe@users.noreply.github.com>

[Full comparison](https://github.com/pederbe/esdieff/compare/v0.2.0...v0.3.0)

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
