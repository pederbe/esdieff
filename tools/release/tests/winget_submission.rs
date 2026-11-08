use std::{fs, process::Command};

#[test]
fn failed_body_update_resumes_existing_pr_without_resubmission() {
    let root = std::env::temp_dir().join(format!("winget-submission-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::create_dir(root.join("manifests")).unwrap();
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let gh = root.join(format!("bin/gh{extension}"));
    let status = Command::new("rustc")
        .args(["--edition=2024", "-Dunsafe_code", "-Dwarnings"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/winget-gh.rs"
        ))
        .arg("-o")
        .arg(&gh)
        .status()
        .unwrap();
    assert!(status.success());
    let client = root.join(format!("bin/wingetcreate{extension}"));
    fs::copy(&gh, &client).unwrap();
    for (suffix, kind) in [
        ("", "version"),
        (".installer", "installer"),
        (".locale.en-US", "defaultLocale"),
    ] {
        fs::write(
            root.join(format!("manifests/Publisher.Tool{suffix}.yaml")),
            format!("# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.1.28.0.schema.json\nPackageIdentifier: Publisher.Tool\nPackageVersion: 1.2.3\nManifestType: {kind}\nManifestVersion: 1.28.0\n"),
        )
        .unwrap();
    }
    fs::write(root.join("template"), "## 📖 Description\n\n## ✅ Checklist\n- [ ] Signed the [Contributor License Agreement](url)\n## 📦 Manifest Checklist\n- [ ] This PR only modifies one (1) manifest\n").unwrap();
    fs::write(root.join("fail-patch"), "yes").unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_esdieff-release"))
            .args([
                "winget-submit",
                root.join("manifests").to_str().unwrap(),
                "Publisher.Tool",
                "1.2.3",
                "--submit",
            ])
            .current_dir(&root)
            .env("PATH", &path)
            .env("FIXTURE_ROOT", &root)
            .env("WINGET_SUBMIT", &client)
            .env("GH_TOKEN", "fixture-token")
            .env("WINGET_CHECKED", "true")
            .env("GITHUB_REPOSITORY", "fixture/tool")
            .env("GITHUB_RUN_ID", "123")
            .output()
            .unwrap()
    };
    let failed = run();
    assert!(!failed.status.success(), "{failed:?}");
    assert!(root.join("created").exists());
    fs::remove_file(root.join("fail-patch")).unwrap();
    let retried = run();
    assert!(retried.status.success(), "{retried:?}");
    let body = fs::read_to_string(root.join("body-json")).unwrap();
    assert!(body.contains("- [x] Signed"));
    assert!(body.contains("actions/runs/123"));
    assert!(body.contains("Maintainer note."));
    assert!(!body.contains("Pull request has been created with"));
    assert!(run().status.success());
    fs::remove_dir_all(&root).unwrap();
}
