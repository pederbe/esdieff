use std::io::Write;
use std::process::{Command, Stdio};

fn convert(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_terminal-html"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn plain_unicode_and_prompt() {
    assert_eq!(
        convert("$ esdieff check café.sdf.json\n<温度 & 'value' >\n  $ text"),
        "<pre class=\"terminal\"><code><span class=\"p\">$</span> esdieff check café.sdf.json\n&lt;温度 &amp; 'value' &gt;\n  $ text</code></pre>\n"
    );
    assert_eq!(convert(""), "<pre class=\"terminal\"><code></code></pre>\n");
}

#[test]
fn styles_reset_together_and_stay_balanced_across_lines() {
    assert_eq!(
        convert(
            "\x1b[1;31mfailed\nagain\x1b[0m plain \x1b[32mOK\x1b[39m\n\x1b[1mheading\x1b[36m syntax\x1b[22m cyan\x1b[m\x1b[33munfinished"
        ),
        "<pre class=\"terminal\"><code><b><span class=\"r\">failed\nagain</span></b> plain <span class=\"g\">OK</span>\n<b>heading</b><b><span class=\"c\"> syntax</span></b><span class=\"c\"> cyan</span><span class=\"y\">unfinished</span></code></pre>\n"
    );
}

#[test]
fn unsupported_controls_do_not_become_styles_or_text() {
    assert_eq!(
        convert(
            "\x1b[2K\x1b[4;31mred\x1b[0m \x1b[38;2;31;32;33mplain\x1b[48;5;31m too\x1b[0m\n\x1b[1m$ check\x1b[0m"
        ),
        "<pre class=\"terminal\"><code><span class=\"r\">red</span> plain too\n<b><span class=\"p\">$</span> check</b></code></pre>\n"
    );
}

#[test]
fn captured_esdieff_outputs_keep_the_existing_html() {
    // Captured with --color=always: check examples/thermostat-check/thermostat.sdf.json,
    // and the first three lines of --help. Expected HTML comes from the old launcher.
    for (input, expected) in [
        (
            include_str!("fixtures/check.ansi"),
            include_str!("fixtures/check.html"),
        ),
        (
            include_str!("fixtures/help.ansi"),
            include_str!("fixtures/help.html"),
        ),
    ] {
        assert_eq!(convert(input), expected);
    }
}
