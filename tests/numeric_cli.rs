//! Explicit numeric advice follows ADR 0030: Bounded numeric advice.
use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], bytes: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn explicit_const_advice_preserves_cli_identity_and_strict_policy() {
    let bytes = br#"{"info":{},"sdfData":{"value":{"const":0.5,"minimum":1}}}"#;
    let initial = run(&["check", "--format", "json", "-"], bytes);
    assert_eq!(initial.status.code(), Some(0));
    let initial: serde_json::Value = serde_json::from_slice(&initial.stdout).unwrap();
    assert_eq!(initial["request"]["assessment"], "esdieff.initial/1");
    assert_eq!(initial["outcome"]["warnings"], 0);
    let normal = run(
        &[
            "check",
            "--assessment",
            "expanded-numeric-v1",
            "--format",
            "json",
            "-",
        ],
        bytes,
    );
    let strict = run(
        &[
            "check",
            "--assessment",
            "expanded-numeric-v1",
            "--strict-warnings",
            "--format",
            "json",
            "-",
        ],
        bytes,
    );
    assert_eq!(normal.status.code(), Some(0));
    assert_eq!(strict.status.code(), Some(1));
    assert!(normal.stderr.is_empty() && strict.stderr.is_empty());
    let normal: serde_json::Value = serde_json::from_slice(&normal.stdout).unwrap();
    let strict: serde_json::Value = serde_json::from_slice(&strict.stdout).unwrap();
    for wire in [&normal, &strict] {
        assert_inventory(wire);
        for identity in [
            &wire["request"]["assessment"],
            &wire["request"]["inventory"],
            &wire["coverage"]["requested"]["inventory"],
        ] {
            assert_eq!(identity, "esdieff.expanded.numeric/1");
        }
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert_eq!(wire["outcome"]["warnings"], 1);
        assert_eq!(wire["outcome"]["errors"], 0);
        assert_eq!(wire["outcome"]["complete"], true);
        let conditions = wire["diagnostics"]["items"].as_array().unwrap();
        let advice = conditions
            .iter()
            .find(|row| row["code"] == "proof.numeric_const_bound")
            .unwrap();
        assert_eq!(advice["severity"], "warning");
        assert_eq!(advice["basis"]["kind"], "advice");
        assert!(advice["arguments"].get("type").is_none());
    }
    assert_eq!(
        normal["outcome"]["incomplete_checks"],
        strict["outcome"]["incomplete_checks"]
    );
    let human = run(
        &["check", "--assessment", "expanded-numeric-v1", "-"],
        bytes,
    );
    assert_eq!(human.status.code(), Some(0));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("esdieff.expanded.numeric/1"));
    assert!(human.contains("proof.numeric_const_bound"));
    assert!(human.contains("warnings: 1"));
    let help = run(&["--help"], b"");
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("expanded-numeric-v1")
    );
}

fn assert_inventory(wire: &serde_json::Value) {
    let expected: std::collections::BTreeSet<_> = [
        "document-envelope/1",
        "base-roles-names/1",
        "metadata-namespaces/1",
        "established-composition/1",
        "basic-parameters/1",
        "alternatives-requiredness/1",
        "extensions-policies/1",
        "advisory-four/1",
        "advisory-numeric-three/1",
    ]
    .into_iter()
    .collect();
    for identity in [
        &wire["request"]["assessment"],
        &wire["request"]["inventory"],
        &wire["coverage"]["requested"]["inventory"],
    ] {
        assert_eq!(identity, "esdieff.expanded.numeric/1");
    }
    let rows = wire["coverage"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 9);
    assert_eq!(
        rows.iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    let mut sum = 0;
    for state in ["assessed", "not_applicable", "unassessed", "undetermined"] {
        let count = rows.iter().filter(|row| row["status"] == state).count() as u64;
        assert_eq!(wire["coverage"]["requested"][state], count);
        sum += count;
    }
    assert_eq!(sum, 9);
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
}

#[test]
fn interval_classes_preserve_cli_policy_and_unavailable_basis() {
    let bytes = br#"{"info":{},"sdfData":{"integer":{"type":"integer","nullable":false,"minimum":0.1,"maximum":0.9},"step":{"type":"number","nullable":false,"minimum":0.1,"maximum":0.2,"multipleOf":0.25}}}"#;
    for strict in [false, true] {
        let mut args = vec![
            "check",
            "--assessment",
            "expanded-numeric-v1",
            "--format",
            "json",
        ];
        if strict {
            args.push("--strict-warnings");
        }
        args.push("-");
        let output = run(&args, bytes);
        assert_eq!(output.status.code(), Some(i32::from(strict)));
        assert!(output.stderr.is_empty());
        let wire: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_inventory(&wire);
        assert_eq!(wire["outcome"]["warnings"], 2);
        assert_eq!(wire["outcome"]["complete"], true);
        for code in ["proof.integer_interval_gap", "proof.step_interval_gap"] {
            let row = wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["code"] == code)
                .unwrap();
            assert_eq!(row["severity"], "warning");
            assert_eq!(row["basis"]["kind"], "advice");
            assert_eq!(row["basis"]["id"], code);
            assert_eq!(row["assessment_unit"], "advisory-numeric-three/1");
        }
    }
    let unavailable = br#"{"info":{},"sdfData":{"value":{"type":"number","nullable":false,"minimum":1e1024,"maximum":2e1024,"multipleOf":1}}}"#;
    let output = run(
        &[
            "check",
            "--assessment",
            "expanded-numeric-v1",
            "--format",
            "json",
            "-",
        ],
        unavailable,
    );
    assert!(!output.status.success());
    let wire: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_inventory(&wire);
    assert_eq!(wire["outcome"]["complete"], false);
    let area = wire["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "advisory-numeric-three/1")
        .unwrap();
    assert_eq!(area["status"], "undetermined");
    assert_eq!(area["basis"]["kind"], "operational");
    assert!(area["basis"].get("citation").is_none());
    let row = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "proof.numeric_capability_unavailable")
        .unwrap();
    assert_eq!(row["basis"]["id"], "expanded-numeric-capability");
    assert!(row["basis"].get("citation").is_none());
}
