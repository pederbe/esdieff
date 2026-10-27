//! Selected samples keep their own input identity and inherited numeric context.
//! Follows ADR0025: Selected numeric samples.
#[cfg(unix)]
mod native_path_support;

use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-sample-cli-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
#[test]
fn sample_stdin_and_invocation_failures_have_the_shared_exit_rules() {
    let model = InputFile::new(br#"{"sdfProperty":{"p":{"type":"number","maximum":100}}}"#);
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            "-",
            "--format",
            "json",
            model.path(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"101").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["result"]["admission"], "rejected");
    let duplicate = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            "-",
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            "-",
            "-",
        ])
        .output()
        .unwrap();
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(duplicate.stdout.is_empty());
    let missing = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
}
#[test]
fn sample_acquisition_and_json_refusal_are_domain_reports() {
    let model = InputFile::new(br#"{"sdfProperty":{"p":{"type":"number"}}}"#);
    let missing_path = format!("{}.missing", model.path());
    let missing = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            &missing_path,
            "--format",
            "json",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(3));
    assert!(missing.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(wire["result"]["admission"], "undetermined");
    assert_eq!(wire["sources"][1]["role"], "sample");
    assert_eq!(wire["sources"][1]["acquisition"], "failed");
    let refused = InputFile::new(br#"{"sdfRequiredInputData":1,"sdfRequiredInputData":2}"#);
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            refused.path(),
            "--format",
            "json",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["result"]["admission"], "undetermined");
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "input.duplicate_name" && item["domain"] == "sample")
    );
    assert!(
        !wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "policy.historical_action_input")
    );
}
impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn needed_sample_decoding_cause_is_visible_in_the_cli_report() {
    let model = InputFile::new(include_bytes!("fixtures/temperature-device.sdf.json"));
    let sample = InputFile::new(b"1e1000001");
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature",
            "--sample",
            sample.path(),
            "--format",
            "json",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["result"]["admission"], "undetermined");
    let causes: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["code"] == "operation.limit" && c["message"] == "lexical exponent capability")
        .collect();
    assert_eq!(causes.len(), 1);
    assert_eq!(causes[0]["domain"], "operation");
    assert_eq!(causes[0]["primary"]["kind"], "sample");
    assert_eq!(causes[0]["primary"]["span"], serde_json::json!([0, 9]));
    assert_eq!(wire["diagnostics"]["omitted"], 0);
}

#[test]
fn device_property_rejects_101_at_its_override_with_an_original_sample_span() {
    let model = InputFile::new(include_bytes!("fixtures/temperature-device.sdf.json"));
    let sample = InputFile::new(b"101");
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfThing/Device/sdfObject/ambient/sdfProperty/temperature",
            "--sample",
            sample.path(),
            "--format",
            "json",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["version"], "1.1");
    assert_eq!(wire["operation"], "sample");
    assert_eq!(wire["result"]["admission"], "rejected");
    assert_eq!(
        wire["result"]["sample_location"]["span"],
        serde_json::json!([0, 3])
    );
    assert_eq!(wire["result"]["sample_location"]["pointer"], "");
    assert_eq!(wire["coverage"]["requested"]["count_unit"], "sample_area");
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
}

#[test]
fn sample_output_failure_has_the_shared_host_exit() {
    let model = InputFile::new(br#"{"sdfProperty":{"p":{"type":"number"}}}"#);
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            "-",
            "--format",
            "json",
            model.path(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(b"1").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(4));
    assert!(!output.stderr.is_empty());
}

#[test]
fn identical_model_and_sample_paths_keep_separate_source_identities() {
    let model = InputFile::new(br#"{"sdfProperty":{"p":{"type":"number"}}}"#);
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            model.path(),
            "--format",
            "json",
            model.path(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["sources"][0]["role"], "model");
    assert_eq!(wire["sources"][1]["role"], "sample");
    assert_ne!(wire["sources"][0]["id"], wire["sources"][1]["id"]);
    assert_eq!(wire["request"]["sample"], wire["sources"][1]["id"]);
}

#[cfg(unix)]
#[test]
fn native_sample_paths_preserve_acquisition_identity() {
    use std::os::unix::ffi::OsStringExt;
    let model = InputFile::new(br#"{"sdfProperty":{"p":{"type":"number","maximum":100}}}"#);
    for suffix in [
        std::ffi::OsString::from("温度"),
        std::ffi::OsString::from_vec(vec![0xff]),
    ] {
        let mut name =
            std::ffi::OsString::from(format!("esdieff-sample-native-{}-", std::process::id()));
        name.push(suffix);
        let sample = InputFile(std::env::temp_dir().join(name));
        let rejection = native_path_support::write(&sample.0, b"101");
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args([
                std::ffi::OsStr::new("sample"),
                std::ffi::OsStr::new("--document"),
                model.0.as_os_str(),
                std::ffi::OsStr::new("--kind"),
                std::ffi::OsStr::new("property"),
                std::ffi::OsStr::new("--pointer"),
                std::ffi::OsStr::new("/sdfProperty/p"),
                std::ffi::OsStr::new("--sample"),
                sample.0.as_os_str(),
                std::ffi::OsStr::new("--format"),
                std::ffi::OsStr::new("json"),
                model.0.as_os_str(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(if rejection.is_none() { 1 } else { 3 }),
            "filesystem rejection: {rejection:?}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            wire["sources"][1]["acquisition"],
            if rejection.is_none() {
                "acquired"
            } else {
                "failed"
            }
        );
        assert_eq!(
            wire["sources"][1]["label"],
            sample.0.to_string_lossy().as_ref()
        );
        if rejection.is_none() {
            assert_eq!(
                wire["result"]["sample_location"]["span"],
                serde_json::json!([0, 3])
            );
        } else {
            assert_eq!(wire["result"]["admission"], "undetermined");
        }
        if sample.0.to_str().is_none() {
            assert!(!std::path::Path::new(sample.0.to_string_lossy().as_ref()).exists());
        }
    }
}
