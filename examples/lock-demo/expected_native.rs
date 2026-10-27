//! Independently authored native expectations for lock-demo/1.
//! They restate the model's supported checks and each value's expected Sample-value checking
//! result literally. Nothing here is derived from the simulator, a trace or a native report.
use crate::expected::{PREFIX_TRACE, expected_steps};
use crate::native::{ModelCase, Role, SampleId};
use crate::scenario::Command;
use esdieff::{Admission, Assessment, SampleKind};

/// A supported complete Model check under strict warnings.
pub struct ModelExpectation {
    pub case: ModelCase,
    pub assessment: Assessment,
    pub inventory: &'static str,
    pub areas: u32,
}

pub const MODELS: [ModelExpectation; 2] = [
    ModelExpectation {
        case: ModelCase::InitialCatalogue,
        assessment: Assessment::Initial,
        inventory: "esdieff.initial/1",
        areas: 8,
    },
    ModelExpectation {
        case: ModelCase::FullSelected,
        assessment: Assessment::FullBaseline,
        inventory: "esdieff.full-model/1",
        areas: 13,
    },
];

pub const SAMPLE_INVENTORY: &str = "esdieff.sample/1";
pub const SAMPLE_AREAS: u32 = 9;

/// The condition that decides a rejection, located in the sample and in the model source.
#[derive(Clone, Debug)]
pub struct DecisiveDiagnostic {
    pub code: &'static str,
    pub sample_pointer: &'static str,
    pub source_pointer: &'static str,
}

/// Every expected sample is complete. Admitted samples pass and rejected samples fail.
#[derive(Clone, Debug)]
pub struct SampleExpectation {
    pub id: SampleId,
    pub kind: SampleKind,
    pub admission: Admission,
    pub decisive: Option<DecisiveDiagnostic>,
}

fn admitted(step: u32, role: Role) -> SampleExpectation {
    SampleExpectation {
        id: SampleId::new(step, role),
        kind: match role {
            Role::Input => SampleKind::ActionInput,
            Role::Output => SampleKind::ActionOutput,
            Role::Event => SampleKind::EventOutput,
            Role::Reading => SampleKind::Property,
        },
        admission: Admission::Admitted,
        decisive: None,
    }
}

fn rejected_input(
    step: u32,
    code: &'static str,
    sample_pointer: &'static str,
    source_pointer: &'static str,
) -> SampleExpectation {
    SampleExpectation {
        admission: Admission::Rejected,
        decisive: Some(DecisiveDiagnostic {
            code,
            sample_pointer,
            source_pointer,
        }),
        ..admitted(step, Role::Input)
    }
}

/// Native expectations for commands that begin with the literal prefix. Later steps follow the
/// authored rules' expected states: every value is admitted, and an Event is expected only
/// when the expected state changes.
pub fn samples(commands: &[Command]) -> Vec<SampleExpectation> {
    let mut samples = prefix_samples();
    for expected in expected_steps(commands).iter().skip(PREFIX_TRACE.len()) {
        // A command without an authored admission has no expected values to match.
        let Some(outcome) = expected.outcome() else {
            continue;
        };
        let step = expected.step;
        samples.push(if outcome.invoked {
            admitted(step, Role::Input)
        } else {
            SampleExpectation {
                admission: Admission::Rejected,
                ..admitted(step, Role::Input)
            }
        });
        if outcome.result.is_some() {
            samples.push(admitted(step, Role::Output));
        }
        if outcome.event.is_some() {
            samples.push(admitted(step, Role::Event));
        }
        samples.push(admitted(step, Role::Reading));
    }
    samples
}

/// Native expectations for the initial reading and the nine literal steps.
fn prefix_samples() -> Vec<SampleExpectation> {
    use Role::{Event, Input, Output, Reading};
    vec![
        admitted(0, Reading),
        admitted(1, Input),
        admitted(1, Output),
        admitted(1, Event),
        admitted(1, Reading),
        admitted(2, Input),
        admitted(2, Output),
        admitted(2, Reading),
        // nope is well-formed: four non-space characters.
        admitted(3, Input),
        admitted(3, Output),
        admitted(3, Reading),
        // abc has three Unicode scalars, below the credential minimum of four.
        rejected_input(
            4,
            "sample.length",
            "/credential",
            "/sdfObject/Lock/sdfAction/authorize/sdfInputData/properties/credential/minLength",
        ),
        admitted(4, Reading),
        // A present null credential meets required presence but the credential is not nullable.
        rejected_input(
            5,
            "sample.null",
            "/credential",
            "/sdfObject/Lock/sdfAction/authorize/sdfInputData/properties/credential/nullable",
        ),
        admitted(5, Reading),
        // operation is the second required entry, and the whole input map lacks it.
        rejected_input(
            6,
            "sample.required",
            "",
            "/sdfObject/Lock/sdfAction/authorize/sdfInputData/required/1",
        ),
        admitted(6, Reading),
        admitted(7, Input),
        admitted(7, Output),
        admitted(7, Event),
        admitted(7, Reading),
        admitted(8, Input),
        admitted(8, Output),
        admitted(8, Reading),
        admitted(9, Input),
        admitted(9, Output),
        admitted(9, Event),
        admitted(9, Reading),
    ]
}
