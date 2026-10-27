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
fn ordinary_scalar_descriptions_establish_model_meaning_without_a_sample() {
    let catalogue = model(
        br#"{"info":{},"sdfData":{
            "quantity":{"type":"number","minimum":-2.5,"exclusiveMaximum":12,"multipleOf":0.125,"const":10.0},
            "count":{"type":"integer","exclusiveMinimum":-1,"maximum":10},
            "text":{"type":"string","minLength":1,"maxLength":2,"const":"\ud83d\ude00"},
            "flag":{"type":"boolean","const":true},
            "unset":{"const":null}
        }}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    assert_eq!(report.outcome().errors(), 0);
    for member in [42, 43, 45, 46] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    // Formal admission is independently established for these exact values.
    for member in [35, 36, 37] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
}

#[test]
fn established_meaning_is_independent_of_satisfiability_and_formal_membership() {
    let catalogue = model(
        br#"{"info":{},"sdfData":{
            "quantity":{"minimum":3,"maximum":2,"multipleOf":0.1,"const":1},
            "text":{"type":"string","minLength":3,"maxLength":2,"const":false}
        }}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    for member in [42, 43, 45, 46] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
    assert!(report.outcome().warnings() > 0);
}

#[test]
fn unavailable_scalar_values_do_not_erase_independent_type_meaning() {
    for (text, limits, member) in [
        (
            br#"{"info":{},"sdfData":{"D":{"type":"number","minimum":12345}}}"#.as_slice(),
            Limits::default().number_token_bytes(2),
            45,
        ),
        (
            br#"{"info":{},"sdfData":{"D":{"type":"string","const":"\ud800"}}}"#.as_slice(),
            Limits::default(),
            43,
        ),
    ] {
        let catalogue = model(text, limits);
        let report = full(&catalogue);
        assert_eq!(status(&report, member), AssessmentStatus::Undetermined);
        assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
        assert!(!report.outcome().complete());
    }
}

#[test]
fn selected_ordinary_scalar_meaning_survives_reference_composition() {
    let catalogue = model(
        br##"{"info":{},"sdfData":{
            "base":{"type":"number","minimum":0,"maximum":100},
            "bounded":{"sdfRef":"#/sdfData/base","maximum":10,"const":3}
        }}"##,
        Limits::default(),
    );
    let report = full(&catalogue);
    for member in [42, 43, 45] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
}

#[test]
fn final_choice_overrides_supply_constant_meaning() {
    let catalogue = model(
        br#"{"info":{},"sdfData":{"D":{
            "type":"string","const":[0],
            "sdfChoice":{"text":{"const":"value"},"flag":{"type":"boolean","const":true}}
        }}}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
    assert_eq!(status(&report, 43), AssessmentStatus::Assessed);
}

#[test]
fn unknown_interpretations_do_not_gain_scalar_completion() {
    for extension in [
        r#""type":"future""#,
        r#""type":"number","sdfType":"future""#,
        r#""type":"number","format":"future""#,
    ] {
        let source =
            format!(r#"{{"info":{{}},"sdfData":{{"D":{{{extension},"minimum":0,"const":1}}}}}}"#);
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        for member in [42, 43, 45] {
            assert_eq!(
                status(&report, member),
                AssessmentStatus::Undetermined,
                "{source} M{member}"
            );
        }
    }
    // Undeclared extension vocabulary is not an unknown interpretation of the
    // description, following ADR 0037: Undeclared extension vocabulary.
    let catalogue = model(
        br#"{"info":{},"sdfData":{"D":{"type":"number","future:value":true,"minimum":0,"const":1}}}"#,
        Limits::default(),
    );
    let report = full(&catalogue);
    for member in [42, 43, 45] {
        assert_eq!(
            status(&report, member),
            AssessmentStatus::Assessed,
            "M{member}"
        );
    }
}

#[test]
fn restricted_donor_meaning_requires_an_explicit_full_override() {
    for (branch, expected) in [
        ("{}", AssessmentStatus::Undetermined),
        (r#"{"multipleOf":0.25}"#, AssessmentStatus::Assessed),
    ] {
        let branch: serde_json::Value = serde_json::from_str(branch).unwrap();
        let source = serde_json::json!({"info":{},"sdfData":{"D":{"type":"array","items":{
            "type":"number","multipleOf":0.125,"sdfChoice":{"n":branch}
        }}}})
        .to_string();
        let catalogue = model(source.as_bytes(), Limits::default());
        let report = full(&catalogue);
        assert_eq!(status(&report, 45), expected, "{source}");
        assert_eq!(status(&report, 41), AssessmentStatus::Undetermined);
    }
}

#[test]
fn semantic_work_limits_and_detail_omission_keep_distinct_meanings() {
    let source = br#"{"info":{},"sdfData":{"D":{"type":"string","const":false}}}"#;
    let generous = model(source, Limits::default());
    let omitted = model(source, Limits::default().conditions(1).diagnostic_bytes(1));
    for catalogue in [&generous, &omitted] {
        let report = full(catalogue);
        assert!(report.outcome().complete());
        assert_eq!(report.outcome().warnings(), 1);
        assert_eq!(status(&report, 42), AssessmentStatus::Assessed);
        assert_eq!(status(&report, 43), AssessmentStatus::Assessed);
        let initial = catalogue.check(CheckRequest::new()).unwrap();
        assert!(initial.outcome().complete());
        assert!(matches!(
            initial.requirement_status("rfc9880.model.M042/1"),
            esdieff::Field::Absent
        ));
    }
    let wire: serde_json::Value = serde_json::from_str(&full(&omitted).to_json()).unwrap();
    assert_eq!(wire["diagnostics"]["truncated"], true);
    assert!(wire["diagnostics"]["omitted"].as_u64().unwrap() > 0);
    let limited = model(source, Limits::default().decoding_work(1));
    let report = full(&limited);
    assert!(!report.outcome().complete());
    assert_eq!(status(&report, 42), AssessmentStatus::Undetermined);
    assert_eq!(status(&report, 43), AssessmentStatus::Undetermined);
}
