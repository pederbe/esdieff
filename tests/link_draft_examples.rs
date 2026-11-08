//! The Link sdfType draft's own examples remain unchanged and usable as offline fixtures.
use serde_json::Value;
use std::{path::Path, process::Command};

const DIR: &str = "examples/link-draft-02";
const OBJECT_LINK: &str = "/sdfObject/LwM2M_v1.1_Test_Object/sdfProperty/ObjLink_Value";
const CORE_LINK: &str = "/sdfObject/LwM2M_v1.1_Test_Object/sdfProperty/CoreLnk_Value";

fn esdieff(args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

fn path(name: &str) -> String {
    format!("{DIR}/{name}")
}

#[test]
fn saved_files_match_their_recorded_hashes() {
    use sha2::{Digest, Sha256};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR);
    let sources = std::fs::read_to_string(root.join("SOURCES.md")).unwrap();
    for name in [
        "section-2-link.sdf.json",
        "my-object.sdf.json",
        "org-types.sdf.json",
        "test-object.sdf.json",
        "oma-types.sdf.json",
    ] {
        let bytes = std::fs::read(root.join(name)).unwrap();
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert!(
            sources.contains(&format!("| `{name}` |")) && sources.contains(&digest),
            "{name}: {digest}"
        );
        serde_json::from_slice::<Value>(&bytes).unwrap();
    }
}

#[test]
fn saved_plan_checks_every_example_and_two_composed_values() {
    let (code, stdout) = esdieff(&[
        "check",
        "--plan",
        &path("check-plan.json"),
        "--format",
        "json",
    ]);
    assert_eq!(code, Some(0), "{stdout}");
    let wire: Value = serde_json::from_str(&stdout).unwrap();
    let cases = wire["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for (case, warnings) in cases.iter().zip([1, 3, 0, 0, 0, 0]) {
        let report = &case["report"];
        assert_eq!(report["outcome"]["complete"], true, "{case}");
        assert_eq!(report["outcome"]["disposition"], "passed", "{case}");
        assert_eq!(report["outcome"]["warnings"], warnings, "{case}");
        assert!(
            report["baseline"]["extensions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == "draft-ietf-asdf-sdftype-link-02"),
            "{case}"
        );
    }
    for case in &cases[4..] {
        assert_eq!(case["report"]["result"]["admission"], "admitted", "{case}");
    }
}

#[test]
fn published_draft_errors_keep_their_own_outcomes() {
    // Appendix A.1 spells `namespaces`, so its prefixes are unmapped.
    let (code, stdout) = esdieff(&["check", "--format", "json", &path("my-object.sdf.json")]);
    assert_eq!(code, Some(1), "{stdout}");
    let wire: Value = serde_json::from_str(&stdout).unwrap();
    let codes: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["severity"] == "error")
        .map(|item| item["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["namespace.default", "reference.prefix_unmapped"]);
    // `minvalue` and `maxvalue` are unfamiliar qualities under ADR 0037.
    let (code, stdout) = esdieff(&[
        "check",
        "--assessment",
        "full-baseline",
        "--format",
        "json",
        &path("org-types.sdf.json"),
    ]);
    assert_eq!(code, Some(3), "{stdout}");
    let wire: Value = serde_json::from_str(&stdout).unwrap();
    let unfamiliar: Vec<_> = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["code"] == "quality.unfamiliar")
        .map(|item| item["primary"]["pointer"].as_str().unwrap())
        .collect();
    assert_eq!(
        unfamiliar,
        [
            "/sdfData/idlink/properties/id/minvalue",
            "/sdfData/idlink/properties/id/maxvalue"
        ]
    );
    assert!(
        wire["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["code"] != "extension.optional_unsupported"),
        "{wire}"
    );
}

fn sample(pointer: &str, value: &str) -> (Option<i32>, Value) {
    let (code, stdout) = esdieff(&[
        "sample",
        "--document",
        &path("test-object.sdf.json"),
        "--kind",
        "property",
        "--pointer",
        pointer,
        "--sample",
        &path(value),
        "--format",
        "json",
        &path("test-object.sdf.json"),
        &path("oma-types.sdf.json"),
    ]);
    (code, serde_json::from_str(&stdout).unwrap())
}

#[test]
fn rejected_values_name_the_refined_bound_and_the_lowercase_rule() {
    for (pointer, value, expected) in [
        (OBJECT_LINK, "object-link-out-of-range.json", "sample.bound"),
        (
            CORE_LINK,
            "core-link-uppercase.json",
            "sample.link_member_name",
        ),
    ] {
        let (code, wire) = sample(pointer, value);
        assert_eq!(code, Some(1), "{wire}");
        assert_eq!(wire["result"]["admission"], "rejected", "{wire}");
        assert!(
            wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == expected),
            "{wire}"
        );
    }
}

#[test]
fn generated_link_vectors_replay_against_the_same_model() {
    let (code, vectors) = esdieff(&[
        "generate",
        "--document",
        &path("test-object.sdf.json"),
        "--kind",
        "property",
        "--pointer",
        OBJECT_LINK,
        "--format",
        "jsonl",
        &path("test-object.sdf.json"),
        &path("oma-types.sdf.json"),
    ]);
    assert_eq!(code, Some(0), "{vectors}");
    let lines: Vec<Value> = vectors
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 25);
    let expected = |value: &str| {
        let value: Value = serde_json::from_str(value).unwrap();
        lines
            .iter()
            .find(|line| line["value"] == value)
            .map(|line| line["expected"].as_str().unwrap().to_owned())
    };
    // The refined maximum is 42, and the exported minimum is 0.
    assert_eq!(
        expected(r#"{"object-instance-id":42}"#).as_deref(),
        Some("admitted")
    );
    assert_eq!(
        expected(r#"{"object-instance-id":43}"#).as_deref(),
        Some("rejected")
    );
    assert_eq!(expected(r#"{"object-id":-1}"#).as_deref(), Some("rejected"));
    assert_eq!(expected(r#"{"object-id":""}"#).as_deref(), Some("rejected"));
    let directory = tempfile_dir();
    let trace = directory.join("link-vectors.jsonl");
    std::fs::write(&trace, &vectors).unwrap();
    let (code, stdout) = esdieff(&[
        "trace",
        "--document",
        &path("test-object.sdf.json"),
        "--trace",
        trace.to_str().unwrap(),
        "--format",
        "json",
        &path("test-object.sdf.json"),
        &path("oma-types.sdf.json"),
    ]);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(code, Some(0), "{stdout}");
    let wire: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(wire["outcome"]["complete"], true, "{wire}");
}

fn tempfile_dir() -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("esdieff-link-vectors-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}
