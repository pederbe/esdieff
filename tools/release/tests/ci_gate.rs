#![cfg(target_os = "linux")]

use std::{fs, path::Path, process::Command};

#[test]
fn aggregate_requires_selected_checks_and_accepts_only_intentional_skips() {
    let workflow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/ci.yml"),
    )
    .unwrap();
    let gate = workflow
        .rsplit_once("        run: |\n")
        .unwrap()
        .1
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    for (changes, checks, docs, artifacts, rust_selected, docs_selected, expected) in [
        (
            "success", "success", "success", "success", "true", "true", true,
        ),
        (
            "success", "skipped", "success", "skipped", "false", "true", true,
        ),
        (
            "success", "skipped", "skipped", "skipped", "false", "false", true,
        ),
        (
            "failure", "skipped", "skipped", "skipped", "false", "false", false,
        ),
        (
            "cancelled",
            "skipped",
            "skipped",
            "skipped",
            "false",
            "false",
            false,
        ),
        (
            "success", "failure", "success", "success", "true", "true", false,
        ),
        (
            "success",
            "cancelled",
            "success",
            "success",
            "true",
            "true",
            false,
        ),
        (
            "success", "success", "success", "skipped", "true", "true", false,
        ),
        (
            "success", "success", "skipped", "success", "true", "true", false,
        ),
        (
            "success", "skipped", "failure", "skipped", "false", "true", false,
        ),
    ] {
        let output = Command::new("bash")
            .args(["-c", &gate])
            .env("CHANGES_RESULT", changes)
            .env("CHECKS_RESULT", checks)
            .env("DOCS_RESULT", docs)
            .env("ARTIFACTS_RESULT", artifacts)
            .env("RUST_SELECTED", rust_selected)
            .env("DOCS_SELECTED", docs_selected)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            expected,
            "incorrect aggregate result for checks={checks}, docs={docs}, artifacts={artifacts}"
        );
    }
}
