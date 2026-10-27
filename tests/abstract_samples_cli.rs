//! The command and borrowed API share abstract-value outcomes and references.
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

struct File(std::path::PathBuf);
impl File {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-byte-posix-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn execute(args: &[&str], stdin: Option<&[u8]>) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = stdin {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    child.wait_with_output().unwrap()
}
fn scoped(human: &str, label: &str) -> Vec<String> {
    let found: Vec<_> = human
        .lines()
        .filter_map(|line| line.strip_prefix(label))
        .collect();
    assert_eq!(found.len(), 1, "one scoped report line: {label}");
    let mut values: Vec<_> = if found[0] == "none" {
        Vec::new()
    } else {
        found[0].split("; ").map(str::to_owned).collect()
    };
    values.sort_unstable();
    values
}
fn support(value: &Value) -> Vec<String> {
    let mut values: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|basis| {
            if basis["kind"] == "rfc" {
                basis["citation"].as_str().unwrap()
            } else {
                basis["id"].as_str().unwrap()
            }
            .to_owned()
        })
        .collect();
    values.sort_unstable();
    values
}
fn human_support(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(
            |basis| match (basis["id"].as_str(), basis["citation"].as_str()) {
                (Some(id), Some(citation)) => format!("{id} ({citation})"),
                (Some(id), None) => id.to_owned(),
                (None, Some(citation)) => citation.to_owned(),
                _ => panic!("reference has no identity"),
            },
        )
        .collect()
}
fn check_pair(
    model: &str,
    sample: &str,
    stdin: Option<&[u8]>,
    expected: &str,
    exit: i32,
    complete: bool,
    id: Option<&str>,
) -> Value {
    let run = |format| {
        execute(
            &[
                "sample",
                "--document",
                model,
                "--kind",
                "property",
                "--pointer",
                "/sdfProperty/p",
                "--sample",
                sample,
                "--format",
                format,
                model,
            ],
            stdin,
        )
    };
    let json = run("json");
    let human = run("human");
    assert_eq!(json.status.code(), Some(exit));
    assert_eq!(human.status.code(), Some(exit));
    assert!(json.stderr.is_empty() && human.stderr.is_empty());
    assert_eq!(json.stdout.last(), Some(&b'\n'));
    assert_eq!(human.stdout.last(), Some(&b'\n'));
    let value: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["schema"], "esdieff.report");
    assert_eq!(value["version"], "1.1");
    assert_eq!(value["operation"], "sample");
    assert_eq!(value["result"]["admission"], expected);
    assert_eq!(value["outcome"]["complete"], complete);
    assert_eq!(value["request"]["selector"], value["result"]["selector"]);
    assert_eq!(value["coverage"]["requested"]["total"], 9);
    assert_eq!(
        value["coverage"]["requested"]["inventory"],
        "esdieff.sample/1"
    );
    let counts = &value["coverage"]["requested"];
    assert_eq!(
        counts["assessed"].as_u64().unwrap()
            + counts["not_applicable"].as_u64().unwrap()
            + counts["unassessed"].as_u64().unwrap()
            + counts["undetermined"].as_u64().unwrap(),
        9
    );
    let sample_id = value["result"]["sample"].as_u64().unwrap();
    assert_eq!(value["result"]["sample_location"]["source_id"], sample_id);
    assert!(value["result"]["sample_location"]["span"].is_array());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(&format!("Sample {sample_id}: {expected}")));
    assert!(human.contains(&format!("Complete: {complete}")));
    // Scoped human lines and JSON references describe the same final result.
    for (field, label) in [
        ("abstract_values", "Sample abstract values"),
        ("const", "Sample const"),
        ("admission", "Sample admission"),
    ] {
        for (key, suffix) in [("basis", "basis"), ("limitations", "limitations")] {
            let references = human_support(&value["result"]["meaning"][field][key]);
            let prefix = format!("{label} {suffix}: ");
            let lines: Vec<_> = human
                .lines()
                .filter_map(|line| line.strip_prefix(&prefix))
                .collect();
            assert_eq!(lines.len(), 1);
            assert_eq!(
                lines[0],
                if references.is_empty() {
                    "none".to_owned()
                } else {
                    references.join("; ")
                }
            );
        }
    }
    let mut actual_ids: Vec<_> = value["baseline"]["interpretations"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| ["byte-string-json/1", "unix-time-posix-coordinate/1"].contains(id))
        .map(str::to_owned)
        .collect();
    actual_ids.sort_unstable();
    assert_eq!(
        scoped(&human, "Consulted sample interpretations: "),
        actual_ids
    );
    if let Some(id) = id {
        assert!(
            value["baseline"]["interpretations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry.as_str() == Some(id))
        );
        assert!(human.contains(id));
    }
    assert!(
        value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["id"] == sample_id && source["role"] == "sample")
    );
    value
}

#[test]
fn focused_domains_use_files_or_one_stdin_and_agree_in_both_formats() {
    let bytes = br#"{"info":{},"sdfProperty":{"p":{"type":"string","sdfType":"byte-string"}}}"#;
    let time = br#"{"info":{},"sdfProperty":{"p":{"type":"number","sdfType":"unix-time"}}}"#;
    let byte_model = File::new(bytes);
    let time_model = File::new(time);
    let valid = File::new(br#""Zg""#);
    let invalid = File::new(br#""_x""#);
    check_pair(
        byte_model.path(),
        valid.path(),
        None,
        "admitted",
        0,
        true,
        Some("byte-string-json/1"),
    );
    check_pair(
        "-",
        invalid.path(),
        Some(bytes),
        "rejected",
        1,
        true,
        Some("byte-string-json/1"),
    );
    check_pair(
        time_model.path(),
        "-",
        Some(b"0.1"),
        "admitted",
        0,
        true,
        Some("unix-time-posix-coordinate/1"),
    );
    // ADR 0007: Generated SDF and sample JSON retains exact negative input
    // without selecting an application denotation.
    check_pair(
        time_model.path(),
        "-",
        Some(b"-1"),
        "undetermined",
        3,
        false,
        Some("unix-time-posix-coordinate/1"),
    );
}

#[test]
fn focused_final_support_and_unfinished_cause_survive_consultation_history() {
    let integer =
        File::new(br#"{"info":{},"sdfProperty":{"p":{"type":"integer","sdfType":"unix-time"}}}"#);
    let negative = check_pair(
        integer.path(),
        "-",
        Some(b"-0.5"),
        "rejected",
        1,
        false,
        Some("unix-time-posix-coordinate/1"),
    );
    assert!(
        support(&negative["result"]["meaning"]["admission"]["basis"])
            .iter()
            .any(|id| id == "RFC 9880 Appendix C.1")
    );
    assert!(
        support(&negative["result"]["meaning"]["abstract_values"]["limitations"])
            .iter()
            .any(|id| id == "sample-abstract-denotation-unavailable")
    );
    let winner = File::new(br#"{"info":{},"sdfProperty":{"p":{"sdfChoice":{"a":{"type":"string","sdfType":"byte-string"},"b":{"type":"string"}}}}}"#);
    let ordinary = check_pair(
        winner.path(),
        "-",
        Some(br#""_x""#),
        "admitted",
        0,
        true,
        Some("byte-string-json/1"),
    );
    let basis = support(&ordinary["result"]["meaning"]["admission"]["basis"]);
    assert!(basis.iter().any(|id| id == "RFC 9880 Appendix C"));
    assert!(!basis.iter().any(|id| id == "byte-string-json/1"));
    assert_eq!(
        ordinary["result"]["meaning"]["const"]["status"],
        "not_applicable"
    );
    assert!(support(&ordinary["result"]["meaning"]["const"]["basis"]).is_empty());
    for unknown in [false, true] {
        let mut choices = serde_json::json!({
            "a":{"type":"string","sdfType":"byte-string","const":"Zg"},
            "b":{"type":"string","sdfType":"byte-string","const":"Zm8"}});
        if unknown {
            choices.as_object_mut().unwrap().insert(
                "c".into(),
                serde_json::json!({"type":"string","sdfType":"vendor"}),
            );
        }
        let model = File::new(
            serde_json::json!({"info":{},"sdfProperty":{"p":{"sdfChoice":choices}}})
                .to_string()
                .as_bytes(),
        );
        let outcome = check_pair(
            model.path(),
            "-",
            Some(br#""Zm9v""#),
            if unknown { "undetermined" } else { "rejected" },
            if unknown { 3 } else { 1 },
            !unknown,
            Some("byte-string-json/1"),
        );
        if unknown {
            assert!(support(&outcome["result"]["meaning"]["admission"]["basis"]).is_empty());
        } else {
            assert!(
                support(&outcome["result"]["meaning"]["admission"]["basis"])
                    .iter()
                    .any(|id| id == "RFC 9880 Section 4.7.2")
            );
        }
    }
}

#[test]
fn duplicate_stdin_missing_fields_and_unknown_options_are_usage_errors() {
    let model = File::new(br#"{"sdfProperty":{"p":{"type":"string","sdfType":"byte-string"}}}"#);
    let sample = File::new(br#""Zg""#);
    for args in [
        vec![
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
        ],
        vec![
            "sample",
            "--document",
            model.path(),
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            sample.path(),
            model.path(),
        ],
        vec![
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            model.path(),
        ],
        vec![
            "sample",
            "--document",
            "unknown",
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            sample.path(),
            model.path(),
        ],
        vec![
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            sample.path(),
            "--format",
            "unknown",
            model.path(),
        ],
    ] {
        let output = execute(&args, None);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn precise_joint_comparator_causes_agree_in_human_and_json() {
    for (format, constant, sample, ids) in [
        (
            "uri",
            "bad",
            "mailto:operator@example.test",
            vec![
                "sample-abstract-denotation-unavailable",
                "sample-establishment",
            ],
        ),
        (
            "date-time",
            "bad",
            "2030-06-30T23:59:60Z",
            vec![
                "sample-abstract-denotation-unavailable",
                "temporal-knowledge",
            ],
        ),
    ] {
        let model=File::new(serde_json::json!({"sdfProperty":{"p":{"type":"string","format":format,"const":constant}}}).to_string().as_bytes());
        let raw = serde_json::to_string(sample).unwrap();
        let wire = check_pair(
            model.path(),
            "-",
            Some(raw.as_bytes()),
            "undetermined",
            3,
            false,
            None,
        );
        let actual: Vec<_> = wire["result"]["meaning"]["const"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|reference| reference["id"].as_str().unwrap())
            .collect();
        assert_eq!(actual, ids);
        assert!(
            wire["result"]["meaning"]["const"]["limitations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|reference| reference["id"] != "sample-abstract-denotation-unavailable")
                .all(|reference| reference["citation"].is_string())
        );
        assert!(
            wire["result"]["meaning"]["admission"]["basis"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn missing_acquired_sample_is_a_domain_result_with_no_invented_null() {
    let model =
        File::new(br#"{"info":{},"sdfProperty":{"p":{"type":"string","sdfType":"byte-string"}}}"#);
    let missing = File::new(b"unused");
    std::fs::remove_file(&missing.0).unwrap();
    let output = execute(
        &[
            "sample",
            "--document",
            model.path(),
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
            missing.path(),
            "--format",
            "json",
            model.path(),
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["admission"], "undetermined");
    assert!(!value["outcome"]["complete"].as_bool().unwrap());
    assert!(value["result"]["sample_location"]["span"].is_null());
    assert!(
        value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["role"] == "sample" && source["acquisition"] == "failed")
    );
    assert!(
        value["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn malformed_and_duplicate_sample_json_keep_the_input_domain() {
    let model = File::new(br#"{"info":{},"sdfProperty":{"p":{"sdfType":"byte-string"}}}"#);
    for raw in [b"{bad".as_slice(), br#"{"x":1,"x":2}"#.as_slice()] {
        let sample = File::new(raw);
        for format in ["json", "human"] {
            let output = execute(
                &[
                    "sample",
                    "--document",
                    model.path(),
                    "--kind",
                    "property",
                    "--pointer",
                    "/sdfProperty/p",
                    "--sample",
                    sample.path(),
                    "--format",
                    format,
                    model.path(),
                ],
                None,
            );
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["result"]["admission"], "undetermined");
                assert!(wire["outcome"]["errors"].as_u64().unwrap() > 0);
                assert!(
                    wire["baseline"]["interpretations"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
                assert!(
                    !wire["diagnostics"]["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|item| item["code"] == "sample.byte_string_representation")
                );
            }
        }
    }
}

#[test]
fn strict_policy_uses_actual_counts_without_adding_unrequested_model_advice() {
    let model=File::new(br#"{"info":{},"sdfProperty":{"p":{"type":"string","sdfChoice":{"a":{"sdfType":"byte-string","extraQuality":true},"b":{}}}}}"#);
    let sample = File::new(br#""Zg""#);
    for strict in [false, true] {
        for format in ["human", "json"] {
            let mut args = vec![
                "sample",
                "--document",
                model.path(),
                "--kind",
                "property",
                "--pointer",
                "/sdfProperty/p",
                "--sample",
                sample.path(),
                "--format",
                format,
                model.path(),
            ];
            if strict {
                args.push("--strict-warnings");
            }
            let output = execute(&args, None);
            assert!(output.stderr.is_empty());
            assert_eq!(output.status.code(), Some(0));
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["result"]["admission"], "admitted");
                assert_eq!(wire["outcome"]["complete"], true);
                assert_eq!(wire["outcome"]["warnings"], 0);
                assert_eq!(wire["outcome"]["errors"], 0);
                assert_eq!(wire["policy"]["strict_warnings"], strict);
            }
        }
    }
}
