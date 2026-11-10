# Architecture

Esdieff is one Rust crate that builds a library and the `esdieff` command. The
command is a thin layer over the library, so both run the same model
operations. [How Esdieff works](https://esdieff.com/docs/guides/how-esdieff-works)
describes those operations for users. Their code is laid out as follows.

## Data flow

1. **Inputs.** `src/main.rs` parses the command line by hand. The `*_cli.rs`
   files, `src/check_plan.rs` and `src/directory_discovery.rs` turn arguments,
   plans and directories into explicit library requests.
2. **Catalogue.** `src/source.rs` acquires the supplied documents into a
   `Catalogue`. `src/json.rs` decodes them within fixed limits and keeps exact
   number tokens and source locations.
3. **Source structure.** `src/structure.rs` and `src/role.rs` establish what
   each document location is and record structural findings.
4. **Effective view.** `src/effective.rs` composes `sdfRef` references among
   the supplied documents and keeps the origin of each quality.
5. **Operations.** Each operation reads the catalogue and its effective views:
   model assessment (`Catalogue::check` in `src/report.rs`, with assessment
   profiles in `src/assessment.rs` and the modules for units, content formats,
   patterns and contradiction advice), sample checking
   (`src/sample.rs`), generation (`src/generation.rs`), traces (`src/trace.rs`),
   revision comparison (`src/comparison.rs`), authoring and editing
   (`src/author.rs`, `src/edit.rs`), model references (`src/reference.rs`) and
   JSON Schema export (`src/schema.rs`).
6. **Reports.** `src/report.rs` holds the shared report. `src/human.rs`,
   `src/sarif.rs` and the JSON renderers present it, and the report's
   disposition becomes the command's exit status.

Work is bounded throughout. `src/limits.rs` holds the fixed ceilings and
`src/allowance.rs` charges each operation's work against its allowance.

## Around the crate

- `tests/` holds the integration tests and fixtures, and `examples/` the
  example models that the documentation and the device catalogue tests use.
- `docs/` is the user documentation, published as a book by
  `scripts/build-site.sh` together with the landing page in `site/`.
- `docs/adr/` holds the decision records, and `GLOSSARY.md` the domain
  language.
- `tools/release` is the release helper and `tools/terminal-html` converts
  captured terminal output for the documentation.
