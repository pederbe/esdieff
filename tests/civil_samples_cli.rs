//! Selected civil samples follow ADR 0018: Civil date and time sample conventions.
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Output, Stdio};

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-civil-{}-{}.json",
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
impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn run(model: &InputFile, sample: &str, format: &str, stdin: Option<&[u8]>) -> Output {
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
            sample,
            "--format",
            format,
            model.path(),
        ])
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

#[test]
fn formatted_null_cli_reports_complete_and_unfinished_const_comparisons() {
    for (meaning, sample, admission, complete, exit) in [
        ("date", "null", "admitted", true, 0),
        (
            "uuid",
            r#""00000000-0000-0000-0000-000000000000""#,
            "rejected",
            true,
            1,
        ),
        ("uri", r#""custom:device""#, "undetermined", false, 3),
    ] {
        let model = InputFile::new(format!(r#"{{"info":{{}},"sdfProperty":{{"p":{{"type":"string","format":"{meaning}","const":null}}}}}}"#).as_bytes());
        for format in ["json", "human"] {
            let output = run(&model, "-", format, Some(sample.as_bytes()));
            assert_eq!(output.status.code(), Some(exit));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                consume(&wire);
                assert_eq!(wire["result"]["admission"], admission);
                assert_eq!(wire["outcome"]["complete"], complete);
                if sample == "null" {
                    assert!(
                        wire["baseline"]["interpretations"]
                            .as_array()
                            .unwrap()
                            .is_empty()
                    );
                    assert!(wire["baseline"].get("temporal_knowledge").is_none());
                }
            } else {
                let text = String::from_utf8(output.stdout).unwrap();
                assert!(text.contains(admission));
                assert!(text.contains(&format!("Complete: {complete}")));
            }
        }
    }
}

#[test]
fn gregorian_date_constants_keep_human_json_and_exit_results_in_agreement() {
    let model = InputFile::new(
        br#"{"info":{},"sdfProperty":{"p":{"type":"string","format":"date","const":"2000-02-29"}}}"#
    );
    for (sample, admission, complete, exit) in [
        (r#""\u0032000-02-29""#, "admitted", true, 0),
        (r#""2000-03-01""#, "rejected", true, 1),
        (r#""1900-02-29""#, "rejected", false, 1),
    ] {
        for format in ["json", "human"] {
            let output = run(&model, "-", format, Some(sample.as_bytes()));
            assert_eq!(output.status.code(), Some(exit));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let value: Value = serde_json::from_slice(&output.stdout).unwrap();
                consume(&value);
                assert_eq!(value["result"]["admission"], admission);
                assert_eq!(value["outcome"]["complete"], complete);
                assert!(value["baseline"].get("temporal_knowledge").is_none());
            } else {
                let text = String::from_utf8(output.stdout).unwrap();
                assert!(text.contains(admission));
                assert!(text.contains(&format!("Complete: {complete}")));
            }
        }
    }
}
#[test]
fn dated_instant_constants_keep_human_json_and_exit_results_in_agreement() {
    let model = InputFile::new(br#"{"info":{},"sdfProperty":{"p":{"type":"string","format":"date-time","const":"1996-12-19T16:39:57-08:00"}}}"#);
    for (sample, admission, complete, exit) in [
        (r#""1996-12-20\u005400:39:57\u005a""#, "admitted", true, 0),
        (r#""1996-12-20T00:39:58Z""#, "rejected", true, 1),
        (r#""2030-06-30T23:59:59Z""#, "undetermined", false, 3),
        (r#""bad""#, "rejected", false, 1),
    ] {
        for format in ["json", "human"] {
            let output = run(&model, "-", format, Some(sample.as_bytes()));
            assert_eq!(output.status.code(), Some(exit));
            assert!(output.stderr.is_empty());
            if format == "json" {
                let value: Value = serde_json::from_slice(&output.stdout).unwrap();
                consume(&value);
                assert_eq!(value["result"]["admission"], admission);
                assert_eq!(value["outcome"]["complete"], complete);
            } else {
                let text = String::from_utf8(output.stdout).unwrap();
                assert!(text.contains(admission));
                assert!(text.contains(&format!("Complete: {complete}")));
            }
        }
    }
}

fn consume(wire: &Value) {
    assert_eq!(wire["schema"], "esdieff.report");
    assert_eq!(wire["version"], "1.1");
    assert_eq!(wire["operation"], "sample");
    assert_eq!(wire["request"]["selector"], wire["result"]["selector"]);
    assert_eq!(wire["request"]["sample"], wire["result"]["sample"]);
    assert_eq!(
        wire["coverage"]["requested"]["inventory"],
        "esdieff.sample/1"
    );
    assert_eq!(wire["coverage"]["requested"]["total"], 9);
    assert_eq!(wire["coverage"]["requested"]["count_unit"], "sample_area");
    assert_eq!(
        ["assessed", "not_applicable", "unassessed", "undetermined"]
            .into_iter()
            .map(|key| wire["coverage"]["requested"][key].as_u64().unwrap())
            .sum::<u64>(),
        9
    );
}
fn actual_temporal_use(wire: &Value, knowledge: bool) {
    assert!(
        wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "ADR0018-selected-representation")
    );
    if knowledge {
        assert_eq!(
            wire["baseline"]["temporal_knowledge"],
            serde_json::json!({
                "id": "iers-bulletin-c72/2026-07-06", "from": "1972-01-01", "through": "2026-12-31",
                "transition_sha256": "6cb6f5d4b819f2e568e25db4b0b26d89dedf031fdffb18bc94d40f4e94e268d7",
                "bulletin_sha256": "310e172eadacacca3adf92cb9bd646fcc0d6320e35e996a106d2644ede3b52f0"
            })
        );
    } else {
        assert!(wire["baseline"].get("temporal_knowledge").is_none());
    }
}
fn space_representation_basis_and_locations(wire: &Value, model: &str, token: &[u8]) {
    let items = wire["diagnostics"]["items"].as_array().unwrap();
    let matching = items
        .iter()
        .filter(|item| item["code"] == "sample.temporal_representation")
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1);
    let item = matching[0];
    assert_eq!(item["domain"], "sample");
    assert_eq!(item["severity"], "error");
    assert_eq!(item["basis"]["kind"], "interpretation");
    assert_eq!(item["basis"]["id"], "ADR0018-selected-representation");
    assert_eq!(
        item["basis"]["citation"],
        "RFC 9880 Appendix C.2; RFC 3339 Section 5.6"
    );
    assert_eq!(item["primary"], wire["result"]["sample_location"]);
    assert_eq!(item["primary"]["kind"], "sample");
    assert_eq!(item["primary"]["source_id"], wire["request"]["sample"]);
    assert_eq!(item["primary"]["pointer"], "");
    assert_eq!(item["primary"]["span"], serde_json::json!([0, token.len()]));
    let related = item["related"].as_array().unwrap();
    let effective = related
        .iter()
        .find(|row| row["kind"] == "effective")
        .unwrap();
    assert_eq!(effective["root"], wire["request"]["selector"]["document"]);
    assert_eq!(effective["pointer"], "/sdfProperty/p/format");
    assert!(effective.get("span").is_none());
    let source = related.iter().find(|row| row["kind"] == "source").unwrap();
    let literal = "\"date-time\"";
    let start = model.find(literal).unwrap();
    assert_eq!(source["source_id"], wire["request"]["selector"]["document"]);
    assert_eq!(source["pointer"], "/sdfProperty/p/format");
    assert_eq!(
        source["span"],
        serde_json::json!([start, start + literal.len()])
    );
}

#[test]
fn representative_original_sample_cli_matches_json_and_human() {
    for (format, text, admission, exit, knowledge) in [
        ("date", "0000-02-29", "admitted", 0, false),
        ("date", "1900-02-29", "rejected", 1, false),
        (
            "date-time",
            "2017-01-01T05:44:60+05:45",
            "admitted",
            0,
            true,
        ),
        ("date-time", "2024-01-01 12:00:00Z", "rejected", 1, false),
        (
            "date-time",
            "2030-06-30T23:59:59.9Z",
            "undetermined",
            3,
            true,
        ),
        ("date-time", "2024-06-30T23:59:59Z", "admitted", 0, true),
        ("date-time", "2030-06-30T12:34:60Z", "rejected", 1, false),
        (
            "time",
            "12:34:56.000000000000000000001-00:00",
            "admitted",
            0,
            false,
        ),
        ("time", "12:34:60Z", "rejected", 1, false),
        ("time", "05:44:60+05:45", "undetermined", 3, false),
    ] {
        let model_text = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":false,"format":"{format}"}}}}}}"#
        );
        let model = InputFile::new(model_text.as_bytes());
        let token = serde_json::to_string(text).unwrap();
        let sample = InputFile::new(token.as_bytes());
        for stdin in [false, true] {
            let sample_path = if stdin { "-" } else { sample.path() };
            let json = run(
                &model,
                sample_path,
                "json",
                stdin.then_some(token.as_bytes()),
            );
            let human = run(
                &model,
                sample_path,
                "human",
                stdin.then_some(token.as_bytes()),
            );
            assert_eq!(json.status.code(), Some(exit));
            assert_eq!(human.status.code(), Some(exit));
            assert!(json.stderr.is_empty());
            assert!(human.stderr.is_empty());
            assert_eq!(json.stdout.last(), Some(&b'\n'));
            let wire: Value = serde_json::from_slice(&json.stdout).unwrap();
            consume(&wire);
            assert_eq!(wire["result"]["admission"], admission);
            actual_temporal_use(&wire, knowledge);
            if text == "2024-01-01 12:00:00Z" {
                space_representation_basis_and_locations(&wire, &model_text, token.as_bytes());
            }
            let human = String::from_utf8(human.stdout).unwrap();
            assert!(human.contains(admission));
            assert!(human.contains(wire["outcome"]["disposition"].as_str().unwrap()));
            assert!(human.contains("Sample area sample-abstract-values/1"));
            assert_eq!(human.contains("iers-bulletin-c72/2026-07-06"), knowledge);
        }
    }
}

#[test]
fn date_unavailable_acquisition_is_unknown_and_duplicate_stdin_is_invocation_error() {
    // Date has no intrinsic calendar unknown. This is a prerequisite control, not Q06 temporal work.
    let model = InputFile::new(br#"{"sdfProperty":{"p":{"type":"string","format":"date"}}}"#);
    let missing = InputFile::new(b"{}");
    std::fs::remove_file(&missing.0).unwrap();
    let output = run(&model, missing.path(), "json", None);
    assert_eq!(output.status.code(), Some(3));
    let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
    consume(&wire);
    assert_eq!(wire["result"]["admission"], "undetermined");
    assert!(
        !wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "ADR0018-selected-representation")
    );
    assert!(wire["baseline"].get("temporal_knowledge").is_none());
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
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
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

// Branch witnesses, original spans and typed samples are exercised by the public caller.
