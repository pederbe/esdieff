//! Selected civil samples follow ADR 0018: Civil date and time sample conventions.
use esdieff::{
    Admission, Catalogue, JsonBuilder, Limits, Report, SampleKind, SampleRequest, SourceInput,
};
use serde_json::Value;

const CONVENTION: &str = "ADR0018-selected-representation";
#[test]
fn specialized_detail_keeps_an_independent_unavailable_child_meaning() {
    // A base quality outside its role leaves the other child's meaning unavailable.
    let model=br#"{"sdfProperty":{"p":{"type":"object","properties":{"stamp":{"type":"string","format":"date-time"},"other":{"type":"string","readable":true}}}}}"#;
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", br#"{"stamp":"2030-06-30T23:59:60Z","other":"x"}"#).sample(),
    ])
    .unwrap();
    let wire = wire(&selected(&catalogue));
    let items = wire["diagnostics"]["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|row| row["code"] == "sample.temporal_knowledge_unavailable")
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|row| row["code"] == "sample.coverage_unavailable"
                && row["assessment_unit"] == "sample-abstract-values/1")
            .count(),
        1
    );
    assert_eq!(
        wire["coverage"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["id"] == "sample-abstract-values/1")
            .count(),
        1
    );
}
const KNOWLEDGE: &str = "iers-bulletin-c72/2026-07-06";
const REPRESENTATION_CITATION: &str = "RFC 9880 Appendix C.2; RFC 3339 Section 5.6";
const CALENDAR_CITATION: &str = "RFC 3339 Section 5.7 and Appendix C; RFC 9880 Appendix C.2";
const CLOCK_CITATION: &str = "RFC 3339 Sections 5.6 and 5.7; RFC 9880 Appendix C.2";
const SLOT_CITATION: &str = "RFC 3339 Section 5.7; RFC 9880 Appendix C.2";
const DATE_FREE_CITATION: &str =
    "ADR 0018: Civil date and time sample conventions; RFC 9880 Appendix C.2; RFC 3339 Section 5.6";
const ABSTRACT_CITATION: &str =
    "RFC 9880 Appendix C.2; ADR 0018: Civil date and time sample conventions";

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
fn model(format: &str, extra: &str) -> String {
    format!(
        r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":false,"format":"{format}"{extra}}}}}}}"#
    )
}
fn wire(report: &Report<'_>) -> Value {
    serde_json::from_str(&report.to_json()).unwrap()
}
fn usage(wire: &Value, convention: bool, knowledge: bool) {
    assert_eq!(
        wire["baseline"]["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == CONVENTION),
        convention
    );
    if knowledge {
        assert_eq!(
            wire["baseline"]["temporal_knowledge"],
            serde_json::json!({
                "id": KNOWLEDGE, "from": "1972-01-01", "through": "2026-12-31",
                "transition_sha256": "6cb6f5d4b819f2e568e25db4b0b26d89dedf031fdffb18bc94d40f4e94e268d7",
                "bulletin_sha256": "310e172eadacacca3adf92cb9bd646fcc0d6320e35e996a106d2644ede3b52f0"
            })
        );
    } else {
        assert!(wire["baseline"].get("temporal_knowledge").is_none());
    }
}
fn original_format_locations(
    condition: &Value,
    wire: &Value,
    model_text: &str,
    format: &str,
    token: &[u8],
    typed: bool,
) {
    let sample = &wire["request"]["sample"];
    let document = &wire["request"]["selector"]["document"];
    assert_eq!(condition["primary"]["kind"], "sample");
    assert_eq!(condition["primary"]["source_id"], *sample);
    assert_eq!(condition["primary"]["pointer"], "");
    assert_eq!(
        condition["primary"]["span"],
        if typed {
            Value::Null
        } else {
            serde_json::json!([0, token.len()])
        }
    );
    assert_eq!(condition["primary"], wire["result"]["sample_location"]);
    let related = condition["related"].as_array().unwrap();
    let effective = related
        .iter()
        .find(|row| row["kind"] == "effective")
        .unwrap();
    assert_eq!(effective["root"], *document);
    assert_eq!(effective["pointer"], "/sdfProperty/p/format");
    assert!(effective.get("span").is_none());
    let source = related.iter().find(|row| row["kind"] == "source").unwrap();
    let literal = serde_json::to_string(format).unwrap();
    let start = model_text.find(literal.as_str()).unwrap();
    assert_eq!(source["source_id"], *document);
    assert_eq!(source["pointer"], "/sdfProperty/p/format");
    assert_eq!(
        source["span"],
        serde_json::json!([start, start + literal.len()])
    );
}

#[test]
fn temporal_conditions_have_exact_typed_bases_domains_and_original_locations() {
    for (
        format,
        text,
        code,
        domain,
        severity,
        basis_kind,
        basis_id,
        citation,
        knowledge,
        unknown,
    ) in [
        (
            "date-time",
            "2024-01-01 12:00:00Z",
            "sample.temporal_representation",
            "sample",
            "error",
            "interpretation",
            Some(CONVENTION),
            REPRESENTATION_CITATION,
            false,
            false,
        ),
        (
            "date",
            "1900-02-29",
            "sample.temporal_calendar",
            "sample",
            "error",
            "rfc",
            None,
            CALENDAR_CITATION,
            false,
            false,
        ),
        (
            "time",
            "24:00:00Z",
            "sample.temporal_clock",
            "sample",
            "error",
            "rfc",
            None,
            CLOCK_CITATION,
            false,
            false,
        ),
        (
            "date-time",
            "2030-06-30T12:34:60Z",
            "sample.temporal_exceptional_slot",
            "sample",
            "error",
            "rfc",
            None,
            SLOT_CITATION,
            false,
            false,
        ),
        (
            "date-time",
            "2024-06-30T23:59:60Z",
            "sample.temporal_leap_second",
            "sample",
            "error",
            "rfc",
            Some(KNOWLEDGE),
            SLOT_CITATION,
            true,
            false,
        ),
        (
            "date-time",
            "2030-06-30T23:59:60Z",
            "sample.temporal_knowledge_unavailable",
            "operation",
            "incomplete",
            "operational",
            Some("temporal-knowledge"),
            SLOT_CITATION,
            true,
            true,
        ),
        (
            "time",
            "23:59:60Z",
            "sample.temporal_meaning_unavailable",
            "operation",
            "incomplete",
            "operational",
            Some("sample-establishment"),
            DATE_FREE_CITATION,
            false,
            true,
        ),
    ] {
        let model_text = model(format, "");
        let token = serde_json::to_string(text).unwrap();
        for typed in [false, true] {
            let sample = if typed {
                SourceInput::typed("s", JsonBuilder::new().text(text).unwrap())
            } else {
                SourceInput::supplied("s", token.as_bytes()).sample()
            };
            let catalogue = Catalogue::take_inputs(vec![
                SourceInput::supplied("m", model_text.as_bytes()),
                sample,
            ])
            .unwrap();
            let report = selected(&catalogue);
            let wire = wire(&report);
            let items = wire["diagnostics"]["items"].as_array().unwrap();
            let conditions = items
                .iter()
                .filter(|row| row["code"] == code)
                .collect::<Vec<_>>();
            assert_eq!(conditions.len(), 1, "{text}");
            let condition = conditions[0];
            assert_eq!(condition["domain"], domain);
            assert_eq!(condition["severity"], severity);
            assert_eq!(condition["basis"]["kind"], basis_kind);
            if let Some(id) = basis_id {
                assert_eq!(condition["basis"]["id"], id);
            } else {
                assert!(condition["basis"].get("id").is_none());
            }
            assert_eq!(condition["basis"]["citation"], citation);
            original_format_locations(
                condition,
                &wire,
                &model_text,
                format,
                token.as_bytes(),
                typed,
            );
            usage(&wire, true, knowledge);
            if unknown {
                assert_eq!(condition["assessment_unit"], "sample-abstract-values/1");
                assert_eq!(wire["outcome"]["incomplete_checks"], 1);
                assert_eq!(wire["outcome"]["errors"], 0);
                assert!(
                    !items
                        .iter()
                        .any(|row| row["code"] == "sample.coverage_unavailable"
                            && row["assessment_unit"] == "sample-abstract-values/1")
                );
            }
            let abstract_row = wire["coverage"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == "sample-abstract-values/1")
                .unwrap();
            if !unknown {
                assert_eq!(abstract_row["basis"]["kind"], "rfc");
                assert!(abstract_row["basis"].get("id").is_none());
                assert_eq!(abstract_row["basis"]["citation"], ABSTRACT_CITATION);
                assert!(
                    !abstract_row["basis"]["citation"]
                        .as_str()
                        .unwrap()
                        .contains("Appendix C.1")
                );
            }
        }
    }
}

#[test]
fn knowledge_is_present_only_for_actual_interval_queries_and_survives_failed_branches() {
    for (format, text, knowledge) in [
        ("date", "0000-02-29", false),
        ("date-time", "2030-06-30T23:59:58Z", false),
        ("date-time", "2024-06-29T23:59:59Z", false),
        ("date-time", "2024-06-30T23:59:59Z", true),
        ("time", "23:59:59Z", false),
        ("time", "23:59:60Z", false),
        ("date-time", "2030-06-30T12:34:60Z", false),
        ("date-time", "2030-06-29T23:59:60Z", false),
    ] {
        let model = model(format, "");
        let token = serde_json::to_string(text).unwrap();
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", token.as_bytes()).sample(),
        ])
        .unwrap();
        usage(&wire(&selected(&catalogue)), true, knowledge);
    }
    let model = br#"{"sdfProperty":{"p":{"type":"string","nullable":false,"sdfChoice":{"aDated":{"format":"date-time"},"zText":{}}}}}"#;
    let text = "2024-06-30T23:59:60Z";
    let token = serde_json::to_string(text).unwrap();
    for typed in [false, true] {
        let sample = if typed {
            SourceInput::typed("s", JsonBuilder::new().text(text).unwrap())
        } else {
            SourceInput::supplied("s", token.as_bytes()).sample()
        };
        let catalogue =
            Catalogue::take_inputs(vec![SourceInput::supplied("m", model), sample]).unwrap();
        let report = selected(&catalogue);
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Admitted
        );
        let wire = wire(&report);
        usage(&wire, true, true);
        assert_eq!(
            wire["result"]["sample_location"]["span"],
            if typed {
                Value::Null
            } else {
                serde_json::json!([0, token.len()])
            }
        );
        assert!(
            !wire["diagnostics"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["code"] == "sample.temporal_leap_second")
        );
    }
}

#[test]
fn temporal_irrelevance_does_not_record_convention_or_knowledge_use() {
    for (nullable, token) in [
        (true, b"null".as_slice()),
        (false, b"null".as_slice()),
        (false, b"true".as_slice()),
    ] {
        let model = format!(
            r#"{{"sdfProperty":{{"p":{{"type":"string","nullable":{nullable},"format":"date"}}}}}}"#
        );
        let catalogue = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", token).sample(),
        ])
        .unwrap();
        usage(&wire(&selected(&catalogue)), false, false);
    }
}

#[test]
fn one_specialized_unknown_does_not_suppress_independent_const_cause() {
    let model = model("date-time", ",\"const\":\"2030-06-30T23:59:60Z\"");
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model.as_bytes()),
        SourceInput::supplied("s", br#""2030-06-30T23:59:60Z""#).sample(),
    ])
    .unwrap();
    let report = selected(&catalogue);
    let wire = wire(&report);
    let items = wire["diagnostics"]["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|row| row["code"] == "sample.temporal_knowledge_unavailable")
            .count(),
        1
    );
    assert!(
        !items
            .iter()
            .any(|row| row["code"] == "sample.coverage_unavailable"
                && row["assessment_unit"] == "sample-abstract-values/1")
    );
    assert!(
        items
            .iter()
            .any(|row| row["code"] == "sample.coverage_unavailable"
                && row["assessment_unit"] == "sample-const/1")
    );
    assert_eq!(wire["outcome"]["incomplete_checks"], 2);
    assert_eq!(wire["outcome"]["errors"], 0);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    usage(&wire, true, true);
}

#[test]
fn known_no_insertion_omission_preserves_complete_error_and_actual_use() {
    let model = model("date-time", "");
    let token = br#""2024-06-30T23:59:60Z""#;
    for typed in [false, true] {
        let mut summaries = Vec::new();
        for limits in [
            Limits::default(),
            Limits::default().diagnostic_bytes(1),
            Limits::default()
                .conditions(1)
                .diagnostic_bytes(1)
                .pointer_bytes(1)
                .pointer_output_bytes(1),
        ] {
            let sample = if typed {
                SourceInput::typed(
                    "s",
                    JsonBuilder::new().text("2024-06-30T23:59:60Z").unwrap(),
                )
            } else {
                SourceInput::supplied("s", token).sample()
            };
            let catalogue = Catalogue::take_inputs_with_limits(
                vec![SourceInput::supplied("m", model.as_bytes()), sample],
                limits,
            )
            .unwrap();
            let report = selected(&catalogue);
            let wire = wire(&report);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Rejected
            );
            assert!(report.outcome().complete());
            assert_eq!(report.outcome().errors(), 1);
            assert_eq!(wire["outcome"]["incomplete_checks"], 0);
            usage(&wire, true, true);
            assert_eq!(
                wire["result"]["sample_location"]["span"],
                if typed {
                    Value::Null
                } else {
                    serde_json::json!([0, token.len()])
                }
            );
            summaries.push((
                wire["outcome"].clone(),
                wire["coverage"]["requested"].clone(),
                wire["baseline"].clone(),
                wire["result"]["sample_location"].clone(),
            ));
        }
        assert!(summaries.iter().all(|row| row == &summaries[0]));
    }
}
