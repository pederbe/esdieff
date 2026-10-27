//! Directory preparation is observed through the ordinary checking CLI.
#[cfg(unix)]
mod native_path_support;

use serde_json::Value;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-discovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: impl AsRef<Path>, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name.as_ref().components().collect::<PathBuf>());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        path
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
fn labels(wire: &Value) -> Vec<String> {
    wire["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| source["label"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn committed_directory_example_matches_its_documented_checks() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = Path::new("examples").join("directory-check");
    let files = [
        "README.md",
        "device.sdf.json",
        "types.sdf.json",
        "nested/status.sdf.json",
        "draft/unfinished.sdf.json",
        "reading.json",
        "plan.json",
        "unrelated.json",
        "device.sdf.json.bak",
    ];
    let input_bytes = || {
        files
            .iter()
            .map(|name| fs::read(workspace.join(&directory).join(name)).unwrap())
            .collect::<Vec<_>>()
    };
    let before = input_bytes();
    let check = |options: &[&OsStr]| {
        Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .current_dir(workspace)
            .args([
                OsStr::new("check"),
                OsStr::new("--directory"),
                directory.as_os_str(),
                OsStr::new("--format"),
                OsStr::new("json"),
            ])
            .args(options)
            .output()
            .unwrap()
    };
    let device = directory.join("device.sdf.json");
    let types = directory.join("types.sdf.json");
    let status = directory.join("nested").join("status.sdf.json");
    let draft = directory.join("draft").join("unfinished.sdf.json");

    let shallow = report(&check(&[]), 0);
    assert_eq!(shallow["outcome"]["complete"], true);
    assert_eq!(
        labels(&shallow),
        [&device, &types].map(|path| path.to_string_lossy().into_owned())
    );

    let recursive = report(
        &check(&[
            OsStr::new("--recursive"),
            OsStr::new("--exclude"),
            OsStr::new("draft"),
        ]),
        0,
    );
    let recursive_sources =
        [&device, &status, &types].map(|path| path.to_string_lossy().into_owned());
    assert_eq!(recursive["outcome"]["complete"], true);
    assert_eq!(labels(&recursive), recursive_sources);

    let selected = report(
        &check(&[
            OsStr::new("--recursive"),
            OsStr::new("--exclude"),
            OsStr::new("draft"),
            OsStr::new("--root"),
            device.as_os_str(),
        ]),
        0,
    );
    assert_eq!(selected["outcome"]["complete"], true);
    assert_eq!(labels(&selected), recursive_sources);
    assert_eq!(selected["request"]["scope"]["kind"], "selected");
    assert_eq!(
        selected["request"]["scope"]["roots"],
        serde_json::json!([1])
    );
    assert_eq!(
        selected["request"]["scope"]["dependencies"],
        serde_json::json!([3])
    );
    assert_eq!(
        selected["request"]["scope"]["assessed"],
        serde_json::json!([1, 3])
    );

    let with_draft = report(&check(&[OsStr::new("--recursive")]), 1);
    assert_eq!(with_draft["outcome"]["complete"], true);
    assert_eq!(with_draft["outcome"]["errors"], 1);
    assert_eq!(
        labels(&with_draft),
        [&device, &draft, &status, &types].map(|path| path.to_string_lossy().into_owned())
    );
    assert!(
        with_draft["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition["code"] == "data.string"
                && condition["primary"]["source_id"] == 2
                && condition["primary"]["pointer"] == "/sdfData/Unfinished/type")
    );
    assert_eq!(input_bytes(), before, "checking changed the example files");
}

#[test]
fn shallow_mixed_directory_selects_only_matching_regular_models_in_native_order() {
    let tree = Tree::new();
    let bytes = br#"{"info":{}}"#;
    let z = tree.write("z.sdf.json", bytes);
    let a = tree.write("a.sdf.json", bytes);
    let hidden = tree.write(".hidden.sdf.json", bytes);
    for name in [
        "sample.json",
        "check.plan.json",
        "other.json",
        "look.sdf.json.bak",
        "upper.SDF.JSON",
        "nested/deep.sdf.json",
    ] {
        tree.write(name, b"[]");
    }
    let explicit = tree.write("explicit.model", bytes);
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        explicit.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]);
    let wire = report(&output, 0);
    let expected = [&explicit, &hidden, &a, &z];
    assert_eq!(
        labels(&wire),
        expected
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    );
    let direct = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        explicit.as_os_str(),
        hidden.as_os_str(),
        a.as_os_str(),
        z.as_os_str(),
    ]);
    assert_eq!(wire, report(&direct, 0));
    for path in expected {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(fs::read(tree.0.join("sample.json")).unwrap(), b"[]");
}
#[test]
fn recursive_mixed_selection_sorts_all_native_relative_paths_after_explicit_inputs() {
    let tree = Tree::new();
    let bytes = br#"{"info":{}}"#;
    let z = tree.write("z.sdf.json", bytes);
    let nested = tree.write("nested/a.sdf.json", bytes);
    let sibling = tree.write("nested.sdf.json", bytes);
    let hidden = tree.write(".hidden/deep.sdf.json", bytes);
    let explicit = tree.write("odd.model", bytes);
    for name in [
        "nested/sample.json",
        "nested/check.plan.json",
        "nested/other.json",
        "nested/upper.SDF.JSON",
        "nested/look.sdf.json.bak",
    ] {
        tree.write(name, b"[]");
    }
    let options = [
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        explicit.as_os_str(),
    ];
    let args = options
        .iter()
        .copied()
        .chain([
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--recursive"),
        ])
        .collect::<Vec<_>>();
    let wire = report(&run(&args), 0);
    // The separator's native byte sorts after '.', so a sibling filename comes
    // before its similarly named directory's children.
    let expected = [&explicit, &hidden, &sibling, &nested, &z];
    assert_eq!(
        labels(&wire),
        expected.map(|path| path.to_string_lossy().into_owned())
    );
    let direct = options
        .iter()
        .copied()
        .chain([&hidden, &sibling, &nested, &z].map(|path| path.as_os_str()))
        .collect::<Vec<_>>();
    assert_eq!(wire, report(&run(&direct), 0));
    for path in [&hidden, &sibling, &nested, &z] {
        fs::remove_file(path).unwrap();
    }
    for path in [&z, &sibling, &hidden, &nested] {
        fs::write(path, bytes).unwrap();
    }
    assert_eq!(wire, report(&run(&args), 0));
    for path in expected {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(fs::read(tree.0.join("nested/sample.json")).unwrap(), b"[]");
}
#[test]
fn literal_exclusions_apply_to_each_directory_while_explicit_files_and_stdin_remain() {
    use std::{io::Write, process::Stdio};
    let first = Tree::new();
    let second = Tree::new();
    let bytes = br#"{"info":{}}"#;
    let explicit = first.write("draft/manual.sdf.json", bytes);
    let hidden = first.write(".hidden/deep.sdf.json", bytes);
    let first_model = first.write("models/keep.sdf.json", bytes);
    let second_model = second.write("models/keep.sdf.json", bytes);
    for tree in [&first, &second] {
        tree.write("draft/unfinished.sdf.json", b"[]");
        tree.write("models/omit.sdf.json", b"[]");
    }
    let run_stdin = |args: &[&OsStr]| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        child.wait_with_output().unwrap()
    };
    let wire = report(
        &run_stdin(&[
            OsStr::new("check"),
            OsStr::new("--format"),
            OsStr::new("json"),
            OsStr::new("--directory"),
            first.0.as_os_str(),
            explicit.as_os_str(),
            OsStr::new("-"),
            OsStr::new("--directory"),
            second.0.as_os_str(),
            OsStr::new("--recursive"),
            OsStr::new("--exclude"),
            OsStr::new("draft"),
            OsStr::new("--exclude"),
            Path::new("models/omit.sdf.json").as_os_str(),
            OsStr::new("--exclude"),
            OsStr::new("nonexistent"),
        ]),
        0,
    );
    assert_eq!(
        labels(&wire),
        [
            explicit.to_string_lossy().into_owned(),
            "-".to_owned(),
            hidden.to_string_lossy().into_owned(),
            first_model.to_string_lossy().into_owned(),
            second_model.to_string_lossy().into_owned(),
        ]
    );
    let direct = run_stdin(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        explicit.as_os_str(),
        OsStr::new("-"),
        hidden.as_os_str(),
        first_model.as_os_str(),
        second_model.as_os_str(),
    ]);
    assert_eq!(wire, report(&direct, 0));
    assert_eq!(fs::read(&explicit).unwrap(), bytes);
    assert_eq!(
        fs::read(first.0.join("draft/unfinished.sdf.json")).unwrap(),
        b"[]"
    );
}
#[test]
fn invalid_exclusions_and_inapplicable_traversal_options_leave_stdin_unread() {
    use std::{io::Seek, process::Stdio};
    let tree = Tree::new();
    let model = tree.write("one.sdf.json", br#"{"info":{}}"#);
    let sentinel = tree.write("stdin-sentinel", br#"{"info":{}}"#);
    let mut stream = fs::File::open(sentinel).unwrap();
    let mut invalid = vec![
        "",
        ".",
        "..",
        "./one.sdf.json",
        "a/./b",
        "a/.",
        "a/../b",
        "/root",
    ];
    #[cfg(windows)]
    invalid.extend([r"C:\root", r"C:relative", r"\root", r"a\.\b", r"a\..\b"]);
    // Keep the mutable list useful on platforms without Windows prefixes.
    invalid.push("../one.sdf.json");
    for exclusion in invalid {
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--format", "json", "-", "--directory"])
            .arg(&tree.0)
            .args(["--exclude", exclusion])
            .stdin(Stdio::from(stream.try_clone().unwrap()))
            .output()
            .unwrap();
        setup_failure(&output);
        assert!(String::from_utf8_lossy(&output.stderr).contains("exclusion"));
        assert_eq!(stream.stream_position().unwrap(), 0);
    }
    for options in [vec!["--recursive"], vec!["--exclude", "one.sdf.json"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--format", "json", "-"])
            .args(&options)
            .stdin(Stdio::from(stream.try_clone().unwrap()))
            .output()
            .unwrap();
        setup_failure(&output);
        assert_eq!(stream.stream_position().unwrap(), 0);
        for command in ["inspect", "sample", "author"] {
            setup_failure(&run(&[
                vec![
                    OsStr::new(command),
                    OsStr::new("--directory"),
                    tree.0.as_os_str(),
                ],
                options.iter().map(OsStr::new).collect(),
            ]
            .concat()));
        }
        setup_failure(&run(&[
            vec![OsStr::new("check"), OsStr::new("--plan"), model.as_os_str()],
            options.iter().map(OsStr::new).collect(),
        ]
        .concat()));
    }
}
#[test]
fn exclusions_remove_automatic_overlap_and_require_a_model_in_every_requested_directory() {
    let tree = Tree::new();
    let excluded = tree.write("one.sdf.json", br#"{"info":{}}"#);
    let retained = tree.write("nested/two.sdf.json", br#"{"info":{}}"#);
    let wire = report(
        &run(&[
            OsStr::new("check"),
            OsStr::new("--format"),
            OsStr::new("json"),
            excluded.as_os_str(),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--recursive"),
            OsStr::new("--exclude"),
            OsStr::new("one.sdf.json"),
        ]),
        0,
    );
    assert_eq!(
        labels(&wire),
        [&excluded, &retained].map(|path| path.to_string_lossy().into_owned())
    );
    setup_failure(&run(&[
        OsStr::new("check"),
        excluded.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--exclude"),
        OsStr::new("one.sdf.json"),
    ]));
    let other = Tree::new();
    other.write("keep.sdf.json", br#"{"info":{}}"#);
    setup_failure(&run(&[
        OsStr::new("check"),
        OsStr::new("--directory"),
        other.0.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--recursive"),
        OsStr::new("--exclude"),
        OsStr::new("one.sdf.json"),
        OsStr::new("--exclude"),
        OsStr::new("nested"),
    ]));
}
#[test]
fn exclusions_are_case_sensitive_native_components_and_do_not_interpret_metacharacters() {
    let tree = Tree::new();
    let keep = tree.write("Folder/one.sdf.json", br#"{"info":{}}"#);
    let options = [
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--recursive"),
    ];
    let wire = report(
        &run(&[
            options.to_vec(),
            vec![OsStr::new("--exclude"), OsStr::new("folder")],
        ]
        .concat()),
        0,
    );
    assert_eq!(labels(&wire), [keep.to_string_lossy().into_owned()]);
    setup_failure(&run(&[
        options.to_vec(),
        vec![OsStr::new("--exclude"), OsStr::new("Folder")],
    ]
    .concat()));
    #[cfg(unix)]
    {
        let literal = tree.write("*.sdf.json", br#"{"info":{}}"#);
        tree.write("$HOME/bad.sdf.json", b"[]");
        tree.write("[draft]/bad.sdf.json", b"[]");
        let output = run(&[
            options.to_vec(),
            vec![
                OsStr::new("--exclude"),
                OsStr::new("*.sdf.json"),
                OsStr::new("--exclude"),
                OsStr::new("$HOME"),
                OsStr::new("--exclude"),
                OsStr::new("[draft]"),
            ],
        ]
        .concat());
        assert_eq!(
            labels(&report(&output, 0)),
            [keep.to_string_lossy().into_owned()]
        );
        assert_eq!(fs::read(literal).unwrap(), br#"{"info":{}}"#);
    }
}
#[test]
fn recursion_admits_depth_sixty_four_and_prunes_an_excluded_deeper_directory() {
    use std::{io::Seek, process::Stdio};
    let tree = Tree::new();
    let mut relative = PathBuf::new();
    for _ in 0..64 {
        relative.push("d");
    }
    let model = tree.write(relative.join("model.sdf.json"), br#"{"info":{}}"#);
    let options = [
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--recursive"),
    ];
    let wire = report(&run(&options), 0);
    assert_eq!(labels(&wire), [model.to_string_lossy().into_owned()]);
    let deeper = relative.join("deeper");
    fs::create_dir(tree.0.join(&deeper)).unwrap();
    let sentinel = tree.write("stdin-sentinel", br#"{"info":{}}"#);
    let mut stream = fs::File::open(sentinel).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(options)
        .arg("-")
        .stdin(Stdio::from(stream.try_clone().unwrap()))
        .output()
        .unwrap();
    setup_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("depth exceeds 64"));
    assert_eq!(stream.stream_position().unwrap(), 0);
    let pruned = run(&[
        options.to_vec(),
        vec![OsStr::new("--exclude"), deeper.as_os_str()],
    ]
    .concat());
    assert_eq!(wire, report(&pruned, 0));
}
#[test]
fn recursive_entry_pressure_is_shared_and_excluded_subtrees_are_not_enumerated() {
    use std::{io::Seek, process::Stdio};
    let tree = Tree::new();
    let model = tree.write("keep.sdf.json", br#"{"info":{}}"#);
    for index in 0..9998 {
        tree.write(format!("large/ignored-{index}"), b"");
    }
    // One model, the large directory itself and its 9,998 ignored children.
    let options = [
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--recursive"),
    ];
    let wire = report(&run(&options), 0);
    assert_eq!(labels(&wire), [model.to_string_lossy().into_owned()]);
    tree.write("large/over-limit", b"");
    let sentinel_tree = Tree::new();
    let sentinel = sentinel_tree.write("stdin-sentinel", br#"{"info":{}}"#);
    let mut stream = fs::File::open(sentinel).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(options)
        .arg("-")
        .stdin(Stdio::from(stream.try_clone().unwrap()))
        .output()
        .unwrap();
    setup_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("10,000"));
    assert_eq!(stream.stream_position().unwrap(), 0);
    let pruned = run(&[
        options.to_vec(),
        vec![OsStr::new("--exclude"), OsStr::new("large")],
    ]
    .concat());
    assert_eq!(wire, report(&pruned, 0));
    fs::remove_file(tree.0.join("large/over-limit")).unwrap();
    let repeated = run(&[
        options.to_vec(),
        vec![OsStr::new("--directory"), tree.0.as_os_str()],
    ]
    .concat());
    setup_failure(&repeated);
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("10,000"));
}
#[test]
fn exclusion_count_and_native_option_bytes_are_admitted_before_model_acquisition() {
    use std::{ffi::OsString, io::Seek, process::Stdio};
    let tree = Tree::new();
    let model = tree.write("one.sdf.json", br#"{"info":{}}"#);
    let values: Vec<_> = (0..256)
        .map(|index| OsString::from(format!("missing-{index}")))
        .collect();
    let mut args = vec![
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ];
    for value in &values {
        args.extend([OsStr::new("--exclude"), value.as_os_str()]);
    }
    assert_eq!(
        labels(&report(&run(&args), 0)),
        [model.to_string_lossy().into_owned()]
    );
    let sentinel = tree.write("stdin-sentinel", br#"{"info":{}}"#);
    let mut stream = fs::File::open(sentinel).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(&args)
        .args(["--exclude", "one-over", "-"])
        .stdin(Stdio::from(stream.try_clone().unwrap()))
        .output()
        .unwrap();
    setup_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("too many literal exclusions"));
    assert_eq!(stream.stream_position().unwrap(), 0);
    // Linux can launch this total argument size. macOS and Windows have smaller
    // native launch limits than the CLI's metadata allowance.
    let large = OsString::from("x".repeat(8192));
    let mut args = vec![
        OsStr::new("check"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("-"),
    ];
    for _ in 0..129 {
        args.extend([OsStr::new("--exclude"), large.as_os_str()]);
    }
    #[cfg(unix)]
    {
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(&args)
            .stdin(Stdio::from(stream.try_clone().unwrap()))
            .output();
        #[cfg(target_os = "macos")]
        assert_eq!(
            output.unwrap_err().kind(),
            std::io::ErrorKind::ArgumentListTooLong
        );
        #[cfg(not(target_os = "macos"))]
        {
            let output = output.unwrap();
            setup_failure(&output);
            assert!(String::from_utf8_lossy(&output.stderr).contains("metadata"));
        }
        assert_eq!(stream.stream_position().unwrap(), 0);
    }
}
#[cfg(unix)]
#[test]
fn excluded_denied_descendants_are_pruned_while_included_enumeration_or_identity_failures_stop_setup()
 {
    use std::{io::Seek, os::unix::fs::PermissionsExt, process::Stdio};
    let tree = Tree::new();
    let keep = tree.write("keep.sdf.json", br#"{"info":{}}"#);
    let blocked_model = tree.write("blocked/model.sdf.json", br#"{"info":{}}"#);
    let blocked = tree.0.join("blocked");
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o0)).unwrap();
    assert_eq!(
        fs::read_dir(&blocked).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let sentinel = tree.write("stdin-sentinel", br#"{"info":{}}"#);
    let mut stream = fs::File::open(sentinel).unwrap();
    let options = [
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--recursive"),
    ];
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(options)
        .arg("-")
        .stdin(Stdio::from(stream.try_clone().unwrap()))
        .output()
        .unwrap();
    let pruned = run(&[
        options.to_vec(),
        vec![OsStr::new("--exclude"), OsStr::new("blocked")],
    ]
    .concat());
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
    setup_failure(&output);
    assert_eq!(stream.stream_position().unwrap(), 0);
    assert_eq!(
        labels(&report(&pruned, 0)),
        [keep.to_string_lossy().into_owned()]
    );
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(fs::read_dir(&blocked).is_ok());
    assert_eq!(
        fs::metadata(&blocked_model).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(options)
        .arg("-")
        .stdin(Stdio::from(stream.try_clone().unwrap()))
        .output()
        .unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
    setup_failure(&output);
    assert_eq!(stream.stream_position().unwrap(), 0);
}
#[cfg(unix)]
#[test]
fn native_non_utf8_exclusions_match_original_components_without_lossy_coalescing() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let tree = Tree::new();
    let excluded_name = OsString::from_vec(b"native-\xfe.sdf.json".to_vec());
    let kept_name = OsString::from_vec(b"native-\xff.sdf.json".to_vec());
    let literal = tree.write("nested/native-�.sdf.json", br#"{"info":{}}"#);
    let relative = Path::new("nested").join(&excluded_name);
    let excluded = tree.0.join(&relative);
    let kept = tree.0.join("nested").join(kept_name);
    let excluded_rejection = native_path_support::write(&excluded, b"[]");
    let kept_rejection = native_path_support::write(&kept, br#"{"info":{}}"#);
    let mut selected = vec![literal.clone()];
    if kept_rejection.is_none() {
        selected.push(kept);
    }
    selected.sort();
    let wire = report(
        &run(&[
            OsStr::new("check"),
            OsStr::new("--format"),
            OsStr::new("json"),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--recursive"),
            OsStr::new("--exclude"),
            relative.as_os_str(),
        ]),
        0,
    );
    assert_eq!(
        labels(&wire),
        selected
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        "filesystem rejections: excluded={excluded_rejection:?}, kept={kept_rejection:?}"
    );
    assert_eq!(wire["sources"][0]["acquisition"], "acquired");
    let mut explicit = vec![
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
    ];
    explicit.extend(selected.iter().map(|path| path.as_os_str()));
    assert_eq!(wire, report(&run(&explicit), 0));
    if excluded_rejection.is_none() {
        assert_eq!(fs::read(excluded).unwrap(), b"[]");
    }
    assert_eq!(fs::read(literal).unwrap(), br#"{"info":{}}"#);
}
fn setup_failure(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert!(output.stderr.len() < 2048);
}
#[test]
fn automatic_overlap_is_refused_while_explicit_duplicates_stay_distinct() {
    let tree = Tree::new();
    let selected = tree.write("a.sdf.json", br#"{"info":{}}"#);
    let external = tree.write("explicit.model", br#"{"info":{}}"#);
    let start = [
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
    ];
    for extra in [
        vec![
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
        ],
        vec![
            selected.as_os_str(),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
        ],
    ] {
        setup_failure(&run(&start
            .iter()
            .copied()
            .chain(extra)
            .collect::<Vec<_>>()));
    }
    let output = run(&[
        start[0],
        start[1],
        start[2],
        external.as_os_str(),
        external.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]);
    assert_eq!(
        labels(&report(&output, 0)),
        [&external, &external, &selected].map(|path| path.to_string_lossy().into_owned())
    );
}
#[cfg(unix)]
#[test]
fn hard_links_and_explicit_file_links_overlap_native_files() {
    use std::os::unix::fs::symlink;
    let tree = Tree::new();
    let a = tree.write("a.sdf.json", br#"{"info":{}}"#);
    let alias = tree.0.join("alias.model");
    fs::hard_link(&a, &alias).unwrap();
    let linked = tree.0.join("linked.model");
    symlink(&a, &linked).unwrap();
    for explicit in [&alias, &linked] {
        setup_failure(&run(&[
            OsStr::new("check"),
            explicit.as_os_str(),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
        ]));
    }
    fs::hard_link(&a, tree.0.join("duplicate.sdf.json")).unwrap();
    setup_failure(&run(&[
        OsStr::new("check"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]));
}
#[test]
fn several_directories_append_after_positional_sources_and_stdin() {
    use std::{io::Write, process::Stdio};
    let first = Tree::new();
    let second = Tree::new();
    let z = first.write("z.sdf.json", br#"{"info":{}}"#);
    let a = first.write("a.sdf.json", br#"{"info":{}}"#);
    let b = second.write("b.sdf.json", br#"{"info":{}}"#);
    let explicit = second.write("odd.model", br#"{"info":{}}"#);
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            OsStr::new("check"),
            OsStr::new("--format"),
            OsStr::new("json"),
            OsStr::new("--directory"),
            first.0.as_os_str(),
            explicit.as_os_str(),
            OsStr::new("-"),
            OsStr::new("--directory"),
            second.0.as_os_str(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"info":{}}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let expected = [
        explicit.to_string_lossy().into_owned(),
        "-".to_owned(),
        a.to_string_lossy().into_owned(),
        z.to_string_lossy().into_owned(),
        b.to_string_lossy().into_owned(),
    ];
    assert_eq!(labels(&report(&output, 0)), expected);
    fs::remove_file(&a).unwrap();
    fs::remove_file(&z).unwrap();
    fs::write(&a, br#"{"info":{}}"#).unwrap();
    fs::write(&z, br#"{"info":{}}"#).unwrap();
    let reordered = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        first.0.as_os_str(),
    ]);
    assert_eq!(
        labels(&report(&reordered, 0)),
        [a, z].map(|path| path.to_string_lossy().into_owned())
    );
}
#[test]
fn discovered_root_keeps_dependency_and_unused_definition_scope_across_profiles() {
    let tree = Tree::new();
    let donor = tree.write("a.sdf.json", br##"{"info":{},"namespace":{"d":"urn:example:discovered"},"defaultNamespace":"d","sdfData":{"Value":{"type":"integer","maximum":10},"Unused":{"description":7}}}"##);
    let root = tree.write("nested/b.sdf.json", br##"{"info":{},"namespace":{"d":"urn:example:discovered"},"sdfProperty":{"Use":{"sdfRef":"d:#/sdfData/Value"}}}"##);
    let start = [
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
    ];
    let catalogue = report(
        &run(&[
            start[0],
            start[1],
            start[2],
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--recursive"),
        ]),
        1,
    );
    assert_eq!(catalogue["outcome"]["errors"], 1);
    for profile in [
        "initial-v1",
        "expanded-modified-v1",
        "expanded-units-v1",
        "expanded-content-format-v1",
        "expanded-pattern-syntax-v1",
        "expanded-numeric-v1",
        "expanded-consistency-v1",
        "full-baseline",
    ] {
        for strict in [false, true] {
            let mut options = vec![
                start[0],
                start[1],
                start[2],
                OsStr::new("--assessment"),
                OsStr::new(profile),
                OsStr::new("--root"),
                root.as_os_str(),
            ];
            if strict {
                options.push(OsStr::new("--strict-warnings"));
            }
            let discovered = run(&options
                .iter()
                .copied()
                .chain([
                    OsStr::new("--directory"),
                    tree.0.as_os_str(),
                    OsStr::new("--recursive"),
                ])
                .collect::<Vec<_>>());
            let direct = run(&options
                .iter()
                .copied()
                .chain([donor.as_os_str(), root.as_os_str()])
                .collect::<Vec<_>>());
            assert_eq!(discovered.status.code(), direct.status.code());
            let wire = report(&discovered, direct.status.code().unwrap());
            assert_eq!(wire, report(&direct, direct.status.code().unwrap()));
            assert_eq!(
                labels(&wire),
                [&donor, &root].map(|path| path.to_string_lossy().into_owned())
            );
            assert_eq!(wire["request"]["scope"]["kind"], "selected");
            assert_eq!(wire["request"]["scope"]["roots"], serde_json::json!([2]));
            assert_eq!(
                wire["request"]["scope"]["dependencies"],
                serde_json::json!([1])
            );
        }
    }
    for profile in [
        "expanded-units-v1",
        "expanded-content-format-v1",
        "full-baseline",
    ] {
        for policy in ["recorded", "withheld"] {
            let options = [
                start[0],
                start[1],
                start[2],
                OsStr::new("--assessment"),
                OsStr::new(profile),
                OsStr::new("--registry-knowledge"),
                OsStr::new(policy),
            ];
            let discovered = run(&options
                .iter()
                .copied()
                .chain([
                    OsStr::new("--directory"),
                    tree.0.as_os_str(),
                    OsStr::new("--recursive"),
                ])
                .collect::<Vec<_>>());
            let direct = run(&options
                .iter()
                .copied()
                .chain([donor.as_os_str(), root.as_os_str()])
                .collect::<Vec<_>>());
            assert_eq!(report(&discovered, 1), report(&direct, 1));
        }
    }
}
#[cfg(unix)]
#[test]
fn explicit_directory_link_preserves_paths_and_descendant_links_and_fifo_are_skipped() {
    use std::ffi::OsString;
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let tree = Tree::new();
    let outside = Tree::new();
    let model = tree.write("one.sdf.json", br#"{"info":{}}"#);
    let nested = tree.write("nested/deep.sdf.json", br#"{"info":{}}"#);
    let unicode = tree.write("native-温度.sdf.json", br#"{"info":{}}"#);
    let non_utf8 = tree
        .0
        .join(OsString::from_vec(b"native-\xff.sdf.json".to_vec()));
    let rejection = native_path_support::write(&non_utf8, br#"{"info":{}}"#);
    let alias = outside.0.join("selected");
    symlink(&tree.0, &alias).unwrap();
    symlink(&model, tree.0.join("linked.sdf.json")).unwrap();
    symlink(&tree.0, tree.0.join("loop")).unwrap();
    symlink(&outside.0, tree.0.join("nested/outside")).unwrap();
    outside.write("outside.sdf.json", b"[]");
    let fifo = tree.0.join("pipe.sdf.json");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        alias.as_os_str(),
        OsStr::new("--recursive"),
    ]);
    let wire = report(&output, 0);
    let mut expected = vec![
        alias.join(unicode.file_name().unwrap()),
        alias.join("nested").join(nested.file_name().unwrap()),
        alias.join("one.sdf.json"),
    ];
    if rejection.is_none() {
        expected.push(alias.join(non_utf8.file_name().unwrap()));
    }
    expected.sort();
    assert_eq!(
        labels(&wire),
        expected
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        "filesystem rejection: {rejection:?}"
    );
    assert!(
        wire["sources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|source| source["acquisition"] == "acquired")
    );
    setup_failure(&run(&[
        OsStr::new("check"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--directory"),
        alias.as_os_str(),
    ]));
}
#[test]
fn selected_malformed_content_and_missing_explicit_file_have_ordinary_reports() {
    let tree = Tree::new();
    let malformed = tree.write("nested/broken.sdf.json", b"[]");
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--recursive"),
    ]);
    let wire = report(&output, 1);
    assert_eq!(labels(&wire), [malformed.to_string_lossy().into_owned()]);
    assert_eq!(wire["sources"][0]["acquisition"], "acquired");
    assert_eq!(wire["outcome"]["errors"], 1);
    fs::write(&malformed, br#"{"info":{}}"#).unwrap();
    let missing = tree.0.join("missing.explicit");
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        missing.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--recursive"),
    ]);
    let wire = report(&output, 3);
    assert_eq!(
        labels(&wire),
        [&missing, &malformed].map(|path| path.to_string_lossy().into_owned())
    );
    assert_eq!(wire["sources"][0]["acquisition"], "failed");
    assert_eq!(wire["sources"][1]["acquisition"], "acquired");
}
#[test]
fn different_byte_identical_contributors_preserve_ordinary_ambiguity() {
    let tree = Tree::new();
    let donor = br##"{"info":{},"namespace":{"d":"urn:example:ambiguous"},"defaultNamespace":"d","sdfData":{"Value":{"type":"integer"}}}"##;
    let a = tree.write("a.sdf.json", donor);
    let b = tree.write("b.sdf.json", donor);
    let c = tree.write("c.sdf.json", br##"{"info":{},"namespace":{"d":"urn:example:ambiguous"},"sdfProperty":{"Use":{"sdfRef":"d:#/sdfData/Value"}}}"##);
    let discovered = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]);
    let wire = report(&discovered, 3);
    assert_eq!(
        labels(&wire),
        [&a, &b, &c].map(|path| path.to_string_lossy().into_owned())
    );
    assert_eq!(wire["outcome"]["complete"], false);
    assert_eq!(
        wire,
        report(
            &run(&[
                OsStr::new("check"),
                OsStr::new("--format"),
                OsStr::new("json"),
                a.as_os_str(),
                b.as_os_str(),
                c.as_os_str()
            ]),
            3
        )
    );
}
#[test]
fn invalid_selection_and_options_stop_before_consuming_stdin() {
    use std::{io::Seek, process::Stdio};
    let tree = Tree::new();
    let empty = Tree::new();
    let model = tree.write("one.sdf.json", br#"{"info":{}}"#);
    let missing = tree.0.join("absent");
    let sentinel = tree.write("stdin-sentinel", br#"{"info":{}}"#);
    let mut stream = fs::File::open(&sentinel).unwrap();
    for extra in [
        vec![OsStr::new("--directory"), missing.as_os_str()],
        vec![OsStr::new("--directory"), model.as_os_str()],
        vec![OsStr::new("--directory"), empty.0.as_os_str()],
        vec![
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--directory"),
            empty.0.as_os_str(),
        ],
        vec![
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--root"),
            OsStr::new("one.sdf.json"),
        ],
        vec![
            model.as_os_str(),
            model.as_os_str(),
            OsStr::new("--root"),
            model.as_os_str(),
        ],
        vec![
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--root"),
            model.as_os_str(),
            model.as_os_str(),
        ],
        vec![
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--registry-knowledge"),
            OsStr::new("recorded"),
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args(["check", "--format", "json", "-"])
            .args(extra)
            .stdin(Stdio::from(stream.try_clone().unwrap()))
            .output()
            .unwrap();
        setup_failure(&output);
        assert_eq!(
            stream.stream_position().unwrap(),
            0,
            "setup failure read stdin"
        );
    }
    for command in ["inspect", "sample", "author"] {
        setup_failure(&run(&[
            OsStr::new(command),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
        ]));
    }
    setup_failure(&run(&[
        OsStr::new("check"),
        OsStr::new("--plan"),
        missing.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]));
}
#[cfg(unix)]
#[test]
fn permissions_distinguish_unavailable_traversal_from_selected_unreadable_content() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new();
    let model = tree.write("one.sdf.json", br#"{"info":{}}"#);
    fs::set_permissions(&model, fs::Permissions::from_mode(0o0)).unwrap();
    // Assert that this host actually enforces the permissions before claiming coverage.
    assert_eq!(
        fs::File::open(&model).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]);
    let wire = report(&output, 3);
    assert_eq!(labels(&wire), [model.to_string_lossy().into_owned()]);
    assert_eq!(wire["sources"][0]["acquisition"], "failed");
    fs::set_permissions(&model, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&tree.0, fs::Permissions::from_mode(0o0)).unwrap();
    assert_eq!(
        fs::read_dir(&tree.0).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let failure = run(&[
        OsStr::new("check"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]);
    fs::set_permissions(&tree.0, fs::Permissions::from_mode(0o700)).unwrap();
    setup_failure(&failure);
    let locked = tree.0.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o0)).unwrap();
    let inaccessible = locked.join("unknown.model");
    let failure = run(&[
        OsStr::new("check"),
        inaccessible.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    setup_failure(&failure);
}
#[test]
fn ignored_entries_and_assembled_source_attempts_have_finite_ceilings() {
    let entries = Tree::new();
    let model = entries.write("selected.sdf.json", br#"{"info":{}}"#);
    for index in 0..9999 {
        entries.write(format!("ignored-{index}"), b"");
    }
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        entries.0.as_os_str(),
    ]);
    assert_eq!(
        labels(&report(&output, 0)),
        [model.to_string_lossy().into_owned()]
    );
    entries.write("ignored-over", b"");
    setup_failure(&run(&[
        OsStr::new("check"),
        OsStr::new("--directory"),
        entries.0.as_os_str(),
    ]));
    let sources = Tree::new();
    for index in 0..256 {
        sources.write(format!("{index:03}.sdf.json"), br#"{"info":{}}"#);
    }
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        sources.0.as_os_str(),
    ]);
    assert_eq!(report(&output, 0)["sources"].as_array().unwrap().len(), 256);
    sources.write("one-over.sdf.json", br#"{"info":{}}"#);
    setup_failure(&run(&[
        OsStr::new("check"),
        OsStr::new("--directory"),
        sources.0.as_os_str(),
    ]));
}
#[test]
fn directory_request_and_repeated_native_metadata_limits_are_preparation_errors() {
    let tree = Tree::new();
    tree.write("one.sdf.json", br#"{"info":{}}"#);
    let mut args = vec![OsStr::new("check")];
    for _ in 0..257 {
        args.extend([OsStr::new("--directory"), tree.0.as_os_str()]);
    }
    setup_failure(&run(&args));
    #[cfg(unix)]
    {
        let mut deep = tree.0.clone();
        // macOS paths are shorter. More legal descendants still fill the same
        // traversal metadata allowance without reaching the source-count limit.
        let depth = if cfg!(target_os = "macos") { 3 } else { 14 };
        for _ in 0..depth {
            deep.push("d".repeat(240));
        }
        fs::create_dir_all(&deep).unwrap();
        for index in 0..200 {
            fs::write(deep.join(format!("{index:03}.sdf.json")), br#"{"info":{}}"#).unwrap();
        }
        #[cfg(target_os = "macos")]
        for index in 0..1024 {
            fs::create_dir(deep.join(format!("metadata-{index:04}"))).unwrap();
        }
        // A repeated long argument list cannot reach the CLI on macOS.
        #[cfg(not(target_os = "macos"))]
        {
            let mut args = vec![OsStr::new("check")];
            for _ in 0..128 {
                args.extend([OsStr::new("--directory"), deep.as_os_str()]);
            }
            let output = run(&args);
            setup_failure(&output);
            assert!(String::from_utf8_lossy(&output.stderr).contains("metadata"));
        }
        let recursive = run(&[
            OsStr::new("check"),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--recursive"),
        ]);
        setup_failure(&recursive);
        assert!(String::from_utf8_lossy(&recursive.stderr).contains("metadata"));
        let pruned = run(&[
            OsStr::new("check"),
            OsStr::new("--format"),
            OsStr::new("json"),
            OsStr::new("--directory"),
            tree.0.as_os_str(),
            OsStr::new("--recursive"),
            OsStr::new("--exclude"),
            OsStr::new(&"d".repeat(240)),
        ]);
        assert_eq!(report(&pruned, 0)["sources"].as_array().unwrap().len(), 1);
    }
}
#[test]
fn relative_discovery_and_selected_roots_use_the_invocation_directory() {
    let tree = Tree::new();
    tree.write("models/one.sdf.json", br#"{"info":{}}"#);
    let root = Path::new("models").join("one.sdf.json");
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir(&tree.0)
        .args([
            "check",
            "--format",
            "json",
            "--directory",
            "models",
            "--root",
        ])
        .arg(&root)
        .output()
        .unwrap();
    let wire = report(&output, 0);
    assert_eq!(labels(&wire), [root.to_string_lossy().into_owned()]);
    assert_eq!(wire["request"]["scope"]["roots"], serde_json::json!([1]));
}
#[test]
fn repeated_traversal_counts_ignored_entries_before_overlap_and_source_limits_count_stdin() {
    use std::{io::Seek, process::Stdio};
    let tree = Tree::new();
    tree.write("one.sdf.json", br#"{"info":{}}"#);
    for index in 0..5000 {
        tree.write(format!("ignored-{index}"), b"");
    }
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]);
    setup_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("10,000"));
    let sources = Tree::new();
    for index in 0..256 {
        sources.write(format!("{index:03}.sdf.json"), br#"{"info":{}}"#);
    }
    let sentinel = sources.write("stdin-sentinel", br#"{"info":{}}"#);
    let mut stream = fs::File::open(sentinel).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["check", "--format", "json", "-", "--directory"])
        .arg(&sources.0)
        .stdin(Stdio::from(stream.try_clone().unwrap()))
        .output()
        .unwrap();
    setup_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("256"));
    assert_eq!(stream.stream_position().unwrap(), 0);
}
#[cfg(unix)]
#[test]
fn long_discovered_native_paths_are_acquired_with_omitted_display_labels() {
    let tree = Tree::new();
    let name = PathBuf::from("a".repeat(240))
        .join("b".repeat(240))
        .join("model.sdf.json");
    let path = tree.write(name, br#"{"info":{}}"#);
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        path.parent().unwrap().as_os_str(),
    ]);
    let wire = report(&output, 0);
    assert_eq!(wire["sources"].as_array().unwrap().len(), 1);
    assert_eq!(wire["sources"][0]["acquisition"], "acquired");
    assert_eq!(wire["sources"][0]["label"], Value::Null);
}
#[test]
fn discovery_help_and_plan_version_one_remain_explicit_about_source_selection() {
    let help = run(&[OsStr::new("check"), OsStr::new("--help")]);
    assert_eq!(help.status.code(), Some(0));
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("--directory DIR"));
    assert!(help.contains("--recursive"));
    assert!(help.contains("--exclude RELATIVE_PATH"));
    assert!(help.contains("all supplied directories"));
    assert!(help.contains("invocation directory"));
    assert!(help.contains(".sdf.json"));
    assert!(help.contains("10,000"));
    let tree = Tree::new();
    tree.write("one.sdf.json", br#"{"info":{}}"#);
    for source in [
        serde_json::json!({"key":"dynamic", "directory":".", "role":"model"}),
        serde_json::json!({"key":"dynamic", "path":"one.sdf.json", "role":"model", "recursive":true}),
        serde_json::json!({"key":"dynamic", "path":"one.sdf.json", "role":"model", "exclude":["draft"]}),
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({"schema":"esdieff.check-plan", "version":"1",
            "sources":[source], "cases":[{"name":"catalogue", "operation":"check", "scope":{"kind":"catalogue"}}]})).unwrap();
        let plan = tree.write("dynamic.plan.json", &bytes);
        setup_failure(&run(&[
            OsStr::new("check"),
            OsStr::new("--plan"),
            plan.as_os_str(),
        ]));
    }
}
#[cfg(windows)]
#[test]
fn windows_hard_links_overlap_and_directory_junction_descendants_are_skipped() {
    let tree = Tree::new();
    let outside = Tree::new();
    let model = tree.write("one.sdf.json", br#"{"info":{}}"#);
    tree.write("nested/deep.sdf.json", br#"{"info":{}}"#);
    let duplicate = outside.0.join("alias.model");
    fs::hard_link(&model, &duplicate).unwrap();
    setup_failure(&run(&[
        OsStr::new("check"),
        duplicate.as_os_str(),
        OsStr::new("--directory"),
        tree.0.as_os_str(),
    ]));
    let junction = outside.0.join("selected");
    let command = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&tree.0)
        .output()
        .unwrap();
    assert!(
        command.status.success(),
        "{}",
        String::from_utf8_lossy(&command.stderr)
    );
    let loop_link = tree.0.join("loop");
    let command = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&loop_link)
        .arg(&tree.0)
        .output()
        .unwrap();
    assert!(
        command.status.success(),
        "{}",
        String::from_utf8_lossy(&command.stderr)
    );
    let output = run(&[
        OsStr::new("check"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--directory"),
        junction.as_os_str(),
        OsStr::new("--recursive"),
    ]);
    assert_eq!(
        labels(&report(&output, 0)),
        [
            junction.join("nested").join("deep.sdf.json"),
            junction.join("one.sdf.json")
        ]
        .map(|path| path.to_string_lossy().into_owned())
    );
    fs::remove_dir(&loop_link).unwrap();
    fs::remove_dir(&junction).unwrap();
}
