//! The offline device Catalogue is observed only through the Check plan CLI.
mod catalogue_support;
mod coffee_machine_catalogue;
mod door_lock_catalogue;
mod irrigation_catalogue;
mod power_strip_catalogue;
mod rgb_lamp_catalogue;
mod thermostat_catalogue;

use catalogue_support::{Admission, Case, Plan, compare, compare_report, run};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    family_root("thermostat-check")
}

fn family_root(directory: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(directory)
}

fn families() -> [(&'static str, Vec<Plan>); 6] {
    [
        ("thermostat-check", thermostat_catalogue::plans()),
        ("rgb-lamp-check", rgb_lamp_catalogue::plans()),
        ("door-lock-check", door_lock_catalogue::plans()),
        ("power-strip-check", power_strip_catalogue::plans()),
        ("irrigation-check", irrigation_catalogue::plans()),
        ("coffee-machine-check", coffee_machine_catalogue::plans()),
    ]
}

#[test]
fn expected_native_rejection_passes_the_regression_runner() {
    let mut plan = thermostat_catalogue::plans().remove(0);
    let output = run(&root().join(plan.file), "json").unwrap();
    compare(&plan, &output).unwrap();
    plan.cases[3].admission = Some(Admission::Admitted);
    assert!(
        compare(&plan, &output)
            .unwrap_err()
            .contains("thermostat/entry/Too hot target: admission")
    );
    plan.cases[3].admission = Some(Admission::Rejected);
    plan.cases[3].complete = false;
    assert!(
        compare(&plan, &output)
            .unwrap_err()
            .contains("thermostat/entry/Too hot target: completion")
    );
}

#[test]
fn thermostat_catalogue_matches_independent_expectations() {
    check_family("thermostat-check", thermostat_catalogue::plans());
}

fn check_family(directory: &str, plans: Vec<Plan>) {
    let root = family_root(directory);
    let before = input_bytes(&root);
    for plan in plans {
        compare(&plan, &run(&root.join(plan.file), "json").unwrap()).unwrap();
    }
    assert_eq!(
        input_bytes(&root),
        before,
        "checking changed source or sample bytes"
    );
}

#[test]
fn rgb_lamp_catalogue_matches_independent_expectations() {
    check_family("rgb-lamp-check", rgb_lamp_catalogue::plans());
}

#[test]
fn door_lock_catalogue_matches_independent_expectations() {
    check_family("door-lock-check", door_lock_catalogue::plans());
}

#[test]
fn power_strip_catalogue_matches_independent_expectations() {
    check_family("power-strip-check", power_strip_catalogue::plans());
}

#[test]
fn irrigation_catalogue_matches_independent_expectations() {
    check_family("irrigation-check", irrigation_catalogue::plans());
}

#[test]
fn coffee_machine_catalogue_matches_independent_expectations() {
    check_family("coffee-machine-check", coffee_machine_catalogue::plans());
}

#[test]
fn all_expected_source_acquisitions_must_remain_visible() {
    let plan = power_strip_catalogue::plans().remove(2);
    let output = run(&family_root("power-strip-check").join(plan.file), "json").unwrap();
    let good = compare(&plan, &output).unwrap();
    let mut changed = good.clone();
    changed["sources"][1]["acquisition"] = json!("acquired");
    assert!(
        compare_report(&plan, &changed, 3)
            .unwrap_err()
            .contains("meter: source acquisition")
    );
    for (index, key) in [(1, "meter"), (2, "on"), (3, "ordinary-total")] {
        let mut changed = good.clone();
        changed["sources"].as_array_mut().unwrap().remove(index);
        assert!(
            compare_report(&plan, &changed, 3)
                .unwrap_err()
                .contains(&format!("{key}: missing source acquisition attempt"))
        );
    }
}

fn controlled_plan() -> Plan {
    Plan {
        device: "controlled",
        variant: "accounting",
        file: "supplied-records",
        exit: 0,
        source_keys: &[],
        failed_sources: &[],
        cases: vec![
            Case::model("first", "A supplied record exercises identity accounting"),
            Case::model("second", "A second record must also be accounted for"),
        ],
    }
}
fn records(names: &[&str]) -> Value {
    json!({"schema":"esdieff.check-run", "version":"1.1", "cases": names.iter().map(|name| json!({"name":name})).collect::<Vec<_>>()})
}
#[test]
fn every_case_and_expectation_must_be_accounted_for_once() {
    for (names, message) in [
        (vec!["first", "surprise"], "surprise: missing expectation"),
        (vec!["first"], "second: unused expectation"),
        (
            vec!["first", "first", "second"],
            "first: duplicate observed case identity",
        ),
    ] {
        assert!(
            compare_report(&controlled_plan(), &records(&names), 0)
                .unwrap_err()
                .contains(message)
        );
    }
    let mut plan = controlled_plan();
    plan.cases.push(plan.cases[0].clone());
    assert!(
        compare_report(&plan, &records(&["first", "second"]), 0)
            .unwrap_err()
            .contains("first: duplicate expectation")
    );
    plan.cases.pop();
    plan.cases[0].rationale = "";
    assert!(
        compare_report(&plan, &records(&["first", "second"]), 0)
            .unwrap_err()
            .contains("first: missing independent rationale")
    );
}

#[test]
fn malformed_unavailable_and_host_failure_outputs_cannot_pass() {
    let plan = thermostat_catalogue::plans().remove(0);
    let mut output = run(&root().join(plan.file), "json").unwrap();
    let good = compare(&plan, &output).unwrap();
    for bytes in [b"".as_slice(), b"{"] {
        output.stdout = bytes.to_vec();
        assert!(
            compare(&plan, &output)
                .unwrap_err()
                .contains("malformed or unavailable report")
        );
    }
    for (pointer, replacement, message) in [
        (
            "/cases/3/report",
            Value::Null,
            "Too hot target: native report unavailable",
        ),
        (
            "/cases/3/outcome/complete",
            json!("true"),
            "Too hot target: completion",
        ),
        (
            "/cases/3/report/coverage",
            Value::Null,
            "Too hot target: coverage inventory",
        ),
        (
            "/cases/1/request/assessment",
            json!("esdieff.initial/1"),
            "Thermostat model: assessment",
        ),
        ("/version", json!("2.0"), "run version"),
        (
            "/cases/3/state",
            json!("request_error"),
            "Too hot target: state",
        ),
        (
            "/cases/3/report/diagnostics/items/0/related/0/pointer",
            json!("/wrong"),
            "Too hot target: missing diagnostic",
        ),
    ] {
        let mut changed = good.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            compare_report(&plan, &changed, 1)
                .unwrap_err()
                .contains(message)
        );
    }
    assert!(
        compare_report(&plan, &good, 4)
            .unwrap_err()
            .contains("CLI exit")
    );
    output.stdout = serde_json::to_vec(&good).unwrap();
    output.stderr = b"host failure".to_vec();
    assert!(compare(&plan, &output).unwrap_err().contains("CLI stderr"));
    assert!(
        compare(
            &plan,
            &run(&root().join("absent-plan.json"), "json").unwrap()
        )
        .is_err()
    );
}

fn input_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn read(root: &Path, directory: &Path, bytes: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                read(root, &path, bytes);
            } else {
                bytes.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut bytes = BTreeMap::new();
    read(root, root, &mut bytes);
    bytes
}
struct CopyDirectory(PathBuf);
impl CopyDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-device-catalogue-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for CopyDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn copied_device_plans_keep_relative_sources_and_input_bytes() {
    for (directory, plans) in families() {
        let root = family_root(directory);
        let before = input_bytes(&root);
        let copy = CopyDirectory::new();
        for (relative, bytes) in &before {
            let path = copy.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        for plan in plans {
            compare(&plan, &run(&copy.0.join(plan.file), "json").unwrap()).unwrap();
        }
        assert_eq!(
            input_bytes(&copy.0),
            before,
            "{directory}: copied bytes changed"
        );
        assert_eq!(
            input_bytes(&root),
            before,
            "{directory}: original bytes changed"
        );
    }
}

#[test]
fn human_cases_agree_with_named_json_admissions_outcomes_and_completion() {
    for (directory, plans) in families() {
        let root = family_root(directory);
        for plan in plans {
            let wire = compare(&plan, &run(&root.join(plan.file), "json").unwrap()).unwrap();
            let human = run(&root.join(plan.file), "human").unwrap();
            assert_eq!(human.status.code(), Some(plan.exit));
            assert!(human.stderr.is_empty());
            let text = String::from_utf8(human.stdout).unwrap();
            for case in wire["cases"].as_array().unwrap() {
                let marker = format!("\nCase: {}\n", case["name"]);
                let section = text
                    .split_once(&marker)
                    .unwrap()
                    .1
                    .split("\nCase:")
                    .next()
                    .unwrap();
                for (heading, value) in [
                    ("Disposition", &case["outcome"]["disposition"]),
                    ("Complete", &case["outcome"]["complete"]),
                    ("Admission", &case["admission"]),
                    ("Classification", &case["classification"]),
                ] {
                    if !value.is_null() {
                        let value = value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string());
                        assert!(
                            section
                                .lines()
                                .any(|line| line == format!("{heading}: {value}")),
                            "{}: {heading}",
                            case["name"]
                        );
                    }
                }
            }
        }
    }
}
