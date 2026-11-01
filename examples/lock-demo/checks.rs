//! Check fixed door-lock sample values through the public model operations.
use esdieff::{
    Admission, Assessment, Catalogue, CheckRequest, Disposition, SampleKind, SampleRequest,
    SourceInput,
};
use std::{error::Error, io::Write};

const MODEL: &[u8] = include_bytes!("../door-lock-check/rich.sdf.json");
const STATE: &str = "/sdfObject/Lock/sdfProperty/state";
const ACTION: &str = "/sdfObject/Lock/sdfAction/authorize";
const EVENT: &str = "/sdfObject/Lock/sdfEvent/stateChanged";

type Case = (
    &'static str,
    SampleKind,
    &'static str,
    &'static [u8],
    Admission,
);

const CASES: &[Case] = &[
    (
        "locked state",
        SampleKind::Property,
        STATE,
        br#""locked""#,
        Admission::Admitted,
    ),
    (
        "unknown state",
        SampleKind::Property,
        STATE,
        br#""open""#,
        Admission::Rejected,
    ),
    (
        "Action input",
        SampleKind::ActionInput,
        ACTION,
        include_bytes!("../door-lock-check/samples/authorize-unicode.json"),
        Admission::Admitted,
    ),
    (
        "missing operation",
        SampleKind::ActionInput,
        ACTION,
        include_bytes!("../door-lock-check/samples/authorize-missing-operation.json"),
        Admission::Rejected,
    ),
    (
        "Action output",
        SampleKind::ActionOutput,
        ACTION,
        include_bytes!("../door-lock-check/samples/result-accepted.json"),
        Admission::Admitted,
    ),
    (
        "wrong output type",
        SampleKind::ActionOutput,
        ACTION,
        include_bytes!("../door-lock-check/samples/result-wrong-type.json"),
        Admission::Rejected,
    ),
    (
        "Event value",
        SampleKind::EventOutput,
        EVENT,
        include_bytes!("../door-lock-check/samples/event-locked.json"),
        Admission::Admitted,
    ),
    (
        "impossible date",
        SampleKind::EventOutput,
        EVENT,
        include_bytes!("../door-lock-check/samples/event-invalid-date.json"),
        Admission::Rejected,
    ),
];

pub fn run(output: &mut impl Write) -> Result<(), Box<dyn Error>> {
    let mut inputs = vec![SourceInput::supplied("lock", MODEL)];
    inputs.extend(
        CASES
            .iter()
            .map(|(name, _, _, bytes, _)| SourceInput::supplied(name, bytes).sample()),
    );
    let catalogue = Catalogue::from_inputs(&inputs)?;
    let model = catalogue.source_id("lock").ok_or("missing lock model")?;
    let report = catalogue.check(
        CheckRequest::new()
            .root(model)
            .assessment(Assessment::FullBaseline)
            .strict_warnings(true),
    )?;
    if !report.outcome().complete() || report.outcome().disposition() != Disposition::Passed {
        return Err(format!("door-lock model check failed:\n{}", report.to_human()).into());
    }
    writeln!(output, "Door-lock model: passed")?;
    for (name, kind, pointer, _, expected) in CASES {
        let sample = catalogue.source_id(name).ok_or("missing sample")?;
        let report =
            catalogue.check_sample(SampleRequest::new(model, *kind, pointer).sample(sample))?;
        let admission = report
            .sample_result()
            .ok_or("missing sample result")?
            .admission();
        if !report.outcome().complete() || admission != *expected {
            return Err(format!(
                "{name}: expected {}, observed {}\n{}",
                expected.as_str(),
                admission.as_str(),
                report.to_human()
            )
            .into());
        }
        writeln!(
            output,
            "{name}: {} (expected {})",
            admission.as_str(),
            expected.as_str()
        )?;
    }
    writeln!(output, "{} sample expectations matched", CASES.len())?;
    Ok(())
}
