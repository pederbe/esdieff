//! Pattern outcomes use the shared sample report in both CLI presentations.
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn matching_mismatch_malformed_and_unsupported_have_matching_human_json_outcomes() {
    for (pattern, admission, complete, code, exit) in [
        ("é", "admitted", true, None, 0),
        ("^é$", "rejected", true, Some("sample.pattern"), 1),
        ("[", "undetermined", false, Some("pattern.syntax"), 1),
        (
            r"(?<x>a)|(?<x>b)",
            "undetermined",
            false,
            Some("pattern.unsupported"),
            3,
        ),
        (
            "a{1000000}",
            "undetermined",
            false,
            Some("operation.limit"),
            3,
        ),
    ] {
        let model =
            serde_json::json!({"info":{},"sdfProperty":{"p":{"type":"string","pattern":pattern}}})
                .to_string();
        let path =
            std::env::temp_dir().join(format!("esdieff-pattern-cli-{}.json", std::process::id()));
        std::fs::write(&path, model).unwrap();
        let run = |format| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"))
                .args([
                    "sample",
                    "--document",
                    path.to_str().unwrap(),
                    "--kind",
                    "property",
                    "--pointer",
                    "/sdfProperty/p",
                    "--sample",
                    "-",
                    "--format",
                    format,
                    path.to_str().unwrap(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            command
                .stdin
                .take()
                .unwrap()
                .write_all("\"café\"".as_bytes())
                .unwrap();
            command.wait_with_output().unwrap()
        };
        let json = run("json");
        let human = run("human");
        std::fs::remove_file(&path).unwrap();
        assert!(json.stderr.is_empty());
        assert!(human.stderr.is_empty());
        assert_eq!(json.status.code(), Some(exit), "{pattern}");
        assert_eq!(human.status.code(), Some(exit), "{pattern}");
        let wire: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(wire["result"]["admission"], admission);
        assert_eq!(wire["outcome"]["complete"], complete);
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        let human = String::from_utf8(human.stdout).unwrap();
        assert!(human.contains(admission));
        if let Some(code) = code {
            assert!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["code"] == code)
            );
            assert!(human.contains(code));
        }
    }
}
