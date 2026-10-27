//! Recorded extension errors and evidence through the CLI, following ADR 0038: Recorded
//! extension vocabulary.
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

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
fn an_excluded_oma_id_is_a_registration_error_in_json_and_human_reports() {
    // The Object ID written as a string, as a converter might emit it.
    let bytes = String::from_utf8(
        include_bytes!("fixtures/recorded-extension/temperature.sdf.json").to_vec(),
    )
    .unwrap()
    .replace(r#""oma:id": 3303"#, r#""oma:id": "3303""#);
    for (policy, code) in [("recorded", 1), ("withheld", 3)] {
        let output = |format| {
            run(
                &[
                    "check",
                    "--assessment",
                    "full-baseline",
                    "--registry-knowledge",
                    policy,
                    "--format",
                    format,
                    "-",
                ],
                bytes.as_bytes(),
            )
        };
        let (json, human) = (output("json"), output("human"));
        assert_eq!(
            (json.status.code(), human.status.code()),
            (Some(code), Some(code)),
            "{policy}"
        );
        assert!(json.stderr.is_empty() && human.stderr.is_empty());
        let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
        let human = String::from_utf8(human.stdout).unwrap();
        let errors: Vec<_> = wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["code"] == "extension.recorded_value_excluded")
            .collect();
        let record = wire["baseline"]["registry_knowledge"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["registry"] == "omna.sdf-quality-names")
            .unwrap();
        if policy == "withheld" {
            assert!(errors.is_empty());
            assert_eq!(wire["outcome"]["errors"], 0);
            assert_eq!(
                (&record["state"], &record["reason"], &record["snapshot"]),
                (
                    &json!("unavailable"),
                    &json!("caller_withheld"),
                    &Value::Null
                )
            );
            assert!(!human.contains("extension.recorded_value_excluded"));
            continue;
        }
        assert_eq!(wire["outcome"]["errors"], 1);
        assert_eq!(wire["outcome"]["disposition"], "failed");
        assert_eq!(wire["outcome"]["complete"], true);
        assert_eq!(errors.len(), 1);
        let error = errors[0];
        assert_eq!(error["severity"], "error");
        assert_eq!(
            error["basis"],
            json!({
                "kind": "registration",
                "citation": "OMNA SDF Quality Names registry",
                "id": "oma:id",
            })
        );
        assert_eq!(error["assessment_unit"], "full.critical-extensions/1");
        assert_eq!(error["coverage_owner"], "full.critical-extensions/1");
        assert_eq!(
            (&error["primary"]["kind"], &error["primary"]["pointer"]),
            (&json!("effective"), &json!("/sdfObject/Temperature/oma:id"))
        );
        assert_eq!(
            (
                &error["related"][0]["kind"],
                &error["related"][0]["pointer"]
            ),
            (&json!("source"), &json!("/sdfObject/Temperature/oma:id"))
        );
        assert_eq!(
            (
                &record["state"],
                &record["snapshot"],
                &record["entries_count"]
            ),
            (
                &json!("available"),
                &json!("omna.sdf-quality-names/2026-10-16/58b2e81f1a27e35f"),
                &json!(1)
            )
        );
        assert!(human.contains("Errors: 1;"), "{human}");
        assert!(
            human.contains("\nextension.recorded_value_excluded: "),
            "{human}"
        );
        assert!(
            human.contains("effective 1 \"/sdfObject/Temperature/oma:id\""),
            "{human}"
        );
        assert!(
            human.contains("Registry omna.sdf-quality-names: available"),
            "{human}"
        );
    }
}
