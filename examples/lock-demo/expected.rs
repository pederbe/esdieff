//! Independently authored lock-demo/1 behavior expectations.
//! The initial conditions, the authored admission of each command form, the expected state
//! progression under the authored rules and the literal prefix table restate the replay
//! contract. Nothing here is derived from the simulator, an observed trace or a native report,
//! and nothing here depends on native checking.
use crate::scenario::{Command, Credential, LockState, Operation};

/// The authored initial state, stated independently of the simulator's starting value.
pub const INITIAL_STATE: LockState = LockState::Locked;

/// The authored admission of an Action input, never computed from a native result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputAdmission {
    Admitted,
    Rejected,
}

/// Look up a command's authored admission by its literal form. The three malformed prefix
/// forms are expected rejected, and well-formed demo and nope requests are expected admitted.
/// Any other form has no authored expectation.
pub fn input_admission(command: &Command) -> Option<InputAdmission> {
    use Credential::{Null, Token};
    use Operation::Lock;
    match (command.credential, command.operation) {
        (Token("demo" | "nope"), Some(_)) => Some(InputAdmission::Admitted),
        (Token("abc"), Some(Lock)) | (Null, Some(Lock)) | (Token("demo"), None) => {
            Some(InputAdmission::Rejected)
        }
        _ => None,
    }
}

/// The only token the authored policy accepts.
const ACCEPTED_TOKEN: &str = "demo";

/// What the authored rules expect of one command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expectation {
    /// A malformed input is never invoked.
    Rejected,
    /// A well-formed request without the demo token is refused.
    Refused,
    /// A demo request moves the lock to the requested state.
    Accepted(LockState),
}

/// One command step's expectation and the state it is expected to start from.
#[derive(Clone, Copy, Debug)]
pub struct ExpectedStep {
    pub step: u32,
    pub prior: LockState,
    /// Absent when the command form has no authored admission, so it cannot be judged.
    pub expectation: Option<Expectation>,
}
impl ExpectedStep {
    pub fn after(&self) -> LockState {
        match self.expectation {
            Some(Expectation::Accepted(requested)) => requested,
            _ => self.prior,
        }
    }

    /// The values the authored rules expect this step to show, when its command can be judged.
    pub fn outcome(&self) -> Option<Outcome> {
        let prior = self.prior;
        Some(match self.expectation? {
            Expectation::Rejected => Outcome {
                invoked: false,
                result: None,
                event: None,
                reading: prior,
            },
            Expectation::Refused => Outcome {
                invoked: true,
                result: Some((false, prior)),
                event: None,
                reading: prior,
            },
            Expectation::Accepted(requested) => Outcome {
                invoked: true,
                result: Some((true, requested)),
                event: (requested != prior).then_some(requested),
                reading: requested,
            },
        })
    }
}

/// A step's expected invocation, Action result, stateChanged Event and subsequent reading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Outcome {
    pub invoked: bool,
    /// Accepted flag and result state.
    pub result: Option<(bool, LockState)>,
    /// The new state an Event announces.
    pub event: Option<LockState>,
    pub reading: LockState,
}

/// Track the expected state from the authored initial state through every authored command.
pub fn expected_steps(commands: &[Command]) -> Vec<ExpectedStep> {
    let mut state = INITIAL_STATE;
    (1u32..)
        .zip(commands)
        .map(|(step, command)| {
            let expectation = match (input_admission(command), command.operation) {
                (None, _) => None,
                (Some(InputAdmission::Rejected), _) => Some(Expectation::Rejected),
                (Some(InputAdmission::Admitted), Some(operation))
                    if command.credential == Credential::Token(ACCEPTED_TOKEN) =>
                {
                    Some(Expectation::Accepted(match operation {
                        Operation::Lock => LockState::Locked,
                        Operation::Unlock => LockState::Unlocked,
                    }))
                }
                (Some(InputAdmission::Admitted), _) => Some(Expectation::Refused),
            };
            let expected = ExpectedStep {
                step,
                prior: state,
                expectation,
            };
            state = expected.after();
            expected
        })
        .collect()
}

/// One row of the literal prefix table, written as the observable values.
pub struct LiteralStep {
    pub step: u32,
    pub at: &'static str,
    pub input: &'static str,
    pub invoked: bool,
    /// Accepted flag and result state.
    pub result: Option<(bool, &'static str)>,
    /// Event state and timestamp.
    pub event: Option<(&'static str, &'static str)>,
    pub reading: &'static str,
}

/// The initial reading's time and state.
pub const INITIAL_READING: (&str, &str) = ("2024-01-01T12:00:00Z", "locked");

pub const PREFIX_TRACE: [LiteralStep; 9] = [
    LiteralStep {
        step: 1,
        at: "2024-01-01T12:00:01Z",
        input: r#"{"credential":"demo","operation":"unlock"}"#,
        invoked: true,
        result: Some((true, "unlocked")),
        event: Some(("unlocked", "2024-01-01T12:00:01Z")),
        reading: "unlocked",
    },
    LiteralStep {
        step: 2,
        at: "2024-01-01T12:00:02Z",
        input: r#"{"credential":"demo","operation":"unlock"}"#,
        invoked: true,
        result: Some((true, "unlocked")),
        event: None,
        reading: "unlocked",
    },
    LiteralStep {
        step: 3,
        at: "2024-01-01T12:00:03Z",
        input: r#"{"credential":"nope","operation":"lock"}"#,
        invoked: true,
        result: Some((false, "unlocked")),
        event: None,
        reading: "unlocked",
    },
    LiteralStep {
        step: 4,
        at: "2024-01-01T12:00:04Z",
        input: r#"{"credential":"abc","operation":"lock"}"#,
        invoked: false,
        result: None,
        event: None,
        reading: "unlocked",
    },
    LiteralStep {
        step: 5,
        at: "2024-01-01T12:00:05Z",
        input: r#"{"credential":null,"operation":"lock"}"#,
        invoked: false,
        result: None,
        event: None,
        reading: "unlocked",
    },
    LiteralStep {
        step: 6,
        at: "2024-01-01T12:00:06Z",
        input: r#"{"credential":"demo"}"#,
        invoked: false,
        result: None,
        event: None,
        reading: "unlocked",
    },
    LiteralStep {
        step: 7,
        at: "2024-01-01T12:00:07Z",
        input: r#"{"credential":"demo","operation":"lock"}"#,
        invoked: true,
        result: Some((true, "locked")),
        event: Some(("locked", "2024-01-01T12:00:07Z")),
        reading: "locked",
    },
    LiteralStep {
        step: 8,
        at: "2024-01-01T12:00:08Z",
        input: r#"{"credential":"demo","operation":"lock"}"#,
        invoked: true,
        result: Some((true, "locked")),
        event: None,
        reading: "locked",
    },
    LiteralStep {
        step: 9,
        at: "2024-01-01T12:00:09Z",
        input: r#"{"credential":"demo","operation":"unlock"}"#,
        invoked: true,
        result: Some((true, "unlocked")),
        event: Some(("unlocked", "2024-01-01T12:00:09Z")),
        reading: "unlocked",
    },
];
