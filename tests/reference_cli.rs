//! Markdown reference selection, acquisition, output and exit codes.
use esdieff::{Catalogue, ReferenceRequest, SourceInput};
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};
const THERMOSTAT: &str = "examples/thermostat-check/rich.sdf.json";
#[test]
fn uses_selects_shared_data_and_rejects_duplicate_flag() {
    let shared = "examples/shared-definitions/shared.sdf.json";
    let devices = "examples/shared-definitions/devices.sdf.json";
    let output = run(&[
        "reference",
        "--uses",
        "--document",
        shared,
        "--pointer",
        "/sdfData/Temperature",
        shared,
        devices,
    ]);
    assert_eq!(output.status.code(), Some(0), "{:?}", output);
    let markdown = String::from_utf8(output.stdout).unwrap();
    assert!(markdown.contains("Shared definition uses"));
    assert!(markdown.contains("/sdfObject/Cool/sdfProperty/temperature/maximum"));
    assert!(markdown.contains("Effective value: 40"));
    let output = run(&[
        "reference",
        "--uses",
        "--uses",
        "--document",
        shared,
        "--pointer",
        "/sdfData/Temperature",
        shared,
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(args)
        .output()
        .unwrap()
}
fn stdin(model: &[u8], pointer: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args(["reference", "--document", "-", "--pointer", pointer, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(model).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn thermostat_cli_equals_library_and_never_styles_markdown() {
    let output = run(&[
        "reference",
        "--document",
        THERMOSTAT,
        "--pointer",
        "/sdfObject/Thermostat",
        "--color",
        "always",
        THERMOSTAT,
    ]);
    assert_eq!(output.status.code(), Some(0), "{:?}", output);
    let catalogue =
        Catalogue::from_inputs(&[
            SourceInput::file("input1", std::path::Path::new(THERMOSTAT)).label(THERMOSTAT),
        ])
        .unwrap();
    let result = catalogue
        .reference(ReferenceRequest::new(
            catalogue.source_id("input1").unwrap(),
            "/sdfObject/Thermostat",
        ))
        .unwrap();
    assert_eq!(output.stdout, result.to_markdown().as_bytes());
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.contains(&27));
}
#[test]
fn required_and_duplicate_options_invalid_selectors_and_inputs() {
    for args in [
        vec!["reference", THERMOSTAT],
        vec!["reference", "--document", THERMOSTAT, THERMOSTAT],
        vec![
            "reference",
            "--document",
            THERMOSTAT,
            "--document",
            THERMOSTAT,
            "--pointer",
            "/sdfObject/Thermostat",
            THERMOSTAT,
        ],
        vec![
            "reference",
            "--document",
            THERMOSTAT,
            "--pointer",
            "/sdfObject/Thermostat",
            "--pointer",
            "",
            THERMOSTAT,
        ],
        vec![
            "reference",
            "--document",
            THERMOSTAT,
            "--pointer",
            "/bad~2",
            THERMOSTAT,
        ],
        vec![
            "reference",
            "--document",
            THERMOSTAT,
            "--pointer",
            "/sdfObject/missing",
            THERMOSTAT,
        ],
        vec![
            "reference",
            "--document",
            THERMOSTAT,
            "--pointer",
            "/sdfObject/Thermostat/sdfProperty/currentTemperature",
            THERMOSTAT,
        ],
        vec![
            "reference",
            "--document",
            "not-supplied",
            "--pointer",
            "/sdfObject/Thermostat",
            THERMOSTAT,
        ],
        vec![
            "reference",
            "--document",
            "-",
            "--pointer",
            "/sdfObject/Thermostat",
            "-",
            "-",
        ],
        vec!["reference", "--format", "json", THERMOSTAT],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty());
    }
}
#[test]
fn stdin_and_domain_failures_emit_markdown() {
    assert_eq!(
        stdin(
            include_bytes!("../examples/thermostat-check/rich.sdf.json"),
            "/sdfObject/Thermostat"
        )
        .status
        .code(),
        Some(0)
    );
    let output = stdin(b"{", "/sdfObject/Thermostat");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Definite requested processing failure")
    );
    let output = run(&[
        "reference",
        "--document",
        "missing-reference-input.sdf.json",
        "--pointer",
        "/sdfObject/X",
        "missing-reference-input.sdf.json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stdout.is_empty());
    let output = stdin(br#"{"namespace":{"missing":"https://example.invalid/missing"},"sdfObject":{"Device":{"sdfRef":"missing:#/sdfObject/X"}}}"#, "/sdfObject/Device");
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert!(!output.stdout.is_empty());
}
#[test]
fn supplied_dependencies_have_no_first_or_last_precedence() {
    let base = "examples/context-reuse/base.sdf.json";
    let device = "examples/context-reuse/device.sdf.json";
    for inputs in [[base, device], [device, base]] {
        let output = run(&[
            "reference",
            "--document",
            device,
            "--pointer",
            "/sdfThing/Installation",
            "--",
            inputs[0],
            inputs[1],
        ]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(String::from_utf8(output.stdout).unwrap().contains("Office"));
    }
}
#[test]
fn help_and_failed_output_writing() {
    let output = run(&["reference", "--help"]);
    let text = String::from_utf8(output.stdout).unwrap();
    for text_part in [
        "reference",
        "--document MODEL",
        "Markdown",
        "supplied dependencies",
        "does not check",
    ] {
        assert!(text.contains(text_part));
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .args([
            "reference",
            "--document",
            THERMOSTAT,
            "--pointer",
            "/sdfObject/Thermostat",
            THERMOSTAT,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(4), "{output:?}");
}
