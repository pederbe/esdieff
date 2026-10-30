#![cfg(target_os = "linux")]

use std::{fs, path::Path, process::Command};

#[test]
fn website_selection_tracks_published_inputs_without_publishing_source_only_changes() {
    let workflow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/ci.yml"),
    )
    .unwrap();
    let selection = workflow
        .split_once("\n  checks:\n")
        .unwrap()
        .0
        .rsplit_once("        run: |\n")
        .unwrap()
        .1
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    // Supply changed paths independently of the working tree under test.
    let script = format!(
        "git() {{ if [ \"$1\" = cat-file ]; then return 0; fi; printf '%s\\0' \"$CHANGED_PATH\"; if [ -n \"${{SECOND_CHANGED_PATH:-}}\" ]; then printf '%s\\0' \"$SECOND_CHANGED_PATH\"; fi; }}\n{selection}"
    );
    let temporary = std::env::temp_dir().join(format!(
        "esdieff-ci-selection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&temporary).unwrap();
    let output_path = temporary.join("outputs");
    let selected = |path: &str, second: &str| {
        fs::write(&output_path, "").unwrap();
        let output = Command::new("bash")
            .args(["-c", &script])
            .env("EVENT", "push")
            .env("BASE", "checked-base")
            .env("CHANGED_PATH", path)
            .env("SECOND_CHANGED_PATH", second)
            .env("RUNNER_TEMP", &temporary)
            .env("GITHUB_OUTPUT", &output_path)
            .output()
            .unwrap();
        assert!(output.status.success(), "path selection failed for {path}");
        fs::read_to_string(&output_path).unwrap()
    };
    for (path, rust, docs, styles, website) in [
        ("site/index.html", false, true, true, true),
        ("site/og.png", false, true, true, true),
        ("tools/site/package-lock.json", false, true, true, true),
        ("docs/guides/testing-values.md", false, true, false, true),
        ("book.toml", false, true, false, true),
        ("CONTEXT.md", false, true, false, true),
        ("GLOSSARY.md", false, true, false, true),
        ("wrangler.jsonc", false, true, false, true),
        ("scripts/build-site.sh", false, true, false, true),
        (".github/workflows/website.yml", false, true, false, true),
        ("README.md", false, true, false, false),
        ("src/lib.rs", true, false, false, false),
        (".github/FUNDING.yml", false, false, false, false),
    ] {
        assert_eq!(
            selected(path, ""),
            format!(
                "rust={rust}\ndocs={docs}\nstyles={styles}\nwebsite={website}\nrelease_changelog=false\n"
            ),
            "incorrect check and deployment selection for {path}"
        );
    }
    for path in [
        "tools/release-changelog/src/main.rs",
        "tools/release-changelog/Cargo.lock",
        "scripts/check-release-changelog.sh",
        "scripts/record-release-changelog.sh",
        "scripts/configure-release-changelog.sh",
        "scripts/prepare-release.sh",
        ".github/workflows/release-changelog.yml",
        ".github/workflows/release-changelog-check.yml",
    ] {
        assert_eq!(
            selected(path, ""),
            "rust=false\ndocs=false\nstyles=false\nwebsite=false\nrelease_changelog=true\n",
            "standalone helper change must not select application checks: {path}"
        );
    }
    assert_eq!(
        selected("tools/release-changelog/src/main.rs", "src/lib.rs"),
        "rust=true\ndocs=false\nstyles=false\nwebsite=false\nrelease_changelog=true\n",
        "helper selection must preserve application checks for a mixed change"
    );
    fs::remove_dir_all(temporary).unwrap();
}

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
            .env("RELEASE_CHANGELOG_SELECTED", "false")
            .env(
                "RELEASE_CHANGELOG_RESULT",
                if rust_selected == "true" {
                    "success"
                } else {
                    "skipped"
                },
            )
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            expected,
            "incorrect aggregate result for checks={checks}, docs={docs}, artifacts={artifacts}"
        );
    }
    for (rust_selected, helper_selected, helper_result, expected) in [
        ("false", "true", "success", true),
        ("false", "true", "skipped", false),
        ("false", "true", "failure", false),
        ("false", "true", "cancelled", false),
        ("false", "false", "skipped", true),
        ("false", "false", "success", false),
        ("true", "false", "skipped", false),
    ] {
        let selected_result = if rust_selected == "true" {
            "success"
        } else {
            "skipped"
        };
        let output = Command::new("bash")
            .args(["-c", &gate])
            .env("CHANGES_RESULT", "success")
            .env("CHECKS_RESULT", selected_result)
            .env("DOCS_RESULT", "skipped")
            .env("ARTIFACTS_RESULT", selected_result)
            .env("RUST_SELECTED", rust_selected)
            .env("DOCS_SELECTED", "false")
            .env("RELEASE_CHANGELOG_SELECTED", helper_selected)
            .env("RELEASE_CHANGELOG_RESULT", helper_result)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            expected,
            "incorrect aggregate result for rust={rust_selected}, helper={helper_selected}/{helper_result}"
        );
    }
}
