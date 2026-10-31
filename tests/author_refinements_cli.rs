#[cfg(unix)]
mod native_path_support;

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
const SOURCE: &str = include_str!("fixtures/authoring/reusable.sdf.json");
const PLAN: &str = include_str!("fixtures/authoring/refine.plan.json");
const TEMPERATURE: &str =
    "/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor/sdfProperty/temperature";
struct Fixture(PathBuf);
impl Fixture {
    fn new(plan: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-refinement-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("base.json"), SOURCE).unwrap();
        fs::write(path.join("plan.json"), plan).unwrap();
        Self(path)
    }
    fn author(&self, format: &str, document: &str, assessment: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
        command.current_dir(&self.0).args([
            "author",
            "--plan",
            "plan.json",
            "--source",
            "base",
            "base.json",
            "--document-output",
            document,
            "--check",
            assessment,
            "--format",
            format,
        ]);
        command
    }
    fn inspect(&self, pointer: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&self.0)
            .args([
                "inspect",
                "--document",
                "product.json",
                "--view",
                "effective",
                "--pointer",
                pointer,
                "--format",
                "json",
                "base.json",
                "product.json",
            ])
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_generation_and_existing_inspection_expose_exact_refinements_and_origins() {
    let fixture = Fixture::new(PLAN);
    let authored = fixture
        .author("json", "product.json", "initial-v1")
        .output()
        .unwrap();
    assert!(
        authored.status.success(),
        "{}",
        String::from_utf8_lossy(&authored.stderr)
    );
    assert_eq!(
        fs::read(fixture.0.join("base.json")).unwrap(),
        SOURCE.as_bytes()
    );
    let generated = fs::read_to_string(fixture.0.join("product.json")).unwrap();
    assert!(generated.contains("1e2"));
    let inspected = fixture.inspect(TEMPERATURE);
    assert!(inspected.status.success());
    assert!(String::from_utf8_lossy(&inspected.stdout).contains("1e2"));
    let report: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(
        report["result"]["json"]["description"],
        "Product temperature"
    );
    assert_eq!(
        report["result"]["json"]["maximum"],
        serde_json::from_str::<serde_json::Value>("1e2").unwrap()
    );
    assert_eq!(report["result"]["json"]["minimum"], -50);
    assert_eq!(report["result"]["json"]["writable"], false);
    let explanations = report["result"]["explanations"].as_array().unwrap();
    let maximum = explanations
        .iter()
        .find(|explanation| explanation["effective"]["pointer"] == format!("{TEMPERATURE}/maximum"))
        .unwrap();
    assert_eq!(maximum["kind"], "override");
    assert_eq!(maximum["sources"][0]["source_id"], 2);
    assert_eq!(maximum["sources"][1]["source_id"], 1);
    let minimum = explanations
        .iter()
        .find(|explanation| explanation["effective"]["pointer"] == format!("{TEMPERATURE}/minimum"))
        .unwrap();
    assert_eq!(minimum["kind"], "inherited");
    let object = fixture.inspect("/sdfThing/Room ~1 Monitor/sdfObject/ambient ~1 sensor");
    let object: serde_json::Value = serde_json::from_slice(&object.stdout).unwrap();
    assert!(object["result"]["json"].get("description").is_none());
    assert_eq!(
        object["result"]["json"]["sdfEvent"]["alarm"]["sdfOutputData"]["maximum"],
        150
    );
}

#[test]
fn failed_refinement_reports_preserve_human_json_outcomes_and_recipe_spans() {
    let invalid = PLAN.replace("1e2", "\"too hot\"");
    let fixture = Fixture::new(&invalid);
    let json = fixture
        .author("json", "failed-json.json", "initial-v1")
        .output()
        .unwrap();
    let human = fixture
        .author("human", "failed-human.json", "initial-v1")
        .output()
        .unwrap();
    assert_eq!(json.status.code(), Some(1));
    assert_eq!(human.status.code(), Some(1));
    assert!(!fixture.0.join("failed-json.json").exists());
    assert!(!fixture.0.join("failed-human.json").exists());
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("failed"));
    assert!(human.contains("data.number"));
    assert!(human.contains(&format!("Errors: {}", report["outcome"]["errors"])));
    assert!(human.contains("/objects/0/refinement/sdfProperty/temperature/maximum"));
    let recipe = report["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|condition| condition["related"].as_array().unwrap())
        .find(|location| {
            location["kind"] == "author_request"
                && location["pointer"] == "/objects/0/refinement/sdfProperty/temperature/maximum"
        })
        .unwrap();
    let span = recipe["span"].as_array().unwrap();
    assert_eq!(
        &invalid[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
        "\"too hot\""
    );
    assert_eq!(
        fs::read(fixture.0.join("base.json")).unwrap(),
        SOURCE.as_bytes()
    );
}

#[test]
fn formal_count_errors_and_strict_warnings_prevent_document_delivery() {
    let fixture = Fixture::new(PLAN);
    let full = fixture
        .author("json", "full.json", "full-baseline")
        .output()
        .unwrap();
    assert_eq!(full.status.code(), Some(1));
    assert!(!fixture.0.join("full.json").exists());
    let report: serde_json::Value = serde_json::from_slice(&full.stdout).unwrap();
    assert!(String::from_utf8_lossy(&full.stdout).contains("formal.uint"));
    assert_eq!(
        report["result"]["checking"]["coverage"]["requested"]["total"],
        13
    );
    assert_eq!(
        report["result"]["checking"]["coverage"]["requirements"]["total"],
        54
    );
    assert_eq!(report["coverage"]["requested"]["total"], 1);
    let strict = fixture
        .author("json", "strict.json", "initial-v1")
        .arg("--strict-warnings")
        .output()
        .unwrap();
    assert_eq!(strict.status.code(), Some(1));
    assert!(!fixture.0.join("strict.json").exists());
    let strict: serde_json::Value = serde_json::from_slice(&strict.stdout).unwrap();
    assert_eq!(strict["result"]["fidelity"]["state"], "established");
    assert_eq!(strict["result"]["generation"], "complete");
}

#[cfg(unix)]
#[test]
fn native_paths_and_distinct_aliased_source_keys_keep_explicit_selection() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    for name in [
        OsString::from("source-温度.json"),
        OsString::from_vec(b"source-\xff.json".to_vec()),
    ] {
        let fixture = Fixture::new(PLAN);
        let rejection = native_path_support::write(&fixture.0.join(&name), SOURCE.as_bytes());
        fs::remove_file(fixture.0.join("base.json")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&fixture.0)
            .args(["author", "--plan", "plan.json", "--source", "base"])
            .arg(&name)
            .args(["--document-output", "raw.json", "--format", "json"])
            .output()
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report["sources"][0]["acquisition"],
            if rejection.is_none() {
                "acquired"
            } else {
                "failed"
            },
            "filesystem rejection: {rejection:?}; report: {report}; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if rejection.is_some() {
            assert_eq!(output.status.code(), Some(3));
            assert!(!fixture.0.join("raw.json").exists());
            assert!(!fixture.0.join(name.to_string_lossy().as_ref()).exists());
            continue;
        }
        assert!(output.status.success());
        fs::hard_link(fixture.0.join(&name), fixture.0.join("alias.json")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&fixture.0)
            .args(["author", "--plan", "plan.json", "--source", "base"])
            .arg(&name)
            .args([
                "--source",
                "alias",
                "alias.json",
                "--document-output",
                "ambiguous.json",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(!fixture.0.join("ambiguous.json").exists());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["sources"].as_array().unwrap().len(), 3);
    }
}
