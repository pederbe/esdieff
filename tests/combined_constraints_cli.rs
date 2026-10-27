//! Combined sample and advisory outcomes retain shared CLI/report semantics.
use serde_json::Value;
use std::process::Command;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("esdieff-combined-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn combined_samples_and_consistency_advice_keep_cli_human_json_parity() {
    let files = Fixture::new();
    let root = files.write(
        "root.json",
        include_bytes!("fixtures/combined-constraints/root.sdf.json"),
    );
    let donor = files.write(
        "donor.json",
        include_bytes!("fixtures/combined-constraints/donor.sdf.json"),
    );
    for (raw, admission, exit) in [
        (
            include_bytes!("fixtures/combined-constraints/admitted.json").as_slice(),
            "admitted",
            0,
        ),
        (
            include_bytes!("fixtures/combined-constraints/over-bound.json").as_slice(),
            "rejected",
            1,
        ),
    ] {
        let sample = files.write("sample.json", raw);
        for format in ["human", "json"] {
            let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
                .args([
                    "sample",
                    "--document",
                    &root,
                    "--kind",
                    "property",
                    "--pointer",
                    "/sdfProperty/p",
                    "--sample",
                    &sample,
                    "--format",
                    format,
                    &root,
                    &donor,
                ])
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(exit),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(output.stderr.is_empty());
            if format == "human" {
                let human = String::from_utf8(output.stdout).unwrap();
                assert!(human.contains(admission));
                assert!(human.contains("Complete: true"));
                if exit == 1 {
                    assert!(human.contains("sample.bound") && human.contains("/level"));
                }
            } else {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["version"], "1.1");
                assert_eq!(wire["result"]["admission"], admission);
                assert_eq!(wire["outcome"]["complete"], true);
                assert_eq!(wire["coverage"]["requested"]["total"], 9);
                assert_eq!(wire["coverage"]["broader"]["state"], "partial");
            }
        }
        assert_eq!(std::fs::read(&sample).unwrap(), raw);
    }
    let mut description: Value = serde_json::from_slice(include_bytes!(
        "fixtures/combined-constraints/root.sdf.json"
    ))
    .unwrap();
    description["sdfProperty"]["p"]["default"] = serde_json::from_slice(include_bytes!(
        "fixtures/combined-constraints/over-bound.json"
    ))
    .unwrap();
    let root = files.write("with-default.json", description.to_string().as_bytes());
    for strict in [false, true] {
        for format in ["human", "json"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
            command.args([
                "check",
                "--root",
                &root,
                "--assessment",
                "expanded-consistency-v1",
                "--format",
                format,
            ]);
            if strict {
                command.arg("--strict-warnings");
            }
            let output = command.args([&root, &donor]).output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(i32::from(strict)),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(output.stderr.is_empty());
            if format == "human" {
                let human = String::from_utf8(output.stdout).unwrap();
                assert!(
                    human.contains("consistency.default_excluded")
                        && human.contains("Complete: true")
                );
            } else {
                let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(wire["version"], "1.0");
                assert_eq!(wire["outcome"]["errors"], 0);
                assert_eq!(wire["outcome"]["warnings"], 1);
                assert_eq!(wire["outcome"]["complete"], true);
                assert_eq!(wire["coverage"]["requested"]["total"], 10);
                let advice = wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|item| item["code"] == "consistency.default_excluded")
                    .unwrap();
                assert_eq!(advice["severity"], "warning");
                assert_eq!(advice["primary"]["pointer"], "/sdfProperty/p/default/level");
            }
        }
    }
}
