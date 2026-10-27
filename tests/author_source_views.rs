//! Generated SDF documents retain typed views under ADR0017: Reference-preserving sdfThing authoring.
use esdieff::{AuthorRequest, Catalogue, Class, Field, Lookup, SourceInput, SourceRole};

#[test]
fn generated_sources_expose_typed_definitions_while_samples_remain_separate() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "base",
            include_bytes!("fixtures/authoring/reusable.sdf.json"),
        ),
        SourceInput::supplied("sample", b"{}").sample(),
    ])
    .unwrap();
    let result = AuthorRequest::from_plan(
        catalogue,
        include_bytes!("fixtures/authoring/refine.plan.json"),
    )
    .unwrap()
    .author()
    .unwrap();
    let source = result
        .catalogue()
        .source(result.generated_source())
        .unwrap();
    assert_eq!(source.role(), SourceRole::Generated);
    assert!(matches!(
        source.definition(Class::Thing, "Room / Monitor"),
        Lookup::Found(_)
    ));
    assert_eq!(
        source.definitions(Class::Thing).require().unwrap().count(),
        1
    );

    let sample = result
        .catalogue()
        .source(result.catalogue().source_id("sample").unwrap())
        .unwrap();
    assert!(matches!(
        sample.definitions(Class::Thing),
        Field::Unavailable(_)
    ));
}
