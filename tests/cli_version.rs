use std::process::Command;

#[test]
fn version_reports_the_package_version_without_terminal_styling() {
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .arg("--version")
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        format!("esdieff {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
}
