use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const BASE: &[u8] = include_bytes!("../examples/context-authoring/base.sdf.json");
const RECIPE: &[u8] = include_bytes!("../examples/context-authoring/recipe.json");
const EDITS: &[u8] = include_bytes!("../examples/context-authoring/edits.json");
const OUTSIDE: &[u8] = include_bytes!("../examples/context-authoring/outside-boundary.json");

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-context-authoring-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("base.json"), BASE).unwrap();
        fs::write(path.join("recipe.json"), RECIPE).unwrap();
        fs::write(path.join("edits.json"), EDITS).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn author(&self) -> Output {
        self.run(&[
            "author",
            "--source",
            "base",
            "base.json",
            "--plan",
            "recipe.json",
            "--document-output",
            "installation.json",
            "--check",
            "full-baseline",
            "--format",
            "json",
        ])
    }
    fn edit(&self, output: &str) -> Output {
        self.run(&[
            "edit",
            "--source",
            "base",
            "base.json",
            "--plan",
            "edits.json",
            "--document-output",
            output,
            "--check",
            "full-baseline",
            "--format",
            "json",
        ])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn report(output: &Output, code: i32) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_authors_and_inspects_refined_context_with_original_sources_and_draft_checking() {
    let fixture = Fixture::new();
    let output = fixture.author();
    let wire = report(&output, 0);
    assert_eq!(wire["result"]["generation"], "complete");
    assert_eq!(wire["result"]["fidelity"]["state"], "established");
    assert_eq!(
        wire["result"]["checking"]["outcome"]["disposition"],
        "passed"
    );
    assert_eq!(
        wire["baseline"]["extensions"][0]["id"],
        "draft-ietf-asdf-sdf-nonaffordance-06"
    );
    let output = fixture.run(&[
        "inspect",
        "--view",
        "effective",
        "--document",
        "installation.json",
        "--pointer",
        "/sdfThing/Installation/sdfObject/sensor",
        "--format",
        "json",
        "base.json",
        "installation.json",
    ]);
    let wire = report(&output, 0);
    let contexts = &wire["result"]["json"]["sdfContext"];
    assert_eq!(contexts["calibration"]["minimum"], -10);
    assert_eq!(contexts["calibration"]["maximum"], 2.5);
    assert!(contexts["calibration"].get("description").is_none());
    assert!(contexts.get("retired").is_none());
    assert_eq!(contexts["siteName"]["const"], "Office");
    let origin = wire["result"]["explanations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["effective"]["pointer"]
                == "/sdfThing/Installation/sdfObject/sensor/sdfContext/calibration/maximum"
        })
        .unwrap();
    assert_eq!(origin["kind"], "override");
    assert_eq!(fs::read(fixture.0.join("base.json")).unwrap(), BASE);
    assert_eq!(fs::read(fixture.0.join("recipe.json")).unwrap(), RECIPE);
    assert!(
        fs::read_to_string(fixture.0.join("installation.json"))
            .unwrap()
            .contains("2.50")
    );
}

#[test]
fn cli_edits_authored_context_and_keeps_failed_checks_separate_from_faithful_changes() {
    let fixture = Fixture::new();
    let wire = report(&fixture.edit("revised.json"), 0);
    assert_eq!(wire["result"]["fidelity"]["state"], "established");
    assert_eq!(
        wire["baseline"]["extensions"][0]["id"],
        "draft-ietf-asdf-sdf-nonaffordance-06"
    );
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.0.join("revised.json")).unwrap()).unwrap();
    let site = &document["sdfObject"]["Sensor"]["sdfContext"]["siteName"];
    assert_eq!(site["description"], "Site name chosen during installation");
    assert_eq!(site["minLength"], 2);
    assert!(site.get("label").is_none());
    assert!(site.get("const").is_none());
    assert_eq!(fs::read(fixture.0.join("base.json")).unwrap(), BASE);
    assert_eq!(fs::read(fixture.0.join("edits.json")).unwrap(), EDITS);

    let mut plan: serde_json::Value = serde_json::from_slice(EDITS).unwrap();
    plan["edits"][0]["value"] = 17.into();
    fs::write(
        fixture.0.join("edits.json"),
        serde_json::to_vec(&plan).unwrap(),
    )
    .unwrap();
    let wire = report(&fixture.edit("invalid.json"), 1);
    assert_eq!(wire["result"]["generation"], "complete");
    assert_eq!(wire["result"]["fidelity"]["state"], "established");
    assert_eq!(
        wire["result"]["checking"]["outcome"]["disposition"],
        "failed"
    );
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == "metadata.string")
        .unwrap();
    assert!(
        finding["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|location| location["kind"] == "edit_request"
                && location["pointer"] == "/edits/0/value")
    );
    assert!(!fixture.0.join("invalid.json").exists());
}

#[test]
fn cli_refuses_context_edits_inside_reference_boundaries_and_unsupported_qualities() {
    let fixture = Fixture::new();
    report(&fixture.author(), 0);
    for (source, path, owner, quality, reason) in [
        (
            "installation",
            "installation.json",
            "/sdfThing/Installation/sdfObject/sensor/sdfContext/calibration",
            "maximum",
            "sdfRef composition boundary",
        ),
        (
            "base",
            "base.json",
            "/sdfObject/Sensor/sdfContext/siteName",
            "readable",
            "unsupported editing Quality Name",
        ),
    ] {
        let plan = if source == "installation" {
            serde_json::from_slice(OUTSIDE).unwrap()
        } else {
            serde_json::json!({"key":"outside","source":source,"generated_key":"refused","edits":[{"owner":owner,"quality":quality,"operation":"remove"}]})
        };
        fs::write(
            fixture.0.join("refused.json"),
            serde_json::to_vec(&plan).unwrap(),
        )
        .unwrap();
        let output = fixture.run(&[
            "edit",
            "--source",
            source,
            path,
            "--plan",
            "refused.json",
            "--document-output",
            "outside.json",
            "--format",
            "json",
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(reason));
        assert!(output.stdout.is_empty());
        assert!(!fixture.0.join("outside.json").exists());
    }
}
