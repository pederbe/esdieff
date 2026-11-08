# Contributing

Bug reports, examples and improvements are welcome. Open an issue to discuss a
substantial change before implementing it.

Use the Rust toolchain pinned in `rust-toolchain.toml` and run the maintained checks:

```sh
bash scripts/check.sh
```

Website changes also need `bash scripts/build-site.sh`. See
[website build instructions](tools/site/README.md) for its prerequisites.

Every change, including the maintainer's, enters `main` through a pull request
that passes the required hosted `CI` check. The maintainer uses a `contrib/`
branch in this repository. Other contributors use a branch in their fork.
Explain the problem, the resulting behavior and the relevant validation.

CI validates pull requests before merging. After merging, it only builds and
deploys the website when relevant files change. Manual CI and release validation
continue to run the full checks.

Release-helper-only changes reuse the platform jobs with
`bash scripts/check.sh --release-tools-only`. Changes to the release helper's
tests and release-notes code skip application builds. Packaging, installation
and artifact workflow changes retain release installation checks. Application,
shared-fixture and mixed changes retain full coverage, as do changes to CI
selection or the check script.

## Changelog and releases

The [changelog](CHANGELOG.md) is generated from the titles of merged pull
requests. Each pull request is squashed into one commit on `main`, and
[release-plz](https://release-plz.dev) groups those commit titles by their
conventional prefix:

- `feat:` for a new capability, listed under Added
- `fix:` for a bug fix, listed under Fixed
- `perf:` for a measured performance improvement, listed under Performance
- `docs:` for documentation that users should notice, listed under Documentation
- `fix(deps):` for a dependency update that ships to users, listed under
  Dependencies
- `ci:`, `build:`, `chore:`, `test:`, `style:` or `refactor:` for internal work,
  which stays out of the changelog

Titles without a prefix are listed under Other changes. Write the rest of the
title for users, describing the new behavior rather than the implementation.
Each entry links to its pull request, so explain compatibility consequences,
report format versions and any action users need to take in the description.
For a breaking change, add `!` after the prefix, such as `feat!:`, and add a
`BREAKING CHANGE:` footer with the required action to the squashed commit
message. GitHub uses the commit title instead of the pull request title when a
pull request has a single commit, so give both the same title.

Only changes to files in the published crate count towards a release. Workflow,
packaging and release-helper changes are not released on their own and stay out
of the changelog.

Every Monday, the Release-plz workflow opens a release pull request when `main`
has changes for the changelog. It raises the version and adds a dated section to
the changelog. Before 1.0, a breaking change raises the minor version and any
other change raises the patch version. An incompatible library API change found
by `cargo-semver-checks` also raises the minor version. The release pull request
merges automatically once CI passes. When another change merges first, the
workflow replaces it with an up-to-date release pull request. Merging the
release pull request tags the version and starts the Release workflow, which
builds and publishes the release with that changelog section as its notes.

To edit a release pull request, disable its auto-merge first, then merge it
before anything else merges, because any merge to `main` replaces it with a new
one that merges automatically. To skip a week, close the release pull request.
To release a new minor version for an additive milestone, raise the version in
`Cargo.toml` and `Cargo.lock` in an ordinary pull request. The next release pull
request keeps that version. Maintainers can open a release pull request at
another time by running the Release-plz workflow manually.

If the Release workflow fails after tagging for a reason outside the tagged
sources, such as a flaky job, a secret or a setting, fix the cause and run it
again for that tag with publishing enabled. Release-plz compares `main` with the
version published on crates.io, so later releases wait until this one is
published.

After publication, the Distribution updates workflow validates and installs the
published WinGet manifest, submits it with WinGetCreate, and completes the PR
description from its check evidence. Microsoft controls catalog admission. Retry
that workflow on `main` with the same release tag and submission enabled to finish
an interrupted submission or description update. Manual runs default to preparation
without public changes. Existing matching PRs are reused, and manual description
edits are preserved. Submission requires `WINGET_ENABLED=true` and `WINGET_TOKEN`
in the `distribution` environment. The CLA checkbox reflects Microsoft's check
and never signs an agreement.

The workflow needs a `RELEASE_PLZ_TOKEN` repository secret holding a
fine-grained personal access token for this repository, with read and write
access to Contents and Pull requests. GitHub does not run CI for pull requests
opened with the built-in workflow token, so the release pull request could not
pass its required check without it.

Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion in this work, as defined in the Apache-2.0 license, shall
be dual licensed under MIT or Apache-2.0, at the recipient's option, without
any additional terms or conditions.
