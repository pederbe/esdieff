//! Assess a supplied catalogue and read effective values through public views.
use esdieff::{Assessment, Catalogue, CheckRequest, Class, EffectiveRequest, Lookup, SourceInput};

fn main() {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied(
            "device",
            include_bytes!("../tests/fixtures/coverage/native-1.json"),
        ),
        SourceInput::supplied(
            "controller",
            include_bytes!("../tests/fixtures/coverage/native-2.json"),
        ),
        SourceInput::supplied(
            "level",
            include_bytes!("../tests/fixtures/coverage/native-3.json"),
        ),
    ])
    .unwrap();
    let initial = catalogue.check(CheckRequest::new()).unwrap();
    assert!(initial.outcome().complete());
    assert_eq!(initial.outcome().errors(), 0);
    println!("Initial complete: {}", initial.outcome().complete());

    let controller_id = catalogue.source_id("controller").unwrap();
    let effective = catalogue
        .effective(EffectiveRequest::new(controller_id))
        .unwrap();
    let Lookup::Found(controller) = effective.definition(Class::Object, "Controller") else {
        panic!("Controller missing")
    };
    let level = controller
        .children(Class::Property)
        .require()
        .unwrap()
        .find(|(name, _)| *name == "level")
        .unwrap()
        .1
        .require()
        .unwrap();
    let description = level.data_description().require().unwrap();
    assert_eq!(description.minimum().require().unwrap().lexeme(), "0");
    assert_eq!(description.maximum().require().unwrap().lexeme(), "80");
    println!(
        "Effective Controller.level range: {}..{}",
        description.minimum().require().unwrap().lexeme(),
        description.maximum().require().unwrap().lexeme()
    );

    let source = catalogue
        .source(catalogue.source_id("level").unwrap())
        .unwrap();
    let Lookup::Found(level) = source.definition(Class::Data, "Level") else {
        panic!("Level missing")
    };
    assert_eq!(
        level
            .data_description()
            .require()
            .unwrap()
            .maximum()
            .require()
            .unwrap()
            .lexeme(),
        "100"
    );

    let full = catalogue
        .check(CheckRequest::new().assessment(Assessment::FullBaseline))
        .unwrap();
    let coverage = full.requirement_coverage().require().unwrap();
    assert_eq!(
        full.requested_coverage().inventory(),
        Some("esdieff.full-model/1")
    );
    assert_eq!(coverage.inventory(), Some("rfc9880.model/1"));
    assert_eq!(coverage.total(), Some(54));
    assert!(coverage.unassessed() + coverage.undetermined() > 0);
    for member in full.requirement_descriptors().require().unwrap() {
        let state = full.requirement_status(member.id()).require().unwrap();
        println!(
            "{}: {} ({})",
            member.id(),
            state.as_str(),
            member.assessment_area()
        );
    }
}
