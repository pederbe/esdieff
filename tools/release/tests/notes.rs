use std::{fs, process::Command};

#[test]
fn release_notes_select_only_the_tagged_section() {
    let root = std::env::temp_dir().join(format!("esdieff-release-notes-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n## [0.3.1] - 2026-11-09\n\n### Fixed\n\n- Keep the selected section.\n\n## [0.3.0] - 2026-11-02\n\n- Earlier release.\n",
    )
    .unwrap();
    let run = |tag: &str| {
        Command::new(env!("CARGO_BIN_EXE_esdieff-release"))
            .current_dir(&root)
            .args(["release-notes", tag, "notes.md"])
            .output()
            .unwrap()
    };
    let output = run("v0.3.1");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("notes.md")).unwrap(),
        "### Fixed\n\n- Keep the selected section.\n"
    );
    assert!(!run("v0.4.0").status.success());
    fs::remove_dir_all(&root).unwrap();
}
