//! Combined model work through the same public library and CLI interfaces.
use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, SourceInput};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

const ORDINARY: &[u8] = include_bytes!("fixtures/model-meaning/ordinary.sdf.json");
const COMPLETED: [usize; 18] = [
    24, 26, 27, 32, 35, 36, 37, 42, 43, 45, 46, 48, 49, 50, 51, 52, 53, 54,
];

#[test]
fn combined_ordinary_model_completes_without_a_sample_even_when_detail_is_omitted() {
    for limits in [Limits::default(), Limits::default().diagnostic_bytes(1)] {
        let catalogue =
            Catalogue::with_limits(&[SourceInput::supplied("ordinary", ORDINARY)], limits).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        for member in COMPLETED {
            let id = format!("rfc9880.model.M{member:03}/1");
            assert_eq!(
                report.requirement_status(&id).require().unwrap(),
                AssessmentStatus::Assessed,
                "{id}"
            );
        }
        assert!(report.outcome().complete());
        let coverage = report.requirement_coverage().require().unwrap();
        assert_eq!(coverage.total(), Some(54));
        assert_eq!(coverage.unassessed(), 0);
        assert_eq!(coverage.undetermined(), 0);
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["coverage"]["broader"]["state"], "complete");
        assert_eq!(wire["outcome"]["disposition"], "passed");
    }
}

#[test]
fn combined_model_has_matching_cli_completion_and_exit_status() {
    for format in ["json", "human"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_esdieff"))
            .args([
                "check",
                "--assessment",
                "full-baseline",
                "--format",
                format,
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(ORDINARY).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.stderr.is_empty());
        assert_eq!(output.status.code(), Some(0));
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["outcome"]["complete"], true);
            assert_eq!(wire["coverage"]["broader"]["state"], "complete");
            assert_eq!(wire["request"]["inventory"], "esdieff.full-model/1");
        } else {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("Complete: true")
            );
        }
    }
}

#[test]
fn content_format_knowledge_does_not_hide_independent_text_constraints() {
    for (declaration, content_status) in [
        ("text/plain;charset=utf-8", AssessmentStatus::Assessed),
        ("multipart/mixed", AssessmentStatus::Unassessed),
    ] {
        let text = format!(
            r#"{{"info":{{}},"sdfData":{{"D":{{"type":"string","minLength":0,"contentFormat":"{declaration}"}}}}}}"#
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("content", text.as_bytes())]).unwrap();
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        for id in ["rfc9880.model.M042/1", "rfc9880.model.M046/1"] {
            assert_eq!(
                report.requirement_status(id).require().unwrap(),
                AssessmentStatus::Assessed,
                "{id}"
            );
        }
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M024/1")
                .require()
                .unwrap(),
            content_status
        );
    }
}
