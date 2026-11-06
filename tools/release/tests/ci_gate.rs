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
    let selected = |event: &str, base: &str, path: &str, second: &str| {
        fs::write(&output_path, "").unwrap();
        let output = Command::new("bash")
            .args(["-c", &script])
            .env("EVENT", event)
            .env("BASE", base)
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
            selected("pull_request", "checked-base", path, ""),
            format!(
                "rust={rust}\nrelease_tools=false\nartifacts=false\ndocs={docs}\nstyles={styles}\nwebsite={website}\n"
            ),
            "incorrect check and deployment selection for {path}"
        );
        assert_eq!(
            selected("push", "checked-base", path, ""),
            format!(
                "rust=false\nrelease_tools=false\nartifacts=false\ndocs=false\nstyles=false\nwebsite={website}\n"
            ),
            "post-merge selection must only retain website delivery for {path}"
        );
    }
    for base in ["checked-base", ""] {
        assert_eq!(
            selected("workflow_dispatch", base, "src/lib.rs", ""),
            "rust=true\nrelease_tools=true\nartifacts=true\ndocs=true\nstyles=true\nwebsite=true\n",
            "manual CI and the manually dispatched release caller must retain full checks"
        );
    }
    for (path, artifacts) in [
        ("tools/release/tests/ci_gate.rs", false),
        ("tools/release/tests/fixtures/gh.rs", false),
        ("tools/release/src/distribution_checks.rs", false),
        ("tools/release/src/metadata.rs", false),
        ("tools/release/src/distribution.rs", true),
        ("tools/release/src/smoke.rs", true),
        ("tools/release/src/licenses.rs", true),
        ("tools/release/Cargo.toml", true),
        ("tools/release/Cargo.lock", true),
        (".github/workflows/artifacts.yml", true),
        (".github/workflows/release.yml", true),
        (".github/workflows/release-plz.yml", true),
        ("release-plz.toml", true),
        (".github/workflows/distribution.yml", true),
        (".github/workflows/package-repository.yml", true),
    ] {
        assert_eq!(
            selected("pull_request", "checked-base", path, ""),
            format!(
                "rust=false\nrelease_tools=true\nartifacts={artifacts}\ndocs=false\nstyles=false\nwebsite=false\n"
            ),
            "incorrect release-helper coverage for {path}"
        );
        assert_eq!(
            selected("push", "checked-base", path, ""),
            "rust=false\nrelease_tools=false\nartifacts=false\ndocs=false\nstyles=false\nwebsite=false\n",
            "post-merge selection must not repeat release-helper checks: {path}"
        );
    }
    for path in [
        "src/lib.rs",
        "examples/thermostat-check/rich.sdf.json",
        "tests/fixtures/model.json",
        "scripts/check.sh",
        ".github/workflows/ci.yml",
        "unknown-input",
    ] {
        for helper in ["", "tools/release/tests/ci_gate.rs"] {
            assert_eq!(
                selected("pull_request", "checked-base", path, helper),
                format!(
                    "rust=true\nrelease_tools={}\nartifacts=false\ndocs=false\nstyles=false\nwebsite=false\n",
                    !helper.is_empty()
                ),
                "application, shared or unknown inputs must retain full checks: {path}"
            );
        }
    }
    assert_eq!(
        selected("workflow_call", "", "tools/release/tests/ci_gate.rs", ""),
        "rust=true\nrelease_tools=true\nartifacts=true\ndocs=true\nstyles=true\nwebsite=true\n",
        "release validation without a comparison base must retain full checks"
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
            .env("RELEASE_TOOLS_SELECTED", "false")
            .env("ARTIFACTS_SELECTED", "false")
            .env("DOCS_SELECTED", docs_selected)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            expected,
            "incorrect aggregate result for checks={checks}, docs={docs}, artifacts={artifacts}"
        );
    }
    for (artifacts_selected, artifacts_result) in [("false", "skipped"), ("true", "success")] {
        for checks_result in ["success", "failure", "cancelled", "skipped"] {
            for actual_artifacts in ["success", "skipped", "failure", "cancelled"] {
                let output = Command::new("bash")
                    .args(["-c", &gate])
                    .env("CHANGES_RESULT", "success")
                    .env("CHECKS_RESULT", checks_result)
                    .env("DOCS_RESULT", "skipped")
                    .env("ARTIFACTS_RESULT", actual_artifacts)
                    .env("RUST_SELECTED", "false")
                    .env("RELEASE_TOOLS_SELECTED", "true")
                    .env("ARTIFACTS_SELECTED", artifacts_selected)
                    .env("DOCS_SELECTED", "false")
                    .output()
                    .unwrap();
                assert_eq!(
                    output.status.success(),
                    checks_result == "success" && actual_artifacts == artifacts_result,
                    "incorrect helper gate for checks={checks_result}, artifacts={artifacts_selected}/{actual_artifacts}"
                );
            }
        }
    }
}

#[test]
fn maintained_check_command_limits_only_the_requested_scope() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (argument, count) in [("", 9), ("--all", 9), ("--release-tools-only", 3)] {
        let output = Command::new("bash")
            .args([
                "-c",
                "cargo() { printf '%s\\n' \"$*\"; }; export -f cargo; bash scripts/check.sh $1",
                "checks",
                argument,
            ])
            .current_dir(&root)
            .env("GITHUB_ACTIONS", "true")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let commands = String::from_utf8(output.stdout).unwrap();
        assert_eq!(commands.lines().count(), count);
        if argument == "--release-tools-only" {
            assert!(
                commands
                    .lines()
                    .all(|line| line.contains("--manifest-path tools/release/Cargo.toml"))
            );
        }
    }
    let output = Command::new("bash")
        .args(["scripts/check.sh", "--unknown"])
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
