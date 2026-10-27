//! Acquisition help and invocation styling. Receipt styling uses the offline
//! fixtures inside the binary crate, since this harness cannot serve acquisitions.
mod cli_color_support;
use cli_color_support::{closed_stdout, plain, run};

const STYLED_HELP: &[u8] = b"\x1b[1mUsage:\x1b[0m\n\x1b[36mesdieff\x1b[0m \x1b[36macquire\x1b[0m \x1b[36m--github\x1b[0m OWNER/REPOSITORY";

#[test]
fn acquisition_help_accepts_controls_before_and_after_the_command() {
    let reference = run(&["acquire", "--help"], b"", &[]);
    assert_eq!(reference.status.code(), Some(0));
    assert!(!reference.stdout.contains(&0x1b));
    for args in [
        vec!["--color=always", "acquire", "--help"],
        vec!["--color", "always", "acquire", "--help"],
        vec!["acquire", "--color=always", "--help"],
        vec!["acquire", "--help", "--color", "always"],
        vec!["--no-color", "acquire", "--color=always", "--help"],
    ] {
        let output = run(&args, b"", &[]);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert!(output.stderr.is_empty(), "{args:?}");
        assert!(output.stdout.starts_with(STYLED_HELP), "{args:?}");
        assert_eq!(plain(&output.stdout), plain(&reference.stdout), "{args:?}");
    }
    for args in [
        vec!["--color=always", "acquire", "--no-color", "--help"],
        vec!["acquire", "--color=always", "--color", "never", "--help"],
        vec!["acquire", "--color=auto", "--help"],
    ] {
        let output = run(&args, b"", &[]);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert_eq!(output.stdout, reference.stdout, "{args:?}");
    }
}

#[test]
fn acquisition_help_styles_only_syntax_and_headings() {
    let styled = run(&["acquire", "--color=always", "--help"], b"", &[]);
    let text = String::from_utf8(styled.stdout).unwrap();
    for syntax in [
        "--commit",
        "--path",
        "--sha256",
        "--document-output",
        "--report-output",
        "--format",
        "--color",
        "--no-color",
    ] {
        assert!(
            text.contains(&format!("\x1b[36m{syntax}\x1b[0m")),
            "{syntax}"
        );
    }
    for value in [
        "OWNER/REPOSITORY",
        "DIGEST",
        "human|json",
        "auto|always|never",
    ] {
        assert!(!text.contains(&format!("{value}\x1b[0m")), "{value}");
    }
    assert!(text.contains("\x1b[1mGlobal options:\x1b[0m"));
}

#[test]
fn acquisition_option_values_are_never_rescanned_as_controls() {
    // The values are consumed by their options, so --help still runs with the earlier mode.
    for (args, styled) in [
        (
            vec![
                "--color=never",
                "acquire",
                "--path",
                "--color=always",
                "--help",
            ],
            false,
        ),
        (
            vec![
                "--color=always",
                "acquire",
                "--github",
                "--no-color",
                "--help",
            ],
            true,
        ),
        (vec!["acquire", "--sha256", "--color", "--help"], false),
    ] {
        let output = run(&args, b"", &[]);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert_eq!(output.stdout.contains(&0x1b), styled, "{args:?}");
    }
}

#[test]
fn acquisition_invalid_controls_keep_invocation_errors_and_the_last_valid_mode() {
    for (args, styled) in [
        (vec!["acquire", "--color=always", "--color=sometimes"], true),
        (vec!["acquire", "--color=always", "--color"], true),
        (vec!["acquire", "--no-color", "--color=sometimes"], false),
        (vec!["--color=always", "acquire", "--unknown"], true),
        (vec!["acquire", "--color=always", "--github"], true),
        (
            vec!["--color=always", "acquire", "--no-color", "--unknown"],
            false,
        ),
    ] {
        let output = run(&args, b"", &[]);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(
            output.stderr.starts_with(b"\x1b[31mesdieff:\x1b[0m "),
            styled,
            "{args:?}"
        );
        if !styled {
            assert!(!output.stderr.contains(&0x1b), "{args:?}");
        }
    }
    let invalid = run(&["acquire", "--color=sometimes"], b"", &[]);
    assert_eq!(
        plain(&invalid.stderr),
        "esdieff: color must be auto, always or never\n"
    );
}

#[test]
fn acquisition_help_output_failure_keeps_exit_four_and_styles_only_the_prefix() {
    let output = closed_stdout(&["acquire", "--color=always", "--help"], b"");
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    assert!(
        output
            .stderr
            .starts_with(b"\x1b[31mesdieff:\x1b[0m cannot emit acquisition help: ")
    );
    assert_eq!(
        output.stderr.iter().filter(|byte| **byte == 0x1b).count(),
        2
    );
}

#[test]
fn acquisition_help_honors_environment_suppression_and_overrides() {
    for environment in [
        vec![],
        vec![("TERM", "dumb")],
        vec![("NO_COLOR", "1")],
        vec![("FORCE_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
    ] {
        let redirected = run(&["acquire", "--help"], b"", &environment);
        assert!(!redirected.stdout.contains(&0x1b));
        let never = run(&["acquire", "--color=never", "--help"], b"", &environment);
        assert!(!never.stdout.contains(&0x1b));
        let always = run(&["acquire", "--color=always", "--help"], b"", &environment);
        assert!(always.stdout.starts_with(STYLED_HELP));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn automatic_acquisition_styling_follows_each_actual_stream() {
    use cli_color_support::terminal;
    for environment in [vec![], vec![("NO_COLOR", "")]] {
        let help = terminal(&["acquire", "--help"], &environment, false, true);
        assert_eq!(help.status.code(), Some(0));
        assert!(newlines(&help.stdout).starts_with(STYLED_HELP));
        let redirected = terminal(&["acquire", "--help"], &environment, true, false);
        assert!(!redirected.stdout.contains(&0x1b));
        let error = terminal(&["acquire", "--unknown"], &environment, true, false);
        assert_eq!(error.status.code(), Some(2));
        assert!(error.stderr.starts_with(b"\x1b[31mesdieff:\x1b[0m "));
        let redirected_error = terminal(&["acquire", "--unknown"], &environment, false, true);
        assert!(!redirected_error.stderr.contains(&0x1b));
    }
    for environment in [vec![("TERM", "dumb")], vec![("NO_COLOR", "1")]] {
        let help = terminal(&["acquire", "--help"], &environment, false, true);
        assert!(!help.stdout.contains(&0x1b));
        let forced = terminal(
            &["acquire", "--color=always", "--help"],
            &environment,
            false,
            true,
        );
        assert!(newlines(&forced.stdout).starts_with(STYLED_HELP));
    }
}

/// The terminal translates each newline to CRLF.
#[cfg(target_os = "linux")]
fn newlines(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .into_bytes()
}
