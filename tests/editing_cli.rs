use esdieff::{Catalogue, Edit, EditRequest, JsonBuilder, SourceInput};
#[allow(dead_code)]
mod cli_color_support;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

const SOURCE: &[u8] = br#"{"sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"type":"number","maximum":10,"default":1,"x:opaque":[0.10]}}}}}"#;
const PLAN: &[u8] = br#"{"key":"correction","source":"base","generated_key":"revised","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"replace","value":"corrected"},{"owner":"/sdfObject/Sensor/sdfProperty/reading","quality":"default","operation":"replace","value":null},{"owner":"/sdfObject/Sensor/sdfProperty/reading","quality":"maximum","operation":"remove"}]}"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-edit-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("base.json"), SOURCE).unwrap();
        fs::write(path.join("plan.json"), PLAN).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_esdieff"));
        command.current_dir(&self.0).args([
            "edit",
            "--plan",
            "plan.json",
            "--source",
            "base",
            "base.json",
            "--format",
            "json",
        ]);
        command
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
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn refused(output: Output, code: i32) -> String {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn explicit_plan_matches_typed_edit_and_keeps_null_removal_and_inputs_distinct() {
    let fixture = Fixture::new();
    let output = fixture
        .command()
        .args(["--document-output", "revised.json"])
        .output()
        .unwrap();
    let wire = report(&output, 0);
    assert_eq!(wire["version"], "1.2");
    assert_eq!(wire["operation"], "edit");
    assert_eq!(wire["result"]["checking"]["state"], "not_requested");
    assert_eq!(wire["coverage"]["requested"]["count_unit"], "editing");
    assert_eq!(wire["coverage"]["requested"]["total"], 1);
    let bytes = fs::read(fixture.path("revised.json")).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["sdfObject"]["Sensor"]["description"], "corrected");
    let reading = &document["sdfObject"]["Sensor"]["sdfProperty"]["reading"];
    assert_eq!(reading["default"], serde_json::Value::Null);
    assert!(reading.get("default").is_some());
    assert!(reading.get("maximum").is_none());
    assert!(String::from_utf8_lossy(&bytes).contains("0.10"));
    assert_eq!(fs::read(fixture.path("base.json")).unwrap(), SOURCE);
    assert_eq!(fs::read(fixture.path("plan.json")).unwrap(), PLAN);
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("base", SOURCE)]).unwrap();
    let original = catalogue.source_id("base").unwrap();
    let typed = EditRequest::new(catalogue, "correction", original, "revised")
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor",
                "description",
                JsonBuilder::new().text("corrected").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_edit(
            Edit::replace(
                "/sdfObject/Sensor/sdfProperty/reading",
                "default",
                JsonBuilder::new().null().unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_edit(Edit::remove("/sdfObject/Sensor/sdfProperty/reading", "maximum").unwrap())
        .unwrap()
        .edit()
        .unwrap();
    assert_eq!(typed.generated_document().unwrap(), bytes);
    let repeated = fixture
        .command()
        .args(["--document-output", "again.json"])
        .output()
        .unwrap();
    assert!(repeated.status.success());
    assert_eq!(fs::read(fixture.path("again.json")).unwrap(), bytes);
}

#[test]
fn requested_checks_gate_document_delivery_and_keep_generation_independent() {
    for (assessment, strict, code) in [
        ("initial-v1", false, 0),
        ("initial-v1", true, 1),
        ("full-baseline", false, 1),
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.path("base.json"), br#"{"sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"type":"number","multipleOf":0.10,"maxLength":18446744073709551616,"default":1,"maximum":10}}}}}"#).unwrap();
        let mut command = fixture.command();
        command.args(["--document-output", "revised.json", "--check", assessment]);
        if strict {
            command.arg("--strict-warnings");
        }
        let output = command.output().unwrap();
        let wire = report(&output, code);
        assert_eq!(fixture.path("revised.json").exists(), code == 0);
        assert_eq!(wire["result"]["generation"], "complete");
        assert_eq!(wire["result"]["fidelity"]["state"], "established");
        assert_eq!(wire["result"]["checking"]["state"], "requested");
        assert_eq!(
            wire["result"]["checking"]["coverage"]["requested"]["count_unit"],
            "assessment_area"
        );
        if assessment == "full-baseline" {
            assert_eq!(
                wire["result"]["checking"]["coverage"]["requirements"]["total"],
                54
            );
        }
    }
    let fixture = Fixture::new();
    fs::write(fixture.path("base.json"), br##"{"namespace":{"n":"https://example.org/dependency"},"sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"sdfRef":"n:#/sdfData/reading"}}}}}"##).unwrap();
    fs::write(fixture.path("plan.json"), br#"{"key":"correction","source":"base","generated_key":"revised","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"replace","value":"corrected"}]}"#).unwrap();
    let output = fixture
        .command()
        .args([
            "--document-output",
            "revised.json",
            "--check",
            "initial-v1",
            "--source",
            "dependency",
            "missing.json",
        ])
        .output()
        .unwrap();
    let wire = report(&output, 3);
    assert!(!fixture.path("revised.json").exists());
    assert_eq!(wire["result"]["generation"], "complete");
    assert_eq!(wire["result"]["fidelity"]["state"], "established");
    assert_eq!(wire["coverage"]["requested"]["undetermined"], 1);
    assert_eq!(
        wire["request"]["scope"]["unavailable_required"],
        serde_json::json!([2])
    );
    fs::remove_file(fixture.path("base.json")).unwrap();
    let output = fixture
        .command()
        .args(["--document-output", "unreadable.json"])
        .output()
        .unwrap();
    let wire = report(&output, 3);
    assert_eq!(
        wire["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["key"] == "base")
            .unwrap()["acquisition"],
        "failed"
    );
    assert!(!fixture.path("unreadable.json").exists());
}

#[test]
fn human_and_json_reports_agree_on_findings_and_unfinished_source_work() {
    for unavailable in [false, true] {
        let fixture = Fixture::new();
        if unavailable {
            fs::remove_file(fixture.path("base.json")).unwrap();
        } else {
            fs::write(fixture.path("base.json"), br#"{"sdfObject":{"Sensor":{"description":"old","sdfProperty":{"reading":{"type":"number","multipleOf":0.10,"maxLength":18446744073709551616,"default":1,"maximum":10}}}}}"#).unwrap();
        }
        let expected = if unavailable { 3 } else { 1 };
        let json = fixture
            .command()
            .args([
                "--document-output",
                "json-output.json",
                "--check",
                "full-baseline",
            ])
            .output()
            .unwrap();
        let wire = report(&json, expected);
        let human = fixture
            .command()
            .args([
                "--document-output",
                "human-output.json",
                "--check",
                "full-baseline",
                "--format",
                "human",
                "--color",
                "always",
            ])
            .output()
            .unwrap();
        assert_eq!(human.status.code(), Some(expected));
        let plain = cli_color_support::plain(&human.stdout);
        assert!(
            plain.contains(if unavailable {
                "Edit correction: undetermined"
            } else {
                "Edit correction: failed"
            }),
            "{plain}"
        );
        assert!(
            plain.contains(&format!(
                "Generation: {}",
                wire["result"]["generation"].as_str().unwrap()
            )),
            "{plain}"
        );
        assert!(
            plain.contains(&format!(
                "Fidelity: {}",
                wire["result"]["fidelity"]["state"].as_str().unwrap()
            )),
            "{plain}"
        );
        assert!(
            plain.contains(&format!("Errors: {}", wire["outcome"]["errors"])),
            "{plain}"
        );
        assert!(!fixture.path("json-output.json").exists());
        assert!(!fixture.path("human-output.json").exists());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["--color", "never", "edit", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("esdieff edit --plan"));
}

#[test]
fn destinations_protect_all_inputs_and_refuse_collisions() {
    let fixture = Fixture::new();
    fs::write(fixture.path("other.json"), br#"{"sdfObject":{"Other":{}}}"#).unwrap();
    for (input, alias) in [
        ("base.json", "base-hard.json"),
        ("plan.json", "plan-hard.json"),
        ("other.json", "other-hard.json"),
    ] {
        fs::hard_link(fixture.path(input), fixture.path(alias)).unwrap();
        let original = fs::read(fixture.path(input)).unwrap();
        for target in [input, alias] {
            for report_output in [false, true] {
                let mut command = fixture.command();
                command.args(["--source", "other", "other.json"]);
                if report_output {
                    command.args([
                        "--document-output",
                        "out.json",
                        "--report-output",
                        target,
                        "--replace-report",
                    ]);
                } else {
                    command.args(["--document-output", target, "--replace-document"]);
                }
                assert!(refused(command.output().unwrap(), 2).contains("protected"));
                assert!(!fixture.path("out.json").exists());
            }
        }
        assert_eq!(fs::read(fixture.path(input)).unwrap(), original);
    }
    for arguments in [
        vec!["--document-output", "-"],
        vec![
            "--document-output",
            "same.json",
            "--report-output",
            "same.json",
        ],
    ] {
        refused(fixture.command().args(arguments).output().unwrap(), 2);
    }
    fs::write(fixture.path("doc.json"), "original destination").unwrap();
    fs::hard_link(fixture.path("doc.json"), fixture.path("report.json")).unwrap();
    assert!(
        refused(
            fixture
                .command()
                .args([
                    "--document-output",
                    "doc.json",
                    "--report-output",
                    "report.json",
                    "--replace-document",
                    "--replace-report"
                ])
                .output()
                .unwrap(),
            2
        )
        .contains("destinations alias")
    );
    assert_eq!(
        fs::read_to_string(fixture.path("doc.json")).unwrap(),
        "original destination"
    );
}

#[test]
fn replacement_requires_each_explicit_flag_and_never_creates_parents() {
    let fixture = Fixture::new();
    fs::write(fixture.path("document.json"), "old document").unwrap();
    fs::write(fixture.path("report.json"), "old report").unwrap();
    for args in [
        vec!["--document-output", "document.json"],
        vec![
            "--document-output",
            "new.json",
            "--report-output",
            "report.json",
        ],
        vec!["--document-output", "document.json", "--replace-report"],
        vec![
            "--document-output",
            "-",
            "--report-output",
            "new-report.json",
            "--replace-document",
        ],
        vec!["--document-output", "new.json", "--replace-report"],
        vec!["--document-output", "missing-parent/new.json"],
        vec!["--document-output", ".", "--replace-document"],
    ] {
        refused(fixture.command().args(args).output().unwrap(), 2);
    }
    assert!(!fixture.path("missing-parent").exists());
    assert_eq!(
        fs::read_to_string(fixture.path("document.json")).unwrap(),
        "old document"
    );
    assert_eq!(
        fs::read_to_string(fixture.path("report.json")).unwrap(),
        "old report"
    );
    let output = fixture
        .command()
        .args([
            "--document-output",
            "document.json",
            "--report-output",
            "report.json",
            "--replace-document",
            "--replace-report",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(fixture.path("report.json")).unwrap()
        )
        .unwrap()["operation"],
        "edit"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(fixture.path("document.json")).unwrap()
        )
        .unwrap()["sdfObject"]["Sensor"]["description"],
        "corrected"
    );
    assert_no_staging(&fixture);
}

fn assert_no_staging(fixture: &Fixture) {
    assert!(!fs::read_dir(&fixture.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".esdieff-stage-")
    }));
}

#[cfg(unix)]
#[test]
fn source_symlinks_and_resolved_parents_do_not_allow_evidence_overwrites() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    symlink(".", fixture.path("parent-alias")).unwrap();
    symlink("base.json", fixture.path("input-link.json")).unwrap();
    for args in [
        vec!["--document-output", "input-link.json", "--replace-document"],
        vec![
            "--document-output",
            "parent-alias/base.json",
            "--replace-document",
        ],
        vec![
            "--document-output",
            "out.json",
            "--report-output",
            "parent-alias/plan.json",
            "--replace-report",
        ],
        vec![
            "--document-output",
            "parent-alias/absent.json",
            "--source",
            "missing",
            "absent.json",
        ],
        vec![
            "--document-output",
            "out.json",
            "--report-output",
            "parent-alias/absent.json",
            "--source",
            "missing",
            "absent.json",
        ],
        vec![
            "--document-output",
            "absent-out.json",
            "--report-output",
            "parent-alias/absent-out.json",
        ],
    ] {
        refused(fixture.command().args(args).output().unwrap(), 2);
    }
    // A symlink supplied as input still protects its target through same-file identity.
    refused(
        Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&fixture.0)
            .args([
                "edit",
                "--source",
                "base",
                "input-link.json",
                "--plan",
                "plan.json",
                "--document-output",
                "base.json",
                "--replace-document",
            ])
            .output()
            .unwrap(),
        2,
    );
    assert_eq!(fs::read(fixture.path("base.json")).unwrap(), SOURCE);
    assert_eq!(fs::read(fixture.path("plan.json")).unwrap(), PLAN);
}

#[test]
fn document_stdout_is_pure_json_under_every_color_and_report_mode() {
    let fixture = Fixture::new();
    let mut expected = None;
    for color in ["auto", "always", "never"] {
        for format in ["human", "json"] {
            let target = format!("{color}-{format}.report");
            let output = fixture
                .command()
                .args([
                    "--document-output",
                    "-",
                    "--report-output",
                    &target,
                    "--color",
                    color,
                    "--format",
                    format,
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            assert!(!output.stdout.contains(&0x1b));
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["sdfObject"]["Sensor"]
                    ["description"],
                "corrected"
            );
            if let Some(expected) = &expected {
                assert_eq!(&output.stdout, expected);
            } else {
                expected = Some(output.stdout);
            }
            let report = fs::read(fixture.path(&target)).unwrap();
            assert_eq!(
                report.contains(&0x1b),
                color == "always" && format == "human"
            );
            if format == "json" {
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&report).unwrap()["result"]["checking"]
                        ["state"],
                    "not_requested"
                );
            } else {
                let plain = cli_color_support::plain(&report);
                assert!(plain.contains("Checking: not requested"), "{plain}");
                assert!(plain.contains("Fidelity: established"), "{plain}");
            }
        }
    }
    let output = fixture
        .command()
        .args([
            "--document-output",
            "for-human.json",
            "--format",
            "human",
            "--color",
            "always",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let human = cli_color_support::plain(&output.stdout);
    assert!(
        human.contains("Requested editing coverage: total 1, assessed 1, undetermined 0"),
        "{human}"
    );
}

#[test]
fn closed_stdout_is_a_host_error_before_or_after_document_delivery() {
    let fixture = Fixture::new();
    let plan = fixture.path("plan.json");
    let source = fixture.path("base.json");
    let output_document = fixture.path("out.json");
    let output_report = fixture.path("report.json");
    let common = [
        "edit",
        "--plan",
        plan.to_str().unwrap(),
        "--source",
        "base",
        source.to_str().unwrap(),
        "--format",
        "json",
    ];
    let mut args = common.to_vec();
    args.extend([
        "--document-output",
        "-",
        "--report-output",
        output_report.to_str().unwrap(),
    ]);
    let error = refused(cli_color_support::closed_stdout(&args, b""), 4);
    assert!(error.contains("delivery not completed to standard output"));
    assert!(!output_report.exists());
    let mut args = common.to_vec();
    args.extend(["--document-output", output_document.to_str().unwrap()]);
    let error = refused(cli_color_support::closed_stdout(&args, b""), 4);
    assert!(error.contains("already delivered (committed)"));
    assert!(error.contains(output_document.to_str().unwrap()));
    assert!(output_document.exists());
    assert_no_staging(&fixture);
}

#[cfg(unix)]
fn after_destination_validation(fixture: &Fixture, args: &[&str], change: impl FnOnce()) -> Output {
    let plan = fixture.path("plan.json");
    fs::remove_file(&plan).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(&plan)
            .status()
            .unwrap()
            .success()
    );
    let child = fixture
        .command()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Opening a FIFO writer proves that the child reached plan acquisition after validation.
    let mut writer = fs::OpenOptions::new().write(true).open(&plan).unwrap();
    change();
    writer.write_all(PLAN).unwrap();
    drop(writer);
    child.wait_with_output().unwrap()
}

#[cfg(unix)]
#[test]
fn delivery_refuses_nonalias_source_identity_changes_before_file_or_stdout_output() {
    for stdout in [false, true] {
        let fixture = Fixture::new();
        let replacement = br#"{"sdfObject":{"Sensor":{"description":"replacement","sdfProperty":{"reading":{"type":"number","maximum":999,"default":1,"x:opaque":[0.10]}}}}}"#;
        fs::write(fixture.path("replacement.json"), replacement).unwrap();
        let args = if stdout {
            vec!["--document-output", "-", "--report-output", "report.json"]
        } else {
            vec!["--document-output", "out.json"]
        };
        let output = after_destination_validation(&fixture, &args, || {
            fs::rename(fixture.path("replacement.json"), fixture.path("base.json")).unwrap();
        });
        let error = refused(output, 4);
        assert!(
            error.contains("protected input identity changed"),
            "{error}"
        );
        assert!(!fixture.path("out.json").exists());
        assert!(!fixture.path("report.json").exists());
        assert_eq!(fs::read(fixture.path("base.json")).unwrap(), replacement);
        assert_no_staging(&fixture);
    }
}

#[cfg(unix)]
#[test]
fn delivery_refuses_nonalias_plan_identity_changes_before_file_or_stdout_output() {
    for stdout in [false, true] {
        let fixture = Fixture::new();
        fs::write(fixture.path("replacement-plan.json"), PLAN).unwrap();
        let args = if stdout {
            vec!["--document-output", "-", "--report-output", "report.json"]
        } else {
            vec!["--document-output", "out.json"]
        };
        let output = after_destination_validation(&fixture, &args, || {
            // The acquired FIFO remains readable, but its retained path now has a new identity.
            fs::rename(
                fixture.path("replacement-plan.json"),
                fixture.path("plan.json"),
            )
            .unwrap();
        });
        let error = refused(output, 4);
        assert!(
            error.contains("protected input identity changed"),
            "{error}"
        );
        assert!(!fixture.path("out.json").exists());
        assert!(!fixture.path("report.json").exists());
        assert_eq!(fs::read(fixture.path("base.json")).unwrap(), SOURCE);
        assert_eq!(fs::read(fixture.path("plan.json")).unwrap(), PLAN);
        assert_no_staging(&fixture);
    }
}

#[cfg(unix)]
#[test]
fn delivery_refuses_changed_destinations_and_new_input_aliases() {
    use std::os::unix::fs::symlink;
    for changed in 0..5 {
        let fixture = Fixture::new();
        if changed == 1 || changed == 4 {
            fs::write(fixture.path("out.json"), SOURCE).unwrap();
        }
        if changed == 3 {
            fs::create_dir(fixture.path("first")).unwrap();
            fs::create_dir(fixture.path("second")).unwrap();
            symlink("first", fixture.path("parent")).unwrap();
        }
        let target = if changed == 3 {
            "parent/out.json"
        } else {
            "out.json"
        };
        let args = if changed == 1 || changed == 4 {
            vec!["--document-output", target, "--replace-document"]
        } else {
            vec!["--document-output", target]
        };
        let output = after_destination_validation(&fixture, &args, || match changed {
            0 => fs::write(fixture.path("out.json"), "introduced destination").unwrap(),
            1 => {
                fs::remove_file(fixture.path("out.json")).unwrap();
                fs::write(fixture.path("out.json"), "replacement identity").unwrap();
            }
            2 => fs::hard_link(fixture.path("base.json"), fixture.path("out.json")).unwrap(),
            3 => {
                fs::remove_file(fixture.path("parent")).unwrap();
                symlink("second", fixture.path("parent")).unwrap();
            }
            4 => {
                fs::remove_file(fixture.path("base.json")).unwrap();
                symlink("out.json", fixture.path("base.json")).unwrap();
            }
            _ => unreachable!(),
        });
        let error = refused(output, 4);
        assert!(
            error.contains("identity changed") || error.contains("aliases a protected"),
            "{error}"
        );
        if changed == 0 {
            assert_eq!(
                fs::read_to_string(fixture.path(target)).unwrap(),
                "introduced destination"
            );
        }
        if changed == 1 {
            assert_eq!(
                fs::read_to_string(fixture.path(target)).unwrap(),
                "replacement identity"
            );
        }
        if changed == 2 || changed == 4 {
            assert_eq!(fs::read(fixture.path(target)).unwrap(), SOURCE);
        }
        if changed == 3 {
            assert!(!fixture.path(target).exists());
        }
        assert_no_staging(&fixture);
    }
}

#[cfg(unix)]
#[test]
fn changed_report_discloses_the_delivered_document_and_new_output_alias_is_refused() {
    let fixture = Fixture::new();
    let output = after_destination_validation(
        &fixture,
        &[
            "--document-output",
            "out.json",
            "--report-output",
            "report.json",
        ],
        || {
            fs::write(fixture.path("report.json"), "introduced report").unwrap();
        },
    );
    let error = refused(output, 4);
    assert!(error.contains("identity changed"));
    assert!(error.contains("already delivered (committed) to out.json"));
    assert!(fixture.path("out.json").exists());
    assert_eq!(
        fs::read_to_string(fixture.path("report.json")).unwrap(),
        "introduced report"
    );
    assert_no_staging(&fixture);

    let fixture = Fixture::new();
    fs::write(fixture.path("out.json"), "original document").unwrap();
    fs::write(fixture.path("report.json"), "original report").unwrap();
    let output = after_destination_validation(
        &fixture,
        &[
            "--document-output",
            "out.json",
            "--report-output",
            "report.json",
            "--replace-document",
            "--replace-report",
        ],
        || {
            fs::remove_file(fixture.path("report.json")).unwrap();
            fs::hard_link(fixture.path("out.json"), fixture.path("report.json")).unwrap();
        },
    );
    assert!(refused(output, 4).contains("destinations now alias"));
    assert_eq!(
        fs::read_to_string(fixture.path("out.json")).unwrap(),
        "original document"
    );
    assert_eq!(
        fs::read_to_string(fixture.path("report.json")).unwrap(),
        "original document"
    );
    assert_no_staging(&fixture);
}

#[cfg(unix)]
#[test]
fn exhausted_staging_names_do_not_overwrite_preexisting_files() {
    let fixture = Fixture::new();
    let plan = fixture.path("plan.json");
    fs::remove_file(&plan).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(&plan)
            .status()
            .unwrap()
            .success()
    );
    let child = fixture
        .command()
        .args(["--document-output", "out.json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut writer = fs::OpenOptions::new().write(true).open(&plan).unwrap();
    for sequence in 0..100 {
        fs::write(
            fixture.path(&format!(".esdieff-stage-{}-{sequence}", child.id())),
            "prior staged evidence",
        )
        .unwrap();
    }
    writer.write_all(PLAN).unwrap();
    drop(writer);
    assert!(
        refused(child.wait_with_output().unwrap(), 4)
            .contains("bounded staging-name attempts exhausted")
    );
    assert!(!fixture.path("out.json").exists());
    for entry in fs::read_dir(&fixture.0).unwrap() {
        let entry = entry.unwrap();
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(".esdieff-stage-")
        {
            assert_eq!(
                fs::read_to_string(entry.path()).unwrap(),
                "prior staged evidence"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn failed_staging_write_is_cleaned_up_and_returns_a_host_error() {
    let fixture = Fixture::new();
    let mut source: serde_json::Value = serde_json::from_slice(SOURCE).unwrap();
    source["sdfObject"]["Sensor"]["x:large"] = serde_json::json!("x".repeat(4096));
    fs::write(
        fixture.path("base.json"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    let output = Command::new("bash")
        .current_dir(&fixture.0)
        .args([
            "-c",
            "trap '' XFSZ; ulimit -f 1; exec \"$@\"",
            "esdieff-stage-test",
            env!("CARGO_BIN_EXE_esdieff"),
            "edit",
            "--plan",
            "plan.json",
            "--source",
            "base",
            "base.json",
            "--document-output",
            "out.json",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let error = refused(output, 4);
    assert!(error.contains("cannot stage output"), "{error}");
    assert!(error.contains("not delivered (not committed)"), "{error}");
    assert!(!fixture.path("out.json").exists());
    assert_no_staging(&fixture);
}

#[test]
fn invalid_plans_and_invocations_never_deliver_a_document() {
    let fixture = Fixture::new();
    for plan in [
        "{",
        "[]",
        r#"{"key":"r","source":"base","generated_key":"out","edits":[],"unknown":1}"#,
        r#"{"key":"r","source":"base","generated_key":"base","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove"}]}"#,
        r#"{"key":"r","source":"not-base","generated_key":"out","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove"}]}"#,
        r#"{"key":"r","source":"base","generated_key":"out","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"replace"}]}"#,
        r#"{"key":"r","source":"base","generated_key":"out","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove","value":null}]}"#,
        r#"{"key":"r","source":"base","generated_key":"out","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"merge","value":{}}]}"#,
        r#"{"key":"r","source":"base","generated_key":"out","edits":[{"owner":"/sdfObject/Sensor","quality":"label","operation":"remove"}]}"#,
        r#"{"key":"r","source":"base","generated_key":"out","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove"},{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove"}]}"#,
        r#"{"key":"r","\u006bey":"r","source":"base","generated_key":"out","edits":[{"owner":"/sdfObject/Sensor","quality":"description","operation":"remove"}]}"#,
    ] {
        fs::write(fixture.path("plan.json"), plan).unwrap();
        refused(
            fixture
                .command()
                .args(["--document-output", "out.json"])
                .output()
                .unwrap(),
            2,
        );
        assert!(!fixture.path("out.json").exists());
    }
    fs::write(fixture.path("plan.json"), PLAN).unwrap();
    for args in [
        vec![],
        vec!["--document-output", "out.json", "--directory", "."],
        vec![
            "--document-output",
            "out.json",
            "--check",
            "expanded-units-v1",
        ],
        vec![
            "--document-output",
            "out.json",
            "--source",
            "base",
            "base.json",
        ],
        vec!["--document-output", "out.json", "--plan", "plan.json"],
    ] {
        refused(fixture.command().args(args).output().unwrap(), 2);
    }
    fs::write(fixture.path("plan.json"), vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    assert!(
        refused(
            fixture
                .command()
                .args(["--document-output", "out.json"])
                .output()
                .unwrap(),
            2
        )
        .contains("4 MiB")
    );
    assert_eq!(fs::read(fixture.path("base.json")).unwrap(), SOURCE);
}

#[test]
fn one_stdin_can_supply_either_plan_or_source() {
    let fixture = Fixture::new();
    for (plan, source, input, target) in [
        ("-", "base.json", PLAN, "plan-stdin.json"),
        ("plan.json", "-", SOURCE, "source-stdin.json"),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&fixture.0)
            .args([
                "edit",
                "--plan",
                plan,
                "--source",
                "base",
                source,
                "--document-output",
                target,
                "--format",
                "json",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        assert_eq!(
            report(&child.wait_with_output().unwrap(), 0)["operation"],
            "edit"
        );
    }
    refused(
        Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(&fixture.0)
            .args([
                "edit",
                "--plan",
                "-",
                "--source",
                "base",
                "-",
                "--document-output",
                "out.json",
            ])
            .output()
            .unwrap(),
        2,
    );
    refused(
        fixture
            .command()
            .args([
                "--document-output",
                "out.json",
                "--source",
                "a",
                "-",
                "--source",
                "b",
                "-",
            ])
            .output()
            .unwrap(),
        2,
    );
}

#[test]
fn metadata_and_source_attempt_ceilings_apply_before_acquisition() {
    let fixture = Fixture::new();
    let mut accepted = fixture.command();
    accepted.args(["--document-output", "out.json"]);
    for index in 1..255 {
        accepted.args(["--source", &format!("source{index}"), "base.json"]);
    }
    let wire = report(&accepted.output().unwrap(), 0);
    assert_eq!(wire["sources"].as_array().unwrap().len(), 256);
    accepted.args([
        "--source",
        "one-too-many",
        "base.json",
        "--replace-document",
    ]);
    assert!(refused(accepted.output().unwrap(), 2).contains("255"));
    let mut metadata = fixture.command();
    metadata.args(["--document-output", "metadata.json"]);
    let large = "x".repeat(5000);
    for index in 1..255 {
        metadata.args(["--source", &format!("source{index}"), &large]);
    }
    let metadata = metadata.output();
    // These native process boundaries are lower than the CLI metadata budget.
    #[cfg(windows)]
    assert_eq!(metadata.unwrap_err().raw_os_error(), Some(206));
    #[cfg(target_os = "macos")]
    assert_eq!(
        metadata.unwrap_err().kind(),
        std::io::ErrorKind::ArgumentListTooLong
    );
    #[cfg(not(any(windows, target_os = "macos")))]
    assert!(refused(metadata.unwrap(), 2).contains("metadata"));
    assert!(!fixture.path("metadata.json").exists());
}
