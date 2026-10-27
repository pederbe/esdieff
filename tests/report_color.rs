//! Human styling preserves the independent facts in ADR 0011 and ADR 0013.
mod cli_color_support;
use cli_color_support::{closed_stdout, plain, run};
use esdieff::{
    Assessment, Catalogue, CheckRequest, Disposition, HumanStyle, InspectRequest, Limits,
    SampleKind, SampleRequest, SourceInput, View,
};

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(bytes: &[u8]) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-report-color-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn styled(report: &esdieff::Report<'_>) -> String {
    let default = report.to_human();
    assert!(!default.contains('\x1b'));
    assert_eq!(default, report.to_human_with_style(HumanStyle::Plain));
    let output = report.to_human_with_style(HumanStyle::Ansi);
    assert_eq!(plain(output.as_bytes()), default);
    output
}

#[test]
fn outcomes_coverage_and_severities_retain_their_independent_roles() {
    for strict in [false, true] {
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("passed error", b"{}")]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().strict_warnings(strict))
            .unwrap();
        assert!(report.outcome().complete());
        let output = styled(&report);
        assert!(output.starts_with(if strict {
            "\x1b[1mEsdieff:\x1b[0m \x1b[31mfailed\x1b[0m\n"
        } else {
            "\x1b[1mEsdieff:\x1b[0m \x1b[32mpassed\x1b[0m\n"
        }));
        assert!(output.contains("\x1b[33mmetadata.info_absent\x1b[0m: "));
        assert!(output.contains("\x1b[1mBroader RFC coverage:\x1b[0m partial\n"));
        assert!(output.contains("[\"passed error\"]"));
        assert!(!output.contains("\x1b[32massessed"));
        assert!(!output.contains("\x1b[32mnot_applicable"));
        for condition in report.diagnostics().conditions() {
            let sequence = match condition.severity() {
                esdieff::Severity::Error => "31",
                esdieff::Severity::Warning | esdieff::Severity::Incomplete => "33",
                esdieff::Severity::Notice => "36",
                _ => unreachable!(),
            };
            assert!(output.contains(&format!("\x1b[{sequence}m{}\x1b[0m: ", condition.code())));
        }
    }
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("bad", b"[]"),
        SourceInput::failed("missing"),
    ])
    .unwrap();
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().disposition(), Disposition::Failed);
    assert!(!report.outcome().complete());
    let output = styled(&report);
    assert!(output.contains("\x1b[31mfailed\x1b[0m"));
    assert!(output.contains("\x1b[1mComplete:\x1b[0m \x1b[33mfalse\x1b[0m"));
    let diagnostics = report.diagnostics();
    let error = diagnostics
        .conditions()
        .find(|condition| condition.severity() == esdieff::Severity::Error)
        .unwrap();
    assert!(output.contains(&format!("\x1b[31m{}\x1b[0m", error.code())));
}

#[test]
fn assessment_completion_is_plain_even_when_completed_work_has_errors() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "m",
        br#"{"info":{},"sdfData":{"D":{"unit":"urn:ietf:params:unit:Cel"}}}"#,
    )])
    .unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let output = styled(&report);
    assert_eq!(report.outcome().disposition(), Disposition::Failed);
    assert!(output.contains(" unit-spelling/1: assessed\n"));
    assert!(!output.contains("\x1b[32massessed"));
    assert!(!output.contains("\x1b[32mcomplete"));
}

#[test]
fn notice_codes_and_unfinished_requiredness_keep_their_separate_roles() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "m",
        br#"{"sdfObject":{"O":{"sdfRequired":["missing"]}}}"#,
    )])
    .unwrap();
    let report = catalogue
        .inspect(
            InspectRequest::new(catalogue.source_id("m").unwrap(), "/sdfObject/O")
                .required_declarations(true),
        )
        .unwrap();
    let output = styled(&report);
    assert!(output.contains("\x1b[36mrequired.target_missing\x1b[0m: "));
    assert!(output.contains("\x1b[1mRequired declarations:\x1b[0m"));
}

#[test]
fn source_and_effective_inspection_keep_payload_names_and_exact_values_plain() {
    let text = br#"{"info":{},"sdfData":{"passed":{"type":"number","const":1.000e+200,"description":"failed error warning passed"}}}"#;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("error passed", text)]).unwrap();
    let id = catalogue.source_id("error passed").unwrap();
    for view in [View::Source, View::Effective] {
        let report = catalogue
            .inspect(InspectRequest::new(id, "/sdfData/passed").view(view))
            .unwrap();
        let output = styled(&report);
        assert!(output.contains("\x1b[1mInspection\x1b[0m"));
        assert!(output.contains("{\"type\":\"number\",\"const\":1.000e+200,\"description\":\"failed error warning passed\"}"));
        assert!(output.contains("\"/sdfData/passed\""));
    }
    let unavailable = Catalogue::from_inputs(&[SourceInput::failed("missing")]).unwrap();
    let report = unavailable
        .inspect(InspectRequest::new(
            unavailable.source_id("missing").unwrap(),
            "",
        ))
        .unwrap();
    assert_eq!(report.outcome().disposition(), Disposition::Undetermined);
    assert!(styled(&report).contains("\x1b[33mundetermined\x1b[0m"));
}

#[test]
fn samples_style_admission_without_turning_assessed_or_inapplicable_into_validity() {
    for (input, admission, disposition, sequence) in [
        (Some(b"2".as_slice()), "admitted", Disposition::Passed, "32"),
        (Some(b"0".as_slice()), "rejected", Disposition::Failed, "31"),
        (None, "undetermined", Disposition::Undetermined, "33"),
    ] {
        let model = br#"{"info":{},"sdfData":{"passed":{"type":"integer","minimum":1}}}"#;
        let sample = input
            .map_or_else(
                || SourceInput::failed("sample"),
                |bytes| SourceInput::supplied("sample", bytes),
            )
            .sample();
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("model", model), sample]).unwrap();
        let report = catalogue
            .check_sample(
                SampleRequest::new(
                    catalogue.source_id("model").unwrap(),
                    SampleKind::Data,
                    "/sdfData/passed",
                )
                .sample(catalogue.source_id("sample").unwrap()),
            )
            .unwrap();
        assert_eq!(report.outcome().disposition(), disposition);
        let output = styled(&report);
        assert!(output.contains(&format!("\x1b[{sequence}m{admission}\x1b[0m; context ")));
        assert!(!output.contains("\x1b[32massessed"));
        assert!(!output.contains("\x1b[32mnot_applicable"));
        assert!(output.contains("Sample area"));
    }
}

#[test]
fn color_modes_preserve_reports_and_json_for_all_three_commands() {
    let model =
        InputFile::new(br#"{"info":{},"sdfData":{"passed":{"type":"integer","minimum":1}}}"#);
    let sample = InputFile::new(b"0");
    let admitted = InputFile::new(b"2");
    let bad = InputFile::new(b"[]");
    let duplicate = InputFile::new(br#"{"x":1,"x":2}"#);
    let missing = InputFile::new(b"");
    std::fs::remove_file(&missing.0).unwrap();
    let broken_ref =
        InputFile::new(br##"{"info":{},"sdfData":{"passed":{"sdfRef":"#/sdfData/missing"}}}"##);
    for (args, exit) in [
        (vec!["check", model.path()], 0),
        (vec!["check", bad.path()], 1),
        (vec!["check", missing.path()], 3),
        (
            vec![
                "inspect",
                "--document",
                model.path(),
                "--pointer",
                "/sdfData/passed",
                model.path(),
            ],
            0,
        ),
        (
            vec![
                "inspect",
                "--view",
                "effective",
                "--document",
                model.path(),
                "--pointer",
                "/sdfData/passed",
                model.path(),
            ],
            0,
        ),
        (
            vec![
                "inspect",
                "--document",
                missing.path(),
                "--pointer",
                "",
                missing.path(),
            ],
            3,
        ),
        (
            vec![
                "inspect",
                "--view",
                "effective",
                "--document",
                missing.path(),
                "--pointer",
                "",
                missing.path(),
            ],
            3,
        ),
        (
            vec![
                "inspect",
                "--document",
                duplicate.path(),
                "--pointer",
                "",
                duplicate.path(),
            ],
            1,
        ),
        (
            vec![
                "inspect",
                "--view",
                "effective",
                "--document",
                duplicate.path(),
                "--pointer",
                "",
                duplicate.path(),
            ],
            1,
        ),
        (
            vec![
                "inspect",
                "--view",
                "effective",
                "--document",
                broken_ref.path(),
                "--pointer",
                "/sdfData/passed",
                broken_ref.path(),
            ],
            3,
        ),
        (
            vec![
                "sample",
                "--document",
                model.path(),
                "--kind",
                "data",
                "--pointer",
                "/sdfData/passed",
                "--sample",
                sample.path(),
                model.path(),
            ],
            1,
        ),
        (
            vec![
                "sample",
                "--document",
                model.path(),
                "--kind",
                "data",
                "--pointer",
                "/sdfData/passed",
                "--sample",
                admitted.path(),
                model.path(),
            ],
            0,
        ),
        (
            vec![
                "sample",
                "--document",
                model.path(),
                "--kind",
                "data",
                "--pointer",
                "/sdfData/passed",
                "--sample",
                missing.path(),
                model.path(),
            ],
            3,
        ),
    ] {
        let mut plain_args = args.clone();
        plain_args.push("--color=never");
        let expected = run(&plain_args, b"", &[]);
        assert_eq!(expected.status.code(), Some(exit), "{args:?}");
        assert!(expected.stderr.is_empty());
        assert!(!expected.stdout.contains(&0x1b));
        for mode in ["auto", "always", "never"] {
            let color = format!("--color={mode}");
            let mut selected = args.clone();
            selected.push(&color);
            let output = run(&selected, b"", &[("NO_COLOR", "1"), ("TERM", "dumb")]);
            assert_eq!(output.status.code(), expected.status.code());
            assert_eq!(plain(&output.stdout), plain(&expected.stdout));
            assert_eq!(output.stdout.contains(&0x1b), mode == "always");
            assert!(output.stderr.is_empty());
            selected.extend(["--format", "json"]);
            let mut plain_json_args = plain_args.clone();
            plain_json_args.extend(["--format", "json"]);
            let json = run(&selected, b"", &[]);
            let expected_json = run(&plain_json_args, b"", &[]);
            assert_eq!(json.stdout, expected_json.stdout);
            assert!(!json.stdout.contains(&0x1b));
        }
    }
    let closed = closed_stdout(&["check", "--color=always", model.path()], b"");
    assert_eq!(closed.status.code(), Some(4));
}

#[cfg(target_os = "linux")]
#[test]
fn automatic_report_styling_uses_actual_stdout_destination() {
    use cli_color_support::terminal;
    let model = InputFile::new(br#"{"info":{},"sdfData":{"passed":{"type":"integer"}}}"#);
    let sample = InputFile::new(b"2");
    for args in [
        vec!["check", model.path()],
        vec![
            "inspect",
            "--document",
            model.path(),
            "--pointer",
            "/sdfData/passed",
            model.path(),
        ],
        vec![
            "inspect",
            "--view",
            "effective",
            "--document",
            model.path(),
            "--pointer",
            "/sdfData/passed",
            model.path(),
        ],
        vec![
            "sample",
            "--document",
            model.path(),
            "--kind",
            "data",
            "--pointer",
            "/sdfData/passed",
            "--sample",
            sample.path(),
            model.path(),
        ],
    ] {
        for (environment, styled) in [
            (vec![], true),
            (vec![("NO_COLOR", "")], true),
            (vec![("NO_COLOR", "1")], false),
            (vec![("TERM", "dumb")], false),
        ] {
            let output = terminal(&args, &environment, false, true);
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(output.stdout.contains(&0x1b), styled);
            assert!(output.stderr.is_empty());
        }
        let redirected = terminal(&args, &[], true, false);
        assert!(!redirected.stdout.contains(&0x1b));
        let mut forced_args = args.clone();
        forced_args.push("--color=always");
        let forced = terminal(&forced_args, &[], true, false);
        assert!(forced.stdout.contains(&0x1b));
    }
}

#[test]
fn a_tight_style_budget_preserves_the_same_retained_payload_and_omissions() {
    let text = format!("{{\"passed\":\"{}\"}}", "warning error ".repeat(1600));
    let inputs = [SourceInput::supplied("model", text.as_bytes())];
    let catalogue = Catalogue::from_inputs(&inputs).unwrap();
    let id = catalogue.source_id("model").unwrap();
    let plain_report = catalogue
        .inspect(InspectRequest::new(id, ""))
        .unwrap()
        .to_human();
    let limit = plain_report.len() + 128;
    let limited = Catalogue::with_limits(&inputs, Limits::default().report_bytes(limit)).unwrap();
    let report = limited.inspect(InspectRequest::new(id, "")).unwrap();
    let ordinary = report.to_human();
    let requested = report.to_human_with_style(HumanStyle::Ansi);
    assert_eq!(
        requested, ordinary,
        "ANSI overhead must fall back to the same plain report"
    );
    assert!(ordinary.contains(&text));
    assert_eq!(ordinary, plain_report);
    assert!(ordinary.len() <= limit);
    assert_eq!(report.outcome().disposition(), Disposition::Passed);
}
