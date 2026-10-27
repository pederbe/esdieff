//! Verify owned catalogue inputs and authored projections through the public caller interface.
//! Follows ADR0004: Supplied catalogues and checking scope, and
//! ADR0010: Domain-oriented library and caller simplicity.

use esdieff::{Catalogue, SourceInput};

#[test]
fn supplied_sources_are_owned_and_remain_distinct() {
    let catalogue = {
        let text = String::from("{\"sdfData\":{\"Count\":{\"type\":\"integer\"}}}");
        Catalogue::from_inputs(&[
            SourceInput::supplied("first", text.as_bytes()),
            SourceInput::supplied("second", text.as_bytes()),
        ])
        .unwrap()
    };
    let sources: Vec<_> = catalogue.sources().collect();
    assert_eq!(sources.len(), 2);
    assert_ne!(sources[0].id(), sources[1].id());
    assert_eq!(sources[0].key(), "first");
    assert_eq!(sources[0].acquisition(), esdieff::Acquisition::Supplied);
    assert_eq!(
        sources[0].raw_bytes(),
        b"{\"sdfData\":{\"Count\":{\"type\":\"integer\"}}}"
    );
}

#[test]
fn one_map_is_required_and_failed_sources_remain_inspectable_as_bytes() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("map", b"{\"opaque\":null}"),
        SourceInput::supplied("array", b"[]"),
        SourceInput::supplied("syntax", b"{\"x\":1,}"),
        SourceInput::supplied("utf8", &[0xff]),
        SourceInput::failed("unreadable"),
    ])
    .unwrap();
    let sources: Vec<_> = catalogue.sources().collect();
    assert!(sources[0].json().present().is_some());
    for source in &sources[1..] {
        assert!(source.json().present().is_none());
    }
    assert_eq!(sources[3].raw_bytes(), &[0xff]);
    assert_eq!(sources[0].text(), Some("{\"opaque\":null}"));
    assert_eq!(sources[1].text(), Some("[]"));
    assert_eq!(sources[2].text(), Some("{\"x\":1,}"));
    assert_eq!(sources[3].text(), None);
    assert_eq!(sources[4].text(), Some(""));
    assert_eq!(sources[4].raw_bytes(), b"");
    assert_eq!(sources[4].acquisition(), esdieff::Acquisition::Failed);
}

#[test]
fn raw_source_views_preserve_unicode_escapes_containers_and_number_lexemes() {
    let text = " \n".to_owned()
        + r#"{"é":"温😀","esca\u0070ed":{ "values" : [true, null, -0.00, 1.2300e+02, "\uD834\uDD1E", "line\nquote\""] }}"#
        + "\n ";
    let catalogue =
        Catalogue::from_inputs(&[SourceInput::supplied("raw", text.as_bytes())]).unwrap();
    let source = catalogue.sources().next().unwrap();
    assert_eq!(source.text(), Some(text.as_str()));
    let root = source.json().require().unwrap();
    assert_eq!(root.raw_json(), text.trim());

    let unicode = root.get("é").require().unwrap();
    assert_eq!(unicode.raw_json(), r#""温😀""#);
    assert_eq!(unicode.text().require().unwrap(), "温😀");
    assert_eq!(unicode.span(), 8..17);

    let nested = root.get("escaped").require().unwrap();
    assert_eq!(
        nested.raw_json(),
        r#"{ "values" : [true, null, -0.00, 1.2300e+02, "\uD834\uDD1E", "line\nquote\""] }"#
    );
    let values = nested.get("values").require().unwrap();
    assert_eq!(
        values.raw_json(),
        r#"[true, null, -0.00, 1.2300e+02, "\uD834\uDD1E", "line\nquote\""]"#
    );
    let raw_values: Vec<_> = values.children().map(|value| value.raw_json()).collect();
    assert_eq!(
        raw_values,
        [
            "true",
            "null",
            "-0.00",
            "1.2300e+02",
            r#""\uD834\uDD1E""#,
            r#""line\nquote\"""#,
        ]
    );
    for (index, lexeme) in [(2, "-0.00"), (3, "1.2300e+02")] {
        assert_eq!(
            values
                .at(index)
                .require()
                .unwrap()
                .number()
                .require()
                .unwrap()
                .lexeme(),
            lexeme
        );
    }
    assert_eq!(
        values.at(4).require().unwrap().text().require().unwrap(),
        "𝄞"
    );
    assert_eq!(
        values.at(5).require().unwrap().text().require().unwrap(),
        "line\nquote\""
    );
}

#[test]
fn all_six_source_classes_and_opaque_content_are_accessible() {
    use esdieff::{Class, JsonKind};
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "six",
        br#"{
        "sdfThing":{"Device":{}},"sdfObject":{"Sensor":{}},
        "sdfProperty":{"Temperature":{"type":"number","vendor:calibration":{"delta":0.1}}},
        "sdfAction":{"Set":{}},"sdfEvent":{"Changed":{}},"sdfData":{"Count":{}}
    }"#,
    )])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    for (class, name) in [
        (Class::Thing, "Device"),
        (Class::Object, "Sensor"),
        (Class::Property, "Temperature"),
        (Class::Action, "Set"),
        (Class::Event, "Changed"),
        (Class::Data, "Count"),
    ] {
        let definitions: Vec<_> = source.definitions(class).require().unwrap().collect();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].0, name);
        assert_eq!(definitions[0].1.class(), class);
    }
    let property = source
        .definitions(Class::Property)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1;
    assert_eq!(
        property
            .json()
            .get("vendor:calibration")
            .require()
            .unwrap()
            .kind(),
        JsonKind::Map
    );
    assert_eq!(
        property
            .json()
            .get("vendor:calibration")
            .require()
            .unwrap()
            .get("delta")
            .require()
            .unwrap()
            .number()
            .require()
            .unwrap()
            .lexeme(),
        "0.1"
    );
}

#[test]
fn source_item_views_keep_restricted_roles_and_do_not_invent_full_meaning() {
    use esdieff::{Class, DataRole, Field};
    let catalogue=Catalogue::from_inputs(&[SourceInput::supplied("items",br#"{"sdfData":{"Values":{"type":"array","items":{"type":"array","nullable":false,"default":[]}}}}"#)]).unwrap();
    let item = catalogue
        .sources()
        .next()
        .unwrap()
        .definitions(Class::Data)
        .require()
        .unwrap()
        .next()
        .unwrap()
        .1
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert_eq!(item.role(), DataRole::DirectItem);
    assert!(matches!(item.data_type(), Field::Unavailable(_)));
    assert!(matches!(item.nullable(), Field::Unavailable(_)));
    assert!(
        !item
            .json()
            .get("nullable")
            .require()
            .unwrap()
            .boolean()
            .require()
            .unwrap()
    );
}

#[test]
fn consuming_owned_buffers_preserves_metadata_and_content() {
    let catalogue = Catalogue::take_inputs(vec![
        SourceInput::owned("owned", b"{\"x\":1}".to_vec()).version_token("v1"),
    ])
    .unwrap();
    let source = catalogue.sources().next().unwrap();
    assert_eq!(source.key(), "owned");
    assert_eq!(source.acquisition(), esdieff::Acquisition::Supplied);
    assert_eq!(source.version_token(), Some("v1"));
    assert_eq!(source.raw_bytes(), b"{\"x\":1}");
    assert!(source.content_complete());
}

#[test]
fn required_entries_need_established_object_meaning_in_direct_items() {
    use esdieff::{Class, Field};
    for authored_type in [
        r#""type":"number","#,
        r#""type":"string","#,
        r#""type":"array","#,
        r#""type":"mystery","#,
        "",
    ] {
        let text = format!(
            r#"{{"sdfData":{{"Values":{{"type":"array","items":{{{authored_type}"required":["entry"],"properties":{{"entry":{{"type":"number"}}}}}}}}}}}}"#
        );
        let catalogue =
            Catalogue::from_inputs(&[SourceInput::supplied("items", text.as_bytes())]).unwrap();
        let item = catalogue
            .sources()
            .next()
            .unwrap()
            .definitions(Class::Data)
            .require()
            .unwrap()
            .next()
            .unwrap()
            .1
            .data_description()
            .require()
            .unwrap()
            .items()
            .require()
            .unwrap();
        assert!(
            matches!(item.required_entries(), Field::Unavailable(_)),
            "{authored_type}"
        );
        assert!(
            matches!(item.properties(), Field::Unavailable(_)),
            "{authored_type}"
        );
        assert_eq!(
            item.json().get("required").require().unwrap().raw_json(),
            r#"["entry"]"#
        );
    }
}

#[test]
fn object_required_syntax_absence_and_full_description_roles_remain_distinct() {
    use esdieff::{Class, Field};
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied(
        "roles",
        br#"{
        "sdfData":{
            "Object":{"items":{"type":"object","required":["entry"],"properties":{"entry":{}}}},
            "Absent":{"items":{"type":"number"}},
            "Malformed":{"items":{"type":"object","required":[1]}},
            "Full":{"type":"number","required":["entry"]}
        }
    }"#,
    )])
    .unwrap();
    let definitions: Vec<_> = catalogue
        .sources()
        .next()
        .unwrap()
        .definitions(Class::Data)
        .require()
        .unwrap()
        .collect();
    let object = definitions[0]
        .1
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    let mut entries = object.required_entries().require().unwrap();
    assert_eq!(entries.next().unwrap().text().require().unwrap(), "entry");
    assert!(entries.next().is_none());
    assert_eq!(
        object.properties().require().unwrap().next().unwrap().0,
        "entry"
    );
    let absent = definitions[1]
        .1
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(absent.required_entries(), Field::Absent));
    assert!(matches!(absent.properties(), Field::Absent));
    let malformed = definitions[2]
        .1
        .data_description()
        .require()
        .unwrap()
        .items()
        .require()
        .unwrap();
    assert!(matches!(
        malformed.required_entries(),
        Field::Unavailable(_)
    ));
    let full = definitions[3].1.data_description().require().unwrap();
    assert_eq!(
        full.required_entries()
            .require()
            .unwrap()
            .next()
            .unwrap()
            .text()
            .require()
            .unwrap(),
        "entry"
    );
}
