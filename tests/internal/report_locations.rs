//! Typed primary and related condition locations agree with the report JSON, following
//! ADR 0010: Domain-oriented library and caller simplicity.
use super::location_support;
use super::map_support;
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, Field, Limits, ReportLocation, SourceInput,
};

const UNIQUE_MODEL: &[u8] =
    br#"{"sdfProperty":{"p":{"type":"array","uniqueItems":true,"items":{"type":"number"}}}}"#;

#[test]
fn rejected_sample_condition_locates_its_sample_with_related_constraint_and_sample() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("m", UNIQUE_MODEL),
        SourceInput::supplied("s", b"[1,1]").sample(),
    ])
    .unwrap();
    let model = catalogue.source_id("m").unwrap();
    let sample = catalogue.source_id("s").unwrap();
    let report = map_support::selected(&catalogue);
    assert_eq!(
        report.sample_result().unwrap().admission(),
        Admission::Rejected
    );
    let diagnostics = report.diagnostics();
    location_support::assert_all_agree(&diagnostics, &report.to_json());
    let duplicate = diagnostics
        .conditions()
        .find(|condition| condition.code() == "sample.unique")
        .unwrap();
    let Field::Present(ReportLocation::Sample(second)) = duplicate.primary() else {
        panic!("second duplicate element is primary");
    };
    assert_eq!(second.source_id(), sample);
    assert!(matches!(second.pointer(), Field::Present("/1")));
    assert_eq!(second.span(), Some(3..4));
    let related: Vec<_> = duplicate.related().collect();
    assert_eq!(related.len(), 3);
    let ReportLocation::Effective(effective) = &related[0] else {
        panic!("effective constraint first");
    };
    assert_eq!(effective.root_document(), model);
    assert!(matches!(
        effective.pointer(),
        Field::Present("/sdfProperty/p/uniqueItems")
    ));
    let ReportLocation::Source(source) = &related[1] else {
        panic!("authored constraint second");
    };
    assert_eq!(source.source_id(), model);
    assert!(matches!(
        source.pointer(),
        Field::Present("/sdfProperty/p/uniqueItems")
    ));
    let span = source.span().unwrap();
    assert_eq!(
        &UNIQUE_MODEL[span.start as usize..span.end as usize],
        b"true"
    );
    let ReportLocation::Sample(first) = &related[2] else {
        panic!("first duplicate element last");
    };
    assert_eq!(first.source_id(), sample);
    assert!(matches!(first.pointer(), Field::Present("/0")));
    assert_eq!(first.span(), Some(1..2));
}

#[test]
fn inherited_model_condition_is_located_effectively_and_relates_its_authored_source() {
    let source =
        br##"{"info":{},"sdfData":{"d":{"pattern":"["}},"sdfProperty":{"p":{"sdfRef":"#/sdfData/d"}}}"##;
    let catalogue = Catalogue::from_inputs(&[SourceInput::supplied("m", source)]).unwrap();
    let model = catalogue.source_id("m").unwrap();
    let report = catalogue
        .check(CheckRequest::new().assessment(Assessment::ExpandedPatternSyntax))
        .unwrap();
    let diagnostics = report.diagnostics();
    location_support::assert_all_agree(&diagnostics, &report.to_json());
    let (inherited, primary) = diagnostics
        .conditions()
        .filter(|condition| condition.code() == "pattern.syntax")
        .find_map(|condition| match condition.primary() {
            Field::Present(ReportLocation::Effective(location))
                if matches!(location.pointer(), Field::Present("/sdfProperty/p/pattern")) =>
            {
                Some((condition, location))
            }
            _ => None,
        })
        .expect("inherited pattern finding");
    assert_eq!(primary.root_document(), model);
    let related: Vec<_> = inherited.related().collect();
    let ReportLocation::Source(location) = &related[0] else {
        panic!("authored pattern source");
    };
    assert_eq!(location.source_id(), model);
    assert!(matches!(
        location.pointer(),
        Field::Present("/sdfData/d/pattern")
    ));
    let span = location.span().unwrap();
    assert_eq!(&source[span.start as usize..span.end as usize], br#""[""#);
}

#[test]
fn omitted_related_pointers_stay_unavailable_with_their_identity_and_spans() {
    let catalogue = Catalogue::take_inputs_with_limits(
        vec![
            SourceInput::supplied("m", UNIQUE_MODEL),
            SourceInput::supplied("s", b"[1,1]").sample(),
        ],
        Limits::default().pointer_output_bytes(1),
    )
    .unwrap();
    let report = map_support::selected(&catalogue);
    let diagnostics = report.diagnostics();
    location_support::assert_all_agree(&diagnostics, &report.to_json());
    let duplicate = diagnostics
        .conditions()
        .find(|condition| condition.code() == "sample.unique")
        .unwrap();
    let related: Vec<_> = duplicate.related().collect();
    let ReportLocation::Effective(effective) = &related[0] else {
        panic!("effective constraint first");
    };
    assert!(matches!(effective.pointer(), Field::Unavailable(_)));
    assert_eq!(effective.root_document(), catalogue.source_id("m").unwrap());
    let ReportLocation::Source(source) = &related[1] else {
        panic!("authored constraint second");
    };
    assert!(matches!(source.pointer(), Field::Unavailable(_)));
    assert!(source.span().is_some());
}
