//! Typed ownership and input limits follow ADR0025: Selected numeric samples.
use esdieff::{Catalogue, Field, JsonBuilder, JsonKind, Limits, SourceInput, SourceRole};

#[test]
fn constructor_array_metadata_shares_projection_and_decoding_work() {
    let ordinary = JsonBuilder::new();
    let bounded = JsonBuilder::with_limits(Limits::default().decoding_work(14)).unwrap();
    assert!(bounded.null().is_ok());
    assert!(ordinary.array(vec![ordinary.null().unwrap()]).is_ok());
    assert!(bounded.array(vec![ordinary.null().unwrap()]).is_err());
}

#[test]
fn constructor_map_keys_share_projection_and_decoding_work() {
    let ordinary = JsonBuilder::new();
    let bounded = JsonBuilder::with_limits(Limits::default().decoding_work(26)).unwrap();
    assert!(bounded.null().is_ok());
    assert!(ordinary.map(vec![("a", ordinary.null().unwrap())]).is_ok());
    assert!(bounded.map(vec![("a", ordinary.null().unwrap())]).is_err());
}

#[test]
fn constructor_duplicate_comparisons_share_the_remaining_work() {
    let ordinary = JsonBuilder::new();
    let bounded = JsonBuilder::with_limits(Limits::default().decoding_work(49)).unwrap();
    assert!(bounded.null().is_ok());
    assert!(
        ordinary
            .map(vec![
                ("a", ordinary.null().unwrap()),
                ("b", ordinary.null().unwrap())
            ])
            .is_ok()
    );
    assert!(
        bounded
            .map(vec![
                ("a", ordinary.null().unwrap()),
                ("b", ordinary.null().unwrap())
            ])
            .is_err()
    );
}

#[test]
fn constructor_text_scan_shares_projection_and_decoding_work() {
    let ordinary = JsonBuilder::new();
    let bounded = JsonBuilder::with_limits(Limits::default().decoding_work(9)).unwrap();
    assert!(bounded.null().is_ok());
    assert_eq!(
        ordinary.text("a").unwrap().view().text().require().unwrap(),
        "a"
    );
    assert!(bounded.text("a").is_err());
}

#[test]
fn byte_sample_is_one_json_value_with_its_original_span() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("model", b"{}"),
        SourceInput::supplied("sample", b"101").sample(),
    ])
    .unwrap();
    let sample = catalogue
        .source(catalogue.source_id("sample").unwrap())
        .unwrap();
    assert_eq!(sample.role(), SourceRole::Sample);
    let value = sample.sample_value().require().unwrap();
    assert_eq!(value.kind(), JsonKind::Number);
    assert_eq!(value.number().require().unwrap().lexeme(), "101");
    assert_eq!(value.span(), Some(0..3));
    assert!(sample.has_original_bytes());
}
#[test]
fn typed_constructors_reject_duplicate_names_and_precharge_aggregate_limits() {
    let builder = JsonBuilder::new();
    assert!(
        builder
            .map(vec![
                ("a", builder.null().unwrap()),
                ("a", builder.boolean(true).unwrap())
            ])
            .is_err()
    );
    for bad in [
        "1 2",
        " 1",
        "1 ",
        "01",
        "NaN",
        "Infinity",
        "+1",
        "1e1000001",
    ] {
        assert!(builder.number(bad).is_err(), "{bad}");
    }
    let bounded = JsonBuilder::with_limits(Limits::default().document_nodes(2)).unwrap();
    assert!(
        bounded
            .array(vec![builder.null().unwrap(), builder.null().unwrap()])
            .is_err()
    );
    let text = builder.text("Å\n\"\\").unwrap();
    assert_eq!(text.view().text().require().unwrap(), "Å\n\"\\");
    assert_eq!(
        builder
            .i64(i64::MIN)
            .unwrap()
            .view()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "-9223372036854775808"
    );
    assert_eq!(
        builder
            .i64(i64::MAX)
            .unwrap()
            .view()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "9223372036854775807"
    );
    let map = builder
        .map(vec![("a/b~c", builder.u64(u64::MAX).unwrap())])
        .unwrap();
    let catalogue = Catalogue::take_inputs(vec![SourceInput::typed("s", map)]).unwrap();
    let value = catalogue
        .sources()
        .next()
        .unwrap()
        .sample_value()
        .require()
        .unwrap()
        .get("a/b~c")
        .require()
        .unwrap();
    assert_eq!(value.pointer(8192).require().unwrap(), "/a~1b~0c");
    assert_eq!(value.span(), None);
    assert_eq!(
        value.number().require().unwrap().lexeme(),
        "18446744073709551615"
    );
}

#[test]
fn consuming_and_borrowed_typed_inputs_never_expose_projection_spans() {
    let builder = JsonBuilder::new();
    let number = builder.number("9007199254740993").unwrap();
    let nested = builder
        .array(vec![number, builder.null().unwrap()])
        .unwrap();
    let value = builder.map(vec![("values", nested)]).unwrap();
    assert_eq!(
        value
            .view()
            .get("values")
            .require()
            .unwrap()
            .at(0)
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "9007199254740993"
    );
    let inputs = vec![
        SourceInput::supplied("model", b"{}"),
        SourceInput::typed("sample", value),
    ];
    let borrowed = Catalogue::from_inputs(&inputs).unwrap();
    let consumed = Catalogue::take_inputs(inputs).unwrap();
    for catalogue in [&borrowed, &consumed] {
        let source = catalogue
            .source(catalogue.source_id("sample").unwrap())
            .unwrap();
        assert!(!source.has_original_bytes());
        assert!(matches!(source.json(), Field::Unavailable(_)));
        let root = source.sample_value().require().unwrap();
        assert_eq!(root.span(), None);
        let array = root.get("values").require().unwrap();
        assert_eq!(array.span(), None);
        for child in array.children() {
            assert_eq!(child.span(), None);
        }
        assert_eq!(
            array
                .at(0)
                .require()
                .unwrap()
                .number()
                .require()
                .unwrap()
                .lexeme(),
            "9007199254740993"
        );
        assert_eq!(source.raw_bytes(), br#"{"values":[9007199254740993,null]}"#);
    }
}
