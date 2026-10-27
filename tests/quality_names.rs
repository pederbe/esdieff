//! Full Quality Name spelling follows RFC 9880 Section 2.3.3 and Appendix A.
mod location_support;
use esdieff::{
    Assessment, AssessmentStatus, BasisKind, Catalogue, CheckRequest, Limits, Severity, SourceInput,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn catalogue(model: &Value, limits: Limits) -> Catalogue {
    Catalogue::with_limits(
        &[SourceInput::supplied(
            "model",
            &serde_json::to_vec(model).unwrap(),
        )],
        limits,
    )
    .unwrap()
}

#[test]
fn full_rejects_malformed_quality_names_as_assessed_rfc_findings() {
    for name in [
        "",
        "BadName",
        "bad-name",
        "bad_name",
        "bad/name",
        "bad~name",
        "bad name",
        "bad.name",
        "9name",
        "naïve",
        ":quality",
        "Bad:quality",
        "9p:quality",
        "p-x:quality",
        "p:",
        "p:Bad",
        "p:bad-name",
        "p:q:r",
    ] {
        let model = json!({"info":{}, "sdfData":{"D":{"type":"number", name:1}}});
        let catalogue = catalogue(&model, Limits::default());
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(
            report.outcome().errors(),
            1,
            "{name}: {}",
            report.to_human()
        );
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M005/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        let diagnostics = report.diagnostics();
        let finding = diagnostics
            .conditions()
            .find(|finding| finding.code() == "quality.name_syntax")
            .unwrap();
        assert_eq!(finding.severity(), Severity::Error);
        assert_eq!(finding.basis().kind(), BasisKind::Rfc);
        assert_eq!(finding.coverage_owner(), Some("full.document-syntax/1"));
        let source = location_support::primary_source(finding);
        assert_eq!(
            source.pointer().require().unwrap(),
            format!("/sdfData/D/{}", name.replace('~', "~0").replace('/', "~1"))
        );
        assert!(source.span().is_some());
    }
}

#[test]
fn full_checks_quality_positions_without_entering_given_names_or_opaque_values() {
    for model in [
        json!({"info":{}, "bad-name":1, "sdfData":{"D":{}}}),
        json!({"info":{"bad-name":1}, "sdfData":{"D":{}}}),
        json!({"info":{}, "sdfThing":{"T":{"bad-name":1}}}),
        json!({"info":{}, "sdfObject":{"O":{"bad-name":1}}}),
        json!({"info":{}, "sdfProperty":{"P":{"bad-name":1}}}),
        json!({"info":{}, "sdfAction":{"A":{"sdfInputData":{"bad-name":1}}}}),
        json!({"info":{}, "sdfEvent":{"E":{"sdfOutputData":{"bad-name":1}}}}),
        json!({"info":{}, "sdfData":{"D":{"items":{"bad-name":1}}}}),
        json!({"info":{}, "sdfData":{"D":{"sdfChoice":{"branch":{"bad-name":1}}}}}),
        json!({"info":{}, "sdfData":{"D":{"type":"object","properties":{"field":{"bad-name":1}}}}}),
    ] {
        let catalogue = catalogue(&model, Limits::default());
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|finding| finding.code() == "quality.name_syntax"),
            "{}",
            report.to_human()
        );
    }
    let model = json!({
        "info":{}, "namespace":{"Bad-name":"urn:example:model"},
        "sdfData":{
            "Bad-name":{
                "type":"object", "properties":{"Bad-name":{}},
                "const":{"Bad-name":1}, "default":{"Bad-name":1},
                "custom":{"Bad:quality":1}, "p:custom":{"bad-name":1}
            },
            "Choice":{"sdfChoice":{"Bad-name":{}}}
        }
    });
    let catalogue = catalogue(&model, Limits::default());
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert_eq!(report.outcome().errors(), 0, "{}", report.to_human());
    assert_eq!(
        catalogue
            .source(catalogue.source_id("model").unwrap())
            .unwrap()
            .raw_bytes(),
        serde_json::to_vec(&model).unwrap()
    );
}

#[test]
fn admitted_quality_names_keep_unfamiliar_advice_and_optional_notices() {
    for name in ["a", "$", "$custom", "aA0$", "p:q", "p0:$qA9"] {
        let model = json!({"info":{}, "sdfData":{"D":{"type":"number", name:1}}});
        let catalogue = catalogue(&model, Limits::default());
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0, "{name}");
        let expected = if name.contains(':') {
            "extension.optional_unsupported"
        } else {
            "quality.unfamiliar"
        };
        assert!(
            report
                .diagnostics()
                .conditions()
                .any(|finding| finding.code() == expected),
            "{name}"
        );
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M005/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
    }
}

#[test]
fn inherited_quality_findings_retain_effective_and_source_coordinates() {
    let model = json!({"info":{},"sdfData":{
        "Base":{"type":"number","bad-name":1},
        "Use":{"sdfRef":"#/sdfData/Base","minimum":0},
        "Deleted":{"sdfRef":"#/sdfData/Base","bad-name":null}
    }});
    let catalogue = catalogue(&model, Limits::default());
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    let rows = wire["diagnostics"]["items"].as_array().unwrap();
    assert!(rows.iter().any(|row| row["code"] == "quality.name_syntax"
        && row["primary"]["kind"] == "source"
        && row["primary"]["pointer"] == "/sdfData/Base/bad-name"));
    let inherited = rows
        .iter()
        .find(|row| {
            row["code"] == "quality.name_syntax"
                && row["primary"]["kind"] == "effective"
                && row["primary"]["pointer"] == "/sdfData/Use/bad-name"
        })
        .unwrap();
    assert!(
        inherited["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["kind"] == "source"
                && source["pointer"] == "/sdfData/Base/bad-name")
    );
    assert!(!rows.iter().any(|row| row["code"] == "quality.name_syntax"
        && row["primary"]["kind"] == "effective"
        && row["primary"]["pointer"] == "/sdfData/Deleted/bad-name"));
}

#[test]
fn narrower_assessments_keep_existing_vocabulary_behavior() {
    for assessment in [Assessment::Initial, Assessment::ExpandedPatternSyntax] {
        let model =
            json!({"info":{},"sdfData":{"D":{"type":"number","bad-name":1,"Bad:quality":1}}});
        let catalogue = catalogue(&model, Limits::default());
        let report = catalogue
            .check(CheckRequest::new().assessment(assessment))
            .unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(report.outcome().warnings(), 1);
        assert!(
            !report
                .diagnostics()
                .conditions()
                .any(|finding| finding.code() == "quality.name_syntax")
        );
    }
}

#[test]
fn omitted_detail_keeps_quality_errors_and_authoritative_coverage() {
    let model = json!({"info":{},"sdfData":{"D":{"bad-name":1,"Bad:quality":1}}});
    for limits in [
        Limits::default().conditions(1),
        Limits::default().diagnostic_bytes(1),
    ] {
        let catalogue = catalogue(&model, limits);
        let report = catalogue
            .check(CheckRequest::new().assessment(Assessment::FullBaseline))
            .unwrap();
        assert_eq!(report.outcome().errors(), 2);
        assert_eq!(
            report
                .requirement_status("rfc9880.model.M005/1")
                .require()
                .unwrap(),
            AssessmentStatus::Assessed
        );
        assert!(report.diagnostics().truncated());
    }
}

#[test]
fn exhausted_scan_work_keeps_name_coverage_undetermined() {
    let name = format!("p:{}-", "q".repeat(600));
    let model = json!({"info":{},"sdfData":{"D":{name:1}}});
    let catalogue = catalogue(&model, Limits::default().decoding_work(2000));
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    assert!(!report.outcome().complete());
    assert_eq!(
        report
            .requirement_status("rfc9880.model.M005/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
    assert!(
        !report
            .diagnostics()
            .conditions()
            .any(|finding| finding.code() == "quality.name_syntax")
    );
}

#[test]
fn full_quality_errors_fail_both_cli_report_formats() {
    for format in ["human", "json"] {
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
        child
            .stdin
            .take()
            .unwrap()
            .write_all(br#"{"info":{},"sdfData":{"D":{"type":"number","Bad:quality":1}}}"#)
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        if format == "json" {
            let wire: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(wire["outcome"]["errors"], 1);
            assert!(
                wire["diagnostics"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["code"] == "quality.name_syntax" && row["severity"] == "error")
            );
        } else {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("quality.name_syntax")
            );
        }
    }
}
