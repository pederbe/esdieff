use serde_json::Value;
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};
// Shared assertions keep CLI parity checks separate from library test bodies.
#[path = "resource_support/cli_use.rs"]
mod format_use_contract;
use format_use_contract::{
    BARE, GENERIC, HTTPS, OPAQUE, assert_wire_and_human_use, relative_rule_ids,
};

struct InputFile(PathBuf);
impl InputFile {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-uri-uuid-{}-{}.json",
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
impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn run(model: &InputFile, kind: &str, pointer: &str, sample: &[u8], format: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            model.path(),
            "--kind",
            kind,
            "--pointer",
            pointer,
            "--sample",
            "-",
            "--format",
            format,
            model.path(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(sample).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn all_three_formats_expose_admission_rejection_and_unknown_with_human_agreement() {
    for (format, sample, admission, exit, complete, used) in [
        (
            "uri",
            br#""https://example.test/#r""#.as_slice(),
            "admitted",
            0,
            true,
            &[GENERIC, HTTPS][..],
        ),
        (
            "uri",
            br#""https:device""#.as_slice(),
            "rejected",
            1,
            true,
            &[GENERIC, HTTPS][..],
        ),
        (
            "uri",
            br#""custom:device""#.as_slice(),
            "undetermined",
            3,
            false,
            &[GENERIC][..],
        ),
        (
            "uri-reference",
            br#""""#.as_slice(),
            "admitted",
            0,
            true,
            relative_rule_ids(),
        ),
        (
            "uri-reference",
            br#""1thing:part""#.as_slice(),
            "rejected",
            1,
            true,
            &[GENERIC][..],
        ),
        (
            "uri-reference",
            br#""custom:device""#.as_slice(),
            "undetermined",
            3,
            false,
            &[GENERIC][..],
        ),
        (
            "uuid",
            br#""F81D4FAE-7DEC-F1D0-F765-00A0C91E6BF6""#.as_slice(),
            "admitted",
            0,
            true,
            &[BARE, OPAQUE][..],
        ),
        (
            "uuid",
            br#""{f81d4fae-7dec-11d0-a765-00a0c91e6bf6}""#.as_slice(),
            "rejected",
            1,
            true,
            &[BARE][..],
        ),
    ] {
        let bytes = format!(
            r#"{{"info":{{}},"sdfData":{{"V":{{"type":"string","nullable":false,"format":"{format}"}}}}}}"#
        );
        let model = InputFile::new(bytes.as_bytes());
        let json = run(&model, "data", "/sdfData/V", sample, "json");
        let human = run(&model, "data", "/sdfData/V", sample, "human");
        assert_eq!(json.status.code(), Some(exit));
        assert_eq!(human.status.code(), Some(exit));
        assert!(json.stderr.is_empty() && human.stderr.is_empty());
        assert_eq!(json.stdout.last(), Some(&b'\n'));
        assert_eq!(human.stdout.last(), Some(&b'\n'));
        let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(wire["result"]["admission"], admission);
        assert_eq!(wire["outcome"]["complete"], complete);
        assert_eq!(wire["request"]["selector"]["kind"], "data");
        assert_eq!(wire["request"]["selector"]["pointer"], "/sdfData/V");
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert_eq!(wire["sources"][1]["role"], "sample");
        assert_eq!(
            wire["result"]["sample_location"]["span"],
            serde_json::json!([0, sample.len()])
        );
        let text = String::from_utf8(human.stdout).unwrap();
        // Existing human labels are used only to extract equivalent observations.
        // Do not freeze whole prose, ordering or unspecified condition messages.
        assert!(text.contains(&format!(": {admission}; context ")));
        assert!(text.contains(&format!("Complete: {complete}")));
        assert!(text.contains(wire["outcome"]["disposition"].as_str().unwrap()));
        assert!(text.contains("/sdfData/V"));
        assert!(text.contains("sample_area"));
        assert_wire_and_human_use(&wire, &text, used);
    }
    let unknown_uuid = InputFile::new(br#"{"info":{"features":["https://example.test/unknown-critical"]},"sdfData":{"V":{"type":"string","format":"uuid"}}}"#);
    let sample = br#""f81d4fae-7dec-f1d0-f765-00a0c91e6bf6""#;
    let mut outputs = Vec::new();
    for format in ["json", "human"] {
        let output = run(&unknown_uuid, "data", "/sdfData/V", sample, format);
        assert_eq!(output.status.code(), Some(3));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["result"]["admission"], "undetermined");
            assert_eq!(wire["outcome"]["errors"], 0);
        }
        outputs.push(output.stdout);
    }
    let wire: Value = serde_json::from_slice(&outputs[0]).unwrap();
    assert_unavailable(&wire, std::str::from_utf8(&outputs[1]).unwrap());
}

#[test]
fn finite_acquisition_is_a_domain_result_for_each_format_and_stdin_roles_are_explicit() {
    // Runtime-generated cap control, no dataset has been generated by preparation.
    // File sample avoids a pipe failure if the finite CLI stops reading stdin.
    let over_cap = vec![b' '; 4 * 1024 * 1024 + 1];
    let sample = InputFile::new(&over_cap);
    for format in ["uri", "uri-reference", "uuid"] {
        let bytes =
            format!(r#"{{"info":{{}},"sdfData":{{"V":{{"type":"string","format":"{format}"}}}}}}"#);
        let model = InputFile::new(bytes.as_bytes());
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args([
                "sample",
                "--document",
                model.path(),
                "--kind",
                "data",
                "--pointer",
                "/sdfData/V",
                "--sample",
                sample.path(),
                "--format",
                "json",
                model.path(),
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(3));
        let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(wire["result"]["admission"], "undetermined");
        assert!(!wire["outcome"]["complete"].as_bool().unwrap());
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["basis"]["kind"] != "rfc" || c["severity"] != "error")
        );
        // Incomplete input returns before the consultation owner is admitted.
        assert!(
            wire.get("baseline")
                .unwrap()
                .get("format_knowledge")
                .unwrap()
                .is_null()
        );
        let human = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args([
                "sample",
                "--document",
                model.path(),
                "--kind",
                "data",
                "--pointer",
                "/sdfData/V",
                "--sample",
                sample.path(),
                "--format",
                "human",
                model.path(),
            ])
            .output()
            .unwrap();
        assert_eq!(human.status.code(), Some(3));
        assert!(human.stderr.is_empty());
        assert_unavailable(&wire, std::str::from_utf8(&human.stdout).unwrap());
    }
    let duplicate = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "sample",
            "--document",
            "-",
            "--kind",
            "data",
            "--pointer",
            "/sdfData/V",
            "--sample",
            "-",
            "-",
        ])
        .output()
        .unwrap();
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(duplicate.stdout.is_empty());
}
fn assert_unavailable(wire: &Value, human: &str) {
    assert!(
        wire.get("baseline")
            .unwrap()
            .get("format_knowledge")
            .unwrap()
            .is_null()
    );
    assert!(human.contains("Format knowledge: unavailable (initial owner)"));
    assert!(!human.contains("Interpretation: uuid-"));
}

#[test]
fn five_selector_kinds_reach_the_same_uuid_semantics() {
    let model = InputFile::new(br#"{"info":{},"sdfData":{"V":{"type":"string","format":"uuid"}},"sdfProperty":{"p":{"type":"string","format":"uuid"}},"sdfAction":{"a":{"sdfInputData":{"type":"string","format":"uuid"},"sdfOutputData":{"type":"string","format":"uuid"}}},"sdfEvent":{"e":{"sdfOutputData":{"type":"string","format":"uuid"}}}}"#);
    for (kind, pointer, description) in [
        ("data", "/sdfData/V", "/sdfData/V"),
        ("property", "/sdfProperty/p", "/sdfProperty/p"),
        ("action_input", "/sdfAction/a", "/sdfAction/a/sdfInputData"),
        (
            "action_output",
            "/sdfAction/a",
            "/sdfAction/a/sdfOutputData",
        ),
        ("event_output", "/sdfEvent/e", "/sdfEvent/e/sdfOutputData"),
    ] {
        let output = run(
            &model,
            kind,
            pointer,
            br#""00000000-0000-0000-0000-000000000000""#,
            "json",
        );
        assert_eq!(output.status.code(), Some(0));
        let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(wire["request"]["selector"]["kind"], kind);
        assert_eq!(wire["result"]["description"]["pointer"], description);
        let human = run(
            &model,
            kind,
            pointer,
            br#""00000000-0000-0000-0000-000000000000""#,
            "human",
        );
        assert_eq!(human.status.code(), Some(0));
        assert_wire_and_human_use(
            &wire,
            std::str::from_utf8(&human.stdout).unwrap(),
            &[BARE, OPAQUE],
        );
    }
}
