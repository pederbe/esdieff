//! CLI producers share declaration and recorded-evidence semantics.
//! Follows ADR 0029: Content-format declarations and recorded registry guidance.
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn run(bytes: &[u8], assessment: &str, format: &str, policy: Option<&str>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
    command.args(["check", "--assessment", assessment, "--format", format]);
    if let Some(policy) = policy {
        command.args(["--registry-knowledge", policy]);
    }
    command
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn expanded_option_adds_syntax_while_initial_option_keeps_the_same_shape_inventory() {
    let bytes = br#"{"info":{"title":"Authored format"},"sdfData":{"Payload":{"contentFormat":"text/plain;"}}}"#;
    let json = run(bytes, "expanded-content-format-v1", "json", None);
    assert_eq!(json.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(report["schema"], "esdieff.report");
    assert_eq!(report["operation"], "check");
    assert_eq!(
        report["request"]["assessment"],
        "esdieff.expanded.content-format/1"
    );
    assert_eq!(report["outcome"]["errors"], 1);
    assert_eq!(report["outcome"]["disposition"], "failed");
    assert_eq!(report["coverage"]["requested"]["total"], 10);
    let human = run(bytes, "expanded-content-format-v1", "human", None);
    assert_eq!(human.status.code(), json.status.code());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 1"));
    assert!(human.contains("content_format.syntax"));
    assert!(human.contains("content-format-syntax/1"));
    let initial = run(bytes, "initial-v1", "json", None);
    let initial: Value = serde_json::from_slice(&initial.stdout).unwrap();
    assert_eq!(initial["outcome"]["errors"], 0);
    assert_eq!(initial["coverage"]["requested"]["total"], 8);
    assert!(
        !initial["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "content_format.syntax")
    );
}

#[test]
fn registry_miss_is_incomplete_with_no_syntax_error_in_both_presentations() {
    let bytes = br#"{"info":{"title":"Authored format"},"sdfData":{"Payload":{"contentFormat":"application/unknown-model-format"}}}"#;
    let json = run(bytes, "expanded-content-format-v1", "json", None);
    assert_eq!(json.status.code(), Some(3));
    let report: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(report["outcome"]["errors"], 0);
    assert_eq!(report["outcome"]["complete"], false);
    assert_eq!(report["outcome"]["disposition"], "undetermined");
    assert!(
        report["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "content_format.registration_unknown")
    );
    let human = run(bytes, "expanded-content-format-v1", "human", None);
    assert_eq!(human.status.code(), json.status.code());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Errors: 0"));
    assert!(human.contains("content_format.registration_unknown"));
    // No full message text or diagnostic ordering is an automation contract.
}

#[test]
fn file_scope_and_source_effective_origins_match_the_shared_library_conclusion() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = root.join("tests/fixtures/content-format/base.sdf.json");
    let consumer = root.join("tests/fixtures/content-format/inherited.sdf.json");
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "check",
            "--assessment",
            "expanded-content-format-v1",
            "--format",
            "json",
            "--root",
        ])
        .arg(&consumer)
        .arg(&base)
        .arg(&consumer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["outcome"]["errors"], 1);
    let syntax = report["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "content_format.syntax")
        .unwrap();
    assert_eq!(syntax["primary"]["kind"], "effective");
    assert_eq!(
        syntax["primary"]["pointer"],
        "/sdfProperty/Payload/contentFormat"
    );
    assert!(
        syntax["related"].as_array().unwrap().iter().any(
            |loc| loc["kind"] == "source" && loc["pointer"] == "/sdfData/Payload/contentFormat"
        )
    );
}

#[test]
fn actual_policy_producers_keep_syntax_and_admitted_evidence_separate() {
    let bytes =
        br#"{"info":{"title":"Declared content"},"sdfData":{"Payload":{"contentFormat":"60"}}}"#;
    for (policy, exit, state) in [("recorded", 0, "available"), ("withheld", 3, "unavailable")] {
        for format in ["json", "human"] {
            let output = run(bytes, "expanded-content-format-v1", format, Some(policy));
            assert_eq!(output.status.code(), Some(exit));
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["outcome"]["errors"], 0);
                assert_eq!(
                    wire["policy"]["recorded_registry"],
                    if policy == "recorded" {
                        "use_recorded"
                    } else {
                        "withhold_recorded"
                    }
                );
                assert_eq!(wire["baseline"]["registry_knowledge"][0]["state"], state);
                if policy == "withheld" {
                    assert!(wire["baseline"]["registry_knowledge"][0]["snapshot"].is_null());
                    assert_eq!(
                        wire["baseline"]["registry_knowledge"][0]["reason"],
                        "caller_withheld"
                    );
                }
            } else {
                let human = String::from_utf8(output.stdout).unwrap();
                assert!(human.contains("content-format-syntax/1"));
                assert!(human.contains("coap-content-formats"));
            }
        }
    }
    let malformed = br#"{"sdfData":{"Payload":{"contentFormat":"text/plain;"}}}"#;
    assert_eq!(
        run(
            malformed,
            "expanded-content-format-v1",
            "json",
            Some("withheld")
        )
        .status
        .code(),
        Some(1)
    );
    for assessment in ["initial-v1", "expanded-modified-v1"] {
        for policy in ["recorded", "withheld"] {
            let output = run(&[], assessment, "json", Some(policy));
            assert_eq!(output.status.code(), Some(2)); // Usage/configuration error, not model disposition.
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "esdieff-content-policy-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let model = directory.join("model.json");
    let sample = directory.join("sample.json");
    std::fs::write(
        &model,
        br#"{"sdfProperty":{"Payload":{"type":"string","contentFormat":"60"}}}"#,
    )
    .unwrap();
    std::fs::write(&sample, br#""text""#).unwrap();
    for policy in ["recorded", "withheld"] {
        let inspect = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["inspect", "--document"])
            .arg(&model)
            .args([
                "--view",
                "effective",
                "--pointer",
                "/sdfProperty/Payload",
                "--registry-knowledge",
                policy,
            ])
            .arg(&model)
            .output()
            .unwrap();
        assert_eq!(inspect.status.code(), Some(2));
        let sample_output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["sample", "--document"])
            .arg(&model)
            .args([
                "--kind",
                "property",
                "--pointer",
                "/sdfProperty/Payload",
                "--sample",
            ])
            .arg(&sample)
            .args(["--registry-knowledge", policy])
            .arg(&model)
            .output()
            .unwrap();
        assert_eq!(sample_output.status.code(), Some(2));
    }
    std::fs::remove_dir_all(directory).unwrap(); // This caller owns the unique created directory.
}
