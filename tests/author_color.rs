//! Authoring presentation preserves the independent authoring and checking facts.
mod cli_color_support;

use cli_color_support::plain;
use esdieff::{
    Assessment, AuthorObject, AuthorRequest, Catalogue, HumanStyle, Limits, SourceInput,
};
use std::{fs, path::PathBuf};

const SOURCE: &str = r#"{"namespace":{"b":"https://example.com/base"},"defaultNamespace":"b","sdfObject":{"Sensor":{"sdfProperty":{"reading":{"type":"integer"}}}}}"#;

fn request(source: &str, limits: Limits) -> AuthorRequest {
    let catalogue =
        Catalogue::with_limits(&[SourceInput::supplied("base", source.as_bytes())], limits)
            .unwrap();
    let source = catalogue.source_id("base").unwrap();
    AuthorRequest::new(catalogue, "error recipe", "product", "failed")
        .unwrap()
        .namespace("reuse", "https://example.com/base")
        .unwrap()
        .object(AuthorObject::new("warning", source, "/sdfObject/Sensor", "reuse", false).unwrap())
        .unwrap()
}

#[test]
fn author_style_is_explicit_and_leaves_names_and_unrequested_checking_plain() {
    let result = request(SOURCE, Limits::default()).author().unwrap();
    let ordinary = result.to_human();
    assert_eq!(ordinary, result.to_human_with_style(HumanStyle::Plain));
    let styled = result.to_human_with_style(HumanStyle::Ansi);
    assert_eq!(plain(styled.as_bytes()), ordinary);
    assert!(styled.contains("\x1b[1mAuthor\x1b[0m failed: \x1b[32mpassed\x1b[0m"));
    assert!(styled.contains("\x1b[1mGeneration:\x1b[0m \x1b[32mcomplete\x1b[0m"));
    assert!(styled.contains("\x1b[1mFidelity:\x1b[0m \x1b[32mestablished\x1b[0m"));
    assert!(styled.contains("\x1b[1mChecking:\x1b[0m not requested"));
    assert!(styled.contains("\x1b[1mBroader RFC coverage:\x1b[0m partial\n"));
    assert!(!styled.contains("\x1b[31mfailed"));
    assert!(!ordinary.contains('\x1b'));
}

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "esdieff-author-color-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("base.json"), source).unwrap();
        fs::write(path.join("plan.json"), r#"{"key":"error recipe","generated_key":"product","thing_name":"failed","namespace":{"reuse":"https://example.com/base"},"objects":[{"name":"warning","source":"base","pointer":"/sdfObject/Sensor","prefix":"reuse","required":false}]}"#).unwrap();
        Self(path)
    }

    fn args(&self, document: &str, report: &str, format: &str, mode: &str) -> Vec<String> {
        let destination = |name: &str| {
            if name == "-" {
                name.to_owned()
            } else {
                self.0.join(name).to_str().unwrap().to_owned()
            }
        };
        let mut args = vec![
            "author".to_owned(),
            format!("--color={mode}"),
            "--plan".to_owned(),
            self.0.join("plan.json").to_str().unwrap().to_owned(),
            "--source".to_owned(),
            "base".to_owned(),
            self.0.join("base.json").to_str().unwrap().to_owned(),
            "--document-output".to_owned(),
            destination(document),
            "--report-output".to_owned(),
            destination(report),
            "--format".to_owned(),
            format.to_owned(),
        ];
        if document != "-" {
            args.push("--replace-document".to_owned());
        }
        if report != "-" {
            args.push("--replace-report".to_owned());
        }
        args
    }

    fn run(&self, args: &[String], environment: &[(&str, &str)]) -> std::process::Output {
        cli_color_support::run(
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            b"",
            environment,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn color_controls_only_change_human_reports_and_never_generated_sdf_or_json_reports() {
    let fixture = Fixture::new(SOURCE);
    let never = fixture.run(&fixture.args("document.json", "-", "human", "never"), &[]);
    assert!(never.status.success(), "{:?}", never.stderr);
    let document = fs::read(fixture.0.join("document.json")).unwrap();
    for mode in ["auto", "always", "never"] {
        let output = fixture.run(&fixture.args("document.json", "-", "human", mode), &[]);
        assert!(output.status.success());
        assert_eq!(plain(&output.stdout), plain(&never.stdout));
        assert_eq!(output.stdout.contains(&0x1b), mode == "always");
        assert_eq!(fs::read(fixture.0.join("document.json")).unwrap(), document);
    }
    let never = fixture.run(&fixture.args("document.json", "-", "json", "never"), &[]);
    assert!(never.status.success());
    for mode in ["auto", "always", "never"] {
        let output = fixture.run(&fixture.args("document.json", "-", "json", mode), &[]);
        assert!(output.status.success());
        assert_eq!(output.stdout, never.stdout);
        assert_eq!(fs::read(fixture.0.join("document.json")).unwrap(), document);
    }
    let report = fixture.run(&fixture.args("document.json", "-", "human", "never"), &[]);
    for mode in ["auto", "always", "never"] {
        let output = fixture.run(&fixture.args("-", "report.txt", "human", mode), &[]);
        assert!(output.status.success());
        assert_eq!(output.stdout, document);
        let report_file = fs::read(fixture.0.join("report.txt")).unwrap();
        assert_eq!(report_file.contains(&0x1b), mode == "always");
        assert_eq!(plain(&report_file), plain(&report.stdout));
        let output = fixture.run(&fixture.args("-", "report.json", "json", mode), &[]);
        assert!(output.status.success());
        assert_eq!(output.stdout, document);
        assert_eq!(
            fs::read(fixture.0.join("report.json")).unwrap(),
            never.stdout
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn automatic_author_styling_uses_the_report_destination_and_shared_environment_policy() {
    use cli_color_support::terminal;
    let fixture = Fixture::new(SOURCE);
    let args = fixture.args("document.json", "-", "human", "auto");
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    let ordinary = fixture.run(&fixture.args("document.json", "-", "human", "never"), &[]);
    for (environment, styled) in [
        (vec![], true),
        (vec![("NO_COLOR", "")], true),
        (vec![("NO_COLOR", "1")], false),
        (vec![("TERM", "dumb")], false),
    ] {
        let output = terminal(&args, &environment, false, true);
        assert!(output.status.success());
        assert_eq!(output.stdout.contains(&0x1b), styled);
        assert_eq!(plain(&output.stdout), plain(&ordinary.stdout));
        assert!(output.stderr.is_empty());
    }
    let redirected = terminal(&args, &[], true, false);
    assert!(redirected.status.success());
    assert!(!redirected.stdout.contains(&0x1b));
    assert_eq!(redirected.stdout, ordinary.stdout);

    let args = fixture.args("-", "report.txt", "human", "auto");
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    let output = terminal(&args, &[], false, true);
    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    assert!(
        serde_json::from_str::<serde_json::Value>(&plain(&output.stdout)).unwrap()["sdfThing"]
            .is_object()
    );
    assert_eq!(
        fs::read(fixture.0.join("report.txt")).unwrap(),
        ordinary.stdout
    );
    for (mode, environment, styled) in [
        ("always", vec![("NO_COLOR", "1"), ("TERM", "dumb")], true),
        (
            "never",
            vec![("FORCE_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
            false,
        ),
        (
            "auto",
            vec![("FORCE_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
            false,
        ),
    ] {
        let output = fixture.run(
            &fixture.args("-", "report.txt", "human", mode),
            &environment,
        );
        assert!(output.status.success());
        assert!(!output.stdout.contains(&0x1b));
        let report = fs::read(fixture.0.join("report.txt")).unwrap();
        assert_eq!(report.contains(&0x1b), styled);
        assert_eq!(plain(&report), plain(&ordinary.stdout));
    }
}

#[test]
fn optional_checking_failures_and_fidelity_limits_remain_visible_together() {
    let source = SOURCE.replace(
        "\"type\":\"integer\"",
        "\"type\":\"integer\",\"minimum\":\"cold\",\"vendor:meaning\":{}",
    );
    let result = request(&source, Limits::default())
        .checking(Assessment::Initial)
        .unwrap()
        .author()
        .unwrap();
    assert_eq!(result.generation(), esdieff::Generation::Complete);
    assert_eq!(result.fidelity(), esdieff::Fidelity::Undetermined);
    assert_eq!(result.outcome().disposition(), esdieff::Disposition::Failed);
    assert!(!result.document_deliverable());
    let ordinary = result.to_human();
    let styled = result.to_human_with_style(HumanStyle::Ansi);
    assert_eq!(plain(styled.as_bytes()), ordinary);
    assert!(styled.contains("\x1b[32mcomplete\x1b[0m"));
    assert!(styled.contains("\x1b[1mFidelity:\x1b[0m \x1b[33mundetermined\x1b[0m"));
    assert!(styled.contains("esdieff.initial/1: \x1b[31mfailed\x1b[0m"));
    assert!(styled.contains("\x1b[31merror\x1b[0m "));
    assert!(styled.contains("\x1b[33mincomplete\x1b[0m author.fidelity_unknown"));
    assert!(!styled.contains("\x1b[31merror recipe"));
    assert!(!styled.contains("\x1b[32massessed"));

    let fixture = Fixture::new(&source);
    let plan = fs::read_to_string(fixture.0.join("plan.json")).unwrap().replace(
        "\"required\":false",
        "\"required\":false,\"refinement\":{\"sdfProperty\":{\"reading\":{\"minimum\":\"cold\"}}}",
    );
    fs::write(fixture.0.join("plan.json"), plan).unwrap();
    let mut reference = None;
    for mode in ["auto", "always", "never"] {
        let mut args = fixture.args("document.json", "-", "human", mode);
        args.extend(["--check".to_owned(), "initial-v1".to_owned()]);
        let output = fixture.run(&args, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(!fixture.0.join("document.json").exists());
        let report = plain(&output.stdout);
        assert!(report.contains("Author request error recipe at /objects/0"));
        if let Some(reference) = &reference {
            assert_eq!(&report, reference);
        } else {
            reference = Some(report);
        }
    }
}

#[test]
fn generation_limits_and_report_omissions_preserve_the_plain_facts() {
    let local = r#"{"sdfObject":{"Sensor":{}}}"#;
    for (source, limits, expected_code) in [
        (local, Limits::default(), "author.local_only_object"),
        (
            SOURCE,
            Limits::default().catalogue_bytes(SOURCE.len() + 40),
            "author.generation_limit",
        ),
    ] {
        let result = request(source, limits).author().unwrap();
        assert_eq!(result.generation(), esdieff::Generation::Unavailable);
        assert_eq!(
            result.outcome().disposition(),
            esdieff::Disposition::Undetermined
        );
        assert!(result.generated_document().is_none());
        let ordinary = result.to_human();
        let styled = result.to_human_with_style(HumanStyle::Ansi);
        assert_eq!(plain(styled.as_bytes()), ordinary);
        assert!(styled.contains("\x1b[1mGeneration:\x1b[0m \x1b[33munavailable\x1b[0m"));
        assert!(styled.contains(expected_code));
    }
    let source = SOURCE.replace(
        "\"type\":\"integer\"",
        "\"type\":\"integer\",\"minimum\":\"cold\",\"vendor:meaning\":{}",
    );
    let limit = 40_000;
    let result = request(
        &source,
        Limits::default().report_bytes(limit).diagnostic_bytes(1),
    )
    .checking(Assessment::Initial)
    .unwrap()
    .author()
    .unwrap();
    let ordinary = result.to_human();
    let styled = result.to_human_with_style(HumanStyle::Ansi);
    assert_eq!(plain(styled.as_bytes()), ordinary);
    assert!(ordinary.contains("Diagnostic details omitted:"));
    assert!(ordinary.contains("Errors: 1"));
    assert!(ordinary.len() <= limit);
    assert!(styled.len() <= limit);
    assert_eq!(result.outcome().disposition(), esdieff::Disposition::Failed);
    assert!(!ordinary.contains("error formal."));
}

#[test]
fn delivery_failures_preserve_exit_status_disclosure_and_only_style_the_failure_prefix() {
    let fixture = Fixture::new(SOURCE);
    for mode in ["auto", "always", "never"] {
        for (document, report, document_delivered) in
            [("-", "report.txt", false), ("document.json", "-", true)]
        {
            let args = fixture.args(document, report, "human", mode);
            let args: Vec<_> = args.iter().map(String::as_str).collect();
            let output = cli_color_support::closed_stdout(&args, b"");
            assert_eq!(output.status.code(), Some(4));
            assert!(output.stdout.is_empty());
            let diagnostic = plain(&output.stderr);
            assert_eq!(diagnostic.contains("already delivered"), document_delivered);
            assert_eq!(output.stderr.contains(&0x1b), mode == "always");
            if mode == "always" {
                assert!(output.stderr.starts_with(b"\x1b[31mesdieff:\x1b[0m "));
                assert_eq!(
                    output.stderr.iter().filter(|&&byte| byte == 0x1b).count(),
                    2
                );
            }
            if document_delivered {
                let bytes = fs::read(fixture.0.join(document)).unwrap();
                assert!(!bytes.contains(&0x1b));
                assert!(
                    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["sdfThing"]
                        .is_object()
                );
            } else {
                assert!(!fixture.0.join(report).exists());
            }
        }
    }
}
