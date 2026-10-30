# Contributing

Bug reports, examples and improvements are welcome. Open an issue to discuss a
substantial change before implementing it.

Use the stable Rust toolchain and run the maintained checks:

```sh
bash scripts/check.sh
```

Website changes also need `bash scripts/build-site.sh`. See
[website build instructions](tools/site/README.md) for its prerequisites.

Every change, including the maintainer's, enters `main` through a pull request
that passes the required hosted `CI` check. The maintainer uses a `contrib/`
branch in this repository. Other contributors use a branch in their fork.
Explain the problem, the resulting behavior and the relevant validation.

Include a concise entry under `Unreleased` in [CHANGELOG.md](CHANGELOG.md) in the
same pull request as a user-visible change. Cover new capabilities, fixes,
installation changes and changes to command-line behavior, report versions or
file formats. Explain compatibility consequences and any action users need to
take. Keep measured performance claims tied to their workloads. For changes
that need no entry, explain why in the pull request's Changelog section.

Before preparing a release, compare the Unreleased entries with all merged
changes since the previous published release. Check feature coverage, fixes,
support boundaries and upgrade instructions. The generated commit history
supplements this user-facing summary and does not replace it.

For an incompatible command-line, report or file-format change, include a
`BREAKING CHANGE:` explanation in the final squash commit. Automatic releases
use this marker to select a major increment, or a minor increment before 1.0.
Use the manual minor increment for an additive feature milestone when needed.

Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion in this work, as defined in the Apache-2.0 license, shall
be dual licensed under MIT or Apache-2.0, at the recipient's option, without
any additional terms or conditions.
