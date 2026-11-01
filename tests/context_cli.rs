use serde_json::Value;
const DRAFT: &str = "draft-ietf-asdf-sdf-nonaffordance-06";
/// Small source files for the public CLI examples, removed when the test ends.
struct TempFile(std::path::PathBuf);

impl TempFile {
    fn new(bytes: &[u8]) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-context-{}-{}.sdf.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn cli_checks_and_source_and_effective_inspection_identify_the_draft() {
    let model = TempFile::new(include_bytes!(
        "../examples/context-inspection/device.sdf.json"
    ));
    for args in [
        vec![
            "check",
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            model.path(),
        ],
        vec![
            "inspect",
            "--view",
            "source",
            "--document",
            model.path(),
            "--pointer",
            "/sdfObject/Device",
            "--format",
            "json",
            model.path(),
        ],
        vec![
            "inspect",
            "--view",
            "effective",
            "--document",
            model.path(),
            "--pointer",
            "/sdfObject/Device/sdfContext/installationLocation",
            "--format",
            "json",
            model.path(),
        ],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["baseline"]["extensions"][0]["id"], DRAFT, "{report}");
        assert_eq!(report["version"], "1.0");
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "check",
            "--assessment",
            "full-baseline",
            "--color",
            "never",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8(output.stdout).unwrap().contains(DRAFT));
}

#[test]
fn cli_context_placement_errors_keep_their_location_and_draft_basis() {
    let model = TempFile::new(br#"{"sdfContext":{"serial":{"type":"string"}}}"#);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "json", model.path()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let condition = report["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|condition| condition["code"] == "context.placement")
        .unwrap();
    assert_eq!(condition["primary"]["pointer"], "/sdfContext");
    assert_eq!(condition["basis"]["id"], DRAFT);
    assert_eq!(condition["basis"]["kind"], "interpretation");
}

#[test]
fn removing_context_preserves_client_interactions() {
    let old = TempFile::new(br#"{"sdfObject":{"O":{"sdfContext":{"c":{"type":"number"}}}}}"#);
    let new = TempFile::new(br#"{"sdfObject":{"O":{"sdfContext":{}}}}"#);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "diff",
            "--old",
            old.path(),
            "--new",
            new.path(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["result"]["differences"][0]["subject"], "sdfContext");
    assert_eq!(wire["result"]["differences"][0]["kind"], "removed");
    assert_eq!(
        wire["result"]["differences"][0]["classification"],
        "compatible"
    );
}
