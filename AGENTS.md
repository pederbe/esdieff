Esdieff: an SDF toolkit for connected-device models (RFC 9880).

Use `Esdieff` for the product name and `esdieff` for the crate, command, repository, and folder names.

## Git workflow

Every change, including the maintainer's, enters `main` through a pull request
that passes the required hosted `CI` check. The maintainer uses a `contrib/`
branch in this repository. Other contributors use a branch in their fork.
Never force-push. Run independent Spec and Standards review for code changes
and resolve material findings before integration.

## Checks

Before committing: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

The maintained command is `bash scripts/check.sh`. It also checks release
tooling. Website changes additionally run `bash scripts/build-site.sh`.

When `cargo test` runs as root, the two permission tests in `directory_discovery_cli` fail by design because root bypasses file permissions. Every other test must still pass. Report the gap, and run those two tests on a non-root host before relying on that coverage.

## Agent skills

### Issue tracker

Issues, specs, and tickets live in this repository's GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the five default triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Use a single context: root `GLOSSARY.md` and `docs/adr/`. See `docs/agents/domain.md`.
