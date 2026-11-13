# Contributing

Bug reports, examples and improvements are welcome. Open an issue to discuss a
substantial change before implementing it. Report suspected vulnerabilities
through the [security policy](SECURITY.md).

## Build and check

Use the Rust toolchain selected by [rust-toolchain.toml](rust-toolchain.toml).
On Linux, install the `git-cliff` version selected by the
[release workflow](.github/workflows/release-plz.yml) first. The release-note
tests use local Git fixtures without contacting GitHub. Run the maintained checks:

```sh
bash scripts/check.sh
```

For release-helper-only changes, you can use
`bash scripts/check.sh --release-tools-only`. Website changes also need
`bash scripts/build-site.sh`. The [website guide](tools/site/README.md) lists its
prerequisites and authoring tools. CI selects the checks relevant to each PR.

[ARCHITECTURE.md](ARCHITECTURE.md) maps the code. Use the terms in
[GLOSSARY.md](GLOSSARY.md) and read the [design decisions](docs/adr/README.md)
relevant to your change. The [profiling guide](docs/profiling.md) describes the
existing workloads and what their measurements mean.

## Public issues and pull requests

Create a branch in your fork and open a pull request. Every change enters `main`
through a pull request that passes the required hosted `CI` check.

Issue descriptions use **Problem**, **Desired outcome** and **Evidence**.
Describe the task and its impact, the result that would resolve it, and available
reproduction details or examples. Incomplete observations are welcome.
Pull requests use **Problem**, **Solution** and **Validation**. Explain the
reason for the change, its behavior, compatibility effects and meaningful checks.
Put migration or upgrade instructions in Solution. Keep small changes brief and
links and examples within these sections.

Keep public discussion focused on software behavior, technical rationale and
evidence. Remove private data from reports, attribute checks accurately, and
preserve other people's observations and wording when updating a report.

## Changelog and releases

The [changelog](CHANGELOG.md) is generated from merged pull request titles. Squash
merges keep the PR title and description. Use a conventional title that describes
the resulting behavior for users:

- `feat:` for a new capability
- `fix:` for a bug fix, including `fix(security):` and `fix(deps):` when applicable
- `perf:` for a measured performance improvement
- `docs:` for documentation users should notice
- `feat(deprecate):` for deprecation and `feat(remove)!:` for removal
- `ci:`, `build:`, `chore:`, `test:`, `style:` or `refactor:` for internal work,
  which stays out of the changelog

Titles without a prefix appear under Other changes. For a breaking change, add
`!` after the prefix, such as `feat!:`. Explain compatibility consequences, report
format versions and any migration or upgrade action in Solution. Changelog entries
link to their PRs. No changelog or breaking-change footer is required.
Keep `## [Unreleased]` empty.

The [release tooling guide](tools/release/README.md) explains release integrity,
recovery and distribution. Its linked workflows and configuration define the
current release schedule and settings.

## License

Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion in this work, as defined in the Apache-2.0 license, shall
be dual licensed under MIT or Apache-2.0, at the recipient's option, without
any additional terms or conditions.
