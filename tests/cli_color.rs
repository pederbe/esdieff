//! Color is CLI presentation only, following ADR 0011 and ADR 0013.
mod cli_color_support;
use cli_color_support::{plain, run};

struct OutputDirectory(std::path::PathBuf);
impl OutputDirectory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-color-output-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for OutputDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn help_accepts_both_control_forms_and_only_styles_syntax() {
    let ordinary = run(&["--color", "never", "--help"], b"", &[]);
    assert_eq!(ordinary.status.code(), Some(0));
    assert!(!ordinary.stdout.contains(&0x1b));
    for args in [
        vec!["--color", "always", "--help"],
        vec!["--help", "--color=always"],
        vec!["check", "--color", "always", "--help"],
        vec!["--color=always", "author", "--help"],
    ] {
        let output = run(&args, b"", &[]);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert!(output.stderr.is_empty());
        assert!(output.stdout.contains(&0x1b));
        assert_eq!(plain(&output.stdout), plain(&ordinary.stdout));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("\x1b[1mUsage:\x1b[0m"));
        assert!(text.contains("\x1b[36m--color\x1b[0m auto|always|never"));
        assert!(!text.contains("\x1b[36mauto"));
        assert!(text.ends_with("standard input.\n"));
    }
}

#[test]
fn leading_help_preserves_unrelated_trailing_argument_behavior() {
    let help = run(&["--help"], b"", &[]);
    for args in [
        vec!["--help", "check"],
        vec!["--help", "--unknown"],
        vec!["--help", "--color=always", "check"],
    ] {
        let output = run(&args, b"", &[]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_eq!(plain(&output.stdout), plain(&help.stdout));
    }
    let invalid = run(&["--help", "--color=invalid"], b"", &[]);
    assert_eq!(invalid.status.code(), Some(2));
}

#[test]
fn all_commands_scan_controls_before_and_after_the_command() {
    for command in ["check", "inspect", "sample", "author", "diff"] {
        for args in [
            vec!["--color=always", command],
            vec![command, "--color", "always"],
        ] {
            let output = run(&args, b"", &[]);
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            assert!(output.stdout.is_empty());
            assert!(output.stderr.starts_with(b"\x1b[31mesdieff:\x1b[0m "));
            assert_eq!(
                output.stderr.iter().filter(|byte| **byte == 0x1b).count(),
                2
            );
        }
    }
}

#[test]
fn ordinary_scanning_keeps_last_valid_mode_and_does_not_rescan_values() {
    for args in [
        vec!["--color=always", "check", "--no-color", "--unknown"],
        vec![
            "--color=always",
            "check",
            "--color=never",
            "--color=invalid",
        ],
        vec!["--color=never", "check", "--pointer", "--color=always"],
        vec![
            "--color=never",
            "author",
            "--source",
            "--color=always",
            "missing",
        ],
        vec!["--color=never", "check", "--", "--color=always"],
        vec!["--color=never", "author", "--", "--color=always"],
        vec!["--color=never", "inspect", "--pointer", "--color=always"],
        vec!["--color=never", "author", "--plan", "--color=always"],
        vec!["--color=never", "sample", "--sample", "--color=always"],
    ] {
        let output = run(&args, b"", &[]);
        assert!(!output.stdout.contains(&0x1b), "{args:?}");
        assert!(!output.stderr.contains(&0x1b), "{args:?}");
    }
    for args in [
        vec!["--no-color", "check", "--color=always", "--unknown"],
        vec!["--color=always", "--unknown", "--color=never"],
        vec!["check", "--color=always", "--color=invalid"],
        vec!["check", "--color=always", "--color"],
        vec!["author", "--color=always", "--color"],
        vec!["--color=always", "--color=invalid", "check"],
        vec!["--color=always", "--color"],
    ] {
        let output = run(&args, b"", &[]);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(
            output.stderr.starts_with(b"\x1b[31mesdieff:\x1b[0m "),
            "{args:?}"
        );
    }
    let never = run(&["--color=never", "check", "--unknown"], b"", &[]);
    let always = run(&["--color=always", "check", "--unknown"], b"", &[]);
    assert_eq!(plain(&always.stderr), plain(&never.stderr));
    assert!(always.stderr.ends_with(b"with -\n"));
}

#[test]
fn suppression_and_overrides_apply_to_every_generated_style() {
    for environment in [
        vec![],
        vec![("TERM", "dumb")],
        vec![("NO_COLOR", "1")],
        vec![("NO_COLOR", "")],
        vec![("FORCE_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
    ] {
        for mode in ["auto", "never"] {
            let output = run(&["--color", mode, "--help"], b"", &environment);
            assert_eq!(output.status.code(), Some(0));
            assert!(!output.stdout.contains(&0x1b));
        }
        let output = run(&["--color=always", "--help"], b"", &environment);
        assert!(output.stdout.contains(&0x1b));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn automatic_mode_detects_actual_terminals_and_each_stream_independently() {
    use cli_color_support::terminal;
    for environment in [vec![], vec![("NO_COLOR", "")]] {
        let help = terminal(&["--help"], &environment, false, true);
        assert_eq!(help.status.code(), Some(0));
        assert!(help.stdout.contains(&0x1b));
        assert!(help.stderr.is_empty());
        let error = terminal(&["check"], &environment, true, false);
        assert_eq!(error.status.code(), Some(2));
        assert!(error.stdout.is_empty());
        assert!(error.stderr.contains(&0x1b));
    }
    for environment in [vec![("TERM", "dumb")], vec![("NO_COLOR", "1")]] {
        let help = terminal(&["--help"], &environment, false, true);
        assert!(!help.stdout.contains(&0x1b));
        let error = terminal(&["check"], &environment, true, false);
        assert!(!error.stderr.contains(&0x1b));
        let override_help = terminal(&["--color=always", "--help"], &environment, false, true);
        assert!(override_help.stdout.contains(&0x1b));
        let override_error = terminal(&["--color=always", "check"], &environment, true, false);
        assert!(override_error.stderr.contains(&0x1b));
    }
    let redirected = terminal(&["--help"], &[("FORCE_COLOR", "1")], true, true);
    assert!(!redirected.stdout.contains(&0x1b));
    let disabled = terminal(
        &["--no-color", "--help"],
        &[("CLICOLOR_FORCE", "1")],
        false,
        true,
    );
    assert!(!disabled.stdout.contains(&0x1b));
    let restored_auto = terminal(
        &["--no-color", "--color", "auto", "--help"],
        &[],
        false,
        true,
    );
    assert!(restored_auto.stdout.contains(&0x1b));
}

#[test]
fn help_and_report_output_failures_keep_exit_four_and_style_only_the_prefix() {
    for (args, input) in [
        (vec!["--color=always", "--help"], b"".as_slice()),
        (vec!["check", "--color=always", "-"], b"{}".as_slice()),
    ] {
        let output = cli_color_support::closed_stdout(&args, input);
        assert_eq!(output.status.code(), Some(4));
        assert!(
            output
                .stderr
                .starts_with(b"\x1b[31mesdieff:\x1b[0m cannot emit requested ")
        );
        assert_eq!(
            output.stderr.iter().filter(|byte| **byte == 0x1b).count(),
            2
        );
        assert!(output.stderr.ends_with(b"\n"));
    }
}

#[test]
fn json_reports_remain_plain_when_color_is_forced() {
    let output = run(
        &["--color=always", "check", "--format", "json", "-"],
        b"{}",
        &[],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.contains(&0x1b));
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();

    let source = br#"{"sdfData":{"Count":{"type":"integer","maximum":100}}}"#;
    let inspect = run(
        &[
            "inspect",
            "--color=always",
            "--document",
            "-",
            "--pointer",
            "/sdfData/Count",
            "--format",
            "json",
            "-",
        ],
        source,
        &[],
    );
    assert_eq!(inspect.status.code(), Some(0));
    assert!(!inspect.stdout.contains(&0x1b));
    serde_json::from_slice::<serde_json::Value>(&inspect.stdout).unwrap();

    let directory = OutputDirectory::new();
    let model_path = directory.0.join("model.json");
    std::fs::write(&model_path, source).unwrap();
    let model_path = model_path.to_str().unwrap();
    let sample = run(
        &[
            "sample",
            "--color=always",
            "--document",
            model_path,
            "--kind",
            "data",
            "--pointer",
            "/sdfData/Count",
            "--sample",
            "-",
            "--format",
            "json",
            model_path,
        ],
        b"12",
        &[],
    );
    assert_eq!(sample.status.code(), Some(0));
    assert!(!sample.stdout.contains(&0x1b));
    serde_json::from_slice::<serde_json::Value>(&sample.stdout).unwrap();
}

#[test]
fn generated_documents_and_author_json_remain_plain_on_both_destinations() {
    let directory = OutputDirectory::new();
    let plan = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/authoring/refine.plan.json"
    );
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/authoring/reusable.sdf.json"
    );
    let report_path = directory.0.join("report.json");
    let document_path = directory.0.join("document.json");
    for (document, report) in [
        ("-", report_path.to_str().unwrap()),
        (document_path.to_str().unwrap(), "-"),
    ] {
        let output = run(
            &[
                "author",
                "--color=always",
                "--plan",
                plan,
                "--source",
                "base",
                source,
                "--document-output",
                document,
                "--report-output",
                report,
                "--format",
                "json",
            ],
            b"",
            &[],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.contains(&0x1b));
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();
    }
    for file in [document_path, report_path] {
        let bytes = std::fs::read(file).unwrap();
        assert!(!bytes.contains(&0x1b));
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
    }
}

#[test]
fn option_values_and_embedded_inspection_source_stay_plain() {
    let source =
        br#"{"sdfData":{"--color=always":{"type":"string","default":"check --color=never"}}}"#;
    let output = run(
        &[
            "--color=always",
            "inspect",
            "--document",
            "-",
            "--pointer",
            "/sdfData/--color=always",
            "-",
        ],
        source,
        &[],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    // Inspection source JSON stays untouched when human report styling is added.
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("{\"type\":\"string\",\"default\":\"check --color=never\"}")
    );
}
