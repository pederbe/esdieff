//! `esdieff diff` compares two Model revisions supplied as directories or single files.
//! Follows ADR 0039: Client-side revision comparison, and ADR 0004: Supplied catalogues and
//! checking scope.
use esdieff::{Catalogue, ComparisonRequest, HumanStyle, SourceInput};
use serde_json::{Value, json};
#[allow(dead_code)]
mod cli_color_support;
#[cfg(unix)]
mod native_path_support;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const LOCK: &str = include_str!("../examples/door-lock-check/rich.sdf.json");
const THERMOSTAT: &str = include_str!("../examples/thermostat-check/rich.sdf.json");

#[test]
fn declaration_obligations_share_classifications_across_reports_and_saved_plans() {
    let tree = Tree::new();
    let optional = r#"{"info":{"version":"1"},"sdfObject":{"Oven":{"sdfProperty":{"temp":{"type":"number"}}}}}"#;
    let mandatory = optional.replace("\"Oven\":{", "\"Oven\":{\"sdfRequired\":[\"temp\"],");
    let old = tree.write("optional.json", optional);
    let new = tree.write("mandatory.json", &mandatory);
    for (earlier, later, expected, exit) in
        [(&old, &new, "breaking", 1), (&new, &old, "compatible", 0)]
    {
        let output = diff(
            earlier.as_os_str(),
            later.as_os_str(),
            &["--perspective", "implementer", "--format", "json"],
        );
        let wire = report(&output, exit);
        assert_eq!(wire["version"], "1.7");
        assert_eq!(
            wire["request"]["comparison"],
            "esdieff.revision-comparison/6"
        );
        assert_eq!(wire["policy"]["perspective"], "implementer");
        assert_eq!(wire["result"]["differences"][0]["quality"], "sdfRequired");
        assert_eq!(wire["result"]["differences"][0]["classification"], expected);
        assert_eq!(wire["result"]["differences"][0]["directions"], json!([]));
        for format in ["human", "markdown", "sarif"] {
            let output = diff(
                earlier.as_os_str(),
                later.as_os_str(),
                &["--perspective", "implementer", "--format", format],
            );
            assert_eq!(output.status.code(), Some(exit));
            let rendered = String::from_utf8(output.stdout).unwrap();
            if format == "sarif" {
                let log: Value = serde_json::from_str(&rendered).unwrap();
                assert_eq!(log["runs"][0]["properties"]["perspective"], "implementer");
                assert!(rendered.contains("sdfRequired"));
                assert!(rendered.contains(expected));
            } else {
                assert!(rendered.contains("Perspective: implementer."));
                assert!(rendered.contains(expected));
                assert!(rendered.contains("sdfRequired"));
            }
        }
        let plan = tree.write(
            "requiredness-plan.json",
            &json!({
                "schema":"esdieff.check-plan","version":"1",
                "sources":[{"key":"current","path":"mandatory.json","role":"model"}],
                "cases":[{"name":"obligations","operation":"diff","perspective":"implementer",
                    "earlier":{"model":earlier.file_name().unwrap().to_str().unwrap()},
                    "later":{"model":later.file_name().unwrap().to_str().unwrap()}}]
            })
            .to_string(),
        );
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--plan"])
            .arg(&plan)
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
        let result = &plan["cases"][0]["report"]["result"];
        assert_eq!(result["summary"], wire["result"]["summary"]);
        assert_eq!(result["outcome"], expected);
        assert_eq!(result["differences"].as_array().unwrap().len(), 1);
        for field in [
            "kind",
            "subject",
            "quality",
            "classification",
            "directions",
            "earlier",
            "later",
        ] {
            assert_eq!(
                result["differences"][0][field],
                wire["result"]["differences"][0][field]
            );
        }
        assert_eq!(
            plan["cases"][0]["report"]["policy"]["perspective"],
            "implementer"
        );
    }
    // Client remains the default, with the opposite classification for new obligations.
    let client = report(
        &diff(old.as_os_str(), new.as_os_str(), &["--format", "json"]),
        0,
    );
    assert_eq!(client["policy"]["perspective"], "client");
    assert_eq!(
        client["result"]["differences"][0]["classification"],
        "compatible"
    );
}

#[test]
fn implementer_perspective_flows_through_cli_reports_and_saved_plans() {
    let tree = Tree::new();
    let before = r#"{"sdfObject":{"Device":{"sdfProperty":{"value":{"type":"integer","maximum":10,"writable":true,"readable":false,"observable":false}}}}}"#;
    let after = before.replace("maximum\":10", "maximum\":20");
    let old = tree.write("old.json", before);
    let new = tree.write("new.json", &after);
    let options = ["--perspective", "implementer", "--format", "json"];
    let wire = report(&diff(old.as_os_str(), new.as_os_str(), &options), 1);
    assert_eq!(wire["policy"]["perspective"], "implementer");
    let default = report(
        &diff(old.as_os_str(), new.as_os_str(), &["--format", "json"]),
        0,
    );
    assert_eq!(default["policy"]["perspective"], "client");
    for format in ["human", "markdown", "sarif"] {
        let result = diff(
            old.as_os_str(),
            new.as_os_str(),
            &["--perspective", "implementer", "--format", format],
        );
        assert_eq!(result.status.code(), Some(1));
        let rendered = String::from_utf8(result.stdout).unwrap();
        if format == "sarif" {
            let log: Value = serde_json::from_str(&rendered).unwrap();
            assert_eq!(log["runs"][0]["properties"]["perspective"], "implementer");
        } else {
            assert!(rendered.contains("Perspective: implementer."));
        }
    }
    let plan = tree.write("plan.json", &json!({
        "schema":"esdieff.check-plan","version":"1","sources":[{"key":"current","path":"new.json","role":"model"}],
        "cases":[{"name":"device","operation":"diff","perspective":"implementer",
            "earlier":{"model":"old.json"},"later":{"model":"new.json"}}]
    }).to_string());
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--plan"])
        .arg(&plan)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        plan["cases"][0]["report"]["policy"]["perspective"],
        "implementer"
    );
}

#[test]
fn invalid_and_duplicate_perspectives_are_invocation_errors() {
    for options in [
        vec!["--perspective", "unknown"],
        vec!["--perspective", "client", "--perspective", "implementer"],
        vec!["--perspective"],
    ] {
        let output = diff(
            OsStr::new("missing-old"),
            OsStr::new("missing-new"),
            &options,
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("cannot read"));
    }
}

#[test]
fn unavailable_context_scalars_preserve_independent_breaking_changes() {
    let before = r#"{"sdfObject":{"Device":{
        "sdfContext":{"metadata":{"const":1e1000001}},
        "sdfProperty":{"reading":{"type":"integer","maximum":10}}
    }}}"#;
    let after = before
        .replace("\"const\":", "\"description\":\"Updated\",\"const\":")
        .replace("\"maximum\":10", "\"maximum\":20");
    let tree = Tree::new();
    let old = tree.write("old.json", before);
    let new = tree.write("new.json", &after);
    let wire = report(
        &diff(old.as_os_str(), new.as_os_str(), &["--format", "json"]),
        1,
    );
    assert_eq!(wire["outcome"]["complete"], false);
    assert_eq!(wire["result"]["summary"]["breaking"], 1);
    assert_eq!(wire["result"]["summary"]["undetermined"], 1);
    let context = wire["result"]["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| difference["subject"] == "sdfContext")
        .unwrap();
    assert_eq!(context["kind"], "unavailable");
    assert_eq!(context["reason"]["code"], "data.number_unavailable");
    let human = diff(old.as_os_str(), new.as_os_str(), &[]);
    assert_eq!(human.status.code(), Some(1));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(context["reason"]["message"].as_str().unwrap()));
    assert!(human.contains("Breaking differences:"));
}

#[test]
fn context_differences_use_existing_reports_and_check_plans() {
    let before = r##"{"info":{"version":"1"},
        "sdfData":{"Value":{"type":"integer","maximum":10,"nullable":false}},
        "sdfObject":{"Device":{
            "sdfContext":{"value":{"sdfRef":"#/sdfData/Value"},"retired":{"type":"boolean"}},
            "sdfProperty":{"value":{"sdfRef":"#/sdfData/Value","writable":false}}
        }}
    }"##;
    let cases = [
        (
            "metadata change",
            "/sdfObject/Device/sdfContext/value/maximum",
            json!(5),
            0,
            1,
        ),
        (
            "addition",
            "/sdfObject/Device/sdfContext/added",
            json!({"type":"string"}),
            0,
            1,
        ),
        (
            "same-name Property",
            "/sdfObject/Device/sdfProperty/value/maximum",
            json!(20),
            1,
            0,
        ),
        ("shared data", "/sdfData/Value/maximum", json!(20), 1, 1),
    ];
    for (name, pointer, value, exit, compatible) in cases {
        let tree = Tree::new();
        let old = tree.write("old.json", before);
        let after = revised(before, |document| {
            // The addition is the only path whose last token does not exist yet.
            let (owner, key) = pointer.rsplit_once('/').unwrap();
            document.pointer_mut(owner).unwrap()[key] = value;
        });
        let new = tree.write("new.json", &after);
        let wire = report(
            &diff(old.as_os_str(), new.as_os_str(), &["--format", "json"]),
            exit,
        );
        assert_eq!(
            wire["result"]["summary"]["compatible"], compatible,
            "{name}"
        );
        assert_eq!(wire["result"]["summary"]["breaking"], exit, "{name}");
        assert_eq!(wire["policy"]["compatibility_scope"], "client_interactions");
        assert_eq!(wire["outcome"]["warnings"], exit);
        let human = diff(old.as_os_str(), new.as_os_str(), &[]);
        assert_eq!(human.status.code(), Some(exit));
        let human = String::from_utf8(human.stdout).unwrap();
        assert!(
            human.contains(
                "Client interactions only. Context metadata compatibility is not assessed."
            )
        );
        if compatible > 0 {
            assert!(human.contains("changed sdfContext") || human.contains("added sdfContext"));
            assert!(
                wire["result"]["differences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|difference| difference["subject"] == "sdfContext"
                        && difference["directions"] == json!([]))
            );
        }
        let plan = tree.write(
            "plan.json",
            &json!({
                "schema":"esdieff.check-plan","version":"1",
                "sources":[{"key":"current","path":"new.json","role":"model"}],
                "cases":[{"name":name,"operation":"diff",
                    "earlier":{"document":"old.json"},"later":{"document":"new.json"}}]
            })
            .to_string(),
        );
        let saved = report(
            &run(&[
                OsStr::new("check"),
                OsStr::new("--plan"),
                plan.as_os_str(),
                OsStr::new("--format"),
                OsStr::new("json"),
            ]),
            exit,
        );
        assert_eq!(saved["cases"][0]["report"], wire, "{name}");
    }
    for remove in ["sdfContext", "sdfProperty"] {
        let tree = Tree::new();
        let old = tree.write("old.json", before);
        let after = revised(before, |document| {
            document["sdfObject"]["Device"]
                .as_object_mut()
                .unwrap()
                .remove(remove);
        });
        let new = tree.write("new.json", &after);
        let exit = i32::from(remove == "sdfProperty");
        let wire = report(
            &diff(old.as_os_str(), new.as_os_str(), &["--format", "json"]),
            exit,
        );
        assert_eq!(wire["result"]["summary"]["breaking"], exit);
        assert!(
            wire["result"]["differences"]
                .as_array()
                .unwrap()
                .iter()
                .all(
                    |difference| difference["kind"] == "removed" && difference["subject"] == remove
                )
        );
    }
}

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-diff-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: impl AsRef<Path>, text: &str) -> PathBuf {
        let path = self.0.join(name.as_ref().components().collect::<PathBuf>());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, text).unwrap();
        path
    }
    fn path(&self) -> &OsStr {
        self.0.as_os_str()
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .output()
        .unwrap()
}
fn diff(old: &OsStr, new: &OsStr, options: &[&str]) -> Output {
    let mut args = vec![
        OsStr::new("diff"),
        OsStr::new("--old"),
        old,
        OsStr::new("--new"),
        new,
    ];
    args.extend(options.iter().map(OsStr::new));
    run(&args)
}
fn report(output: &Output, exit: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}
fn revised(text: &str, revise: impl FnOnce(&mut Value)) -> String {
    let mut document: Value = serde_json::from_str(text).unwrap();
    revise(&mut document);
    serde_json::to_string_pretty(&document).unwrap()
}
/// The source key of a request-local ID within one revision.
fn key(wire: &Value, revision: &str, id: &Value) -> String {
    wire["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["revision"] == revision && source["id"] == *id)
        .and_then(|source| source["key"].as_str())
        .unwrap()
        .to_owned()
}
/// Document correspondence as source keys: compared pairs, removed and added documents.
fn documents(wire: &Value) -> (Vec<(String, String)>, Vec<String>, Vec<String>) {
    let documents = &wire["result"]["documents"];
    let compared = documents["compared"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            (
                key(wire, "earlier", &pair["earlier"]),
                key(wire, "later", &pair["later"]),
            )
        })
        .collect();
    let unmatched = |field: &str, revision: &str| {
        documents[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| key(wire, revision, id))
            .collect()
    };
    (
        compared,
        unmatched("removed", "earlier"),
        unmatched("added", "later"),
    )
}
fn pairs(keys: &[&str]) -> Vec<(String, String)> {
    keys.iter()
        .map(|key| (key.to_string(), key.to_string()))
        .collect()
}
fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn directories_pair_documents_by_their_relative_paths() {
    let old = Tree::new();
    let new = Tree::new();
    old.write("lock.sdf.json", LOCK);
    old.write("rooms/hall/thermostat.sdf.json", THERMOSTAT);
    old.write("rooms/spare.sdf.json", THERMOSTAT);
    new.write(
        "lock.sdf.json",
        &revised(LOCK, |document| {
            document["sdfObject"]["Lock"]["sdfProperty"]["state"]["description"] =
                json!("Bolt position");
        }),
    );
    new.write("rooms/hall/thermostat.sdf.json", THERMOSTAT);
    new.write("archive/spare.sdf.json", THERMOSTAT);
    let wire = report(
        &diff(old.path(), new.path(), &["--recursive", "--format", "json"]),
        1,
    );
    assert_eq!(wire["operation"], "diff");
    assert_eq!(
        documents(&wire),
        (
            pairs(&["lock.sdf.json", "rooms/hall/thermostat.sdf.json"]),
            strings(&["rooms/spare.sdf.json"]),
            strings(&["archive/spare.sdf.json"]),
        )
    );
    assert_eq!(
        wire["result"]["summary"],
        json!({"breaking": 1, "undetermined": 0, "compatible": 2,
            "acknowledged": {"breaking": 0, "undetermined": 0},
            "documents": {"compared": 2, "removed": 1, "added": 1}})
    );
}

#[test]
fn two_files_correspond_directly_whatever_their_names() {
    let tree = Tree::new();
    let old = tree.write("v1/lock.sdf.json", LOCK);
    let new = tree.write(
        "lock-2.json",
        &revised(LOCK, |document| {
            document["sdfObject"]["Lock"]["sdfProperty"]["state"]["description"] =
                json!("Bolt position");
        }),
    );
    let wire = report(
        &diff(old.as_os_str(), new.as_os_str(), &["--format", "json"]),
        0,
    );
    assert_eq!(
        wire["result"]["summary"],
        json!({"breaking": 0, "undetermined": 0, "compatible": 1,
            "acknowledged": {"breaking": 0, "undetermined": 0},
            "documents": {"compared": 1, "removed": 0, "added": 0}})
    );
    let labels: Vec<_> = wire["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| (source["revision"].clone(), source["label"].clone()))
        .collect();
    assert_eq!(
        labels,
        [
            (json!("earlier"), json!(old.to_str().unwrap())),
            (json!("later"), json!(new.to_str().unwrap())),
        ]
    );
}

#[test]
fn recursion_and_exclusions_select_both_revisions_alike() {
    let old = Tree::new();
    let new = Tree::new();
    for tree in [&old, &new] {
        tree.write("lock.sdf.json", LOCK);
        tree.write("nested/thermostat.sdf.json", THERMOSTAT);
    }
    old.write("draft/first.sdf.json", THERMOSTAT);
    new.write("draft/second.sdf.json", THERMOSTAT);
    let shallow = report(&diff(old.path(), new.path(), &["--format", "json"]), 0);
    assert_eq!(
        documents(&shallow),
        (pairs(&["lock.sdf.json"]), vec![], vec![])
    );
    let excluded = report(
        &diff(
            old.path(),
            new.path(),
            &["--recursive", "--exclude", "draft", "--format", "json"],
        ),
        0,
    );
    assert_eq!(
        documents(&excluded),
        (
            pairs(&["lock.sdf.json", "nested/thermostat.sdf.json"]),
            vec![],
            vec![]
        )
    );
    let recursive = report(
        &diff(old.path(), new.path(), &["--recursive", "--format", "json"]),
        1,
    );
    assert_eq!(
        documents(&recursive),
        (
            pairs(&["lock.sdf.json", "nested/thermostat.sdf.json"]),
            strings(&["draft/first.sdf.json"]),
            strings(&["draft/second.sdf.json"])
        )
    );
}

/// The lock with its state description reworded, its credential pattern changed or its
/// authorize Action removed: a compatible, an undetermined and a breaking revision.
fn lock_revisions() -> [(String, i32); 3] {
    let lock = |revise: fn(&mut Value)| {
        revised(LOCK, |document| revise(&mut document["sdfObject"]["Lock"]))
    };
    [
        (
            lock(|lock| lock["sdfProperty"]["state"]["description"] = json!("Bolt position")),
            0,
        ),
        (
            lock(|lock| lock["sdfProperty"]["credential"]["pattern"] = json!("^[a-z]+$")),
            3,
        ),
        (
            lock(|lock| {
                lock["sdfAction"]
                    .as_object_mut()
                    .unwrap()
                    .remove("authorize");
            }),
            1,
        ),
    ]
}

#[test]
fn exit_codes_follow_the_comparison_outcome() {
    let old = Tree::new();
    old.write("lock.sdf.json", LOCK);
    for (text, exit) in lock_revisions() {
        let new = Tree::new();
        new.write("lock.sdf.json", &text);
        let wire = report(&diff(old.path(), new.path(), &["--format", "json"]), exit);
        let (outcome, disposition) = match exit {
            0 => ("compatible", "passed"),
            1 => ("breaking", "failed"),
            _ => ("undetermined", "undetermined"),
        };
        assert_eq!(wire["result"]["outcome"], outcome);
        assert_eq!(wire["outcome"]["disposition"], disposition);
        let human = diff(old.path(), new.path(), &[]);
        assert_eq!(human.status.code(), Some(exit));
    }
    let new = Tree::new();
    new.write("lock.sdf.json", LOCK);
    let args = [
        "diff",
        "--old",
        old.0.to_str().unwrap(),
        "--new",
        new.0.to_str().unwrap(),
    ];
    let closed = cli_color_support::closed_stdout(&args, b"");
    assert_eq!(closed.status.code(), Some(4));
    assert!(
        closed
            .stderr
            .starts_with(b"esdieff: cannot emit requested report: ")
    );
}

#[test]
fn reports_match_the_library_comparison_in_every_format() {
    let old = Tree::new();
    let new = Tree::new();
    let [_, (pattern, _), (removed, _)] = lock_revisions();
    let earlier = [
        old.write("lock.sdf.json", LOCK),
        old.write("rooms/thermostat.sdf.json", THERMOSTAT),
    ];
    let later = [
        new.write("lock.sdf.json", &pattern),
        new.write("rooms/lock.sdf.json", &removed),
    ];
    let catalogue = |paths: &[PathBuf], keys: &[&str]| {
        let inputs: Vec<_> = paths
            .iter()
            .zip(keys)
            .map(|(path, key)| SourceInput::file(key, path).label(path.to_str().unwrap()))
            .collect();
        Catalogue::from_inputs(&inputs).unwrap()
    };
    let earlier = catalogue(&earlier, &["lock.sdf.json", "rooms/thermostat.sdf.json"]);
    let later = catalogue(&later, &["lock.sdf.json", "rooms/lock.sdf.json"]);
    let comparison = ComparisonRequest::new(&earlier, &later).compare().unwrap();
    let text = |output: Output| {
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    };
    let with_newline = |report: String| {
        if report.ends_with('\n') {
            report
        } else {
            report + "\n"
        }
    };
    let options = ["--recursive", "--format", "json"];
    let json = with_newline(comparison.to_json());
    assert_eq!(text(diff(old.path(), new.path(), &options)), json);
    let forced = ["--recursive", "--format", "json", "--color", "always"];
    assert_eq!(text(diff(old.path(), new.path(), &forced)), json);
    assert_eq!(
        text(diff(old.path(), new.path(), &["--recursive"])),
        with_newline(comparison.to_human())
    );
    assert_eq!(
        text(diff(
            old.path(),
            new.path(),
            &["--recursive", "--color=always"]
        )),
        with_newline(comparison.to_human_with_style(HumanStyle::Ansi))
    );
    assert!(
        comparison
            .to_human_with_style(HumanStyle::Ansi)
            .contains('\x1b')
    );
    assert_eq!(
        text(diff(old.path(), new.path(), &["--no-color", "--recursive"])),
        with_newline(comparison.to_human())
    );
}

/// A source key must be text, so a non-UTF-8 relative path cannot pair documents.
#[cfg(unix)]
#[test]
fn a_relative_path_that_is_not_utf8_is_an_invocation_error() {
    use std::os::unix::ffi::OsStrExt;
    let old = Tree::new();
    let new = Tree::new();
    let mut created = true;
    for tree in [&old, &new] {
        let path = tree.0.join(OsStr::from_bytes(b"lock\xff.sdf.json"));
        if native_path_support::write(&path, LOCK.as_bytes()).is_some() {
            created = false;
            // APFS cannot create this name. The rejected native path must still
            // produce an invocation error rather than a comparison report.
            assert!(
                invocation_error(&diff(path.as_os_str(), new.path(), &[]))
                    .contains("cannot prepare revision")
            );
        }
    }
    if created {
        assert!(invocation_error(&diff(old.path(), new.path(), &[])).contains("not UTF-8"));
    }
}

#[test]
fn help_lists_the_diff_command() {
    let global = run(&[OsStr::new("--help")]);
    for args in [&["diff", "--help"][..], &["diff", "-h", "--old", "missing"]] {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        let output = run(&args);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, global.stdout);
    }
    let help = String::from_utf8(global.stdout).unwrap();
    assert!(help.contains(
            "esdieff diff --old PATH --new PATH [--recursive] [--exclude RELATIVE_PATH] [--perspective client|implementer] [--acknowledged FILE] [--acknowledgements-output FILE] [--replace-acknowledgements] [--format human|json|sarif|markdown]\n"
    ));
}

fn invocation_error(output: &Output) -> String {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.len() < 2048);
    let message = String::from_utf8(output.stderr.clone()).unwrap();
    assert!(message.starts_with("esdieff: ") && message.ends_with('\n'));
    message
}

#[test]
fn unsupported_invocations_stop_before_any_comparison() {
    let old = Tree::new();
    let new = Tree::new();
    let file = old.write("lock.sdf.json", LOCK);
    new.write("lock.sdf.json", LOCK);
    let empty = Tree::new();
    let file = file.as_os_str();
    let missing = old.0.join("missing.sdf.json");
    let missing = missing.as_os_str();
    let dash = OsStr::new("-");
    for (old, new, options) in [
        (file, new.path(), &[][..]),
        (old.path(), file, &[]),
        (dash, file, &[]),
        (file, dash, &[]),
        (dash, new.path(), &[]),
        (old.path(), empty.path(), &[]),
        (empty.path(), new.path(), &["--recursive"]),
        (missing, file, &[]),
        (file, file, &["--recursive"]),
        (file, file, &["--exclude", "draft"]),
        (old.path(), new.path(), &["--exclude", "../draft"]),
        (old.path(), new.path(), &["--format", "yaml"]),
        (old.path(), new.path(), &["--root", "lock.sdf.json"]),
        (old.path(), new.path(), &["--old", "again"]),
        (old.path(), new.path(), &["lock.sdf.json"]),
    ] {
        invocation_error(&diff(old, new, options));
    }
    for args in [
        &["diff"][..],
        &["diff", "--old", "lock.sdf.json"],
        &["diff", "--new", "lock.sdf.json"],
        &["diff", "--old"],
    ] {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        invocation_error(&run(&args));
    }
    assert!(
        invocation_error(&diff(file, new.path(), &[]))
            .contains("must both be directories or both be files")
    );
    assert!(invocation_error(&diff(dash, file, &[])).contains("standard input"));
    assert!(
        invocation_error(&diff(old.path(), empty.path(), &[]))
            .contains("no regular .sdf.json files selected")
    );
}

/// The lock without its credential Property or its authorize Action.
fn lock_without(credential: bool, authorize: bool) -> String {
    revised(LOCK, |document| {
        let lock = &mut document["sdfObject"]["Lock"];
        if credential {
            lock["sdfProperty"]
                .as_object_mut()
                .unwrap()
                .remove("credential");
        }
        if authorize {
            lock["sdfAction"]
                .as_object_mut()
                .unwrap()
                .remove("authorize");
        }
    })
}
/// An acknowledgement file whose entries name these removed definitions of the lock.
fn acknowledged(entries: &[(&str, &str, &str)]) -> String {
    let entries: Vec<_> = entries
        .iter()
        .map(|(subject, name, classification)| {
            json!({"kind": "removed", "subject": subject, "classification": classification,
                "earlier": {"document": "lock.sdf.json", "pointer": format!("/sdfObject/Lock/{subject}/{name}")},
                "later": null})
        })
        .collect();
    json!({"schema": "esdieff.acknowledged-differences", "version": "1", "differences": entries})
        .to_string()
}
const CREDENTIAL: (&str, &str, &str) = ("sdfProperty", "credential", "breaking");
const AUTHORIZE: (&str, &str, &str) = ("sdfAction", "authorize", "breaking");

#[test]
fn an_acknowledged_breaking_difference_exits_0_while_the_reports_say_breaking() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    old.write("lock.sdf.json", LOCK);
    new.write("lock.sdf.json", &lock_without(true, false));
    let file = files.write("acknowledged.json", &acknowledged(&[CREDENTIAL]));
    let options = ["--acknowledged", file.to_str().unwrap()];
    let wire = report(
        &diff(
            old.path(),
            new.path(),
            &[&options[..], &["--format", "json"]].concat(),
        ),
        0,
    );
    assert_eq!(wire["version"], "1.7");
    assert_eq!(wire["result"]["outcome"], "breaking");
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(wire["result"]["summary"]["breaking"], 1);
    assert_eq!(wire["result"]["differences"][0]["acknowledged"], true);
    let human = diff(old.path(), new.path(), &options);
    assert_eq!(human.status.code(), Some(0));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.starts_with(
        "Revision comparison esdieff.revision-comparison/6: breaking\nDisposition: passed, with 1 breaking and 0 undetermined differences acknowledged\n"
    ));
    assert!(human.contains(
        "Acknowledged differences:\n  removed sdfProperty, breaking\n    earlier lock.sdf.json /sdfObject/Lock/sdfProperty/credential\n"
    ));
    assert!(!human.contains("Breaking differences:"));
}

#[test]
fn only_unacknowledged_differences_decide_the_exit_code() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    old.write("lock.sdf.json", LOCK);
    new.write("lock.sdf.json", &lock_without(true, true));
    let undetermined = ("sdfProperty", "credential", "undetermined");
    for (entries, exit, errors, unused) in [
        (&[CREDENTIAL][..], 1, 1, json!([])),
        (&[CREDENTIAL, AUTHORIZE], 0, 0, json!([])),
        (&[undetermined, AUTHORIZE], 1, 1, json!([0])),
    ] {
        let file = files.write("acknowledged.json", &acknowledged(entries));
        let wire = report(
            &diff(
                old.path(),
                new.path(),
                &["--acknowledged", file.to_str().unwrap(), "--format", "json"],
            ),
            exit,
        );
        assert_eq!(wire["outcome"]["errors"], errors);
        assert_eq!(wire["result"]["unused_acknowledgements"], unused);
    }
    // An unused entry leaves a compatible comparison passing, and both reports list it.
    let file = files.write("acknowledged.json", &acknowledged(&[CREDENTIAL]));
    let same = Tree::new();
    same.write("lock.sdf.json", LOCK);
    let options = ["--acknowledged", file.to_str().unwrap()];
    let wire = report(
        &diff(
            old.path(),
            same.path(),
            &[&options[..], &["--format", "json"]].concat(),
        ),
        0,
    );
    assert_eq!(wire["result"]["unused_acknowledgements"], json!([0]));
    let human = diff(old.path(), same.path(), &options);
    assert_eq!(human.status.code(), Some(0));
    assert!(String::from_utf8(human.stdout).unwrap().ends_with(
        "Unused acknowledgements:\n  /differences/0 removed sdfProperty, breaking\n    earlier lock.sdf.json /sdfObject/Lock/sdfProperty/credential\n"
    ));
}

#[test]
fn every_acknowledgement_file_error_is_an_invocation_error() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    old.write("lock.sdf.json", LOCK);
    new.write("lock.sdf.json", &lock_without(true, false));
    let valid = acknowledged(&[CREDENTIAL]);
    let entry = |member: &str, value: Value| {
        let mut file: Value = serde_json::from_str(&valid).unwrap();
        file["differences"][0][member] = value;
        file.to_string()
    };
    let path = |name: &str, text: &str| files.write(name, text).to_str().unwrap().to_owned();
    let cases = [
        path("malformed.json", "{\"schema\": "),
        path(
            "oversized.json",
            &format!("{valid}{}", " ".repeat(1024 * 1024)),
        ),
        path(
            "duplicated.json",
            &valid.replacen(
                "\"version\":\"1\"",
                "\"version\":\"1\",\"version\":\"1\"",
                1,
            ),
        ),
        path("unavailable.json", &entry("kind", json!("unavailable"))),
        path(
            "compatible.json",
            &entry("classification", json!("compatible")),
        ),
        path("repeated.json", &acknowledged(&[CREDENTIAL, CREDENTIAL])),
    ];
    let good = path("good.json", &valid);
    let missing = files.0.join("missing.json");
    for options in [
        vec!["--acknowledged", cases[0].as_str()],
        vec!["--acknowledged", cases[1].as_str()],
        vec!["--acknowledged", cases[2].as_str()],
        vec!["--acknowledged", cases[3].as_str()],
        vec!["--acknowledged", cases[4].as_str()],
        vec!["--acknowledged", cases[5].as_str()],
        vec!["--acknowledged", missing.to_str().unwrap()],
        vec!["--acknowledged", files.0.to_str().unwrap()],
        vec!["--acknowledged", "-"],
        vec!["--acknowledged"],
        vec!["--acknowledged", &good, "--acknowledged", &good],
    ] {
        invocation_error(&diff(old.path(), new.path(), &options));
    }
    assert!(
        invocation_error(&diff(old.path(), new.path(), &["--acknowledged", "-"]))
            .contains("standard input")
    );
    assert!(
        invocation_error(&diff(
            old.path(),
            new.path(),
            &["--acknowledged", &cases[3]]
        ))
        .contains("cannot read acknowledgements")
    );
    // The file is decoded before any revision is prepared.
    let absent = files.0.join("absent");
    assert!(
        invocation_error(&diff(
            absent.as_os_str(),
            new.path(),
            &["--acknowledged", &cases[0]]
        ))
        .contains("acknowledgement")
    );
}

/// A monitor whose read-only Properties state these units, or none.
fn monitor(units: &[(&str, Option<&str>)]) -> String {
    let mut properties = serde_json::Map::new();
    for (name, unit) in units {
        let mut property = json!({"type": "number", "writable": false});
        if let Some(unit) = unit {
            property["unit"] = json!(unit);
        }
        properties.insert((*name).to_owned(), property);
    }
    json!({"sdfObject": {"Monitor": {"sdfProperty": properties}}}).to_string()
}
/// An acknowledgement file whose entries name unit changes of these monitor Properties.
fn acknowledged_units(entries: &[(&str, &str)]) -> String {
    let entries: Vec<_> = entries
        .iter()
        .map(|(property, classification)| {
            let side = json!({"document": "monitor.sdf.json",
                "pointer": format!("/sdfObject/Monitor/sdfProperty/{property}/unit")});
            json!({"kind": "changed", "subject": "quality", "quality": "unit",
                "classification": classification, "earlier": side, "later": side})
        })
        .collect();
    json!({"schema": "esdieff.acknowledged-differences", "version": "1", "differences": entries})
        .to_string()
}

#[test]
fn an_acknowledged_undetermined_difference_exits_0_and_reports_complete() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    old.write("monitor.sdf.json", &monitor(&[("power", None)]));
    new.write("monitor.sdf.json", &monitor(&[("power", Some("W"))]));
    let file = files.write(
        "acknowledged.json",
        &acknowledged_units(&[("power", "undetermined")]),
    );
    let options = ["--acknowledged", file.to_str().unwrap()];
    let wire = report(
        &diff(
            old.path(),
            new.path(),
            &[&options[..], &["--format", "json"]].concat(),
        ),
        0,
    );
    assert_eq!(wire["result"]["outcome"], "undetermined");
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["outcome"]["complete"], true);
    assert_eq!(wire["outcome"]["incomplete_checks"], 0);
    assert_eq!(wire["coverage"]["requested"]["assessed"], 1);
    assert_eq!(wire["coverage"]["requested"]["undetermined"], 0);
    let difference = &wire["result"]["differences"][0];
    assert_eq!(difference["classification"], "undetermined");
    assert_eq!(difference["reason"]["code"], "comparison.unit_added");
    assert_eq!(difference["acknowledged"], true);
    let human = diff(old.path(), new.path(), &options);
    assert_eq!(human.status.code(), Some(0));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(
        human.starts_with(
            "Revision comparison esdieff.revision-comparison/6: undetermined\nDisposition: passed, with 0 breaking and 1 undetermined differences acknowledged\n"
        ),
        "{human}"
    );
    assert!(
        human.contains(
            "Acknowledged differences:\n  changed quality unit (thing-supplied data), undetermined: unit is added, which changes the stated meaning of values\n"
        ),
        "{human}"
    );
    assert!(!human.contains("Undetermined differences:"), "{human}");
}

#[test]
fn exported_acknowledgements_can_be_reviewed_and_supplied_to_diff() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    old.write("lock.sdf.json", LOCK);
    new.write("lock.sdf.json", &lock_without(true, false));
    let destination = files.0.join("reviewed.json");
    let path = destination.to_str().unwrap();
    let wire = report(
        &diff(
            old.path(),
            new.path(),
            &["--acknowledgements-output", path, "--format", "json"],
        ),
        1,
    );
    assert_eq!(wire["outcome"]["disposition"], "failed");
    let exported: Value = serde_json::from_slice(&fs::read(&destination).unwrap()).unwrap();
    let expected: Value = serde_json::from_str(&acknowledged(&[CREDENTIAL])).unwrap();
    assert_eq!(exported, expected);
    let reviewed = report(
        &diff(
            old.path(),
            new.path(),
            &["--acknowledged", path, "--format", "json"],
        ),
        0,
    );
    assert_eq!(reviewed["result"]["summary"]["acknowledged"]["breaking"], 1);
}

#[test]
fn acknowledgement_export_keeps_only_eligible_entries_in_every_report_format() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    let model = |unit: Option<&str>, bound| {
        let mut value: Value = serde_json::from_str(&monitor(&[("power", unit)])).unwrap();
        value["sdfObject"]["Monitor"]["description"] =
            json!(if bound == 10 { "later" } else { "earlier" });
        // This malformed direction prevents examination of the changed bound.
        value["sdfObject"]["Monitor"]["sdfProperty"]["level"] =
            json!({"type": "number", "maximum": bound, "writable": "yes"});
        value.to_string()
    };
    old.write("monitor.sdf.json", &model(None, 100));
    new.write("monitor.sdf.json", &model(Some("W"), 10));
    let expected: Value =
        serde_json::from_str(&acknowledged_units(&[("power", "undetermined")])).unwrap();
    for format in ["human", "json", "sarif", "markdown"] {
        let destination = files.0.join(format!("{format}.json"));
        let output = diff(
            old.path(),
            new.path(),
            &[
                "--acknowledgements-output",
                destination.to_str().unwrap(),
                "--format",
                format,
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.is_empty());
        let exported: Value = serde_json::from_slice(&fs::read(destination).unwrap()).unwrap();
        assert_eq!(exported, expected);
    }
    let destination = files.0.join("empty.json");
    let output = diff(
        old.path(),
        old.path(),
        &["--acknowledgements-output", destination.to_str().unwrap()],
    );
    assert_eq!(output.status.code(), Some(0));
    let exported: Value = serde_json::from_slice(&fs::read(destination).unwrap()).unwrap();
    assert_eq!(exported["differences"], json!([]));
}

#[test]
fn acknowledgement_export_requires_explicit_replacement_and_protects_inputs() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    let source = old.write("lock.sdf.json", LOCK);
    let later = new.write("lock.sdf.json", &lock_without(true, false));
    let destination = files.write("reviewed.json", "keep me");
    let path = destination.to_str().unwrap();
    invocation_error(&diff(
        old.path(),
        new.path(),
        &["--acknowledgements-output", path],
    ));
    assert_eq!(fs::read_to_string(&destination).unwrap(), "keep me");
    let output = diff(
        old.path(),
        new.path(),
        &[
            "--acknowledgements-output",
            path,
            "--replace-acknowledgements",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let supplied = files.write("supplied.json", &acknowledged(&[CREDENTIAL]));
    for protected in [&source, &later, &supplied] {
        let before = fs::read(protected).unwrap();
        invocation_error(&diff(
            old.path(),
            new.path(),
            &[
                "--acknowledged",
                supplied.to_str().unwrap(),
                "--acknowledgements-output",
                protected.to_str().unwrap(),
                "--replace-acknowledgements",
            ],
        ));
        assert_eq!(fs::read(protected).unwrap(), before);
    }
    for options in [
        vec!["--acknowledgements-output"],
        vec!["--acknowledgements-output", "-"],
        vec!["--replace-acknowledgements"],
        vec![
            "--acknowledgements-output",
            path,
            "--acknowledgements-output",
            path,
        ],
        vec![
            "--acknowledgements-output",
            path,
            "--replace-acknowledgements",
            "--replace-acknowledgements",
        ],
        vec![
            "--acknowledgements-output",
            files.0.to_str().unwrap(),
            "--replace-acknowledgements",
        ],
    ] {
        invocation_error(&diff(old.path(), new.path(), &options));
    }
    let missing = files.0.join("missing-parent/reviewed.json");
    invocation_error(&diff(
        old.path(),
        new.path(),
        &["--acknowledgements-output", missing.to_str().unwrap()],
    ));

    // Export includes eligible differences even when already acknowledged.
    let output = diff(
        old.path(),
        new.path(),
        &[
            "--acknowledged",
            supplied.to_str().unwrap(),
            "--acknowledgements-output",
            path,
            "--replace-acknowledgements",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        fs::read(destination).unwrap(),
        fs::read(supplied)
            .unwrap()
            .into_iter()
            .chain(*b"\n")
            .collect::<Vec<_>>()
    );
}

#[cfg(unix)]
#[test]
fn acknowledgement_export_refuses_symlinks_and_hard_linked_revision_inputs() {
    use std::os::unix::fs::symlink;
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    let source = old.write("lock.sdf.json", LOCK);
    new.write("lock.sdf.json", &lock_without(true, false));
    let hard = files.0.join("hard.json");
    let link = files.0.join("link.json");
    let dangling = files.0.join("dangling.json");
    fs::hard_link(&source, &hard).unwrap();
    symlink(&source, &link).unwrap();
    symlink(files.0.join("absent.json"), &dangling).unwrap();
    for path in [&hard, &link, &dangling] {
        invocation_error(&diff(
            old.path(),
            new.path(),
            &[
                "--acknowledgements-output",
                path.to_str().unwrap(),
                "--replace-acknowledgements",
            ],
        ));
    }
    assert_eq!(fs::read_to_string(source).unwrap(), LOCK);
}

#[test]
fn a_truncated_report_exports_only_the_entries_it_retained() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    let properties: serde_json::Map<String, Value> = (0..4097)
        .map(|index| (format!("property{index}"), json!({"type": "boolean"})))
        .collect();
    old.write(
        "device.sdf.json",
        &json!({"sdfObject": {"Device": {"sdfProperty": properties}}}).to_string(),
    );
    new.write("device.sdf.json", r#"{"sdfObject":{"Device":{}}}"#);
    let destination = files.write("reviewed.json", "keep me");
    let wire = report(
        &diff(
            old.path(),
            new.path(),
            &[
                "--acknowledgements-output",
                destination.to_str().unwrap(),
                "--replace-acknowledgements",
                "--format",
                "json",
            ],
        ),
        1,
    );
    assert!(wire["result"]["differences_omitted"].as_u64().unwrap() > 0);
    let exported: Value = serde_json::from_slice(&fs::read(&destination).unwrap()).unwrap();
    assert_eq!(
        exported["differences"].as_array().unwrap().len(),
        wire["result"]["differences"].as_array().unwrap().len()
    );
    // The partial file is readable but cannot acknowledge the omitted differences.
    let reviewed = report(
        &diff(
            old.path(),
            new.path(),
            &[
                "--acknowledged",
                destination.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        1,
    );
    assert!(
        reviewed["result"]["summary"]["acknowledged"]["breaking"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(reviewed["result"]["unused_acknowledgements"], json!([]));
}

#[test]
fn an_unacknowledged_undetermined_difference_beside_acknowledged_ones_exits_3() {
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    // A breaking change between current unit names, an added unit, and an unrecorded spelling.
    old.write(
        "monitor.sdf.json",
        &monitor(&[
            ("temperature", Some("Cel")),
            ("power", None),
            ("energy", Some("kWh")),
        ]),
    );
    new.write(
        "monitor.sdf.json",
        &monitor(&[
            ("temperature", Some("K")),
            ("power", Some("W")),
            ("energy", Some("kwh")),
        ]),
    );
    let reviewed = [("temperature", "breaking"), ("power", "undetermined")];
    for (entries, exit, disposition) in [
        (&reviewed[..], 3, "undetermined"),
        (
            &[reviewed[0], reviewed[1], ("energy", "undetermined")],
            0,
            "passed",
        ),
    ] {
        let file = files.write("acknowledged.json", &acknowledged_units(entries));
        let options = ["--acknowledged", file.to_str().unwrap()];
        let wire = report(
            &diff(
                old.path(),
                new.path(),
                &[&options[..], &["--format", "json"]].concat(),
            ),
            exit,
        );
        assert_eq!(wire["result"]["outcome"], "breaking");
        assert_eq!(wire["outcome"]["disposition"], disposition);
        assert_eq!(wire["outcome"]["errors"], 0);
        assert_eq!(wire["outcome"]["complete"], exit == 0);
        assert_eq!(wire["result"]["unused_acknowledgements"], json!([]));
        let human = diff(old.path(), new.path(), &options);
        assert_eq!(human.status.code(), Some(exit));
        let human = String::from_utf8(human.stdout).unwrap();
        let acknowledged = &wire["result"]["summary"]["acknowledged"];
        assert!(
            human.contains(&format!(
                "Disposition: {disposition}, with {} breaking and {} undetermined differences acknowledged\n",
                acknowledged["breaking"], acknowledged["undetermined"]
            )),
            "{human}"
        );
        assert_eq!(
            human.contains(
                "Undetermined differences:\n  changed quality unit (thing-supplied data): a unit spelling has no recorded SenML entry\n"
            ),
            exit == 3,
            "{human}"
        );
    }
}

#[test]
fn an_acknowledgement_never_passes_a_difference_left_undetermined_by_unavailable_content() {
    // A changed bound whose Property's `writable` is not a Boolean, and an `sdfRequired` that
    // names a declaration the later revision cannot resolve.
    let valve = |maximum: u32| {
        json!({"sdfObject": {"Valve": {"sdfProperty": {"level":
            {"type": "number", "minimum": 0, "maximum": maximum, "writable": "yes"}}}}})
        .to_string()
    };
    let oven = |required: &str| {
        json!({"sdfObject": {"Oven": {"sdfRequired": [required], "sdfProperty":
            {"temp": {"type": "number"}, "mode": {"type": "string"}}}}})
        .to_string()
    };
    for (earlier, later, quality, pointer, reason) in [
        (
            valve(100),
            valve(10),
            "maximum",
            "/sdfObject/Valve/sdfProperty/level/maximum",
            "comparison.directions_unavailable",
        ),
        (
            oven("temp"),
            oven("missing"),
            "sdfRequired",
            "/sdfObject/Oven/sdfRequired",
            "required.unavailable",
        ),
    ] {
        let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
        old.write("device.sdf.json", &earlier);
        new.write("device.sdf.json", &later);
        let side = json!({"document": "device.sdf.json", "pointer": pointer});
        let file = files.write(
            "acknowledged.json",
            &json!({"schema": "esdieff.acknowledged-differences", "version": "1",
                "differences": [{"kind": "changed", "subject": "quality", "quality": quality,
                    "classification": "undetermined", "earlier": side, "later": side}]})
            .to_string(),
        );
        let wire = report(
            &diff(
                old.path(),
                new.path(),
                &["--acknowledged", file.to_str().unwrap(), "--format", "json"],
            ),
            3,
        );
        let difference = &wire["result"]["differences"][0];
        assert_eq!(difference["reason"]["code"], reason);
        assert_eq!(difference["acknowledged"], false);
        assert_eq!(wire["outcome"]["disposition"], "undetermined");
        assert_eq!(wire["result"]["unused_acknowledgements"], json!([0]));
    }
}

/// A fault interrupter switch at one version, with these state enum values.
fn switch(version: &str, states: &[&str]) -> String {
    serde_json::to_string_pretty(&json!({
        "info": {"title": "Fault Interrupter Switch", "version": version},
        "sdfObject": {"switch.fault": {"sdfProperty": {"state": {"type": "string", "enum": states}}}}
    }))
    .unwrap()
}

#[test]
fn a_breaking_revision_that_keeps_its_version_warns_in_every_format() {
    const CODE: &str = "comparison.version_not_increased";
    let (old, new, files) = (Tree::new(), Tree::new(), Tree::new());
    old.write(
        "switch.sdf.json",
        &switch("2019-11-14", &["on", "off", "faulted"]),
    );
    let later = switch("2019-11-14", &["on", "off"]);
    new.write("switch.sdf.json", &later);
    let wire = report(&diff(old.path(), new.path(), &["--format", "json"]), 1);
    assert_eq!(wire["version"], "1.7");
    assert_eq!(wire["result"]["summary"]["breaking"], 1);
    assert_eq!(wire["outcome"]["warnings"], 1);
    let condition = &wire["diagnostics"]["items"][0];
    assert_eq!(condition["code"], CODE);
    assert_eq!(condition["arguments"]["earlier_version"], "2019-11-14");
    assert_eq!(condition["arguments"]["later_version"], "2019-11-14");

    let human = diff(old.path(), new.path(), &[]);
    assert_eq!(human.status.code(), Some(1));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(CODE), "{human}");

    let markdown = diff(old.path(), new.path(), &["--format", "markdown"]);
    assert_eq!(markdown.status.code(), Some(1));
    assert!(
        String::from_utf8(markdown.stdout)
            .unwrap()
            .contains("## Warnings")
    );

    let sarif = diff(old.path(), new.path(), &["--format", "sarif"]);
    assert_eq!(sarif.status.code(), Some(1));
    let sarif: Value = serde_json::from_slice(&sarif.stdout).unwrap();
    let result = sarif["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["ruleId"] == CODE)
        .unwrap()
        .clone();
    assert_eq!(result["level"], "warning");
    let physical = &result["locations"][0]["physicalLocation"];
    let start = later.find("\"2019-11-14\"").unwrap();
    assert_eq!(physical["region"]["byteOffset"], start);
    assert_eq!(physical["region"]["byteLength"], 12);
    assert!(
        physical["artifactLocation"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("switch.sdf.json")
    );

    // An acknowledged break passes and still warns.
    let side = json!({"document": "switch.sdf.json",
        "pointer": "/sdfObject/switch.fault/sdfProperty/state/enum"});
    let file = files.write(
        "acknowledged.json",
        &json!({"schema": "esdieff.acknowledged-differences", "version": "1",
            "differences": [{"kind": "changed", "subject": "quality", "quality": "enum",
                "classification": "breaking", "earlier": side, "later": side}]})
        .to_string(),
    );
    let flag = ["--acknowledged", file.to_str().unwrap()];
    let wire = report(
        &diff(
            old.path(),
            new.path(),
            &[&flag[..], &["--format", "json"]].concat(),
        ),
        0,
    );
    assert_eq!(wire["outcome"]["disposition"], "passed");
    assert_eq!(wire["outcome"]["warnings"], 1);
    let markdown = diff(
        old.path(),
        new.path(),
        &[&flag[..], &["--format", "markdown"]].concat(),
    );
    assert_eq!(markdown.status.code(), Some(0));
}

#[test]
fn directory_comparison_warns_only_for_the_breaking_document() {
    let (old, new) = (Tree::new(), Tree::new());
    old.write("switch.sdf.json", &switch("1", &["on", "off", "faulted"]));
    new.write("switch.sdf.json", &switch("1", &["on", "off"]));
    // A compatible change to a document that keeps its version earns no warning.
    old.write("lamp.sdf.json", &switch("2", &["on", "off"]));
    new.write(
        "lamp.sdf.json",
        &revised(&switch("2", &["on", "off"]), |document| {
            document["sdfObject"]["switch.fault"]["description"] = json!("A lamp switch");
        }),
    );
    let wire = report(&diff(old.path(), new.path(), &["--format", "json"]), 1);
    assert_eq!(wire["result"]["summary"]["documents"]["compared"], 2);
    assert_eq!(wire["result"]["summary"]["breaking"], 1);
    assert_eq!(wire["result"]["summary"]["compatible"], 1);
    assert_eq!(wire["outcome"]["warnings"], 1);
    let primary = &wire["diagnostics"]["items"][0]["primary"];
    assert_eq!(
        key(&wire, "later", &primary["source_id"]),
        "switch.sdf.json"
    );
}
