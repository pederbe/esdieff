//! Exercise shared operations and exit codes through explicit command-line inputs.
//! Follows ADR0010: Domain-oriented library and caller simplicity, and
//! ADR0013: Shared workflow and report guarantees.

mod coverage_support;
#[cfg(unix)]
mod native_path_support;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn run(args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn stdin_check_completes_initial_work_in_both_formats() {
    let output = run(&["check", "--format", "json", "-"], b"{}");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    coverage_support::assert_wire_counts(&json);
    assert_eq!(json["sources"][0]["content_complete"], true);
    let human = run(&["check", "-"], b"{}");
    assert_eq!(human.status.code(), Some(0));
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("incomplete checks: 0")
    );
}

#[test]
fn expanded_modified_request_checks_metadata_in_both_formats() {
    let source = br#"{"info":{"modified":"1900-02-29"}}"#;
    let output = run(
        &[
            "check",
            "--assessment",
            "expanded-modified-v1",
            "--format",
            "json",
            "-",
        ],
        source,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["request"]["assessment"], "esdieff.expanded.modified/1");
    assert_eq!(
        wire["coverage"]["requested"]["inventory"],
        "esdieff.expanded.modified/1"
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    coverage_support::assert_wire_counts(&wire);
    assert_eq!(wire["outcome"]["errors"], 1);
    coverage_support::assert_wire_counts(&wire);
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "metadata.modified_calendar")
    );

    let human = run(
        &["check", "--assessment", "expanded-modified-v1", "-"],
        source,
    );
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("esdieff.expanded.modified/1"));
    assert!(human.contains("Assessment area modified-date-time/1: assessed"));
    assert!(human.contains("metadata.modified_calendar"));
    let ordinary = run(&["check", "--format", "json", "-"], source);
    assert_eq!(ordinary.status.code(), Some(0));
    let wire: Value = serde_json::from_slice(&ordinary.stdout).unwrap();
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    assert_eq!(wire["outcome"]["errors"], 0);
}

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(content: &[u8]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-cli-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, content).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn explicit_files_and_inspection_emit_one_source_report() {
    let file = InputFile::new(br#"{"sdfData":{"Count":{"type":"integer","maximum":100}}}"#);
    let output = run(
        &[
            "inspect",
            "--document",
            file.path(),
            "--pointer",
            "/sdfData/Count",
            "--format",
            "json",
            file.path(),
        ],
        b"",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["json"]["maximum"], 100);
    assert_eq!(value["sources"][0]["acquisition"], "acquired");
    let human = run(
        &[
            "inspect",
            "--document",
            file.path(),
            "--pointer",
            "/sdfData/Count",
            file.path(),
        ],
        b"",
    );
    assert_eq!(human.status.code(), Some(0));
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("\"maximum\":100")
    );
}

#[test]
fn file_acquisition_and_usage_failures_have_distinct_exits() {
    let missing = std::env::temp_dir().join(format!("esdieff-missing-{}.json", std::process::id()));
    let output = run(
        &["check", "--format", "json", missing.to_str().unwrap()],
        b"",
    );
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["sources"][0]["acquisition"], "failed");
    assert_eq!(json["outcome"]["errors"], 0);
    for args in [
        vec!["check"],
        vec!["check", "-", "-"],
        vec!["check", "--root", "missing", "-"],
        vec!["check", "--format", "nonsense", "-"],
        vec!["inspect", "--document", "-", "-"],
    ] {
        let output = run(&args, b"");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let malformed = run(&["check", "--format", "json", "-"], b"[]");
    assert_eq!(malformed.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&malformed.stdout).unwrap();
    assert_eq!(json["outcome"]["complete"], false);
    assert_eq!(json["outcome"]["errors"], 1);
}

#[test]
fn output_failure_uses_host_failure_exit() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(4));
    assert!(!output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn native_paths_preserve_acquisition_identity_with_safe_labels() {
    use std::os::unix::ffi::OsStringExt;
    for suffix in [
        std::ffi::OsString::from("温度"),
        std::ffi::OsString::from_vec(vec![0xff]),
    ] {
        let mut name = std::ffi::OsString::from(format!("esdieff-native-{}-", std::process::id()));
        name.push(suffix);
        let file = InputFile(std::env::temp_dir().join(name));
        let rejection = native_path_support::write(&file.0, b"{}");
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args([
                std::ffi::OsStr::new("check"),
                std::ffi::OsStr::new("--format"),
                std::ffi::OsStr::new("json"),
                file.0.as_os_str(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(if rejection.is_none() { 0 } else { 3 }),
            "filesystem rejection: {rejection:?}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            json["sources"][0]["acquisition"],
            if rejection.is_none() {
                "acquired"
            } else {
                "failed"
            }
        );
        assert_eq!(
            json["sources"][0]["label"],
            file.0.to_string_lossy().as_ref()
        );
        if file.0.to_str().is_none() {
            assert!(!std::path::Path::new(file.0.to_string_lossy().as_ref()).exists());
        }
    }
}

#[test]
fn names_namespaces_and_grouping_findings_agree_in_json_and_human_outputs() {
    let text = br#"{
        "info":{}, "namespace":{"home":"relative/path"}, "defaultNamespace":"home",
        "sdfThing":{"Device":{"minItems":-1,"sdfProperty":{"bad:name":{}}}}
    }"#;
    let output = run(&["check", "--format", "json", "-"], text);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 3);
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(wire["coverage"]["requested"]["total"], 8);
    let codes: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|condition| condition["code"].as_str())
        .collect();
    for code in ["name.colon", "namespace.uri", "grouping.count"] {
        assert!(codes.contains(&code));
    }
    let human = run(&["check", "-"], text);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stderr.is_empty());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("Errors: 3"));
    assert!(text.contains("incomplete checks: 0"));
    for code in ["name.colon", "namespace.uri", "grouping.count"] {
        assert!(text.contains(code));
    }
}

#[test]
fn cli_keeps_ordinary_names_and_permitted_extension_roles_inspectable() {
    let text = br#"{
        "namespace":{"shared":"urn:example:device#model"},"defaultNamespace":"shared",
        "sdfThing":{"Room /~ Device":{"minItems":1.00e3,"maxItems":1e1000000}},
        "sdfObject":{"Sensor":{"sdfThing":{"extension:name":{}}}}
    }"#;
    let checked = run(&["check", "--format", "json", "-"], text);
    assert_eq!(checked.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 0);
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "structure.role_unavailable")
    );
    let inspection = run(
        &[
            "inspect",
            "--document",
            "-",
            "--pointer",
            "/sdfThing/Room ~1~0 Device",
            "--format",
            "json",
            "-",
        ],
        text,
    );
    assert_eq!(inspection.status.code(), Some(0));
    assert!(inspection.stderr.is_empty());
    // Inspect output retains the accepted exact model token, independent of a consumer's number backend.
    assert!(
        String::from_utf8(inspection.stdout)
            .unwrap()
            .contains("1e1000000")
    );
}

#[test]
fn cli_defers_unknown_critical_structural_meaning_before_any_semantic_error() {
    let text = br#"{
        "sdfThing":{"bad:Name":{"minItems":-1}},
        "namespace":{"home":"relative/path"},"defaultNamespace":"missing",
        "info":{"features":[{"future":true}]}
    }"#;
    let output = run(&["check", "--format", "json", "-"], text);
    assert_eq!(output.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 0);
    coverage_support::assert_wire_counts(&wire);
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "structure.feature_context_unavailable")
    );
    let human = run(&["check", "-"], text);
    assert_eq!(human.status.code(), Some(3));
    let output = String::from_utf8(human.stdout).unwrap();
    assert!(output.contains("Errors: 0"));
    assert!(output.contains("structure.feature_context_unavailable"));
    assert!(output.contains("extension.feature_declaration_unavailable"));
}

#[test]
fn metadata_vocabulary_policy_and_strict_warning_outcomes_agree_across_cli_formats() {
    let text = br#"{
        "info":{"title":false,"versoin":"typo","example:note":{"sdfRequiredInputData":true}},
        "sdfRequiredInputData":true,
        "sdfData":{"value":{"type":"number","const":9007199254740993.125}}
    }"#;
    let json = run(&["check", "--format", "json", "-"], text);
    let human = run(&["check", "-"], text);
    assert_eq!(json.status.code(), Some(1));
    assert_eq!(human.status.code(), Some(1));
    assert!(json.stderr.is_empty());
    assert!(human.stderr.is_empty());
    let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 2);
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["outcome"]["notices"], 1);
    coverage_support::assert_wire_counts(&wire);
    assert_eq!(
        wire["policy"]["restrictions"],
        serde_json::json!(["A001", "A002"])
    );
    let historical = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "policy.historical_action_input")
        .unwrap();
    assert_eq!(historical["basis"]["kind"], "acceptance");
    assert_eq!(historical["basis"]["id"], "A001");
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 2; warnings: 1; notices: 1; incomplete checks: 0"));
    assert!(human.contains("Acceptance restrictions: A001, A002"));
    for code in [
        "metadata.string",
        "quality.unfamiliar",
        "policy.historical_action_input",
        "extension.optional_unsupported",
    ] {
        assert!(human.contains(code));
    }
    let strict_json = run(
        &["check", "--strict-warnings", "--format", "json", "-"],
        b"{}",
    );
    let strict_human = run(&["check", "--strict-warnings", "-"], b"{}");
    assert_eq!(strict_json.status.code(), Some(1));
    assert_eq!(strict_human.status.code(), Some(1));
    let wire: Value = serde_json::from_slice(&strict_json.stdout).unwrap();
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(wire["outcome"]["warnings"], 1);
    assert_eq!(wire["policy"]["strict_warnings"], true);
    let inspected = run(
        &[
            "inspect",
            "--document",
            "-",
            "--pointer",
            "/sdfData/value",
            "--format",
            "json",
            "-",
        ],
        text,
    );
    assert_eq!(inspected.status.code(), Some(0));
    let inspected = String::from_utf8(inspected.stdout).unwrap();
    assert!(inspected.contains("9007199254740993.125"));
    let wire: Value = serde_json::from_str(&inspected).unwrap();
    assert_eq!(wire["policy"]["restrictions"], serde_json::json!(["A002"]));
}
