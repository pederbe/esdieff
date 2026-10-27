//! A Recorded trace checks every Trace entry independently against one selected document.
//! Follows ADR 0042: Recorded traces with independent entries.
use esdieff::{
    Admission, Catalogue, Disposition, SourceInput, TraceOutcome, TraceRequest, TraceResult,
};

const THERMOSTAT: &[u8] = include_bytes!("../examples/thermostat-check/thermostat.sdf.json");
const TARGET: &str = "/sdfObject/Thermostat/sdfProperty/targetTemperature";

fn thermostat() -> Catalogue {
    Catalogue::from_inputs(&[SourceInput::supplied("model", THERMOSTAT)]).unwrap()
}

/// A Trace entry line for the thermostat's target temperature, with `rest` after the value.
fn target(value: &str, rest: &str) -> String {
    format!("{{\"kind\":\"property\",\"pointer\":\"{TARGET}\",\"value\":{value}{rest}}}\n")
}

fn check<'a>(catalogue: &'a Catalogue, trace: &str) -> TraceResult<'a> {
    let document = catalogue.source_id("model").unwrap();
    catalogue
        .check_trace(TraceRequest::new(document, trace.as_bytes()))
        .unwrap()
}

/// Each listed entry's line, outcome, admission and condition codes.
fn listed(result: &TraceResult<'_>) -> Vec<(u32, TraceOutcome, Option<Admission>, Vec<String>)> {
    result
        .entries()
        .map(|entry| {
            (
                entry.line(),
                entry.outcome(),
                entry.admission(),
                entry.conditions().map(|c| c.code().to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn admitted_entries_pass_and_a_rejected_entry_fails_with_its_line_and_label() {
    let catalogue = thermostat();
    let trace = [
        target("21", ""),
        target("35", r#","label":"too hot""#),
        target("5", ""),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    assert_eq!(result.disposition(), Disposition::Failed);
    assert_eq!(result.count(TraceOutcome::Passed), 2);
    assert_eq!(result.count(TraceOutcome::Failed), 1);
    assert_eq!(result.admissions(Admission::Rejected), 1);
    assert_eq!(
        listed(&result),
        [(
            2,
            TraceOutcome::Failed,
            Some(Admission::Rejected),
            vec!["sample.bound".to_owned()]
        )]
    );
    assert_eq!(result.entries().next().unwrap().label(), Some("too hot"));
}

#[test]
fn an_expected_rejection_passes_and_a_differing_definite_admission_fails() {
    let catalogue = thermostat();
    let trace = [
        target("35", r#","expected":"rejected""#),
        target("21", r#","expected":"rejected""#),
        target("40", r#","expected":"admitted""#),
        target("21", r#","expected":"admitted""#),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    assert_eq!(result.disposition(), Disposition::Failed);
    assert_eq!(
        listed(&result),
        [
            (
                2,
                TraceOutcome::Failed,
                Some(Admission::Admitted),
                vec!["trace.expected".to_owned()]
            ),
            (
                3,
                TraceOutcome::Failed,
                Some(Admission::Rejected),
                vec!["trace.expected".to_owned(), "sample.bound".to_owned()]
            ),
        ]
    );
    let second = result.entries().next().unwrap();
    assert_eq!(second.expected(), Some(Admission::Rejected));
    assert_eq!(result.count(TraceOutcome::Passed), 2);
}

const CODES: &[u8] = br#"{"sdfProperty":{
    "code":{"type":"string","pattern":"["},
    "level":{"type":"integer","maximum":10}
}}"#;

fn codes() -> Catalogue {
    Catalogue::from_inputs(&[SourceInput::supplied("model", CODES)]).unwrap()
}

#[test]
fn an_undetermined_admission_stays_undetermined_whatever_was_expected() {
    let catalogue = codes();
    let code = |rest: &str| {
        format!(
            "{{\"kind\":\"property\",\"pointer\":\"/sdfProperty/code\",\"value\":\"a\"{rest}}}\n"
        )
    };
    let trace = [
        code(""),
        code(r#","expected":"rejected""#),
        code(r#","expected":"admitted""#),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    assert_eq!(result.disposition(), Disposition::Undetermined);
    assert_eq!(result.count(TraceOutcome::Undetermined), 3);
    assert_eq!(result.admissions(Admission::Undetermined), 3);
    for entry in result.entries() {
        assert_eq!(entry.outcome(), TraceOutcome::Undetermined);
        assert_eq!(entry.admission(), Some(Admission::Undetermined));
    }
    // A failed entry decides the disposition over undetermined ones.
    let level = "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/level\",\"value\":11}\n";
    let result = check(&catalogue, &format!("{trace}{level}"));
    assert_eq!(result.disposition(), Disposition::Failed);
}

#[test]
fn a_malformed_or_unselectable_entry_fails_only_its_own_line() {
    let catalogue = thermostat();
    let trace = [
        target("21", ""),
        "{\"kind\":\"property\",\"pointer\":\n".to_owned(),
        target("21", r#","colour":"red""#),
        format!("{{\"kind\":\"property\",\"pointer\":\"{TARGET}\"}}\n"),
        target("21", r#","expected":"maybe""#),
        "{\"kind\":\"property\",\"pointer\":\"/sdfObject/Thermostat/sdfProperty/absent\",\"value\":1}\n"
            .to_owned(),
        "{\"kind\":\"event_output\",\"pointer\":\"/sdfObject/Thermostat/sdfProperty/targetTemperature\",\"value\":1}\n"
            .to_owned(),
        "[1]\n".to_owned(),
        target("35", ""),
        target("22", ""),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    let codes = |codes: &[&str]| {
        codes
            .iter()
            .map(|code| code.to_string())
            .collect::<Vec<_>>()
    };
    let failed = |line, admission, conditions: &[&str]| {
        (line, TraceOutcome::Failed, admission, codes(conditions))
    };
    assert_eq!(
        listed(&result),
        [
            failed(2, None, &["json.syntax"]),
            failed(3, None, &["trace.unknown_member"]),
            failed(4, None, &["trace.missing_member"]),
            failed(5, None, &["trace.invalid_member"]),
            failed(6, None, &["trace.unselectable"]),
            failed(7, None, &["trace.unselectable"]),
            failed(8, None, &["trace.entry"]),
            failed(9, Some(Admission::Rejected), &["sample.bound"]),
        ]
    );
    assert_eq!(result.count(TraceOutcome::Passed), 2);
    let unknown = result
        .entries()
        .nth(1)
        .unwrap()
        .conditions()
        .next()
        .unwrap();
    assert_eq!(unknown.pointer(), Some("/colour"));
    let missing_value = result.entries().nth(2).unwrap();
    assert_eq!(
        missing_value.selector(),
        Some((esdieff::SampleKind::Property, TARGET))
    );
    let unselectable = result.entries().nth(4).unwrap();
    assert_eq!(
        unselectable.selector(),
        Some((
            esdieff::SampleKind::Property,
            "/sdfObject/Thermostat/sdfProperty/absent"
        ))
    );
}

#[test]
fn crlf_endings_a_final_empty_line_and_a_missing_last_lf_are_accepted() {
    let catalogue = thermostat();
    let crlf = target("21", "").replace('\n', "\r\n");
    let last = target("35", "");
    let cases = [
        (format!("{crlf}{crlf}\r\n"), 2, 0),
        (format!("{crlf}\n"), 1, 0),
        (format!("{crlf}{}", last.trim_end()), 1, 1),
        ("\n".to_owned(), 0, 0),
        (String::new(), 0, 0),
    ];
    for (trace, passed, failed) in cases {
        let result = check(&catalogue, &trace);
        assert_eq!(result.count(TraceOutcome::Passed), passed, "{trace:?}");
        assert_eq!(result.count(TraceOutcome::Failed), failed, "{trace:?}");
    }
    // Only the last line may be empty.
    let result = check(&catalogue, &format!("{crlf}\n\n"));
    assert_eq!(
        listed(&result),
        [(
            2,
            TraceOutcome::Failed,
            None,
            vec!["json.syntax".to_owned()]
        )]
    );
}

const RICH: &[u8] = include_bytes!("../examples/thermostat-check/rich.sdf.json");
const CONFIGURE: &str = "/sdfObject/Thermostat/sdfAction/configure";

#[test]
fn condition_spans_select_trace_bytes_and_the_library_result_matches_the_report() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("model", RICH)]).unwrap();
    let configure = |value: &str| {
        format!(
            "{{\"kind\":\"action_input\",\"pointer\":\"{CONFIGURE}\",\"value\":{value},\"label\":\"cfg\"}}\r\n"
        )
    };
    let trace = [
        configure(r#"{"targetTemperature":21,"mode":"heat"}"#),
        configure(r#"{"targetTemperature":31,"mode":"heat"}"#),
        target("21", r#","colour":"red""#),
        configure(r#"{"targetTemperature":20.5}"#),
        target("40", r#","expected":"admitted""#),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    let report: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(report["version"], "1.5");
    assert_eq!(report["operation"], "trace");
    let summary = &report["result"]["summary"];
    let outcomes = [
        TraceOutcome::Passed,
        TraceOutcome::Failed,
        TraceOutcome::Undetermined,
    ];
    for outcome in outcomes {
        assert_eq!(summary[outcome.as_str()], result.count(outcome));
    }
    for admission in [
        Admission::Admitted,
        Admission::Rejected,
        Admission::Undetermined,
    ] {
        assert_eq!(
            summary["admissions"][admission.as_str()],
            result.admissions(admission)
        );
    }
    assert_eq!(
        (summary["passed"].as_u64(), summary["failed"].as_u64()),
        (Some(1), Some(4))
    );
    assert_eq!(
        report["result"]["entries_omitted"],
        result.entries_omitted()
    );
    assert_eq!(report["outcome"]["disposition"], "failed");
    assert_eq!(report["coverage"]["requested"]["count_unit"], "trace_entry");
    let trace_id = result.trace_source().get();
    let source = report["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == trace_id)
        .unwrap();
    assert_eq!(source["role"], "sample");
    let entries = report["result"]["entries"].as_array().unwrap();
    // Passing entries are counted, not listed.
    assert_eq!(entries.len(), 4);
    assert_eq!(entries.len(), result.entries().len());
    let mut selected = Vec::new();
    for (wire, typed) in entries.iter().zip(result.entries()) {
        assert_eq!(wire["line"], typed.line());
        assert_eq!(wire["label"].as_str(), typed.label());
        assert_eq!(wire["outcome"], typed.outcome().as_str());
        assert_eq!(
            wire["admission"].as_str(),
            typed.admission().map(|a| a.as_str())
        );
        assert_eq!(
            wire["expected"].as_str(),
            typed.expected().map(|a| a.as_str())
        );
        let (kind, pointer) = typed.selector().unwrap();
        assert_eq!(wire["selector"]["kind"], kind.as_str());
        assert_eq!(wire["selector"]["pointer"], pointer);
        assert_eq!(wire["selector"]["document"], result.document().get());
        let conditions = wire["conditions"].as_array().unwrap();
        assert_eq!(conditions.len(), typed.conditions().len());
        for (wire, typed) in conditions.iter().zip(typed.conditions()) {
            assert_eq!(wire["code"], typed.code());
            assert_eq!(wire["primary"]["kind"], "sample");
            assert_eq!(wire["primary"]["source_id"], trace_id);
            assert_eq!(wire["primary"]["pointer"].as_str(), typed.pointer());
            let span = typed.span().unwrap();
            assert_eq!(
                wire["primary"]["span"],
                serde_json::json!([span.start, span.end])
            );
            selected.push((
                typed.code().to_owned(),
                typed.pointer().unwrap().to_owned(),
                trace[span].to_owned(),
            ));
        }
    }
    let row = |code: &str, pointer: &str, text: &str| {
        (code.to_owned(), pointer.to_owned(), text.to_owned())
    };
    assert_eq!(
        selected,
        [
            row("sample.bound", "/value/targetTemperature", "31"),
            row("trace.unknown_member", "/colour", r#""red""#),
            row("sample.required", "/value", r#"{"targetTemperature":20.5}"#),
            row("trace.expected", "/expected", r#""admitted""#),
            row("sample.bound", "/value", "40"),
        ]
    );
}

#[test]
fn entries_beyond_the_report_byte_limit_are_counted_in_line_order() {
    let trace = target("35", "").repeat(2000);
    let limited = |limit| {
        let limits = esdieff::Limits::default().report_bytes(limit);
        Catalogue::with_limits(&[SourceInput::supplied("model", THERMOSTAT)], limits).ok()
    };
    // The smallest report limit that admits the trace envelope, in 4 KiB steps, with room for
    // a few entries more.
    let limit = (1..)
        .map(|steps| steps * 4096)
        .find(|limit| {
            limited(*limit).is_some_and(|catalogue| {
                let document = catalogue.source_id("model").unwrap();
                catalogue
                    .check_trace(TraceRequest::new(document, trace.as_bytes()))
                    .is_ok()
            })
        })
        .unwrap()
        + 4096;
    let catalogue = limited(limit).unwrap();
    let result = check(&catalogue, &trace);
    assert_eq!(result.count(TraceOutcome::Failed), 2000);
    let listed = result.entries().len() as u32;
    assert!(listed > 0 && result.entries_omitted() > 0);
    assert_eq!(listed + result.entries_omitted(), 2000);
    let lines: Vec<u32> = result.entries().map(|entry| entry.line()).collect();
    assert_eq!(lines, (1..=listed).collect::<Vec<_>>());
    let json = result.to_json();
    assert!(json.len() <= limit && result.to_human().len() <= limit);
    let report: serde_json::Value = serde_json::from_str(&json).unwrap();
    let omission = serde_json::json!({"field":"result.entries","reason":"report_limit",
        "omitted_count":result.entries_omitted()});
    assert_eq!(report["result"]["omissions"], serde_json::json!([omission]));
    assert_eq!(report["diagnostics"]["truncated"], true);
    assert_eq!(report["result"]["summary"]["failed"], 2000);
}

#[test]
fn an_oversized_trace_line_or_entry_count_is_a_request_error() {
    let catalogue = thermostat();
    let document = catalogue.source_id("model").unwrap();
    let refused = |trace: &[u8]| {
        catalogue
            .check_trace(TraceRequest::new(document, trace))
            .err()
    };
    let file = vec![b' '; TraceRequest::FILE_BYTES + 1];
    assert_eq!(refused(&file), Some("trace exceeds 64 MiB"));
    let mut line = vec![b' '; 1024 * 1024];
    assert_eq!(refused(&line), None);
    line.push(b' ');
    assert_eq!(refused(&line), Some("trace line exceeds 1 MiB"));
    let entries = "{}\n".repeat(100_000);
    assert_eq!(refused(entries.as_bytes()), None);
    let entries = format!("{entries}{{}}");
    assert_eq!(
        refused(entries.as_bytes()),
        Some("trace exceeds 100,000 entries")
    );
}

#[test]
fn numbers_stay_exact_and_duplicate_names_are_refused() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br#"{"sdfProperty":{"count":{"type":"integer","maximum":9007199254740992},
            "settings":{"type":"object"}}}"#,
    )])
    .unwrap();
    let count = |value: &str| {
        format!("{{\"kind\":\"property\",\"pointer\":\"/sdfProperty/count\",\"value\":{value}}}\n")
    };
    let trace = [
        count("9007199254740992"),
        count("9007199254740993"),
        "{\"kind\":\"property\",\"kind\":\"property\",\"pointer\":\"/sdfProperty/count\",\"value\":1}\n"
            .to_owned(),
        "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/settings\",\"value\":{\"a\":1,\"a\":2}}\n"
            .to_owned(),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    assert_eq!(
        listed(&result),
        [
            (
                2,
                TraceOutcome::Failed,
                Some(Admission::Rejected),
                vec!["sample.bound".to_owned()]
            ),
            (
                3,
                TraceOutcome::Failed,
                None,
                vec!["input.duplicate_name".to_owned()]
            ),
            (
                4,
                TraceOutcome::Failed,
                None,
                vec!["input.duplicate_name".to_owned()]
            ),
        ]
    );
}

#[test]
fn an_entry_that_exhausts_its_allowance_stays_undetermined_while_later_entries_settle() {
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied(
            "model",
            br#"{"sdfProperty":{"readings":{"type":"array","uniqueItems":true,
                "items":{"type":"integer"}}}}"#,
        )],
        esdieff::Limits::default().equality_pairs(100),
    )
    .unwrap();
    let readings = |value: &str| {
        format!(
            "{{\"kind\":\"property\",\"pointer\":\"/sdfProperty/readings\",\"value\":{value}}}\n"
        )
    };
    // Comparing every pair of 30 items needs more than the 100 equality pairs one Sample
    // request receives, while each later entry needs only one pair.
    let heavy: Vec<String> = (0..30).map(|item| item.to_string()).collect();
    let trace = [
        readings(&format!("[{}]", heavy.join(","))),
        readings("[1,2]"),
        readings("[1,1]"),
        readings("[3,4]"),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    assert_eq!(
        listed(&result),
        [
            (
                1,
                TraceOutcome::Undetermined,
                Some(Admission::Undetermined),
                vec!["operation.limit".to_owned()]
            ),
            (
                3,
                TraceOutcome::Failed,
                Some(Admission::Rejected),
                vec!["sample.unique".to_owned()]
            ),
        ]
    );
    assert_eq!(result.count(TraceOutcome::Passed), 2);
}

#[test]
fn a_line_that_is_not_utf8_is_undetermined_and_the_trace_source_partial() {
    let catalogue = thermostat();
    let mut trace = target("\"\u{1}\"", "").into_bytes();
    let quote = trace.iter().rposition(|byte| *byte == 1).unwrap();
    trace[quote] = 0xFF;
    trace.extend_from_slice(target("21", "").as_bytes());
    let document = catalogue.source_id("model").unwrap();
    let result = catalogue
        .check_trace(TraceRequest::new(document, &trace))
        .unwrap();
    assert_eq!(
        listed(&result),
        [(
            1,
            TraceOutcome::Undetermined,
            None,
            vec!["input.encoding".to_owned()]
        )]
    );
    assert_eq!(result.count(TraceOutcome::Passed), 1);
    let report: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    let trace_id = result.trace_source().get();
    let source = report["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == trace_id)
        .unwrap();
    assert_eq!(source["decode"], "partial");
}

#[test]
fn a_description_that_cannot_be_established_leaves_its_entries_undetermined() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "model",
        br##"{"sdfProperty":{"broken":{"sdfRef":"#/sdfData/absent"}}}"##,
    )])
    .unwrap();
    let trace = "{\"kind\":\"property\",\"pointer\":\"/sdfProperty/broken\",\"value\":1,\"expected\":\"rejected\"}\n";
    let result = check(&catalogue, trace);
    assert_eq!(result.disposition(), Disposition::Undetermined);
    assert_eq!(
        listed(&result),
        [(
            1,
            TraceOutcome::Undetermined,
            Some(Admission::Undetermined),
            vec!["trace.unavailable".to_owned()]
        )]
    );
}

#[test]
fn conditions_beyond_eight_are_counted_with_a_count_limit_record() {
    let catalogue = thermostat();
    let extra: String = (0..10)
        .map(|index| format!(",\"extra{index}\":1"))
        .collect();
    let result = check(&catalogue, &target("21", &extra));
    let entry = result.entries().next().unwrap();
    assert_eq!(entry.conditions().len(), 8);
    assert_eq!(entry.conditions_omitted(), 2);
    let report: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    assert_eq!(report["result"]["entries"][0]["conditions_omitted"], 2);
    assert_eq!(
        report["result"]["omissions"],
        serde_json::json!([{"field":"result.entries.conditions","reason":"count_limit","omitted_count":2}])
    );
}

#[test]
fn a_long_label_and_a_malformed_pointer_fail_their_entries() {
    let catalogue = thermostat();
    let label = "a".repeat(257);
    let trace = [
        target("21", &format!(",\"label\":\"{label}\"")),
        "{\"kind\":\"property\",\"pointer\":\"sdfProperty/level\",\"value\":1,\"expected\":\"rejected\"}\n"
            .to_owned(),
    ]
    .concat();
    let result = check(&catalogue, &trace);
    assert_eq!(
        listed(&result),
        [
            (
                1,
                TraceOutcome::Failed,
                None,
                vec!["trace.invalid_member".to_owned()]
            ),
            (
                2,
                TraceOutcome::Failed,
                None,
                vec!["trace.unselectable".to_owned()]
            ),
        ]
    );
    let entries: Vec<_> = result.entries().collect();
    assert_eq!(entries[0].label(), None);
    assert_eq!(
        entries[0].conditions().next().unwrap().pointer(),
        Some("/label")
    );
    // A valid kind and a pointer string make a selector, whether or not it selects anything.
    assert_eq!(
        entries[1].selector(),
        Some((esdieff::SampleKind::Property, "sdfProperty/level"))
    );
    // The human report shows the Expected admission of an entry that was not checked.
    let human = result.to_human();
    assert!(human.contains("\n  expected rejected\n"), "{human}");
}

#[test]
fn an_omitted_label_is_recorded_like_any_other_source() {
    let catalogue = thermostat();
    let document = catalogue.source_id("model").unwrap();
    let result = catalogue
        .check_trace(TraceRequest::new(document, target("21", "").as_bytes()).omit_label())
        .unwrap();
    let report: serde_json::Value = serde_json::from_str(&result.to_json()).unwrap();
    let trace_id = result.trace_source().get();
    let source = report["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == trace_id)
        .unwrap();
    assert_eq!(source["label"], serde_json::Value::Null);
    assert_eq!(source["label_omitted"]["field"], "sources.label");
    assert_eq!(report["diagnostics"]["truncated"], true);
}
