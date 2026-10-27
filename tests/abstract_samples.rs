//! Selected abstract channels preserve exact inputs and independent obligations.
use esdieff::{
    Admission, AssessmentStatus, Catalogue, JsonBuilder, JsonKind, Limits, Report, SampleKind,
    SampleRequest, SourceInput,
};
use serde_json::Value;

const BYTE_ID: &str = "byte-string-json/1";
const POSIX_ID: &str = "unix-time-posix-coordinate/1";
const AREAS: [&str; 9] = [
    "sample-context/1",
    "sample-type-null/1",
    "sample-numeric/1",
    "sample-text-boolean/1",
    "sample-const/1",
    "sample-alternatives/1",
    "sample-map/1",
    "sample-array/1",
    "sample-abstract-values/1",
];

#[derive(Clone, Copy)]
enum Input<'a> {
    Text(&'a str),
    Number(&'a str),
    Null,
}
impl Input<'_> {
    fn typed(self) -> esdieff::JsonValue {
        let builder = JsonBuilder::new();
        match self {
            Self::Text(text) => builder.text(text).unwrap(),
            Self::Number(lexeme) => builder.number(lexeme).unwrap(),
            Self::Null => builder.null().unwrap(),
        }
    }
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
fn inputs(model: &[u8], raw: &[u8], typed: Input<'_>, projection: bool) -> Catalogue {
    Catalogue::take_inputs(vec![
        SourceInput::supplied("m", model),
        if projection {
            SourceInput::typed("s", typed.typed())
        } else {
            SourceInput::supplied("s", raw).sample()
        },
    ])
    .unwrap()
}
fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}
fn used(value: &Value, id: &str) -> bool {
    value["baseline"]["interpretations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry.as_str() == Some(id))
}
fn assert_shared(report: &Report<'_>, expected: Admission, projection: bool, raw_len: usize) {
    let sample = report.sample_result().unwrap();
    assert_eq!(sample.admission(), expected);
    let coverage = report.requested_coverage();
    assert_eq!(coverage.inventory(), Some("esdieff.sample/1"));
    assert_eq!(coverage.count_unit(), "sample_area");
    assert_eq!(coverage.total(), Some(9));
    assert_eq!(
        coverage.assessed()
            + coverage.not_applicable()
            + coverage.unassessed()
            + coverage.undetermined(),
        9
    );
    let value = wire(report);
    assert_eq!(value["version"], "1.1");
    assert_eq!(value["operation"], "sample");
    assert_eq!(value["result"]["admission"], expected.as_str());
    assert_eq!(value["result"]["selector"]["pointer"], "/sdfProperty/p");
    assert_eq!(
        value["result"]["sample_location"]["source_id"],
        sample.sample_id().get()
    );
    assert_eq!(value["result"]["sample_location"]["pointer"], "");
    if projection {
        assert!(value["result"]["sample_location"]["span"].is_null());
    } else {
        assert_eq!(
            value["result"]["sample_location"]["span"],
            serde_json::json!([0, raw_len])
        );
    }
    let human = report.to_human();
    assert!(human.contains(&format!(
        "Sample {}: {}",
        sample.sample_id().get(),
        expected.as_str()
    )));
    assert!(human.contains(&format!("Complete: {}", report.outcome().complete())));
    for id in AREAS {
        let status = sample.area_status(id).require().unwrap();
        if let Some(row) = value["coverage"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
        {
            assert_eq!(row["status"], status.as_str());
        }
        assert_eq!(report.assessment_area_status(id).require().unwrap(), status);
    }
    for id in [BYTE_ID, POSIX_ID] {
        assert_eq!(
            human.contains(id),
            used(&value, id),
            "actual-use agreement for {id}"
        );
    }
}
fn exercise(
    qualities: &str,
    raw: &str,
    typed: Input<'_>,
    expected: Admission,
    complete: bool,
    interpretation: Option<&str>,
) {
    let model = format!(r#"{{"info":{{}},"sdfProperty":{{"p":{qualities}}}}}"#);
    for projection in [false, true] {
        let catalogue = inputs(model.as_bytes(), raw.as_bytes(), typed, projection);
        let report = selected(&catalogue);
        assert_shared(&report, expected, projection, raw.len());
        assert_eq!(
            report.outcome().complete(),
            complete,
            "{qualities} with {raw}"
        );
        let value = wire(&report);
        if let Some(id) = interpretation {
            assert!(used(&value, id));
            let abstract_row = value["coverage"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == "sample-abstract-values/1")
                .unwrap();
            if abstract_row["status"] == "assessed" {
                assert_eq!(abstract_row["basis"]["kind"], "interpretation");
                assert_eq!(abstract_row["basis"]["id"], id);
            }
        } else {
            assert!(!used(&value, BYTE_ID));
            assert!(!used(&value, POSIX_ID));
        }
        if matches!(typed, Input::Number(_)) {
            assert_eq!(
                report
                    .sample_result()
                    .unwrap()
                    .sample_value()
                    .require()
                    .unwrap()
                    .number()
                    .require()
                    .unwrap()
                    .lexeme(),
                raw
            );
        }
        if let Input::Text(decoded) = typed {
            let actual = report
                .sample_result()
                .unwrap()
                .sample_value()
                .require()
                .unwrap();
            assert_eq!(
                actual.text().require().unwrap().as_bytes(),
                decoded.as_bytes(),
                "original and typed inputs must denote the same decoded text"
            );
        }
    }
}

#[test]
fn canonical_bytes_use_decoded_text_and_admit_empty_sequences() {
    for (raw, decoded, expected) in [
        (r#""""#, "", Admission::Admitted),
        (r#""Zg""#, "Zg", Admission::Admitted),
        (r#""Zm8""#, "Zm8", Admission::Admitted),
        (r#""Zm9v""#, "Zm9v", Admission::Admitted),
        (r#""_w""#, "_w", Admission::Admitted),
        (r#""-w""#, "-w", Admission::Admitted),
        (r#""__8""#, "__8", Admission::Admitted),
        (r#""\u005fw""#, "_w", Admission::Admitted),
        (r#""A""#, "A", Admission::Rejected),
        (r#""Zg==""#, "Zg==", Admission::Rejected),
        (r#""Zg=""#, "Zg=", Admission::Rejected),
        (r#""/w""#, "/w", Admission::Rejected),
        (r#""+w""#, "+w", Admission::Rejected),
        (r#""Z g""#, "Z g", Admission::Rejected),
        (r#""Zg\n""#, "Zg\n", Admission::Rejected),
        (r#""é""#, "é", Admission::Rejected),
        (r#""_x""#, "_x", Admission::Rejected),
        (r#""__9""#, "__9", Admission::Rejected),
    ] {
        exercise(
            r#"{"type":"string","sdfType":"byte-string"}"#,
            raw,
            Input::Text(decoded),
            expected,
            true,
            Some(BYTE_ID),
        );
    }
}

#[test]
fn exact_posix_coordinate_input_is_distinct_from_negative_civil_denotation() {
    for lexeme in [
        "0",
        "-0",
        "0.1",
        "1.5",
        "1e-1000000",
        "1e1000000",
        "9007199254740993",
        "18446744073709551616",
    ] {
        exercise(
            r#"{"type":"number","sdfType":"unix-time"}"#,
            lexeme,
            Input::Number(lexeme),
            Admission::Admitted,
            true,
            Some(POSIX_ID),
        );
    }
    // ADR 0007: Generated SDF and sample JSON keeps negative coordinates exact
    // while their application denotation remains unresolved.
    for lexeme in ["-1", "-0.5", "-1e1000000"] {
        exercise(
            r#"{"type":"number","sdfType":"unix-time"}"#,
            lexeme,
            Input::Number(lexeme),
            Admission::Undetermined,
            false,
            Some(POSIX_ID),
        );
    }
}

#[test]
fn equality_establishes_both_abstract_operands_before_const() {
    for (qualities, raw, typed, expected, complete, id) in [
        (
            r#"{"type":"string","sdfType":"byte-string","const":"Zg"}"#,
            r#""\u005ag""#,
            Input::Text("Zg"),
            Admission::Admitted,
            true,
            BYTE_ID,
        ),
        (
            r#"{"type":"string","sdfType":"byte-string","const":""}"#,
            r#""""#,
            Input::Text(""),
            Admission::Admitted,
            true,
            BYTE_ID,
        ),
        (
            r#"{"type":"string","sdfType":"byte-string","const":"_w"}"#,
            r#""-w""#,
            Input::Text("-w"),
            Admission::Rejected,
            true,
            BYTE_ID,
        ),
        (
            r#"{"type":"string","sdfType":"byte-string","const":"_x"}"#,
            r#""_w""#,
            Input::Text("_w"),
            Admission::Undetermined,
            false,
            BYTE_ID,
        ),
        (
            r#"{"type":"string","sdfType":"byte-string","const":"_x"}"#,
            r#""_x""#,
            Input::Text("_x"),
            Admission::Rejected,
            false,
            BYTE_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":10}"#,
            "1e1",
            Input::Number("1e1"),
            Admission::Admitted,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":-0}"#,
            "0",
            Input::Number("0"),
            Admission::Admitted,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":0.1}"#,
            "0.10000000000000001",
            Input::Number("0.10000000000000001"),
            Admission::Rejected,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":9007199254740992}"#,
            "9007199254740993",
            Input::Number("9007199254740993"),
            Admission::Rejected,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":1e1000000}"#,
            "1.0e1000000",
            Input::Number("1.0e1000000"),
            Admission::Admitted,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","const":-1}"#,
            "-1",
            Input::Number("-1"),
            Admission::Undetermined,
            false,
            POSIX_ID,
        ),
    ] {
        exercise(qualities, raw, typed, expected, complete, Some(id));
    }
}

#[test]
fn compatible_constraints_and_scoped_byte_lengths_keep_their_meaning() {
    exercise(
        r#"{"sdfType":"byte-string"}"#,
        r#""Zg""#,
        Input::Text("Zg"),
        Admission::Admitted,
        true,
        Some(BYTE_ID),
    );
    exercise(
        r#"{"sdfType":"unix-time"}"#,
        "1.5",
        Input::Number("1.5"),
        Admission::Admitted,
        true,
        Some(POSIX_ID),
    );
    for (qualities, raw, typed, expected, complete, id) in [
        (
            r#"{"type":"string","sdfType":"byte-string","maxLength":1}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            BYTE_ID,
        ),
        (
            r#"{"type":"string","sdfType":"byte-string","minLength":2}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            BYTE_ID,
        ),
        (
            r#"{"type":"integer","sdfType":"unix-time"}"#,
            "10.0",
            Input::Number("10.0"),
            Admission::Admitted,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"integer","sdfType":"unix-time"}"#,
            "0.5",
            Input::Number("0.5"),
            Admission::Rejected,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","exclusiveMaximum":10}"#,
            "10",
            Input::Number("10"),
            Admission::Rejected,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","multipleOf":0.1}"#,
            "0.3",
            Input::Number("0.3"),
            Admission::Admitted,
            true,
            POSIX_ID,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","multipleOf":0.1}"#,
            "0.31",
            Input::Number("0.31"),
            Admission::Rejected,
            true,
            POSIX_ID,
        ),
    ] {
        exercise(qualities, raw, typed, expected, complete, Some(id));
    }
}

#[test]
fn no_use_is_claimed_for_null_ordinary_values_or_unavailable_context() {
    for (qualities, raw, typed, expected, complete) in [
        (
            r#"{"type":"string","sdfType":"byte-string"}"#,
            "null",
            Input::Null,
            Admission::Admitted,
            true,
        ),
        (
            r#"{"type":"number","sdfType":"unix-time","nullable":false}"#,
            "null",
            Input::Null,
            Admission::Rejected,
            true,
        ),
        (
            r#"{"type":"string"}"#,
            r#""notbase64""#,
            Input::Text("notbase64"),
            Admission::Admitted,
            true,
        ),
        (
            r#"{"type":"number","const":10}"#,
            "1e1",
            Input::Number("1e1"),
            Admission::Admitted,
            true,
        ),
        (
            r#"{"type":"string","sdfType":"vendor"}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
        ),
    ] {
        exercise(qualities, raw, typed, expected, complete, None);
    }
    let model = br#"{"info":{"features":["vendor"]},"sdfProperty":{"p":{"type":"string","sdfType":"byte-string"}}}"#;
    let catalogue = inputs(model, br#""Zg""#, Input::Text("Zg"), false);
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert!(!used(&wire(&report), BYTE_ID));
}

#[test]
fn a_whole_witness_and_unknown_branch_preserve_independent_model_conditions() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"sdfChoice":{"bad":{"type":"string","sdfType":42},"ordinary":{"type":"string"}}}}}"#;
    let catalogue = inputs(model, br#""ok""#, Input::Text("ok"), false);
    let report = selected(&catalogue);
    assert_shared(&report, Admission::Admitted, false, 4);
    assert!(report.outcome().errors() > 0);
    let value = wire(&report);
    assert!(
        value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["domain"] == "model" && row["code"] == "data.string")
    );
    assert!(
        !value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["domain"] == "sample" && row["severity"] == "error")
    );
    exercise(
        r#"{"sdfChoice":{"bytes":{"type":"string","sdfType":"byte-string","const":"Zg"},"unknown":{"sdfRef":false}}}"#,
        r#""Zm8""#,
        Input::Text("Zm8"),
        Admission::Undetermined,
        false,
        Some(BYTE_ID),
    );
    exercise(
        r#"{"sdfChoice":{"ordinary":{"type":"string","const":"Zg"},"bytes":{"type":"string","sdfType":"byte-string","const":"Zg"}}}"#,
        r#""Zg""#,
        Input::Text("Zg"),
        Admission::Undetermined,
        false,
        Some(BYTE_ID),
    );
}

#[test]
fn actual_sdf_type_origins_and_sample_spans_survive_presentation_omission() {
    let model = br##"{"info":{},"sdfData":{"Base":{"type":"string","sdfType":"byte-string"}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/Base"}}}"##;
    for projection in [false, true] {
        let catalogue = inputs(model, br#""_x""#, Input::Text("_x"), projection);
        let report = selected(&catalogue);
        let value = wire(&report);
        assert_shared(&report, Admission::Rejected, projection, 4);
        let finding = value["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["code"] == "sample.byte_string_representation")
            .unwrap();
        assert_eq!(finding["domain"], "sample");
        assert_eq!(finding["basis"]["kind"], "interpretation");
        assert_eq!(finding["basis"]["id"], BYTE_ID);
        assert_eq!(finding["primary"]["kind"], "sample");
        let related = finding["related"].as_array().unwrap();
        assert!(related.iter().any(|row| row["kind"] == "effective"
            && row["pointer"] == "/sdfProperty/p/sdfType"));
        assert!(related.iter().any(|row| row["kind"] == "source"
            && row["pointer"] == "/sdfData/Base/sdfType"
            && !row["span"].is_null()));
        let limited = Catalogue::take_inputs_with_limits(
            vec![
                SourceInput::supplied("m", model),
                if projection {
                    SourceInput::typed("s", Input::Text("_x").typed())
                } else {
                    SourceInput::supplied("s", br#""_x""#).sample()
                },
            ],
            Limits::default()
                .conditions(1)
                .diagnostic_bytes(1)
                .pointer_bytes(1),
        )
        .unwrap();
        let limited_report = selected(&limited);
        let shown = wire(&limited_report);
        assert_eq!(
            limited_report.sample_result().unwrap().admission(),
            Admission::Rejected
        );
        assert_eq!(limited_report.outcome().errors(), report.outcome().errors());
        assert_eq!(
            limited_report.outcome().complete(),
            report.outcome().complete()
        );
        assert!(used(&shown, BYTE_ID));
        assert_eq!(
            shown["coverage"]["requested"],
            value["coverage"]["requested"]
        );
        for id in AREAS {
            assert_eq!(
                limited_report
                    .sample_result()
                    .unwrap()
                    .area_status(id)
                    .require()
                    .unwrap(),
                report
                    .sample_result()
                    .unwrap()
                    .area_status(id)
                    .require()
                    .unwrap()
            );
        }
        assert!(shown["diagnostics"]["truncated"].as_bool().unwrap());
    }
}

#[test]
fn original_numeric_capability_failure_never_becomes_time_rejection() {
    let model = br#"{"info":{},"sdfProperty":{"p":{"type":"number","sdfType":"unix-time"}}}"#;
    let catalogue = Catalogue::with_limits(
        &[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", b"1e101").sample(),
        ],
        Limits::default().exponent_magnitude(100),
    )
    .unwrap();
    let report = selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .sample_value()
            .require()
            .unwrap()
            .kind(),
        JsonKind::Number
    );
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .area_status("sample-abstract-values/1")
            .require()
            .unwrap(),
        AssessmentStatus::Undetermined
    );
    assert!(
        !wire(&report)["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "sample.unix_time_representation")
    );
}

#[test]
fn applicability_null_and_branch_matrix_preserves_independent_obligations() {
    for (id, qualities, raw, typed, expected, complete, interpretation) in [
        (
            "A03",
            r#"{"type":"number","sdfType":"byte-string"}"#,
            "1",
            Input::Number("1"),
            Admission::Undetermined,
            false,
            None,
        ),
        (
            "A04",
            r#"{"type":"string","sdfType":"unix-time"}"#,
            r#""1""#,
            Input::Text("1"),
            Admission::Undetermined,
            false,
            None,
        ),
        (
            "A06",
            r#"{"type":"string","sdfType":42}"#,
            r#""x""#,
            Input::Text("x"),
            Admission::Undetermined,
            false,
            None,
        ),
        (
            "A07",
            r#"{"type":"string","sdfType":"byte-string","format":"uuid"}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            None,
        ),
        (
            "A08",
            r#"{"type":"string","sdfType":"byte-string","format":null}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            None,
        ),
        (
            "A09",
            r#"{"type":"string","sdfType":"byte-string","contentFormat":"application/octet-stream"}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            None,
        ),
        // Undeclared extension vocabulary leaves the channel intact, following
        // ADR 0037: Undeclared extension vocabulary.
        (
            "A10",
            r#"{"type":"string","sdfType":"byte-string","vendorQuality":true}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Admitted,
            true,
            Some(BYTE_ID),
        ),
        (
            "A13",
            r#"{"type":"string","sdfType":"byte-string","pattern":"^Zg$"}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            Some(BYTE_ID),
        ),
        (
            "A14",
            r#"{"type":"number","sdfType":"unix-time","maxLength":1}"#,
            "100",
            Input::Number("100"),
            Admission::Undetermined,
            false,
            Some(POSIX_ID),
        ),
        (
            "A15",
            r#"{"type":"string","sdfType":"byte-string","minimum":2}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            Some(BYTE_ID),
        ),
        (
            "A16",
            r#"{"type":"number","sdfType":"unix-time","minimum":0,"maximum":10}"#,
            "10.0",
            Input::Number("10.0"),
            Admission::Admitted,
            true,
            Some(POSIX_ID),
        ),
        (
            "A20",
            r#"{"type":"number","sdfType":"unix-time","minimum":0}"#,
            "-1",
            Input::Number("-1"),
            Admission::Undetermined,
            false,
            Some(POSIX_ID),
        ),
        (
            "T11",
            r#"{"type":"number","sdfType":"unix-time"}"#,
            r#""1""#,
            Input::Text("1"),
            Admission::Rejected,
            true,
            Some(POSIX_ID),
        ),
        (
            "T14",
            r#"{"type":"number","sdfType":"unix-time"}"#,
            r#""2016-12-31T23:59:60Z""#,
            Input::Text("2016-12-31T23:59:60Z"),
            Admission::Rejected,
            true,
            Some(POSIX_ID),
        ),
        (
            "N02",
            r#"{"type":"number","sdfType":"unix-time","nullable":true}"#,
            "null",
            Input::Null,
            Admission::Admitted,
            true,
            None,
        ),
        (
            "N03",
            r#"{"type":"string","sdfType":"byte-string","nullable":false}"#,
            "null",
            Input::Null,
            Admission::Rejected,
            true,
            None,
        ),
        (
            "N04",
            r#"{"type":"number","sdfType":"unix-time","nullable":false,"const":null}"#,
            "null",
            Input::Null,
            Admission::Rejected,
            true,
            None,
        ),
        (
            "N05",
            r#"{"type":"string","sdfType":"byte-string","const":"Zg"}"#,
            "null",
            Input::Null,
            Admission::Rejected,
            true,
            Some(BYTE_ID),
        ),
        (
            "N06",
            r#"{"type":"number","sdfType":"unix-time","nullable":"vendor"}"#,
            "null",
            Input::Null,
            Admission::Undetermined,
            false,
            None,
        ),
        (
            "E06",
            r#"{"type":"string","sdfType":"byte-string","const":"Zg=="}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Undetermined,
            false,
            Some(BYTE_ID),
        ),
        (
            "E13",
            r#"{"type":"number","sdfType":"unix-time","const":-0.5}"#,
            "-5e-1",
            Input::Number("-5e-1"),
            Admission::Undetermined,
            false,
            Some(POSIX_ID),
        ),
        (
            "C01",
            r#"{"sdfChoice":{"a":{"type":"string"},"b":{"type":"string","sdfType":"byte-string"}}}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Admitted,
            true,
            None,
        ),
        (
            "C03",
            r#"{"sdfChoice":{"a":{"type":"string","sdfType":"byte-string","const":"_x"},"b":{"type":"string","sdfType":"byte-string","const":"_w"}}}"#,
            r#""_w""#,
            Input::Text("_w"),
            Admission::Admitted,
            true,
            Some(BYTE_ID),
        ),
        (
            "C06",
            r#"{"type":"string","sdfType":"byte-string","sdfChoice":{"a":{"sdfChoice":{"x":{"const":"Zg"},"y":{"const":"Zm8"}}}}}"#,
            r#""Zm8""#,
            Input::Text("Zm8"),
            Admission::Admitted,
            true,
            Some(BYTE_ID),
        ),
        (
            "C07",
            r#"{"sdfChoice":{"a":{"type":"string","sdfType":"byte-string","const":"Zg"},"b":{"type":"integer","sdfType":"unix-time","const":10}}}"#,
            "10.0",
            Input::Number("10.0"),
            Admission::Admitted,
            true,
            Some(POSIX_ID),
        ),
        (
            "C08",
            r#"{"type":"string","sdfType":"vendor","sdfChoice":{"a":{"sdfType":"byte-string"}}}"#,
            r#""Zg""#,
            Input::Text("Zg"),
            Admission::Admitted,
            true,
            Some(BYTE_ID),
        ),
        (
            "C09",
            r#"{"sdfChoice":{"a":{"type":"string","sdfType":"byte-string"},"b":{"type":"string","format":null}}}"#,
            r#""x""#,
            Input::Text("x"),
            Admission::Undetermined,
            false,
            Some(BYTE_ID),
        ),
        (
            "I02",
            r#"{"type":"string","sdfType":"byte-string","nullable":false,"const":"_x"}"#,
            "null",
            Input::Null,
            Admission::Rejected,
            false,
            Some(BYTE_ID),
        ),
        (
            "I04",
            r#"{"type":42,"sdfType":"unix-time"}"#,
            "1",
            Input::Number("1"),
            Admission::Undetermined,
            false,
            None,
        ),
    ] {
        eprintln!("abstract matrix {id}");
        exercise(qualities, raw, typed, expected, complete, interpretation);
    }
}

#[test]
fn unavailable_json_input_never_becomes_an_abstract_representation_error() {
    let model = br#"{"sdfProperty":{"p":{"type":"number","sdfType":"unix-time"}}}"#;
    for raw in ["NaN", "Infinity", "-Infinity"] {
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model),
            SourceInput::supplied("s", raw.as_bytes()).sample(),
        ])
        .unwrap();
        let report = selected(&cat);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined
        );
        assert!(report.outcome().errors() > 0);
        assert!(!used(&wire(&report), POSIX_ID));
        assert!(
            !wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.unix_time_representation")
        );
    }
}

#[test]
fn original_numeric_token_coefficient_and_exponent_boundaries_remain_authoritative() {
    let model = br#"{"sdfProperty":{"p":{"sdfType":"unix-time"}}}"#;
    for (raw, limits, available) in [
        ("12345", Limits::default().number_token_bytes(5), true),
        ("12345", Limits::default().number_token_bytes(4), false),
        ("12345", Limits::default().coefficient_digits(5), true),
        ("12345", Limits::default().coefficient_digits(4), false),
        ("1e100", Limits::default().exponent_magnitude(100), true),
        ("1e100", Limits::default().exponent_magnitude(99), false),
        ("-12345", Limits::default().number_token_bytes(6), true),
        ("-12345", Limits::default().number_token_bytes(5), false),
    ] {
        let cat = Catalogue::with_limits(
            &[
                SourceInput::supplied("m", model),
                SourceInput::supplied("s", raw.as_bytes()).sample(),
            ],
            limits,
        )
        .unwrap();
        let report = selected(&cat);
        let sample = report.sample_result().unwrap();
        assert_eq!(
            sample
                .sample_value()
                .require()
                .unwrap()
                .number()
                .present()
                .is_some(),
            available,
            "{raw}"
        );
        assert_eq!(
            sample.admission(),
            if available && !raw.starts_with('-') {
                Admission::Admitted
            } else {
                Admission::Undetermined
            }
        );
        assert!(
            !wire(&report)["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == "sample.unix_time_representation")
        );
    }
}
