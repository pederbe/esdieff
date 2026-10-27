//! Command admission shares ADR0016: Selected samples with explicit admission and coverage,
//! including forward-compatible reports under ADR0011: Machine-readable report contracts.
use serde_json::Value;
use std::process::Command;

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(bytes: &[u8]) -> Self {
        use std::io::Write;
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-text-boolean-{}-{}.json",
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

fn consume_sample(wire: &Value) -> (&str, &str, u64) {
    assert_eq!(wire["schema"], "esdieff.report");
    assert_eq!(wire["version"], "1.1");
    assert_eq!(wire["operation"], "sample");
    assert_eq!(wire["request"]["assessment"], "esdieff.sample/1");
    assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
    assert_eq!(wire["request"]["sample"], wire["result"]["sample"]);
    let counts = &wire["coverage"]["requested"];
    assert_eq!(counts["total"], 9);
    assert_eq!(counts["count_unit"], "sample_area");
    assert_eq!(
        ["assessed", "not_applicable", "unassessed", "undetermined"]
            .into_iter()
            .map(|name| counts[name].as_u64().unwrap())
            .sum::<u64>(),
        9
    );
    (
        wire["result"]["admission"].as_str().unwrap(),
        wire["outcome"]["disposition"].as_str().unwrap(),
        wire["outcome"]["errors"].as_u64().unwrap(),
    )
}

#[test]
fn text_and_boolean_cli_reports_match_human_output_and_tolerate_additive_fields() {
    for (qualities, value, admission, exit) in [
        (
            r#""type":"string","minLength":4,"maxLength":4"#,
            r#""é𝄞e\u0301""#,
            "admitted",
            0,
        ),
        (
            r#""type":"string","maxLength":1"#,
            r#""e\u0301""#,
            "rejected",
            1,
        ),
        (r#""type":"boolean""#, "true", "admitted", 0),
        (r#""type":"boolean""#, r#""true""#, "rejected", 1),
        (
            r#""type":"string","pattern":"^on$""#,
            r#""on""#,
            "admitted",
            0,
        ),
        (r#""type":"string","nullable":false"#, "null", "rejected", 1),
    ] {
        let model =
            InputFile::new(format!(r#"{{"sdfProperty":{{"p":{{{qualities}}}}}}}"#).as_bytes());
        let sample = InputFile::new(value.as_bytes());
        let run = |format| {
            Command::new(env!("CARGO_BIN_EXE_esdieff"))
                .args([
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
                ])
                .output()
                .unwrap()
        };
        let json = run("json");
        let human = run("human");
        assert!(json.stderr.is_empty());
        assert!(human.stderr.is_empty());
        assert_eq!(
            json.status.code(),
            Some(exit),
            "{qualities}: {}",
            String::from_utf8_lossy(&json.stdout)
        );
        assert_eq!(human.status.code(), Some(exit));
        let mut wire: Value = serde_json::from_slice(&json.stdout).unwrap();
        let conclusion = consume_sample(&wire);
        assert_eq!(conclusion.0, admission);
        assert!(String::from_utf8(human.stdout).unwrap().contains(admission));
        let before = (
            conclusion.0.to_owned(),
            conclusion.1.to_owned(),
            conclusion.2,
        );
        wire["future_detail"] = serde_json::json!({"retained":true});
        wire["result"]["future_detail"] = serde_json::json!([]);
        let conclusion = consume_sample(&wire);
        assert_eq!(
            (
                conclusion.0.to_owned(),
                conclusion.1.to_owned(),
                conclusion.2
            ),
            before
        );
    }
}
