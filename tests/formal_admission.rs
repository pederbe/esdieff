use esdieff::{Assessment, AssessmentStatus, Catalogue, CheckRequest, Limits, Report, SourceInput};

fn model(text: &[u8], limits: Limits) -> Catalogue {
    Catalogue::with_limits(&[SourceInput::supplied("model", text)], limits).unwrap()
}
fn full(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap()
}
fn status(report: &Report<'_>, member: u16) -> AssessmentStatus {
    report
        .requirement_status(&format!("rfc9880.model.M{member:03}/1"))
        .require()
        .unwrap()
}

#[test]
fn formal_number_membership_admits_every_json_number() {
    for value in [
        "0",
        "-0",
        "10.0",
        "100e-1",
        "18446744073709551616",
        "-18446744073709551617",
        "0.5",
        "0.1",
        "5e-2",
        "1.275",
        "1e400",
        "1e-400",
        "1e1000000",
        "1e-1000000",
    ] {
        let source =
            format!(r#"{{"info":{{}},"sdfData":{{"D":{{"type":"number","minimum":{value}}}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 36), AssessmentStatus::Assessed, "{value}");
        assert_eq!(report.outcome().errors(), 0, "{value}");
        assert_eq!(status(&report, 45), AssessmentStatus::Assessed, "{value}");
        assert_eq!(
            catalogue
                .check(CheckRequest::new())
                .unwrap()
                .outcome()
                .errors(),
            0
        );
    }
}

#[test]
fn uint_counts_are_bounded_by_value_without_expanding_exponents() {
    for (value, admitted) in [
        ("-0", true),
        ("10.0", true),
        ("1e1", true),
        ("18446744073709551615", true),
        ("18446744073709551616", false),
        ("0.5", false),
        ("-1", false),
        ("1e1000000", false),
    ] {
        let source =
            format!(r#"{{"info":{{}},"sdfData":{{"D":{{"type":"string","minLength":{value}}}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 37), AssessmentStatus::Assessed, "{value}");
        assert_eq!(report.outcome().errors() > 0, !admitted, "{value}");
    }
}

#[test]
fn allowed_literal_extension_admission_does_not_invent_semantics() {
    for (literal, base) in [
        ("0.5", true),
        ("0.1", true),
        ("1e400", true),
        ("18446744073709551615", true),
        ("[]", true),
        ("[0.5,2]", true),
        ("[0.1]", true),
        (r#"["a","b"]"#, true),
        ("[true,false]", true),
        (r#"{"x":[[0.1]],"n":1e400}"#, true),
        ("[null]", false),
        (r#"[1,"x"]"#, false),
        ("[[1]]", false),
        ("[{}]", false),
    ] {
        let source = format!(r#"{{"info":{{}},"sdfData":{{"D":{{"const":{literal}}}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(report.outcome().errors(), 0, "{literal}");
        assert_eq!(status(&report, 35), AssessmentStatus::Assessed, "{literal}");
        if !base {
            assert_eq!(
                status(&report, 28),
                AssessmentStatus::Undetermined,
                "{literal}"
            );
            assert_eq!(
                status(&report, 43),
                AssessmentStatus::Undetermined,
                "{literal}"
            );
        }
    }
}

#[test]
fn grouping_fallback_preserves_framework_admission_and_narrower_profiles() {
    for value in ["-1", "0.5", "18446744073709551616", "true", "null", "{}"] {
        let source = format!(r#"{{"info":{{}},"sdfObject":{{"O":{{"minItems":{value}}}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(report.outcome().errors(), 0, "{value}");
        assert_eq!(status(&report, 37), AssessmentStatus::Assessed, "{value}");
        assert_eq!(
            status(&report, 32),
            AssessmentStatus::Undetermined,
            "{value}"
        );
        assert_eq!(
            status(&report, 28),
            AssessmentStatus::Undetermined,
            "{value}"
        );
        let initial = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(
            initial.outcome().errors() > 0,
            value != "18446744073709551616",
            "{value}"
        );
    }
    let catalogue = model(
        br#"{"info":{},"sdfObject":{"O":{"minItems":10.0,"maxItems":18446744073709551615}}}"#,
        Limits::default(),
    );
    for member in [32, 37] {
        assert_eq!(
            status(&full(&catalogue), member),
            AssessmentStatus::Assessed
        );
    }
}

#[test]
fn decoding_and_numeric_budget_limits_are_unavailable_membership() {
    let source = br#"{"info":{},"sdfData":{"D":{"type":"number","minimum":12345}}}"#;
    let catalogue = model(source, Limits::default().number_token_bytes(2));
    let report = full(&catalogue);
    assert_eq!(status(&report, 36), AssessmentStatus::Undetermined);
    assert_eq!(report.outcome().errors(), 0);
    assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
    let source = br#"{"info":{},"sdfData":{"D":{"type":"string","maxLength":12345}}}"#;
    let catalogue = model(source, Limits::default().number_token_bytes(2));
    let report = full(&catalogue);
    assert_eq!(status(&report, 37), AssessmentStatus::Undetermined);
    assert_eq!(report.outcome().errors(), 0);
    for literal in ["12345", "[12345]"] {
        let source = format!(r#"{{"info":{{}},"sdfData":{{"D":{{"const":{literal}}}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default().number_token_bytes(2));
        let report = full(&catalogue);
        assert_eq!(
            status(&report, 35),
            AssessmentStatus::Undetermined,
            "{literal}"
        );
        assert_eq!(report.outcome().errors(), 0, "{literal}");
    }
}

#[test]
fn inherited_formal_findings_use_effective_locations_and_original_operands() {
    let source = br##"{"info":{},"sdfData":{"Base":{"maxLength":18446744073709551616},"D":{"sdfRef":"#/sdfData/Base"}}}"##;
    let catalogue = model(source, Limits::default());
    let report = full(&catalogue);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let finding = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["code"] == "formal.uint" && item["primary"]["pointer"] == "/sdfData/D/maxLength"
        })
        .unwrap();
    assert_eq!(finding["primary"]["kind"], "effective");
    assert_eq!(finding["related"][0]["pointer"], "/sdfData/Base/maxLength");
    let span = finding["related"][0]["span"].as_array().unwrap();
    assert_eq!(
        &source[span[0].as_u64().unwrap() as usize..span[1].as_u64().unwrap() as usize],
        b"18446744073709551616"
    );
}

fn decimal_multiply(mut digits: Vec<u8>, factor: u16, times: usize) -> String {
    for _ in 0..times {
        let mut carry = 0u16;
        for digit in digits.iter_mut().rev() {
            let value = u16::from(*digit - b'0') * factor + carry;
            *digit = (value % 10) as u8 + b'0';
            carry = value / 10;
        }
        while carry > 0 {
            digits.insert(0, (carry % 10) as u8 + b'0');
            carry /= 10;
        }
    }
    String::from_utf8(digits).unwrap()
}

#[test]
fn binary64_extremes_are_numbers_and_decode_limits_remain_distinct() {
    for value in [
        decimal_multiply(b"9007199254740991".to_vec(), 2, 971),
        decimal_multiply(b"1".to_vec(), 2, 1024),
        format!("{}e-700", decimal_multiply(b"1".to_vec(), 5, 700)),
        "1e22".to_owned(),
        "1e23".to_owned(),
    ] {
        let source = format!(r#"{{"info":{{}},"sdfData":{{"D":{{"minimum":{value}}}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 36), AssessmentStatus::Assessed, "{value}");
        assert_eq!(report.outcome().errors(), 0, "{value}");
    }
    let tiny = format!("{}e-1074", decimal_multiply(b"1".to_vec(), 5, 1074));
    let source = format!(r#"{{"info":{{}},"sdfData":{{"D":{{"minimum":{tiny}}}}}}}"#);
    let catalogue = model(source.as_bytes(), Limits::default());
    let report = full(&catalogue);
    assert_eq!(status(&report, 36), AssessmentStatus::Undetermined);
    assert_eq!(report.outcome().errors(), 0);
}

#[test]
fn extension_constants_leave_only_needed_meaning_unfinished() {
    let catalogue=model(br#"{"info":{},"sdfData":{"D":{"type":"object","properties":{"optional":{"type":"number","const":[null]}}}}}"#,Limits::default());
    let report = full(&catalogue);
    assert_eq!(status(&report, 35), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 43), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 53), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
    let catalogue=model(br#"{"info":{},"sdfData":{"D":{"type":"string","const":[null],"sdfChoice":{"a":{"const":"text"},"b":{"const":true,"type":"boolean"}}}}}"#,Limits::default());
    let report = full(&catalogue);
    assert_eq!(status(&report, 43), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 28), AssessmentStatus::NotApplicable);
}

#[test]
fn selected_composition_excludes_unused_and_deleted_formal_operands() {
    let donor=br#"{"info":{},"namespace":{"d":"https://example.test/d"},"defaultNamespace":"d","sdfData":{"Base":{"maxLength":18446744073709551616},"unused":{"maxLength":18446744073709551616}}}"#;
    for local in [r#""maxLength":null"#, r#""maxLength":5"#] {
        let consumer = format!(
            r##"{{"info":{{}},"namespace":{{"d":"https://example.test/d"}},"sdfData":{{"D":{{"sdfRef":"d:#/sdfData/Base",{local}}}}}}}"##
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("donor", donor),
            SourceInput::supplied("consumer", consumer.as_bytes()),
        ])
        .unwrap();
        let report = catalogue
            .check(
                CheckRequest::new()
                    .root(catalogue.source_id("consumer").unwrap())
                    .assessment(Assessment::FullBaseline),
            )
            .unwrap();
        assert_eq!(report.outcome().errors(), 0, "{local}");
        assert_eq!(
            status(&report, 37),
            if local.contains("null") {
                AssessmentStatus::NotApplicable
            } else {
                AssessmentStatus::Assessed
            }
        );
        assert!(full(&catalogue).outcome().errors() > 0);
    }
}

#[test]
fn optional_detail_omission_preserves_formal_findings_and_statuses() {
    let source = br#"{"info":{},"sdfData":{"D":{"type":"string","minLength":18446744073709551616,"maxLength":18446744073709551617}}}"#;
    for limits in [
        Limits::default(),
        Limits::default().conditions(1).diagnostic_bytes(1),
    ] {
        let catalogue = model(source, limits);
        let report = full(&catalogue);
        assert_eq!(report.outcome().errors(), 2);
        assert!(report.outcome().complete());
        assert_eq!(status(&report, 37), AssessmentStatus::Assessed);
        assert_eq!(status(&report, 46), AssessmentStatus::Assessed);
    }
}
