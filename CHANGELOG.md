# Changelog

## [Unreleased]

- Generate deterministic test values for selected Context metadata and replay
  Context vectors against the same or a changed model.

- Author Things from Objects containing Context metadata, refine their Context
  descriptions and edit supported qualities on authored Context definitions.

- Check recorded traces about 6 times faster and generate test values
  about twice as fast when values must match a `pattern`. Rarely, a value
  with no pattern polls to spare is now undetermined in a trace or generation
  request where it would settle on its own.

## [0.1.0] - 2026-10-27

The first public release of Esdieff provides a Rust library and command-line
tool for working with connected-device models in Semantic Definition Format.

- Inspect and check supplied models, shared definitions and references.
- Compare model revisions and review changes that affect applications.
- Generate test values and check samples and recorded values against models.
- Author and edit composed models while retaining source information.
- Save human, JSON, SARIF and Markdown reports for supported operations.
- Acquire an explicitly selected GitHub document with a pinned revision and digest.
