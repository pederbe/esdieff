#![cfg(target_os = "linux")]

use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Candidate(PathBuf);

impl Candidate {
    fn new() -> Self {
        let root = PathBuf::from("/var/tmp").join(format!(
            "esdieff-publication-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for name in ["bin", "target/distrib", "remote/assets", "temporary"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        let compile = Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "-Dunsafe_code"])
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/gh.rs"))
            .arg("-o")
            .arg(root.join("bin/gh"))
            .output()
            .unwrap();
        assert!(compile.status.success(), "{compile:?}");
        let this = Self(root);
        for (name, data) in [
            ("archive.tar.xz", "retained executable bytes"),
            ("esdieff-0.1.0.crate", "retained crate bytes"),
            ("RELEASE-NOTES.md", "First release notes"),
            ("SOURCE", SHA),
        ] {
            fs::write(this.0.join("target/distrib").join(name), data).unwrap();
        }
        let mut checksums = String::new();
        for path in this.files() {
            let digest: String = Sha256::digest(fs::read(&path).unwrap())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            checksums.push_str(&format!(
                "{digest}  {}\n",
                path.file_name().unwrap().to_str().unwrap()
            ));
        }
        fs::write(this.0.join("target/distrib/SHA256SUMS"), checksums).unwrap();
        fs::write(this.0.join("remote/state"), "missing").unwrap();
        this
    }

    fn files(&self) -> Vec<PathBuf> {
        let mut files: Vec<_> = fs::read_dir(self.0.join("target/distrib"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        files.sort();
        files
    }

    fn publish_existing(&self) {
        fs::write(self.0.join("remote/state"), "false").unwrap();
        for path in self.files() {
            fs::copy(
                &path,
                self.0.join("remote/assets").join(path.file_name().unwrap()),
            )
            .unwrap();
        }
    }

    fn stage(&self) -> Output {
        let workflow = include_str!("../../../.github/workflows/release.yml");
        let block = workflow
            .split_once("      - name: Verify the retained candidate and stage the release\n")
            .unwrap()
            .1
            .split_once("        run: |\n")
            .unwrap()
            .1;
        let script = block
            .lines()
            .take_while(|line| line.is_empty() || line.starts_with("          "))
            .map(|line| line.strip_prefix("          ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(script.contains("cmp \"$file\""));
        Command::new("bash")
            .args(["-c", &script])
            .current_dir(&self.0)
            .env_clear()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.0.join("bin").display()),
            )
            .env("TAG", "v0.1.0")
            .env("GITHUB_REF", "refs/tags/v0.1.0")
            .env("GITHUB_SHA", SHA)
            .env("RUNNER_TEMP", self.0.join("temporary"))
            .env("FAKE_GH_ROOT", self.0.join("remote"))
            .output()
            .unwrap()
    }

    fn calls(&self, operation: &str) -> usize {
        fs::read_to_string(self.0.join("remote/calls"))
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with(&format!("release\t{operation}\t")))
            .count()
    }
}

impl Drop for Candidate {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn new_draft_uploads_downloads_and_checks_every_retained_asset() {
    let candidate = Candidate::new();
    let output = candidate.stage();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(candidate.calls("create"), 1);
    assert_eq!(candidate.calls("upload"), candidate.files().len());
    assert_eq!(candidate.calls("download"), candidate.files().len());
    for file in candidate.files() {
        assert_eq!(
            fs::read(&file).unwrap(),
            fs::read(
                candidate
                    .0
                    .join("remote/assets")
                    .join(file.file_name().unwrap())
            )
            .unwrap()
        );
    }
}

#[test]
fn newly_uploaded_corruption_is_rejected_by_the_real_byte_comparison() {
    let candidate = Candidate::new();
    fs::write(candidate.0.join("remote/corrupt-upload"), "").unwrap();
    let output = candidate.stage();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("differ"));
    assert_eq!(candidate.calls("upload"), 1);
    assert_eq!(candidate.calls("download"), 1);
}

#[test]
fn published_matching_retry_checks_bytes_without_mutating_the_release() {
    let candidate = Candidate::new();
    candidate.publish_existing();
    let output = candidate.stage();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(candidate.calls("create"), 0);
    assert_eq!(candidate.calls("upload"), 0);
    assert_eq!(candidate.calls("download"), candidate.files().len());
}

#[test]
fn published_mismatched_or_missing_assets_are_refused_without_upload() {
    for missing in [false, true] {
        let candidate = Candidate::new();
        candidate.publish_existing();
        let archive = candidate.0.join("remote/assets/archive.tar.xz");
        if missing {
            fs::remove_file(archive).unwrap();
        } else {
            fs::write(archive, "different published bytes").unwrap();
        }
        let output = candidate.stage();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert_eq!(candidate.calls("create"), 0);
        assert_eq!(candidate.calls("upload"), 0);
    }
}

#[test]
fn local_checksum_tampering_stops_before_any_release_operation() {
    let candidate = Candidate::new();
    fs::write(
        candidate.0.join("target/distrib/archive.tar.xz"),
        "tampered",
    )
    .unwrap();
    let output = candidate.stage();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(candidate.calls("view"), 0);
    assert_eq!(candidate.calls("create"), 0);
    assert_eq!(candidate.calls("upload"), 0);
}
