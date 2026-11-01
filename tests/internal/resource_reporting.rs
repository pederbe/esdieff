#[path = "../resource_support/contexts.rs"]
mod contexts;
#[path = "../resource_support/format_use_contract.rs"]
mod format_use_contract;

#[test]
fn initial_unavailable_is_null_and_unknown_id_absent() {
    use esdieff::*;
    let inputs = [
        SourceInput::supplied(
            "m",
            br#"{"sdfData":{"V":{"type":"string","format":"uuid"}}}"#,
        ),
        SourceInput::supplied("s", br#""00000000-0000-0000-0000-000000000000""#).sample(),
    ];
    let cat =
        Catalogue::with_limits(&inputs, Limits::default().effective_allocation_bytes(1)).unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(
        wire.get("baseline")
            .unwrap()
            .get("format_knowledge")
            .unwrap()
            .is_null()
    );
    for (id, _) in format_use_contract::RULES {
        assert!(matches!(report.format_knowledge(id), Field::Unavailable(_)));
    }
    assert!(matches!(report.format_knowledge("unknown"), Field::Absent));
}

#[test]
fn established_owner_without_a_supported_format_reports_an_empty_array() {
    use esdieff::*;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "m",
            br#"{"info":{},"sdfData":{"V":{"type":"string","format":"future-format"}}}"#,
        ),
        SourceInput::supplied("s", br#""device""#).sample(),
    ])
    .unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Undetermined
    );
    let wire = serde_json::from_str(&report.to_json()).unwrap();
    format_use_contract::assert_sample_use(&report, &wire, &report.to_human(), &[]);
}

#[test]
fn all_contexts_and_restricted_items_keep_guarded_meaning() {
    use esdieff::*;
    for extra in [
        r#""sdfType":"byte-string""#,
        r#""contentFormat":"application/json""#,
        r#""futureMeaning":true""#,
    ] {
        let model = format!(
            r#"{{"sdfData":{{"V":{{"type":"string","format":"uuid",{extra},"const":"invalid"}}}}}}"#
        );
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", br#""invalid""#).sample(),
        ])
        .unwrap();
        let report = cat
            .check_sample(
                SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                    .sample(cat.source_id("s").unwrap()),
            )
            .unwrap();
        assert_eq!(
            report.sample_result().unwrap().admission(),
            Admission::Undetermined,
            "{extra}"
        );
        assert!(
            !matches!(
                report.format_knowledge(format_use_contract::BARE),
                Field::Present(_)
            ),
            "{extra}"
        );
        let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        if wire["baseline"]["format_knowledge"].is_null() {
            format_use_contract::assert_initial_unavailable(&report, &wire);
        } else {
            format_use_contract::assert_sample_use(&report, &wire, &report.to_human(), &[]);
        }
    }
    let model=br#"{"sdfData":{"V":{"type":"array","items":{"type":"string","format":"uuid"},"uniqueItems":true}}}"#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied(
            "s",
            br#"["f81d4fae-7dec-11d0-a765-00a0c91e6bf6","F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6"]"#,
        )
        .sample(),
    ])
    .unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    assert!(report.outcome().complete());
    assert!(matches!(
        report.format_knowledge(format_use_contract::BARE),
        Field::Present(_)
    ));
    assert!(
        report
            .diagnostics()
            .conditions()
            .any(|condition| condition.code() == "sample.unique")
    );
}

#[test]
fn nested_scalar_bases_fold_and_all_actual_facts_survive() {
    use esdieff::*;
    let model=br#"{"sdfData":{"V":{"type":"object","properties":{"id":{"type":"string","format":"uuid"},"link":{"type":"string","format":"uri"},"when":{"type":"string","format":"date"}}}}}"#;
    let sample=br#"{"id":"00000000-0000-0000-0000-000000000000","link":"https://example.test/","when":"2024-02-29"}"#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", sample).sample(),
    ])
    .unwrap();
    let report = cat
        .check_sample(
            SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
                .sample(cat.source_id("s").unwrap()),
        )
        .unwrap();
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let row = wire["coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "sample-abstract-values/1")
        .unwrap();
    assert_eq!(row["basis"]["kind"], "rfc");
    assert!(
        row["basis"]["citation"]
            .as_str()
            .unwrap()
            .contains("ADR 0019")
    );
    assert!(
        row["basis"]["citation"]
            .as_str()
            .unwrap()
            .contains("ADR 0018")
    );
    format_use_contract::assert_sample_use(
        &report,
        &wire,
        &report.to_human(),
        &[
            format_use_contract::BARE,
            format_use_contract::OPAQUE,
            format_use_contract::GENERIC,
            format_use_contract::HTTPS,
        ],
    );
}

#[test]
fn resource_declared_consistency_uses_the_shared_denotation_and_guards() {
    use esdieff::*;
    for (format, constant, default, code) in [
        (
            "uuid",
            "F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6",
            "f81d4fae-7dec-11d0-a765-00a0c91e6bf6",
            None,
        ),
        (
            "uuid",
            "f81d4fae-7dec-11d0-a765-00a0c91e6bf6",
            "f81d4fae-7dec-11d0-a765-00a0c91e6bf7",
            Some("consistency.default_excluded"),
        ),
        (
            "uri",
            "https://EXAMPLE.test/",
            "https://example.test/",
            None,
        ),
    ] {
        let model=serde_json::json!({"info":{},"sdfData":{"V":{"type":"string","format":format,"const":constant,"default":default}}}).to_string();
        let cat = Catalogue::from_inputs(&[SourceInput::supplied("m", model.as_bytes())]).unwrap();
        let report = cat
            .check(CheckRequest::new().assessment(Assessment::ExpandedConsistency))
            .unwrap();
        let diagnostics = report.diagnostics();
        let warnings = diagnostics
            .conditions()
            .filter(|condition| condition.code().starts_with("consistency."))
            .map(|condition| condition.code())
            .collect::<Vec<_>>();
        if let Some(code) = code {
            assert!(warnings.contains(&code));
        } else {
            assert!(warnings.is_empty());
        }
        assert!(matches!(
            report.format_knowledge(format_use_contract::BARE),
            Field::Absent
        ));
    }
}
