//! Guard exact retained input and operational limits through public source views.
//! Follows ADR0012: Exact numbers and JSON input policies, and
//! ADR0009: Bounded processing and reporting.

use esdieff::{
    Catalogue, CheckRequest, Class, Field, InspectRequest, Limits, NumberWork, SourceInput,
};
use std::cmp::Ordering;

#[test]
fn exact_numbers_keep_lexemes_order_and_checked_conversion() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "numbers",
        br#"{"a":0.1,"b":0.1000,"c":10e1000000,"d":1e-1000000,"e":1.00e2,"z":-0}"#,
    )])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    let root = source.json().require().unwrap();
    let a = root.get("a").require().unwrap().number().require().unwrap();
    let b = root.get("b").require().unwrap().number().require().unwrap();
    assert_eq!(a.lexeme(), "0.1");
    assert_eq!(
        a.compare_with_work(b, &mut NumberWork::new(2).unwrap())
            .unwrap(),
        Ordering::Equal
    );
    assert!(a.to_i64().is_err());
    assert!(!a.is_integer());
    let huge = root.get("c").require().unwrap().number().require().unwrap();
    assert_eq!(huge.lexeme(), "10e1000000");
    assert!(huge.is_integer());
    assert!(huge.to_i64().is_err());
    assert_eq!(
        root.get("e")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .to_i64()
            .unwrap(),
        100
    );
    assert_eq!(
        root.get("z")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .sign(),
        Ordering::Equal
    );
    let report = catalogue
        .inspect(InspectRequest::new(source.id(), ""))
        .unwrap();
    assert!(report.to_json().contains("\"c\":10e1000000"));
}

#[test]
fn duplicate_decoded_names_are_refused_even_in_opaque_content() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "duplicate",
        br#"{"vendor:opaque":{"name":1,"\u006eame":2}}"#,
    )])
    .unwrap();
    assert!(
        catalogue
            .sources()
            .next()
            .unwrap()
            .json()
            .present()
            .is_none()
    );
    let report = catalogue.check(CheckRequest::new()).unwrap();
    assert_eq!(report.outcome().errors(), 1);
    let wire: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let condition = wire["diagnostics"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["code"] == "input.duplicate_name")
        .unwrap();
    assert_eq!(condition["basis"]["kind"], "acceptance");
    assert_eq!(condition["basis"]["id"], "A002");
}

#[test]
fn unsupported_unicode_and_numbers_keep_raw_content_and_independent_views() {
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "partial",
        br#"{"paired":"\uD834\uDD1E","unknown":"\uD800","limit":1e1000001,"known":42}"#,
    )])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    let root = source.json().require().unwrap();
    assert_eq!(
        root.get("paired")
            .require()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "𝄞"
    );
    assert!(matches!(
        root.get("unknown").require().unwrap().text(),
        Field::Unavailable(_)
    ));
    assert_eq!(
        root.get("unknown").require().unwrap().raw_json(),
        r#""\uD800""#
    );
    assert!(matches!(
        root.get("limit").require().unwrap().number(),
        Field::Unavailable(_)
    ));
    assert!(
        catalogue
            .inspect(InspectRequest::new(source.id(), "/known"))
            .unwrap()
            .outcome()
            .complete()
    );
    assert!(
        !catalogue
            .inspect(InspectRequest::new(source.id(), "/unknown"))
            .unwrap()
            .outcome()
            .complete()
    );
    assert_eq!(
        catalogue
            .check(CheckRequest::new())
            .unwrap()
            .outcome()
            .errors(),
        0
    );
}

#[test]
fn configured_depth_nodes_and_allocations_stop_work_without_rfc_errors() {
    for limits in [
        Limits::default().depth(1),
        Limits::default().document_nodes(2),
        Limits::default().parsed_nodes(2),
        Limits::default().parsed_allocation_bytes(1),
        Limits::default().decoding_work(1),
    ] {
        let catalogue = Catalogue::with_limits(
            &[SourceInput::supplied("limited", b"{\"a\":{\"b\":{}}}")],
            limits,
        )
        .unwrap();
        let report = catalogue.check(CheckRequest::new()).unwrap();
        assert_eq!(report.outcome().errors(), 0);
        assert!(!report.outcome().complete());
        assert!(report.to_json().contains("operation.limit"));
    }
    assert!(
        Catalogue::with_limits(
            &[SourceInput::supplied("x", b"{}")],
            Limits::default().depth(65)
        )
        .is_err()
    );
    assert!(
        Catalogue::with_limits(
            &[SourceInput::supplied("x", b"{}")],
            Limits::default().numeric_work(0)
        )
        .is_err()
    );
}

fn nested_source(depth: usize, map: bool, empty: bool) -> String {
    let mut value = if empty {
        if map { "{}" } else { "[]" }.to_owned()
    } else {
        "42".to_owned()
    };
    for _ in usize::from(empty)..depth {
        value = if map {
            format!("{{\"child\":{value}}}")
        } else {
            format!("[{value}]")
        };
    }
    format!("{{\"nested\":{value}}}")
}

#[test]
fn container_depth_admits_empty_containers_and_their_scalar_children() {
    for depth in [1, 64] {
        for map in [true, false] {
            for empty in [true, false] {
                let text = nested_source(depth, map, empty);
                let catalogue = Catalogue::with_limits(
                    &[SourceInput::supplied("boundary", text.as_bytes())],
                    if depth == 64 {
                        Limits::default()
                    } else {
                        Limits::default().depth(depth)
                    },
                )
                .unwrap();
                let source = catalogue.sources().next().unwrap();
                assert!(source.json().present().is_some(), "{depth}/{map}/{empty}");
                let report = catalogue
                    .inspect(InspectRequest::new(source.id(), ""))
                    .unwrap();
                assert!(report.outcome().complete(), "{depth}/{map}/{empty}");
                assert_eq!(report.outcome().errors(), 0);
                assert_eq!(report.inspection_json().unwrap().raw_json(), text);
            }
        }
    }
    let catalogue = Catalogue::with_limits(
        &[SourceInput::supplied(
            "scalars",
            br#"{"a":null,"b":true,"c":"text","d":0.1}"#,
        )],
        Limits::default().depth(1),
    )
    .unwrap();
    assert!(
        catalogue
            .sources()
            .next()
            .unwrap()
            .json()
            .present()
            .is_some()
    );
}

#[test]
fn additional_containers_stop_operationally_and_preserve_independent_findings() {
    for depth in [1, 64] {
        for map in [true, false] {
            let text = nested_source(depth + 1, map, true);
            let catalogue = Catalogue::with_limits(
                &[
                    SourceInput::supplied("too-deep", text.as_bytes()),
                    SourceInput::supplied("independent", br#"{"a":1,"a":2}"#),
                ],
                Limits::default().depth(depth),
            )
            .unwrap();
            let source = catalogue.sources().next().unwrap();
            assert!(matches!(source.json(), Field::Unavailable(_)));
            let report = catalogue.check(CheckRequest::new()).unwrap();
            assert_eq!(report.outcome().errors(), 1);
            assert!(!report.outcome().complete());
            let diagnostics = report.diagnostics();
            assert!(
                diagnostics
                    .conditions()
                    .any(|item| item.code() == "operation.limit")
            );
            assert!(
                diagnostics
                    .conditions()
                    .any(|item| item.code() == "input.duplicate_name")
            );
        }
    }
}

#[test]
fn metadata_preflight_rejects_identity_loss_and_duplicate_keys() {
    let key = "x".repeat(257);
    assert!(Catalogue::from_inputs(&[SourceInput::supplied(&key, b"{}")]).is_err());
    assert!(
        Catalogue::from_inputs(&[
            SourceInput::supplied("x", b"{}"),
            SourceInput::supplied("x", b"{}")
        ])
        .is_err()
    );
    let uri = "u".repeat(2049);
    assert!(
        Catalogue::from_inputs(&[SourceInput::supplied("x", b"{}").acquisition_uri(&uri)]).is_err()
    );
    assert!(
        Catalogue::with_limits(
            &[SourceInput::supplied("x", b"{}")],
            Limits::default().report_bytes(100)
        )
        .is_err()
    );
}

#[test]
fn malformed_source_fields_stay_unavailable_and_defaults_stay_absent() {
    let catalogue=Catalogue::from_inputs(&[SourceInput::supplied("fields",br#"{"sdfProperty":{"P":{"writable":"yes","type":"vendor:temperature"},"Q":{"type":"mystery"}},"sdfAction":{"A":{"sdfInputData":0}}}"#)]).unwrap();
    let source = catalogue.sources().next().unwrap();
    let mut properties = source.definitions(Class::Property).require().unwrap();
    let p = properties.next().unwrap().1;
    assert!(matches!(p.writable(), Field::Unavailable(_)));
    assert!(matches!(p.readable(), Field::Absent));
    assert!(matches!(
        p.data_description().require().unwrap().data_type(),
        Field::Present(esdieff::DataType::Extended("vendor:temperature"))
    ));
    let q = properties.next().unwrap().1;
    assert!(matches!(
        q.data_description().require().unwrap().data_type(),
        Field::Present(esdieff::DataType::Undetermined("mystery"))
    ));
    assert!(matches!(
        source
            .definitions(Class::Action)
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .input(),
        Field::Unavailable(_)
    ));
}

#[test]
fn source_paths_use_compact_ancestry_and_render_with_escaping_limits() {
    let text = format!("{{\"{}\":{{\"a/b~c\":[42]}}}}", "x".repeat(100));
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("path", text.as_bytes())]).unwrap();
    let leaf = catalogue
        .sources()
        .next()
        .unwrap()
        .json()
        .require()
        .unwrap()
        .get(&"x".repeat(100))
        .require()
        .unwrap()
        .get("a/b~c")
        .require()
        .unwrap()
        .at(0)
        .require()
        .unwrap();
    assert_eq!(
        leaf.pointer().require().unwrap(),
        format!("/{}/a~1b~0c/0", "x".repeat(100))
    );
    assert!(matches!(leaf.pointer_with_limit(10), Field::Unavailable(_)));
    assert_eq!(leaf.raw_json(), "42");
}

#[test]
fn owned_buffer_capacity_is_bounded_and_charged_across_the_catalogue() {
    let mut oversized = Vec::with_capacity(32);
    oversized.extend_from_slice(b"{}");
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![
            SourceInput::owned("large-capacity", oversized),
            SourceInput::supplied("other", b"{}"),
        ],
        Limits::default().document_bytes(4).catalogue_bytes(4),
    )
    .unwrap();
    assert!(catalogue.sources().all(|source| source.content_complete()));
    let mut bounded = Vec::with_capacity(4);
    bounded.extend_from_slice(b"{}");
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![
            SourceInput::owned("bounded-capacity", bounded),
            SourceInput::supplied("other", b"{}"),
        ],
        Limits::default().document_bytes(4).catalogue_bytes(4),
    )
    .unwrap();
    assert_eq!(catalogue.sources().next().unwrap().raw_bytes(), b"{}");
    assert!(!catalogue.sources().nth(1).unwrap().content_complete());
}
