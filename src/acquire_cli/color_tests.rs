use super::support::*;
use super::*;
use crate::cli_style::Style;
use std::{fs, path::Path};

struct Acquired {
    code: u8,
    stdout: Vec<u8>,
    document: Option<Vec<u8>>,
    report: Option<Vec<u8>>,
}

/// One acquisition into a reusable directory, so receipts name identical paths across modes.
fn acquire_in(
    directory: &Path,
    mut color: ColorMode,
    stdout_terminal: bool,
    digest: &str,
    json: bool,
    report_file: bool,
    faults: transport::TestFaults,
) -> Acquired {
    for name in ["document", "receipt"] {
        let _ = fs::remove_file(directory.join(name));
    }
    let document = directory.join("document");
    let report = directory.join("receipt");
    let mut arguments = args(&document, digest);
    if !json {
        arguments.truncate(11);
    }
    if report_file {
        arguments.extend(["--report-output".into(), report.clone().into()]);
    }
    let mut fixture = Fixture::body(b"abc");
    fixture.source.faults = faults;
    let mut stdout = Vec::new();
    let code = acquire(
        &arguments,
        &mut color,
        stdout_terminal,
        &fixture.source,
        &mut stdout,
    )
    .unwrap();
    fixture.finish();
    Acquired {
        code,
        stdout,
        document: fs::read(&document).ok(),
        report: fs::read(&report).ok(),
    }
}

fn plain(bytes: &[u8]) -> String {
    let mut text = String::from_utf8(bytes.to_vec()).unwrap();
    for sequence in [
        "\x1b[0m", "\x1b[1m", "\x1b[31m", "\x1b[32m", "\x1b[33m", "\x1b[36m",
    ] {
        text = text.replace(sequence, "");
    }
    text
}

fn human(directory: &Path, color: ColorMode, terminal: bool, digest: &str) -> Acquired {
    acquire_in(
        directory,
        color,
        terminal,
        digest,
        false,
        false,
        Default::default(),
    )
}

#[test]
fn successful_receipts_style_headings_and_outcome_only() {
    let directory = Temp::new();
    let styled = human(&directory.0, ColorMode::Always, false, ABC);
    let reference = human(&directory.0, ColorMode::Never, false, ABC);
    assert_eq!((styled.code, reference.code), (0, 0));
    assert!(!reference.stdout.contains(&0x1b));
    assert_eq!(
        plain(&styled.stdout),
        String::from_utf8(reference.stdout).unwrap()
    );
    let text = String::from_utf8(styled.stdout).unwrap();
    assert!(text.starts_with(
        "\x1b[1mAcquisition\x1b[0m \x1b[32macquired\x1b[0m\n\x1b[1mRepository\x1b[0m Owner/models\n"
    ));
    for line in [
        "\x1b[1mCommit\x1b[0m 0123456789abcdef0123456789abcdef01234567\n",
        &format!("\x1b[1mActual SHA-256\x1b[0m {ABC}\n"),
        "\x1b[1mComplete\x1b[0m true\n",
        "\x1b[1mDocument committed\x1b[0m true\n",
        "\x1b[1mBytes\x1b[0m 3\n",
    ] {
        assert!(text.contains(line), "{line:?}");
    }
    assert_eq!(styled.document.as_deref(), Some(b"abc".as_slice()));
    assert_eq!(reference.document.as_deref(), Some(b"abc".as_slice()));
}

#[test]
fn failed_receipts_mark_the_outcome_and_each_failure_phase() {
    let directory = Temp::new();
    let styled = human(&directory.0, ColorMode::Always, false, EMPTY);
    let reference = human(&directory.0, ColorMode::Never, false, EMPTY);
    assert_eq!((styled.code, reference.code), (1, 1));
    assert_eq!((styled.document, reference.document), (None, None));
    assert_eq!(
        plain(&styled.stdout),
        String::from_utf8(reference.stdout).unwrap()
    );
    let text = String::from_utf8(styled.stdout).unwrap();
    for line in [
        "\x1b[1mAcquisition\x1b[0m \x1b[31mfailed\x1b[0m\n",
        "\x1b[1mComplete\x1b[0m \x1b[33mfalse\x1b[0m\n",
        "\x1b[1mDocument committed\x1b[0m false\n",
        "\x1b[31mdigest:\x1b[0m complete response SHA-256 differs from requested digest\n",
    ] {
        assert!(text.contains(line), "{line:?}");
    }
}

#[test]
fn failures_after_commitment_still_disclose_the_committed_document() {
    let directory = Temp::new();
    let faults = || transport::TestFaults {
        cleanup: true,
        ..Default::default()
    };
    let styled = acquire_in(
        &directory.0,
        ColorMode::Always,
        true,
        ABC,
        false,
        false,
        faults(),
    );
    let reference = acquire_in(
        &directory.0,
        ColorMode::Never,
        true,
        ABC,
        false,
        false,
        faults(),
    );
    assert_eq!((styled.code, reference.code), (4, 4));
    assert_eq!(styled.document.as_deref(), Some(b"abc".as_slice()));
    let text = String::from_utf8(styled.stdout).unwrap();
    assert!(text.contains("\x1b[1mDocument committed\x1b[0m \x1b[36mtrue\x1b[0m\n"));
    assert!(text.contains("\x1b[31mcleanup:\x1b[0m cannot remove task-created staging"));
    assert!(text.contains("\x1b[1mRetained staging\x1b[0m "));
    // Staging names differ per run, so compare everything before them.
    let before_staging = |text: &str| text.split("Retained staging").next().unwrap().to_owned();
    assert_eq!(
        before_staging(&plain(text.as_bytes())),
        before_staging(&String::from_utf8(reference.stdout).unwrap())
    );
    for entry in fs::read_dir(&directory.0).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().unwrap() != "document" {
            fs::remove_file(path).unwrap();
        }
    }
}

#[test]
fn automatic_styling_follows_the_receipt_destination() {
    let directory = Temp::new();
    let redirected = human(&directory.0, ColorMode::Auto, false, ABC);
    assert!(!redirected.stdout.contains(&0x1b));
    let terminal = human(&directory.0, ColorMode::Auto, true, ABC);
    // The shared controls own TERM and NO_COLOR; the receipt follows their decision.
    assert_eq!(
        terminal.stdout.contains(&0x1b),
        ColorMode::Auto.for_terminal(true).enabled()
    );
    assert_eq!(plain(&terminal.stdout), plain(&redirected.stdout));
    let never = human(&directory.0, ColorMode::Never, true, ABC);
    assert!(!never.stdout.contains(&0x1b));

    let file = |color| {
        acquire_in(
            &directory.0,
            color,
            true,
            ABC,
            false,
            true,
            Default::default(),
        )
    };
    let automatic = file(ColorMode::Auto);
    assert_eq!((automatic.code, automatic.stdout.len()), (0, 0));
    let automatic = automatic.report.unwrap();
    assert!(!automatic.contains(&0x1b));
    let forced = file(ColorMode::Always).report.unwrap();
    assert!(forced.starts_with(b"\x1b[1mAcquisition\x1b[0m \x1b[32macquired"));
    assert_eq!(plain(&forced), plain(&automatic));
    assert!(!file(ColorMode::Never).report.unwrap().contains(&0x1b));
}

#[test]
fn json_receipts_and_documents_are_identical_in_every_mode() {
    let directory = Temp::new();
    for (digest, report_file) in [(ABC, false), (ABC, true), (EMPTY, false)] {
        let runs: Vec<_> = [
            (ColorMode::Never, false),
            (ColorMode::Auto, true),
            (ColorMode::Always, true),
        ]
        .into_iter()
        .map(|(color, terminal)| {
            acquire_in(
                &directory.0,
                color,
                terminal,
                digest,
                true,
                report_file,
                Default::default(),
            )
        })
        .collect();
        for run in &runs[1..] {
            assert_eq!(run.code, runs[0].code);
            assert_eq!(run.stdout, runs[0].stdout);
            assert_eq!(run.report, runs[0].report);
            assert_eq!(run.document, runs[0].document);
        }
        let receipt = runs[0].report.as_ref().unwrap_or(&runs[0].stdout);
        assert!(!receipt.contains(&0x1b));
        serde_json::from_slice::<serde_json::Value>(receipt).unwrap();
    }
}

#[test]
fn styling_that_cannot_fit_the_receipt_ceiling_yields_the_plain_receipt() {
    let directory = Temp::new();
    let document = directory.path("document");
    let arguments = args(&document, EMPTY);
    let request = request::parse(&arguments, &mut ColorMode::default())
        .unwrap()
        .unwrap();
    let receipt = Receipt {
        request: &request,
        actual: Some(ABC.to_owned()),
        count: Some(3),
        committed: false,
        verified_file: None,
        staging: None,
        failures: vec![Failure::new("digest", "differs")],
    };
    let always = ColorMode::Always.for_terminal(false);
    let plain_receipt = receipt.human_within(Style::PLAIN, RECEIPT_BYTES);
    let styled = receipt.human_within(always, RECEIPT_BYTES);
    assert!(styled.len() > plain_receipt.len());
    assert_eq!(plain(styled.as_bytes()), plain_receipt);
    assert_eq!(receipt.human_within(always, styled.len()), styled);
    for limit in [plain_receipt.len(), styled.len() - 1] {
        assert_eq!(receipt.human_within(always, limit), plain_receipt);
    }
}
