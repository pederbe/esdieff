# Release tooling

The release helper builds and checks distribution metadata, exercises installed
executables and extracts release notes. The workflows use those checks before
publishing a crate, release or package update.

For local helper changes, run from the repository root:

```sh
bash scripts/check.sh --release-tools-only
```

The [contribution guide](../../CONTRIBUTING.md#build-and-check) lists prerequisites.
The [CI workflow](../../.github/workflows/ci.yml) selects checks, and the
[artifact workflow](../../.github/workflows/artifacts.yml) owns platform builds,
archive installation and package checks.

## Release preparation

[Release-plz](../../.github/workflows/release-plz.yml) prepares a release pull
request, refreshes it when another change merges and enables automatic merging
after CI passes. Its workflow owns the schedule and manual controls.
[release-plz.toml](../../release-plz.toml) defines version selection and changelog
groups. Before 1.0, features and breaking changes raise the minor version, as do
incompatible library API changes found by `cargo-semver-checks`. Other changes
raise the patch version.

Version selection uses changes to the published crate. The
[changelog postprocessor](changelog.sh) includes entries from the whole repository
in each prepared release, so relevant installation and packaging changes also
reach the notes. Internal changes stay out. A week without changes for the
changelog produces no release.

Edits made directly to the release pull request are replaced when the next merge
to `main` regenerates it and enables auto-merge again. Put lasting corrections
in the source or release configuration.

Release automation uses a repository credential that allows its pull requests to
trigger CI. Pull requests opened with the built-in workflow token do not trigger
those checks. The workflow declares the credential it uses.

## Release integrity and recovery

Merging the release pull request tags the version and dispatches the
[Release workflow](../../.github/workflows/release.yml). That workflow checks the
tag against the package version, validates the source and retains the candidate
archives, native packages, crate, notes, checksums and source identity.
The release notes come from the tag's dated changelog section.

Publication requires the selected source revision to be the requested tag. The
workflow checks retained checksums and compares existing uploaded assets before
publishing. It also checks the crate checksum if that version is already on
crates.io. Existing published assets are not overwritten.

If publication fails for a reason outside the tagged sources, such as a job
failure, credential or repository setting, fix the cause and rerun the Release
workflow for that same tag with publishing enabled. Use the workflow's declared
inputs. Preparation without publishing remains available for checking a
candidate. Preserve released tags and artifacts. A fix to tagged source code
needs a new version.

Release-plz compares `main` with the version published on crates.io, so later
releases wait until a tagged release is published.

## Distribution checks

After publication, the Release workflow dispatches package, distribution and
website delivery. Each workflow declares its current inputs and publication
controls.

- The [package repository workflow](../../.github/workflows/package-repository.yml)
  verifies released package checksums, prepares signed repository metadata and
  checks installation, upgrade and removal before advancing the live indexes.
- The release helper prepares the Homebrew formula from the checked release
  artifacts. The [tap updater](../../packaging/homebrew/update-esdieff.yml) consumes
  that released formula.
- The [distribution workflow](../../.github/workflows/distribution.yml) validates
  and installs the published WinGet manifest before submitting it with Komac.
  Microsoft controls catalog admission.
- The [website guide](../site/README.md#deployment) explains how a deployment's
  source revision relates to the latest published release.

WinGet preparation makes no public changes and does not use the fork-sync
credential. Submission uses separate credentials for submission and fork
synchronization, with the submission credential restricted to public repositories.
Synchronization does not force or overwrite a divergent fork.

To finish an interrupted WinGet submission or description update, retry the
distribution workflow on `main` for the same release tag with submission enabled.
It reuses matching PRs and preserves manual description edits. The PR description
comes from check evidence. Its CLA checkbox reflects Microsoft's check and never
signs an agreement.
