//! Saved checks are observed at the approved CLI boundary.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-check-plan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        // Unix current_dir resolves temporary-directory aliases such as /var.
        // Use that same parent spelling without canonicalizing any supplied plan.
        #[cfg(unix)]
        let path = path.canonicalize().unwrap();
        Self(path)
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
    fn plan(&self, cases: Value) -> PathBuf {
        self.write("plan.json", serde_json::to_vec(&json!({"schema":"esdieff.check-plan", "version":"1", "sources":[{"key":"device", "path":"device.json", "role":"model"}], "cases":cases})).unwrap())
    }
    fn mixed(&self, cases: Value) -> PathBuf {
        self.write(
            "plan.json",
            serde_json::to_vec(
                &json!({"schema":"esdieff.check-plan","version":"1","sources":[
            {"key":"device","path":"device.json","role":"model"},
            {"key":"value","path":"value.json","role":"sample"}],"cases":cases}),
            )
            .unwrap(),
        )
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(path: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--plan"])
        .arg(path)
        .args(extra)
        .output()
        .unwrap()
}
fn wire(output: &Output, exit: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}
fn keys(record: &Value) -> Vec<&str> {
    let mut keys: Vec<_> = record
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}
fn assert_native_report(actual: &Value, expected: &Value, context: &str) {
    for field in [
        "request",
        "outcome",
        "baseline",
        "policy",
        "coverage",
        "diagnostics",
        "result",
    ] {
        assert_eq!(actual[field], expected[field], "{context}: {field}");
    }
}
#[test]
fn named_catalogue_check_preserves_native_result_and_input() {
    let files = Files::new();
    let bytes = br#"{"sdfData":{"Count":{"type":"integer","maximum":9007199254740993}}}"#;
    let model = files.write("device.json", bytes);
    let plan = files
        .plan(json!([{"name":"catalogue", "operation":"check", "scope":{"kind":"catalogue"}}]));
    let output = run(&plan, &["--format", "json"]);
    let result = wire(&output, 0);
    assert_eq!(result["schema"], "esdieff.check-run");
    assert_eq!(result["version"], "1.1");
    assert_eq!(result["sources"][0]["key"], "device");
    assert_eq!(result["cases"][0]["name"], "catalogue");
    let direct = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "json"])
        .arg(&model)
        .output()
        .unwrap();
    let direct = wire(&direct, 0);
    assert_native_report(&result["cases"][0]["report"], &direct, "catalogue");
    assert_eq!(result["cases"][0]["outcome"], direct["outcome"]);
    assert_eq!(result["summary"]["complete"], true);
    assert_eq!(result["summary"]["passed"], 1);
    assert_eq!(fs::read(model).unwrap(), bytes);
    let human = run(&plan, &[]);
    assert_eq!(human.status.code(), Some(0));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("catalogue"));
    assert!(human.contains("Broader RFC coverage: partial"));
}

#[test]
fn property_sample_and_model_check_share_sources_and_native_results() {
    let files = Files::new();
    let bytes = br#"{"info":{},"sdfProperty":{"p":{"type":"integer","maximum":9007199254740993}}}"#;
    let model = files.write("device.json", bytes);
    let sample = files.write("value.json", b"9007199254740993");
    let plan = files.write("plan.json", serde_json::to_vec(&json!({"schema":"esdieff.check-plan","version":"1","sources":[
        {"key":"device","path":"device.json","role":"model"},
        {"key":"value","path":"value.json","role":"sample"}],"cases":[
        {"name":"value","operation":"sample","document":"device","kind":"property","pointer":"/sdfProperty/p","sample":"value"},
        {"name":"model","operation":"check","scope":{"kind":"catalogue"}}
    ]})).unwrap());
    let output = run(&plan, &["--format", "json"]);
    let result = wire(&output, 0);
    let direct = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["sample", "--document"])
        .arg(&model)
        .args([
            "--kind",
            "property",
            "--pointer",
            "/sdfProperty/p",
            "--sample",
        ])
        .arg(&sample)
        .args(["--format", "json"])
        .arg(&model)
        .output()
        .unwrap();
    let direct = wire(&direct, 0);
    assert_native_report(&result["cases"][0]["report"], &direct, "sample");
    assert_eq!(result["cases"][0]["admission"], "admitted");
    assert_eq!(
        result["cases"][0]["request"]["inventory"],
        "esdieff.sample/1"
    );
    assert_eq!(
        result["cases"][1]["request"]["scope"]["assessed"],
        json!([1])
    );
    assert_eq!(result["summary"]["admitted"], 1);
    assert_eq!(result["summary"]["passed"], 2);
    assert_eq!(result["sources"].as_array().unwrap().len(), 2);
    assert_eq!(fs::read(model).unwrap(), bytes);
    assert_eq!(fs::read(sample).unwrap(), b"9007199254740993");
}

fn sample_case(name: &str, kind: &str, pointer: &str) -> Value {
    json!({"name":name,"operation":"sample","document":"device","kind":kind,"pointer":pointer,"sample":"value"})
}

#[test]
fn sample_request_errors_retain_only_tagged_fields_and_continue_independent_cases() {
    let files = Files::new();
    files.write("device.json", br#"{"info":{},"sdfProperty":{"p":{"type":"integer"}},"sdfData":{"D":{"type":"string"}},"sdfAction":{"a":{}}}"#);
    files.write("value.json", b"4");
    let errors = vec![
        sample_case("absent", "property", "/sdfProperty/missing"),
        sample_case("wrong class", "property", "/sdfData/D"),
        sample_case("no output", "action_output", "/sdfAction/a"),
    ];
    let mut cases = errors.clone();
    cases.push(sample_case("independent", "property", "/sdfProperty/p"));
    cases.push(json!({"name":"model","operation":"check","scope":{"kind":"catalogue"}}));
    let plan = files.mixed(json!(cases));
    let result = wire(&run(&plan, &["--format", "json"]), 2);
    for (index, case) in result["cases"]
        .as_array()
        .unwrap()
        .iter()
        .take(3)
        .enumerate()
    {
        assert_eq!(case["state"], "request_error");
        assert_eq!(case["request_error"]["code"], "sample.request");
        assert_eq!(
            case["request_error"]["location"],
            json!({"kind":"plan","pointer":format!("/cases/{index}")})
        );
        assert!(
            !case["request_error"]["message"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        let mut keys: Vec<_> = case
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["name", "operation", "request", "request_error", "state"]
        );
    }
    assert_eq!(result["cases"][3]["admission"], "admitted");
    assert_eq!(result["cases"][4]["outcome"]["disposition"], "passed");
    assert_eq!(result["summary"]["request_errors"], 3);
    assert_eq!(result["summary"]["complete_cases"], 2);
    assert_eq!(result["summary"]["incomplete_cases"], 3);
    assert_eq!(result["summary"]["verdict"], "invalid");
    let human = run(&plan, &[]);
    assert_eq!(human.status.code(), Some(2));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("wrong class"));
    assert!(human.contains("Admission: admitted"));
    assert!(human.contains(&serde_json::to_string(&result["summary"]).unwrap()));
    let all_errors = files.mixed(json!(errors));
    let result = wire(&run(&all_errors, &["--format", "json"]), 2);
    assert_eq!(result["sources"].as_array().unwrap().len(), 2);
    assert_eq!(result["summary"]["request_errors"], 3);
    assert_eq!(result["summary"]["omitted_reports"], 0);
    assert_eq!(result["summary"]["admitted"], 0);
}

#[test]
fn all_sample_kinds_admit_and_reject_with_direct_native_parity() {
    use esdieff::{Catalogue, SampleKind, SampleRequest, SourceInput};
    let files = Files::new();
    let model = files.write("device.json", br#"{"info":{},"sdfProperty":{"p":{"type":"integer","maximum":9007199254740992}},"sdfData":{"D":{"type":"integer","maximum":9007199254740992}},"sdfAction":{"a":{"sdfInputData":{"type":"integer","maximum":9007199254740992},"sdfOutputData":{"type":"integer","maximum":9007199254740992}}},"sdfEvent":{"e":{"sdfOutputData":{"type":"integer","maximum":9007199254740992}}}}"#);
    let kinds = [
        (SampleKind::Property, "/sdfProperty/p"),
        (SampleKind::Data, "/sdfData/D"),
        (SampleKind::ActionInput, "/sdfAction/a"),
        (SampleKind::ActionOutput, "/sdfAction/a"),
        (SampleKind::EventOutput, "/sdfEvent/e"),
    ];
    let cases: Vec<_> = kinds
        .iter()
        .map(|(kind, pointer)| sample_case(kind.as_str(), kind.as_str(), pointer))
        .collect();
    let plan = files.mixed(json!(cases));
    for (bytes, admission, exit) in [
        ("9007199254740992", "admitted", 0),
        ("9007199254740993", "rejected", 1),
    ] {
        let sample = files.write("value.json", bytes);
        let catalogue = Catalogue::take_inputs(vec![
            SourceInput::file("device", &model).label(model.to_str().unwrap()),
            SourceInput::file("value", &sample)
                .label(sample.to_str().unwrap())
                .sample(),
        ])
        .unwrap();
        let output = run(&plan, &["--format", "json"]);
        let result = wire(&output, exit);
        for (index, (kind, pointer)) in kinds.iter().enumerate() {
            let direct = catalogue
                .check_sample(
                    SampleRequest::new(catalogue.source_id("device").unwrap(), *kind, pointer)
                        .sample(catalogue.source_id("value").unwrap()),
                )
                .unwrap()
                .to_json();
            assert_eq!(
                result["cases"][index]["report"],
                serde_json::from_str::<Value>(&direct).unwrap()
            );
            assert_eq!(result["cases"][index]["admission"], admission);
            assert_eq!(
                result["cases"][index]["request"]["selector"]["pointer"],
                *pointer
            );
            assert_eq!(result["cases"][index]["outcome"]["complete"], true);
            // The full native JSON is embedded without a number round trip.
            assert!(
                String::from_utf8_lossy(&output.stdout).contains(&format!("\"report\":{direct}"))
            );
        }
        assert_eq!(result["summary"]["complete_cases"], 5);
        assert_eq!(result["summary"][admission], 5);
        assert_eq!(fs::read(sample).unwrap(), bytes.as_bytes());
        let human = run(&plan, &["--color", "always"]);
        assert_eq!(human.status.code(), Some(exit));
        assert!(human.stdout.contains(&0x1b));
        let human = String::from_utf8(human.stdout).unwrap();
        assert_eq!(
            human.matches(&format!("Admission: {admission}\n")).count(),
            5
        );
    }
}

#[test]
fn admission_strict_warnings_and_mixed_exit_precedence_remain_independent() {
    let files = Files::new();
    // `title` is a base quality outside a Property's role, so equality stays unfinished.
    files.write("device.json", br#"{"info":{},"sdfProperty":{"warn":{"type":"integer","const":1,"title":"t"},"reject":{"type":"string"},"unknown":{"type":"integer","const":1,"title":"t"},"good":{"type":"integer"},"null":{"type":"integer","nullable":true}}}"#);
    files.write("value.json", b"1");
    let mut strict = sample_case("strict", "property", "/sdfProperty/warn");
    strict["strict_warnings"] = json!(true);
    let plan = files.mixed(json!([
        strict,
        sample_case("unresolved", "property", "/sdfProperty/unknown"),
        sample_case("reject", "property", "/sdfProperty/reject"),
        sample_case("after", "property", "/sdfProperty/good")
    ]));
    let result = wire(&run(&plan, &["--format", "json"]), 1);
    assert_eq!(result["cases"][0]["admission"], "undetermined");
    assert_eq!(result["cases"][0]["outcome"]["disposition"], "undetermined");
    assert_eq!(result["cases"][0]["outcome"]["complete"], false);
    assert_eq!(result["cases"][0]["policy"]["strict_warnings"], true);
    assert_eq!(result["cases"][3]["admission"], "admitted");
    assert_eq!(result["summary"]["verdict"], "failed");
    let plan = files.mixed(json!([
        sample_case("unresolved", "property", "/sdfProperty/unknown"),
        sample_case("after", "property", "/sdfProperty/good")
    ]));
    let result = wire(&run(&plan, &["--format", "json"]), 3);
    assert_eq!(result["cases"][0]["admission"], "undetermined");
    assert_eq!(result["cases"][0]["outcome"]["disposition"], "undetermined");
    assert_eq!(result["summary"]["admission_undetermined"], 1);
    let plan = files.mixed(json!([sample_case(
        "unresolved",
        "property",
        "/sdfProperty/unknown"
    )]));
    let result = wire(&run(&plan, &["--format", "json"]), 3);
    assert_eq!(result["summary"]["undetermined"], 1);
    let plan = files.mixed(json!([
        sample_case("mistake", "property", "/sdfProperty/absent"),
        sample_case("unfinished", "property", "/sdfProperty/unknown"),
        sample_case("failure", "property", "/sdfProperty/reject")
    ]));
    let result = wire(&run(&plan, &["--format", "json"]), 2);
    assert_eq!(result["summary"]["request_errors"], 1);
    assert_eq!(result["summary"]["failed"], 1);
    assert_eq!(result["summary"]["undetermined"], 1);
    assert_eq!(result["summary"]["complete_cases"], 1);
    assert_eq!(result["summary"]["incomplete_cases"], 2);
    assert_eq!(result["summary"]["verdict"], "invalid");
    files.write("value.json", b"null");
    let plan = files.mixed(json!([sample_case(
        "supplied null",
        "property",
        "/sdfProperty/null"
    )]));
    let result = wire(&run(&plan, &["--format", "json"]), 0);
    assert_eq!(result["cases"][0]["admission"], "admitted");
    assert_eq!(fs::read(files.0.join("value.json")).unwrap(), b"null");
}

#[test]
fn sample_configuration_rejects_roles_aliases_extra_fields_and_malformed_selectors() {
    let files = Files::new();
    files.write("device.json", b"{}");
    files.write("value.json", b"null");
    let case = sample_case("sample", "property", "/sdfProperty/p");
    let mut invalid = Vec::new();
    for (key, value) in [
        ("document", json!("missing")),
        ("document", json!("value")),
        ("sample", json!("missing")),
        ("sample", json!("device")),
        ("document", json!(7)),
        ("sample", json!(null)),
        ("kind", json!("action")),
        ("kind", json!(null)),
        ("pointer", json!(5)),
        ("pointer", json!("sdfProperty/p")),
        ("pointer", json!("/sdfProperty/~")),
        ("pointer", json!("/sdfProperty/~2")),
        ("pointer", json!(format!("/{}", "x".repeat(8192)))),
        ("assessment", json!("initial-v1")),
        ("scope", json!({"kind":"catalogue"})),
        ("expected", json!("rejected")),
        ("strict_warnings", json!(1)),
    ] {
        let mut bad = case.clone();
        bad[key] = value;
        invalid.push(bad);
    }
    for key in ["document", "sample", "kind", "pointer"] {
        let mut bad = case.clone();
        bad.as_object_mut().unwrap().remove(key);
        invalid.push(bad);
    }
    #[cfg(unix)]
    {
        // A read of this source would block. Configuration must finish first.
        fs::remove_file(files.0.join("device.json")).unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(files.0.join("device.json"))
                .status()
                .unwrap()
                .success()
        );
    }
    for case in invalid {
        let plan = files.mixed(json!([case]));
        let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--plan"])
            .arg(&plan)
            .args(["--format", "json"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("invalid sample configuration acquired a source");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn sample_selector_limits_and_escaped_definition_names_keep_native_meaning() {
    let files = Files::new();
    files.write("value.json", b"1");
    for (name, pointer) in [
        ("slash/tilde~", "/sdfProperty/slash~1tilde~0".to_owned()),
        (
            &"x".repeat(8179),
            format!("/sdfProperty/{}", "x".repeat(8179)),
        ),
    ] {
        files.write(
            "device.json",
            serde_json::to_vec(&json!({"info":{},"sdfProperty":{name:{"type":"integer"}}}))
                .unwrap(),
        );
        assert!(pointer.len() <= 8192);
        let plan = files.mixed(json!([sample_case("value", "property", &pointer)]));
        let result = wire(&run(&plan, &["--format", "json"]), 0);
        assert_eq!(result["cases"][0]["admission"], "admitted");
        assert_eq!(
            result["cases"][0]["request"]["selector"]["pointer"],
            pointer
        );
    }
}

#[test]
fn repeated_sample_selectors_are_charged_to_plan_control_metadata() {
    let files = Files::new();
    let sources: Vec<_> = (0..256).map(|i| json!({"key":format!("source{i}"),"path":"x".repeat(1000),"role":if i == 1 { "sample" } else { "model" }})).collect();
    let selector = format!("/{}", "p".repeat(8191));
    let cases: Vec<_> = (0..64).map(|i| json!({"name":format!("sample{i}"),"operation":"sample","document":"source0","kind":"property","pointer":selector,"sample":"source1"})).collect();
    let bytes = serde_json::to_vec(
        &json!({"schema":"esdieff.check-plan","version":"1","sources":sources,"cases":cases}),
    )
    .unwrap();
    assert!(bytes.len() < 1024 * 1024);
    let plan = files.write("plan.json", bytes);
    let output = run(&plan, &["--format", "json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("control metadata")
    );
}

#[test]
fn copied_requiredness_completes_and_unavailable_contributors_preserve_independent_samples() {
    use esdieff::{Catalogue, SampleKind, SampleRequest, SourceInput};
    let files = Files::new();
    let root = files.write("device.json", br##"{"info":{},"namespace":{"n":"https://example.test/n"},"sdfProperty":{"independent":{"type":"integer"},"dependent":{"sdfRef":"n:#/sdfData/D"}},"sdfObject":{"Base":{"sdfRequired":["#/sdfObject/Base/sdfProperty/p"],"sdfProperty":{"p":{}}},"Copy":{"sdfRef":"#/sdfObject/Base"}}}"##);
    let donor = files.write("donor.json", br##"{"info":{},"namespace":{"n":"https://example.test/n"},"defaultNamespace":"n","sdfData":{"D":{"type":"integer"}}}"##);
    let sample = files.write("value.json", b"1");
    let unused = files.write("unneeded.json", b"{}");
    let cases = json!([
        {"name":"copied requiredness","operation":"check","scope":{"kind":"selected","roots":["device"]}},
        sample_case("independent", "property", "/sdfProperty/independent"),
        sample_case("dependent", "property", "/sdfProperty/dependent")
    ]);
    let sources = json!([
        {"key":"device","path":"device.json","role":"model"},
        {"key":"donor","path":"donor.json","role":"model"},
        {"key":"unused","path":"unneeded.json","role":"model"},
        {"key":"value","path":"value.json","role":"sample"}
    ]);
    let plan = files.write(
        "plan.json",
        serde_json::to_vec(
            &json!({"schema":"esdieff.check-plan","version":"1","sources":sources,"cases":cases}),
        )
        .unwrap(),
    );
    for missing in [false, true] {
        if missing {
            fs::remove_file(&donor).unwrap();
            fs::remove_file(&unused).unwrap();
        }
        // The copied pointer relocates to Copy's own declaration under ADR 0036:
        // Copied reference relocation, so only the missing donor leaves work.
        let result = wire(
            &run(&plan, &["--format", "json"]),
            if missing { 3 } else { 0 },
        );
        assert_eq!(result["cases"][0]["outcome"]["complete"], !missing);
        assert_eq!(result["cases"][1]["admission"], "admitted");
        assert_eq!(
            result["cases"][2]["admission"],
            if missing { "undetermined" } else { "admitted" }
        );
        assert_eq!(result["cases"][2]["state"], "report");
        assert_eq!(result["summary"]["request_errors"], 0);
        let catalogue = Catalogue::take_inputs(vec![
            SourceInput::file("device", &root).label(root.to_str().unwrap()),
            SourceInput::file("donor", &donor).label(donor.to_str().unwrap()),
            SourceInput::file("unused", &unused).label(unused.to_str().unwrap()),
            SourceInput::file("value", &sample)
                .label(sample.to_str().unwrap())
                .sample(),
        ])
        .unwrap();
        for (index, pointer) in [
            (1, "/sdfProperty/independent"),
            (2, "/sdfProperty/dependent"),
        ] {
            let native = catalogue
                .check_sample(
                    SampleRequest::new(
                        catalogue.source_id("device").unwrap(),
                        SampleKind::Property,
                        pointer,
                    )
                    .sample(catalogue.source_id("value").unwrap()),
                )
                .unwrap()
                .to_json();
            assert_eq!(
                result["cases"][index]["report"],
                serde_json::from_str::<Value>(&native).unwrap()
            );
        }
    }
}

#[test]
fn mixed_report_omission_retains_admission_request_errors_and_late_failures() {
    let files = Files::new();
    let rejected_name = format!("reject{}", "x".repeat(512));
    let rejected_pointer = format!("/sdfProperty/{rejected_name}");
    let mut qualities = serde_json::Map::new();
    qualities.insert("type".into(), json!("string"));
    for index in 0..1500 {
        qualities.insert(format!("unknown{index}{}", "x".repeat(100)), json!(true));
    }
    files.write("device.json", serde_json::to_vec(&json!({"info":{},"sdfData":{"D":qualities},"sdfProperty":{
        rejected_name:{"type":"array","items":{"type":"integer"}},"good":{"type":"array","items":{"type":"string"}}
    }})).unwrap());
    files.write(
        "value.json",
        serde_json::to_vec(&json!(vec!["wrong"; 1500])).unwrap(),
    );
    let mut cases = vec![json!({"name":"model","operation":"check","scope":{"kind":"catalogue"}})];
    cases.extend(
        (0..61).map(|i| sample_case(&format!("rejection {i}"), "property", &rejected_pointer)),
    );
    cases.extend([
        sample_case("late admission", "property", "/sdfProperty/good"),
        sample_case("late request error", "property", "/sdfProperty/absent"),
    ]);
    let plan = files.mixed(json!(cases));
    let output = run(&plan, &["--format", "json"]);
    let result = wire(&output, 2);
    assert!(output.stdout.len() <= 8 * 1024 * 1024);
    assert_eq!(result["cases"].as_array().unwrap().len(), 64);
    assert_eq!(
        result["cases"][0]["report"]["diagnostics"]["truncated"],
        true
    );
    assert!(result["summary"]["omitted_reports"].as_u64().unwrap() > 0);
    for (index, admission) in [(61, "rejected"), (62, "admitted")] {
        let case = &result["cases"][index];
        assert_eq!(case["admission"], admission);
        if index == 61 {
            assert_eq!(case["report_omission"]["reason"], "run-report-budget");
            assert!(case.get("report").is_none());
        }
        assert_eq!(case["request"]["assessment"], "esdieff.sample/1");
        assert!(case["baseline"].is_object());
        assert_eq!(case["policy"]["strict_warnings"], false);
    }
    assert_eq!(result["cases"][60]["outcome"]["disposition"], "failed");
    assert_eq!(result["cases"][60]["outcome"]["complete"], true);
    assert_eq!(result["cases"][63]["state"], "request_error");
    assert_eq!(result["summary"]["admitted"], 1);
    assert_eq!(result["summary"]["rejected"], 61);
    assert_eq!(result["summary"]["admission_undetermined"], 0);
    assert_eq!(result["summary"]["request_errors"], 1);
    assert_eq!(result["summary"]["complete_cases"], 63);
    assert_eq!(result["summary"]["incomplete_cases"], 1);
    assert_eq!(result["summary"]["total"], 64);
    // Human detail is smaller. Fill its allowance with ordinary model reports
    // before the final independent sample and request-error summaries.
    let mut human_cases: Vec<_> = (0..61).map(|i| json!({"name":format!("model {i}"),"operation":"check","scope":{"kind":"catalogue"}})).collect();
    human_cases.extend([
        sample_case("late rejection", "property", &rejected_pointer),
        sample_case("late admission", "property", "/sdfProperty/good"),
        sample_case("late request error", "property", "/sdfProperty/absent"),
    ]);
    let plan = files.mixed(json!(human_cases));
    let human = run(&plan, &[]);
    assert_eq!(human.status.code(), Some(2));
    assert!(human.stdout.len() <= 8 * 1024 * 1024);
    let human = String::from_utf8(human.stdout).unwrap();
    for admission in ["admitted", "rejected"] {
        assert!(human.contains(&format!("Admission: {admission}\n")));
    }
    assert!(human.contains("late request error"));
    assert!(human.contains("Native report omitted: run-report-budget"));
}

#[test]
fn sample_native_limits_and_unreadable_content_remain_domain_results() {
    let files = Files::new();
    let mut large = b"1".to_vec();
    large.resize(4 * 1024 * 1024 + 1, b' ');
    files.write(
        "device.json",
        br#"{"info":{},"sdfProperty":{"p":{"type":"integer"}}}"#,
    );
    files.write("value.json", &large);
    let plan = files.mixed(json!([
        sample_case("limited", "property", "/sdfProperty/p"),
        sample_case("limited again", "property", "/sdfProperty/p"),
        {"name":"independent model","operation":"check","scope":{"kind":"catalogue"}}
    ]));
    for missing in [false, true] {
        if missing {
            fs::remove_file(files.0.join("value.json")).unwrap();
        }
        let result = wire(&run(&plan, &["--format", "json"]), 3);
        assert_eq!(result["cases"][0]["admission"], "undetermined");
        assert_eq!(result["cases"][0]["state"], "report");
        assert_eq!(result["cases"][0]["outcome"], result["cases"][1]["outcome"]);
        assert_eq!(result["cases"][2]["outcome"]["disposition"], "passed");
        assert_eq!(result["summary"]["request_errors"], 0);
        assert_eq!(result["summary"]["admission_undetermined"], 2);
        assert_eq!(result["summary"]["complete_cases"], 1);
    }
}

#[test]
fn mixed_relative_and_symlink_paths_and_duplicate_file_aliases_retain_identity() {
    let files = Files::new();
    files.write(
        "device.json",
        br#"{"info":{},"sdfProperty":{"p":{"type":"integer"}}}"#,
    );
    files.write("value.json", b"1");
    let mut second = sample_case("second alias", "property", "/sdfProperty/p");
    second["document"] = json!("other model");
    second["sample"] = json!("other sample");
    let plan = files.write("plan.json", serde_json::to_vec(&json!({"schema":"esdieff.check-plan","version":"1","sources":[
        {"key":"device","path":"device.json","role":"model"}, {"key":"value","path":"value.json","role":"sample"},
        {"key":"other model","path":"device.json","role":"model"}, {"key":"other sample","path":"value.json","role":"sample"}
    ],"cases":[sample_case("first", "property", "/sdfProperty/p"),second,{"name":"models","operation":"check","scope":{"kind":"catalogue"}}]})).unwrap());
    let result = wire(&run(&plan, &["--format", "json"]), 0);
    let other_cwd = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir("/")
        .args(["check", "--plan"])
        .arg(&plan)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(result, wire(&other_cwd, 0));
    assert_eq!(result["cases"][0]["request"]["sample"], 2);
    assert_eq!(result["cases"][1]["request"]["sample"], 4);
    assert_eq!(result["cases"][1]["request"]["selector"]["document"], 3);
    assert_eq!(
        result["cases"][2]["request"]["scope"]["assessed"],
        json!([1, 3])
    );
    assert_eq!(result["sources"].as_array().unwrap().len(), 4);
    #[cfg(unix)]
    {
        let target = files.0.join("target");
        fs::create_dir(&target).unwrap();
        fs::copy(&plan, target.join("plan.json")).unwrap();
        fs::write(target.join("value.json"), b"false").unwrap();
        std::os::unix::fs::symlink(target.join("plan.json"), files.0.join("link.json")).unwrap();
        let linked = wire(&run(&files.0.join("link.json"), &["--format", "json"]), 0);
        assert_eq!(linked["sources"], result["sources"]);
        assert_eq!(linked["cases"], result["cases"]);
    }
}

#[cfg(unix)]
#[test]
fn mixed_cases_reuse_a_single_acquisition_after_a_sample_path_disappears() {
    use std::io::Write;
    let files = Files::new();
    files.write(
        "device.json",
        br#"{"info":{},"sdfProperty":{"p":{"type":"integer"}}}"#,
    );
    let fifo = files.0.join("value.json");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let writer = std::thread::spawn(move || {
        let mut stream = fs::OpenOptions::new().write(true).open(&fifo).unwrap();
        fs::remove_file(fifo).unwrap();
        stream.write_all(b"1").unwrap();
    });
    let plan = files.mixed(json!([
        sample_case("first", "property", "/sdfProperty/p"),
        {"name":"model","operation":"check","scope":{"kind":"catalogue"}},
        sample_case("again", "property", "/sdfProperty/p")
    ]));
    let result = wire(&run(&plan, &["--format", "json"]), 0);
    writer.join().unwrap();
    assert!(!files.0.join("value.json").exists());
    assert_eq!(result["sources"][1]["acquisition"], "acquired");
    assert_eq!(result["cases"][0]["report"], result["cases"][2]["report"]);
    assert_eq!(result["summary"]["admitted"], 2);
}

#[test]
fn every_profile_strict_policy_and_case_order_agree_with_direct_checks() {
    let files = Files::new();
    let model = files.write("device.json", b"{}");
    let profiles = [
        "initial-v1",
        "expanded-modified-v1",
        "expanded-units-v1",
        "expanded-content-format-v1",
        "expanded-pattern-syntax-v1",
        "expanded-numeric-v1",
        "expanded-consistency-v1",
        "full-baseline",
    ];
    let mut cases: Vec<Value> = profiles.iter().map(|profile| json!({"name":profile, "operation":"check", "scope":{"kind":"catalogue"}, "assessment":profile})).collect();
    cases.push(json!({"name":"strict", "operation":"check", "scope":{"kind":"catalogue"}, "strict_warnings":true}));
    cases.push(json!({"name":"after failure", "operation":"check", "scope":{"kind":"catalogue"}}));
    let plan = files.plan(json!(cases));
    let result = wire(&run(&plan, &["--format", "json", "--color", "always"]), 1);
    for (index, profile) in profiles.iter().enumerate() {
        let direct = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--assessment", profile, "--format", "json"])
            .arg(&model)
            .output()
            .unwrap();
        assert_eq!(result["cases"][index]["name"], *profile);
        let native: Value = serde_json::from_slice(&direct.stdout).unwrap();
        assert_native_report(&result["cases"][index]["report"], &native, profile);
        assert_eq!(result["cases"][index]["outcome"], native["outcome"]);
    }
    assert_eq!(result["cases"][8]["outcome"]["disposition"], "failed");
    assert_eq!(result["cases"][8]["outcome"]["complete"], true);
    assert_eq!(result["cases"][9]["outcome"]["disposition"], "passed");
    assert_eq!(result["summary"]["total"], 10);
    assert_eq!(result["summary"]["failed"], 1);
    assert_eq!(result["summary"]["request_errors"], 0);
    assert_eq!(result["summary"]["admitted"], 0);
}

#[test]
fn selected_roots_are_explicit_and_unneeded_failed_sources_do_not_spoil_them() {
    let files = Files::new();
    files.write(
        "one.json",
        br#"{"info":{},"sdfData":{"A":{"type":"integer"}}}"#,
    );
    files.write(
        "two.json",
        br#"{"info":{},"sdfData":{"B":{"type":"string"}}}"#,
    );
    files.write("sample.json", b"null");
    let plan = files.write("plan.json", serde_json::to_vec(&json!({"schema":"esdieff.check-plan", "version":"1", "sources":[
        {"key":"one","path":"one.json","role":"model"}, {"key":"two","path":"two.json","role":"model"}, {"key":"unused","path":"missing.json","role":"model"}, {"key":"sample","path":"sample.json","role":"sample"}], "cases":[
        {"name":"all models","operation":"check","scope":{"kind":"catalogue"}},
        {"name":"two roots","operation":"check","scope":{"kind":"selected","roots":["two","one"]}},
        {"name":"one root","operation":"check","scope":{"kind":"selected","roots":["one"]}}
    ]})).unwrap());
    let result = wire(&run(&plan, &["--format", "json"]), 3);
    assert_eq!(result["cases"][0]["outcome"]["disposition"], "undetermined");
    assert_eq!(
        result["cases"][0]["request"]["scope"]["assessed"],
        json!([1, 2, 3])
    );
    assert_eq!(
        result["cases"][1]["request"]["scope"]["roots"],
        json!([2, 1])
    );
    assert_eq!(result["cases"][1]["outcome"]["disposition"], "passed");
    assert_eq!(result["cases"][2]["outcome"]["disposition"], "passed");
    assert_eq!(result["sources"][2]["acquisition"], "failed");
    assert_eq!(result["sources"][3]["role"], "sample");
    assert_eq!(result["summary"]["complete"], false);
    assert_eq!(result["summary"]["complete_cases"], 2);
}

#[test]
fn malformed_configuration_is_rejected_without_a_run() {
    let files = Files::new();
    let valid = json!({"schema":"esdieff.check-plan","version":"1","sources":[{"key":"m","path":"missing","role":"model"}],"cases":[{"name":"c","operation":"check","scope":{"kind":"catalogue"}}]});
    let mut bad = vec![
        b"{".to_vec(),
        vec![b' '; 1024 * 1024 + 1],
        br#"{"schema":"esdieff.check-plan","version":"1","sources":[],"sources":[],"cases":[]}"#
            .to_vec(),
    ];
    for (pointer, value) in [
        ("/version", json!("2")),
        ("/sources", json!([])),
        ("/cases", json!([])),
        ("/sources/0/key", json!("")),
        ("/sources/0/key", json!("x".repeat(257))),
        ("/sources/0/path", json!("")),
        ("/sources/0/role", json!("other")),
        ("/cases/0/name", json!("")),
        ("/cases/0/name", json!("x".repeat(257))),
        ("/cases/0/operation", json!("sample")),
        (
            "/cases/0/scope",
            json!({"kind":"selected","roots":["missing"]}),
        ),
        (
            "/cases/0/scope",
            json!({"kind":"selected","roots":["m","m"]}),
        ),
        ("/cases/0/scope", json!({"kind":"selected","roots":[]})),
    ] {
        let mut plan = valid.clone();
        *plan.pointer_mut(pointer).unwrap() = value;
        bad.push(serde_json::to_vec(&plan).unwrap());
    }
    for (pointer, member, value) in [
        ("", "extra", json!(true)),
        ("/sources/0", "extra", json!(true)),
        ("/cases/0", "assessment", json!("unknown")),
        ("/cases/0", "strict_warnings", json!("yes")),
        ("/cases/0/scope", "roots", json!(["m"])),
    ] {
        let mut plan = valid.clone();
        plan.pointer_mut(pointer).unwrap()[member] = value;
        bad.push(serde_json::to_vec(&plan).unwrap());
    }
    for (pointer, count) in [("/sources", 257), ("/cases", 65)] {
        let mut plan = valid.clone();
        let first = plan.pointer(pointer).unwrap()[0].clone();
        *plan.pointer_mut(pointer).unwrap() = json!(vec![first; count]);
        bad.push(serde_json::to_vec(&plan).unwrap());
    }
    for pointer in ["/sources", "/cases"] {
        let mut plan = valid.clone();
        let first = plan.pointer(pointer).unwrap()[0].clone();
        plan.pointer_mut(pointer)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(first);
        bad.push(serde_json::to_vec(&plan).unwrap());
    }
    let escaped = serde_json::to_string(&valid)
        .unwrap()
        .replace("\"key\":\"m\"", "\"key\":\"m\",\"\\u006bey\":\"m\"");
    bad.push(escaped.into_bytes());
    for (index, bytes) in bad.iter().enumerate() {
        let path = files.write("bad.json", bytes);
        let output = run(&path, &["--format", "json"]);
        assert_eq!(output.status.code(), Some(2), "invalid plan {index}");
        assert!(output.stdout.is_empty(), "invalid plan {index}");
        assert!(!output.stderr.is_empty());
    }
    assert_eq!(run(&files.0.join("absent"), &[]).status.code(), Some(2));
}

#[test]
fn relative_paths_and_symlink_plan_use_the_supplied_parent() {
    let files = Files::new();
    files.write("device.json", b"{}");
    let plan =
        files.plan(json!([{"name":"relative","operation":"check","scope":{"kind":"catalogue"}}]));
    let from_other = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir("/")
        .args(["check", "--plan"])
        .arg(&plan)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let first = wire(&from_other, 0);
    let from_parent = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir(&files.0)
        .args(["check", "--plan", "plan.json", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(first, wire(&from_parent, 0));
    #[cfg(unix)]
    {
        let target = files.0.join("target");
        fs::create_dir(&target).unwrap();
        fs::copy(&plan, target.join("plan.json")).unwrap();
        fs::write(target.join("device.json"), b"{").unwrap();
        std::os::unix::fs::symlink(target.join("plan.json"), files.0.join("link.json")).unwrap();
        let linked = wire(&run(&files.0.join("link.json"), &["--format", "json"]), 0);
        assert_eq!(linked["sources"], first["sources"]);
        assert_eq!(linked["cases"], first["cases"]);
    }
}

#[test]
fn literal_paths_distinct_keys_and_decoded_identity_rules_are_preserved() {
    let files = Files::new();
    // NTFS forbids '*', while bracket wildcards remain legal literal filenames.
    let glob = if cfg!(windows) {
        "[literal].json"
    } else {
        "*.json"
    };
    for path in ["-", "$HOME", glob] {
        files.write(path, b"{}");
    }
    let mut plan = json!({"schema":"esdieff.check-plan","version":"1","sources":[
        {"key":"dash","path":"-","role":"model"}, {"key":"env","path":"$HOME","role":"model"}, {"key":"glob","path":glob,"role":"model"}, {"key":"same bytes","path":"-","role":"model"}],"cases":[
        {"name":"repeat","operation":"check","scope":{"kind":"selected","roots":["dash"]}}, {"name":"again","operation":"check","scope":{"kind":"selected","roots":["dash"]}}, {"name":"distinct","operation":"check","scope":{"kind":"selected","roots":["same bytes"]}}
    ]});
    let path = files.write("plan.json", serde_json::to_vec(&plan).unwrap());
    let result = wire(&run(&path, &["--format", "json"]), 0);
    assert_eq!(result["sources"].as_array().unwrap().len(), 4);
    for (index, key) in ["dash", "env", "glob", "same bytes"].iter().enumerate() {
        assert_eq!(result["sources"][index]["id"], index + 1);
        assert_eq!(result["sources"][index]["key"], *key);
        assert_eq!(result["sources"][index]["acquisition"], "acquired");
    }
    assert_eq!(result["cases"][0]["request"], result["cases"][1]["request"]);
    assert_eq!(result["cases"][2]["request"]["scope"]["roots"], json!([4]));
    plan["sources"][0]["role"] = json!("sample");
    let path = files.write("plan.json", serde_json::to_vec(&plan).unwrap());
    let bad = run(&path, &[]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(bad.stdout.is_empty());
    for (array, key) in [("sources", "key"), ("cases", "name")] {
        let bytes = format!(
            "{{\"schema\":\"esdieff.check-plan\",\"version\":\"1\",\"{array}\":[{{\"{key}\":\"a\"}},{{\"{key}\":\"\\u0061\"}}]}}"
        );
        let path = files.write("bad.json", bytes);
        assert_eq!(run(&path, &[]).status.code(), Some(2));
    }
}

#[test]
fn plan_mode_rejects_direct_inputs_and_overrides_in_either_order() {
    let files = Files::new();
    files.write("device.json", b"{}");
    let plan = files.plan(json!([{"name":"c","operation":"check","scope":{"kind":"catalogue"}}]));
    for flags in [
        vec!["device.json"],
        vec!["--strict-warnings"],
        vec!["--assessment", "initial-v1"],
        vec!["--registry-knowledge", "recorded"],
        vec!["--root", "device.json"],
    ] {
        let output = run(&plan, &flags);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .arg("check")
            .args(&flags)
            .arg("--plan")
            .arg(&plan)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    for command in ["inspect", "sample"] {
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args([command, "--plan"])
            .arg(&plan)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    let output = run(&plan, &["--plan", plan.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    for mode in ["always", "never", "auto"] {
        let output = run(&plan, &["--color", mode, "--format", "json"]);
        wire(&output, 0);
        assert!(!output.stdout.contains(&0x1b));
        let output = run(&plan, &["--color", mode]);
        assert_eq!(output.status.code(), Some(0));
        if mode == "always" {
            assert!(output.stdout.contains(&0x1b));
        } else {
            assert!(!output.stdout.contains(&0x1b));
        }
    }
}

#[test]
fn failed_complete_and_incomplete_cases_continue_with_native_exit_precedence() {
    let files = Files::new();
    files.write("bad.json", br#"{"info":{},"sdfData":{"D":{"type":42}}}"#);
    files.write(
        "incomplete.json",
        br#"{"info":{"features":["not-supported"]},"sdfData":{"D":{"type":42}}}"#,
    );
    files.write("good.json", br#"{"info":{},"sdfData":{"D":{}}}"#);
    let plan = json!({"schema":"esdieff.check-plan","version":"1","sources":[
        {"key":"bad","path":"bad.json","role":"model"},{"key":"incomplete","path":"incomplete.json","role":"model"},{"key":"good","path":"good.json","role":"model"}],"cases":[
        {"name":"bad","operation":"check","scope":{"kind":"selected","roots":["bad"]}}, {"name":"incomplete","operation":"check","scope":{"kind":"selected","roots":["bad","incomplete"]}}, {"name":"good","operation":"check","scope":{"kind":"selected","roots":["good"]}}
    ]});
    let path = files.write("plan.json", serde_json::to_vec(&plan).unwrap());
    let result = wire(&run(&path, &["--format", "json"]), 1);
    assert_eq!(result["cases"][0]["outcome"]["complete"], true);
    assert_eq!(result["cases"][0]["outcome"]["disposition"], "failed");
    assert_eq!(result["cases"][1]["outcome"]["complete"], false);
    assert_eq!(result["cases"][1]["outcome"]["disposition"], "failed");
    assert_eq!(result["cases"][2]["outcome"]["disposition"], "passed");
    assert_eq!(result["summary"]["failed"], 2);
    assert_eq!(result["summary"]["complete_cases"], 2);
    assert_eq!(result["summary"]["incomplete_cases"], 1);
    assert_eq!(result["summary"]["complete"], false);
}

#[test]
fn native_truncation_and_outer_omission_keep_later_failures_and_all_case_counts() {
    let files = Files::new();
    let mut qualities = serde_json::Map::new();
    qualities.insert("type".into(), json!("string"));
    for index in 0..1500 {
        qualities.insert(format!("unknown{index}{}", "x".repeat(100)), json!(true));
    }
    let bytes = serde_json::to_vec(&json!({"info":{},"sdfData":{"D":qualities}})).unwrap();
    let model = files.write("device.json", &bytes);
    let cases:Vec<Value>=(0..64).map(|index| json!({"name":format!("case {index}"),"operation":"check","scope":{"kind":"catalogue"},"strict_warnings":index==63})).collect();
    let plan = files.plan(json!(cases));
    let output = run(&plan, &["--format", "json"]);
    let result = wire(&output, 1);
    assert!(output.stdout.len() <= 8 * 1024 * 1024);
    assert_eq!(result["cases"].as_array().unwrap().len(), 64);
    assert_eq!(result["summary"]["total"], 64);
    assert_eq!(result["summary"]["passed"], 63);
    assert_eq!(result["summary"]["failed"], 1);
    assert_eq!(result["summary"]["complete"], true);
    assert_eq!(
        result["cases"][0]["report"]["diagnostics"]["truncated"],
        true
    );
    assert!(result["summary"]["omitted_reports"].as_u64().unwrap() > 0);
    assert_eq!(
        result["cases"][63]["report_omission"]["reason"],
        "run-report-budget"
    );
    assert!(result["cases"][63].get("report").is_none());
    let direct = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--strict-warnings", "--format", "json"])
        .arg(model)
        .output()
        .unwrap();
    let direct = wire(&direct, 1);
    for field in ["request", "outcome", "baseline", "policy"] {
        assert_eq!(result["cases"][63][field], direct[field], "omitted {field}");
    }
    assert_eq!(fs::read(files.0.join("device.json")).unwrap(), bytes);
    let human = run(&plan, &[]);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stdout.len() <= 8 * 1024 * 1024);
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("case 63"));
    assert!(human.contains("Native report omitted: run-report-budget"));
}

#[test]
fn native_processing_limits_remain_incomplete_and_are_independent_per_case() {
    let files = Files::new();
    let mut bytes = b"{}".to_vec();
    bytes.resize(4 * 1024 * 1024 + 1, b' ');
    files.write("large.json", &bytes);
    files.write("device.json", br#"{"info":{}}"#);
    let plan = json!({"schema":"esdieff.check-plan","version":"1","sources":[{"key":"large","path":"large.json","role":"model"},{"key":"good","path":"device.json","role":"model"}],"cases":[
        {"name":"limited","operation":"check","scope":{"kind":"selected","roots":["large"]}},
        {"name":"limited again","operation":"check","scope":{"kind":"selected","roots":["large"]}},
        {"name":"independent","operation":"check","scope":{"kind":"selected","roots":["good"]}}]});
    let path = files.write("plan.json", serde_json::to_vec(&plan).unwrap());
    let result = wire(&run(&path, &["--format", "json"]), 3);
    assert_eq!(result["sources"][0]["retained_bytes"], 4 * 1024 * 1024);
    assert_eq!(result["sources"][0]["content_complete"], false);
    assert_eq!(result["cases"][0]["outcome"]["complete"], false);
    assert_eq!(result["cases"][0]["outcome"], result["cases"][1]["outcome"]);
    assert_eq!(result["cases"][2]["outcome"]["complete"], true);
    assert_eq!(result["summary"]["undetermined"], 2);
    assert_eq!(result["summary"]["passed"], 1);
}

#[test]
fn native_plan_depth_nodes_and_metadata_are_rejected_as_configuration() {
    let files = Files::new();
    let valid = json!({"schema":"esdieff.check-plan","version":"1","sources":[{"key":"m","path":"missing","role":"model"}],"cases":[{"name":"c","operation":"check","scope":{"kind":"catalogue"}}]});
    let base = serde_json::to_string(&valid).unwrap();
    for extra in [
        format!("{}0{}", "[".repeat(66), "]".repeat(66)),
        format!("[{}0]", "0,".repeat(100001)),
    ] {
        let mut bytes = base.clone();
        bytes.pop();
        bytes.push_str(&format!(",\"extra\":{extra}}}"));
        let path = files.write("plan.json", bytes);
        let output = run(&path, &[]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let sources: Vec<Value> = (0..256)
        .map(|i| json!({"key":format!("m{i}"),"path":"x".repeat(2500),"role":"model"}))
        .collect();
    let mut plan = valid;
    plan["sources"] = json!(sources);
    let bytes = serde_json::to_vec(&plan).unwrap();
    assert!(bytes.len() < 1024 * 1024);
    let path = files.write("plan.json", bytes);
    let output = run(&path, &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("metadata")
    );
}

#[test]
fn mandatory_envelope_is_rejected_before_acquisition() {
    let files = Files::new();
    // This plan's control metadata is 257 bytes per byte of the temporary
    // directory plus 512 per byte of each source path, plus 344704. Only the band
    // from 1045846 bytes up to the 1 MiB plan budget fits the plan but not the
    // report envelope, so aim at its middle whatever the directory length.
    let path = (702_507 - 257 * files.0.as_os_str().len()) / 512;
    let sources:Vec<Value>=(0..256).map(|i|json!({"key":format!("source{i:03}{}","k".repeat(23)),"path":"x".repeat(path),"role":"model"})).collect();
    // Repeated selected references are part of control metadata even though
    // acquisition is shared. Choose a bounded plan close to the envelope limit.
    let cases:Vec<Value>=(0..64).map(|i|json!({"name":format!("case{i}"),"operation":"check","scope":{"kind":"selected","roots":sources.iter().take(100).map(|s|s["key"].clone()).collect::<Vec<_>>()}})).collect();
    let bytes = serde_json::to_vec(
        &json!({"schema":"esdieff.check-plan","version":"1","sources":sources,"cases":cases}),
    )
    .unwrap();
    assert!(bytes.len() < 1024 * 1024);
    let path = files.write("plan.json", bytes);
    let output = run(&path, &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("mandatory envelope")
    );
}

#[test]
fn selected_dependencies_and_unavailable_contributors_match_native_requests() {
    let files = Files::new();
    let root=files.write("root.json",br##"{"info":{},"namespace":{"n":"https://example.test/n"},"sdfObject":{"O":{"sdfProperty":{"p":{"sdfRef":"n:#/sdfData/D"}}}}}"##);
    let donor=files.write("donor.json",br##"{"info":{},"namespace":{"n":"https://example.test/n"},"defaultNamespace":"n","sdfData":{"D":{"type":"number"}}}"##);
    let mut plan = json!({"schema":"esdieff.check-plan","version":"1","sources":[{"key":"root","path":"root.json","role":"model"},{"key":"donor","path":"donor.json","role":"model"}],"cases":[{"name":"product","operation":"check","scope":{"kind":"selected","roots":["root"]}}]});
    for missing in [false, true] {
        if missing {
            plan["sources"][1]["path"] = json!("missing.json");
        }
        let path = files.write("plan.json", serde_json::to_vec(&plan).unwrap());
        let result = wire(
            &run(&path, &["--format", "json"]),
            if missing { 3 } else { 0 },
        );
        let candidate = if missing {
            files.0.join("missing.json")
        } else {
            donor.clone()
        };
        let direct = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--root"])
            .arg(&root)
            .args(["--format", "json"])
            .arg(&root)
            .arg(candidate)
            .output()
            .unwrap();
        let direct: Value = serde_json::from_slice(&direct.stdout).unwrap();
        assert_native_report(
            &result["cases"][0]["report"],
            &direct,
            "selected dependencies",
        );
        if !missing {
            assert_eq!(
                result["cases"][0]["request"]["scope"]["dependencies"],
                json!([2])
            );
        }
    }
}

#[test]
fn raw_native_detail_preserves_large_exact_numerical_tokens() {
    let files = Files::new();
    let model=files.write("device.json",br#"{"info":{},"sdfData":{"D":{"type":"number","nullable":false,"minimum":10e1000000,"maximum":9007199254740993}}}"#);
    let plan =
        files.plan(json!([{"name":"exact","operation":"check","scope":{"kind":"catalogue"}}]));
    let output = run(&plan, &["--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("10e1000000"));
    assert!(text.contains("9007199254740993"));
    let outer: std::collections::BTreeMap<&str, &serde_json::value::RawValue> =
        serde_json::from_str(&text).unwrap();
    let cases: Vec<std::collections::BTreeMap<&str, &serde_json::value::RawValue>> =
        serde_json::from_str(outer["cases"].get()).unwrap();
    let native = cases[0]["report"].get();
    let direct = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "json"])
        .arg(model)
        .output()
        .unwrap();
    // Compare the whole native serialization, changing only the deliberately
    // caller-chosen key. Paths, attempt IDs and all diagnostics remain present.
    let direct = String::from_utf8(direct.stdout)
        .unwrap()
        .replace("\"key\":\"input1\"", "\"key\":\"device\"");
    assert_eq!(native, direct.trim_end());
}

#[cfg(unix)]
#[test]
fn output_failure_has_exit_four_without_a_success_claim() {
    use std::{os::fd::OwnedFd, os::unix::net::UnixStream, process::Stdio};
    let files = Files::new();
    files.write(
        "device.json",
        br#"{"info":{},"sdfProperty":{"p":{"type":"integer"}}}"#,
    );
    files.write("value.json", b"1");
    let plan = files.mixed(json!([
        sample_case("sample", "property", "/sdfProperty/p"),
        sample_case("request error", "property", "/sdfProperty/absent"),
        {"name":"model","operation":"check","scope":{"kind":"catalogue"}}
    ]));
    for format in ["human", "json"] {
        let (reader, writer) = UnixStream::pair().unwrap();
        drop(reader);
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--plan"])
            .arg(&plan)
            .args(["--format", format])
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(4));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("cannot emit requested run report")
        );
    }
}

#[test]
fn decoded_duplicate_identities_and_unavailable_json_text_are_configuration_errors() {
    let files = Files::new();
    let duplicate_keys = r#"{"schema":"esdieff.check-plan","version":"1","sources":[{"key":"m","path":"missing","role":"model"},{"key":"\u006d","path":"missing","role":"model"}],"cases":[{"name":"c","operation":"check","scope":{"kind":"catalogue"}}]}"#;
    let duplicate_names = r#"{"schema":"esdieff.check-plan","version":"1","sources":[{"key":"m","path":"missing","role":"model"}],"cases":[{"name":"c","operation":"check","scope":{"kind":"catalogue"}},{"name":"\u0063","operation":"check","scope":{"kind":"catalogue"}}]}"#;
    let unavailable = r#"{"schema":"esdieff.check-plan","version":"1","sources":[{"key":"m","path":"\uD800","role":"model"}],"cases":[{"name":"c","operation":"check","scope":{"kind":"catalogue"}}]}"#;
    for bytes in [duplicate_keys, duplicate_names, unavailable] {
        let path = files.write("plan.json", bytes);
        let output = run(&path, &[]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn reference_uncertainty_is_not_hidden_by_repetition() {
    let files = Files::new();
    // A Boolean `sdfRef` has no processing meaning, so its import stays undetermined.
    files.write(
        "device.json",
        br##"{"info":{},"sdfObject":{"Example":{"sdfData":{"BooleanRef":{"sdfRef":true}}}}}"##,
    );
    let plan = files.plan(json!([
        {"name":"unresolved reference","operation":"check","scope":{"kind":"catalogue"}},
        {"name":"same request","operation":"check","scope":{"kind":"catalogue"}}
    ]));
    let result = wire(&run(&plan, &["--format", "json"]), 3);
    assert_eq!(result["cases"][0]["outcome"]["complete"], false);
    assert_eq!(result["cases"][0]["outcome"], result["cases"][1]["outcome"]);
    assert_eq!(result["summary"]["undetermined"], 2);
    assert_eq!(result["summary"]["complete"], false);
}

const MINIMAL_LOCK: &str = include_str!("../examples/door-lock-check/minimal.sdf.json");
const RICH_LOCK: &str = include_str!("../examples/door-lock-check/rich.sdf.json");

fn diff_case(name: &str, earlier: Value, later: Value) -> Value {
    json!({"name":name,"operation":"diff","earlier":earlier,"later":later})
}
fn direct_diff(earlier: &Path, later: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["diff", "--old"])
        .arg(earlier)
        .arg("--new")
        .arg(later)
        .args(extra)
        .output()
        .unwrap()
}
/// Write `acknowledged.json` with every entry a direct comparison's JSON report offers, and
/// return its path and entry count.
fn acknowledge_all(files: &Files, report: &Value) -> (PathBuf, usize) {
    let entries: Vec<_> = report["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|difference| difference.get("acknowledgement").cloned())
        .collect();
    let file =
        json!({"schema":"esdieff.acknowledged-differences","version":"1","differences":entries});
    let path = files.write("acknowledged.json", serde_json::to_vec(&file).unwrap());
    (path, entries.len())
}

#[test]
fn a_check_case_and_a_revision_comparison_share_one_run_report() {
    let files = Files::new();
    files.write("device.json", RICH_LOCK);
    let earlier = files.write("minimal.sdf.json", MINIMAL_LOCK);
    let later = files.write("rich.sdf.json", RICH_LOCK);
    let plan = files.plan(json!([
        {"name":"model","operation":"check","scope":{"kind":"catalogue"}},
        diff_case("minimal to rich", json!({"document":"minimal.sdf.json"}), json!({"document":"rich.sdf.json"}))
    ]));
    let result = wire(&run(&plan, &["--format", "json"]), 1);
    assert_eq!(result["schema"], "esdieff.check-run");
    assert_eq!(result["version"], "1.1");
    let direct = wire(&direct_diff(&earlier, &later, &["--format", "json"]), 1);
    assert_eq!(direct["result"]["outcome"], "breaking");
    let case = &result["cases"][1];
    assert_eq!(
        keys(case),
        [
            "baseline",
            "classification",
            "name",
            "operation",
            "outcome",
            "policy",
            "report",
            "request",
            "state"
        ]
    );
    assert_eq!(case["name"], "minimal to rich");
    assert_eq!(case["operation"], "diff");
    assert_eq!(case["state"], "report");
    assert_eq!(case["classification"], "breaking");
    assert_eq!(case["report"], direct);
    for field in ["request", "outcome", "baseline", "policy"] {
        assert_eq!(case[field], direct[field], "{field}");
    }
    assert_eq!(result["cases"][0]["outcome"]["disposition"], "passed");
    assert_eq!(result["summary"]["passed"], 1);
    assert_eq!(result["summary"]["failed"], 1);
    assert_eq!(result["summary"]["complete_cases"], 2);
    assert_eq!(result["summary"]["verdict"], "failed");
    // The comparison's documents are its own acquisition attempts, not plan sources.
    assert_eq!(result["sources"].as_array().unwrap().len(), 1);
    assert_eq!(fs::read(earlier).unwrap(), MINIMAL_LOCK.as_bytes());
}

#[test]
fn a_diff_case_keeps_the_version_warning_of_its_native_report() {
    let files = Files::new();
    files.write("device.json", RICH_LOCK);
    let switch = |states: &[&str]| {
        json!({"info": {"version": "2019-11-14"},
            "sdfObject": {"switch.fault": {"sdfProperty": {"state": {"type": "string", "enum": states}}}}})
        .to_string()
    };
    let earlier = files.write("earlier.sdf.json", switch(&["on", "off", "faulted"]));
    let later = files.write("later.sdf.json", switch(&["on", "off"]));
    let case = diff_case(
        "switch",
        json!({"switch.sdf.json":"earlier.sdf.json"}),
        json!({"switch.sdf.json":"later.sdf.json"}),
    );
    let plan = files.plan(json!([case]));
    let result = wire(&run(&plan, &["--format", "json"]), 1);
    let direct = wire(&direct_diff(&earlier, &later, &["--format", "json"]), 1);
    let case = &result["cases"][0];
    assert_eq!(case["outcome"]["warnings"], 1);
    assert_eq!(case["outcome"]["disposition"], "failed");
    assert_eq!(case["outcome"], direct["outcome"]);
    assert_eq!(
        case["report"]["diagnostics"]["items"][0]["code"],
        "comparison.version_not_increased"
    );
    let human = run(&plan, &[]);
    assert_eq!(human.status.code(), Some(1));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(
        human.contains("comparison.version_not_increased"),
        "{human}"
    );
}

#[test]
fn acknowledgements_from_direct_diff_pass_a_breaking_case_with_ordinary_precedence() {
    let files = Files::new();
    for directory in ["releases", "releases/1", "current"] {
        fs::create_dir(files.0.join(directory)).unwrap();
    }
    files.write("releases/1/lock.sdf.json", MINIMAL_LOCK);
    files.write(
        "releases/1/retired.sdf.json",
        br#"{"info":{},"sdfObject":{"Retired":{}}}"#,
    );
    files.write("current/lock.sdf.json", RICH_LOCK);
    files.write(
        "current/added.sdf.json",
        br#"{"info":{},"sdfObject":{"Added":{}}}"#,
    );
    let (old, new) = (files.0.join("releases/1"), files.0.join("current"));
    let direct = wire(&direct_diff(&old, &new, &["--format", "json"]), 1);
    let (acknowledged, entries) = acknowledge_all(&files, &direct);
    assert!(entries >= 3);
    let flag = ["--acknowledged", acknowledged.to_str().unwrap()];
    let direct = wire(
        &direct_diff(&old, &new, &[&flag[..], &["--format", "json"]].concat()),
        0,
    );
    // Keys name each document's path within its revision, as direct directory comparison does.
    let earlier = json!({
        "lock.sdf.json":Path::new("releases/1").join("lock.sdf.json"),
        "retired.sdf.json":Path::new("releases/1").join("retired.sdf.json")
    });
    let later = json!({
        "lock.sdf.json":Path::new("current").join("lock.sdf.json"),
        "added.sdf.json":Path::new("current").join("added.sdf.json")
    });
    let open = diff_case("open", earlier.clone(), later.clone());
    let mut intended = diff_case("intended", earlier, later);
    intended["acknowledged"] = json!("acknowledged.json");
    files.write("device.json", RICH_LOCK);
    let result = wire(
        &run(&files.plan(json!([intended])), &["--format", "json"]),
        0,
    );
    let case = &result["cases"][0];
    assert_eq!(case["classification"], "breaking");
    assert_eq!(case["outcome"]["disposition"], "passed");
    assert_eq!(case["policy"]["acknowledgements"], entries);
    assert_eq!(case["report"], direct);
    let documents = &case["report"]["result"]["summary"]["documents"];
    assert_eq!(*documents, json!({"compared":1,"removed":1,"added":1}));
    assert_eq!(result["summary"]["verdict"], "passed");
    // A Boolean `sdfRef` leaves the model check undetermined.
    files.write(
        "device.json",
        br##"{"info":{},"sdfObject":{"Example":{"sdfData":{"BooleanRef":{"sdfRef":true}}}}}"##,
    );
    let undetermined = json!({"name":"model","operation":"check","scope":{"kind":"catalogue"}});
    let plan = files.plan(json!([open, intended, undetermined]));
    let result = wire(&run(&plan, &["--format", "json"]), 1);
    assert_eq!(result["cases"][0]["classification"], "breaking");
    assert_eq!(result["cases"][0]["outcome"]["disposition"], "failed");
    assert_eq!(result["cases"][1]["outcome"]["disposition"], "passed");
    assert_eq!(result["summary"]["failed"], 1);
    assert_eq!(result["summary"]["passed"], 1);
    assert_eq!(result["summary"]["undetermined"], 1);
    assert_eq!(result["summary"]["complete_cases"], 2);
    let plan = files.plan(json!([intended, undetermined]));
    let result = wire(&run(&plan, &["--format", "json"]), 3);
    assert_eq!(result["summary"]["verdict"], "undetermined");
    // A request error decides the run before a failed comparison.
    files.write("value.json", b"1");
    let mistake = sample_case("mistake", "property", "/sdfProperty/absent");
    let result = wire(
        &run(&files.mixed(json!([open, mistake])), &["--format", "json"]),
        2,
    );
    assert_eq!(result["cases"][0]["outcome"]["disposition"], "failed");
    assert_eq!(result["summary"]["request_errors"], 1);
    assert_eq!(result["summary"]["verdict"], "invalid");
}

#[test]
fn an_acknowledged_undetermined_comparison_is_a_complete_passed_case() {
    let files = Files::new();
    files.write("device.json", MINIMAL_LOCK);
    // An added unit changes how values read, so the comparison leaves it undetermined.
    let earlier = files.write(
        "earlier.json",
        br#"{"info":{},"sdfObject":{"O":{"sdfProperty":{"p":{"type":"number"}}}}}"#,
    );
    let later = files.write(
        "later.json",
        br#"{"info":{},"sdfObject":{"O":{"sdfProperty":{"p":{"type":"number","unit":"Cel"}}}}}"#,
    );
    let direct = wire(&direct_diff(&earlier, &later, &["--format", "json"]), 3);
    assert_eq!(
        direct["result"]["differences"][0]["reason"]["code"],
        "comparison.unit_added"
    );
    assert_eq!(acknowledge_all(&files, &direct).1, 1);
    let sides = (
        json!({"document":"earlier.json"}),
        json!({"document":"later.json"}),
    );
    let mut reviewed = diff_case("reviewed", sides.0.clone(), sides.1.clone());
    reviewed["acknowledged"] = json!("acknowledged.json");
    let plan = files.plan(json!([diff_case("open", sides.0, sides.1), reviewed]));
    let result = wire(&run(&plan, &["--format", "json"]), 3);
    for (index, disposition, complete) in [(0, "undetermined", false), (1, "passed", true)] {
        let case = &result["cases"][index];
        assert_eq!(case["classification"], "undetermined");
        assert_eq!(case["outcome"]["disposition"], disposition);
        assert_eq!(case["outcome"]["complete"], complete);
    }
    assert_eq!(result["summary"]["complete_cases"], 1);
    assert_eq!(result["summary"]["incomplete_cases"], 1);
}

fn namespaced(title: &str) -> String {
    format!(
        r##"{{"info":{{"title":"{title}"}},"namespace":{{"n":"https://example.test/n"}},"defaultNamespace":"n","sdfData":{{"D":{{"type":"integer"}}}},"sdfObject":{{"O":{{"sdfProperty":{{"p":{{"sdfRef":"n:#/sdfData/D"}}}}}}}}}}"##
    )
}

#[test]
fn earlier_documents_stay_out_of_check_cases_and_a_missing_one_is_a_failed_acquisition() {
    let files = Files::new();
    // Both revisions define one namespace, so one Catalogue holding both would make the
    // reference ambiguous.
    let model = files.write("device.json", namespaced("Release 2"));
    files.write("old.json", namespaced("Release 1"));
    let check = json!({"name":"model","operation":"check","scope":{"kind":"catalogue"}});
    let mut after = check.clone();
    after["name"] = json!("after");
    let plan = files.plan(json!([
        diff_case(
            "release",
            json!({"device":"old.json"}),
            json!({"device":"device.json"})
        ),
        check,
        diff_case(
            "missing",
            json!({"device":"missing.json"}),
            json!({"device":"device.json"})
        ),
        after
    ]));
    let result = wire(&run(&plan, &["--format", "json"]), 3);
    let direct = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "json"])
        .arg(&model)
        .output()
        .unwrap();
    let direct = wire(&direct, 0);
    for index in [1, 3] {
        assert_native_report(&result["cases"][index]["report"], &direct, "model");
    }
    assert_eq!(result["sources"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["cases"][1]["request"]["scope"]["assessed"],
        json!([1])
    );
    let release = &result["cases"][0]["report"];
    assert_eq!(release["sources"].as_array().unwrap().len(), 2);
    assert_eq!(release["result"]["summary"]["documents"]["compared"], 1);
    // An unreadable document is an ordinary acquisition result whose content stays unknown.
    let missing = &result["cases"][2];
    assert_eq!(missing["state"], "report");
    assert_eq!(missing["classification"], "undetermined");
    assert_eq!(missing["outcome"]["complete"], false);
    let sources = missing["report"]["sources"].as_array().unwrap();
    assert_eq!(sources[0]["revision"], "earlier");
    assert_eq!(sources[0]["acquisition"], "failed");
    assert_eq!(sources[1]["acquisition"], "acquired");
    assert_eq!(result["cases"][3]["outcome"]["disposition"], "passed");
    assert_eq!(result["summary"]["undetermined"], 1);
    assert_eq!(result["summary"]["complete_cases"], 3);
}

const NO_ACKNOWLEDGEMENTS: &str =
    r#"{"schema":"esdieff.acknowledged-differences","version":"1","differences":[]}"#;

#[test]
fn diff_documents_and_acknowledgements_resolve_beside_the_supplied_plan_path() {
    let files = Files::new();
    files.write("device.json", RICH_LOCK);
    files.write("minimal.sdf.json", MINIMAL_LOCK);
    files.write("rich.sdf.json", RICH_LOCK);
    files.write("acknowledged.json", NO_ACKNOWLEDGEMENTS);
    let mut case = diff_case(
        "release",
        json!({"document":"minimal.sdf.json"}),
        json!({"document":"rich.sdf.json"}),
    );
    case["acknowledged"] = json!("acknowledged.json");
    let plan = files.plan(json!([case]));
    let from_other = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir("/")
        .args(["check", "--plan"])
        .arg(&plan)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let first = wire(&from_other, 1);
    assert_eq!(first["cases"][0]["classification"], "breaking");
    let from_parent = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir(&files.0)
        .args(["check", "--plan", "plan.json", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(first, wire(&from_parent, 1));
    #[cfg(unix)]
    {
        // Beside the target, the earlier revision would equal the later one and the
        // acknowledgement file would be missing.
        let target = files.0.join("target");
        fs::create_dir(&target).unwrap();
        fs::copy(&plan, target.join("plan.json")).unwrap();
        fs::write(target.join("minimal.sdf.json"), RICH_LOCK).unwrap();
        std::os::unix::fs::symlink(target.join("plan.json"), files.0.join("link.json")).unwrap();
        let linked = wire(&run(&files.0.join("link.json"), &["--format", "json"]), 1);
        assert_eq!(linked["cases"], first["cases"]);
    }
}

/// Run a plan whose model source would block if read, and return its configuration error.
fn configuration_error(plan: &Path) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--plan"])
        .arg(plan)
        .args(["--format", "json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("invalid diff configuration acquired a source");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn malformed_diff_cases_and_acknowledgement_files_are_refused_before_acquisition() {
    let files = Files::new();
    files.write("minimal.sdf.json", MINIMAL_LOCK);
    files.write("acknowledged.json", NO_ACKNOWLEDGEMENTS);
    files.write("malformed.json", b"{");
    let mut padded = NO_ACKNOWLEDGEMENTS.as_bytes().to_vec();
    padded.resize(1024 * 1024 + 1, b' ');
    files.write("oversized.json", &padded);
    let case = diff_case(
        "release",
        json!({"document":"minimal.sdf.json"}),
        json!({"document":"minimal.sdf.json"}),
    );
    let documents: serde_json::Map<_, _> = (0..257)
        .map(|i| (format!("k{i}"), json!("minimal.sdf.json")))
        .collect();
    let mut invalid = Vec::new();
    for (member, value) in [
        ("earlier", json!({})),
        ("earlier", json!(["minimal.sdf.json"])),
        ("earlier", json!("minimal.sdf.json")),
        ("later", json!(documents)),
        ("later", json!({"":"minimal.sdf.json"})),
        ("later", json!({"x".repeat(257):"minimal.sdf.json"})),
        ("later", json!({"document":""})),
        ("later", json!({"document":7})),
        ("later", json!({"document":null})),
        ("scope", json!({"kind":"catalogue"})),
        ("assessment", json!("initial-v1")),
        ("strict_warnings", json!(false)),
        ("expected", json!("breaking")),
        ("acknowledged", json!(7)),
        ("acknowledged", json!("")),
        ("acknowledged", json!("absent.json")),
        ("acknowledged", json!("malformed.json")),
        ("acknowledged", json!("oversized.json")),
    ] {
        let mut bad = case.clone();
        bad[member] = value;
        invalid.push(json!([bad]));
    }
    for member in ["earlier", "later"] {
        let mut bad = case.clone();
        bad.as_object_mut().unwrap().remove(member);
        invalid.push(json!([bad]));
    }
    #[cfg(unix)]
    assert!(
        Command::new("mkfifo")
            .arg(files.0.join("device.json"))
            .status()
            .unwrap()
            .success()
    );
    for (index, cases) in invalid.into_iter().enumerate() {
        let message = configuration_error(&files.plan(cases));
        assert!(!message.is_empty(), "invalid plan {index}");
    }
    // Two thousand documents need more mandatory report space than the run report has.
    let side: serde_json::Map<_, _> = (0..256).map(|i| (format!("k{i:03}"), json!("p"))).collect();
    let wide: Vec<_> = (0..4)
        .map(|i| diff_case(&format!("wide {i}"), json!(side), json!(side)))
        .collect();
    let message = configuration_error(&files.plan(json!(wide)));
    assert!(message.contains("mandatory envelope"), "{message}");
    // A key repeated on either side is refused, including a differently escaped spelling.
    let escaped = format!("{}u0064ocument", '\\');
    for repeated in ["document", &escaped] {
        let twice = format!(r#"{{"document":"minimal.sdf.json","{repeated}":"other.sdf.json"}}"#);
        let once = r#"{"document":"minimal.sdf.json"}"#;
        for (earlier, later) in [(twice.as_str(), once), (once, twice.as_str())] {
            let plan = format!(
                r#"{{"schema":"esdieff.check-plan","version":"1","sources":[{{"key":"device","path":"device.json","role":"model"}}],"cases":[{{"name":"c","operation":"diff","earlier":{earlier},"later":{later}}}]}}"#
            );
            configuration_error(&files.write("plan.json", plan));
        }
    }
}

#[test]
fn an_omitted_diff_report_keeps_the_mandatory_case_fields() {
    let files = Files::new();
    files.write("device.json", MINIMAL_LOCK);
    // Removing three thousand Properties fills a native diff report's difference detail.
    let properties: serde_json::Map<_, _> = (0..3000)
        .map(|i| {
            (
                format!("removed{i}{}", "x".repeat(64)),
                json!({"type":"integer"}),
            )
        })
        .collect();
    let earlier = files.write(
        "earlier.json",
        serde_json::to_vec(&json!({"info":{},"sdfObject":{"O":{"sdfProperty":properties}}}))
            .unwrap(),
    );
    let later = files.write("later.json", br#"{"info":{},"sdfObject":{"O":{}}}"#);
    let cases: Vec<_> = (0..10)
        .map(|i| {
            diff_case(
                &format!("release {i}"),
                json!({"document":"earlier.json"}),
                json!({"document":"later.json"}),
            )
        })
        .collect();
    let output = run(&files.plan(json!(cases)), &["--format", "json"]);
    let result = wire(&output, 1);
    assert!(output.stdout.len() <= 8 * 1024 * 1024);
    let direct = wire(&direct_diff(&earlier, &later, &["--format", "json"]), 1);
    let omitted = &result["cases"][9];
    assert_eq!(
        keys(omitted),
        [
            "baseline",
            "classification",
            "name",
            "operation",
            "outcome",
            "policy",
            "report_omission",
            "request",
            "state"
        ]
    );
    assert_eq!(omitted["report_omission"]["reason"], "run-report-budget");
    assert_eq!(omitted["classification"], "breaking");
    for field in ["request", "outcome", "baseline", "policy"] {
        assert_eq!(omitted[field], direct[field], "omitted {field}");
    }
    assert_eq!(result["cases"][0]["report"], direct);
    assert!(result["summary"]["omitted_reports"].as_u64().unwrap() > 0);
    assert_eq!(result["summary"]["failed"], 10);
    assert_eq!(result["summary"]["complete_cases"], 10);
}

#[test]
fn human_run_output_shows_each_diff_case_classification_and_native_report() {
    let files = Files::new();
    files.write("device.json", RICH_LOCK);
    let earlier = files.write("minimal.sdf.json", MINIMAL_LOCK);
    let later = files.write("rich.sdf.json", RICH_LOCK);
    let direct = wire(&direct_diff(&earlier, &later, &["--format", "json"]), 1);
    let (acknowledged, _) = acknowledge_all(&files, &direct);
    let sides = (
        json!({"document":"minimal.sdf.json"}),
        json!({"document":"rich.sdf.json"}),
    );
    let mut intended = diff_case("intended", sides.0.clone(), sides.1.clone());
    intended["acknowledged"] = json!("acknowledged.json");
    let plan = files.plan(json!([diff_case("open", sides.0, sides.1), intended]));
    let human = run(&plan, &[]);
    assert_eq!(human.status.code(), Some(1));
    let text = String::from_utf8(human.stdout).unwrap();
    let (open, intended) = text
        .split_once("\nCase: \"open\"\n")
        .unwrap()
        .1
        .split_once("\nCase: \"intended\"\n")
        .unwrap();
    let native =
        |extra: &[&str]| String::from_utf8(direct_diff(&earlier, &later, extra).stdout).unwrap();
    for (section, disposition, native) in [
        (open, "failed", native(&[])),
        (
            intended,
            "passed",
            native(&["--acknowledged", acknowledged.to_str().unwrap()]),
        ),
    ] {
        assert!(section.starts_with("Operation: diff\n"), "{section}");
        assert!(section.contains(&format!("\nDisposition: {disposition}\n")));
        assert!(section.contains("\nClassification: breaking\n"));
        assert!(section.contains(native.trim_end()), "{section}");
    }
    let colored = run(&plan, &["--color", "always"]);
    assert_eq!(colored.status.code(), Some(1));
    assert!(colored.stdout.contains(&0x1b));
}

#[test]
fn acknowledgement_files_share_four_mebibytes_per_plan() {
    let files = Files::new();
    files.write("device.json", MINIMAL_LOCK);
    files.write("minimal.sdf.json", MINIMAL_LOCK);
    let mut padded = NO_ACKNOWLEDGEMENTS.as_bytes().to_vec();
    padded.resize(1024 * 1024, b' ');
    files.write("acknowledged.json", &padded);
    let cases = |count: usize| {
        json!(
            (0..count)
                .map(|i| {
                    let mut case = diff_case(
                        &format!("release {i}"),
                        json!({"document":"minimal.sdf.json"}),
                        json!({"document":"minimal.sdf.json"}),
                    );
                    case["acknowledged"] = json!("acknowledged.json");
                    case
                })
                .collect::<Vec<_>>()
        )
    };
    let result = wire(&run(&files.plan(cases(4)), &["--format", "json"]), 0);
    assert_eq!(result["summary"]["passed"], 4);
    assert_eq!(result["cases"][3]["classification"], "compatible");
    let output = run(&files.plan(cases(5)), &["--format", "json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("4 MiB"));
}
