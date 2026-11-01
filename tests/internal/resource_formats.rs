use esdieff::{Admission, Catalogue, JsonBuilder, Limits, SampleKind, SampleRequest, SourceInput};
use serde_json::Value;

fn check<'a>(cat: &'a Catalogue) -> esdieff::Report<'a> {
    cat.check_sample(
        SampleRequest::new(cat.source_id("m").unwrap(), SampleKind::Data, "/sdfData/V")
            .sample(cat.source_id("s").unwrap()),
    )
    .unwrap()
}

fn assert_nine(report: &esdieff::Report<'_>) {
    let coverage = report.requested_coverage();
    assert_eq!(coverage.inventory(), Some("esdieff.sample/1"));
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
fn ordinary_uri_reference_and_uuid_conventional_cases() {
    use Admission::{Admitted as A, Rejected as R, Undetermined as U};
    // id, effective format, exact conventional JSON token, projected admission
    let cases: &[(&str, &str, &[u8], Admission)] = &[
        (
            "uri-fragment",
            "uri",
            br#""https://example.test/device#reading""#,
            A,
        ),
        ("uri-http", "uri", br#""http://example.test/""#, A),
        ("uri-mixed-scheme", "uri", br#""hTtPs://example.test/""#, A),
        (
            "uri-escaped-scheme",
            "uri",
            br#""https\u003a//example.test/""#,
            A,
        ),
        ("uri-empty-host", "uri", br#""https://""#, R),
        ("uri-nonauthority", "uri", br#""https:device""#, R),
        ("uri-userinfo-empty-host", "uri", br#""http://user@/""#, R),
        ("uri-userinfo", "uri", br#""https://user@example.test/""#, A),
        ("uri-empty-port", "uri", br#""http://example.test:/""#, A),
        ("uri-ipv6", "uri", br#""https://[2001:db8::1]/""#, A),
        ("uri-ipfuture", "uri", br#""https://[v1.a:b]/""#, A),
        ("uri-bad-brackets", "uri", br#""https://[arbitrary]/""#, R),
        ("uri-bad-port", "uri", br#""https://example.test:port/""#, R),
        ("uri-excess-at", "uri", br#""https://u@a@b/""#, R),
        (
            "uri-percent-octets",
            "uri",
            br#""https://example.test/%C3%A9""#,
            A,
        ),
        (
            "uri-invalid-escape",
            "uri",
            br#""https://example.test/%GG""#,
            R,
        ),
        (
            "uri-short-escape",
            "uri",
            br#""https://example.test/%1""#,
            R,
        ),
        ("uri-space", "uri", br#""https://example.test/a b""#, R),
        (
            "uri-literal-nonascii",
            "uri",
            "\"https://example.test/é\"".as_bytes(),
            R,
        ),
        ("uri-backslash", "uri", br#""https://example.test/a\\b""#, R),
        (
            "uri-two-fragments",
            "uri",
            br#""https://example.test/#a#b""#,
            R,
        ),
        ("uri-relative", "uri", br#""../device""#, R),
        ("uri-empty", "uri", br#""""#, R),
        (
            "uri-mailto-meaning",
            "uri",
            br#""mailto:operator@example.test""#,
            U,
        ),
        (
            "uri-urn-meaning",
            "uri",
            br#""urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#,
            U,
        ),
        ("uri-custom-meaning", "uri", br#""custom:device""#, U),
        (
            "reference-relative",
            "uri-reference",
            br#""../device?mode=1#reading""#,
            A,
        ),
        (
            "reference-network",
            "uri-reference",
            br#""//example.test/device""#,
            A,
        ),
        (
            "reference-absolute-path",
            "uri-reference",
            br#""/device""#,
            A,
        ),
        ("reference-query", "uri-reference", br#""?mode=1""#, A),
        ("reference-fragment", "uri-reference", br##""#reading""##, A),
        ("reference-empty", "uri-reference", br#""""#, A),
        (
            "reference-first-colon",
            "uri-reference",
            br#""1thing:part""#,
            R,
        ),
        (
            "reference-later-colon",
            "uri-reference",
            br#""./1thing:part""#,
            A,
        ),
        (
            "reference-short-percent",
            "uri-reference",
            br#""device%""#,
            R,
        ),
        (
            "reference-backslash",
            "uri-reference",
            br#""device\\reading""#,
            R,
        ),
        (
            "reference-http-empty-host",
            "uri-reference",
            br#""https://""#,
            R,
        ),
        (
            "reference-scheme-unknown",
            "uri-reference",
            br#""custom:device""#,
            U,
        ),
        (
            "uuid-lower",
            "uuid",
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#,
            A,
        ),
        (
            "uuid-mixed",
            "uuid",
            br#""F81d4FaE-7dEc-11D0-A765-00a0C91e6Bf6""#,
            A,
        ),
        (
            "uuid-escaped",
            "uuid",
            br#""\u006681d4fae-7dec-11d0-a765-00a0c91e6bf6""#,
            A,
        ),
        (
            "uuid-nil",
            "uuid",
            br#""00000000-0000-0000-0000-000000000000""#,
            A,
        ),
        (
            "uuid-max",
            "uuid",
            br#""ffffffff-ffff-ffff-ffff-ffffffffffff""#,
            A,
        ),
        (
            "uuid-v7-opaque",
            "uuid",
            br#""01890f3e-6c4a-7d4b-8e23-4d87b637659b""#,
            A,
        ),
        (
            "uuid-reserved-opaque",
            "uuid",
            br#""f81d4fae-7dec-f1d0-f765-00a0c91e6bf6""#,
            A,
        ),
        (
            "uuid-urn-channel",
            "uuid",
            br#""urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#,
            R,
        ),
        (
            "uuid-braces-channel",
            "uuid",
            br#""{f81d4fae-7dec-11d0-a765-00a0c91e6bf6}""#,
            R,
        ),
        (
            "uuid-undashed-channel",
            "uuid",
            br#""f81d4fae7dec11d0a76500a0c91e6bf6""#,
            R,
        ),
        (
            "uuid-space-channel",
            "uuid",
            br#"" f81d4fae-7dec-11d0-a765-00a0c91e6bf6""#,
            R,
        ),
        (
            "uuid-nonhex",
            "uuid",
            br#""g81d4fae-7dec-11d0-a765-00a0c91e6bf6""#,
            R,
        ),
        (
            "uuid-short",
            "uuid",
            br#""f81d4fae-7dec-11d0-a765-00a0c91e6bf""#,
            R,
        ),
        (
            "uuid-number-channel",
            "uuid",
            b"329800735698586629295641978511506172918",
            R,
        ),
        ("uuid-array-channel", "uuid", b"[248,29,79,174]", R),
    ];
    for &(id, format, sample, expected) in cases {
        let model = format!(
            r#"{{"info":{{}},"sdfData":{{"V":{{"type":"string","nullable":false,"format":"{format}"}}}}}}"#
        );
        let cat = Catalogue::from_inputs(&[
            SourceInput::supplied("m", model.as_bytes()),
            SourceInput::supplied("s", sample).sample(),
        ])
        .unwrap();
        let report = check(&cat);
        let result = report.sample_result().unwrap();
        assert_eq!(result.admission(), expected, "{id}");
        assert_nine(&report);
        assert_eq!(
            cat.source(cat.source_id("m").unwrap()).unwrap().raw_bytes(),
            model.as_bytes()
        );
        assert_eq!(
            cat.source(cat.source_id("s").unwrap()).unwrap().raw_bytes(),
            sample
        );
        assert_eq!(
            result.sample_value().require().unwrap().span(),
            Some(0..sample.len())
        );
        let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(wire["operation"], "sample");
        assert_eq!(wire["version"], "1.1");
        assert_eq!(wire["result"]["admission"], expected.as_str());
        assert_eq!(wire["coverage"]["requested"]["total"], 9);
        assert_eq!(wire["result"]["sample_location"]["kind"], "sample");
        assert_eq!(
            wire["result"]["sample_location"]["span"],
            serde_json::json!([0, sample.len()])
        );
        assert_eq!(wire["result"]["description"]["kind"], "effective");
        assert!(wire["result"]["description"].get("span").is_none());
        assert_eq!(report.outcome().complete(), expected != U, "{id}");
        assert_eq!(report.outcome().errors() > 0, expected == R, "{id}");
    }
}

#[test]
fn three_formats_preserve_unknown_context_and_finite_storage_outcomes() {
    for (format, sample) in [
        ("uri", br#""https://example.test/""#.as_slice()),
        ("uri-reference", br#""../device""#.as_slice()),
        (
            "uuid",
            br#""f81d4fae-7dec-f1d0-f765-00a0c91e6bf6""#.as_slice(),
        ),
    ] {
        let normal =
            format!(r#"{{"info":{{}},"sdfData":{{"V":{{"type":"string","format":"{format}"}}}}}}"#);
        let unknown = format!(
            r#"{{"info":{{"features":["https://example.test/unknown-critical"]}},"sdfData":{{"V":{{"type":"string","format":"{format}"}}}}}}"#
        );
        for (model, limits) in [
            (unknown.as_bytes(), Limits::default()),
            (
                normal.as_bytes(),
                Limits::default().effective_allocation_bytes(1),
            ),
        ] {
            let cat = Catalogue::with_limits(
                &[
                    SourceInput::supplied("m", model),
                    SourceInput::supplied("s", sample).sample(),
                ],
                limits,
            )
            .unwrap();
            let report = check(&cat);
            assert_eq!(
                report.sample_result().unwrap().admission(),
                Admission::Undetermined
            );
            assert!(!report.outcome().complete());
            assert_eq!(report.outcome().errors(), 0);
            assert_nine(&report);
        }
    }
}

#[test]
fn typed_uuid_has_no_original_token_span_and_escaped_bytes_do() {
    let model = br#"{"info":{},"sdfData":{"V":{"type":"string","format":"uuid"}}}"#;
    let builder = JsonBuilder::new();
    let cat = Catalogue::take_inputs(vec![
        SourceInput::supplied("m", model),
        SourceInput::typed(
            "s",
            builder
                .text("F81d4FaE-7dEc-11D0-A765-00a0C91e6Bf6")
                .unwrap(),
        ),
    ])
    .unwrap();
    let report = check(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .sample_value()
            .require()
            .unwrap()
            .span(),
        None
    );
    assert!(
        !cat.source(cat.source_id("s").unwrap())
            .unwrap()
            .has_original_bytes()
    );
    let wire: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert!(wire["result"]["sample_location"]["span"].is_null());
    let original = br#"  "\u006681d4fae-7dec-11d0-a765-00a0c91e6bf6" "#;
    let cat = Catalogue::from_inputs(&[
        SourceInput::supplied("m", model),
        SourceInput::supplied("s", original).sample(),
    ])
    .unwrap();
    let report = check(&cat);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Admitted
    );
    assert_eq!(
        report
            .sample_result()
            .unwrap()
            .sample_value()
            .require()
            .unwrap()
            .span(),
        Some(2..original.len() - 1)
    );
}
