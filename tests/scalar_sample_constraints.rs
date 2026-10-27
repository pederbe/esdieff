//! Scalar constraints follow ADR0015: Bounded value equality in a resolved data context,
//! and ADR0016: Selected samples with explicit admission and coverage.
use esdieff::{
    Admission, AssessmentStatus, Catalogue, Report, SampleKind, SampleRequest, SourceInput,
};

fn catalogue(qualities: &str, sample: &str) -> Catalogue {
    let model = format!(r#"{{"sdfProperty":{{"p":{qualities}}}}}"#);
    Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", sample.as_bytes()).sample(),
    ])
    .unwrap()
}
fn selected(catalogue: &Catalogue) -> Report<'_> {
    catalogue
        .check_sample(
            SampleRequest::new(
                catalogue.source_id("m").unwrap(),
                SampleKind::Property,
                "/sdfProperty/p",
            )
            .sample(catalogue.source_id("s").unwrap()),
        )
        .unwrap()
}
fn admission(qualities: &str, sample: &str, expected: Admission) {
    let catalogue = catalogue(qualities, sample);
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        expected,
        "{qualities} with {sample}"
    );
    let coverage = report.requested_coverage();
    assert_eq!(coverage.total(), Some(9));
    assert_eq!(
        coverage.assessed()
            + coverage.not_applicable()
            + coverage.unassessed()
            + coverage.undetermined(),
        9
    );
}
#[test]
fn scalar_constants_use_exact_ordinary_values_without_inserting_type() {
    for (qualities, sample, expected) in [
        (r#"{"const":true}"#, "true", Admission::Admitted),
        (r#"{"const":true}"#, "false", Admission::Rejected),
        (r#"{"const":true}"#, "1", Admission::Rejected),
        (r#"{"const":null}"#, "null", Admission::Admitted),
        (r#"{"const":null}"#, "0", Admission::Rejected),
        (r#"{"const":10}"#, "10.0", Admission::Admitted),
        (r#"{"const":10}"#, "1e1", Admission::Admitted),
        (r#"{"const":-0}"#, "0", Admission::Admitted),
        (r#"{"const":1e1000000}"#, "1.0e1000000", Admission::Admitted),
        (
            r#"{"const":9007199254740992}"#,
            "9007199254740993",
            Admission::Rejected,
        ),
        (
            r#"{"const":0.1}"#,
            "0.10000000000000001",
            Admission::Rejected,
        ),
        (r#"{"const":"on"}"#, r#""\u006fn""#, Admission::Admitted),
        (r#"{"const":"é"}"#, r#""e\u0301""#, Admission::Rejected),
    ] {
        admission(qualities, sample, expected);
    }
}
#[test]
fn nullable_and_matching_constants_do_not_bypass_applicable_constraints() {
    for (qualities, sample, expected) in [
        (
            r#"{"type":"number","const":null,"nullable":false}"#,
            "null",
            Admission::Rejected,
        ),
        (
            r#"{"type":"number","const":1}"#,
            "null",
            Admission::Rejected,
        ),
        (r#"{"const":10,"maximum":5}"#, "10", Admission::Rejected),
        (r#"{"const":10,"multipleOf":3}"#, "10", Admission::Rejected),
        (
            r#"{"const":"on","minLength":3}"#,
            r#""on""#,
            Admission::Rejected,
        ),
        (
            r#"{"const":"on","pattern":"^on$"}"#,
            r#""on""#,
            Admission::Admitted,
        ),
        (
            r#"{"const":"on","pattern":"^off$"}"#,
            r#""on""#,
            Admission::Rejected,
        ),
        (
            r#"{"const":"on","pattern":"(?<x>on)|(?<x>off)"}"#,
            r#""on""#,
            Admission::Undetermined,
        ),
        (
            r#"{"const":null,"maximum":0,"minLength":10}"#,
            "null",
            Admission::Admitted,
        ),
        (
            r#"{"type":"number","default":10,"const":10}"#,
            "null",
            Admission::Rejected,
        ),
    ] {
        admission(qualities, sample, expected);
    }
}
#[test]
fn enum_occurrences_are_effective_text_constants_even_without_type() {
    for (sample, expected) in [
        (r#""on""#, Admission::Admitted),
        (r#""\u006fn""#, Admission::Admitted),
        (r#""other""#, Admission::Rejected),
        ("null", Admission::Rejected),
    ] {
        admission(r#"{"enum":["on","off","on"]}"#, sample, expected);
    }
}
#[test]
fn one_complete_branch_witness_survives_an_irrelevant_unknown_branch() {
    admission(
        r#"{"type":"string","sdfChoice":{"ordinary":{},"unknown":{"sdfRef":false}}}"#,
        r#""x""#,
        Admission::Admitted,
    );
    admission(
        r#"{"type":"string","sdfChoice":{"ordinary":{"const":"x"},"unknown":{"sdfRef":false}}}"#,
        r#""x""#,
        Admission::Undetermined,
    );
}
#[test]
fn shared_interpretation_is_established_before_a_favorable_constant_result() {
    admission(
        r#"{"type":"string","sdfChoice":{"ordinary":{"const":"x"},"dated":{"format":"date-time","const":"x"}}}"#,
        r#""x""#,
        Admission::Undetermined,
    );
    admission(
        r#"{"sdfChoice":{"boolean":{"type":"boolean"},"dated":{"type":"string","format":"future-format","const":"x"}}}"#,
        r#""x""#,
        Admission::Undetermined,
    );
    admission(
        r#"{"sdfChoice":{"boolean":{"type":"boolean"},"dated":{"type":"string","format":"date-time","const":"x"}}}"#,
        r#""x""#,
        Admission::Rejected,
    );
    admission(
        r#"{"const":true,"format":"date-time"}"#,
        "true",
        Admission::Undetermined,
    );
    admission(
        r#"{"sdfChoice":{"ordinary":{"type":"number","const":10},"dated":{"type":"string","format":"date-time","const":"2020-01-01T00:00:00Z"}}}"#,
        "1e1",
        Admission::Admitted,
    );
    admission(
        r#"{"sdfChoice":{"ordinary":{"type":"string","const":"x"},"dated":{"type":"string","format":"date-time","const":"x"}}}"#,
        r#""x""#,
        Admission::Undetermined,
    );
    admission(
        r#"{"sdfChoice":{"ordinary":{"type":"number","const":10},"unknown":{"type":"string","format":"vendor","const":"x"}}}"#,
        "1e1",
        Admission::Undetermined,
    );
    admission(
        r#"{"sdfChoice":{"ordinary":{"type":"number","const":10},"dated":{"type":"string","format":"date-time","sdfType":"vendor","const":"x"}}}"#,
        "1e1",
        Admission::Undetermined,
    );
}
#[test]
fn effective_overrides_and_whole_branches_control_admission() {
    for (qualities, sample, expected) in [
        (
            r#"{"type":"number","sdfChoice":{"a":{"const":10},"b":{"const":20}}}"#,
            "1e1",
            Admission::Admitted,
        ),
        (
            r#"{"type":"number","sdfChoice":{"a":{"const":10},"b":{"const":20}}}"#,
            "30",
            Admission::Rejected,
        ),
        (
            r#"{"type":"number","maximum":5,"sdfChoice":{"small":{},"wide":{"maximum":20}}}"#,
            "10",
            Admission::Admitted,
        ),
        (
            r#"{"type":"number","const":5,"sdfChoice":{"small":{},"wide":{"const":10}}}"#,
            "10",
            Admission::Admitted,
        ),
        (
            r#"{"type":"vendor","sdfChoice":{"explicit":{"type":"number","const":10}}}"#,
            "10",
            Admission::Admitted,
        ),
        (
            r#"{"type":"number","sdfChoice":{"a":{"minimum":20,"maximum":30},"b":{"minimum":0,"maximum":5}}}"#,
            "10",
            Admission::Rejected,
        ),
        (
            r#"{"type":"number","sdfChoice":{"a":{"minimum":20,"maximum":30},"b":{"sdfRef":false}}}"#,
            "10",
            Admission::Undetermined,
        ),
        (
            r#"{"type":"number","sdfChoice":{"outer":{"sdfChoice":{"low":{"maximum":5},"high":{"minimum":20}}},"middle":{"minimum":6,"maximum":19}}}"#,
            "10",
            Admission::Admitted,
        ),
        (
            r#"{"type":"number","sdfChoice":{"outer":{"sdfChoice":{"low":{"maximum":5},"high":{"minimum":20}}}}}"#,
            "10",
            Admission::Rejected,
        ),
    ] {
        admission(qualities, sample, expected);
    }
}
#[test]
fn compound_or_unavailable_meaning_is_scoped_and_never_raw_representation_equality() {
    admission(r#"{"const":{}}"#, "0", Admission::Rejected);
    admission(r#"{"const":{}}"#, r#"{}"#, Admission::Admitted);
    admission(
        r##"{"const":{"sdfRef":"#/missing","type":"magic","sdfChoice":{"x":{}}}}"##,
        r#"{}"#,
        Admission::Rejected,
    );
    admission(
        r#"{"type":"magic","const":true}"#,
        "true",
        Admission::Undetermined,
    );
    admission(
        r#"{"items":{"format":"date-time"},"const":true}"#,
        "true",
        Admission::Undetermined,
    );
    admission(
        r#"{"properties":{"x":{"format":"date-time"}},"const":true}"#,
        "true",
        Admission::Undetermined,
    );
}
#[test]
fn tentative_branch_errors_are_not_top_level_errors_for_another_witness_or_unknown() {
    for (other, expected) in [
        (r#"{"maximum":20}"#, Admission::Admitted),
        (r#"{"sdfRef":false}"#, Admission::Undetermined),
    ] {
        let qualities = format!(
            r#"{{"type":"number","sdfChoice":{{"first":{{"maximum":5}},"other":{other}}}}}"#
        );
        let catalogue = catalogue(&qualities, "10");
        let report = selected(&catalogue);
        assert_eq!(report.sample_result().unwrap().admission(), expected);
        assert_eq!(report.outcome().errors(), 0);
        assert_eq!(
            report
                .sample_result()
                .unwrap()
                .area_status("sample-alternatives/1")
                .require()
                .unwrap(),
            if expected == Admission::Admitted {
                AssessmentStatus::Assessed
            } else {
                AssessmentStatus::Undetermined
            }
        );
    }
}
