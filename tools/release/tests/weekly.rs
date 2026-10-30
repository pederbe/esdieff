use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = loop {
            let root = std::env::temp_dir().join(format!(
                "esdieff-weekly-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => break root,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create fixture: {error}"),
            }
        };
        let fixture = Self {
            output: root.join("plan"),
            root: root.join("repo"),
        };
        fs::create_dir(&fixture.root).unwrap();
        fixture.git(&["init", "-b", "main"]);
        fixture.git(&["config", "user.name", "Release check"]);
        fixture.git(&["config", "user.email", "release-check@example.com"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        fixture.git(&["config", "core.autocrlf", "false"]);
        fixture.write(
            "Cargo.toml",
            "[package]\nname = \"esdieff\"\nversion = \"0.1.0\"\n",
        );
        fixture.write("Cargo.lock", "version = 4\n\n[[package]]\nname = \"dependency\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"esdieff\"\nversion = \"0.1.0\"\n");
        fixture.write(
            "CHANGELOG.md",
            "# Changelog\n\n## [Unreleased]\n\n## [0.1.0] - 2026-10-27\n\nInitial release.\n",
        );
        fixture.commit("Initial release", "");
        fixture.git(&["tag", "v0.1.0"]);
        fixture
    }

    fn write(&self, file: &str, content: &str) {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(&self.root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn commit(&self, subject: &str, body: &str) {
        self.git(&["add", "."]);
        self.git(&["commit", "-m", subject, "-m", body]);
    }

    fn prepare(&self, previous: &str, mode: &str, minimum: Option<&str>) -> Output {
        let mut command = self.command();
        command
            .args(["prepare-weekly", previous, "2026-11-02", mode])
            .arg(&self.output);
        if let Some(minimum) = minimum {
            command.arg(minimum);
        }
        command.output().unwrap()
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff-release"));
        command.current_dir(&self.root);
        command
    }

    fn plan(&self) -> String {
        fs::read_to_string(&self.output).unwrap()
    }
    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.root.join(path)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.root.parent().unwrap());
    }
}

fn passed(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unchanged_and_documentation_only_weeks_do_not_modify_metadata() {
    let fixture = Fixture::new();
    let before = fixture.read("Cargo.toml");
    passed(fixture.prepare("v0.1.0", "auto", None));
    assert!(fixture.plan().starts_with("changed=false\nsource="));
    for path in [
        "README.md",
        "docs/guide.md",
        "site/app.js",
        "tools/site/package-lock.json",
        "scripts/build-site.sh",
        "wrangler.jsonc",
    ] {
        fixture.write(path, "Documentation delivery\n");
    }
    fixture.commit("Improve installation guide", "");
    passed(fixture.prepare("v0.1.0", "auto", None));
    assert!(fixture.plan().starts_with("changed=false\n"));
    assert_eq!(fixture.read("Cargo.toml"), before);
    assert!(fixture.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn dated_changelogs_prepare_against_legacy_and_dated_release_tags() {
    for legacy in [true, false] {
        let fixture = Fixture::new();
        let (previous, next) = if legacy {
            ("v0.1.0", "0.1.1")
        } else {
            fixture.write(
                "CHANGELOG.md",
                "# Changelog\n\n## [0.1.1] - 2026-10-30\n\nFirst fix.\n\n## [0.1.0] - 2026-10-27\n\nInitial release.\n",
            );
            fixture.commit("Release 0.1.1", "");
            fixture.git(&["tag", "v0.1.1"]);
            ("v0.1.1", "0.1.2")
        };
        let dated = fixture
            .read("CHANGELOG.md")
            .replace("## [Unreleased]\n\n", "");
        fixture.write("CHANGELOG.md", &dated);
        fixture.write("src/main.rs", "fn main() {}\n");
        fixture.commit("fix: Handle the next change", "");
        passed(fixture.prepare(previous, "auto", None));
        let changelog = fixture.read("CHANGELOG.md");
        assert!(changelog.starts_with(&format!("# Changelog\n\n## [{next}] - 2026-11-02\n")));
        assert!(!changelog.contains("## [Unreleased]"));
        assert_eq!(changelog.matches("Initial release.").count(), 1);
        if !legacy {
            assert_eq!(changelog.matches("First fix.").count(), 1);
        }
    }
}

#[test]
fn malformed_changelogs_do_not_modify_release_metadata() {
    for changelog in [
        "# Changelog\n",
        "# Changelog\n\n## [Unreleased]\n\n- A pending fix.\n",
        "# Changelog\n\n## [Soon] - 2026-10-30\n\nA fix.\n",
        "# Changelog\n\n## [0.1.0] - not-a-date\n\nA fix.\n",
        "# Changelog\n\n## [0.1.0] - 2026-10-27\n\nInitial.\n\n## [Unreleased]\n",
    ] {
        let fixture = Fixture::new();
        fixture.write("CHANGELOG.md", changelog);
        fixture.write("src/main.rs", "fn main() {}\n");
        fixture.commit("fix: Handle the next change", "");
        let manifest = fixture.read("Cargo.toml");
        let lock = fixture.read("Cargo.lock");
        assert!(!fixture.prepare("v0.1.0", "auto", None).status.success());
        assert_eq!(fixture.read("Cargo.toml"), manifest);
        assert_eq!(fixture.read("Cargo.lock"), lock);
        assert_eq!(fixture.read("CHANGELOG.md"), changelog);
        assert!(!fixture.output.exists());
    }
}

#[test]
fn every_commit_enters_notes_and_root_versions_stay_consistent() {
    let fixture = Fixture::new();
    fixture.write("README.md", "New guide\n");
    fixture.commit("Explain supplied models", "Include the supplier workflow.");
    fixture.write("src/main.rs", "fn main() {}\n");
    fixture.commit(
        "Correct report output",
        "Preserve the explanation after the summary.",
    );
    fixture.write("packaging/package.conf", "package\n");
    fixture.commit("fix: Add packaging metadata", "");
    fixture.write("Cargo.toml", "[package]\nname = \"esdieff\"\nversion = \"0.1.0\"\n\n[package.metadata.release]\nversion = \"ignored\"\n");
    fixture.commit("Record release configuration", "");
    let source = fixture.git(&["rev-parse", "HEAD"]);
    passed(fixture.prepare("v0.1.0", "auto", None));
    assert_eq!(
        fixture.plan(),
        format!("changed=true\ntag=v0.1.1\nsource={}\n", source.trim())
    );
    assert!(fixture.read("Cargo.toml").contains("version = \"0.1.1\""));
    assert!(fixture.read("Cargo.toml").contains("version = \"ignored\""));
    let lock = fixture.read("Cargo.lock");
    assert!(lock.contains("name = \"esdieff\"\nversion = \"0.1.1\""));
    assert!(lock.contains("name = \"dependency\"\nversion = \"0.1.0\""));
    let changelog = fixture.read("CHANGELOG.md");
    assert!(changelog.starts_with("# Changelog\n\n## [0.1.1] - 2026-11-02\n"));
    assert!(!changelog.contains("## [Unreleased]"));
    for text in [
        "Explain supplied models",
        "supplier workflow",
        "Correct report output",
        "Preserve the explanation",
        "fix: Add packaging metadata",
        "Record release configuration",
        "## [0.1.1] - 2026-11-02",
        "## [0.1.0]",
    ] {
        assert!(changelog.contains(text), "missing {text}");
    }
    assert!(changelog.contains("/commit/"));
    let notes = fixture.output.with_extension("md");
    passed(
        fixture
            .command()
            .args(["release-notes", "v0.1.1"])
            .arg(&notes)
            .output()
            .unwrap(),
    );
    let notes = fs::read_to_string(notes).unwrap();
    assert!(notes.contains("Correct report output"));
    assert!(!notes.contains("Initial release."));
}

#[test]
fn breaking_markers_and_manual_increments_choose_versions() {
    for (subject, body, mode, expected) in [
        ("fix!: Change report schema", "", "auto", "v0.2.0"),
        (
            "Change report schema",
            "BREAKING CHANGE: Remove an output field.",
            "auto",
            "v0.2.0",
        ),
        (
            "Change report schema",
            "BREAKING-CHANGE: Remove an output field.",
            "auto",
            "v0.2.0",
        ),
        ("feat: Add a report", "", "auto", "v0.1.1"),
        ("Add a report", "", "minor", "v0.2.0"),
        ("Add a report", "", "major", "v1.0.0"),
    ] {
        let fixture = Fixture::new();
        fixture.write("src/main.rs", "fn main() {}\n");
        fixture.commit(subject, body);
        passed(fixture.prepare("v0.1.0", mode, None));
        assert!(fixture.plan().contains(&format!("tag={expected}\n")));
        if !body.is_empty() {
            assert!(fixture.read("CHANGELOG.md").contains(body));
        }
    }
}

#[test]
fn metadata_tags_do_not_loop_and_later_releases_keep_history_once() {
    let fixture = Fixture::new();
    fixture.write("src/main.rs", "fn main() {}\n");
    fixture.write("CHANGELOG.md", "# Changelog\n\n## [Unreleased]\n\n### Fixed\n\n- Preserve the exact numeric spelling.\n  Includes numbers in nested values.\n\n## [0.1.0] - 2026-10-27\n\nInitial release.\n");
    fixture.commit(
        "fix!: Change report schema",
        "BREAKING CHANGE: Report fields changed.",
    );
    let source = fixture.git(&["rev-parse", "HEAD"]);
    passed(fixture.prepare("v0.1.0", "auto", None));
    fixture.commit("Prepare automated release v0.2.0", "");
    fixture.git(&["tag", "v0.2.0"]);
    passed(fixture.prepare("v0.2.0", "auto", None));
    assert!(fixture.plan().starts_with("changed=false\n"));
    fixture.git(&["checkout", "--detach", source.trim()]);
    passed(fixture.prepare("v0.2.0", "auto", None));
    assert!(fixture.plan().starts_with("changed=false\n"));
    fixture.write("src/main.rs", "fn main() { println!(\"changed\"); }\n");
    fixture.commit("Handle a later change", "");
    passed(fixture.prepare("v0.2.0", "auto", None));
    let changelog = fixture.read("CHANGELOG.md");
    assert!(changelog.starts_with("# Changelog\n\n## [0.2.1] - 2026-11-02\n"));
    assert!(!changelog.contains("## [Unreleased]"));
    for text in [
        "## [0.2.1]",
        "## [0.2.0]",
        "## [0.1.0]",
        "Handle a later change",
    ] {
        assert!(changelog.contains(text));
    }
    assert_eq!(changelog.matches("fix!: Change report schema").count(), 1);
    assert_eq!(
        changelog
            .matches("Preserve the exact numeric spelling.")
            .count(),
        1
    );
    fixture.git(&["restore", "Cargo.toml", "Cargo.lock", "CHANGELOG.md"]);
    passed(fixture.prepare("v0.2.0", "auto", Some("v0.2.1")));
    assert!(fixture.plan().contains("tag=v0.2.2\n"));
}

#[test]
fn a_failed_candidate_is_not_silently_reprepared_or_published() {
    let fixture = Fixture::new();
    fixture.write("src/main.rs", "fn main() {}\n");
    fixture.commit("Fix model checking", "");
    passed(fixture.prepare("v0.1.0", "auto", None));
    let before = fixture.read("CHANGELOG.md");
    let output = fixture.prepare("v0.1.0", "auto", None);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("clean checkout"));
    assert_eq!(fixture.read("CHANGELOG.md"), before);
    assert!(fixture.git(&["tag", "--list"]).trim() == "v0.1.0");
}

#[test]
fn invalid_versions_or_missing_lock_entry_leave_checkout_untouched() {
    for previous in ["v0.1.0-beta", "vv0.1.0", "v00.1.0"] {
        let fixture = Fixture::new();
        assert!(!fixture.prepare(previous, "auto", None).status.success());
        assert!(fixture.git(&["status", "--porcelain"]).is_empty());
    }
    let fixture = Fixture::new();
    fixture.write("Cargo.lock", "version = 4\n");
    fixture.write("src/main.rs", "fn main() {}\n");
    fixture.commit("Change source with incomplete metadata", "");
    assert!(!fixture.prepare("v0.1.0", "auto", None).status.success());
    assert!(fixture.git(&["status", "--porcelain"]).is_empty());
    assert!(!Path::new(&fixture.output).exists());
}
