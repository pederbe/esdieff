//! `check --format sarif` and `diff --format sarif` project their reports as SARIF 2.1.0
//! logs for code-scanning annotations. Follows ADR 0043: SARIF and Markdown report
//! projections.
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

/// One finding of each severity: `"low"` on line 4 follows multibyte text, so its columns
/// count Unicode code points, and `colour` is an unfamiliar quality on line 5.
const THERMO: &str = r#"{
  "info": {"title": "Thermo", "version": "1", "copyright": "x", "license": "x"},
  "sdfObject": {"Thermo": {"sdfProperty": {"level": {
    "description": "Grüße 🌡", "minimum": "low",
    "colour": "red"
  }}}}
}
"#;

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-sarif-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        // The invocation directory a child process reports has its symbolic links resolved.
        Self(fs::canonicalize(path).unwrap())
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs esdieff in `directory`, supplying `stdin` when given.
fn run_in(directory: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(stdin.unwrap_or_default().as_bytes())
        .unwrap();
    drop(input);
    child.wait_with_output().unwrap()
}

fn log(output: &Output, exit: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let log: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(log["version"], "2.1.0");
    assert_eq!(log["runs"].as_array().unwrap().len(), 1);
    log
}

fn run_of(log: &Value) -> &Value {
    &log["runs"][0]
}

fn results(log: &Value) -> &[Value] {
    run_of(log)["results"].as_array().unwrap()
}

/// The result with this rule id.
fn result<'a>(log: &'a Value, rule: &str) -> &'a Value {
    results(log)
        .iter()
        .find(|result| result["ruleId"] == rule)
        .unwrap_or_else(|| panic!("no {rule} result in {log:#}"))
}

#[test]
fn check_findings_become_results_with_code_point_regions() {
    let tree = Tree::new();
    tree.write("models/thermo.sdf.json", THERMO);
    let log = log(
        &run_in(
            tree.path(),
            &["check", "--format", "sarif", "models/thermo.sdf.json"],
            None,
        ),
        1,
    );
    let run = run_of(&log);
    assert_eq!(run["tool"]["driver"]["name"], "Esdieff");
    assert_eq!(run["tool"]["driver"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(run["columnKind"], "unicodeCodePoints");
    assert_eq!(results(&log).len(), 2);
    let rules: Vec<_> = run["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| rule["id"].as_str().unwrap())
        .collect();
    assert_eq!(rules, ["data.number", "quality.unfamiliar"]);

    let error = result(&log, "data.number");
    assert_eq!(error["level"], "error");
    assert_eq!(error["ruleIndex"], 0);
    let location = &error["locations"][0]["physicalLocation"];
    assert_eq!(
        location["artifactLocation"]["uri"],
        "models/thermo.sdf.json"
    );
    // `"low"` is bytes 183 to 188 and code points 42 to 46 of line 4.
    assert_eq!(
        location["region"],
        serde_json::json!({"startLine":4,"startColumn":42,"endLine":4,"endColumn":47,
            "byteOffset":183,"byteLength":5})
    );

    let warning = result(&log, "quality.unfamiliar");
    assert_eq!(warning["level"], "warning");
    assert_eq!(
        warning["locations"][0]["physicalLocation"]["region"],
        serde_json::json!({"startLine":5,"startColumn":15,"endLine":5,"endColumn":20,
            "byteOffset":204,"byteLength":5})
    );
}

#[test]
fn effective_findings_use_their_first_related_source_span() {
    let tree = Tree::new();
    tree.write(
        "level.sdf.json",
        r#"{
  "info": {"title": "Level", "version": "1", "copyright": "x", "license": "x"},
  "sdfProperty": {"level": {"type": "number", "nullable": false, "minimum": 2, "maximum": 1}}
}
"#,
    );
    let log = log(
        &run_in(
            tree.path(),
            &["check", "--format", "sarif", "level.sdf.json"],
            None,
        ),
        0,
    );
    let result = result(&log, "proof.empty_numeric_interval");
    assert_eq!(result["level"], "warning");
    let location = &result["locations"][0];
    // The primary location is effective, so the minimum's source span locates the result.
    assert_eq!(
        location["physicalLocation"]["region"],
        serde_json::json!({"startLine":3,"startColumn":77,"endLine":3,"endColumn":78,
            "byteOffset":158,"byteLength":1})
    );
    assert_eq!(
        location["logicalLocations"],
        serde_json::json!([{"fullyQualifiedName":"/sdfProperty/level",
            "properties":{"view":"effective","document":"level.sdf.json"}}])
    );
    // The other related locations follow, without the promoted one.
    let related = result["relatedLocations"].as_array().unwrap();
    assert_eq!(related.len(), 5);
    let spans: Vec<_> = related
        .iter()
        .filter_map(|location| location["physicalLocation"]["region"]["byteOffset"].as_u64())
        .collect();
    assert_eq!(spans, [172, 140]);
}

#[test]
fn incomplete_work_becomes_notifications_with_the_json_exit_code() {
    let tree = Tree::new();
    tree.write("thermo.sdf.json", THERMO);
    tree.write(
        "missing.sdf.json",
        r##"{"info": {"title": "Ref", "version": "1", "copyright": "x", "license": "x"},
 "sdfProperty": {"p": {"sdfRef": "#/sdfData/Missing"}}}"##,
    );
    for (name, exit) in [("thermo.sdf.json", 1), ("missing.sdf.json", 3)] {
        let json = run_in(tree.path(), &["check", "--format", "json", name], None);
        assert_eq!(json.status.code(), Some(exit));
        let sarif = run_in(tree.path(), &["check", "--format", "sarif", name], None);
        assert_eq!(sarif.status.code(), json.status.code(), "{name}");
    }
    let log = log(
        &run_in(
            tree.path(),
            &["check", "--format", "sarif", "missing.sdf.json"],
            None,
        ),
        3,
    );
    let run = run_of(&log);
    assert!(results(&log).is_empty());
    assert_eq!(run["properties"]["disposition"], "undetermined");
    assert_eq!(run["properties"]["omittedResults"], 0);
    let invocation = &run["invocations"][0];
    assert_eq!(invocation["executionSuccessful"], true);
    let notifications = invocation["toolExecutionNotifications"].as_array().unwrap();
    let codes: Vec<_> = notifications
        .iter()
        .map(|notification| notification["descriptor"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(codes[0], "reference.unavailable");
    assert!(codes[1..].iter().all(|code| code.starts_with("coverage.")));
    // The unavailable reference is located at the sdfRef that names it.
    let location = &notifications[0]["locations"][0]["physicalLocation"];
    assert_eq!(location["artifactLocation"]["uri"], "missing.sdf.json");
    assert_eq!(location["region"]["startLine"], 2);
    // Coverage notifications have no location.
    assert!(notifications[1].get("locations").is_none());
    let descriptors: Vec<_> = run["tool"]["driver"]["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|descriptor| descriptor["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        descriptors,
        [
            "reference.unavailable",
            "coverage.unavailable",
            "coverage.advisory_unavailable"
        ]
    );
}

#[test]
fn stdin_sources_have_logical_locations_only() {
    let tree = Tree::new();
    let log = log(
        &run_in(
            tree.path(),
            &["check", "--format", "sarif", "-"],
            Some(THERMO),
        ),
        1,
    );
    let location = &result(&log, "data.number")["locations"][0];
    assert!(location.get("physicalLocation").is_none());
    assert_eq!(
        location["logicalLocations"],
        serde_json::json!([{"fullyQualifiedName":"/sdfObject/Thermo/sdfProperty/level/minimum",
            "properties":{"view":"source","document":"-"}}])
    );
}

#[test]
fn artifact_uris_are_relative_to_the_invocation_directory() {
    let tree = Tree::new();
    let absolute = tree.write("models/my lock #1.sdf.json", THERMO);
    tree.write("outside.sdf.json", THERMO);
    #[cfg(unix)]
    let models = tree.path().join("models");
    let uri = |directory: &Path, path: &str| {
        let log = log(
            &run_in(directory, &["check", "--format", "sarif", path], None),
            1,
        );
        result(&log, "data.number")["locations"][0]["physicalLocation"]["artifactLocation"].clone()
    };
    let relative =
        serde_json::json!({"uri":"models/my%20lock%20%231.sdf.json","uriBaseId":"%SRCROOT%"});
    assert_eq!(uri(tree.path(), "models/my lock #1.sdf.json"), relative);
    // Dot segments inside the invocation directory resolve.
    assert_eq!(
        uri(tree.path(), "./models/../models/my lock #1.sdf.json"),
        relative
    );
    assert_eq!(uri(tree.path(), absolute.to_str().unwrap()), relative);
    // A path that leaves the invocation directory is an absolute file URI without a base.
    #[cfg(unix)]
    assert_eq!(
        uri(&models, "../outside.sdf.json"),
        serde_json::json!({"uri":format!("file://{}/outside.sdf.json", tree.path().display())})
    );
}

#[test]
fn logs_name_the_invocation_directory_and_describe_every_rule() {
    let tree = Tree::new();
    tree.write("thermo.sdf.json", THERMO);
    let log = log(
        &run_in(
            tree.path(),
            &["check", "--format", "sarif", "thermo.sdf.json"],
            None,
        ),
        1,
    );
    let run = run_of(&log);
    #[cfg(unix)]
    assert_eq!(
        run["originalUriBaseIds"],
        serde_json::json!({"%SRCROOT%":{"uri":format!("file://{}/", tree.path().display())}})
    );
    assert_eq!(
        run["tool"]["driver"]["rules"][0],
        serde_json::json!({"id":"data.number","shortDescription":{"text":"data.number"},
            "fullDescription":{"text":"Esdieff error condition data.number, with basis kind rfc."},
            "help":{"text":"The Esdieff report contract describes condition codes, severities and bases under Outcomes, conditions and truncation. Each message explains its finding."}})
    );
}

#[test]
fn locations_without_a_span_name_the_whole_file() {
    let tree = Tree::new();
    let log = log(
        &run_in(
            tree.path(),
            &["check", "--format", "sarif", "absent.sdf.json"],
            None,
        ),
        3,
    );
    let notification = &run_of(&log)["invocations"][0]["toolExecutionNotifications"][0];
    assert_eq!(notification["descriptor"]["id"], "input.acquisition");
    assert_eq!(
        notification["locations"][0]["physicalLocation"],
        serde_json::json!({"artifactLocation":{"uri":"absent.sdf.json","uriBaseId":"%SRCROOT%"},
            "region":{"startLine":1}})
    );
}

#[test]
fn sarif_applies_only_to_direct_check_and_diff() {
    let tree = Tree::new();
    tree.write("thermo.sdf.json", THERMO);
    tree.write("sample.json", "5");
    tree.write("plan.json", "{}");
    for args in [
        &[
            "inspect",
            "--format",
            "sarif",
            "--document",
            "thermo.sdf.json",
            "--pointer",
            "",
            "thermo.sdf.json",
        ][..],
        &[
            "sample",
            "--format",
            "sarif",
            "--document",
            "thermo.sdf.json",
            "--kind",
            "property",
            "--pointer",
            "/sdfObject/Thermo/sdfProperty/level",
            "--sample",
            "sample.json",
            "thermo.sdf.json",
        ],
        &["check", "--plan", "plan.json", "--format", "sarif"],
        &["check", "--format", "markdown", "thermo.sdf.json"],
    ] {
        let output = run_in(tree.path(), args, None);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}

const LOCK: &str = include_str!("../examples/door-lock-check/rich.sdf.json");

/// A later lock revision with a breaking narrower credential, an undetermined added unit, a
/// compatible description and a renamed event, which removes one event and adds another.
/// Every change keeps the lines of the earlier revision.
fn revised_lock() -> String {
    LOCK.replacen("\"maxLength\": 8", "\"maxLength\": 6", 1)
        .replace(
            "An authored demonstration token with Unicode scalar length constraints",
            "A demonstration token",
        )
        .replacen(
            "\"type\": \"string\",",
            "\"type\": \"string\", \"unit\": \"Cel\",",
            1,
        )
        .replace("\"stateChanged\": {", "\"changed\": {")
}

/// Acknowledges the narrower credential.
const ACKNOWLEDGED: &str = r#"{"schema":"esdieff.acknowledged-differences","version":"1","differences":[
    {"kind":"changed","subject":"quality","quality":"maxLength","classification":"breaking",
     "earlier":{"document":"lock.sdf.json","pointer":"/sdfObject/Lock/sdfProperty/credential/maxLength"},
     "later":{"document":"lock.sdf.json","pointer":"/sdfObject/Lock/sdfProperty/credential/maxLength"}}]}"#;

fn lock_revisions() -> Tree {
    let tree = Tree::new();
    tree.write("old/lock.sdf.json", LOCK);
    tree.write("new/lock.sdf.json", &revised_lock());
    tree.write("acknowledged.json", ACKNOWLEDGED);
    tree
}

fn uri_and_line(location: &Value) -> (&str, u64) {
    let physical = &location["physicalLocation"];
    (
        physical["artifactLocation"]["uri"].as_str().unwrap(),
        physical["region"]["startLine"].as_u64().unwrap(),
    )
}

#[test]
fn diff_differences_become_results_by_classification() {
    let tree = lock_revisions();
    let diff = |format| {
        run_in(
            tree.path(),
            &[
                "diff",
                "--old",
                "old",
                "--new",
                "new",
                "--acknowledged",
                "acknowledged.json",
                "--format",
                format,
            ],
            None,
        )
    };
    assert_eq!(diff("json").status.code(), Some(1));
    let log = log(&diff("sarif"), 1);
    let run = run_of(&log);
    assert_eq!(run["properties"]["disposition"], "failed");
    assert_eq!(run["properties"]["omittedResults"], 0);
    let levels: Vec<_> = results(&log)
        .iter()
        .map(|result| {
            (
                result["ruleId"].as_str().unwrap(),
                result["level"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        levels,
        [
            ("comparison.breaking", "error"),
            ("comparison.unit_added", "warning"),
            ("comparison.breaking", "error"),
            ("comparison.compatible", "note"),
            ("comparison.compatible", "note"),
        ]
    );

    // Only the acknowledged narrower credential is suppressed, and every result says so.
    let suppressions: Vec<_> = results(&log)
        .iter()
        .map(|result| result["suppressions"].as_array().unwrap().len())
        .collect();
    assert_eq!(suppressions, [0, 0, 1, 0, 0]);
    let narrower = &results(&log)[2];
    assert_eq!(
        narrower["suppressions"],
        serde_json::json!([{"kind":"external","status":"accepted"}])
    );
    // It is located at the later revision's origin, with the earlier one related.
    let location = &narrower["locations"][0];
    assert_eq!(uri_and_line(location), ("new/lock.sdf.json", 24));
    assert_eq!(location["physicalLocation"]["region"]["startColumn"], 24);
    assert_eq!(
        location["logicalLocations"][0],
        serde_json::json!({"fullyQualifiedName":"/sdfObject/Lock/sdfProperty/credential/maxLength",
            "properties":{"view":"effective","document":Path::new("new").join("lock.sdf.json"),"revision":"later"}})
    );
    let earlier = &narrower["relatedLocations"][0];
    assert_eq!(uri_and_line(earlier), ("old/lock.sdf.json", 24));
    assert_eq!(earlier["message"]["text"], "earlier revision");
    assert_eq!(
        narrower["message"]["text"],
        "Acknowledged breaking difference: changed quality maxLength in lock.sdf.json at /sdfObject/Lock/sdfProperty/credential/maxLength (witness \"aaaaaaa\" for client-supplied data, earlier admitted, later rejected)."
    );

    // The removed event exists only in the earlier revision, so its origin there locates it.
    let removed = &results(&log)[0];
    assert_eq!(
        uri_and_line(&removed["locations"][0]),
        ("old/lock.sdf.json", 82)
    );
    assert!(removed.get("relatedLocations").is_none());
    // The added unit has no earlier origin, so its earlier side is a logical location only.
    let unit = &results(&log)[1];
    assert_eq!(
        uri_and_line(&unit["locations"][0]),
        ("new/lock.sdf.json", 9)
    );
    let related = &unit["relatedLocations"][0];
    assert!(related.get("physicalLocation").is_none());
    assert_eq!(
        related["logicalLocations"][0]["properties"]["revision"],
        "earlier"
    );
    let message = unit["message"]["text"].as_str().unwrap();
    assert!(message.contains("unit is added"), "{message}");
}
