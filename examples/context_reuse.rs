//! Check reused Context metadata and inspect its final values and nearest origins.
use esdieff::{
    Assessment, Catalogue, CheckRequest, Class, EffectiveRequest, Field, Lookup, OriginKind,
    SourceInput,
};

fn main() -> Result<(), &'static str> {
    let catalogue = Catalogue::from_inputs(&[
        SourceInput::supplied("base", include_bytes!("context-reuse/base.sdf.json")),
        SourceInput::supplied("device", include_bytes!("context-reuse/device.sdf.json")),
    ])?;
    let id = catalogue.source_id("device").ok_or("device source")?;
    let report = catalogue.check(
        CheckRequest::new()
            .root(id)
            .assessment(Assessment::FullBaseline),
    )?;
    assert!(report.outcome().complete(), "{}", report.to_human());
    assert_eq!(report.outcome().errors(), 0);

    let pointer = "/sdfThing/Installation/sdfObject/sensor";
    let model = catalogue.effective(EffectiveRequest::new(id).pointer(pointer))?;
    let Lookup::Found(calibration) = model.definition(Class::Context, "calibration") else {
        return Err("calibration Context");
    };
    let data = calibration.data_description().require()?;
    assert_eq!(data.number("minimum").require()?.lexeme(), "-10");
    assert_eq!(data.number("maximum").require()?.lexeme(), "5");
    assert!(matches!(calibration.readable(), Field::Absent));
    assert!(matches!(calibration.description(), Field::Absent));

    let maximum = model
        .explanation(&format!("{pointer}/sdfContext/calibration/maximum"))
        .require()?;
    assert_eq!(maximum.kind(), OriginKind::Override);
    let source = maximum.sources().next().ok_or("maximum origin")?;
    assert_eq!(source.source_id(), id);
    assert_eq!(
        source.pointer().require()?,
        "/sdfThing/Installation/sdfObject/sensor/sdfData/Offset/maximum"
    );
    assert_eq!(
        maximum
            .consumer()
            .ok_or("importing reference")?
            .pointer()
            .require()?,
        "/sdfObject/Sensor/sdfContext/calibration/sdfRef"
    );
    let deletion = model
        .explanation(&format!("{pointer}/sdfContext/retired"))
        .require()?;
    assert_eq!(deletion.kind(), OriginKind::Deleted);
    println!(
        "Selected model checked. Calibration maximum is 5 Cel, with its override and importing reference preserved."
    );
    Ok(())
}
